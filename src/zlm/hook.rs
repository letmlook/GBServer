//! ZLM Webhook 处理

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::cache;

/// Phase 4.3: Protocol enable flags synced to ZLM on `on_server_started`.
pub const PROTOCOL_ENABLE_FLAGS: &[(&str, &str)] = &[
    ("protocol.enable_rtsp", "1"),
    ("protocol.enable_rtmp", "1"),
    ("protocol.enable_hls", "1"),
    ("protocol.enable_http", "1"),
    ("protocol.enable_ws", "1"),
    ("protocol.enable_rtp", "1"),
];

use crate::db::cloud_record::{self, CloudRecordInsert};
use crate::db::{stream_proxy, stream_push};
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
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
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
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    #[serde(default)]
    pub folder: Option<String>,
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
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    #[serde(default)]
    pub folder: Option<String>,
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
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeepaliveData {
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpServerTimeoutData {
    pub app: Option<String>,
    pub port: Option<u16>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

// =====================================================================
// ABL 钩子支持（设计文档 §6.3 阶段 0 缺口 1）
//
// ABL（Another Live media Broadcaster）是 ZLMediaKit 兼容的开源分支，
// 暴露额外的 hook 事件用于细粒度控制。参考 Java 实现在生产部署中
// 会使用这些事件，本实现补齐 on_rtp_playlist / on_record_progress。
// =====================================================================

/// ABL `on_rtp_playlist` 事件：RTP 推流端开始 / 停止推 playlist 时触发
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpPlaylistData {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub ssrc: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    /// "start" / "stop"
    pub action: Option<String>,
}

/// ABL `on_record_progress` 事件：MP4 / HLS 录制进度回调（每 N 秒一次）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordProgressData {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub vhost: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    /// 当前累计录制时长（秒）
    pub current_duration: Option<f64>,
    /// 当前累计文件大小（字节）
    pub current_size: Option<u64>,
    /// 进度时间戳（毫秒）
    pub progress_ts: Option<i64>,
}

/// ABL `on_send_rtp_progress` 事件：SendRtp 推流进度（每 N 包一次）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRtpProgressData {
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub total_sent: Option<u64>,
    pub bytes_sent: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpServerStartedData {
    pub stream_id: String,
    pub port: Option<u16>,
    pub app: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChangedByAppData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

// =====================================================================
// ZlmHookEvent 枚举（Phase 4.1，WVP-Pro 兼容）
//
// 所有 ZLM hook 事件以枚举形式表达，便于 dispatcher 严格匹配及前端按需订阅。
// `from_hook_name` 将 ZLM 字符串 hook 名（如 "on_stream_changed"）解析为枚举；
// `default_response` 返回 WVP-Pro 兼容的成功响应结构。
// =====================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlmHookEvent {
    ServerStarted,
    ServerKeepalive,
    StreamChanged,
    StreamNotFound,
    StreamNoneReader,
    StreamStarted,
    Publish,
    Play,
    RtpServerStarted,
    RtpServerTimeout,
    SendRtpStopped,
    RecordMp4,
    RecordProgress,
    FlowReport,
    Unknown,
}

impl ZlmHookEvent {
    pub fn from_hook_name(name: &str) -> Self {
        match name {
            "on_server_started" => Self::ServerStarted,
            "on_server_keepalive" => Self::ServerKeepalive,
            "on_stream_changed" => Self::StreamChanged,
            "on_stream_not_found" => Self::StreamNotFound,
            "on_stream_none_reader" => Self::StreamNoneReader,
            "on_stream_started" => Self::StreamStarted,
            "on_publish" => Self::Publish,
            "on_play" => Self::Play,
            "on_rtp_server_started" => Self::RtpServerStarted,
            "on_rtp_server_timeout" => Self::RtpServerTimeout,
            "on_send_rtp_stopped" => Self::SendRtpStopped,
            "on_record_mp4" | "on_record_file" => Self::RecordMp4,
            "on_record_progress" => Self::RecordProgress,
            "on_flow_report" => Self::FlowReport,
            _ => Self::Unknown,
        }
    }

    /// WVP-Pro 兼容的默认成功响应：前端只需 `code === 0` 即视为成功。
    pub fn default_response(&self) -> serde_json::Value {
        serde_json::json!({"code": 0, "msg": "success"})
    }
}

fn parse_stream_id(stream: &str) -> Option<(String, String)> {
    if let Some(pos) = stream.find('$') {
        let device_id = stream[..pos].to_string();
        let channel_id = stream[pos + 1..].to_string();
        if device_id.len() == 20 || device_id.len() == 22 {
            return Some((device_id, channel_id));
        }
    }
    if let Some(_pos) = stream.find('/') {
        let parts: Vec<&str> = stream.split('/').collect();
        if parts.len() >= 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}

fn parse_record_time_ms(value: &str) -> i64 {
    if let Ok(ts) = value.parse::<i64>() {
        if ts > 1_000_000_000_000 {
            return ts;
        }
        if ts > 1_000_000_000 {
            return ts * 1000;
        }
    }

    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .map(|dt| dt.and_utc().timestamp_millis())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis())
}

async fn save_record(
    state: &AppState,
    media_server_id: Option<String>,
    app: String,
    stream: String,
    file_name: String,
    folder: Option<String>,
    file_path: String,
    file_size: u64,
    file_duration: f64,
    file_create_time: String,
) {
    let start_time = parse_record_time_ms(&file_create_time);
    let duration_ms = (file_duration.max(0.0) * 1000.0).round() as i64;
    let end_time = start_time + duration_ms;
    let server_id = state
        .config
        .user_settings
        .as_ref()
        .and_then(|settings| settings.server_id.clone());

    let record = CloudRecordInsert {
        app,
        stream,
        call_id: Some(file_name.clone()),
        start_time: Some(start_time),
        end_time: Some(end_time),
        media_server_id,
        server_id,
        file_name: Some(file_name),
        folder,
        file_path: Some(file_path),
        file_size: Some(file_size.min(i64::MAX as u64) as i64),
        time_len: Some(file_duration),
    };

    if let Err(e) = cloud_record::insert(&state.pool, &record).await {
        tracing::warn!("Failed to save cloud record: {}", e);
    }
}

async fn sync_stream_changed(state: &AppState, data: &StreamChangedData) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let media_server_id = data.media_server_id.as_deref();

    match stream_push::update_pushing_status_by_app_stream(
        &state.pool,
        &data.app,
        &data.stream,
        media_server_id,
        data.register,
        &now,
    )
    .await
    {
        Ok(affected) if affected > 0 => {
            tracing::debug!(
                "Stream push status synced: {}/{} pushing={}",
                data.app,
                data.stream,
                data.register
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to sync stream push status: {}", e),
    }

    match stream_proxy::update_pulling_status_by_app_stream(
        &state.pool,
        &data.app,
        &data.stream,
        media_server_id,
        data.register,
        &now,
    )
    .await
    {
        Ok(affected) if affected > 0 => {
            tracing::debug!(
                "Stream proxy status synced: {}/{} pulling={}",
                data.app,
                data.stream,
                data.register
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to sync stream proxy status: {}", e),
    }

    // Phase 7.1: media_server stream_count now goes through StateStore so it's
    // available to select_least_loaded_server_filtered even on single-node deploys.
    if let Some(media_server_id) = media_server_id {
        if data.register {
            state.state_store.set_media_server(
                media_server_id,
                crate::state_store::MediaServerLoad {
                    server_id: media_server_id.to_string(),
                    stream_count: state.state_store.get_media_server(media_server_id)
                        .map(|s| s.stream_count + 1).unwrap_or(1),
                    rtp_server_count: state.state_store.get_media_server(media_server_id)
                        .map(|s| s.rtp_server_count).unwrap_or(0),
                    online: true,
                    last_keepalive: chrono::Utc::now(),
                },
            );
        } else if let Some(mut s) = state.state_store.get_media_server(media_server_id) {
            s.stream_count = (s.stream_count - 1).max(0);
            s.last_keepalive = chrono::Utc::now();
            state.state_store.set_media_server(media_server_id, s);
        }
    }
    // Legacy Redis cache fallback for legacy deployments (will be removed in Phase 7.6).
    if let (Some(redis), Some(media_server_id)) = (&state.redis, media_server_id) {
        if data.register {
            cache::incr_media_server_streams(redis, media_server_id).await;
        } else {
            cache::decr_media_server_streams(redis, media_server_id).await;
        }
    }

    // Update global active stream metrics
    if let Some(ref zlm) = state.zlm_client {
        if let Ok(streams) = zlm.get_media_list(None, None, None).await {
            crate::metrics::set_active_streams(streams.len());
        }
    }

    // Phase 3.1: 当 RTP 流注册时，通知 media_waiter_manager
    // 让 play_start 等媒体到达的 handler 收到 MediaReady 后返回。
    if data.register && data.app == "rtp" {
        if let Some(ref sip_server) = state.sip_server {
            let sip = sip_server.read().await;
            let resolved = sip
                .media_waiter_manager()
                .resolve_by_stream(&data.stream, &data.app);
            tracing::debug!(
                "on_stream_changed rtp/{} register=true media_waiter resolved={}",
                data.stream, resolved
            );
        }

        // Phase 6.3: 如果 stream 命名以 "jt1078_" 开头，路由到 JtMediaSessionManager
        if data.stream.starts_with("jt1078_") {
            // Format: jt1078_{phone}_{channel_id}
            let rest = data.stream.trim_start_matches("jt1078_");
            let parts: Vec<&str> = rest.splitn(2, '_').collect();
            if parts.len() == 2 {
                let phone = parts[0].to_string();
                let channel_id: u8 = parts[1].parse().unwrap_or(0);
                let mgr_guard = state.jt1078_manager.read().await;
                if let Some(m) = mgr_guard.as_ref() {
                    let resolved = m
                        .media_session_manager()
                        .resolve_waiter(&phone, channel_id, &data.stream);
                    tracing::info!(
                        "6.3 on_stream_changed routed to JtMediaSessionManager: phone={} ch={} resolved={}",
                        phone, channel_id, resolved
                    );
                }
            }
        }
    }

    // Phase 3.4: 如果该 stream 属于下载会话，触发下载进度更新。
    // 通过 stream_id 包含 "download_" 前缀识别（与 3.4 中 stream_id 命名一致）。
    if data.register && data.stream.starts_with("download_") {
        if let Some(ref dm) = state.download_manager {
            if let Some(session) = dm.get_by_zlm_stream(&data.stream).await {
                tracing::info!(
                    "Download stream ready: session={} stream={}",
                    session.stream_id, data.stream
                );
                // 状态从 inviting → downloading；进度仍待 ZLM MP4 落盘回调
                dm.update_progress_percent(&session.stream_id, 0.0, "downloading").await;
            }
        }
    }
}

fn should_skip_discovered_push(app: &str) -> bool {
    matches!(
        app.to_ascii_lowercase().as_str(),
        "rtp" | "gb_playback" | "playback" | "download"
    )
}

async fn register_published_stream(state: &AppState, data: &PublishData) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let media_server_id = data.media_server_id.as_deref();

    match stream_proxy::get_by_app_stream(&state.pool, &data.app, &data.stream).await {
        Ok(Some(proxy)) => {
            if let Err(e) = stream_proxy::update_pulling_status_by_app_stream(
                &state.pool,
                &data.app,
                &data.stream,
                media_server_id,
                true,
                &now,
            )
            .await
            {
                tracing::warn!("Failed to mark proxy as pulling: {}", e);
            } else {
                tracing::debug!(
                    "Published stream matched proxy id={} {}/{}",
                    proxy.id,
                    data.app,
                    data.stream
                );
            }
            return;
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("Failed to query stream proxy for publish event: {}", e),
    }

    if should_skip_discovered_push(&data.app) {
        return;
    }

    let server_id = state
        .config
        .user_settings
        .as_ref()
        .and_then(|settings| settings.server_id.as_deref());

    if let Err(e) = stream_push::upsert_discovered(
        &state.pool,
        &data.app,
        &data.stream,
        media_server_id,
        server_id,
        &now,
    )
    .await
    {
        tracing::warn!("Failed to register discovered push stream: {}", e);
    }
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
                sync_stream_changed(&state, &data).await;

                let ws_msg = serde_json::json!({
                    "type": "streamChanged",
                    "app": data.app,
                    "stream": data.stream,
                    "schema": data.schema,
                    "register": data.register,
                });
                state.ws_state.broadcast("streamChanged", ws_msg).await;
            }
        }
        "on_stream_not_found" => {
            if let Some(data) = serde_json::from_value::<StreamNotFoundData>(event.clone()).ok() {
                tracing::warn!("Stream not found: {}/{}/{}",
                    data.schema, data.app, data.stream);

                // Register with reconnect manager for persistent retry
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    let reconnect_mgr = sip.stream_reconnect_manager();
                    // Feed the reconnect manager so it retries on schedule
                    reconnect_mgr.on_stream_not_found(&data.app, &data.stream);

                    // Also attempt immediate one-shot reconnect
                    if let Some((device_id, channel_id)) =
                        crate::sip::gb28181::stream_reconnect::StreamReconnectManager::parse_stream_id(&data.stream)
                    {
                        let device_online = sip.is_device_online(&device_id).await;
                        if device_online {
                            match sip.send_play_invite_and_wait(&device_id, &channel_id, 0, None).await {
                                Ok(_) => {
                                    tracing::info!("Stream reconnect INVITE sent for {}/{}", device_id, channel_id);
                                    reconnect_mgr.mark_success(&data.stream);
                                    return Json(WVPResult::success(serde_json::json!({
                                        "code": 0,
                                        "action": "reconnect",
                                        "deviceId": device_id,
                                        "channelId": channel_id
                                    })));
                                }
                                Err(e) => {
                                    tracing::warn!("Stream reconnect INVITE failed: {}", e);
                                }
                            }
                        } else {
                            tracing::debug!("Device {} offline, skip reconnect", device_id);
                        }
                    }
                }

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
                save_record(
                    &state,
                    data.media_server_id,
                    data.app,
                    data.stream,
                    data.file_name,
                    data.folder,
                    data.file_path,
                    data.file_size,
                    data.file_duration,
                    data.file_create_time,
                ).await;
            }
        }
        "on_record_hls" => {
            if let Some(data) = serde_json::from_value::<RecordHlsData>(event.clone()).ok() {
                tracing::info!("HLS recorded: {} ({} bytes)",
                    data.file_name, data.file_size);
                save_record(
                    &state,
                    data.media_server_id,
                    data.app,
                    data.stream,
                    data.file_name,
                    data.folder,
                    data.file_path,
                    data.file_size,
                    data.file_duration,
                    data.file_create_time,
                ).await;
            }
        }
        "on_play" => {
            // Phase 4.1: 播放鉴权 - 检查是否有设备/通道授权可播放
            if let Some(data) = serde_json::from_value::<PlayData>(event.clone()).ok() {
                // Phase 4.2: secret 鉴权 + IP 白名单
                if let Some(resp) = check_hook_auth(&state, &event, &data.ip).await {
                    return resp;
                }
                tracing::info!("on_play: {}/{}/{} from {}",
                    data.schema, data.app, data.stream, data.ip);
                // 从 stream_id 解析设备/通道（格式：device_id_channel_id 或 device_id$channel_id）
                if let Some((device_id, channel_id)) = parse_stream_id(&data.stream) {
                    // 播放鉴权（预留）
                    tracing::debug!("on_play: device={} channel={}", device_id, channel_id);
                }
            }
        }
        "on_publish" => {
            // Phase 4.1: 推流鉴权 - 验证设备来源
            if let Some(data) = serde_json::from_value::<PublishData>(event.clone()).ok() {
                // Phase 4.2: secret 鉴权 + IP 白名单
                if let Some(resp) = check_hook_auth(&state, &event, &data.ip).await {
                    return resp;
                }
                tracing::info!("on_publish: {}/{}/{} from {}",
                    data.schema, data.app, data.stream, data.ip);
                // 验证推流来源 IP 是否与注册设备匹配
                if let Some((device_id, channel_id)) = parse_stream_id(&data.stream) {
                    if let Some(ref sip_server) = state.sip_server {
                        let sip = sip_server.read().await;
                        if let Some(addr) = sip.device_manager().get_address(&device_id).await {
                            // 如果设备注册了 IP 且不匹配，记录但不拒绝
                            if let Ok(parsed_ip) = data.ip.parse::<std::net::IpAddr>() {
                                if addr.ip() != parsed_ip {
                                    tracing::warn!("on_publish IP mismatch: device={} registered={} actual={}",
                                        device_id, addr.ip(), data.ip);
                                }
                            }
                        }
                    }
                }
                register_published_stream(&state, &data).await;
            }
        }
        "on_server_started" => {
            if let Some(data) = serde_json::from_value::<ServerStartedData>(event.clone()).ok() {
                tracing::info!(
                    "ZLM server started: rtsp={} rtmp={} http={} https={}",
                    data.rtsp_port, data.rtmp_port, data.http_port, data.https_port
                );
                // Reset media server status and reconfigure hooks
                let media_server_id = event.get("mediaServerId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("zlmediakit-1");
                
                // Phase 4.2: 重置节点状态（on_server_started 时）
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let _ = crate::db::media_server::update_ports(
                    &state.pool,
                    media_server_id,
                    data.http_port as i32,
                    Some(data.http_port as i32),
                    Some(data.rtsp_port as i32),
                    Some(data.rtmp_port as i32),
                    &now,
                ).await;
                // 重置流计数（在 on_server_started 时清零，避免旧数据残留）
                if let Some(ref redis) = state.redis {
                    let _ = crate::cache::set_media_server_streams(redis, media_server_id, 0).await;
                }
                tracing::info!("ZLM node {} online: http={} rtsp={} rtmp={}",
                    media_server_id, data.http_port, data.rtsp_port, data.rtmp_port);

                // Reconfigure ZLM hook URL if zlm_client is available
                if let Some(ref zlm_client) = state.zlm_client {
                    let hook_url = state.config.zlm.as_ref()
                        .and_then(|cfg| cfg.servers.iter().find(|s| s.id == media_server_id))
                        .and_then(|sv| sv.hook_url.clone())
                        .unwrap_or_else(|| {
                            let server_port = state.config.server.port;
                            format!("http://127.0.0.1:{}/api/zlm/hook", server_port)
                        });

                    let secret = zlm_client.secret.clone();
                    let config_items = vec![
                        ("hook.enable", "1".to_string()),
                        ("hook.on_server_started", hook_url.clone()),
                        ("hook.on_stream_changed", hook_url.clone()),
                        ("hook.on_stream_not_found", hook_url.clone()),
                        ("hook.on_record_mp4", hook_url.clone()),
                        ("hook.on_publish", hook_url.clone()),
                        ("hook.on_play", hook_url.clone()),
                        ("hook.on_rtp_server_started", hook_url.clone()),
                        ("hook.on_stream_started", hook_url.clone()),
                        ("hook.on_rtp_server_timeout", hook_url.clone()),
                        // ABL 钩子（设计文档 §6.3 阶段 0 缺口 1）
                        ("hook.on_rtp_playlist", hook_url.clone()),
                        ("hook.on_record_progress", hook_url.clone()),
                        ("hook.on_send_rtp_progress", hook_url.clone()),
                    ];
                    for (key, value) in config_items {
                        if let Err(e) = zlm_client.set_server_config(&secret, key, &value).await {
                            tracing::warn!("Failed to set ZLM config {}={}: {}", key, value, e);
                        }
                    }
                    tracing::info!("ZLM hook URLs reconfigured for server {}", media_server_id);

                    // Phase 4.3: 自动同步 RTP 端口范围 + 协议开关到 ZLM
                    // 与 gb_media_server.rtp_port_range / send_rtp_port_range 对齐
                    match crate::db::media_server::get_media_server_by_id(
                        &state.pool, media_server_id,
                    ).await {
                        Ok(Some(server_config)) => {
                            // rtp.port_range（设备推送端口）
                            if let Some(ref rtp_range) = server_config.rtp_port_range {
                                match crate::zlm::client::set_rtp_port_range(
                                    zlm_client, &secret, "rtp.port_range", rtp_range,
                                ).await {
                                    Ok(()) => tracing::info!(
                                        "ZLM rtp.port_range set to {} for server {}",
                                        rtp_range, media_server_id,
                                    ),
                                    Err(e) => tracing::warn!(
                                        "Failed to set ZLM rtp.port_range={}: {}",
                                        rtp_range, e,
                                    ),
                                }
                            }
                            // send_rtp.port_range（推送上级平台端口）
                            if let Some(ref srtp_range) = server_config.send_rtp_port_range {
                                match crate::zlm::client::set_rtp_port_range(
                                    zlm_client, &secret, "send_rtp.port_range", srtp_range,
                                ).await {
                                    Ok(()) => tracing::info!(
                                        "ZLM send_rtp.port_range set to {} for server {}",
                                        srtp_range, media_server_id,
                                    ),
                                    Err(e) => tracing::warn!(
                                        "Failed to set ZLM send_rtp.port_range={}: {}",
                                        srtp_range, e,
                                    ),
                                }
                            }
                            // 协议开关（与 ZLM 默认对齐：全部启用）
                            for (key, value) in PROTOCOL_ENABLE_FLAGS {
                                if let Err(e) = zlm_client.set_server_config(
                                    &secret, key, value,
                                ).await {
                                    tracing::warn!(
                                        "Failed to set ZLM {}={}: {}",
                                        key, value, e,
                                    );
                                }
                            }
                            tracing::info!(
                                "ZLM node {} fully auto-configured (hooks + rtp ranges + protocols)",
                                media_server_id,
                            );
                        }
                        Ok(None) => {
                            tracing::warn!(
                                "Media server {} not found in DB; skipping RTP port range sync",
                                media_server_id,
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to query media_server {} for RTP config: {}",
                                media_server_id, e,
                            );
                        }
                    }
                }

                // Reset stream counts in Redis
                if let Some(ref redis) = state.redis {
                    cache::set_media_server_streams(redis, media_server_id, 0).await;
                }
            }
        }
        "on_server_keepalive" => {
            if let Some(data) = serde_json::from_value::<ServerKeepaliveData>(event.clone()).ok() {
                let server_id = data.media_server_id.as_deref().unwrap_or("zlmediakit-1");
                tracing::debug!("ZLM server keepalive: {}", server_id);
                // Update last keepalive time in DB
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let _ = crate::db::media_server::update_last_keepalive(
                    &state.pool, server_id, &now,
                ).await;
            }
        }
        "on_rtp_server_started" => {
            // Phase 3.1: ZLM 成功开启 RTP Server，等待设备推流到达
            if let Some(data) = serde_json::from_value::<RtpServerStartedData>(event.clone()).ok() {
                // 流 ID 就是 RTP server 对应的 stream_id
                let stream_id = &data.stream_id;
                tracing::info!("ZLM RTP server started: stream_id={}", stream_id);
                // 通过 stream_id 反查 call_id，触发 MediaWaiter
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    let resolved = sip.notify_media_ready_by_stream(stream_id, "rtp").await;
                    if resolved {
                        tracing::info!("MediaWaiter resolved for stream_id={}", stream_id);
                    }
                }
            }
        }
        "on_stream_started" => {
            // Phase 3.1: 设备推流到达（流正式开始）
            if let Some(data) = serde_json::from_value::<StreamChangedData>(event.clone()).ok() {
                tracing::info!("Stream started: app={} stream={}", data.app, data.stream);
                // 通知 media waiter（通过 call_id 或 stream_id）
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    let resolved = sip.notify_media_ready_by_stream(&data.stream, &data.app).await;
                    if resolved {
                        tracing::info!("MediaWaiter resolved for stream={}/{}", data.app, data.stream);
                    }
                }
                // 广播 WebSocket 事件
                state.ws_state.broadcast("streamStarted", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "schema": data.schema,
                })).await;
            }
        }

        "on_rtp_server_timeout" => {
            if let Some(data) = serde_json::from_value::<RtpServerStartedData>(event.clone()).ok() {
                tracing::info!(
                    "RTP server timeout: stream_id={} server={}",
                    data.stream_id,
                    data.media_server_id.as_deref().unwrap_or("unknown")
                );
                // Clean up InviteSession associated with this stream
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    // Parse device_id/channel_id from stream_id
                    if let Some((device_id, channel_id)) = parse_stream_id(&data.stream_id) {
                        if let Ok(_) = sip.send_session_bye(&device_id, &channel_id).await {
                            tracing::info!("Sent BYE for timed-out RTP session {}/{}", device_id, channel_id);
                        }
                    }
                }
            }
        }
        "on_flow_report" => {
            // ZLM sends periodic flow stats; log summary and update Redis stream counts
            let total_traffic = event.get("totalBytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let streams = event.get("streams")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let media_server_id = event.get("mediaServerId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            tracing::debug!(
                "Flow report: server={} streams={} totalBytes={}",
                media_server_id, streams, total_traffic
            );
            // Update media server flow stats in DB
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let _ = crate::db::media_server::update_flow_stats(
                &state.pool, media_server_id, total_traffic as i64, streams as i32, &now,
            ).await;
            // Sync active stream count to Redis
            if let Some(ref redis) = state.redis {
                cache::set_media_server_streams(redis, media_server_id, streams as i64).await;
            }
        }
        "on_stream_none_reader" => {
            if let Some(data) = serde_json::from_value::<StreamChangedData>(event.clone()).ok() {
                tracing::info!(
                    "Stream no readers: {}/{}/{}",
                    data.schema, data.app, data.stream
                );
                // Auto-stop idle streams after a grace period to free ZLM resources
                // Stream push/proxy status is updated via on_stream_changed
                state.ws_state.broadcast("streamNoneReader", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "schema": data.schema,
                })).await;
            }
        }
        "on_send_rtp_stopped" => {
            // Phase 4.1: SendRtp 停止通知（级联平台关闭推流）
            // Phase 5.4: 按 stream 路由到 SendRtpManager 关闭对应 session
            if let Some(data) = serde_json::from_value::<StreamChangedData>(event.clone()).ok() {
                tracing::info!("SendRTP stopped: {}/{}", data.app, data.stream);
                // 5.4: 关闭 SendRtpManager 中匹配的 cascade session
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    if let Some(session) = sip.send_rtp_manager().close_by_stream(&data.stream) {
                        tracing::info!(
                            "5.4 on_send_rtp_stopped → closed cascade session platform={} channel={} stream={}",
                            session.platform_id, session.channel_id, data.stream
                        );
                    }
                }
                // 广播级联停止事件
                state.ws_state.broadcast("sendRtpStopped", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "schema": data.schema,
                })).await;
            }
        }
        "on_record_file" => {
            // Phase 4.1: MP4 录像文件落盘通知
            if let Some(data) = serde_json::from_value::<RecordMp4Data>(event.clone()).ok() {
                tracing::info!("MP4 recording complete: file={} duration={}s",
                    data.file_path.as_str(),
                    data.file_duration);
                // 同步录像文件信息到 DB（cloud_record）
                let duration = data.file_duration as i64;
                let _ = crate::db::cloud_record::insert_from_hook(
                    &state.pool, &data.stream, &data.file_path, duration
                ).await;
            }
        }
        // ============ ABL 钩子（设计文档 §6.3 阶段 0 缺口 1）============
        "on_rtp_playlist" => {
            if let Some(data) = serde_json::from_value::<RtpPlaylistData>(event.clone()).ok() {
                tracing::info!(
                    "ABL rtp_playlist: app={:?} stream={:?} ssrc={:?} action={:?}",
                    data.app, data.stream, data.ssrc, data.action
                );
                // 通知 WS 订阅者（用于前端展示 playlist 状态）
                state.ws_state.broadcast("abl_rtp_playlist", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "ssrc": data.ssrc,
                    "action": data.action,
                })).await;
            }
        }
        "on_record_progress" => {
            if let Some(data) = serde_json::from_value::<RecordProgressData>(event.clone()).ok() {
                tracing::debug!(
                    "ABL record_progress: {}/{} duration={:?}s size={:?}B ts={:?}",
                    data.app.as_deref().unwrap_or(""),
                    data.stream.as_deref().unwrap_or(""),
                    data.current_duration, data.current_size, data.progress_ts
                );
                // 更新 DB 中的录像进度（cloud_record），供前端轮询
                if let (Some(app), Some(stream)) = (data.app.as_deref(), data.stream.as_deref()) {
                    let _ = crate::db::cloud_record::update_recording_progress(
                        &state.pool,
                        stream,
                        app,
                        data.current_duration.unwrap_or(0.0),
                        data.current_size.unwrap_or(0),
                    ).await;
                }
            }
        }
        "on_send_rtp_progress" => {
            if let Some(data) = serde_json::from_value::<SendRtpProgressData>(event.clone()).ok() {
                tracing::debug!(
                    "ABL send_rtp_progress: {}/{} total={:?} bytes={:?}",
                    data.app.as_deref().unwrap_or(""),
                    data.stream.as_deref().unwrap_or(""),
                    data.total_sent, data.bytes_sent
                );
                // 级联推流进度（如不需要可关闭日志），用于监控推流质量
                state.ws_state.broadcast("abl_send_rtp_progress", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "total_sent": data.total_sent,
                    "bytes_sent": data.bytes_sent,
                })).await;
            }
        }
        _ => {
            tracing::debug!("Unhandled webhook: {}", hook_name);
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "code": 0
    })))
}

