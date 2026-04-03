use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZlmServerInfo {
    pub id: String,
    pub ip: String,
    pub http_port: u16,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub app: String,
    pub stream: String,
    pub schema: String,
    pub vhost: String,
    pub reader_count: u32,
    pub total_reader_count: u32,
    pub origin_type: u32,
    pub origin_url: Option<String>,
    pub create_stamp: i64,
    pub alive_second: u32,
    pub bytes_speed: u64,
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
