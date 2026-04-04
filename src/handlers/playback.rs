use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct PlaybackSession {
    pub stream_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub app: String,
    pub stream: String,
    pub media_server_id: Option<String>,
    pub schema: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub current_time: String,
    pub speed: f64,
    pub paused: bool,
    pub source: String,
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

    pub async fn update_current_time(&self, stream_id: &str, current_time: String) {
        if let Some(mut s) = self.sessions.write().await.get_mut(stream_id) {
            s.current_time = current_time;
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

#[derive(Debug, Clone)]
pub struct DownloadSession {
    pub stream_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub file_name: String,
    pub start_time: String,
    pub end_time: String,
    pub url: String,
    pub status: String,
    pub progress: f64,
    pub created_at: DateTime<Utc>,
}

pub struct DownloadManager {
    sessions: Arc<RwLock<std::collections::HashMap<String, DownloadSession>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn create(&self, session: DownloadSession) {
        self.sessions.write().await.insert(session.stream_id.clone(), session);
    }

    pub async fn get(&self, stream_id: &str) -> Option<DownloadSession> {
        self.sessions.read().await.get(stream_id).cloned()
    }

    pub async fn update_progress(&self, stream_id: &str, progress: f64, status: &str) {
        if let Some(mut s) = self.sessions.write().await.get_mut(stream_id) {
            s.progress = progress;
            s.status = status.to_string();
        }
    }

    pub async fn remove(&self, stream_id: &str) {
        self.sessions.write().await.remove(stream_id);
    }

    pub async fn get_all(&self) -> Vec<DownloadSession> {
        self.sessions.read().await.values().cloned().collect()
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
pub struct PlaybackQuery {
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
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
    let app = "playback".to_string();
    let stream = format!("{}${}", device_id, channel_id);
    let media_server_id = state.list_zlm_servers().into_iter().next();
    let mut source = "zlm_proxy".to_string();

    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let end = end_time.clone().unwrap_or_else(|| start_time.clone());
        if sip
            .send_playback_invite(&device_id, &channel_id, &start_time, &end)
            .await
            .is_ok()
        {
            source = "gb28181_invite".to_string();
        }
    }

    if let Some(ref zlm_client) = state.zlm_client {
        let proxy_url = format!("rtsp://127.0.0.1/live/{}/{}", device_id, channel_id);
        let request = crate::zlm::AddStreamProxyRequest {
            secret: zlm_client.secret.clone(),
            vhost: "__defaultVhost__".to_string(),
            app: app.clone(),
            stream: stream.clone(),
            url: proxy_url,
            rtp_type: Some(0),
            timeout_sec: Some(3600.0),
            enable_hls: Some(false),
            enable_mp4: Some(false),
            enable_rtsp: Some(true),
            enable_rtmp: Some(true),
            enable_fmp4: Some(true),
            enable_ts: Some(false),
            enableAAC: Some(false),
        };

        match zlm_client.add_stream_proxy(&request).await {
            Ok(key) => {
                tracing::info!("Playback stream started: {}", key);
                if let Some(ref playback_manager) = state.playback_manager {
                    playback_manager.create(PlaybackSession {
                        stream_id: stream_id.clone(),
                        device_id: device_id.clone(),
                        channel_id: channel_id.clone(),
                        app: app.clone(),
                        stream: key.clone(),
                        media_server_id: media_server_id.clone(),
                        schema: "rtsp".to_string(),
                        start_time: start_time.clone(),
                        end_time: end_time.clone(),
                        current_time: start_time.clone(),
                        speed: 1.0,
                        paused: false,
                        source: source.clone(),
                    }).await;
                }
                return Json(WVPResult::success(serde_json::json!({
                    "streamId": stream_id,
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "app": app,
                    "stream": key,
                    "playUrl": format!("rtsp://127.0.0.1/playback/{}", key),
                    "startTime": start_time,
                    "endTime": end_time,
                    "currentTime": start_time,
                    "speed": 1.0,
                    "source": source
                })));
            }
            Err(e) => {
                tracing::error!("Failed to start playback stream: {}", e);
            }
        }
    }

    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.create(PlaybackSession {
            stream_id: stream_id.clone(),
            device_id: device_id.clone(),
            channel_id: channel_id.clone(),
            app: app.clone(),
            stream: stream.clone(),
            media_server_id: media_server_id.clone(),
            schema: "rtsp".to_string(),
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            current_time: start_time.clone(),
            speed: 1.0,
            paused: false,
            source: source.clone(),
        }).await;
    }

    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "deviceId": device_id,
        "channelId": channel_id,
        "app": app,
        "stream": stream,
        "startTime": start_time,
        "endTime": end_time,
        "currentTime": start_time,
        "speed": 1.0,
        "source": source,
        "msg": "Playback session created"
    })))
}

pub async fn playback_resume(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Playback resume: stream={}", stream_id);
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.resume(&stream_id).await;
    }
    
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let parts: Vec<&str> = stream_id.split('_').collect();
        if parts.len() >= 3 {
            let device_id = parts[1];
            let channel_id = parts[2];
            
            if let Err(e) = sip.send_message_to_device(device_id, crate::sip::SipMethod::Info,
                Some(r#"<?xml version="1.0" encoding="UTF-8"?><Resume><ChannelID>0</ChannelID></Resume>"#),
                Some("Application/MANSCDP+xml")).await {
                tracing::error!("Failed to send resume command: {}", e);
                return Json(WVPResult::error(format!("SIP error: {}", e)));
            }
        }
    }
    
    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "status": "playing",
        "message": "Playback resumed"
    })))
}

