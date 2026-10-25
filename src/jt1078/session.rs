use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::jt1078::frame::{parse_jt1078_frame, parse_jt1078_structured_frame, Jt1078Frame};

/// JT1078 session state for TCP/UDP peer.
/// Handles authentication (simple token), heartbeat timestamp, legacy reassembly buffer,
/// and ordered buffering for structured frames using sequence numbers.
#[derive(Debug)]
pub struct Jt1078Session {
    pub peer: SocketAddr,
    pub authenticated: bool,
    pub buffer: Vec<u8>,
    pub last_heartbeat: Instant,
    /// Optional session-level token override (avoids global env var races in tests)
    pub expected_token: Option<String>,
    /// Pending structured frames keyed by seq for reordering
    pub pending: BTreeMap<u16, Jt1078Frame>,
    /// Next expected sequence to deliver (None until first structured frame seen)
    pub expected_seq: Option<u16>,
    /// Maximum number of pending structured frames to hold (simple DoS protection)
    pub max_pending: usize,
    /// Timestamp of the last missing-sequence alert sent for rate-limiting
    pub last_missing_alert: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameKind {
    AuthSuccess,
    AuthFailure,
    Heartbeat,
    Data(Vec<u8>),
}

/// Parsed business message — emitted by Jt1078Session::process_jt_message
/// after stripping the JT808 framing. Phase 6.1-6.5.
#[derive(Debug, Clone)]
pub enum ParsedMessage {
    /// 0x0002 Heartbeat
    Heartbeat,
    /// 0x0100 Terminal register
    Register(crate::jt1078::response_parser::RegisterRequest),
    /// 0x0102 Terminal authentication（正文即鉴权码）
    Authenticate(String),
    /// 0x0102 Terminal attribute report（厂商非标用法，平台不按此处理）
    AttributeReport(crate::jt1078::response_parser::AttributeReport),
    /// 0x0200 Location report
    LocationReport(crate::jt1078::response_parser::LocationReport),
    /// 0x0801 Media retrieval first item
    MediaItem(crate::jt1078::response_parser::MediaItem),
    /// 0x0802 多媒体数据检索应答（终端录像/图片列表，0x8802 的应答）
    MediaSearchResult(Vec<crate::jt1078::response_parser::MediaSearchItem>),
    /// 0x0001 Generic common response
    CommonResponse {
        /// The serial_no of the original command being responded to
        reply_serial: u16,
        /// The msg_id being responded to
        reply_msg_id: u16,
        /// 0=success, 1=failure, 2=msg_error, 3=unsupported
        result: u8,
    },
    /// 0x0107 Query terminal params response
    QueryParamsResponse(Vec<crate::jt1078::response_parser::TerminalParam>),
    /// Any other unrecognised message — body preserved
    Unknown {
        msg_id: u16,
        serial: u16,
        body: Vec<u8>,
    },
}

impl Jt1078Session {
    pub fn new(peer: SocketAddr) -> Self {
        Self {
            peer,
            authenticated: false,
            buffer: Vec::with_capacity(8 * 1024),
            last_heartbeat: Instant::now(),
            expected_token: None,
            pending: BTreeMap::new(),
            expected_seq: None,
            max_pending: 256,
            last_missing_alert: None,
        }
    }

