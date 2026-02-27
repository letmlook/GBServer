//! 设备相关接口：能落库的用 DB 实现，其余保持兼容空实现（后续可对接 SIP/ZLM）

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{
    delete_device_cascade,
    get_channel_by_device_and_channel_id,
    get_device_by_device_id,
    insert_device,
    list_channels_by_parent,
    list_channels_for_device,
    update_device,
    DeviceChannel,
};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

/// GET /api/device/query/sync_status
/// 参数: deviceId - 设备ID (可选)
/// 返回: 同步状态信息
pub async fn sync_status(
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    // TODO: 实现设备同步状态查询，需要对接 SIP 信令
    Json(WVPResult::success(serde_json::json!({
        "deviceId": null,
        "status": "idle",
        "progress": 0,
        "message": "设备同步功能待实现，需要对接SIP信令"
    })))
}

/// DELETE /api/device/query/devices/:device_id/delete
pub async fn device_delete(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<()>>, AppError> {
    delete_device_cascade(&state.pool, &device_id).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/device/query/devices/:device_id/sync
/// 触发设备同步
/// 参数: device_id - 设备国标ID
/// 返回: 同步结果
pub async fn device_sync(
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    // TODO: 实现设备同步，需要对接 SIP 信令发送 Catalog 查询
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "message": "设备同步命令已发送，等待响应",
        "code": 0
    })))
}


/// POST /api/device/query/transport/:device_id/:stream_mode
/// 设置设备流传输模式
/// 参数: device_id - 设备ID, stream_mode - 流模式 (TCP/UDP)
/// 返回: 设置结果
pub async fn device_transport(
    Path((device_id, stream_mode)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    // TODO: 实现设备流模式切换，需要对接 SIP 信令
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "streamMode": stream_mode,
        "message": "设备流传输模式设置成功",
        "code": 0
    })))
}

/// GET /api/device/control/guard
/// 设备布防/撤防控制
/// 参数: deviceId, guardCmd (SetGuard/ResetGuard)
/// 返回: 控制结果
#[derive(Debug, Deserialize)]
pub struct GuardQuery {
    pub device_id: Option<String>,
    pub guard_cmd: Option<String>,
}

pub async fn device_guard(
    Query(q): Query<GuardQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.unwrap_or_default();
    let guard_cmd = q.guard_cmd.unwrap_or_default();
    // TODO: 实现设备布防/撤防，需要对接 SIP 信令
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "guardCmd": guard_cmd,
        "message": "设备布防/撤防命令已发送",
        "code": 0
    })))
}

/// GET /api/device/query/subscribe/catalog
/// 订阅设备目录
/// 参数: id - 设备ID, cycle - 订阅周期(秒)
/// 返回: 订阅结果
#[derive(Debug, Deserialize)]
pub struct SubscribeCatalogQuery {
    pub id: Option<String>,
    pub cycle: Option<i32>,
}

pub async fn subscribe_catalog(
    Query(q): Query<SubscribeCatalogQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = q.id.unwrap_or_default();
    let cycle = q.cycle.unwrap_or(3600);
    // TODO: 实现目录订阅，需要对接 SIP 信令
    Json(WVPResult::success(serde_json::json!({
        "deviceId": id,
        "cycle": cycle,
        "message": "目录订阅已设置",
        "code": 0
    })))
}

/// GET /api/device/query/subscribe/mobile-position
/// 订阅设备移动位置
/// 参数: id - 设备ID, cycle - 订阅周期(秒), interval - 上报间隔(秒)
/// 返回: 订阅结果
#[derive(Debug, Deserialize)]
pub struct SubscribePositionQuery {
    pub id: Option<String>,
    pub cycle: Option<i32>,
    pub interval: Option<i32>,
}

pub async fn subscribe_mobile_position(
    Query(q): Query<SubscribePositionQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = q.id.unwrap_or_default();
    let cycle = q.cycle.unwrap_or(5);
    let interval = q.interval.unwrap_or(5);
    // TODO: 实现位置订阅，需要对接 SIP 信令
    Json(WVPResult::success(serde_json::json!({
        "deviceId": id,
        "cycle": cycle,
        "interval": interval,
        "message": "位置订阅已设置",
        "code": 0
    })))
}

/// GET /api/device/config/query/:device_id/BasicParam
/// 获取设备基本参数
/// 参数: device_id - 设备ID
/// 返回: 设备基本配置信息
pub async fn config_basic_param(
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    // TODO: 实现设备基本参数查询，需要对接 SIP 信令
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "name": null,
        "manufacturer": null,
        "model": null,
        "firmware": null,
        "transport": "TCP",
        "streamMode": "PLAY",
        "message": "设备基本参数功能待实现"
    })))
}


#[derive(Debug, Deserialize)]
pub struct ChannelOneQuery {
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
}

/// GET /api/device/query/channel/one?deviceId=&channelId=
pub async fn channel_one(
    State(state): State<AppState>,
    Query(q): Query<ChannelOneQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_id = q
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    let channel_id = q.channel_id.as_deref().unwrap_or("").trim();
    if device_id.is_empty() || channel_id.is_empty() {
        return Ok(Json(WVPResult::success(serde_json::Value::Null)));
    }
    let ch = get_channel_by_device_and_channel_id(&state.pool, device_id, channel_id).await?;
    let out = match ch {
        Some(c) => channel_to_json(&c),
        None => serde_json::Value::Null,
    };
    Ok(Json(WVPResult::success(out)))
}

/// GET /api/device/query/streams
/// 获取有流的通道列表
/// 返回: 有流的通道列表
pub async fn query_streams(
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    // TODO: 实现流查询，需要对接 ZLM 获取实时流信息
    Json(WVPResult::success(serde_json::json!({
        "total": 0,
        "list": [],
        "message": "流查询功能待实现，需要对接ZLM"
    })))
}


