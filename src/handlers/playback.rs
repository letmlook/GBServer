use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct PlaybackSession {
    pub stream_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub speed: f64,
    pub paused: bool,
}

pub struct PlaybackManager {
    sessions: Arc<RwLock<std::collections::HashMap<String, PlaybackSession>>>,
}

impl PlaybackManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn create(&self, session: PlaybackSession) {
        self.sessions.write().await.insert(session.stream_id.clone(), session);
    }

    pub async fn get(&self, stream_id: &str) -> Option<PlaybackSession> {
        self.sessions.read().await.get(stream_id).cloned()
    }

    pub async fn update_speed(&self, stream_id: &str, speed: f64) {
        if let Some(mut s) = self.sessions.write().await.get_mut(stream_id) {
            s.speed = speed;
        }
    }

    pub async fn pause(&self, stream_id: &str) {
        if let Some(mut s) = self.sessions.write().await.get_mut(stream_id) {
            s.paused = true;
        }
    }

    pub async fn resume(&self, stream_id: &str) {
        if let Some(mut s) = self.sessions.write().await.get_mut(stream_id) {
            s.paused = false;
        }
    }

    pub async fn remove(&self, stream_id: &str) {
        self.sessions.write().await.remove(stream_id);
    }
}

impl Default for PlaybackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
pub struct PlaybackQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

pub async fn playback_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<PlaybackQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let start_time = q.start_time.clone().unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
    });
    let end_time = q.end_time.clone();

    tracing::info!("Playback start: device={}, channel={}, start={}", device_id, channel_id, start_time);

    let stream_id = format!("playback_{}_{}_{}", device_id, channel_id, 
        chrono::Utc::now().timestamp());

    if let Some(ref zlm_client) = state.zlm_client {
        let proxy_url = format!("rtsp://127.0.0.1/live/{}/{}", device_id, channel_id);
        let request = crate::zlm::AddStreamProxyRequest {
            secret: zlm_client.secret.clone(),
            vhost: "__defaultVhost__".to_string(),
            app: "playback".to_string(),
            stream: format!("{}@{}", device_id, channel_id),
            url: proxy_url,
            rtp_type: Some(0),
            timeout_sec: Some(3600.0),
            enable_hls: Some(false),
            enable_mp4: Some(false),
            enable_rtsp: Some(true),
            enable_rtmp: Some(true),
            enable_fmp4: Some(true),
        };

        match zlm_client.add_stream_proxy(&request).await {
            Ok(key) => {
                tracing::info!("Playback stream started: {}", key);
                return Json(WVPResult::success(serde_json::json!({
                    "streamId": stream_id,
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "app": "playback",
                    "stream": key,
                    "playUrl": format!("rtsp://127.0.0.1/playback/{}", key),
                    "startTime": start_time,
                    "endTime": end_time,
                    "currentTime": start_time,
                    "speed": 1.0
                })));
            }
            Err(e) => {
                tracing::error!("Failed to start playback stream: {}", e);
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "deviceId": device_id,
        "channelId": channel_id,
        "msg": "Playback not available"
    })))
}

pub async fn playback_resume(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<WVPResult<()>> {
    tracing::info!("Playback resume: stream={}", stream_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn playback_pause(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<WVPResult<()>> {
    tracing::info!("Playback pause: stream={}", stream_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn playback_speed(
    State(state): State<AppState>,
    Path((stream_id, speed)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    let speed: f64 = speed.parse().unwrap_or(1.0);
    tracing::info!("Playback speed: stream={}, speed={}", stream_id, speed);
    Json(WVPResult::<()>::success_empty())
}

pub async fn playback_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Playback stop: device={}, channel={}, stream={}", device_id, channel_id, stream_id);

    if let Some(ref zlm_client) = state.zlm_client {
        let _ = zlm_client.close_streams(
            Some("rtsp"),
            Some("playback"),
            Some(&format!("{}@{}", device_id, channel_id)),
            true
        ).await;
    }

    Json(WVPResult::<()>::success_empty())
}

#[derive(Debug, Deserialize)]
pub struct RecordQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub count: Option<u32>,
}

pub async fn gb_record_query(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<RecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Record query: device={}, channel={}", device_id, channel_id);

    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client.get_mp4_record_file("record", &channel_id, None).await {
            Ok(files) => {
                let records: Vec<serde_json::Value> = files.iter().map(|f| {
                    serde_json::json!({
                        "fileName": f.name,
                        "filePath": f.path,
                        "fileSize": f.size,
                        "startTime": f.create_time,
                        "endTime": f.create_time,
                        "downloadUrl": format!("/record/{}", f.name)
                    })
                }).collect();

                return Json(WVPResult::success(serde_json::json!({
                    "list": records,
                    "total": records.len()
                })));
            }
            Err(e) => {
                tracing::error!("Failed to query records: {}", e);
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "list": [],
        "total": 0
    })))
}

pub async fn gb_record_download_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<RecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();

    tracing::info!("Record download: device={}, channel={}, start={}, end={}",
        device_id, channel_id, start_time, end_time);

    let stream_id = format!("download_{}_{}_{}", device_id, channel_id,
        chrono::Utc::now().timestamp());

    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "downloadUrl": format!("/download/{}/{}/{}.mp4", device_id, channel_id, stream_id),
        "progress": 0
    })))
}

pub async fn gb_record_download_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Record download stop: device={}, channel={}, stream={}",
        device_id, channel_id, stream_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn gb_record_download_progress(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "progress": 100,
        "status": "completed"
    })))
}