    /// Feed raw bytes into the session reassembly buffer and extract complete payloads.
    /// Supports both structured frames (with seq/timestamp/checksum) and legacy length-prefixed frames.
    /// For structured frames, performs simple in-order delivery using sequence numbers and an internal buffer.
    pub fn feed_bytes(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        // ── 接收缓冲封顶（未认证对端的 OOM 防线）────────────────────────
        //
        // 旧格式（4 字节大端长度前缀）可以声明一个**永远凑不齐**的长度
        // （例如 0xFFFFFFFF），帧就永远解析不出来。而 `manager::feed_bytes`
        // 在解析**之前**就刷新了 `last_heartbeat` —— 于是未认证的对端只要
        // 持续发垃圾，会话就**永不过期**、缓冲只增不减，内存按线速增长直至
        // OOM；再叠加「每个新源地址新建一个会话」，还能横向放大。
        //
        // JT808 单帧最大 64KB（见 `server.rs` 真帧路径的判据），这里给 256KB
        // 的余量仍然远超任何合法用法。
        const MAX_SESSION_BUFFER_BYTES: usize = 256 * 1024;

        self.buffer.extend_from_slice(data);
        if self.buffer.len() > MAX_SESSION_BUFFER_BYTES {
            tracing::warn!(
                "JT1078 会话 {} 的接收缓冲超过 {} 字节，判定为无效流并丢弃（对端未按长度收敛）",
                self.peer,
                MAX_SESSION_BUFFER_BYTES
            );
            // 流已经失步，清空重来；不返回任何帧（这段数据本就不可信）
            self.buffer.clear();
            return Vec::new();
        }

        let mut out = Vec::new();

        loop {
            // Try structured frame first (more specific)
            if let Some((total_len, frame)) = parse_jt1078_structured_frame(&self.buffer) {
                // consume bytes
                self.buffer.drain(0..total_len);
                // update heartbeat time
                self.last_heartbeat = Instant::now();

                let seq = frame.seq;
                // initialize expected_seq if seeing first structured frame
                if self.expected_seq.is_none() {
                    self.expected_seq = Some(seq);
                }

                // simple protection against unbounded buffering
                if self.pending.len() >= self.max_pending {
                    tracing::warn!(
                        "JT1078 pending buffer full ({}), dropping seq {}",
                        self.pending.len(),
                        seq
                    );
                    // drop the incoming frame to avoid memory growth
                } else {
                    self.pending.insert(seq, frame);
                }

                // attempt in-order delivery
                while let Some(exp) = self.expected_seq {
                    if let Some(f) = self.pending.remove(&exp) {
                        out.push(f.payload);
                        // advance expected (wrapping)
                        self.expected_seq = Some(exp.wrapping_add(1));
                    } else {
                        break;
                    }
                }

                continue;
            }

            // Fallback: legacy length-prefixed frames
            if let Some((frame_len, payload)) = parse_jt1078_frame(&self.buffer) {
                // copy payload out before mutating buffer to avoid borrow conflicts
                let payload_vec = payload.to_vec();
                self.buffer.drain(0..frame_len);
                self.last_heartbeat = Instant::now();
                out.push(payload_vec);
                continue;
            }

            break;
        }

        out
    }

    /// Collect missing sequence numbers that have exceeded the retransmit wait timeout.
    /// Returns a vector of seq numbers (in ascending order) that should be considered missing and handled.
    pub fn collect_timed_out_missing(&mut self, timeout: Duration) -> Vec<u16> {
        // Build list of timed-out missing sequence numbers between expected_seq and highest pending seq.
        let now = Instant::now();
        let mut timed_out = Vec::new();
        if let Some(start) = self.expected_seq {
            if let Some((&max_seq, _)) = self.pending.iter().next_back() {
                // 序号是 u16 且会回绕。`max_seq` 必须**在 start 之后**才谈得上
                // 「中间缺了一段」；否则按回绕算出来的「缺失区间」会横跨整个
                // 序号空间。
                //
                // 典型触发：先收到 seq=5 并被投递（expected_seq 变 6），随后收到
                // 一个**过期/重复**的 seq=3 的旧帧 —— 此时 pending 里最大键是 3，
                // 修复前的 `while s != max_seq.wrapping_add(1)` 会绕 65534 圈，
                // 把 65533 个序号塞进 `timed_out`，再由调用方灌进重传 hook 的
                // JSON（约 400KB）与 UDP 重传报文（必然超过 65507 字节上限，
                // 发送失败还被 `let _ =` 吞掉）。
                //
                // 这里用回绕距离判定：距离为 0 表示没有缺失区间；距离大于半个
                // 序号空间表示 `max_seq` **落后于** `start`（收到的是过期帧），
                // 同样没有可报���的缺失区间。
                let dist = max_seq.wrapping_sub(start);
                if dist == 0 || dist > 0x8000 {
                    return timed_out;
                }
                // 再加一道业务上限：`pending` 最多保留 `max_pending` 帧，
                // 缺失区间不可能比它还长。超出说明序号已经乱序到不可解释。
                if dist as usize > self.max_pending {
                    tracing::warn!(
                        "JT1078 会话 {} 的缺失序号区间长度 {} 超过 pending 上限 {}，忽略（序号已乱序）",
                        self.peer,
                        dist,
                        self.max_pending
                    );
                    return timed_out;
                }

                for i in 0..dist {
                    let s = start.wrapping_add(i);
                    if !self.pending.contains_key(&s) {
                        // Use last_heartbeat as a conservative baseline for age; caller will decide rate-limit
                        if now.duration_since(self.last_heartbeat) > timeout {
                            timed_out.push(s);
                        }
                    }
                }
            }
        }
        timed_out
    }

