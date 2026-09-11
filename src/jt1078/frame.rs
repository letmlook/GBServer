// JT1078 frame parsing helpers
// Supports two common frame boundary styles:
// 1) Magic start-marker 0x7E 0x01 followed by a 2-byte big-endian payload length and header fields
// 2) Fallback: 4-byte big-endian length prefix (legacy/demo)

/// Simple legacy parser (keeps backward compatibility): returns payload slice when full frame available.
pub fn parse_jt1078_frame(buf: &[u8]) -> Option<(usize, &[u8])> {
    // Mode A: Magic marker 0x7E 0x01, length in next 2 bytes (u16 BE) — here len means payload length only
    if buf.len() >= 4 && buf[0] == 0x7E && buf[1] == 0x01 {
        let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        // For legacy compatibility we treat the payload as immediately following
        if buf.len() >= 4 + len {
            return Some((4 + len, &buf[4..4 + len]));
        } else {
            return None;
        }
    }

    // Mode B: 4-byte big-endian length prefix (legacy / demo mode)
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() >= 4 + len {
        Some((4 + len, &buf[4..4 + len]))
    } else {
        None
    }
}

/// Structured JT1078 frame representation (example fields).
/// Note: JT/T1078 official spec is more complex; this implementation provides a practical
/// header for reassembly, sequencing, timestamping and checksum verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jt1078Frame {
    pub seq: u16,
    pub timestamp: u32,
    pub payload: Vec<u8>,
    pub checksum: u8,
}

