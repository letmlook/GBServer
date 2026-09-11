use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZlmServerInfo {
    pub id: String,
    pub ip: String,
    pub http_port: u16,
    pub secret: String,
}

/// ZLM `getMediaList` / `getMediaInfo` 返回的单条流信息。
///
/// 除 `app`/`stream`/`schema`/`vhost` 外的字段一律 `#[serde(default)]`：
/// 不同 ZLM 版本的 MediaInfo 字段集并不完全一致（缺少 `tracks`、
/// `alive_second` 之类并不罕见），而**解析失败会让整条流不可见** ——
/// 例如"无人观看自动关流"会因此拿不到 `readerCount`，
/// 从而误判成"没人看"把正在播放的流掐掉。
///
/// # 命名（实测 ZLM 的 JSON 是**混合**风格）
///
/// * MediaInfo 本体是 camelCase：`readerCount` / `totalReaderCount` /
///   `originType` / `originUrl` / `createStamp` / `aliveSecond` /
///   `bytesSpeed` —— 因此这里必须 `rename_all = "camelCase"`。
///   此前没有这条 rename，加上 `#[serde(default)]` 之后，
///   `reader_count` 之类的字段**永远解析成 0**：没有任何报错，
///   只是"观看者数量永远是 0"，于是自动关流会把正在播放的流掐掉。
/// * 内层的 `tracks` 反而是 snake_case（`codec_id` / `codec_id_name` /
///   `sample_rate` …），所以 `TrackInfo` **不能**跟着 camelCase 化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub app: String,
    pub stream: String,
    pub schema: String,
    pub vhost: String,
    #[serde(default)]
    pub reader_count: u32,
    #[serde(default)]
    pub total_reader_count: u32,
    #[serde(default)]
    pub origin_type: u32,
    #[serde(default)]
    pub origin_url: Option<String>,
    #[serde(default)]
    pub create_stamp: i64,
    #[serde(default)]
    pub alive_second: u32,
    #[serde(default)]
    pub bytes_speed: u64,
    #[serde(default)]
    pub tracks: Vec<TrackInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub codec_id: u32,
    pub codec_id_name: String,
    pub codec_type: u32,
    pub ready: bool,
    pub fps: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub channels: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_rate: Option<u32>,
}

#[cfg(test)]
mod media_info_tests {
    use super::*;