    /// Determine whether a missing-sequence alert should be triggered now for this session.
    /// This implements simple per-session rate-limiting for alerts to avoid flooding.
    pub fn should_trigger_missing_alert(&mut self, interval: Duration) -> bool {
        let now = Instant::now();
        match self.last_missing_alert {
            Some(last) => {
                if now.duration_since(last) < interval {
                    return false;
                }
            }
            None => {}
        }
        self.last_missing_alert = Some(now);
        true
    }

    /// Process a single payload (decoded frame). Returns FrameKind for higher-level handling.
    /// Protocol decisions here are intentionally simple for tests:
    /// - AUTH:<token> => authenticate if matches env var GBSERVER__JT1078__TOKEN (default "secret")
    /// - HEARTBEAT payload => update last_heartbeat
    /// - otherwise => Data(payload)
    pub fn process_payload(&mut self, payload: &[u8]) -> FrameKind {
        if payload.starts_with(b"AUTH:") {
            let token = &payload[5..];
            // allow per-session override to avoid races in tests
            let configured = if let Some(cfg) = &self.expected_token {
                cfg.clone()
            } else {
                std::env::var("GBSERVER__JT1078__TOKEN").unwrap_or_else(|_| "secret".into())
            };
            // compare as UTF-8 trimmed string to be tolerant of minor framing differences
            let token_str = String::from_utf8_lossy(token).trim().to_string();
            if token_str == configured || token_str.contains(&configured) {
                self.authenticated = true;
                return FrameKind::AuthSuccess;
            } else {
                self.authenticated = false;
                return FrameKind::AuthFailure;
            }
        }

        if payload == b"HEARTBEAT" {
            self.last_heartbeat = Instant::now();
            return FrameKind::Heartbeat;
        }

        FrameKind::Data(payload.to_vec())
    }

    /// Whether the session is considered timed out given the provided duration
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        Instant::now().duration_since(self.last_heartbeat) > timeout
    }