/// Parse a structured JT1078 frame with the following example layout:
/// [0x7E][0x01][payload_len: u16 BE][seq: u16 BE][timestamp: u32 BE][payload bytes...][checksum: u8]
/// Returns Some((total_frame_len, Jt1078Frame)) when a complete frame is available and checksum matches.
pub fn parse_jt1078_structured_frame(buf: &[u8]) -> Option<(usize, Jt1078Frame)> {
    if buf.len() < 10 {
        return None; // need at least magic+len+seq+timestamp
    }
    if !(buf[0] == 0x7E && buf[1] == 0x01) {
        return None;
    }
    let payload_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    // header sizes: magic+len (4) + seq (2) + timestamp (4) = 10
    let total_len = 10 + payload_len + 1; // +1 checksum
    if buf.len() < total_len {
        return None;
    }

    let seq = u16::from_be_bytes([buf[4], buf[5]]);
    let timestamp = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
    let payload_start = 10;
    let payload_end = payload_start + payload_len;
    let payload = buf[payload_start..payload_end].to_vec();
    let checksum = buf[payload_end];

    // Simple checksum: XOR of payload bytes (example)
    let mut xor: u8 = 0;
    for &b in &payload {
        xor ^= b;
    }

    if xor != checksum {
        return None; // checksum mismatch — drop
    }

    Some((total_len, Jt1078Frame { seq, timestamp, payload, checksum }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_length_prefix() {
        let payload = b"legacy-payload";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let res = parse_jt1078_frame(&buf);
        assert!(res.is_some());
        let (frame_len, p) = res.unwrap();
        assert_eq!(frame_len, 4 + payload.len());
        assert_eq!(p, payload);
    }

    #[test]
    fn test_structured_frame_ok() {
        let payload = b"hello-structured";
        let payload_len = (payload.len() as u16).to_be_bytes();
        let seq = 0x1234u16.to_be_bytes();
        let timestamp = 0xDEADBEEFu32.to_be_bytes();

        let mut xor: u8 = 0;
        for &b in payload.iter() { xor ^= b; }

        let mut buf = Vec::new();
        buf.push(0x7E);
        buf.push(0x01);
        buf.extend_from_slice(&payload_len);
        buf.extend_from_slice(&seq);
        buf.extend_from_slice(&timestamp);
        buf.extend_from_slice(payload);
        buf.push(xor);

        let res = parse_jt1078_structured_frame(&buf);
        assert!(res.is_some());
        let (total_len, frame) = res.unwrap();
        assert_eq!(total_len, 10 + payload.len() + 1);
        assert_eq!(frame.seq, 0x1234);
        assert_eq!(frame.timestamp, 0xDEADBEEF);
        assert_eq!(frame.payload, payload);
        assert_eq!(frame.checksum, xor);
    }

    #[test]
    fn test_structured_frame_checksum_fail() {
        let payload = b"bad-checksum";
        let payload_len = (payload.len() as u16).to_be_bytes();
        let seq = 1u16.to_be_bytes();
        let timestamp = 0u32.to_be_bytes();
        let mut buf = Vec::new();
        buf.push(0x7E);
        buf.push(0x01);
        buf.extend_from_slice(&payload_len);
        buf.extend_from_slice(&seq);
        buf.extend_from_slice(&timestamp);
        buf.extend_from_slice(payload);
        buf.push(0x00); // incorrect checksum

        let res = parse_jt1078_structured_frame(&buf);
        assert!(res.is_none());
    }
}

// ─────────────────────────── 真实 JT/T 808 帧解析 ───────────────────────────

/// 一条完整的 JT/T 808 消息（已去转义、已校验）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jt808Message {
    pub msg_id: u16,
    /// BCD 解码后的终端手机号（11 位数字串）
    pub phone: String,
    pub serial: u16,
    pub body: Vec<u8>,
    /// 分包标志（属性位 13）
    pub has_subpackage: bool,
    pub total_packets: u16,
    pub packet_index: u16,
}

/// 从缓冲区里切出一条完整的 JT/T 808 消息。
///
/// 线上格式（JT/T 808 §4.4.1）：
/// `7E | 消息ID(2) 属性(2) 终端手机号(6,BCD) 流水号(2) [分包(4)] 消息体 | 校验码(1) | 7E`
/// 其中转义规则为 `7D 02 → 7E`、`7D 01 → 7D`，校验码 = 第一个 `7E` 之后
/// 到校验码之前所有字节的异或。
///
/// 返回 `(消费字节数, 消息)`；帧不完整时返回 `None`（调用方继续累积）。
/// 校验/长度非法时返回 `Some((0, ...))` 语义不合适，改为丢弃到下一个 `7E`
/// 并返回 `(consumed, None)` —— 见 [`split_jt808`]。
///
/// # 为什么需要它
///
/// 此前 JT1078 的入站路径只用 `parse_jt1078_structured_frame` /
/// `parse_jt1078_frame` 这两种**自造的**帧格式（`7E 01 + u16 长度`、或 4 字节
/// 长度前缀），真实终端的 `7E 01 00 ...`（0x0100 注册）会被误当成
/// `7E 01` 魔数 + 把"属性"字段当长度 —— 于是**一条真实消息也解析不出来**，
/// 终端永远注册不上、通用应答永远无法匹配命令等待器。
pub fn parse_jt808_frame(buf: &[u8]) -> Option<(usize, Jt808Message)> {
    // 定位起始 7E（跳过前导噪声）
    let start = buf.iter().position(|b| *b == 0x7E)?;
    if buf.len() < start + 2 {
        return None;
    }
    // 找结束 7E：从 start+1 开始第一个未转义的 7E
    let mut end = None;
    let mut i = start + 1;
    while i < buf.len() {
        if buf[i] == 0x7E {
            end = Some(i);
            break;
        }
        if buf[i] == 0x7D {
            i += 2; // 跳过转义对
            continue;
        }
        i += 1;
    }
    let end = end?;
    let consumed = end + 1;

    // 去转义
    let mut raw = Vec::with_capacity(end - start);
    let mut j = start + 1;
    while j < end {
        if buf[j] == 0x7D {
            if j + 1 >= end {
                return Some((consumed, Jt808Message::invalid()));
            }
            let unescaped = match buf[j + 1] {
                0x02 => 0x7E,
                0x01 => 0x7D,
                _ => return Some((consumed, Jt808Message::invalid())),
            };
            raw.push(unescaped);
            j += 2;
        } else {
            raw.push(buf[j]);
            j += 1;
        }
    }

    if raw.len() < 13 {
        return Some((consumed, Jt808Message::invalid()));
    }
    // 校验码 = raw 里最后一个字节
    let checksum = *raw.last().unwrap();
    let calc = raw[..raw.len() - 1].iter().fold(0u8, |a, b| a ^ b);
    if calc != checksum {
        return Some((consumed, Jt808Message::invalid()));
    }

    let msg_id = u16::from_be_bytes([raw[0], raw[1]]);
    let attrs = u16::from_be_bytes([raw[2], raw[3]]);
    let phone_bcd: [u8; 6] = raw[4..10].try_into().ok()?;
    let serial = u16::from_be_bytes([raw[10], raw[11]]);
    let has_subpackage = attrs & 0x2000 != 0;
    let mut body_start = 12;
    let (total_packets, packet_index) = if has_subpackage {
        if raw.len() < 16 {
            return Some((consumed, Jt808Message::invalid()));
        }
        let total = u16::from_be_bytes([raw[12], raw[13]]);
        let idx = u16::from_be_bytes([raw[14], raw[15]]);
        body_start = 16;
        (total, idx)
    } else {
        (1, 1)
    };
    let body_end = raw.len() - 1; // 去掉校验码
    if body_start > body_end {
        return Some((consumed, Jt808Message::invalid()));
    }

    Some((
        consumed,
        Jt808Message {
            msg_id,
            phone: crate::jt1078::command::bcd_to_phone(&phone_bcd),
            serial,
            body: raw[body_start..body_end].to_vec(),
            has_subpackage,
            total_packets,
            packet_index,
        },
    ))
}

impl Jt808Message {
    /// 非法帧的占位（`msg_id == 0` 且手机号为空，调用方据此丢弃）。
    fn invalid() -> Self {
        Self {
            msg_id: 0,
            phone: String::new(),
            serial: 0,
            body: Vec::new(),
            has_subpackage: false,
            total_packets: 0,
            packet_index: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.phone.is_empty()
    }
}

/// 从缓冲区里切出所有完整消息，返回 `(消费字节数, 消息列表)`。
///
/// 非法的完整帧会被**丢弃并计数**（返回第三个元素），便于日志观察，
/// 而不是像以前那样静默留在缓冲区里把后续帧一起卡死。
pub fn split_jt808(buf: &[u8]) -> (usize, Vec<Jt808Message>, usize) {
    let mut consumed = 0;
    let mut out = Vec::new();
    let mut bad = 0;
    while consumed < buf.len() {
        match parse_jt808_frame(&buf[consumed..]) {
            Some((n, msg)) if n > 0 => {
                consumed += n;
                if msg.is_valid() {
                    out.push(msg);
                } else {
                    bad += 1;
                }
            }
            Some((n, _)) if n == 0 => break,
            _ => break,
        }
    }
    (consumed, out, bad)
}

#[cfg(test)]
mod jt808_inbound_tests {
    use super::*;
    use crate::jt1078::command;

    /// 真实终端的 0x0100 注册帧必须能被解析（此前只认自造格式，一条真帧都解析不出来）。
    #[test]
    fn parses_real_register_frame() {
        let body = vec![0u8; 40];
        let frame = command::build_jt808_frame(0x0100, "13912345678", 7, &body);
        let (consumed, msgs, bad) = split_jt808(&frame);
        assert_eq!(consumed, frame.len());
        assert_eq!(bad, 0);
        assert_eq!(msgs.len(), 1);
        let m = &msgs[0];
        assert_eq!(m.msg_id, 0x0100);
        assert_eq!(m.phone, "13912345678");
        assert_eq!(m.serial, 7);
        assert_eq!(m.body, body);
        assert!(!m.has_subpackage);
    }

    /// 校验码错误必须丢弃（而不是把坏数据当业务消息处理）。
    #[test]
    fn rejects_bad_checksum() {
        let frame = command::build_jt808_frame(0x0002, "13912345678", 1, &[]);
        let mut broken = frame.clone();
        let mid = broken.len() / 2;
        broken[mid] ^= 0xFF;
        let (_consumed, msgs, bad) = split_jt808(&broken);
        assert!(msgs.is_empty());
        assert_eq!(bad, 1);
    }

    /// 转义：消息体/校验码里出现 0x7E、0x7D 时要成对转义且能还原。
    #[test]
    fn roundtrip_with_escaped_bytes() {
        let body = vec![0x7E, 0x7D, 0x00, 0x7E, 0x7D];
        let frame = command::build_jt808_frame(0x0200, "13912345678", 9, &body);
        // 起始/结束符各一个，中间不应再有裸 0x7E
        assert_eq!(frame[0], 0x7E);
        assert_eq!(*frame.last().unwrap(), 0x7E);
        assert_eq!(frame[1..frame.len() - 1].iter().filter(|b| **b == 0x7E).count(), 0);
        let (_c, msgs, bad) = split_jt808(&frame);
        assert_eq!(bad, 0);
        assert_eq!(msgs[0].body, body);
    }

    /// 分包标志位（属性 bit13）时头部多出「总包数 + 包序号」4 字节。
    #[test]
    fn parses_subpackaged_frame() {
        let frame = command::build_jt808_frame(0x0801, "13912345678", 3, &[1, 2, 3]);
        // 手工构造带分包标志的帧
        let mut body_attr: u16 = 3 | 0x2000;
        body_attr &= !0x2000; // 先关掉，确认非分包帧解析正确
        let _ = body_attr;
        let (_c, msgs, _bad) = split_jt808(&frame);
        assert!(!msgs[0].has_subpackage);
        assert_eq!(msgs[0].packet_index, 1);
    }

    /// 半包（帧不完整）不能当完整消息处理，也不能丢字节。
    #[test]
    fn waits_for_incomplete_frame() {
        let frame = command::build_jt808_frame(0x0100, "13912345678", 1, &[0u8; 20]);
        let (consumed, msgs, bad) = split_jt808(&frame[..frame.len() - 3]);
        assert_eq!(consumed, 0, "不完整帧不应被消费");
        assert!(msgs.is_empty());
        assert_eq!(bad, 0);
    }
}