    /// 回归：ZLM 的 `getMediaList` / `getMediaInfo` **实际**返回 camelCase
    /// （`readerCount` / `totalReaderCount` / `originType` / `createStamp` /
    /// `aliveSecond` / `bytesSpeed`），内层 `tracks` 却是 snake_case。
    ///
    /// 少一个 `rename_all = "camelCase"` 不会有任何报错：字段静默变成
    /// 0/None，表现为"观看者数量永远是 0"，进而把正在播放的流当成
    /// 无人观看而关闭。
    #[test]
    fn media_info_parses_camel_case_payload_with_snake_case_tracks() {
        let raw = r#"{
            "app": "rtp",
            "stream": "34020000001320000001_34020000001320000002",
            "schema": "rtsp",
            "vhost": "__defaultVhost__",
            "readerCount": 2,
            "totalReaderCount": 3,
            "originType": 1,
            "originUrl": "",
            "createStamp": 1700000000,
            "aliveSecond": 42,
            "bytesSpeed": 123456,
            "tracks": [
                {"codec_id": 0, "codec_id_name": "CodecH264", "codec_type": 0,
                 "ready": true, "fps": 25, "width": 1920, "height": 1080}
            ]
        }"#;
        let info: MediaInfo = serde_json::from_str(raw).expect("必须能解析真实 ZLM 载荷");
        assert_eq!(info.reader_count, 2);
        assert_eq!(info.total_reader_count, 3);
        assert_eq!(info.origin_type, 1);
        assert_eq!(info.create_stamp, 1_700_000_000);
        assert_eq!(info.alive_second, 42);
        assert_eq!(info.bytes_speed, 123_456);
        assert_eq!(info.tracks.len(), 1);
        assert_eq!(info.tracks[0].codec_id_name, "CodecH264");
        assert_eq!(info.tracks[0].width, Some(1920));
    }

    /// 字段缺失（版本差异）不能导致整条流不可见。
    #[test]
    fn media_info_tolerates_missing_optional_fields() {
        let raw = r#"{"app":"rtp","stream":"s","schema":"rtsp","vhost":"__defaultVhost__"}"#;
        let info: MediaInfo = serde_json::from_str(raw).expect("缺字段也要能解析");
        assert_eq!(info.reader_count, 0);
        assert!(info.tracks.is_empty());
        assert_eq!(info.origin_url, None);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub msg: Option<String>,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn is_success(&self) -> bool {
        self.code == 0
    }

    pub fn error_msg(&self) -> String {
        self.msg
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string())
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddStreamProxyRequest {
    pub secret: String,
    pub vhost: String,
    pub app: String,
    pub stream: String,
    pub url: String,
    pub rtp_type: Option<u32>,
    pub timeout_sec: Option<f64>,
    pub enable_hls: Option<bool>,
    pub enable_mp4: Option<bool>,
    pub enable_rtsp: Option<bool>,
    pub enable_rtmp: Option<bool>,
    pub enable_fmp4: Option<bool>,
    pub enable_ts: Option<bool>,
    pub enableAAC: Option<bool>,
}

impl Default for AddStreamProxyRequest {
    fn default() -> Self {
        Self {
            secret: String::new(),
            vhost: "__defaultVhost__".to_string(),
            app: "rtp".to_string(),
            stream: String::new(),
            url: String::new(),
            rtp_type: Some(0),
            timeout_sec: Some(30.0),
            enable_hls: Some(false),
            enable_mp4: Some(false),
            enable_rtsp: Some(true),
            enable_rtmp: Some(false),
            enable_fmp4: Some(false),
            enable_ts: Some(false),
            enableAAC: Some(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddStreamProxyResponse {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseStreamsRequest {
    pub secret: String,
    pub schema: Option<String>,
    pub vhost: Option<String>,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseStreamsResponse {
    pub count_hit: u32,
    pub count_closed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordRequest {
    pub secret: String,
    pub type_: String,
    pub vhost: String,
    pub app: String,
    pub stream: String,
    pub file_name: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapRequest {
    pub secret: String,
    pub url: String,
    pub timeout_sec: Option<f64>,
    pub save_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapResponse {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mp4RecordFile {
    pub name: String,
    pub size: u64,
    pub create_time: String,
    pub path: String,
    pub file_path: Option<String>,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mp4RecordResponse {
    pub list: Vec<Mp4RecordFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpServerInfo {
    pub port: u16,
    pub stream_id: String,
    pub ssrc: Option<String>,
    pub client_ip: Option<String>,
    pub client_port: Option<u16>,
    pub server_port: Option<u16>,
    pub selectrtp_conn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRtpServerRequest {
    pub secret: String,
    pub stream_id: String,
    pub port: Option<u16>,
    pub use_tcp: Option<bool>,
    pub rtp_type: Option<u32>,
    pub recv_port: Option<u16>,
}

impl Default for OpenRtpServerRequest {
    fn default() -> Self {
        Self {
            secret: String::new(),
            stream_id: String::new(),
            port: None,
            use_tcp: Some(false),
            rtp_type: Some(0),
            recv_port: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpInfo {
    pub stream_id: String,
    pub ssrc: String,
    pub peer_ip: String,
    pub peer_port: u16,
    pub local_port: u16,
    pub alive_second: u32,
    pub rtt: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub api_enabled: Option<bool>,
    pub api_debug: Option<bool>,
    pub port: Option<u16>,
    pub ssl_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUrlInfo {
    pub url: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    pub schema: String,
    pub master: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitLogo {
    pub url: String,
    pub x: i32,
    pub y: i32,
    pub宽: Option<i32>,
    pub high: Option<i32>,
    pub timeout: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddFFmpegSourceRequest {
    pub secret: String,
    pub src_url: String,
    pub dst_url: String,
    pub timeout_ms: Option<u32>,
    pub ffmpeg_cmd_key: Option<String>,
    pub enable_hls: Option<bool>,
    pub enable_mp4: Option<bool>,
    pub enable_rtsp: Option<bool>,
    pub enable_rtmp: Option<bool>,
    pub enable_fmp4: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZlmMediaInfo {
    pub exist: bool,
    pub schema: Option<String>,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub vhost: Option<String>,
    pub reader_count: Option<u32>,
    pub total_reader_count: Option<u32>,
    pub origin_type: Option<u32>,
    pub origin_url: Option<String>,
    pub create_stamp: Option<i64>,
    pub alive_second: Option<u32>,
    pub bytes_speed: Option<u64>,
    pub tracks: Option<Vec<TrackInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickSessionRequest {
    pub secret: String,
    pub vhost: String,
    pub app: String,
    pub stream: String,
    pub schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickSessionResponse {
    pub hit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub url: String,
    pub file_name: String,
    pub save_path: String,
    pub status: String,
    pub progress: f64,
    pub size: u64,
    pub downloaded: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub secret: String,
    pub url: String,
    pub file_name: String,
    pub save_path: Option<String>,
}

impl Default for DownloadRequest {
    fn default() -> Self {
        Self {
            secret: String::new(),
            url: String::new(),
            file_name: String::new(),
            save_path: Some("./".to_string()),
        }
    }
}
