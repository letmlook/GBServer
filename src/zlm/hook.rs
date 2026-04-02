//! ZLM Webhook 处理

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebHookRequest {
    pub hook_name: String,
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hook_name")]
pub enum WebHookEvent {
    #[serde(rename = "on_stream_changed")]
    StreamChanged(StreamChangedData),
    #[serde(rename = "on_stream_not_found")]
    StreamNotFound(StreamNotFoundData),
    #[serde(rename = "on_record_mp4")]
    RecordMp4(RecordMp4Data),
    #[serde(rename = "on_record_hls")]
    RecordHls(RecordHlsData),
    #[serde(rename = "on_play")]
    Play(PlayData),
    #[serde(rename = "on_publish")]
    Publish(PublishData),
    #[serde(rename = "on_server_started")]
    ServerStarted(ServerStartedData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChangedData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    pub register: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamNotFoundData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    pub ssrc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordMp4Data {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    pub file_name: String,
    pub file_path: String,
    pub file_size: u64,
    pub file_duration: f64,
    pub file_create_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordHlsData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    pub file_name: String,
    pub file_path: String,
    pub file_size: u64,
    pub file_duration: f64,
    pub file_create_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayData {
    pub ip: String,
    pub port: u16,
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishData {
    pub ip: String,
    pub port: u16,
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStartedData {
    pub port: u16,
    pub hook_port: u16,
    pub rtsp_port: u16,
    pub rtmp_port: u16,
    pub http_port: u16,
    pub https_port: u16,
}

pub async fn handle_webhook(
    State(state): State<AppState>,
    Json(event): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let hook_name = event.get("hook_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match hook_name {
        "on_stream_changed" => {
            if let Some(data) = event.get("schema").and_then(|_| {
                serde_json::from_value::<StreamChangedData>(event.clone()).ok()
            }) {
                tracing::info!("Stream changed: {}/{}/{} register={}", 
                    data.schema, data.app, data.stream, data.register);
            }
        }
        "on_stream_not_found" => {
            if let Some(data) = serde_json::from_value::<StreamNotFoundData>(event.clone()).ok() {
                tracing::warn!("Stream not found: {}/{}/{}", 
                    data.schema, data.app, data.stream);
            }
        }
        "on_record_mp4" => {
            if let Some(data) = serde_json::from_value::<RecordMp4Data>(event.clone()).ok() {
                tracing::info!("MP4 recorded: {} ({} bytes)", 
                    data.file_name, data.file_size);
            }
        }
        "on_play" => {
            if let Some(data) = serde_json::from_value::<PlayData>(event.clone()).ok() {
                tracing::info!("Play request: {}/{}/{} from {}", 
                    data.schema, data.app, data.stream, data.ip);
            }
        }
        "on_publish" => {
            if let Some(data) = serde_json::from_value::<PublishData>(event.clone()).ok() {
                tracing::info!("Publish: {}/{}/{} from {}", 
                    data.schema, data.app, data.stream, data.ip);
            }
        }
        "on_server_started" => {
            tracing::info!("ZLM server started");
        }
        _ => {
            tracing::debug!("Unhandled webhook: {}", hook_name);
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "code": 0
    })))
}