    /// Phase 6.1+ — process a fully-decoded JT808 message (msg_id, serial, body).
    /// Performs message-type-based dispatch and returns a `ParsedMessage`.
    pub fn process_jt_message(&mut self, msg_id: u16, serial: u16, body: &[u8]) -> ParsedMessage {
        use crate::jt1078::response_parser;
        match msg_id {
            0x0002 => {
                self.last_heartbeat = Instant::now();
                ParsedMessage::Heartbeat
            }
            0x0100 => match response_parser::parse_register_request(body) {
                Ok(req) => ParsedMessage::Register(req),
                Err(e) => {
                    tracing::warn!("parse_register_request failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            // 0x0102 是**终端鉴权**（JT/T 808），正文即鉴权码。
            // 此前这里按「终端属性上报」解析，正文结构完全不同，鉴权码因此
            // 一直收不到，平台也从未校验过（见 OPEN_ISSUES C3）。
            0x0102 => match response_parser::parse_auth_request(body) {
                Ok(code) => ParsedMessage::Authenticate(code),
                Err(e) => {
                    tracing::warn!("parse_auth_request failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            0x0200 => match response_parser::parse_location_report(body) {
                Ok(l) => ParsedMessage::LocationReport(l),
                Err(e) => {
                    tracing::warn!("parse_location_report failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            // 0x0201 位置信息查询应答：消息体与 0x0200 **完全相同**
            // （JT/T 808-2013 §8.19）。此前落到 Unknown，于是
            // `/api/jt1078/position-info` 的"实时查终端"分支恒失败（第四十八轮）。
            0x0201 => match response_parser::parse_location_report(body) {
                Ok(l) => ParsedMessage::LocationReport(l),
                Err(e) => {
                    tracing::warn!("parse_location_report(0x0201) failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            0x0802 => match response_parser::parse_media_search_response(body) {
                Ok(items) => ParsedMessage::MediaSearchResult(items),
                Err(e) => {
                    tracing::warn!("parse_media_search_response failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            0x0801 => match response_parser::parse_media_item_first(body) {
                Ok(m) => ParsedMessage::MediaItem(m),
                Err(e) => {
                    tracing::warn!("parse_media_item_first failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            0x0001 => {
                if body.len() >= 5 {
                    let reply_serial = u16::from_be_bytes([body[0], body[1]]);
                    let reply_msg_id = u16::from_be_bytes([body[2], body[3]]);
                    let result = body[4];
                    ParsedMessage::CommonResponse {
                        reply_serial,
                        reply_msg_id,
                        result,
                    }
                } else {
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            }
            0x0107 => match response_parser::parse_query_params_response(body) {
                Ok(p) => ParsedMessage::QueryParamsResponse(p),
                Err(e) => {
                    tracing::warn!("parse_query_params_response failed: {}", e);
                    ParsedMessage::Unknown {
                        msg_id,
                        serial,
                        body: body.to_vec(),
                    }
                }
            },
            _ => ParsedMessage::Unknown {
                msg_id,
                serial,
                body: body.to_vec(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_addr() -> SocketAddr {
        "127.0.0.1:60000".parse().unwrap()
    }

    #[test]
    fn test_reassembly_and_payload_extraction() {
        let mut sess = Jt1078Session::new(make_addr());
        let payload = b"hello-reassembly";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        // split into two parts
        let split = 5;
        let part1 = &buf[..split];
        let part2 = &buf[split..];

        let frames1 = sess.feed_bytes(part1);
        assert!(frames1.is_empty());

        let frames2 = sess.feed_bytes(part2);
        assert_eq!(frames2.len(), 1);
        assert_eq!(frames2[0], payload);
    }

    #[test]
    fn test_auth_and_heartbeat_processing() {
        std::env::set_var("GBSERVER__JT1078__TOKEN", "mytoken");
        let mut sess = Jt1078Session::new(make_addr());
        sess.expected_token = Some("mytoken".into());

        // construct AUTH frame (length-prefixed)
        let payload = b"AUTH:mytoken";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let frames = sess.feed_bytes(&buf);
        assert_eq!(frames.len(), 1);
        // ensure payload extracted exactly
        assert_eq!(
            frames[0].as_slice(),
            payload,
            "extracted payload mismatch: {:?}",
            frames[0]
        );
        let kind = sess.process_payload(&frames[0]);
        assert_eq!(kind, FrameKind::AuthSuccess);
        assert!(sess.authenticated);

        // heartbeat
        let hb = b"HEARTBEAT";
        let len = (hb.len() as u32).to_be_bytes();
        let mut buf2 = Vec::new();
        buf2.extend_from_slice(&len);
        buf2.extend_from_slice(hb);

        // record previous heartbeat time
        let prev = sess.last_heartbeat;
        let frames2 = sess.feed_bytes(&buf2);
        let kind2 = sess.process_payload(&frames2[0]);
        assert_eq!(kind2, FrameKind::Heartbeat);
        assert!(sess.last_heartbeat >= prev);
    }

    #[test]
    fn test_auth_failure() {
        std::env::set_var("GBSERVER__JT1078__TOKEN", "good");
        let mut sess = Jt1078Session::new(make_addr());
        sess.expected_token = Some("good".into());

        let payload = b"AUTH:bad";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let frames = sess.feed_bytes(&buf);
        let kind = sess.process_payload(&frames[0]);
        assert_eq!(kind, FrameKind::AuthFailure);
        assert!(!sess.authenticated);
    }

    #[test]
    fn test_structured_seq_reassembly_out_of_order() {
        let mut sess = Jt1078Session::new(make_addr());

        // helper to build structured frame bytes
        fn build_structured(seq: u16, payload: &[u8]) -> Vec<u8> {
            let payload_len = (payload.len() as u16).to_be_bytes();
            let seq_b = seq.to_be_bytes();
            let ts = 0u32.to_be_bytes();
            let mut xor: u8 = 0;
            for &b in payload {
                xor ^= b;
            }
            let mut buf = Vec::new();
            buf.push(0x7E);
            buf.push(0x01);
            buf.extend_from_slice(&payload_len);
            buf.extend_from_slice(&seq_b);
            buf.extend_from_slice(&ts);
            buf.extend_from_slice(payload);
            buf.push(xor);
            buf
        }

        let p1 = b"first";
        let p2 = b"second";
        let f2 = build_structured(2, p2);
        let f1 = build_structured(1, p1);

        // feed seq 2 first
        let res1 = sess.feed_bytes(&f2);
        // not delivered yet because expected_seq is 2 but pending holds only 2 and expected starts at 2 -> it should deliver 2 immediately
        // our implementation sets expected_seq to first seen seq and then delivers contiguous frames; feeding only seq2 should deliver seq2
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0], p2);

        // now feed seq1 (older); since expected_seq advanced, seq1 will be stored but out-of-order older than delivered — drop behavior may vary
        let _res2 = sess.feed_bytes(&f1);
        // if expected_seq was 3 after delivering 2, seq1 won't be delivered. But we want to ensure in-order delivery when frames arrive 1 then 2.
        // To test reordering, feed 1 then 2
        let mut sess2 = Jt1078Session::new(make_addr());
        let res_a = sess2.feed_bytes(&f1);
        assert_eq!(res_a.len(), 1);
        assert_eq!(res_a[0], p1);
        let res_b = sess2.feed_bytes(&f2);
        assert_eq!(res_b.len(), 1);
        assert_eq!(res_b[0], p2);
    }
}

#[cfg(test)]
mod session_buffer_bound_tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_addr() -> SocketAddr {
        "127.0.0.1:60000".parse().unwrap()
    }

    /// **回归**：接收缓冲必须有上限。
    ///
    /// 旧格式（4 字节大端长度前缀）可以声明一个**永远凑不齐**的长度
    /// （0xFFFFFFFF），帧就永远解析不出来；而 `manager::feed_bytes` 在解析
    /// **之前**就刷新了 `last_heartbeat`，于是未认证的对端持续发垃圾就能让
    /// 缓冲只增不减、会话永不过期 —— 内存按线速增长直至 OOM。
    #[test]
    fn session_buffer_is_bounded() {
        let mut s = Jt1078Session::new(make_addr());
        // 旧格式帧头：声明 0xFFFFFFFF 字节的正文（永远凑不齐）
        let mut chunk = vec![0xFF, 0xFF, 0xFF, 0xFF];
        chunk.extend_from_slice(&[0x00; 4096]);

        for _ in 0..200 {
            s.feed_bytes(&chunk);
        }

        assert!(
            s.buffer.len() <= 256 * 1024,
            "接收缓冲必须封顶，实际 {} 字节",
            s.buffer.len()
        );
    }

    /// **回归**：合法的大帧仍要能正常解析（封顶不能误伤正常流量）。
    #[test]
    fn bounded_buffer_still_parses_legacy_frames() {
        let mut s = Jt1078Session::new(make_addr());
        let payload = b"legacy-payload";
        let mut raw = Vec::new();
        raw.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        raw.extend_from_slice(payload);

        let out = s.feed_bytes(&raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], payload);
        assert!(s.buffer.is_empty());
    }

}

#[cfg(test)]
mod missing_seq_tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_addr() -> SocketAddr {
        "127.0.0.1:60000".parse().unwrap()
    }

    /// **回归**：缺失序号统计**不能**因为 u16 回绕而构造出 65533 条。
    ///
    /// 触发条件：先收到 seq=5 并被投递（expected_seq 变 6），随后收到一个
    /// 过期/重复的 seq=3 的旧帧 —— 此时 pending 的最大键是 3。修复前的
    /// `while s != max_seq.wrapping_add(1)` 会绕 65534 圈，把 65533 个序号
    /// 塞进结果，再被灌进重传 hook 的 JSON 与 UDP 重传报文。
    #[test]
    fn missing_seq_scan_does_not_wrap_around() {
        let mut s = Jt1078Session::new(make_addr());
        s.expected_seq = Some(6);
        // 收到一个过期帧：seq=3（比 expected_seq 小）
        s.pending.insert(
            3,
            Jt1078Frame {
                seq: 3,
                timestamp: 0,
                payload: vec![1, 2, 3],
                checksum: 0,
            },
        );

        let missing = s.collect_timed_out_missing(Duration::from_secs(0));
        assert!(
            missing.len() <= s.max_pending,
            "缺失序号数量必须受限，实际 {} 条",
            missing.len()
        );
    }

    /// **对照**：正常场景下缺失区间仍要被如实报出（修复不能把功能改没了）。
    #[test]
    fn missing_seq_scan_reports_real_gap() {
        let mut s = Jt1078Session::new(make_addr());
        s.expected_seq = Some(10);
        s.pending.insert(
            13,
            Jt1078Frame {
                seq: 13,
                timestamp: 0,
                payload: vec![1],
                checksum: 0,
            },
        );

        let missing = s.collect_timed_out_missing(Duration::from_secs(0));
        assert_eq!(missing, vec![10, 11, 12]);
    }
}