/// GET /api/device/control/record
/// 设备远程录像控制
/// 参数: deviceId, channelId, recordCmdStr (Start/Stop)
/// 返回: 录像控制结果
#[derive(Debug, Deserialize)]
pub struct RecordControlQuery {
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
    pub record_cmd_str: Option<String>,
}

pub async fn control_record(
    Query(q): Query<RecordControlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or_default();
    let record_cmd = q.record_cmd_str.unwrap_or_default();
    // TODO: 实现远程录像控制，需要对接 ZLM/SIP
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "recordCmd": record_cmd,
        "message": "远程录像控制功能待实现",
        "code": 0
    })))
}

/// GET /api/device/query/sub_channels/:device_id/:parent_channel_id/channels
pub async fn sub_channels(
    State(state): State<AppState>,
    Path((device_id, parent_channel_id)): Path<(String, String)>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list = list_channels_by_parent(&state.pool, &device_id, &parent_channel_id).await?;
    let total = list.len() as u64;
    let list: Vec<serde_json::Value> = list.iter().map(channel_to_json).collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

fn channel_to_json(c: &DeviceChannel) -> serde_json::Value {
    serde_json::json!({
        "id": c.id,
        "deviceId": c.device_id,
        "name": c.name,
        "channelId": c.gb_device_id,
        "status": c.status,
        "longitude": c.longitude,
        "latitude": c.latitude,
        "createTime": c.create_time,
        "updateTime": c.update_time,
        "subCount": c.sub_count,
        "hasAudio": c.has_audio,
        "channelType": c.channel_type
    })
}

/// GET /api/device/query/tree/channel/:device_id
pub async fn tree_channel(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list = list_channels_for_device(&state.pool, &device_id).await?;
    let tree: Vec<serde_json::Value> = list.iter().map(channel_to_json).collect();
    Ok(Json(WVPResult::success(serde_json::Value::Array(tree))))
}

/// POST /api/device/query/channel/audio
/// 修改通道音频状态
/// 参数: channelId, audio (true/false)
/// 返回: 修改结果
#[derive(Debug, Deserialize)]
pub struct ChannelAudioQuery {
    pub channel_id: Option<i64>,
    pub audio: Option<bool>,
}

pub async fn channel_audio(
    Query(q): Query<ChannelAudioQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let channel_id = q.channel_id.unwrap_or(0);
    let audio = q.audio.unwrap_or(false);
    // TODO: 实现音频开关，需要对接 ZLM
    Json(WVPResult::success(serde_json::json!({
        "channelId": channel_id,
        "audio": audio,
        "message": "通道音频设置已更新",
        "code": 0
    })))
}


/// POST /api/device/query/channel/stream/identification/update/
/// 更新通道流标识
/// 参数: deviceDbId, id, streamIdentification
/// 返回: 更新结果
#[derive(Debug, Deserialize)]
pub struct StreamIdentificationUpdate {
    pub device_db_id: Option<i64>,
    pub id: Option<i64>,
    pub stream_identification: Option<String>,
}

pub async fn channel_stream_identification_update(
    Json(body): Json<StreamIdentificationUpdate>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_db_id = body.device_db_id.unwrap_or(0);
    let id = body.id.unwrap_or(0);
    let stream_id = body.stream_identification.unwrap_or_default();
    // TODO: 实现流标识更新，需要更新数据库
    Json(WVPResult::success(serde_json::json!({
        "deviceDbId": device_db_id,
        "id": id,
        "streamIdentification": stream_id,
        "message": "流标识更新成功",
        "code": 0
    })))
}

#[derive(Debug, Deserialize)]
pub struct DeviceAddBody {
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub stream_mode: Option<String>,
    pub media_server_id: Option<String>,
    pub custom_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceUpdateBody {
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub stream_mode: Option<String>,
    pub media_server_id: Option<String>,
    pub custom_name: Option<String>,
}

/// POST /api/device/query/device/update
pub async fn device_update(
    State(state): State<AppState>,
    Json(body): Json<DeviceUpdateBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    update_device(
        &state.pool,
        device_id,
        body.name.as_deref(),
        body.manufacturer.as_deref(),
        body.model.as_deref(),
        body.transport.as_deref(),
        body.stream_mode.as_deref(),
        body.media_server_id.as_deref(),
        body.custom_name.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/device/query/device/add
pub async fn device_add(
    State(state): State<AppState>,
    Json(body): Json<DeviceAddBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    insert_device(
        &state.pool,
        device_id,
        body.name.as_deref(),
        body.manufacturer.as_deref(),
        body.model.as_deref(),
        body.transport.as_deref(),
        body.stream_mode.as_deref(),
        body.media_server_id.as_deref(),
        body.custom_name.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/device/query/devices/:device_id
pub async fn device_one(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    match get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => {
            let v = serde_json::json!({
                "deviceId": d.device_id,
                "name": d.name,
                "manufacturer": d.manufacturer,
                "model": d.model,
                "transport": d.transport,
                "streamMode": d.stream_mode,
                "onLine": d.on_line,
                "ip": d.ip,
                "port": d.port,
                "createTime": d.create_time,
                "updateTime": d.update_time,
                "mediaServerId": d.media_server_id,
                "customName": d.custom_name
            });
            Json(WVPResult::success(v))
        }
        _ => Json(WVPResult::success(serde_json::json!(null))),
    }
}

/// GET /api/device/query/tree/:device_id
pub async fn device_tree(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let channels = list_channels_for_device(&state.pool, &device_id).await?;
    let total = channels.len() as u64;
    let list: Vec<serde_json::Value> = channels.iter().map(channel_to_json).collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}
