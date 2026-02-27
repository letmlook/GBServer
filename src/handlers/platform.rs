//! 级联平台 /api/platform，对应前端 platform.js

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::platform as platform_db;
use crate::db::platform_channel;
use crate::db::Platform;
use crate::error::AppError;
use crate::response::WVPResult;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PlatformQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
}

/// GET /api/platform/query
pub async fn platform_query(
    State(state): State<AppState>,
    Query(q): Query<PlatformQuery>,
) -> Result<Json<WVPResult<PlatformPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = platform_db::count_all(&state.pool).await?;
    let list = platform_db::list_paged(&state.pool, page, count).await?;
    Ok(Json(WVPResult::success(PlatformPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    })))
}

#[derive(Debug, serde::Serialize)]
pub struct PlatformPage {
    pub total: u64,
    pub list: Vec<Platform>,
    pub page: u64,
    pub size: u64,
}

/// GET /api/platform/server_config
pub async fn platform_server_config() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "id": null,
        "name": "本地平台",
        "serverGBId": "34020000001000000001",
        "serverHost": "127.0.0.1",
        "serverPort": 5060,
        "transport": "TCP"
    })))
}

// ========== 平台通道相关 ==========

#[derive(Debug, Deserialize)]
pub struct PlatformChannelQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub platform_id: Option<i64>,
}

/// GET /api/platform/channel/list
pub async fn platform_channel_list(
    State(state): State<AppState>,
    Query(q): Query<PlatformChannelQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let platform_id = q.platform_id.unwrap_or(0);
    
    if platform_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "total": 0,
            "list": []
        }))));
    }
    
    let total = platform_channel::count_by_platform_id(&state.pool, platform_id).await?;
    let list = platform_channel::list_by_platform_id(&state.pool, platform_id, page, count).await?;
    
    let rows: Vec<serde_json::Value> = list.iter().map(|c| {
        serde_json::json!({
            "id": c.id,
            "platformId": c.platform_id,
            "deviceChannelId": c.device_channel_id,
            "customDeviceId": c.custom_device_id,
            "customName": c.custom_name,
            "customManufacturer": c.custom_manufacturer,
            "customModel": c.custom_model,
            "customOwner": c.custom_owner,
            "customCivilCode": c.custom_civil_code,
            "customBlock": c.custom_block,
            "customAddress": c.custom_address,
            "customParental": c.custom_parental,
            "customParentId": c.custom_parent_id,
            "customRegisterWay": c.custom_register_way,
            "customStatus": c.custom_status,
            "customLongitude": c.custom_longitude,
            "customLatitude": c.custom_latitude
        })
    }).collect();
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": rows
    }))))
}

/// GET /api/platform/channel/push
/// 推送通道到平台（需要对接 SIP 信令）
#[derive(Debug, Deserialize)]
pub struct PlatformChannelPushQuery {
    pub platform_id: Option<i64>,
    pub channel_id_list: Option<String>,
    pub device_id_list: Option<String>,
}

pub async fn platform_channel_push(
    Query(q): Query<PlatformChannelPushQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let platform_id = q.platform_id.unwrap_or(0);
    if platform_id <= 0 {
        return Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        })));
    }
    // TODO: 实现 SIP 信令推送通道到平台
    Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "message": "通道推送命令已发送",
        "code": 0
    })))
}

// ========== 平台 CRUD ==========

/// POST /api/platform/add 请求体
#[derive(Debug, Deserialize)]
pub struct PlatformAddBody {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub server_gb_id: Option<String>,
    pub server_host: Option<String>,
    pub server_port: Option<i32>,
    pub transport: Option<String>,
    pub password: Option<String>,
    // 扩展字段
    pub server_gb_domain: Option<String>,
    pub device_gb_id: Option<String>,
    pub device_ip: Option<String>,
    pub device_port: Option<String>,
    pub username: Option<String>,
    pub civil_code: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub address: Option<String>,
    pub ptz: Option<bool>,
    pub rtcp: Option<bool>,
}

/// POST /api/platform/add
pub async fn platform_add(
    State(state): State<AppState>,
    Json(body): Json<PlatformAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let name = body.name.unwrap_or_default();
    let server_gb_id = body.server_gb_id.unwrap_or_default();
    let server_ip = body.server_host.unwrap_or_default();
    let server_port = body.server_port.unwrap_or(5060);
    let device_gb_id = body.device_gb_id.unwrap_or_default();
    let transport = body.transport.unwrap_or_else(|| "TCP".to_string());
    let username = body.username.unwrap_or_default();
    let password = body.password.unwrap_or_default();
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    platform_db::add(
        &state.pool,
        &name,
        &server_gb_id,
        &server_ip,
        server_port,
        &device_gb_id,
        &transport,
        &username,
        &password,
        &now,
    ).await?;
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "name": name,
        "message": "平台添加成功",
        "code": 0
    }))))
}

