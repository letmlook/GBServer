use super::invite_session::StreamType;

/// GB28181 中 INVITE 未显式指定 SSRC 时的默认占位值。
/// 国标要求 `y=` 行必须存在，设备在 200 OK 中回显同一值。
pub const DEFAULT_SSRC: &str = "0100000001";

/// 业务类型字符串 → [`StreamType`]。
///
/// SIP 信令层用字符串（`"Play"` / `"Playback"` / …）传递业务类型；这里集中做
/// 一次归一化，避免各调用点各自 `match` 出一套互不相同的映射。
pub fn stream_type_from_str(s: &str) -> StreamType {
    match s.trim().to_ascii_lowercase().as_str() {
        "playback" | "play_back" | "history" => StreamType::Playback,
        "download" => StreamType::Download,
        "talk" | "audio" => StreamType::Talk,
        "broadcast" => StreamType::Broadcast,
        // 含 "play" 及一切未知取值都按实时流处理（与历史行为一致）
        _ => StreamType::Play,
    }
}

/// 把回放/下载时间参数归一化成 SDP `t=` 与 `a=range:npt=` 需要的 **UNIX 秒**。
///
/// 上游（前端 el-date-picker、RecordInfo 查询）传进来的是
/// `YYYY-MM-DD HH:MM:SS` / `YYYY-MM-DDTHH:MM:SS`，而国标要求 SDP 的
/// `t=` 与 `a=range:npt=` 用 UNIX 秒。此前把 ISO 串原样写进 SDP，
/// 设备收到的是无法解析的时间区间。
///
/// 无法识别时返回 `None`，调用方回退成 `0 0`（不限时），
/// 而不是把非法字符串发出去。
pub fn to_unix_seconds(t: &str) -> Option<i64> {
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    // 已经是 UNIX 秒（0 表示不限时）
    if let Ok(v) = t.parse::<i64>() {
        return Some(v);
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
    ] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, fmt) {
            return Some(dt.and_utc().timestamp());
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Some(dt.timestamp());
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdpDirection {
    SendOnly,
    RecvOnly,
    SendRecv,
    Inactive,
}

impl SdpDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            SdpDirection::SendOnly => "sendonly",
            SdpDirection::RecvOnly => "recvonly",
            SdpDirection::SendRecv => "sendrecv",
            SdpDirection::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdpSetup {
    Active,
    Passive,
    ActPass,
}

impl SdpSetup {
    pub fn as_str(&self) -> &'static str {
        match self {
            SdpSetup::Active => "active",
            SdpSetup::Passive => "passive",
            SdpSetup::ActPass => "actpass",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    Udp,
    Tcp,
}

pub struct SdpBuilder {
    ip: String,
    media_port: u16,
    stream_type: StreamType,
    ssrc: String,
    direction: SdpDirection,
    transport: TransportMode,
    setup: Option<SdpSetup>,
    start_time: Option<String>,
    end_time: Option<String>,
    session_id: u64,
}

impl SdpBuilder {
    pub fn new(ip: &str, media_port: u16, stream_type: StreamType, ssrc: &str) -> Self {
        // 方向语义（GB/T 28181-2016 附录示例）：
        //   平台发出的 INVITE 请求点播/回放/下载 → 平台**收**，设备发 → recvonly
        //   语音对讲/广播 → 平台既发又收（可视对讲）→ sendrecv
        // 设备侧的 200 OK 才用 sendonly。
        let direction = match stream_type {
            StreamType::Play | StreamType::Playback | StreamType::Download => SdpDirection::RecvOnly,
            StreamType::Talk | StreamType::Broadcast => SdpDirection::SendRecv,
        };
        Self {
            ip: ip.to_string(),
            media_port,
            stream_type,
            ssrc: ssrc.to_string(),
            direction,
            transport: TransportMode::Udp,
            setup: None,
            start_time: None,
            end_time: None,
            session_id: 0,
        }
    }

    pub fn direction(mut self, direction: SdpDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn transport(mut self, transport: TransportMode) -> Self {
        self.transport = transport;
        self
    }

    pub fn setup(mut self, setup: SdpSetup) -> Self {
        self.setup = Some(setup);
        self
    }

    /// 设置回放/下载时间区间。
    ///
    /// 入参可以是 UNIX 秒（已经是秒）或 ISO 时间串，内部统一归一化成 UNIX 秒，
    /// 因为 `t=` 与 `a=range:npt=` 都必须是秒。
    pub fn time_range(mut self, start: &str, end: &str) -> Self {
        self.start_time = to_unix_seconds(start).map(|v| v.to_string());
        self.end_time = to_unix_seconds(end).map(|v| v.to_string());
        self
    }

    pub fn session_id(mut self, id: u64) -> Self {
        self.session_id = id;
        self
    }

    pub fn build(self) -> String {
        let session_name = match self.stream_type {
            StreamType::Play => "Play",
            StreamType::Playback => "Playback",
            StreamType::Download => "Download",
            StreamType::Talk => "Talk",
            StreamType::Broadcast => "Broadcast",
        };

        let proto = match self.transport {
            TransportMode::Udp => "RTP/AVP",
            TransportMode::Tcp => "TCP/RTP/AVP",
        };

        let t_field = match (&self.start_time, &self.end_time) {
            (Some(s), Some(e)) if s != "0" => format!("{} {}", s, e),
            _ => "0 0".to_string(),
        };

        let mut sdp = format!(
            "v=0\r\n\
             o=- {} 0 IN IP4 {}\r\n\
             s={}\r\n\
             c=IN IP4 {}\r\n\
             t={}\r\n",
            self.session_id, self.ip, session_name, self.ip, t_field
        );

        match self.stream_type {
            StreamType::Talk | StreamType::Broadcast => {
                sdp.push_str(&format!(
                    "m=audio {} {} 8 0 101\r\n\
                     a=rtpmap:8 PCMA/8000\r\n\
                     a=rtpmap:0 PCMU/8000\r\n\
                     a=rtpmap:101 telephone-event/8000\r\n"
                    , self.media_port, proto
                ));
            }
            _ => {
                sdp.push_str(&format!(
                    "m=video {} {} 96\r\n\
                     a=rtpmap:96 PS/90000\r\n"
                    , self.media_port, proto
                ));
            }
        }

        sdp.push_str(&format!("a={}\r\n", self.direction.as_str()));

        if self.transport == TransportMode::Tcp {
            if let Some(ref setup) = self.setup {
                sdp.push_str(&format!("a=setup:{}\r\n", setup.as_str()));
            } else {
                sdp.push_str("a=setup:passive\r\n");
            }
            sdp.push_str("a=connection:new\r\n");
        }

        if let (Some(s), Some(_)) = (&self.start_time, &self.end_time) {
            if s != "0" {
                sdp.push_str(&format!("a=range:npt={},{}\r\n", 
                    self.start_time.as_deref().unwrap_or("0"),
                    self.end_time.as_deref().unwrap_or("0")
                ));
            }
        }

        sdp.push_str(&format!("y={}\r\n", self.ssrc));

        match self.stream_type {
            StreamType::Talk | StreamType::Broadcast => {}
            _ => {
                sdp.push_str("f=v/1/96/1/2/1/1/0\r\n");
            }
        }

        sdp
    }
}

pub fn play_sdp(ip: &str, media_port: u16, ssrc: &str) -> String {
    SdpBuilder::new(ip, media_port, StreamType::Play, ssrc)
        .build()
}

pub fn playback_sdp(ip: &str, media_port: u16, ssrc: &str, start_time: &str, end_time: &str) -> String {
    SdpBuilder::new(ip, media_port, StreamType::Playback, ssrc)
        .time_range(start_time, end_time)
        .build()
}

pub fn download_sdp(ip: &str, media_port: u16, ssrc: &str, start_time: &str, end_time: &str) -> String {
    SdpBuilder::new(ip, media_port, StreamType::Download, ssrc)
        .time_range(start_time, end_time)
        .build()
}

pub fn talk_sdp(ip: &str, audio_port: u16, ssrc: &str) -> String {
    SdpBuilder::new(ip, audio_port, StreamType::Talk, ssrc)
        .build()
}

pub fn broadcast_sdp(ip: &str, audio_port: u16, ssrc: &str) -> String {
    SdpBuilder::new(ip, audio_port, StreamType::Broadcast, ssrc)
        .build()
}

/// 从设备 200 OK 的 SDP 里解析首个 `m=video` / `m=audio` 的端口号。
///
/// 例如 `m=video 11001 TCP/RTP/AVP 96` → `Some(11001)`。
///
/// GB28181 设备在 200 OK 里宣告的端口**未必**等于我们在 INVITE 里给的
/// 端口：TCP 被动模式（`a=setup:passive` 语义）下设备宣告的是它自己的
/// 监听端口，需要我们用 `connectRtpServer` 让 ZLM 主动去连。因此这个
/// 解析是「按需接管」判定的依据，放在 SDP 的唯一真源模块里，
/// 避免 hook / play 各写一份而行为漂移。
pub fn parse_media_port(sdp: &str) -> Option<u16> {
    for line in sdp.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("m=") else {
            continue;
        };
        let mut it = rest.split_whitespace();
        let Some(media_type) = it.next() else { continue };
        if media_type != "video" && media_type != "audio" {
            continue;
        }
        if let Some(Ok(port)) = it.next().map(str::parse::<u16>) {
            return Some(port);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_sdp() {
        let sdp = play_sdp("192.168.1.100", 50000, "0100000001");
        assert!(sdp.contains("s=Play"));
        assert!(sdp.contains("c=IN IP4 192.168.1.100"));
        assert!(sdp.contains("m=video 50000 RTP/AVP 96"));
        assert!(sdp.contains("a=recvonly"));
        assert!(sdp.contains("y=0100000001"));
        assert!(sdp.contains("f=v/1/96/1/2/1/1/0"));
    }

    #[test]
    fn test_playback_sdp() {
        let sdp = playback_sdp("192.168.1.100", 50000, "0100000001", "1700000000", "1700003600");
        assert!(sdp.contains("s=Playback"));
        assert!(sdp.contains("t=1700000000 1700003600"));
        assert!(sdp.contains("a=range:npt=1700000000,1700003600"));
    }

    #[test]
    fn test_download_sdp() {
        let sdp = download_sdp("192.168.1.100", 50000, "0100000001", "1700000000", "1700003600");
        assert!(sdp.contains("s=Download"));
        assert!(sdp.contains("a=range:npt=1700000000,1700003600"));
    }

    #[test]
    fn test_talk_sdp() {
        let sdp = talk_sdp("192.168.1.100", 50002, "0200005678");
        assert!(sdp.contains("s=Talk"));
        assert!(sdp.contains("m=audio 50002 RTP/AVP 8 0 101"));
        assert!(sdp.contains("a=sendrecv"));
        assert!(sdp.contains("y=0200005678"));
        assert!(!sdp.contains("f=v/1"));
    }

    #[test]
    fn test_tcp_transport() {
        let sdp = SdpBuilder::new("192.168.1.100", 50000, StreamType::Play, "0100000001")
            .transport(TransportMode::Tcp)
            .setup(SdpSetup::Passive)
            .build();
        assert!(sdp.contains("TCP/RTP/AVP"));
        assert!(sdp.contains("a=setup:passive"));
        assert!(sdp.contains("a=connection:new"));
    }

    #[test]
    fn test_sendonly_direction() {
        let sdp = SdpBuilder::new("192.168.1.100", 50000, StreamType::Play, "0100000001")
            .direction(SdpDirection::SendOnly)
            .build();
        assert!(sdp.contains("a=sendonly"));
    }

    /// 平台发出的点播 INVITE 里平台是接收方，必须是 recvonly。
    /// 写成 sendonly 等于告诉设备「我发你收」，与实时点播的实际数据流向相反。
    #[test]
    fn platform_invite_direction_is_recvonly_for_pull_streams() {
        for st in [StreamType::Play, StreamType::Playback, StreamType::Download] {
            let sdp = SdpBuilder::new("192.168.1.100", 50000, st.clone(), "0100000001").build();
            assert!(
                sdp.contains("a=recvonly"),
                "{:?} 的 INVITE 应为 recvonly, 实际:\n{}",
                st,
                sdp
            );
            assert!(!sdp.contains("a=sendonly"), "{:?} 不应是 sendonly", st);
        }
    }

    /// 对讲/广播是双向会话。
    #[test]
    fn talk_and_broadcast_are_sendrecv() {
        for st in [StreamType::Talk, StreamType::Broadcast] {
            let sdp = SdpBuilder::new("192.168.1.100", 50002, st.clone(), "0200005678").build();
            assert!(sdp.contains("a=sendrecv"), "{:?} 应为 sendrecv", st);
            assert!(sdp.contains("m=audio 50002 RTP/AVP 8 0 101"));
        }
    }

    /// 回放时间必须是 UNIX 秒：ISO 串直接写进 t=/npt= 设备无法解析。
    #[test]
    fn playback_time_range_is_normalized_to_unix_seconds() {
        let sdp = playback_sdp(
            "192.168.1.100",
            50000,
            "0100000001",
            "2024-01-01 10:00:00",
            "2024-01-01 11:00:00",
        );
        // 1704103200 = 2024-01-01T10:00:00Z, 1704106800 = 11:00:00Z
        assert!(sdp.contains("t=1704103200 1704106800"), "实际:\n{}", sdp);
        assert!(
            sdp.contains("a=range:npt=1704103200,1704106800"),
            "实际:\n{}",
            sdp
        );
        assert!(!sdp.contains("2024-01-01"), "不应残留 ISO 串:\n{}", sdp);
    }

    #[test]
    fn playback_time_range_accepts_existing_unix_seconds() {
        let sdp = playback_sdp("192.168.1.100", 50000, "0100000001", "1700000000", "1700003600");
        assert!(sdp.contains("t=1700000000 1700003600"));
    }

    /// 起止时间无法解析时回退为不限时，而不是把非法串发出去。
    #[test]
    fn playback_time_range_falls_back_to_unbounded_on_garbage() {
        let sdp = playback_sdp("192.168.1.100", 50000, "0100000001", "not-a-time", "also-bad");
        assert!(sdp.contains("t=0 0"), "实际:\n{}", sdp);
        assert!(!sdp.contains("a=range:npt"), "实际:\n{}", sdp);
        assert!(!sdp.contains("not-a-time"));
    }

    #[test]
    fn to_unix_seconds_handles_supported_forms() {
        assert_eq!(to_unix_seconds("0"), Some(0));
        assert_eq!(to_unix_seconds("1700000000"), Some(1700000000));
        assert_eq!(to_unix_seconds("2024-01-01T10:00:00"), Some(1704103200));
        assert_eq!(to_unix_seconds("2024-01-01 10:00:00"), Some(1704103200));
        assert_eq!(
            to_unix_seconds("2024-01-01T10:00:00Z"),
            Some(1704103200)
        );
        assert_eq!(to_unix_seconds(""), None);
        assert_eq!(to_unix_seconds("   "), None);
        assert_eq!(to_unix_seconds("nonsense"), None);
    }

    #[test]
    fn stream_type_from_str_normalizes_case_and_aliases() {
        use super::stream_type_from_str as f;
        assert_eq!(f("Play"), StreamType::Play);
        assert_eq!(f("play"), StreamType::Play);
        assert_eq!(f("Playback"), StreamType::Playback);
        assert_eq!(f("Download"), StreamType::Download);
        assert_eq!(f("Talk"), StreamType::Talk);
        assert_eq!(f("Broadcast"), StreamType::Broadcast);
        // 未知取值按实时流处理
        assert_eq!(f("whatever"), StreamType::Play);
    }

    /// SSRC 必须来自调用方：所有回放会话共用同一个 y= 会让媒体面无法区分流。
    #[test]
    fn ssrc_is_taken_from_caller_not_hardcoded() {
        let a = playback_sdp("10.0.0.1", 5000, "0100000001", "1700000000", "1700003600");
        let b = playback_sdp("10.0.0.1", 5001, "1100000002", "1700000000", "1700003600");
        assert!(a.contains("y=0100000001"));
        assert!(b.contains("y=1100000002"));
        assert!(!b.contains("y=0100000001"));
    }

    #[test]
    fn parse_media_port_reads_video_and_audio() {
        assert_eq!(
            parse_media_port("v=0\r\nm=video 11001 TCP/RTP/AVP 96\r\n"),
            Some(11001)
        );
        assert_eq!(parse_media_port("m=audio 8000 RTP/AVP 8\r\n"), Some(8000));
        // 多 track：取第一个可解析的
        assert_eq!(
            parse_media_port("m=audio 10002 RTP/AVP 8\r\nm=video 10003 RTP/AVP 96\r\n"),
            Some(10002)
        );
        // 非法/缺失都要返回 None，而不是 panic 或编造 0
        assert_eq!(parse_media_port(""), None);
        assert_eq!(parse_media_port("m=video\r\n"), None);
        assert_eq!(parse_media_port("m=video abc RTP/AVP 96\r\n"), None);
        assert_eq!(parse_media_port("m=application 1234 udp\r\n"), None);
    }

    /// 设备可能把端口写成 0（`m=video 0`）—— 这不是合法收流端口，
    /// 调用方必须能区分「解析到 0」和「解析到真实端口」之外的失败。
    #[test]
    fn parse_media_port_distinguishes_zero_port() {
        assert_eq!(parse_media_port("m=video 0 RTP/AVP 96\r\n"), Some(0));
    }
}
