//! ZLM Webhook 处理

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::cache;
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
    pub stream_id: String,
    pub port: Option<u16>,
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

    if let (Some(redis), Some(media_server_id)) = (&state.redis, media_server_id) {
        if data.register {
            cache::incr_media_server_streams(redis, media_server_id).await;
        } else {
            cache::decr_media_server_streams(redis, media_server_id).await;
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

                if crate::sip::gb28181::stream_reconnect::StreamReconnectManager::is_gb28181_stream(&data.stream) {
                    if let Some((device_id, channel_id)) = crate::sip::gb28181::stream_reconnect::StreamReconnectManager::parse_stream_id(&data.stream) {
                        tracing::info!("GB28181 stream not found, attempting reconnect: device={} channel={}", device_id, channel_id);

                        if let Some(ref sip_server) = state.sip_server {
                            let sip = sip_server.read().await;
                            let device_online = sip.is_device_online(&device_id).await;

                            if device_online {
                                match sip.send_play_invite_and_wait(&device_id, &channel_id, 0, None).await {
                                    Ok(_) => {
                                        tracing::info!("Stream reconnect INVITE sent for {}/{}", device_id, channel_id);
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
            if let Some(data) = serde_json::from_value::<PlayData>(event.clone()).ok() {
                tracing::info!("Play request: {}/{}/{} from {}", 
                    data.schema, data.app, data.stream, data.ip);
            }
        }
        "on_publish" => {
            if let Some(data) = serde_json::from_value::<PublishData>(event.clone()).ok() {
                tracing::info!("Publish: {}/{}/{} from {}", 
                    data.schema, data.app, data.stream, data.ip);
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
                
                // Update media server DB record with actual ports
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
                        ("hook.on_rtp_server_timeout", hook_url.clone()),
                    ];
                    for (key, value) in config_items {
                        if let Err(e) = zlm_client.set_server_config(&secret, key, &value).await {
                            tracing::warn!("Failed to set ZLM config {}={}: {}", key, value, e);
                        }
                    }
                    tracing::info!("ZLM hook URLs reconfigured for server {}", media_server_id);
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
        "on_rtp_server_timeout" => {
            if let Some(data) = serde_json::from_value::<RtpServerTimeoutData>(event.clone()).ok() {
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
        _ => {
            tracing::debug!("Unhandled webhook: {}", hook_name);
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "code": 0
    })))
}