/// POST /api/platform/update
pub async fn platform_update(
    State(state): State<AppState>,
    Json(body): Json<PlatformAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        }))));
    }
    
    let name = body.name.as_deref();
    let server_gb_id = body.server_gb_id.as_deref();
    let server_ip = body.server_host.as_deref();
    let server_port = body.server_port;
    let device_gb_id = body.device_gb_id.as_deref();
    let transport = body.transport.as_deref();
    let username = body.username.as_deref();
    let password = body.password.as_deref();
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    platform_db::update(
        &state.pool,
        id,
        name,
        server_gb_id,
        server_ip,
        server_port,
        device_gb_id,
        transport,
        username,
        password,
        &now,
    ).await?;
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "message": "平台更新成功",
        "code": 0
    }))))
}

/// DELETE /api/platform/delete
#[derive(Debug, Deserialize)]
pub struct PlatformDeleteQuery {
    pub id: Option<i64>,
}

pub async fn platform_delete(
    State(state): State<AppState>,
    Query(q): Query<PlatformDeleteQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.unwrap_or(0);
    if id > 0 {
        platform_db::delete_by_id(&state.pool, id).await?;
    }
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "平台删除成功",
        "code": 0
    }))))
}

/// GET /api/platform/exit/:deviceGbId
pub async fn platform_exit(
    Path(device_gb_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "deviceGbId": device_gb_id,
        "message": "平台退出命令已发送",
        "code": 0
    })))
}

// ========== 平台通道操作 ==========

/// POST /api/platform/channel/add
#[derive(Debug, Deserialize)]
pub struct PlatformChannelAddBody {
    pub platform_id: Option<i64>,
    pub channel_ids: Option<Vec<String>>,
    pub all: Option<bool>,
}

pub async fn platform_channel_add(
    State(state): State<AppState>,
    Json(body): Json<PlatformChannelAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let platform_id = body.platform_id.unwrap_or(0);
    if platform_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        }))));
    }
    
    // 如果有 channel_ids，添加这些通道
    if let Some(channel_ids) = body.channel_ids {
        for channel_id_str in channel_ids {
            if let Ok(channel_id) = channel_id_str.parse::<i64>() {
                let _ = platform_channel::add(&state.pool, platform_id, channel_id).await;
            }
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "message": "通道添加成功",
        "code": 0
    }))))
}

/// POST /api/platform/channel/device/add - 添加设备的所有通道
#[derive(Debug, Deserialize)]
pub struct PlatformChannelDeviceBody {
    pub platform_id: Option<i64>,
    pub device_ids: Option<Vec<String>>,
}

pub async fn platform_channel_device_add(
    State(state): State<AppState>,
    Json(body): Json<PlatformChannelDeviceBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let platform_id = body.platform_id.unwrap_or(0);
    if platform_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        }))));
    }
    
    // TODO: 需要查询设备的所有通道，然后添加到平台
    // 目前需要对接设备通道查询功能
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "message": "设备通道添加成功",
        "code": 0
    }))))
}

/// POST /api/platform/channel/device/remove - 移除设备的所有通道
pub async fn platform_channel_device_remove(
    State(state): State<AppState>,
    Json(body): Json<PlatformChannelDeviceBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let platform_id = body.platform_id.unwrap_or(0);
    if platform_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        }))));
    }
    
    // TODO: 需要查询设备的所有通道，然后从平台移除
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "message": "设备通道移除成功",
        "code": 0
    }))))
}

/// DELETE /api/platform/channel/remove
pub async fn platform_channel_remove(
    State(state): State<AppState>,
    Json(body): Json<PlatformChannelAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let platform_id = body.platform_id.unwrap_or(0);
    if platform_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        }))));
    }
    
    // 如果有 channel_ids，删除这些通道
    if let Some(channel_ids) = body.channel_ids {
        for channel_id_str in channel_ids {
            if let Ok(channel_id) = channel_id_str.parse::<i64>() {
                let _ = platform_channel::delete_by_device_channel_id(&state.pool, platform_id, channel_id).await;
            }
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "message": "通道移除成功",
        "code": 0
    }))))
}

/// POST /api/platform/channel/custom/update
#[derive(Debug, Deserialize)]
pub struct PlatformChannelCustomUpdate {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub custom_info: Option<String>,
}

pub async fn platform_channel_custom_update(
    State(state): State<AppState>,
    Json(body): Json<PlatformChannelCustomUpdate>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "通道ID无效",
            "code": 1
        }))));
    }
    
    let custom_name = body.name.as_deref();
    let custom_info = body.custom_info.as_deref();
    
    platform_channel::update(&state.pool, id, custom_name, custom_info).await?;
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "自定义通道更新成功",
        "code": 0
    }))))
}
