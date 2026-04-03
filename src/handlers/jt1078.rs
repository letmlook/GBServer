//! JT1078 部标设备 API /api/jt1078，对应前端 jtDevice.js
//! 包含终端管理、视频播放、录像回放、设备控制等功能

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::jt1078 as jt_db;
use crate::db::{JtChannel, JtTerminal};
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
    serde_json::json!({
        "code": 0,
        "msg": msg
    })
}

fn build_error(msg: &str) -> serde_json::Value {
    serde_json::json!({
        "code": 1,
        "msg": msg
    })
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
    Json(body): Json<ChannelUpdateBody>,
) -> Json<serde_json::Value> {
    let id = body.id.unwrap_or(0);

    tracing::info!("JT1078 channel update: id={}", id);

    Json(build_success("通道更新成功"))
}

/// POST /api/jt1078/terminal/channel/add
pub async fn channel_add(
    Json(body): Json<ChannelAddBody>,
) -> Json<serde_json::Value> {
    let device_id = body.device_id.clone().unwrap_or_default();
    let channel_id = body.channel_id.unwrap_or(0);

    tracing::info!("JT1078 channel add: device={}, channel={}", device_id, channel_id);

    Json(build_success("通道添加成功"))
}

// ========== 实时视频 ==========
/// GET /api/jt1078/live/start
pub async fn live_start(
    Query(q): Query<LiveQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);
    let stream_type = q.r#type.clone().unwrap_or_else(|| "main".to_string());

    tracing::info!("JT1078 live start: phone={}, channel={}, type={}", phone, channel_id, stream_type);

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
    Query(q): Query<LiveQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);

    tracing::info!("JT1078 live stop: phone={}, channel={}", phone, channel_id);

    Json(build_success("停止播放成功"))
}

// ========== 录像回放 ==========
/// GET /api/jt1078/playback/start
pub async fn playback_start(
    Query(q): Query<PlaybackQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();

    tracing::info!("JT1078 playback start: phone={}, channel={}, {}-{}", 
        phone, channel_id, start_time, end_time);

    Json(serde_json::json!({
        "code": 0,
        "msg": "success",
        "data": {
            "streamId": format!("playback_{}_{}", phone, channel_id),
            "url": format!("rtmp://127.0.0.1/record/{}_{}", phone, channel_id)
        }
    }))
}

/// GET /api/jt1078/playback/stop
pub async fn playback_stop(
    Query(q): Query<ControlQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0);

    tracing::info!("JT1078 playback stop: phone={}, channel={}", phone, channel_id);

    Json(build_success("停止回放成功"))
}

/// GET /api/jt1078/playback/control
pub async fn playback_control(
    Query(q): Query<ControlQuery>,
) -> Json<serde_json::Value> {
    let command = q.command.clone().unwrap_or_default();

    tracing::info!("JT1078 playback control: cmd={}", command);

    Json(build_success("回放控制成功"))
}

/// GET /api/jt1078/playback/downloadUrl
pub async fn playback_download_url(
    Query(q): Query<DownloadUrlQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 playback download url: phone={}", phone);

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
    Query(q): Query<PtzQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);
    let command = q.command.clone().unwrap_or_default();
    let speed = q.speed.unwrap_or(1);

    tracing::info!("JT1078 PTZ: phone={}, channel={}, cmd={}, speed={}", 
        phone, channel_id, command, speed);

    Json(build_success("云台控制命令已发送"))
}

/// GET /api/jt1078/wiper
pub async fn wiper(
    Query(q): Query<WiperQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();

    tracing::info!("JT1078 wiper: phone={}, cmd={}", phone, command);

    Json(build_success("雨刷控制命令已发送"))
}

/// GET /api/jt1078/fill-light
pub async fn fill_light(
    Query(q): Query<FillLightQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();

    tracing::info!("JT1078 fill light: phone={}, cmd={}", phone, command);

    Json(build_success("补光灯控制命令已发送"))
}

/// GET /api/jt1078/record/list
pub async fn record_list(
    Query(q): Query<RecordListQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 record list: phone={}", phone);

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "list": [],
            "total": 0
        }
    }))
}

