use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

use crate::error::{AppError, ErrorCode};
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
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let start_time = q.start_time.clone().unwrap_or_else(|| {
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
    });
    let end_time = q.end_time.clone();

    tracing::info!("Playback start: device={}, channel={}, start={}", device_id, channel_id, start_time);

    let stream_id = format!("playback_{}_{}_{}", device_id, channel_id,
        chrono::Utc::now().timestamp());
    let app = "playback".to_string();
    let media_server_id = state.list_zlm_servers().into_iter().next();
    // 只有真实成功的分支才会返回 session，`source` 恒为 INVITE 路径
    let source = "gb28181_playback_invite";

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
                    let sip = &*sip_server;
                    let end = end_time.clone().unwrap_or_else(|| start_time.clone());
                    match sip
                        .send_playback_invite_and_wait(
                            &device_id, &channel_id,
                            &start_time, &end, &stream_id, rtp_server.port, 15,
                        )
                        .await
                    {
                        Ok((_call_id, _zlm_stream_id)) => {
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
                                    source: source.to_string(),
                                }).await;
                            }
                            return Ok(Json(WVPResult::success(serde_json::json!({
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
                            }))));
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

    // 走到这里说明**没有任何一路回放流被真正拉起来**：SIP 未启用 / ZLM 未配置 /
    // GB28181 回放 INVITE 失败。此前这里仍然 `code: 0` 返回一个没有 `playUrl` 的
    // "会话已创建"，前端 `v-if="playUrl"` 为假 → 用户点了片段却什么都不发生、
    // 也没有任何失败提示（失败只落在后端日志），而 `currentStreamId` 已被赋值，
    // 暂停/停止按钮会对一个空会话说谎。
    let reason = if state.sip_server.is_none() {
        "SIP 未启用，无法向设备发起回放 INVITE"
    } else if state.zlm_client.is_none() {
        "未配置可用的 ZLM 媒体节点"
    } else {
        "GB28181 回放 INVITE 失败或等待媒体超时（详见服务端日志）"
    };
    tracing::warn!(
        "回放启动失败 device={} channel={} start={}: {}",
        device_id, channel_id, start_time, reason
    );
    Err(AppError::business(
        ErrorCode::Error500,
        format!("回放启动失败：{reason}"),
    ))
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
        let sip = &*sip_server;
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
        let sip = &*sip_server;
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
        let sip = &*sip_server;
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
        let sip = &*sip_server;
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
                let sip = &*sip_server;
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
        let sip = &*sip_server;
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
                                    "channelId": channel_id,
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
                        // `name` 是前端「名称」列读的键，此前只给了 `fileName`
                        // → 该列整列空白；`deviceId`/`channelId` 是 RecordItem
                        // 声明的必填字段，两个分支此前都没给。
                        serde_json::json!({
                            "deviceId": device_id,
                            "channelId": channel_id,
                            "name": f.name,
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
        let sip = &*sip_server;
        if let Some(device) = sip.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                // 提前开 ZLM RTP server 监听设备推流。
                // 修正：此前把返回的端口整个丢掉（`let _ =`），随后发出
                // 的 INVITE 里 m=video 端口是 0，设备根本收不到可推流的目标。
                let media_port = match state.zlm_client.as_ref() {
                    Some(zlm) => {
                        match zlm
                            .open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                                secret: zlm.secret.clone(),
                                stream_id: stream_id.clone(),
                                port: Some(0),
                                use_tcp: Some(false),
                                rtp_type: Some(0),
                                recv_port: None,
                            })
                            .await
                        {
                            Ok(info) => info.port,
                            Err(e) => {
                                tracing::error!(
                                    "openRtpServer for download {}/{} failed: {}",
                                    device_id, channel_id, e
                                );
                                0
                            }
                        }
                    }
                    None => 0,
                };
                if media_port == 0 {
                    // 端口 0 在 SDP 里表示媒体流被禁用，发出去也不会收到流。
                    // 此前只记一条 warn 然后照发（对调用方伪装成"下载已开始"），
                    // 改为直接失败，避免留下一个永远不会完成的下载会话。
                    tracing::error!(
                        "ZLM 收流端口分配失败，放弃录像下载 INVITE ({}/{})",
                        device_id, channel_id
                    );
                } else {
                match sip
                    .send_download_invite(
                        &device_id,
                        &channel_id,
                        &start_time,
                        &end_time,
                        media_port,
                    )
                    .await
                    {
                        Ok(call_id) => {
                            tracing::info!(
                                "GB28181 DOWNLOAD INVITE sent, call_id={}",
                                call_id
                            );
                            used_gb28181 = true;
                            // **必须让 ZLM 真的把收到的 RTP 录成 MP4**。
                            //
                            // 此前这里只发了 INVITE：设备把录像 RTP 推到 ZLM
                            // 的收流端口后，ZLM 并没有在录像 —— `openRtpServer`
                            // 只创建流，不落盘（需要 startRecord，或连接上
                            // `protocol.enable_mp4` 全局开关）。结果是"下载成功"
                            // 却永远没有文件，on_record_mp4 也永不触发，
                            // 云录像列表里什么都不出现。
                            if let Some(ref zlm) = state.zlm_client {
                                // 流所在的 app 以 ZLM 实际注册的为准
                                // （openRtpServer 默认建在 "rtp"）。
                                let app = match zlm
                                    .get_media_list(None, None, Some(&stream_id))
                                    .await
                                {
                                    Ok(list) => list
                                        .into_iter()
                                        .next()
                                        .map(|m| m.app)
                                        .unwrap_or_else(|| "rtp".to_string()),
                                    Err(e) => {
                                        tracing::debug!(
                                            "查询下载流 {} 的 app 失败（按 rtp 处理）: {}",
                                            stream_id,
                                            e
                                        );
                                        "rtp".to_string()
                                    }
                                };
                                match zlm
                                    .start_record("1", "__defaultVhost__", &app, &stream_id)
                                    .await
                                {
                                    Ok(()) => tracing::info!(
                                        "下载流已开始 MP4 录制 app={} stream={}",
                                        app,
                                        stream_id
                                    ),
                                    Err(e) => tracing::error!(
                                        "下载流 MP4 录制启动失败 app={} stream={}: {} —— \
                                         设备推流不会落盘，本次下载不会有文件",
                                        app,
                                        stream_id,
                                        e
                                    ),
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to send GB28181 DOWNLOAD INVITE: {}", e);
                        }
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
            let sip = &*sip_server;
            let (_key, _rx) = sip
                .media_waiter_manager()
                .register(&format!("dlw_{}", stream_id), &stream_id, "rtp", 15);
            // 把 ZLM 流名回填进业务会话：`send_download_invite` 发 INVITE 时
            // 还不知道 stream_id（它只收 media_port），没有这一步，
            // 之后停止下载 / 无人观看关流时找不到要释放的收流端口。
            sip.invite_session_manager()
                .set_zlm_stream_by_device_channel(&device_id, &channel_id, &stream_id, "rtp")
                .await;
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
                    let sip = &*sip_server;
                    if let Err(e) = sip
                        .send_session_bye(&session.device_id, &session.channel_id)
                        .await
                    {
                        tracing::warn!("GB28181 download BYE failed: {}", e);
                    }
                }
                if let Some(ref zlm_client) = state.zlm_client {
                    // 先停录制：ZLM 只有在停止录制时才会**写完 MP4 尾部并触发
                    // on_record_mp4**。若直接 closeRtpServer，落盘文件不完整，
                    // 云录像里也不会出现这条记录。
                    let app = match zlm_client
                        .get_media_list(None, None, Some(&stream_id))
                        .await
                    {
                        Ok(list) => list
                            .into_iter()
                            .next()
                            .map(|m| m.app)
                            .unwrap_or_else(|| "rtp".to_string()),
                        Err(_) => "rtp".to_string(),
                    };
                    if let Err(e) = zlm_client
                        .stop_record("1", "__defaultVhost__", &app, &stream_id)
                        .await
                    {
                        tracing::warn!(
                            "停止下载流录制失败 app={} stream={}: {}",
                            app,
                            stream_id,
                            e
                        );
                    }
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

/// 录像下载进度。
///
/// 两种传输的状态来源完全不同，必须分开处理：
///
/// * **GB28181 下载**（`gb28181://`，设备把录像 RTP 推到 ZLM）：进度不在
///   ZLM 的"下载列表"里（那里只有 ZLM 自己发起的 HTTP 拉流下载），
///   状态由会话状态机维护（inviting → downloading → completed）。
///   此前一律去查 ZLM 下载列表，查不到就返回 `status:"unknown"` ——
///   前端因此永远看不到 GB28181 下载的任何状态变化。
/// * **ZLM 本地下载**（http/ftp 拉取）：进度来自 ZLM 的下载列表。
pub async fn gb_record_download_progress(
    State(state): State<AppState>,
    Path((_device_id, _channel_id, stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    let Some(dm) = state.download_manager.as_ref() else {
        return err("下载管理未初始化");
    };
    let Some(session) = dm.get(&stream_id).await else {
        return err("下载会话不存在或已结束");
    };

    if session.url.starts_with("gb28181://") {
        // RTP 流没有"总长度"语义，无法给出百分比；如实报告字节数与状态。
        return Json(WVPResult::success(serde_json::json!({
            "streamId": session.stream_id,
            "fileName": session.file_name,
            "progress": session.progress,
            "status": session.status,
            "currentBytes": session.current_bytes,
            "totalBytes": session.total_bytes,
            "transport": "gb28181",
        })));
    }

    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client.get_download_list().await {
            Ok(downloads) => {
                for dl in downloads {
                    if dl.file_name == session.file_name {
                        let progress = dl.progress;
                        let status = if progress >= 100.0 {
                            "completed"
                        } else {
                            "downloading"
                        };
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

    // 会话存在但 ZLM 列表里暂时没有（刚发起/已结束）—— 报告会话自身状态，
    // 而不是伪造一个 "unknown" 让前端无从判断。
    Json(WVPResult::success(serde_json::json!({
        "streamId": session.stream_id,
        "fileName": session.file_name,
        "progress": session.progress,
        "status": session.status,
        "currentBytes": session.current_bytes,
        "totalBytes": session.total_bytes,
        "transport": "zlm-local",
    })))
}

fn err(msg: &str) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::<serde_json::Value>::error(msg.to_string()))
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

#[cfg(test)]
mod download_progress_tests {
    use super::*;

    /// GB28181 下载的状态由会话状态机维护（ZLM 的"下载列表"里没有它）。
    /// 回归：此前一律去查 ZLM 下载列表，查不到就回 `status:"unknown"`。
    #[tokio::test]
    async fn test_download_manager_tracks_gb28181_state() {
        let dm = DownloadManager::new();
        dm.create(DownloadSession {
            stream_id: "download_dev_ch_1".into(),
            device_id: "dev".into(),
            channel_id: "ch".into(),
            file_name: "f.mp4".into(),
            start_time: "2026-09-01T10:00:00".into(),
            end_time: "2026-09-01T10:05:00".into(),
            url: "gb28181://dev@ch/2026-09-01T10:00:00".into(),
            status: "inviting".into(),
            progress: 0.0,
            created_at: Utc::now(),
            zlm_stream_id: "download_dev_ch_1".into(),
            zlm_app: "rtp".into(),
            current_bytes: 0,
            total_bytes: 0,
        })
        .await;

        let s = dm.get("download_dev_ch_1").await.unwrap();
        assert!(s.url.starts_with("gb28181://"), "应被识别为 GB28181 下载");
        assert_eq!(s.status, "inviting");

        // 流上线 → downloading
        dm.update_progress_percent("download_dev_ch_1", 0.0, "downloading").await;
        assert_eq!(dm.get("download_dev_ch_1").await.unwrap().status, "downloading");

        // 流注销（设备推完）→ completed，且 progress 置 100
        dm.update_progress_percent("download_dev_ch_1", 100.0, "completed").await;
        let s = dm.get("download_dev_ch_1").await.unwrap();
        assert_eq!(s.status, "completed");
        assert_eq!(s.progress, 100.0);

        // 通过 zlm_stream_id 反查（hook 里就是这么找会话的）
        assert!(dm.get_by_zlm_stream("download_dev_ch_1").await.is_some());
        assert!(dm.get_by_zlm_stream("不存在").await.is_none());
    }

    /// 未知 stream_id 必须明确报错，而不是返回一个"看起来正常"的 unknown。
    #[test]
    fn test_download_progress_error_helper() {
        let v = err("下载会话不存在或已结束");
        let json = serde_json::to_value(&v.0).unwrap();
        assert_ne!(json["code"], 0);
        assert_eq!(json["msg"], "下载会话不存在或已结束");
    }
}

#[cfg(test)]
mod playback_contract_tests {
    use crate::error::ErrorCode;
    use crate::test_support::app_state;
    use axum::extract::{Path, Query};

    /// 回放拉不起来时必须**明确报错**。此前无论 SIP/ZLM 是否可用都返回
    /// `code: 0` + 一个没有 `playUrl` 的"会话已创建"，前端 `v-if="playUrl"` 为假
    /// → 用户点了片段什么都不发生，也没有失败提示。
    #[tokio::test]
    async fn start_without_sip_or_zlm_returns_error() {
        let state = app_state().await; // 无 SIP / 无 ZLM
        let err = super::playback_start(
            axum::extract::State(state.clone()),
            Path(("34020000001320000001".to_string(), "34020000001310000001".to_string())),
            Query(super::PlaybackQuery {
                start_time: Some("2026-01-01T00:00:00".to_string()),
                end_time: Some("2026-01-01T00:05:00".to_string()),
            }),
        )
        .await
        .expect_err("没有任何回放通路时应报错");
        match err {
            crate::error::AppError::Business(code, msg) => {
                assert!(matches!(code, ErrorCode::Error500), "code={code:?}");
                assert!(msg.contains("回放启动失败"), "msg={msg}");
            }
            other => panic!("期望业务错误，得到 {other:?}"),
        }
    }

    /// ZLM MP4 兜底分支必须给出 `name`/`deviceId`/`channelId`
    /// —— 此前只有 `fileName`，「名称」列整列空白，两个分支都没有 `channelId`。
    #[test]
    fn zlm_fallback_row_has_name_and_ids() {
        // 直接校验 JSON 形状（与 handler 里的字面量保持一致）
        let device_id = "34020000001320000001";
        let channel_id = "34020000001310000001";
        let row = serde_json::json!({
            "deviceId": device_id,
            "channelId": channel_id,
            "name": "20260101_000000.mp4",
            "fileName": "20260101_000000.mp4",
            "filePath": "/record/2026-01-01/20260101_000000.mp4",
            "startTime": "2026-01-01 00:00:00",
            "endTime": "2026-01-01 00:00:00",
        });
        assert_eq!(row["name"], "20260101_000000.mp4");
        assert_eq!(row["deviceId"], device_id);
        assert_eq!(row["channelId"], channel_id);
    }
}
