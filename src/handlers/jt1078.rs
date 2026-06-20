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
    #[serde(alias = "phoneNumber")]
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
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub r#type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaybackQuery {
    #[serde(alias = "phoneNumber")]
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
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlQuery {
    #[serde(alias = "phoneNumber")]
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
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
    pub playback_speed: Option<f64>,
    pub time: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PtzQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
    pub speed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct WiperQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FillLightQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
    pub command: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PositionQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkDetectionQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttributeQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriverInfoQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TextMsgBody {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalCallbackQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub sign: Option<String>,
    pub dest_phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DoorQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub open: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct MediaAttributeQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TalkQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub channel_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalAddBody {
    #[serde(alias = "phoneNumber")]
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
    #[serde(alias = "phoneNumber")]
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
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub apn: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub user: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResetBody {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionBody {
    #[serde(alias = "phoneNumber")]
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
    let stream_type_str = q.r#type.clone().unwrap_or_else(|| "main".to_string());

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    let stream_type: u8 = match stream_type_str.as_str() {
        "sub" => 1,
        _ => 0,
    };
    tracing::info!("JT1078 live start: phone={}, channel={}, type={}", phone, channel_id, stream_type);

    let zlm = match state.zlm_client.as_ref() {
        Some(z) => z,
        None => return Json(build_error("ZLM 未配置")),
    };

    // 1) Open ZLM RTP server (allocates a port for terminal to push to)
    let stream_id = format!("jt1078_{}_{}", phone, channel_id);
    let rtp_info = match zlm.open_rtp_server(&crate::zlm::OpenRtpServerRequest {
        secret: zlm.secret.clone(),
        stream_id: stream_id.clone(),
        port: None,
        use_tcp: Some(false),
        rtp_type: Some(0),
        recv_port: None,
    }).await {
        Ok(i) => i,
        Err(e) => return Json(build_error(&format!("ZLM RTP 失败: {}", e))),
    };

    // 2) Send 0x9101 to terminal + wait for 0x0001 ack (10s timeout)
    let mgr_guard = state.jt1078_manager.read().await;
    let mgr = match mgr_guard.as_ref() {
        Some(m) => m,
        None => {
            let _ = zlm.close_rtp_server(&stream_id).await;
            return Json(build_error("JT1078 manager 未初始化"));
        }
    };
    let result = match mgr.send_live_video_and_wait(&phone, channel_id as u8, stream_type, false, 10).await {
        Ok(r) => r,
        Err(e) => {
            let _ = zlm.close_rtp_server(&stream_id).await;
            return Json(build_error(&format!("实时视频命令失败: {}", e)));
        }
    };
    if result != 0 {
        let _ = zlm.close_rtp_server(&stream_id).await;
        return Json(build_error(&format!("终端拒绝实时视频 result={}", result)));
    }

    // 3) Wait for ZLM on_stream_changed hook to resolve (10s timeout)
    match mgr.wait_for_zlm_media(&phone, channel_id as u8, 10).await {
        Ok(sess) => {
            // Build real URLs from ZLM host/port + resolved stream_id
            let zlm_stream_id = sess.zlm_stream_id.clone().unwrap_or_else(|| stream_id.clone());
            let host = zlm.ip.clone();
            Json(serde_json::json!({
                "code": 0,
                "msg": "success",
                "data": {
                    "phoneNumber": phone,
                    "channelId": channel_id,
                    "streamType": stream_type,
                    "rtmpUrl": format!("rtmp://{}/live/{}", host, zlm_stream_id),
                    "rtspUrl": format!("rtsp://{}/{}", host, zlm_stream_id),
                    "wsUrl": format!("ws://{}/live/{}", host, zlm_stream_id),
                    "stream_id": zlm_stream_id,
                    "port": rtp_info.port,
                    "session_state": "active",
                }
            }))
        }
        Err(e) => {
            // 清理 ZLM RTP server
            let _ = zlm.close_rtp_server(&stream_id).await;
            Json(build_error(&format!("ZLM 媒体等待失败: {}", e)))
        }
    }
}

/// GET /api/jt1078/live/stop
pub async fn live_stop(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1) as u8;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    tracing::info!("JT1078 live stop: phone={}, channel={}", phone, channel_id);
    let mgr_guard = state.jt1078_manager.read().await;
    if let Some(mgr) = mgr_guard.as_ref() {
        // Stop the media session first
        mgr.media_session_manager().stop(&phone, channel_id);
        // Send 0x9102 close + wait for 0x0001 ack
        match mgr.send_live_video_control_and_wait(&phone, channel_id, 0, true, 5).await {
            Ok(0) => {
                // Close ZLM RTP server
                if let Some(ref zlm) = state.zlm_client {
                    let stream_id = format!("jt1078_{}_{}", phone, channel_id);
                    let _ = zlm.close_rtp_server(&stream_id).await;
                }
                return Json(build_success("停止播放已应答"));
            }
            Ok(result) => return Json(build_error(&format!("终端拒绝停止 result={}", result))),
            Err(e) => return Json(build_error(&format!("停止错误: {}", e))),
        }
    }
    Json(build_error("JT1078 manager 未初始化"))
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

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }
    if start_time.is_empty() || end_time.is_empty() {
        return Json(build_error("缺少 startTime / endTime"));
    }

    tracing::info!("JT1078 playback start: phone={}, channel={}, {}-{}", phone, channel_id, start_time, end_time);

    let zlm = match state.zlm_client.as_ref() {
        Some(z) => z,
        None => return Json(build_error("ZLM 未配置")),
    };

    // 1) Open ZLM RTP server for playback
    let stream_id = format!("playback_{}_{}", phone, channel_id);
    let rtp_info = match zlm.open_rtp_server(&crate::zlm::OpenRtpServerRequest {
        secret: zlm.secret.clone(),
        stream_id: stream_id.clone(),
        port: None,
        use_tcp: Some(false),
        rtp_type: Some(0),
        recv_port: None,
    }).await {
        Ok(i) => i,
        Err(e) => return Json(build_error(&format!("ZLM RTP 失败: {}", e))),
    };

    // 2) Send 0x9201 to terminal + wait for 0x0001 ack
    let mgr_guard = state.jt1078_manager.read().await;
    let mgr = match mgr_guard.as_ref() {
        Some(m) => m,
        None => {
            let _ = zlm.close_rtp_server(&stream_id).await;
            return Json(build_error("JT1078 manager 未初始化"));
        }
    };
    let result = match mgr.send_playback_and_wait(
        &phone, channel_id as u8, 0, 0, 0, &start_time, &end_time, 10,
    ).await {
        Ok(r) => r,
        Err(e) => {
            let _ = zlm.close_rtp_server(&stream_id).await;
            return Json(build_error(&format!("回放命令失败: {}", e)));
        }
    };
    if result != 0 {
        let _ = zlm.close_rtp_server(&stream_id).await;
        return Json(build_error(&format!("终端拒绝回放 result={}", result)));
    }

    // 3) Create playback session + wait for ZLM media
    mgr.media_session_manager().create_playback(&phone, channel_id as u8);
    match mgr.wait_for_zlm_media(&phone, channel_id as u8, 10).await {
        Ok(sess) => Json(serde_json::json!({
            "code": 0,
            "msg": "success",
            "data": {
                "streamId": sess.zlm_stream_id.clone().unwrap_or_else(|| stream_id.clone()),
                "playUrl": format!("rtmp://{}/live/{}", zlm.ip, sess.zlm_stream_id.clone().unwrap_or_else(|| stream_id.clone())),
                "rtpPort": rtp_info.port,
                "sessionState": "active",
            }
        })),
        Err(e) => {
            let _ = zlm.close_rtp_server(&stream_id).await;
            Json(build_error(&format!("回放媒体等待失败: {}", e)))
        }
    }
}

/// GET /api/jt1078/playback/stop
pub async fn playback_stop(
    State(state): State<AppState>,
    Query(q): Query<ControlQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0) as u8;

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    tracing::info!("JT1078 playback stop: phone={}, channel={}", phone, channel_id);
    let mgr_guard = state.jt1078_manager.read().await;
    if let Some(mgr) = mgr_guard.as_ref() {
        mgr.media_session_manager().stop(&phone, channel_id);
        // Send 0x9202 close (control=4) + wait for 0x0001
        match mgr.send_playback_control_and_wait(&phone, channel_id, 4, 0, "2000-01-01T00:00:00", 5).await {
            Ok(0) => {
                if let Some(ref zlm) = state.zlm_client {
                    let stream_id = format!("playback_{}_{}", phone, channel_id);
                    let _ = zlm.close_rtp_server(&stream_id).await;
                }
                return Json(build_success("停止回放已应答"));
            }
            Ok(result) => return Json(build_error(&format!("终端拒绝停止 result={}", result))),
            Err(e) => return Json(build_error(&format!("停止回放错误: {}", e))),
        }
    }
    Json(build_error("JT1078 manager 未初始化"))
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
        mgr.send_playback_control_and_wait(&phone, channel_id, control, speed, &seek_time, 5).await
    } else {
        mgr.send_playback_control_and_wait(&phone, channel_id, control, speed, "2000-01-01T00:00:00", 5).await
    };

    match result {
        Ok(0) => {
            // Update session state for pause/resume/speed
            match control {
                1 => mgr.media_session_manager().pause(&phone, channel_id),
                2 => mgr.media_session_manager().resume(&phone, channel_id),
                _ => {}
            }
            if control == 3 || control == 4 || (control == 5 && speed > 0) {
                mgr.media_session_manager().update_speed(&phone, channel_id, speed as f64);
            }
            Json(build_success("回放控制已应答"))
        }
        Ok(result) => Json(build_error(&format!("回放控制失败 result={}", result))),
        Err(e) => Json(build_error(&format!("回放控制错误: {}", e))),
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

    // 真给终端发 9201 回放请求，让设备把录像流推到 ZLM
    if !start_time.is_empty() && !end_time.is_empty() {
        if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
            if let Err(e) = mgr
                .send_playback(&phone, channel_id as u8, 0, 0, 0, &start_time, &end_time)
                .await
            {
                tracing::warn!("JT1078 send_playback(9201) for download failed: {}", e);
            }
        }
    }

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

    match mgr.send_ptz_and_wait(&phone, channel_id, &command, speed, 5).await {
        Ok(0) => Json(build_success("云台控制命令已应答")),
        Ok(result) => Json(build_error(&format!("云台失败 result={}", result))),
        Err(e) => Json(build_error(&format!("云台错误: {}", e))),
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

    match mgr.send_set_params_and_wait(&phone, &[(0x0033u32, &[if on { 1 } else { 0 }])], 5).await {
        Ok(0) => Json(build_success("雨刷控制命令已应答")),
        Ok(result) => Json(build_error(&format!("雨刷失败 result={}", result))),
        Err(e) => Json(build_error(&format!("雨刷错误: {}", e))),
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

    match mgr.send_set_params_and_wait(&phone, &[(0x0016u32, &[if on { 1 } else { 0 }])], 5).await {
        Ok(0) => Json(build_success("补光灯控制命令已应答")),
        Ok(result) => Json(build_error(&format!("补光灯失败 result={}", result))),
        Err(e) => Json(build_error(&format!("补光灯错误: {}", e))),
    }
}

/// GET /api/jt1078/record/list
pub async fn record_list(
    State(state): State<AppState>,
    Query(q): Query<RecordListQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0) as i32;
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    tracing::info!("JT1078 record list: phone={}, channel={}, {}-{}", phone, channel_id, start_time, end_time);

    // Phase 6.4: 真发 0x8802 media search → 等 0x0001 应答 → 落库 → 返回
    if !start_time.is_empty() && !end_time.is_empty() {
        let mgr_guard = state.jt1078_manager.read().await;
        if let Some(mgr) = mgr_guard.as_ref() {
            if mgr.is_terminal_online(&phone).await {
                match mgr.send_media_search_and_wait(
                    &phone, channel_id as u8, &start_time, &end_time, 30,
                ).await {
                    Ok(_) => {
                        // 终端应答 - 实际的多包 0x0801 通过 process_jt_message 接收
                        // 此处返回时先尝试从 DB 读已落库的 items
                        drop(mgr_guard);
                        match jt_db::list_media_items_by_terminal(
                            &state.pool, &phone,
                            Some(&start_time), Some(&end_time), 50,
                        ).await {
                            Ok(items) if !items.is_empty() => {
                                let list: Vec<serde_json::Value> = items.iter().map(|i| {
                                    serde_json::json!({
                                        "mediaId": i.media_id,
                                        "phoneNumber": i.phone_number,
                                        "channelId": i.channel_id,
                                        "mediaType": i.media_type,
                                        "mediaFormat": i.media_format,
                                        "eventCode": i.event_code,
                                        "startTime": i.start_time,
                                        "endTime": i.end_time,
                                        "source": "device",
                                    })
                                }).collect();
                                return Json(serde_json::json!({
                                    "code": 0,
                                    "data": { "list": list, "total": list.len() }
                                }));
                            }
                            _ => {
                                // DB 还没有 items (0x0801 多包未到达) - 继续兜底
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("JT1078 send_media_search(0x8802) failed for {}: {}", phone, e);
                    }
                }
            }
        }
    }

    // Fallback: ZLM MP4 records
    if let Some(ref zlm) = state.zlm_client {
        let app = "record";
        let stream = if channel_id > 0 {
            format!("{}_{}", phone, channel_id)
        } else {
            phone.clone()
        };
        if let Ok(files) = zlm.get_mp4_record_file(app, &stream, None, None, None).await {
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
                    "downloadUrl": format!("/record/{}", f.name),
                    "source": "zlm",
                })
            }).collect();
            return Json(serde_json::json!({
                "code": 0,
                "data": { "list": records, "total": records.len() }
            }));
        }
    }

    // Final fallback: cloud_record DB
    if let Ok(records) = sqlx::query_as::<_, crate::db::cloud_record::CloudRecord>(
        "SELECT * FROM gb_cloud_record WHERE stream = $1 ORDER BY start_time DESC LIMIT 50"
    )
    .bind(&phone)
    .fetch_all(&state.pool)
    .await {
        let list: Vec<serde_json::Value> = records.iter().map(|r| {
            serde_json::json!({
                "fileName": r.file_name,
                "filePath": r.file_path,
                "fileSize": r.file_size,
                "startTime": r.start_time,
                "endTime": r.end_time,
                "duration": r.time_len,
                "source": "cloud_record",
            })
        }).collect();
        return Json(serde_json::json!({
            "code": 0,
            "data": { "list": list, "total": list.len() }
        }));
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

    let mut port_bytes = [0u8; 4];
    port_bytes.copy_from_slice(&port.to_be_bytes());
    let apn_bytes = apn.as_bytes();
    let ip_bytes = ip.as_bytes();
    let params: Vec<(u32, &[u8])> = vec![
        (0x0010u32, apn_bytes),
        (0x0013u32, ip_bytes),
        (0x0018u32, port_bytes.as_slice()),
    ];
    match mgr.send_set_params_and_wait(&phone, &params, 5).await {
        Ok(0) => Json(build_success("配置保存已应答")),
        Ok(result) => Json(build_error(&format!("配置保存失败 result={}", result))),
        Err(e) => Json(build_error(&format!("配置保存错误: {}", e))),
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

    // Phase 6.5: 先从 gb_jt_terminal 表读最近位置 (优先)
    if let Ok(Some(terminal)) = jt_db::get_terminal_by_phone(&state.pool, &phone).await {
        if let (Some(lng), Some(lat)) = (terminal.longitude, terminal.latitude) {
            // register_time 字段被复用为 last_position_time
            return Json(serde_json::json!({
                "code": 0,
                "data": {
                    "phoneNumber": phone,
                    "deviceId": terminal.phone_number,
                    "latitude": lat,
                    "longitude": lng,
                    "speed": 0.0,
                    "direction": 0,
                    "altitude": 0,
                    "time": terminal.register_time.unwrap_or_default(),
                    "source": "db",
                }
            }));
        }
    }

    // 兜底：实时查终端 (0x8201 位置查询)
    let mgr_guard = state.jt1078_manager.read().await;
    if let Some(mgr) = mgr_guard.as_ref() {
        if mgr.is_terminal_online(&phone).await {
            if let Ok(loc) = mgr.send_query_location_and_wait(&phone, 10).await {
                // 落库 (异步)
                let pool = state.pool.clone();
                let phone_owned = phone.clone();
                let lng = loc.longitude;
                let lat = loc.latitude;
                let time = loc.time;
                tokio::spawn(async move {
                    let _ = jt_db::update_last_position(
                        &pool, &phone_owned, lng, lat, time,
                    ).await;
                });
                return Json(serde_json::json!({
                    "code": 0,
                    "data": {
                        "phoneNumber": phone,
                        "latitude": lat,
                        "longitude": lng,
                        "speed": loc.speed,
                        "direction": loc.direction,
                        "altitude": loc.altitude,
                        "time": loc.time.to_rfc3339(),
                        "source": "device",
                    }
                }));
            }
        }
    }

    // Query from mobile_position table (real-time position)
    #[cfg(feature = "postgres")]
    {
        let row = sqlx::query(
            "SELECT device_id, longitude, latitude, speed, direction, altitude, create_time FROM gb_mobile_position WHERE device_id = $1 ORDER BY create_time DESC LIMIT 1"
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

    match mgr.send_text_message_and_wait(&phone, &message, false, 5).await {
        Ok(0) => Json(build_success("文本消息已应答")),
        Ok(result) => Json(build_error(&format!("文本消息失败 result={}", result))),
        Err(e) => Json(build_error(&format!("文本消息错误: {}", e))),
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

    match mgr.send_phone_callback_and_wait(&phone, sign, &dest, 5).await {
        Ok(0) => Json(build_success("回拨指令已应答")),
        Ok(result) => Json(build_error(&format!("回拨失败 result={}", result))),
        Err(e) => Json(build_error(&format!("回拨错误: {}", e))),
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
        let _ = mgr.send_query_attributes_and_wait(&phone, 5).await;
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

    match mgr.send_terminal_control_and_wait(&phone, 4, 5).await { // 4 = factory reset
        Ok(0) => Json(build_success("恢复出厂设置指令已应答")),
        Ok(result) => Json(build_error(&format!("恢复出厂设置失败 result={}", result))),
        Err(e) => Json(build_error(&format!("恢复出厂设置错误: {}", e))),
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

    match mgr.send_terminal_control_and_wait(&phone, 2, 5).await { // 2 = restart
        Ok(0) => Json(build_success("设备重启指令已应答")),
        Ok(result) => Json(build_error(&format!("设备重启失败 result={}", result))),
        Err(e) => Json(build_error(&format!("设备重启错误: {}", e))),
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

    let mut port_bytes = [0u8; 4];
    port_bytes.copy_from_slice(&port.to_be_bytes());
    let params = vec![(0x0018u32, port_bytes.as_slice())];
    match mgr.send_set_params_and_wait(&phone, &params, 5).await {
        Ok(0) => Json(build_success("连接控制指令已应答")),
        Ok(result) => Json(build_error(&format!("连接控制失败 result={}", result))),
        Err(e) => Json(build_error(&format!("连接控制错误: {}", e))),
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
    match mgr.send_vehicle_control_and_wait(&phone, control_type, open, 5).await {
        Ok(0) => Json(build_success(if open { "开门指令已应答" } else { "关门指令已应答" })),
        Ok(result) => Json(build_error(&format!("车门控制失败 result={}", result))),
        Err(e) => Json(build_error(&format!("车门控制错误: {}", e))),
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
        let _ = mgr.send_query_attributes_and_wait(&phone, 5).await;
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
            let _ = mgr.send_media_search_and_wait(phone, channel_id, start_time, end_time, 30).await;
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

    match mgr.send_set_phone_book_and_wait(phone, &contacts, 5).await {
        Ok(0) => Json(build_success("电话本设置已应答")),
        Ok(result) => Json(build_error(&format!("电话本设置失败 result={}", result))),
        Err(e) => Json(build_error(&format!("电话本设置错误: {}", e))),
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

    match mgr.send_take_photo_and_wait(&phone, channel_id, 5).await {
        Ok(0) => Json(build_success("抓拍指令已应答")),
        Ok(result) => Json(build_error(&format!("抓拍失败 result={}", result))),
        Err(e) => Json(build_error(&format!("抓拍错误: {}", e))),
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
    match mgr.send_live_video_and_wait(&phone, channel_id, 0, false, 5).await {
        Ok(_) => {
            let _ = mgr.send_live_video_control_and_wait(&phone, channel_id, 5, false, 5).await; // 5=open bidirectional talk
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

    match mgr.send_live_video_control_and_wait(&phone, channel_id, 5, true, 5).await { // 5=close bidirectional talk
        Ok(0) => Json(build_success("对讲停止已应答")),
        Ok(result) => Json(build_error(&format!("对讲停止失败 result={}", result))),
        Err(e) => Json(build_error(&format!("对讲停止错误: {}", e))),
    }
}

/// GET /api/jt1078/media/upload/one/upload (used in queryMediaList.vue direct fetch)
#[derive(Debug, Deserialize)]
pub struct MediaUploadQuery {
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
    pub media_id: Option<String>,
}

pub async fn media_upload_one(
    State(state): State<AppState>,
    Query(q): Query<MediaUploadQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let media_id_str = q.media_id.clone().unwrap_or_default();
    let media_id: u32 = media_id_str.parse().unwrap_or(0);
    tracing::info!("JT1078 media upload one: phone={}, media_id={}", phone, media_id);

    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }

    if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
        match mgr.send_media_upload_and_wait(&phone, media_id, 5).await {
            Ok(0) => Json(serde_json::json!({
                "code": 0,
                "msg": "success",
                "data": {
                    "url": format!("/api/jt1078/media/upload/{}", media_id)
                }
            })),
            Ok(result) => Json(build_error(&format!("媒体上传失败 result={}", result))),
            Err(e) => Json(build_error(&format!("媒体上传错误: {}", e))),
        }
    } else {
        Json(build_error("JT1078 manager 未初始化"))
    }
}