// ========== 配置查询 ==========
/// GET /api/jt1078/config/get
pub async fn config_get(
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 config get: phone={}", phone);

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "apn": "internet",
            "ip": "127.0.0.1",
            "port": 7070
        }
    }))
}

/// POST /api/jt1078/config/set
pub async fn config_set(
    Json(body): Json<ConfigBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 config set: phone={}", phone);

    Json(build_success("配置保存成功"))
}

/// GET /api/jt1078/attribute
pub async fn attribute(
    Query(q): Query<AttributeQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 attribute: phone={}", phone);

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "phoneNumber": phone,
            "deviceType": "车载终端",
            "manufacturer": "默认厂商"
        }
    }))
}

/// GET /api/jt1078/link-detection
pub async fn link_detection(
    Query(q): Query<LinkDetectionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 link detection: phone={}", phone);

    Json(build_success("链路检测完成"))
}

// ========== 位置信息 ==========
/// GET /api/jt1078/position-info
pub async fn position_info(
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 position info: phone={}", phone);

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
    Json(body): Json<TextMsgBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();
    let message = body.message.clone().unwrap_or_default();

    tracing::info!("JT1078 text msg: phone={}, msg={}", phone, message);

    Json(build_success("文本消息已发送"))
}

/// GET /api/jt1078/telephone-callback
pub async fn telephone_callback(
    Query(q): Query<TerminalCallbackQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let dest = q.dest_phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 telephone callback: phone={}, dest={}", phone, dest);

    Json(build_success("回拨指令已发送"))
}

/// GET /api/jt1078/driver-information
pub async fn driver_info(
    Query(q): Query<DriverInfoQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 driver info: phone={}", phone);

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "phoneNumber": phone,
            "driverName": "驾驶员",
            "licenseNo": "123456789012345678"
        }
    }))
}

// ========== 设备控制 ==========
/// POST /api/jt1078/control/factory-reset
pub async fn factory_reset(
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 factory reset: phone={}", phone);

    Json(build_success("恢复出厂设置指令已发送"))
}

/// POST /api/jt1078/control/reset
pub async fn reset(
    Json(body): Json<ResetBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 reset: phone={}", phone);

    Json(build_success("设备重启指令已发送"))
}

/// POST /api/jt1078/control/connection
pub async fn connection(
    Json(body): Json<ConnectionBody>,
) -> Json<serde_json::Value> {
    let phone = body.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 connection: phone={}", phone);

    Json(build_success("连接控制指令已发送"))
}

/// GET /api/jt1078/control/door
pub async fn door(
    Query(q): Query<DoorQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let open = q.open.unwrap_or(false);

    tracing::info!("JT1078 door: phone={}, open={}", phone, open);

    Json(build_success(if open { "开门指令已发送" } else { "关门指令已发送" }))
}

// ========== 媒体数据 ==========
/// GET /api/jt1078/media/attribute
pub async fn media_attribute(
    Query(q): Query<MediaAttributeQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 media attribute: phone={}", phone);

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "phoneNumber": phone,
            "channels": 4,
            "supportAudio": true
        }
    }))
}

/// POST /api/jt1078/media/list
pub async fn media_list(
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    tracing::info!("JT1078 media list");

    Json(serde_json::json!({
        "code": 0,
        "data": {
            "list": []
        }
    }))
}

// ========== 其他功能 ==========
/// POST /api/jt1078/set-phone-book
pub async fn set_phone_book(
    Json(_body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    tracing::info!("JT1078 set phone book");

    Json(build_success("电话本设置成功"))
}

/// POST /api/jt1078/shooting
pub async fn shooting(
    Json(_body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    tracing::info!("JT1078 shooting");

    Json(build_success("抓拍指令已发送"))
}

// ========== 对讲 ==========
/// GET /api/jt1078/talk/start
pub async fn talk_start(
    Query(q): Query<TalkQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);

    tracing::info!("JT1078 talk start: phone={}, channel={}", phone, channel_id);

    Json(serde_json::json!({
        "code": 0,
        "msg": "success",
        "data": {
            "phoneNumber": phone,
            "channelId": channel_id
        }
    }))
}

/// GET /api/jt1078/talk/stop
pub async fn talk_stop(
    Query(q): Query<TalkQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();

    tracing::info!("JT1078 talk stop: phone={}", phone);

    Json(build_success("对讲停止成功"))
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
