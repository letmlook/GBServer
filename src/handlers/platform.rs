//! 级联平台 /api/platform，对应前端 platform.js

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::platform as platform_db;
use crate::db::platform_channel;
use crate::db::{Platform, device as db_device};
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

/// POST /api/platform/channel/push
/// 推送通道到平台（需要对接 SIP 信令）
#[derive(Debug, Deserialize)]
pub struct PlatformChannelPushQuery {
    pub platform_id: Option<i64>,
    pub channel_id_list: Option<String>,
    pub device_id_list: Option<String>,
}

pub async fn platform_channel_push(
    State(state): State<AppState>,
    Query(q): Query<PlatformChannelPushQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let platform_id = q.platform_id.unwrap_or(0);
    if platform_id <= 0 {
        return Ok(Json(WVPResult::success(serde_json::json!({
            "message": "平台ID无效",
            "code": 1
        }))));
    }
    
    let platform = match platform_db::get_by_id(&state.pool, platform_id as i64).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return Ok(Json(WVPResult::success(serde_json::json!({
                "message": "平台不存在",
                "code": 1
            }))));
        }
        Err(e) => {
            tracing::error!("Failed to get platform: {}", e);
            return Ok(Json(WVPResult::error("Database error")));
        }
    };
    
    let server_gb_id = match platform.server_gb_id {
        Some(id) => id,
        None => {
            return Ok(Json(WVPResult::success(serde_json::json!({
                "message": "平台国标ID未设置",
                "code": 1
            }))));
        }
    };
    
    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => {
            return Ok(Json(WVPResult::success(serde_json::json!({
                "message": "SIP服务器未启动",
                "code": 1
            }))));
        }
    };
    
    let sip = sip_server.read().await;
    
    let mut pushed_count = 0;
    let mut errors = Vec::new();
    
    if let Some(channel_id_list) = &q.channel_id_list {
        for channel_id_str in channel_id_list.split(',') {
            let channel_id = channel_id_str.trim();
            if channel_id.is_empty() {
                continue;
            }
            
            match sip.send_platform_invite(&server_gb_id, channel_id, 0).await {
                Ok(_) => {
                    tracing::info!("Sent platform INVITE for channel {} to {}", channel_id, server_gb_id);
                    pushed_count += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to send platform INVITE: {}", e);
                    errors.push(format!("{}: {}", channel_id, e));
                }
            }
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "pushedCount": pushed_count,
        "errors": errors,
        "message": if errors.is_empty() { "通道推送成功" } else { "通道推送完成，部分失败" },
        "code": 0
    }))))
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
    State(state): State<AppState>,
    Path(device_gb_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    // Look up platform by device_gb_id, then unregister from cascade platform via SIP
    let maybe_platform = platform_db::get_by_device_gb_id(&state.pool, &device_gb_id).await;
    if let Ok(Some(p)) = maybe_platform {
        if let Some(ref server_gb_id) = p.server_gb_id {
            if let Some(ref sip_server) = state.sip_server {
                let sip = sip_server.read().await;
                // Send UNREGISTER to cascade platform
                let _ = sip.unregister_from_platform(server_gb_id).await;
            }
        }
    }
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
    if let Some(channel_ids) = body.channel_ids.clone() {
        for channel_id_str in channel_ids {
            if let Ok(channel_id) = channel_id_str.parse::<i64>() {
                let _ = platform_channel::add(&state.pool, platform_id, channel_id).await;
            }
        }
    }

    // 尝试向级联平台发送邀请，保持与WVP实现一致的行为
    if let Some(platform) = platform_db::get_by_id(&state.pool, platform_id).await? {
        if let Some(ref server_gb_id) = platform.server_gb_id {
            if let Some(ref sip_server) = state.sip_server {
                let sip = sip_server.read().await;
                if let Some(channel_ids) = body.channel_ids {
                    for channel_id_str in channel_ids {
                        if let Ok(channel_id) = channel_id_str.parse::<i64>() {
                            let _ = sip.send_platform_invite(server_gb_id, &channel_id.to_string(), 0).await;
                        }
                    }
                }
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
    
    let device_ids = body.device_ids.unwrap_or_default();
    let mut added_count = 0;
    
    for device_id in device_ids {
        let channels = db_device::list_channels_for_device(&state.pool, &device_id).await?;
        
        for channel in channels {
            if let Some(ref channel_id) = channel.gb_device_id {
                let channel_id_num: i64 = channel_id.parse().unwrap_or(0);
                if channel_id_num > 0 {
                    if platform_channel::add(&state.pool, platform_id, channel_id_num).await.is_ok() {
                        added_count += 1;
                        // cascade 通道添加后，发送级联信令
                        if let Some(ref sip_server) = state.sip_server {
                            let sip = sip_server.read().await;
                            if let Some(ref p) = platform_db::get_by_id(&state.pool, platform_id).await?.clone() {
                                if let Some(ref server_gb_id) = p.server_gb_id {
                                    let _ = sip.send_platform_invite(server_gb_id, &channel_id_num.to_string(), 0).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "addedCount": added_count,
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
    
    let device_ids = body.device_ids.unwrap_or_default();
    let mut removed_count = 0;
    
    for device_id in device_ids {
        let channels = db_device::list_channels_for_device(&state.pool, &device_id).await?;
        
        for channel in channels {
            if let Some(ref channel_id) = channel.gb_device_id {
                let channel_id_num: i64 = channel_id.parse().unwrap_or(0);
                if channel_id_num > 0 {
                    if platform_channel::delete_by_device_channel_id(&state.pool, platform_id, channel_id_num).await.is_ok() {
                        removed_count += 1;
                    }
                }
            }
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "removedCount": removed_count,
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
                // 同步级联平台：通知移除该通道
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.read().await;
                    if let Some(ref p) = platform_db::get_by_id(&state.pool, platform_id).await?.clone() {
                        if let Some(ref server_gb_id) = p.server_gb_id {
                            let _ = sip.send_platform_invite(server_gb_id, &channel_id.to_string(), 0).await;
                        }
                    }
                }
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
    // 发送级联信令告知通道信息更新（若该通道属于某级联平台）
    if let Ok(Some(ch)) = platform_channel::get_by_id(&state.pool, id).await {
        if let Some(platform_id) = ch.platform_id {
            if let Ok(Some(p)) = platform_db::get_by_id(&state.pool, platform_id).await {
                if let Some(ref server_gb_id) = p.server_gb_id {
                    if let Some(channel_id) = ch.device_channel_id {
                        if let Some(ref sip_server) = state.sip_server {
                            let sip = sip_server.read().await;
                            let _ = sip.send_platform_invite(server_gb_id, &channel_id.to_string(), 0).await;
                        }
                    }
                }
            }
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "自定义通道更新成功",
        "code": 0
    }))))
}

/// POST /api/platform/catalog/add (used in catalogEdit.vue, commonChannelEditDialog.vue)
#[derive(Debug, Deserialize)]
pub struct CatalogAddBody {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub civil_code: Option<String>,
    pub business_group: Option<String>,
    pub platform_id: Option<i64>,
}

pub async fn catalog_add(
    State(state): State<AppState>,
    Json(body): Json<CatalogAddBody>,
) -> Json<serde_json::Value> {
    let id = body.id.unwrap_or(0);
    let name = body.name.clone().unwrap_or_default();
    let parent = body.parent.clone().unwrap_or_default();
    let civil_code = body.civil_code.clone().unwrap_or_default();
    let business_group = body.business_group.clone().unwrap_or_default();
    let platform_id = body.platform_id.unwrap_or(0);

    tracing::info!("platform catalog add: id={}, name={:?}, parent={}, civil_code={}, platform_id={}", id, name, parent, civil_code, platform_id);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(feature = "mysql")]
    let _ = sqlx::query("INSERT INTO wvp_platform_catalog (name, parent, civil_code, business_group, platform_id, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(name).bind(parent).bind(&civil_code).bind(&business_group).bind(platform_id).bind(&now).bind(&now)
        .execute(&state.pool).await;
    #[cfg(feature = "postgres")]
    let _ = sqlx::query("INSERT INTO wvp_platform_catalog (name, parent, civil_code, business_group, platform_id, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6, $7)")
        .bind(name).bind(parent).bind(&civil_code).bind(&business_group).bind(platform_id).bind(&now).bind(&now)
        .execute(&state.pool).await;
    Json(serde_json::json!({ "code": 0, "msg": "目录添加成功" }))
}

/// POST /api/platform/catalog/edit (used in catalogEdit.vue, commonChannelEditDialog.vue)
#[derive(Debug, Deserialize)]
pub struct CatalogAddBodyEdit {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub civil_code: Option<String>,
    pub business_group: Option<String>,
    pub platform_id: Option<i64>,
}

pub async fn catalog_edit(
    State(state): State<AppState>,
    Json(body): Json<CatalogAddBodyEdit>,
) -> Json<serde_json::Value> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        // Fallback to add if no id provided
        let add_body = CatalogAddBody {
            id: None,
            name: body.name.clone(),
            parent: body.parent.clone(),
            civil_code: body.civil_code.clone(),
            business_group: body.business_group.clone(),
            platform_id: body.platform_id,
        };
        // Reuse add path by delegating to insert logic via direct call
        let _ = catalog_add(State(state.clone()), Json(add_body)).await;
        return Json(serde_json::json!({ "code": 0, "msg": "目录编辑成功" }));
    }
    let name = body.name.clone().unwrap_or_default();
    let parent = body.parent.clone().unwrap_or_default();
    let civil_code = body.civil_code.clone().unwrap_or_default();
    let business_group = body.business_group.clone().unwrap_or_default();
    let platform_id = body.platform_id.unwrap_or(0);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(feature = "mysql")]
    let _ = sqlx::query("UPDATE wvp_platform_catalog SET name = COALESCE(?, name), parent = COALESCE(?, parent), civil_code = COALESCE(?, civil_code), business_group = COALESCE(?, business_group), platform_id = COALESCE(?, platform_id), update_time = ? WHERE id = ?")
        .bind(name).bind(&parent).bind(&civil_code).bind(&business_group).bind(platform_id).bind(&now).bind(id)
        .execute(&state.pool).await;
    #[cfg(feature = "postgres")]
    let _ = sqlx::query("UPDATE wvp_platform_catalog SET name = COALESCE($1, name), parent = COALESCE($2, parent), civil_code = COALESCE($3, civil_code), business_group = COALESCE($4, business_group), platform_id = COALESCE($5, platform_id), update_time = $6 WHERE id = $7")
        .bind(name).bind(parent).bind(civil_code).bind(business_group).bind(platform_id).bind(&now).bind(id)
        .execute(&state.pool).await;
    Json(serde_json::json!({ "code": 0, "msg": "目录编辑成功" }))
}