/// Phase 4.2: hook 鉴权（secret + IP 白名单）
///
/// 返回 `Some(Json)` 表示鉴权失败，调用方应直接 return。
/// 返回 `None` 表示通过（放行或白名单为空）。
async fn check_hook_auth(
    state: &AppState,
    event: &serde_json::Value,
    client_ip_str: &str,
) -> Option<Json<WVPResult<serde_json::Value>>> {
    use crate::zlm::auth::{AuthResult, HookAuthChecker};

    let provided_secret = event
        .get("secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let media_server_id = event
        .get("mediaServerId")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 节点 expected_secret：优先用 in-memory ZLM client，回退到 DB
    let expected_secret: String = if !media_server_id.is_empty() {
        if let Some(client) = state.get_zlm_client(Some(media_server_id)) {
            client.secret.clone()
        } else {
            // 兜底：从 DB 查 secret
            crate::db::media_server::get_media_server_by_id(&state.pool, media_server_id)
                .await
                .ok()
                .flatten()
                .and_then(|s| s.secret)
                .unwrap_or_default()
        }
    } else {
        // 没有 mediaServerId，尝试用默认节点
        state
            .zlm_client
            .as_ref()
            .map(|c| c.secret.clone())
            .unwrap_or_default()
    };

    // 加载白名单 CIDR（按 media_server_id）
    let cidrs_str: Vec<String> = if !media_server_id.is_empty() {
        crate::db::media_server::get_white_list_cidrs(&state.pool, media_server_id)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // 解析 CIDR 字符串为 IpNetwork，解析失败记 warn 并忽略
    let cidrs: Vec<ipnetwork::IpNetwork> = cidrs_str
        .iter()
        .filter_map(|s| match s.parse::<ipnetwork::IpNetwork>() {
            Ok(net) => Some(net),
            Err(e) => {
                tracing::warn!("Invalid CIDR in whitelist '{}': {}", s, e);
                None
            }
        })
        .collect();

    // 客户端 IP 解析失败时拒绝（无法验证白名单）
    let client_ip = match client_ip_str.parse::<std::net::IpAddr>() {
        Ok(ip) => ip,
        Err(_) => {
            tracing::warn!("hook auth: unparseable client IP '{}'", client_ip_str);
            return Some(Json(WVPResult::error("Unauthorized: invalid client IP")));
        }
    };

    let checker = HookAuthChecker::new(&expected_secret).with_whitelist(cidrs);

    match checker.check(provided_secret, &client_ip) {
        AuthResult::Ok => None,
        AuthResult::UnauthorizedSecret => {
            tracing::warn!(
                "hook auth: secret mismatch from {} (server={})",
                client_ip, media_server_id
            );
            Some(Json(WVPResult::error("Unauthorized: secret mismatch")))
        }
        AuthResult::IpNotWhitelisted => {
            tracing::warn!(
                "hook auth: IP {} not in whitelist (server={})",
                client_ip, media_server_id
            );
            Some(Json(WVPResult::error("Unauthorized: IP not in whitelist")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stream_id_dollar() {
        let s = "34020000002000000001$101";
        let res = parse_stream_id(s);
        assert!(res.is_some());
        let (d, c) = res.unwrap();
        assert_eq!(d, "34020000002000000001");
        assert_eq!(c, "101");
    }

    #[test]
    fn test_parse_stream_id_slash() {
        let s = "device/channel";
        let res = parse_stream_id(s);
        assert!(res.is_some());
        let (d, c) = res.unwrap();
        assert_eq!(d, "device");
        assert_eq!(c, "channel");
    }

    #[test]
    fn test_parse_record_time_ms_unix() {
        let v = "1620000000"; // seconds
        let ms = parse_record_time_ms(v);
        assert!(ms >= 1620000000 * 1000);
    }

    // ============== ZlmHookEvent 解析测试（Phase 4.1） ==============

    #[test]
    fn test_zlm_hook_event_parse_stream_changed() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_changed"),
            ZlmHookEvent::StreamChanged
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_publish_play() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_publish"),
            ZlmHookEvent::Publish
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_play"),
            ZlmHookEvent::Play
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_server_events() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_server_started"),
            ZlmHookEvent::ServerStarted
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_server_keepalive"),
            ZlmHookEvent::ServerKeepalive
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_stream_lifecycle() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_not_found"),
            ZlmHookEvent::StreamNotFound
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_none_reader"),
            ZlmHookEvent::StreamNoneReader
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_started"),
            ZlmHookEvent::StreamStarted
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_rtp_events() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_rtp_server_started"),
            ZlmHookEvent::RtpServerStarted
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_rtp_server_timeout"),
            ZlmHookEvent::RtpServerTimeout
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_send_rtp_stopped"),
            ZlmHookEvent::SendRtpStopped
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_record_aliases() {
        // 兼容两个常见 hook 名
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_record_mp4"),
            ZlmHookEvent::RecordMp4
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_record_file"),
            ZlmHookEvent::RecordMp4
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_record_progress"),
            ZlmHookEvent::RecordProgress
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_flow_report"),
            ZlmHookEvent::FlowReport
        );
    }

    #[test]
    fn test_zlm_hook_event_unknown_and_default_response() {
        // 未知 hook 名称 → Unknown
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_something_made_up"),
            ZlmHookEvent::Unknown
        );
        // default_response 始终是 WVP-Pro 兼容的成功结构
        let resp = ZlmHookEvent::StreamChanged.default_response();
        assert_eq!(resp["code"], 0);
        assert_eq!(resp["msg"], "success");
        // 同样适用于 Unknown（保持前端可正常处理）
        let resp_unknown = ZlmHookEvent::Unknown.default_response();
        assert_eq!(resp_unknown["code"], 0);
    }

    // ============== Phase 4.3: set_server_config wiremock 集成测试 ==============
    //
    // 验证 `on_server_started` 自动配置循环中，所有 `set_server_config` 调用
    // （hook.enable / hook.on_* / rtp.port_range / send_rtp.port_range /
    //  protocol.enable_*）实际以正确的 payload 命中 ZLM HTTP API。
    //
    // 由于完整 on_server_started handler 需要 AppState（DB pool / Redis 等），
    // 这里只测底层 `ZlmClient::set_server_config` 的端到端 HTTP 行为，hook.rs
    // 调用端与 client.rs 通过同一方法对接。

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_on_server_started_auto_configures_rtp_port_range() {
        // 1. 启动 wiremock 模拟 ZLM HTTP 服务
        let mock_server = MockServer::start().await;

        // 2. 注册 setServerConfig 端点：返回 code=0
        Mock::given(method("POST"))
            .and(path("/index/api/setServerConfig"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": 0
            })))
            .expect(1..) // 至少 1 次（实际会更多：hooks + rtp + protocols）
            .mount(&mock_server)
            .await;

        // 3. 构造 ZlmClient，指向 mock server
        let uri = mock_server.uri();
        // uri 形如 "http://127.0.0.1:PORT"
        let stripped = uri.trim_start_matches("http://");
        let mut parts = stripped.splitn(2, ':');
        let ip = parts.next().unwrap_or("127.0.0.1").to_string();
        let port: u16 = parts
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(80);

        let zlm_client = crate::zlm::ZlmClient::new(&ip, port, "test-secret");

        // 4. 模拟 on_server_started 中的 3 类 set_server_config 调用
        let secret = "test-secret";

        // (a) hook URL（已有的 Phase 4.1 行为）
        zlm_client
            .set_server_config(secret, "hook.enable", "1")
            .await
            .expect("set_server_config hook.enable");

        // (b) rtp.port_range —— set_rtp_port_range helper handles comma→dash conversion
        crate::zlm::client::set_rtp_port_range(
            &zlm_client, secret, "rtp.port_range", "30000,30200",
        )
        .await
        .expect("set_rtp_port_range rtp.port_range");

        // (c) send_rtp.port_range
        let srtp_value = format!("{}-{}", 40000, 40200);
        zlm_client
            .set_server_config(secret, "send_rtp.port_range", &srtp_value)
            .await
            .expect("set_server_config send_rtp.port_range");

        // (d) 协议开关
        for (key, value) in PROTOCOL_ENABLE_FLAGS {
            zlm_client
                .set_server_config(secret, key, value)
                .await
                .expect("set_server_config protocol flag");
        }

        // 5. 验证 mock server 收到了所有调用（>= 9 次：1 hook + 2 rtp + 6 protocols）
        let received = mock_server.received_requests().await.unwrap_or_default();
        assert!(
            received.len() >= 9,
            "expected at least 9 setServerConfig calls, got {}",
            received.len()
        );

        // 6. 验证每个关键 key 都被正确设置（key 字段在 wiremock 这里我们从 body 解析）
        let mut found_rtp_port_range = false;
        let mut found_send_rtp_port_range = false;
        let mut protocol_flags = std::collections::HashSet::new();
        for req in &received {
            let body = String::from_utf8_lossy(&req.body).to_string();
            if body.contains("\"key\":\"rtp.port_range\"") && body.contains("30000-30200") {
                found_rtp_port_range = true;
            }
            if body.contains("\"key\":\"send_rtp.port_range\"") && body.contains("40000-40200") {
                found_send_rtp_port_range = true;
            }
            for flag in [
                "protocol.enable_rtsp",
                "protocol.enable_rtmp",
                "protocol.enable_hls",
                "protocol.enable_http",
                "protocol.enable_ws",
                "protocol.enable_rtp",
            ] {
                if body.contains(&format!("\"key\":\"{}\"", flag)) {
                    protocol_flags.insert(flag.to_string());
                }
            }
        }
        assert!(found_rtp_port_range, "rtp.port_range=30000-30200 not seen in requests");
        assert!(
            found_send_rtp_port_range,
            "send_rtp.port_range=40000-40200 not seen in requests"
        );
        assert_eq!(
            protocol_flags.len(),
            6,
            "expected 6 protocol.enable_* flags, saw {:?}",
            protocol_flags
        );
    }
}
