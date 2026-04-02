//! ZLM HTTP API 类型定义

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mp4RecordResponse {
    pub list: Vec<Mp4RecordFile>,
}
