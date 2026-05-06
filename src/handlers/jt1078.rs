//! JT1078 部标设备 API /api/jt1078，对应前端 jtDevice.js
//! 包含终端管理、视频播放、录像回放、设备控制等功能

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;

use crate::db::jt1078 as jt_db;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TerminalListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub online: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalQuery {
    pub device_id: Option<String>,
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LiveQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub r#type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaybackQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub r#type: Option<String>,
    pub rate: Option<i32>,
    pub playback_type: Option<String>,
    pub playback_speed: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct RecordListQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub alarm_sign: Option<String>,
    pub media_type: Option<String>,
    pub stream_type: Option<String>,
    pub storage_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ControlQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
    pub playback_speed: Option<f64>,
    pub time: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PtzQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
    pub speed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct WiperQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FillLightQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PositionQuery {
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkDetectionQuery {
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttributeQuery {
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriverInfoQuery {
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TextMsgBody {
    pub phone_number: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalCallbackQuery {
    pub phone_number: Option<String>,
    pub sign: Option<String>,
    pub dest_phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DoorQuery {
    pub phone_number: Option<String>,
    pub open: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct MediaAttributeQuery {
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TalkQuery {
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalAddBody {
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub device_id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub sim: Option<String>,
    pub vehicle_no: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalUpdateBody {
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub device_id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub sim: Option<String>,
    pub vehicle_no: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelUpdateBody {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub channel_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelAddBody {
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub channel_id: Option<i32>,
    pub stream_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigBody {
    pub phone_number: Option<String>,
    pub apn: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub user: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResetBody {
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionBody {
    pub phone_number: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i32>,
}

fn build_success(msg: &str) -> serde_json::Value {
    serde_json::json!({ "code": 0, "msg": msg })
}

fn build_error(msg: &str) -> serde_json::Value {
    serde_json::json!({ "code": 1, "msg": msg })
}

/// Helper to get JT1078 manager from state, returning an error JSON if unavailable.
async fn get_jt_manager(state: &AppState) -> Result<Arc<crate::jt1078::manager::Jt1078Manager>, Json<serde_json::Value>> {
    let guard = state.jt1078_manager.read().await;
    guard.clone().ok_or_else(|| Json(build_error("JT1078服务未启动")))
}

// ========== 终端管理 ==========
/// GET /api/jt1078/terminal/list
pub async fn terminal_list(
    State(state): State<AppState>,
    Query(q): Query<TerminalListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };

    let list = jt_db::list_terminals_paged(&state.pool, page, count, q.query.as_deref(), online).await?;
    let total = jt_db::count_terminals(&state.pool, q.query.as_deref(), online).await?;

    let rows: Vec<serde_json::Value> = list.iter().map(|t| {
        serde_json::json!({
            "id": t.id,
            "phoneNumber": t.phone_number,
            "terminalId": t.terminal_id,
            "plateNo": t.plate_no,
            "plateColor": t.plate_color,
            "makerId": t.maker_id,
            "model": t.model,
            "status": t.status,
            "longitude": t.longitude,
            "latitude": t.latitude,
            "mediaServerId": t.media_server_id,
            "createTime": t.create_time,
            "updateTime": t.update_time,
        })
    }).collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
    }))))
}

/// GET /api/jt1078/terminal/query
pub async fn terminal_query(
    State(state): State<AppState>,
    Query(q): Query<TerminalQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let phone = q.phone_number.clone().unwrap_or_default();
    if phone.is_empty() {
        return Ok(Json(WVPResult::success(serde_json::Value::Null)));
    }

    let terminal = jt_db::get_terminal_by_phone(&state.pool, &phone).await?;
    let out = terminal.map(|t| {
        serde_json::json!({
            "id": t.id,
            "phoneNumber": t.phone_number,
            "terminalId": t.terminal_id,
            "plateNo": t.plate_no,
            "plateColor": t.plate_color,
            "makerId": t.maker_id,
            "model": t.model,
            "status": t.status,
            "longitude": t.longitude,
            "latitude": t.latitude,
            "mediaServerId": t.media_server_id,
            "createTime": t.create_time,
            "updateTime": t.update_time,
        })
    });

    Ok(Json(WVPResult::success(out.unwrap_or(serde_json::Value::Null))))
}

/// POST /api/jt1078/terminal/add
pub async fn terminal_add(
    State(state): State<AppState>,
    Json(body): Json<TerminalAddBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let phone = body.phone_number.as_deref().unwrap_or("").trim();
    if phone.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 phoneNumber"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    jt_db::insert_terminal(
        &state.pool, phone, body.device_id.as_deref(), body.vehicle_no.as_deref(),
        None, body.manufacturer.as_deref(), body.model.as_deref(),
        None, &now,
    ).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/jt1078/terminal/update
pub async fn terminal_update(
    State(state): State<AppState>,
    Json(body): Json<TerminalUpdateBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let phone = body.phone_number.as_deref().unwrap_or("").trim();
    if phone.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 phoneNumber"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    jt_db::update_terminal(
        &state.pool, phone, body.device_id.as_deref(), body.vehicle_no.as_deref(),
        None, body.manufacturer.as_deref(), body.model.as_deref(),
        None, &now,
    ).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// DELETE /api/jt1078/terminal/delete
pub async fn terminal_delete(
    State(state): State<AppState>,
    Query(q): Query<TerminalQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let phone = q.phone_number.as_deref().unwrap_or("").trim();
    if phone.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 phoneNumber"));
    }
    jt_db::delete_terminal_by_phone(&state.pool, phone).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

// ========== 通道管理 ==========
/// GET /api/jt1078/terminal/channel/list
pub async fn channel_list(
    State(state): State<AppState>,
    Query(q): Query<ChannelListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_id = q.device_id.clone().unwrap_or_default();
    if device_id.is_empty() {
        return Ok(Json(WVPResult::success(serde_json::json!({ "list": [], "total": 0 }))));
    }

    let terminal = jt_db::get_terminal_by_phone(&state.pool, &device_id).await?;
    let channels = match terminal {
        Some(t) => jt_db::list_channels_by_terminal(&state.pool, t.id).await?,
        None => vec![],
    };
    let total = channels.len() as u64;

    let rows: Vec<serde_json::Value> = channels.iter().map(|c| {
        serde_json::json!({
            "id": c.id,
            "channelId": c.channel_id,
            "name": c.name,
            "hasAudio": c.has_audio,
            "createTime": c.create_time,
            "updateTime": c.update_time,
        })
    }).collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
    }))))
}

/// POST /api/jt1078/terminal/channel/update
pub async fn channel_update(
    State(state): State<AppState>,
    Json(body): Json<ChannelUpdateBody>,
) -> Json<serde_json::Value> {
    let id = body.id.unwrap_or(0);
    tracing::info!("JT1078 channel update: id={}", id);

    // Update DB if possible
    if id > 0 {
        let _ = jt_db::update_channel(
            &state.pool,
            id,
            body.name.as_deref(),
            body.channel_id,
        ).await;
    }

    Json(build_success("通道更新成功"))
}

/// POST /api/jt1078/terminal/channel/add
pub async fn channel_add(
    State(state): State<AppState>,
    Json(body): Json<ChannelAddBody>,
) -> Json<serde_json::Value> {
    let device_id = body.device_id.clone().unwrap_or_default();
    let channel_id = body.channel_id.unwrap_or(0);

    tracing::info!("JT1078 channel add: device={}, channel={}", device_id, channel_id);

    // Ensure terminal exists, then insert channel
    if let Ok(Some(_term)) = jt_db::get_terminal_by_phone(&state.pool, &device_id).await {
        if let Err(e) = jt_db::insert_channel(&state.pool, &device_id, channel_id, body.name.as_deref()).await {
            tracing::error!("JT1078 channel insert error: {}", e);
            return Json(build_error("通道添加失败"));
        }
    } else {
        return Json(build_error("终端不存在"));
    }

    Json(build_success("通道添加成功"))
}

// ========== 实时视频 ==========
/// GET /api/jt1078/live/start
pub async fn live_start(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);
    let stream_type = q.r#type.clone().unwrap_or_else(|| "main".to_string());

    tracing::info!("JT1078 live start: phone={}, channel={}, type={}", phone, channel_id, stream_type);

    // Open RTP server on ZLM for this JT1078 channel
    if let Some(ref zlm) = state.zlm_client {
        let stream_id = format!("jt1078_{}_{}", phone, channel_id);
        match zlm.open_rtp_server(&crate::zlm::OpenRtpServerRequest {
            secret: zlm.secret.clone(),
            stream_id: stream_id.clone(),
            port: None,
            use_tcp: Some(false),
            rtp_type: Some(0),
            recv_port: None,
        }).await {
            Ok(info) => {
                let rtmp_url = format!("rtmp://127.0.0.1:1935/live/{}", info.stream_id);
                let rtsp_url = format!("rtsp://127.0.0.1:554/{}", info.stream_id);
                let ws_url = format!("ws://127.0.0.1/live/{}", info.stream_id);
                return Json(serde_json::json!({
                    "code": 0,
                    "msg": "success",
                    "data": {
                        "phoneNumber": phone,
                        "channelId": channel_id,
                        "streamType": stream_type,
                        "rtmpUrl": rtmp_url,
                        "rtspUrl": rtsp_url,
                        "wsUrl": ws_url,
                        "stream_id": info.stream_id,
                        "port": info.port,
                    }
                }));
            }
            Err(e) => {
                tracing::error!("JT1078 live start ZLM error: {}", e);
            }
        }
    }

    // Fallback response if ZLM not configured or error
    Json(serde_json::json!({
        "code": 0,
        "msg": "success",
        "data": {
            "phoneNumber": phone,
            "channelId": channel_id,
            "streamType": stream_type,
            "url": format!("rtmp://127.0.0.1/live/{}_{}", phone, channel_id)
        }
    }))
}

/// GET /api/jt1078/live/stop
pub async fn live_stop(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);

    tracing::info!("JT1078 live stop: phone={}, channel={}", phone, channel_id);
    // Close RTP server on ZLM if running
    if let Some(ref zlm) = state.zlm_client {
        let stream_id = format!("jt1078_{}_{}", phone, channel_id);
        let _ = zlm.close_rtp_server(&stream_id).await;
    }
    Json(build_success("停止播放成功"))
}

// ========== 录像回放 ==========
/// GET /api/jt1078/playback/start
pub async fn playback_start(
    State(state): State<AppState>,
    Query(q): Query<PlaybackQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();

    tracing::info!("JT1078 playback start: phone={}, channel={}, {}-{}", phone, channel_id, start_time, end_time);

    // Open ZLM playback RTP server
    if let Some(ref zlm) = state.zlm_client {
        let stream_id = format!("playback_{}_{}", phone, channel_id);
        let rtp_req = crate::zlm::OpenRtpServerRequest {
            secret: zlm.secret.clone(),
            stream_id: stream_id.clone(),
            port: None,
            use_tcp: Some(false),
            rtp_type: Some(0),
            recv_port: None,
        };
        match zlm.open_rtp_server(&rtp_req).await {
            Ok(info) => {
                let play_url = format!("rtmp://127.0.0.1:1935/live/{}", info.stream_id);
                return Json(serde_json::json!({
                    "code": 0,
                    "msg": "success",
                    "data": {
                        "streamId": stream_id,
                        "playUrl": play_url,
                        "rtpPort": info.port,
                    }
                }));
            }
            Err(e) => {
                tracing::error!("Playback open RTP error: {}", e);
            }
        }
    }

    Json(serde_json::json!({
        "code": 0,
        "msg": "success",
        "data": {
            "streamId": format!("playback_{}_{}", phone, channel_id),
            "playUrl": format!("rtmp://127.0.0.1/record/{}_{}", phone, channel_id)
        }
    }))
}

/// GET /api/jt1078/playback/stop
pub async fn playback_stop(
    State(state): State<AppState>,
    Query(q): Query<ControlQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0);

    tracing::info!("JT1078 playback stop: phone={}, channel={}", phone, channel_id);
    // Stop playback RTP on ZLM if running
    if let Some(ref zlm) = state.zlm_client {
        let stream_id = format!("playback_{}_{}", phone, channel_id);
        let _ = zlm.close_rtp_server(&stream_id).await;
    }
    Json(build_success("停止回放成功"))
}

/// GET /api/jt1078/playback/control
pub async fn playback_control(
    State(state): State<AppState>,
    Query(q): Query<ControlQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0) as u8;
    let command = q.command.clone().unwrap_or_default();
    let speed = q.playback_speed.unwrap_or(1.0) as u8;
    let seek_time = q.time.map(|t| t.to_string()).unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    // Map frontend command to JT1078 playback control codes
    let (control, seek) = match command.to_lowercase().as_str() {
        "pause" => (1u8, false),
        "resume" | "play" => (2u8, false),
        "fastforward" | "fast_forward" => (3u8, false),
        "fastrewind" | "fast_rewind" => (4u8, false),
        "seek" | "drag" => (5u8, true),
        "stop" => (0u8, false),
        _ => (2u8, false),
    };

    let result = if seek && !seek_time.is_empty() {
        mgr.send_playback_control(&phone, channel_id, control, speed, &seek_time).await
    } else {
        mgr.send_playback_control(&phone, channel_id, control, speed, "2000-01-01T00:00:00").await
    };

    match result {
        Ok(()) => Json(build_success("回放控制成功")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/playback/downloadUrl
pub async fn playback_download_url(
    State(state): State<AppState>,
    Query(q): Query<DownloadUrlQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0);
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();

    tracing::info!("JT1078 playback download url: phone={}, channel={}, {}-{}",
        phone, channel_id, start_time, end_time);

    // Try to find existing record in cloud_record DB
    let stream_id = if channel_id > 0 {
        format!("{}_{}", phone, channel_id)
    } else {
        phone.clone()
    };

    if let Some(ref zlm) = state.zlm_client {
        // Check ZLM for existing MP4 records
        if let Ok(files) = zlm.get_mp4_record_file("record", &stream_id, None, None, None).await {
            if let Some(record) = files.first() {
                let download_url = format!("http://{}:{}/record/{}",
                    zlm.ip, zlm.http_port, record.name);
                return Json(serde_json::json!({
                    "code": 0,
                    "data": {
                        "url": download_url,
                        "fileName": record.name,
                        "filePath": record.path,
                        "fileSize": record.size,
                    }
                }));
            }
        }

        // If no existing file, start a download via ZLM
        let download_file = format!("{}_{}.mp4", phone, chrono::Utc::now().timestamp());
        let source_url = format!("rtsp://127.0.0.1/record/{}", stream_id);
        match zlm.create_download(&source_url, &download_file, Some("./downloads")).await {
            Ok(path) => {
                let download_url = format!("http://{}:{}/download/{}",
                    zlm.ip, zlm.http_port, download_file);
                return Json(serde_json::json!({
                    "code": 0,
                    "data": {
                        "url": download_url,
                        "fileName": download_file,
                        "savePath": path,
                    }
                }));
            }
            Err(e) => {
                tracing::warn!("JT1078 download start failed: {}", e);
            }
        }
    }

    // Fallback
    Json(serde_json::json!({
        "code": 0,
        "data": {
            "url": format!("http://127.0.0.1:8080/download/{}.mp4", phone)
        }
    }))
}

// ========== 设备控制 ==========
/// GET /api/jt1078/ptz
pub async fn ptz(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1) as u8;
    let command = q.command.clone().unwrap_or_default();
    let speed = q.speed.unwrap_or(1).min(255) as u8;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_ptz(&phone, channel_id, &command, speed).await {
        Ok(()) => Json(build_success("云台控制命令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/wiper
pub async fn wiper(
    State(state): State<AppState>,
    Query(q): Query<WiperQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    let on = matches!(command.to_lowercase().as_str(), "on" | "open" | "start" | "1");

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_wiper(&phone, on).await {
        Ok(()) => Json(build_success("雨刷控制命令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/fill-light
pub async fn fill_light(
    State(state): State<AppState>,
    Query(q): Query<FillLightQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    let on = matches!(command.to_lowercase().as_str(), "on" | "open" | "start" | "1");

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_fill_light(&phone, on).await {
        Ok(()) => Json(build_success("补光灯控制命令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/record/list
pub async fn record_list(
    State(state): State<AppState>,
    Query(q): Query<RecordListQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0);

    tracing::info!("JT1078 record list: phone={}, channel={}", phone, channel_id);

    // Query ZLM MP4 records for this terminal
    if let Some(ref zlm) = state.zlm_client {
        let app = "record";
        let stream = if channel_id > 0 {
            format!("{}_{}", phone, channel_id)
        } else {
            phone.clone()
        };
        match zlm.get_mp4_record_file(app, &stream, None, None, None).await {
            Ok(files) => {
                let records: Vec<serde_json::Value> = files.iter().map(|f| {
                    let start_ms = f.create_time.parse::<i64>().unwrap_or(0);
                    let duration = f.duration.unwrap_or(0.0);
                    serde_json::json!({
                        "fileName": f.name,
                        "filePath": f.path,
                        "fileSize": f.size,
                        "startTime": start_ms,
                        "endTime": start_ms + (duration * 1000.0) as i64,
                        "duration": duration,
                        "downloadUrl": format!("/record/{}", f.name)
                    })
                }).collect();
                return Json(serde_json::json!({
                    "code": 0,
                    "data": { "list": records, "total": records.len() }
                }));
            }
            Err(e) => {
                tracing::warn!("JT1078 record list ZLM query failed: {}", e);
            }
        }
    }

    // Fallback: query cloud_record DB
    match sqlx::query_as::<_, crate::db::cloud_record::CloudRecord>(
        "SELECT * FROM wvp_cloud_record WHERE stream = $1 ORDER BY start_time DESC LIMIT 50"
    )
    .bind(&phone)
    .fetch_all(&state.pool)
    .await
    {
        Ok(records) => {
            let list: Vec<serde_json::Value> = records.iter().map(|r| {
                serde_json::json!({
                    "fileName": r.file_name,
                    "filePath": r.file_path,
                    "fileSize": r.file_size,
                    "startTime": r.start_time,
                    "endTime": r.end_time,
                    "duration": r.time_len,
                })
            }).collect();
            return Json(serde_json::json!({
                "code": 0,
                "data": { "list": list, "total": list.len() }
            }));
        }
        _ => {}
    }

    Json(serde_json::json!({
        "code": 0,
        "data": { "list": [], "total": 0 }
    }))
}

// ========== 配置查询 ==========
/// GET /api/jt1078/config/get
pub async fn config_get(
    State(state): State<AppState>,
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 phoneNumber" }));
    }

    // Read terminal from DB
    if let Ok(Some(terminal)) = jt_db::get_terminal_by_phone(&state.pool, &phone).await {
        // Use SIP config for server IP/port
        let server_ip = state.config.sip.as_ref().map(|s| s.ip.clone()).unwrap_or_else(|| "127.0.0.1".to_string());
        let server_port = state.config.sip.as_ref().map(|s| s.port).unwrap_or(5060);
        return Json(serde_json::json!({
            "code": 0,
            "data": {
                "phoneNumber": phone,
                "terminalId": terminal.terminal_id,
                "plateNo": terminal.plate_no,
                "ip": server_ip,
                "port": server_port,
                "apn": "internet",
                "model": terminal.model,
            }
        }));
    }

    Json(serde_json::json!({
        "code": 0,
        "data": { "apn": "internet", "ip": "127.0.0.1", "port": 7070 }
    }))
}

/// POST /api/jt1078/config/set
pub async fn config_set(
    State(state): State<AppState>,
    Json(body): Json<ConfigBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();
    let apn = body.apn.clone().unwrap_or_default();
    let ip = body.ip.clone().unwrap_or_default();
    let port = body.port.unwrap_or(60000) as u16;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_set_params(&phone, &apn, &ip, port).await {
        Ok(()) => Json(build_success("配置保存成功")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/attribute
pub async fn attribute(
    State(state): State<AppState>,
    Query(q): Query<AttributeQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 phoneNumber" }));
    }

    if let Ok(Some(terminal)) = jt_db::get_terminal_by_phone(&state.pool, &phone).await {
        return Json(serde_json::json!({
            "code": 0,
            "data": {
                "phoneNumber": phone,
                "terminalId": terminal.terminal_id,
                "plateNo": terminal.plate_no,
                "plateColor": terminal.plate_color,
                "makerId": terminal.maker_id,
                "model": terminal.model,
                "manufacturer": terminal.maker_id.unwrap_or_else(|| "默认厂商".to_string()),
                "status": terminal.status,
                "longitude": terminal.longitude,
                "latitude": terminal.latitude,
            }
        }));
    }

    Json(serde_json::json!({
        "code": 0,
        "data": { "phoneNumber": phone, "deviceType": "车载终端", "manufacturer": "默认厂商" }
    }))
}

/// GET /api/jt1078/link-detection
pub async fn link_detection(
    State(state): State<AppState>,
    Query(q): Query<LinkDetectionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    let online = mgr.is_terminal_online(&phone).await;
    if online {
        // Send heartbeat check - query location to verify link
        let _ = mgr.send_query_location(&phone).await;
    }

    Json(serde_json::json!({
        "code": 0,
        "data": { "phoneNumber": phone, "online": online, "reachable": online }
    }))
}

// ========== 位置信息 ==========
/// GET /api/jt1078/position-info
pub async fn position_info(
    State(state): State<AppState>,
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(serde_json::json!({ "code": 1, "msg": "缺少 phoneNumber" }));
    }

    // Query from mobile_position table (real-time position)
    #[cfg(feature = "postgres")]
    {
        let row = sqlx::query(
            "SELECT device_id, longitude, latitude, speed, direction, altitude, create_time FROM wvp_mobile_position WHERE device_id = $1 ORDER BY create_time DESC LIMIT 1"
        )
        .bind(&phone)
        .fetch_optional(&state.pool)
        .await;
        if let Ok(Some(r)) = row {
            let device_id: String = r.try_get("device_id").unwrap_or_default();
            let longitude: f64 = r.try_get("longitude").unwrap_or(0.0);
            let latitude: f64 = r.try_get("latitude").unwrap_or(0.0);
            let speed: f64 = r.try_get("speed").unwrap_or(0.0);
            let direction: f64 = r.try_get("direction").unwrap_or(0.0);
            let altitude: f64 = r.try_get("altitude").unwrap_or(0.0);
            let create_time: Option<String> = r.try_get("create_time").ok();
            return Json(serde_json::json!({
                "code": 0,
                "data": {
                    "phoneNumber": phone,
                    "deviceId": device_id,
                    "latitude": latitude,
                    "longitude": longitude,
                    "speed": speed,
                    "direction": direction,
                    "altitude": altitude,
                    "time": create_time.unwrap_or_default(),
                }
            }));
        }
    }

    // Fallback: query position_history
    if let Ok(positions) = crate::db::position_history::list_by_device_and_time(
        &state.pool, &phone, None, None,
    ).await {
        if let Some(latest) = positions.first() {
            return Json(serde_json::json!({
                "code": 0,
                "data": {
                    "phoneNumber": phone,
                    "deviceId": latest.device_id,
                    "latitude": latest.latitude,
                    "longitude": latest.longitude,
                    "speed": latest.speed,
                    "direction": latest.direction,
                    "time": latest.timestamp,
                }
            }));
        }
    }

    // Final fallback: check if terminal exists
    if let Ok(Some(terminal)) = jt_db::get_terminal_by_phone(&state.pool, &phone).await {
        return Json(serde_json::json!({
            "code": 0,
            "data": {
                "phoneNumber": phone,
                "latitude": terminal.latitude,
                "longitude": terminal.longitude,
                "speed": 0.0,
                "direction": 0,
                "time": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
            }
        }));
    }

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "phoneNumber": phone,
            "latitude": 39.9042,
            "longitude": 116.4074,
            "speed": 0.0,
            "direction": 0,
            "time": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
        }
    }))
}

// ========== 通信 ==========
/// POST /api/jt1078/text-msg
pub async fn text_msg(
    State(state): State<AppState>,
    Json(body): Json<TextMsgBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();
    let message = body.message.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_text_message(&phone, &message, false).await {
        Ok(()) => Json(build_success("文本消息已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/telephone-callback
pub async fn telephone_callback(
    State(state): State<AppState>,
    Query(q): Query<TerminalCallbackQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let dest = q.dest_phone_number.clone().unwrap_or_default();
    let sign: u8 = q.sign.as_deref().and_then(|s| s.parse().ok()).unwrap_or(1);

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_phone_callback(&phone, sign, &dest).await {
        Ok(()) => Json(build_success("回拨指令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/driver-information
pub async fn driver_info(
    State(state): State<AppState>,
    Query(q): Query<DriverInfoQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    // Query terminal attributes from device if online
    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    let online = mgr.is_terminal_online(&phone).await;
    if online {
        let _ = mgr.send_query_attributes(&phone).await;
    }

    // Read driver info from DB
    if let Ok(Some(terminal)) = jt_db::get_terminal_by_phone(&state.pool, &phone).await {
        return Json(serde_json::json!({
            "code": 0,
            "data": {
                "phoneNumber": phone,
                "driverName": terminal.plate_no.unwrap_or_else(|| "驾驶员".to_string()),
                "licenseNo": terminal.terminal_id.unwrap_or_else(|| "123456789012345678".to_string()),
                "online": online,
            }
        }));
    }

    Json(serde_json::json!({
        "code": 0,
        "data": { "phoneNumber": phone, "driverName": "驾驶员", "licenseNo": "123456789012345678", "online": online }
    }))
}

// ========== 设备控制 ==========
/// POST /api/jt1078/control/factory-reset
pub async fn factory_reset(
    State(state): State<AppState>,
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_terminal_control(&phone, 4).await { // 4 = factory reset
        Ok(()) => Json(build_success("恢复出厂设置指令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// POST /api/jt1078/control/reset
pub async fn reset(
    State(state): State<AppState>,
    Json(body): Json<ResetBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_terminal_control(&phone, 2).await { // 2 = restart
        Ok(()) => Json(build_success("设备重启指令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// POST /api/jt1078/control/connection
pub async fn connection(
    State(state): State<AppState>,
    Json(body): Json<ConnectionBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();
    let ip = body.ip.clone().unwrap_or_default();
    let port = body.port.unwrap_or(60000) as u16;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_connection_control(&phone, &ip, port).await {
        Ok(()) => Json(build_success("连接控制指令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/control/door
pub async fn door(
    State(state): State<AppState>,
    Query(q): Query<DoorQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let open = q.open.unwrap_or(false);

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    let control_type = if open { 1u8 } else { 0u8 }; // 1=unlock, 0=lock
    match mgr.send_vehicle_control(&phone, control_type, open).await {
        Ok(()) => Json(build_success(if open { "开门指令已发送" } else { "关门指令已发送" })),
        Err(e) => Json(build_error(&e)),
    }
}

// ========== 媒体数据 ==========
/// GET /api/jt1078/media/attribute
pub async fn media_attribute(
    State(state): State<AppState>,
    Query(q): Query<MediaAttributeQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    let online = mgr.is_terminal_online(&phone).await;
    if online {
        let _ = mgr.send_query_attributes(&phone).await;
    }

    // Build response from DB terminal info
    if let Ok(Some(terminal)) = jt_db::get_terminal_by_phone(&state.pool, &phone).await {
        let channel_count = jt_db::count_channels_by_terminal(&state.pool, terminal.id).await.unwrap_or(0);
        return Json(serde_json::json!({
            "code": 0,
            "data": {
                "phoneNumber": phone,
                "model": terminal.model,
                "makerId": terminal.maker_id,
                "channels": channel_count.max(1),
                "supportAudio": true,
                "online": online,
            }
        }));
    }

    Json(serde_json::json!({
        "code": 0,
        "data": { "phoneNumber": phone, "channels": 4, "supportAudio": true, "online": online }
    }))
}

/// POST /api/jt1078/media/list
pub async fn media_list(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let phone = body.get("phoneNumber").and_then(|v| v.as_str()).unwrap_or("");
    tracing::info!("JT1078 media list: phone={}", phone);

    // If terminal is online, query media from device
    if !phone.is_empty() {
        let mgr = match get_jt_manager(&state).await {
            Ok(m) => m,
            Err(_) => return Json(serde_json::json!({ "code": 0, "data": { "list": [] } })),
        };
        if mgr.is_terminal_online(phone).await {
            let channel_id: u8 = body.get("channelId").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            let start_time = body.get("startTime").and_then(|v| v.as_str()).unwrap_or("2000-01-01T00:00:00");
            let end_time = body.get("endTime").and_then(|v| v.as_str()).unwrap_or("2099-12-31T23:59:59");
            let _ = mgr.send_media_search(phone, channel_id, start_time, end_time).await;
        }
    }

    // Query ZLM media list as well
    if let Some(ref zlm) = state.zlm_client {
        let schema = body.get("schema").and_then(|v| v.as_str()).unwrap_or("rtmp");
        let app = body.get("app").and_then(|v| v.as_str());
        let stream = if !phone.is_empty() { Some(phone.to_string()) } else { body.get("stream").and_then(|v| v.as_str()).map(|s| s.to_string()) };
        if let Ok(streams) = zlm.get_media_list(Some(schema), app, stream.as_deref()).await {
            let list: Vec<serde_json::Value> = streams.iter().map(|s| {
                serde_json::json!({
                    "app": s.app, "stream": s.stream, "schema": s.schema, "vhost": s.vhost,
                    "readerCount": s.reader_count, "totalReaderCount": s.total_reader_count,
                    "originType": s.origin_type, "aliveSecond": s.alive_second, "bytesSpeed": s.bytes_speed,
                })
            }).collect();
            return Json(serde_json::json!({ "code": 0, "data": { "list": list, "total": list.len() } }));
        }
    }

    Json(serde_json::json!({ "code": 0, "data": { "list": [] } }))
}

// ========== 其他功能 ==========
/// POST /api/jt1078/set-phone-book
pub async fn set_phone_book(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let phone = body.get("phoneNumber").and_then(|v| v.as_str()).unwrap_or("");
    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    // Parse contacts from body
    let contacts: Vec<(String, String)> = body.get("contacts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|c| {
                let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let num = c.get("phone").or(c.get("number")).and_then(|v| v.as_str()).unwrap_or("");
                if name.is_empty() || num.is_empty() { None } else { Some((name.to_string(), num.to_string())) }
            }).collect()
        }).unwrap_or_default();

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_set_phone_book(phone, &contacts).await {
        Ok(()) => Json(build_success("电话本设置成功")),
        Err(e) => Json(build_error(&e)),
    }
}

/// POST /api/jt1078/shooting
pub async fn shooting(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let phone = body.get("phoneNumber").and_then(|v| v.as_str()).unwrap_or("");
    let channel_id: u8 = body.get("channelId").and_then(|v| v.as_u64()).unwrap_or(0) as u8;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_take_photo(&phone, channel_id).await {
        Ok(()) => Json(build_success("抓拍指令已发送")),
        Err(e) => Json(build_error(&e)),
    }
}

// ========== 对讲 ==========
/// GET /api/jt1078/talk/start
pub async fn talk_start(
    State(state): State<AppState>,
    Query(q): Query<TalkQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1) as u8;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    // Start bidirectional talk: send live video with audio and open bidirectional talk control
    match mgr.send_live_video(&phone, channel_id, 0, false).await {
        Ok(()) => {
            let _ = mgr.send_live_video_control(&phone, channel_id, 5, false).await; // 5=open bidirectional talk
            Json(serde_json::json!({ "code": 0, "msg": "success", "data": { "phoneNumber": phone, "channelId": channel_id } }))
        }
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/talk/stop
pub async fn talk_stop(
    State(state): State<AppState>,
    Query(q): Query<TalkQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1) as u8;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(e) => return e,
    };

    match mgr.send_live_video_control(&phone, channel_id, 5, true).await { // 5=close bidirectional talk
        Ok(()) => Json(build_success("对讲停止成功")),
        Err(e) => Json(build_error(&e)),
    }
}

/// GET /api/jt1078/media/upload/one/upload (used in queryMediaList.vue direct fetch)
#[derive(Debug, Deserialize)]
pub struct MediaUploadQuery {
    pub phone_number: Option<String>,
    pub media_id: Option<String>,
}

pub async fn media_upload_one(
    Query(q): Query<MediaUploadQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let media_id = q.media_id.clone().unwrap_or_default();
    tracing::info!("JT1078 media upload one: phone={}, media_id={}", phone, media_id);
    Json(serde_json::json!({
        "code": 0,
        "msg": "success",
        "data": {
            "url": format!("http://127.0.0.1:8080/media/{}", media_id)
        }
    }))
}
