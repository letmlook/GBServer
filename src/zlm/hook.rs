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

fn parse_stream_id(stream: &str) -> Option<(String, String)> {
    if let Some(pos) = stream.find('$') {
        let device_id = stream[..pos].to_string();
        let channel_id = stream[pos + 1..].to_string();
        if device_id.len() == 20 || device_id.len() == 22 {
            return Some((device_id, channel_id));
        }
    }
    if let Some(pos) = stream.find('/') {
        let parts: Vec<&str> = stream.split('/').collect();
        if parts.len() >= 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
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

                if let Some((device_id, channel_id)) = parse_stream_id(&data.stream) {
                    tracing::info!("Attempting auto-pull for device={} channel={}", device_id, channel_id);
                    
                    if let Some(ref zlm_client) = state.zlm_client {
                        let pull_url = format!("rtsp://{}:8554/{}", device_id, channel_id);
                        
                        let proxy_req = crate::zlm::AddStreamProxyRequest {
                            secret: zlm_client.secret.clone(),
                            vhost: "__defaultVhost__".to_string(),
                            app: data.app.clone(),
                            stream: data.stream.clone(),
                            url: pull_url.clone(),
                            rtp_type: Some(0),
                            timeout_sec: Some(30.0),
                            enable_hls: Some(false),
                            enable_mp4: Some(false),
                            enable_rtsp: Some(true),
                            enable_rtmp: Some(false),
                            enable_fmp4: Some(false),
                            enable_ts: Some(false),
                            enableAAC: Some(false),
                        };

                        match zlm_client.add_stream_proxy(&proxy_req).await {
                            Ok(stream_key) => {
                                tracing::info!("Auto-pull started: {} -> {}", data.stream, stream_key);
                                return Json(WVPResult::success(serde_json::json!({
                                    "code": 0,
                                    "stream": stream_key,
                                    "url": pull_url
                                })));
                            }
                            Err(e) => {
                                tracing::error!("Auto-pull failed: {}", e);
                            }
                        }
                    }
                }
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