pub async fn playback_pause(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Playback pause: stream={}", stream_id);
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.pause(&stream_id).await;
    }
    
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let parts: Vec<&str> = stream_id.split('_').collect();
        if parts.len() >= 3 {
            let device_id = parts[1];
            let channel_id = parts[2];
            
            if let Err(e) = sip.send_message_to_device(device_id, crate::sip::SipMethod::Info, 
                Some(r#"<?xml version="1.0" encoding="UTF-8"?><Pause><ChannelID>0</ChannelID></Pause>"#),
                Some("Application/MANSCDP+xml")).await {
                tracing::error!("Failed to send pause command: {}", e);
                return Json(WVPResult::error(format!("SIP error: {}", e)));
            }
        }
    }
    
    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "status": "paused",
        "message": "Playback paused"
    })))
}

pub async fn playback_speed(
    State(state): State<AppState>,
    Path((stream_id, speed)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    let speed: f64 = speed.parse().unwrap_or(1.0);
    tracing::info!("Playback speed: stream={}, speed={}", stream_id, speed);
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.update_speed(&stream_id, speed).await;
    }
    
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let parts: Vec<&str> = stream_id.split('_').collect();
        if parts.len() >= 3 {
            let device_id = parts[1];
            let channel_id = parts[2];
            
            let speed_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><PlaybackSpeed><ChannelID>0</ChannelID><Speed>{}</Speed></PlaybackSpeed>"#,
                speed
            );
            
            if let Err(e) = sip.send_message_to_device(device_id, crate::sip::SipMethod::Info,
                Some(&speed_xml),
                Some("Application/MANSCDP+xml")).await {
                tracing::error!("Failed to send speed command: {}", e);
                return Json(WVPResult::error(format!("SIP error: {}", e)));
            }
        }
    }
    
    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "speed": speed,
        "message": "Playback speed updated"
    })))
}

pub async fn playback_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Playback stop: device={}, channel={}, stream={}", device_id, channel_id, stream_id);

    if let Some(ref playback_manager) = state.playback_manager {
        if let Some(session) = playback_manager.get(&stream_id).await {
            if let Some(zlm_client) = state
                .get_zlm_client(session.media_server_id.as_deref())
                .or_else(|| state.zlm_client.clone())
            {
                let _ = zlm_client.close_streams(
                    Some(&session.schema),
                    Some(&session.app),
                    Some(&session.stream),
                    true,
                ).await;
            }
            playback_manager.remove(&stream_id).await;
            return Json(WVPResult::<()>::success_empty());
        }
    }

    if let Some(ref zlm_client) = state.zlm_client {
        let _ = zlm_client.close_streams(
            Some("rtsp"),
            Some("playback"),
            Some(&format!("{}${}", device_id, channel_id)),
            true
        ).await;
    }

    Json(WVPResult::<()>::success_empty())
}

#[derive(Debug, Deserialize)]
pub struct RecordQuery {
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
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
        match zlm_client.get_mp4_record_file("record", &channel_id, None, None, None).await {
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
    
    let file_name = format!("{}_{}_{}_{}.mp4", device_id, channel_id, 
        start_time.replace(":", "").replace("-", "").replace("T", "_"),
        chrono::Utc::now().timestamp());

    if let Some(ref zlm_client) = state.zlm_client {
        let record_url = format!("rtsp://127.0.0.1/record/{}/{}", device_id, channel_id);
        
        match zlm_client.create_download(&record_url, &file_name, Some("./downloads")).await {
            Ok(download_path) => {
                tracing::info!("Download started: {} -> {}", file_name, download_path);
                
                let session = DownloadSession {
                    stream_id: stream_id.clone(),
                    device_id: device_id.clone(),
                    channel_id: channel_id.clone(),
                    file_name: file_name.clone(),
                    start_time: start_time.clone(),
                    end_time: end_time.clone(),
                    url: record_url,
                    status: "downloading".to_string(),
                    progress: 0.0,
                    created_at: Utc::now(),
                };
                
                if let Some(ref dm) = state.download_manager {
                    dm.create(session).await;
                }
                
                return Json(WVPResult::success(serde_json::json!({
                    "streamId": stream_id,
                    "fileName": file_name,
                    "downloadUrl": format!("/download/{}", file_name),
                    "savePath": download_path,
                    "progress": 0,
                    "status": "downloading"
                })));
            }
            Err(e) => {
                tracing::error!("Failed to start download: {}", e);
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "msg": "Download not available"
    })))
}

pub async fn gb_record_download_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Record download stop: device={}, channel={}, stream={}",
        device_id, channel_id, stream_id);

    if let Some(ref zlm_client) = state.zlm_client {
        if let Some(ref dm) = state.download_manager {
            if let Some(session) = dm.get(&stream_id).await {
                let _ = zlm_client.stop_download(&session.file_name).await;
                dm.remove(&stream_id).await;
            }
        }
    }

    Json(WVPResult::<()>::success_empty())
}

pub async fn gb_record_download_progress(
    State(state): State<AppState>,
    Path((device_id, channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    if let Some(ref zlm_client) = state.zlm_client {
        if let Some(ref dm) = state.download_manager {
            if let Some(session) = dm.get(&stream_id).await {
                match zlm_client.get_download_list().await {
                    Ok(downloads) => {
                        for dl in downloads {
                            if dl.file_name == session.file_name {
                                let progress = dl.progress;
                                let status = if progress >= 100.0 { "completed" } else { "downloading" };
                                
                                dm.update_progress(&stream_id, progress, status).await;
                                
                                return Json(WVPResult::success(serde_json::json!({
                                    "streamId": stream_id,
                                    "fileName": dl.file_name,
                                    "progress": progress,
                                    "status": status,
                                    "downloaded": dl.downloaded,
                                    "totalSize": dl.size
                                })));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to get download progress: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "progress": 0,
        "status": "unknown"
    })))
}
