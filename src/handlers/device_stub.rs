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
pub async fn sync_status(State(_state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
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
pub async fn device_sync(Path(_device_id): Path<String>) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

/// POST /api/device/query/transport/:device_id/:stream_mode
pub async fn device_transport(
    Path((_device_id, _stream_mode)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

/// GET /api/device/control/guard
pub async fn device_guard() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

/// GET /api/device/query/subscribe/catalog
pub async fn subscribe_catalog() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

/// GET /api/device/query/subscribe/mobile-position
pub async fn subscribe_mobile_position() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

/// GET /api/device/config/query/:device_id/BasicParam
pub async fn config_basic_param(
    Path(_device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
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
pub async fn query_streams() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "total": 0, "list": [] })))
}

/// GET /api/device/control/record
pub async fn control_record() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
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
pub async fn channel_audio() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

/// POST /api/device/query/channel/stream/identification/update/
pub async fn channel_stream_identification_update() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
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
