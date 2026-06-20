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

    /// 从 `playback_{device_id}_{channel_id}_{ts}` 格式的 stream_id 解析
/// 出 device_id 和 channel_id。GB28181 ID 自身不含下划线，
/// 下划线分隔符安全。
fn parse_playback_target(stream_id: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = stream_id.split('_').collect();
    if parts.len() >= 3 && parts[0] == "playback" {
        Some((parts[1].to_string(), parts[2].to_string()))
    } else {
        None
    }
}

pub async fn create(&self, session: PlaybackSession) {
        self.sessions.write().await.insert(session.stream_id.clone(), session);
    }

    pub async fn get(&self, stream_id: &str) -> Option<PlaybackSession> {
        self.sessions.read().await.get(stream_id).cloned()
    }

    pub async fn update_speed(&self, stream_id: &str, speed: f64) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.speed = speed;
        }
    }

    pub async fn update_current_time(&self, stream_id: &str, current_time: String) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.current_time = current_time;
        }
    }

    pub async fn pause(&self, stream_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.paused = true;
        }
    }

    pub async fn resume(&self, stream_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
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
    /// Phase 3.4: ZLM 流标识（与 ZLM `app/stream` 对应，用于 hook 回调匹配）
    pub zlm_stream_id: String,
    /// Phase 3.4: ZLM app（默认 "rtp"）
    pub zlm_app: String,
    /// Phase 3.4: 累计已下载字节数（来自 ZLM on_stream_changed 钩子）
    pub current_bytes: i64,
    /// Phase 3.4: 目标字节数（来自 ZLM 估算或 start/end_time 推算）
    pub total_bytes: i64,
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

    /// Phase 3.4: 进度更新用绝对字节数（current_bytes / total_bytes * 100.0）
    /// 对外仍返回百分比。
    pub async fn update_progress(&self, stream_id: &str, current_bytes: i64, total_bytes: i64) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
            s.current_bytes = current_bytes;
            s.total_bytes = total_bytes;
            if total_bytes > 0 {
                s.progress = ((current_bytes as f64) / (total_bytes as f64) * 100.0)
                    .clamp(0.0, 100.0);
            }
            s.status = if current_bytes >= total_bytes && total_bytes > 0 {
                "completed".to_string()
            } else {
                "downloading".to_string()
            };
        }
    }

    /// 兼容旧 API：按 0..100 模糊语义更新
    pub async fn update_progress_percent(&self, stream_id: &str, progress: f64, status: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(stream_id) {
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

    /// Phase 3.4: 按 zlm_stream_id 查找（ZLM hook on_stream_changed 调用）
    pub async fn get_by_zlm_stream(&self, zlm_stream_id: &str) -> Option<DownloadSession> {
        self.sessions
            .read()
            .await
            .values()
            .find(|s| s.zlm_stream_id == zlm_stream_id)
            .cloned()
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

    // Phase 3.2: 走真实 GB28181 Playback INVITE + ZLM RTP server 媒体等待
    if let Some(ref sip_server) = state.sip_server {
        if let Some(ref zlm_client) = state.zlm_client {
            // 1. 先开 ZLM RTP server（端口由 ZLM 自动分配）
            let rtp_req = crate::zlm::OpenRtpServerRequest {
                secret: zlm_client.secret.clone(),
                stream_id: stream_id.clone(),
                port: Some(0),
                use_tcp: Some(false),
                rtp_type: Some(0),
                recv_port: None,
            };
            match zlm_client.open_rtp_server(&rtp_req).await {
                Ok(rtp_server) => {
                    // 2. 发 SIP INVITE + 等 ZLM 媒体到达
                    let sip = sip_server.read().await;
                    let end = end_time.clone().unwrap_or_else(|| start_time.clone());
                    match sip
                        .send_playback_invite_and_wait(
                            &device_id, &channel_id,
                            &start_time, &end, &stream_id, rtp_server.port, 15,
                        )
                        .await
                    {
                        Ok((_call_id, _zlm_stream_id)) => {
                            source = "gb28181_playback_invite".to_string();
                            let media_ip = zlm_client.ip.clone();
                            let http_port = zlm_client.http_port;
                            let play_url = format!("rtsp://{}:554/{}/{}", media_ip, app, stream_id);
                            let flv_url = format!("http://{}:{}/{}/{}.flv", media_ip, http_port, app, stream_id);
                            let hls_url = format!("http://{}:{}/{}/{}/hls.m3u8", media_ip, http_port, app, stream_id);
                            if let Some(ref playback_manager) = state.playback_manager {
                                playback_manager.create(PlaybackSession {
                                    stream_id: stream_id.clone(),
                                    device_id: device_id.clone(),
                                    channel_id: channel_id.clone(),
                                    app: app.clone(),
                                    stream: stream_id.clone(),
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
                                "stream": stream_id,
                                "playUrl": play_url,
                                "flvUrl": flv_url,
                                "hls": hls_url,
                                "startTime": start_time,
                                "endTime": end_time,
                                "currentTime": start_time,
                                "speed": 1.0,
                                "source": source
                            })));
                        }
                        Err(e) => {
                            tracing::error!("Playback INVITE + media wait failed: {}", e);
                            // 清理已开 RTP 端口
                            let _ = zlm_client.close_rtp_server(&stream_id).await;
                            // 兜底发 BYE
                            let _ = sip.send_session_bye(&device_id, &channel_id).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to open ZLM RTP server for playback: {}", e);
                }
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

    // Phase 3.2: 用 send_playback_control 走规范 PlaybackCtrl 消息（替代裸 XML）
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let parts: Vec<&str> = stream_id.split('_').collect();
        if parts.len() >= 3 {
            let device_id = parts[1];
            let channel_id = parts[2].to_string();

            if let Err(e) = sip
                .send_playback_control(
                    device_id,
                    &channel_id,
                    crate::sip::PlaybackControlCmd::Resume,
                )
                .await
            {
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

    // Phase 3.2: 用 send_playback_control 走规范 PlaybackCtrl 消息（替代裸 XML）
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let parts: Vec<&str> = stream_id.split('_').collect();
        if parts.len() >= 3 {
            let device_id = parts[1];
            let channel_id = parts[2].to_string();

            if let Err(e) = sip
                .send_playback_control(
                    device_id,
                    &channel_id,
                    crate::sip::PlaybackControlCmd::Pause,
                )
                .await
            {
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
            let channel_id = parts[2].to_string();
            
            if let Err(e) = sip
                .send_playback_control(
                    device_id,
                    &channel_id,
                    crate::sip::PlaybackControlCmd::Scale { speed },
                )
                .await {
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

/// 回放拖动定位（seek）
pub async fn playback_seek(
    State(state): State<AppState>,
    Path((stream_id, seek_time)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Playback seek: stream={}, time={}", stream_id, seek_time);
    
    // 更新本地会话状态
    if let Some(ref playback_manager) = state.playback_manager {
        playback_manager.update_current_time(&stream_id, seek_time.clone()).await;
    }
    
    // 发送 SIP INFO 消息通知设备跳转
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        let parts: Vec<&str> = stream_id.split('_').collect();
        if parts.len() >= 3 {
            let device_id = parts[1];
            let channel_id = parts[2];
            
            if let Err(e) = sip
                .send_playback_control(
                    device_id,
                    &channel_id,
                    crate::sip::PlaybackControlCmd::Seek {
                        seek_time: seek_time.clone(),
                    },
                )
                .await {
                tracing::error!("Failed to send seek command: {}", e);
                return Json(WVPResult::error(format!("SIP error: {}", e)));
            }
        }
    }
    
    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "currentTime": seek_time,
        "message": "Playback seeked"
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

            // Send SIP BYE to stop device playback push
            if let Some(ref sip_server) = state.sip_server {
                let sip = sip_server.read().await;
                match sip.send_session_bye(&device_id, &channel_id).await {
                    Ok(call_id) => tracing::info!("Playback BYE sent call_id={}", call_id),
                    Err(e) => tracing::warn!("Failed to send playback BYE: {}", e),
                }
            }

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

    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();
    let page = q.page.unwrap_or(1).max(1) as i64;
    let count = q.count.unwrap_or(20).clamp(1, 200) as i64;

    // Phase 3.3: 真正等 SIP 多包 RecordInfo 响应（最多 15s），返回聚合 items
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        if let Some(device) = sip.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                let sn = chrono::Utc::now().timestamp() % 10000;
                match sip
                    .send_record_info_query_and_wait(
                        &device_id, &channel_id, &start_time, &end_time, sn,
                    )
                    .await
                {
                    Ok(items) if !items.is_empty() => {
                        // 分页：page 从 1 开始，count 由调用方控制
                        let total = items.len() as i64;
                        let offset = ((page - 1) * count) as usize;
                        let paged: Vec<serde_json::Value> = items
                            .iter()
                            .skip(offset)
                            .take(count as usize)
                            .map(|it| {
                                serde_json::json!({
                                    "deviceId": it.device_id,
                                    "name": it.name,
                                    "filePath": it.file_path,
                                    "startTime": it.start_time,
                                    "endTime": it.end_time,
                                    "address": it.address,
                                    "secrecy": it.secrecy,
                                    "type": it.kind,
                                })
                            })
                            .collect();
                        return Json(WVPResult::success(serde_json::json!({
                            "list": paged,
                            "total": total,
                            "page": page,
                            "count": count,
                            "source": "gb28181_record_info"
                        })));
                    }
                    Ok(_) => {
                        // 空 items：设备回了 RecordInfo 但没有录像段；
                        // 继续走 ZLM 兜底（兼容历史 ZLM MP4 文件）
                    }
                    Err(e) => {
                        tracing::warn!("RecordInfo async wait failed: {}", e);
                    }
                }
            }
        }
    }

    // 兼容路径：ZLM 本地 MP4 文件列表
    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client.get_mp4_record_file("record", &channel_id, None, None, None).await {
            Ok(files) => {
                let total = files.len() as i64;
                let offset = ((page - 1) * count) as usize;
                let records: Vec<serde_json::Value> = files
                    .iter()
                    .skip(offset)
                    .take(count as usize)
                    .map(|f| {
                        serde_json::json!({
                            "fileName": f.name,
                            "filePath": f.path,
                            "fileSize": f.size,
                            "startTime": f.create_time,
                            "endTime": f.create_time,
                            "downloadUrl": format!("/record/{}", f.name)
                        })
                    })
                    .collect();

                return Json(WVPResult::success(serde_json::json!({
                    "list": records,
                    "total": total,
                    "page": page,
                    "count": count,
                    "source": "zlm_mp4"
                })));
            }
            Err(e) => {
                tracing::error!("Failed to query records: {}", e);
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "list": [],
        "total": 0,
        "page": page,
        "count": count
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

    // 1) 优先走 GB28181 录像下载 INVITE（Subject SSRC 前缀 2），
    //    设备推送 9102 端口的 RTP 流，ZLM 自动 MP4 落盘
    let mut used_gb28181 = false;
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.read().await;
        if let Some(device) = sip.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                // 提前开 ZLM RTP server 监听设备推流
                if let Some(ref zlm) = state.zlm_client {
                    let _ = zlm
                        .open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                            secret: zlm.secret.clone(),
                            stream_id: stream_id.clone(),
                            port: Some(0),
                            use_tcp: Some(false),
                            rtp_type: Some(0),
                            recv_port: None,
                        })
                        .await;
                }
                match sip
                    .send_download_invite(&device_id, &channel_id, &start_time, &end_time)
                    .await
                {
                    Ok(call_id) => {
                        tracing::info!(
                            "GB28181 DOWNLOAD INVITE sent, call_id={}",
                            call_id
                        );
                        used_gb28181 = true;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to send GB28181 DOWNLOAD INVITE: {}", e);
                    }
                }
            }
        }
    }

    if used_gb28181 {
        let session = DownloadSession {
            stream_id: stream_id.clone(),
            device_id: device_id.clone(),
            channel_id: channel_id.clone(),
            file_name: file_name.clone(),
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            url: format!("gb28181://{}@{}/{}", device_id, channel_id, start_time),
            status: "downloading".to_string(),
            progress: 0.0,
            created_at: Utc::now(),
            zlm_stream_id: stream_id.clone(),
            zlm_app: "rtp".to_string(),
            current_bytes: 0,
            total_bytes: 0,
        };
        // Phase 3.4: 注册 media waiter，等设备推流到达；流到达后状态从 inviting → downloading
        if let Some(ref sip_server) = state.sip_server {
            let sip = sip_server.read().await;
            let (_key, _rx) = sip
                .media_waiter_manager()
                .register(&format!("dlw_{}", stream_id), &stream_id, "rtp", 15);
        }
        if let Some(ref dm) = state.download_manager {
            dm.create(session).await;
        }
        return Json(WVPResult::success(serde_json::json!({
            "streamId": stream_id,
            "fileName": file_name,
            "downloadUrl": format!("/download/{}", file_name),
            "transport": "gb28181",
            "progress": 0,
            "status": "inviting"
        })));
    }

    // 2) Fallback：ZLM 本地录像拉流（RTSP 拉 + 写文件）
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
                    zlm_stream_id: stream_id.clone(),
                    zlm_app: "rtp".to_string(),
                    current_bytes: 0,
                    total_bytes: 0,
                };

                if let Some(ref dm) = state.download_manager {
                    dm.create(session).await;
                }

                return Json(WVPResult::success(serde_json::json!({
                    "streamId": stream_id,
                    "fileName": file_name,
                    "downloadUrl": format!("/download/{}", file_name),
                    "savePath": download_path,
                    "transport": "zlm-local",
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

    // 如果是 GB28181 下载会话：发 BYE + 关 ZLM RTP server
    if let Some(ref dm) = state.download_manager {
        if let Some(session) = dm.get(&stream_id).await {
            if session.url.starts_with("gb28181://") {
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    if let Err(e) = sip
                        .send_session_bye(&session.device_id, &session.channel_id)
                        .await
                    {
                        tracing::warn!("GB28181 download BYE failed: {}", e);
                    }
                }
                if let Some(ref zlm_client) = state.zlm_client {
                    let _ = zlm_client.close_rtp_server(&stream_id).await;
                }
            } else if let Some(ref zlm_client) = state.zlm_client {
                let _ = zlm_client.stop_download(&session.file_name).await;
            }
            dm.remove(&stream_id).await;
        }
    }

    Json(WVPResult::<()>::success_empty())
}

pub async fn gb_record_download_progress(
    State(state): State<AppState>,
    Path((_device_id, _channel_id, stream_id)): Path<(String, String, String)>,
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

                                // Phase 3.4: ZLM MP4 下载进度用百分比更新（兼容路径）
                                dm.update_progress_percent(&stream_id, progress, status).await;
                                
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

#[cfg(test)]
mod download_manager_tests {
    use super::*;

    fn make_session(stream_id: &str, zlm_stream_id: &str) -> DownloadSession {
        DownloadSession {
            stream_id: stream_id.to_string(),
            device_id: "dev1".to_string(),
            channel_id: "ch1".to_string(),
            file_name: "test.mp4".to_string(),
            start_time: "2026-06-10T10:00:00".to_string(),
            end_time: "2026-06-10T11:00:00".to_string(),
            url: "gb28181://dev1@ch1/start".to_string(),
            status: "downloading".to_string(),
            progress: 0.0,
            created_at: Utc::now(),
            zlm_stream_id: zlm_stream_id.to_string(),
            zlm_app: "rtp".to_string(),
            current_bytes: 0,
            total_bytes: 0,
        }
    }

    /// Phase 3.4: 进度 0 → 50% → 100% 字节更新
    #[tokio::test]
    async fn test_download_progress_by_bytes() {
        let dm = DownloadManager::new();
        dm.create(make_session("s1", "download_dev1_ch1_t1")).await;
        dm.update_progress("s1", 0, 1_000_000).await;
        let s = dm.get("s1").await.unwrap();
        assert_eq!(s.progress, 0.0);
        assert_eq!(s.current_bytes, 0);
        assert_eq!(s.total_bytes, 1_000_000);

        dm.update_progress("s1", 500_000, 1_000_000).await;
        let s = dm.get("s1").await.unwrap();
        assert!((s.progress - 50.0).abs() < 0.01);
        assert_eq!(s.status, "downloading");

        dm.update_progress("s1", 1_000_000, 1_000_000).await;
        let s = dm.get("s1").await.unwrap();
        assert!((s.progress - 100.0).abs() < 0.01);
        assert_eq!(s.status, "completed");
    }

    /// Phase 3.4: 进度超 100% 时 clamp 到 100
    #[tokio::test]
    async fn test_download_progress_clamps_to_100() {
        let dm = DownloadManager::new();
        dm.create(make_session("s2", "download_x")).await;
        dm.update_progress("s2", 2_000_000, 1_000_000).await;
        let s = dm.get("s2").await.unwrap();
        assert_eq!(s.progress, 100.0);
    }

    /// Phase 3.4: 按 zlm_stream_id 查找（ZLM hook 用）
    #[tokio::test]
    async fn test_download_get_by_zlm_stream() {
        let dm = DownloadManager::new();
        dm.create(make_session("a", "download_abc")).await;
        dm.create(make_session("b", "download_xyz")).await;

        let s = dm.get_by_zlm_stream("download_abc").await;
        assert!(s.is_some());
        assert_eq!(s.unwrap().stream_id, "a");

        let s = dm.get_by_zlm_stream("not_found").await;
        assert!(s.is_none());
    }

    /// Phase 3.4: remove 清理会话
    #[tokio::test]
    async fn test_download_remove() {
        let dm = DownloadManager::new();
        dm.create(make_session("s3", "download_remove")).await;
        assert!(dm.get("s3").await.is_some());
        dm.remove("s3").await;
        assert!(dm.get("s3").await.is_none());
    }
}
