//! 级联平台 /api/platform，对应前端 platform.js

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::Row;

use crate::db::platform as platform_db;
use crate::db::platform_channel;
use crate::db::{Platform, device as db_device};
use crate::error::AppError;
use crate::response::WVPResult;

use crate::AppState;

async fn update_platform_status(pool: &crate::db::Pool, id: i64, status: bool) -> Result<(), sqlx::Error> {
    #[cfg(feature = "postgres")]
    sqlx::query("UPDATE wvp_platform SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "mysql")]
    sqlx::query("UPDATE wvp_platform SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn sync_platform_registration(
    state: &AppState,
    platform: &Platform,
) -> Result<bool, AppError> {
    let enable = platform.enable.unwrap_or(false);
    let id = platform.id as i64;
    let Some(server_gb_id) = platform.server_gb_id.as_deref() else {
        update_platform_status(&state.pool, id, false).await?;
        return Ok(false);
    };
    let Some(ref sip_server) = state.sip_server else {
        update_platform_status(&state.pool, id, false).await?;
        return Ok(false);
    };

    let sip = sip_server.read().await;
    let result = if enable {
        sip.register_to_platform(server_gb_id).await
    } else {
        sip.unregister_from_platform(server_gb_id).await
    };

    match result {
        Ok(_) => {
            update_platform_status(&state.pool, id, enable).await?;
            if enable && platform.catalog_with_platform.unwrap_or_default() > 0 {
                let _ = sip.send_platform_catalog(server_gb_id).await;
            }
            Ok(enable)
        }
        Err(err) => {
            tracing::warn!("platform registration sync failed for {}: {}", server_gb_id, err);
            update_platform_status(&state.pool, id, false).await?;
            Ok(false)
        }
    }
}

async fn push_platform_channels(
    state: &AppState,
    platform: &Platform,
    channel_ids: &[String],
) -> Result<u32, AppError> {
    let Some(server_gb_id) = platform.server_gb_id.as_deref() else {
        return Ok(0);
    };
    let Some(ref sip_server) = state.sip_server else {
        return Ok(0);
    };

    let sip = sip_server.read().await;
    let mut pushed_count = 0;
    for channel_id in channel_ids {
        if channel_id.trim().is_empty() {
            continue;
        }
        if sip.send_platform_invite(server_gb_id, channel_id, 0).await.is_ok() {
            pushed_count += 1;
        }
    }
    Ok(pushed_count)
}

async fn refresh_platform_catalog(state: &AppState, platform_id: i64) -> Result<(), AppError> {
    let Some(platform) = platform_db::get_by_id(&state.pool, platform_id).await? else {
        return Ok(());
    };
    if !platform.enable.unwrap_or(false) || !platform.status.unwrap_or(false) {
        return Ok(());
    }
    let Some(server_gb_id) = platform.server_gb_id.as_deref() else {
        return Ok(());
    };
    let Some(ref sip_server) = state.sip_server else {
        return Ok(());
    };
    let sip = sip_server.read().await;
    let _ = sip.send_platform_catalog(server_gb_id).await;
    Ok(())
}

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
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let search = q.query.as_deref().unwrap_or("").trim().to_string();
    let like = format!("%{}%", search);
    let offset = (page.saturating_sub(1) * count) as i64;

    #[cfg(feature = "postgres")]
    let total = if search.is_empty() {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_platform")
            .fetch_one(&state.pool)
            .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM wvp_platform WHERE name LIKE $1 OR server_gb_id LIKE $1 OR device_gb_id LIKE $1",
        )
        .bind(&like)
        .fetch_one(&state.pool)
        .await?
    };
    #[cfg(feature = "mysql")]
    let total = if search.is_empty() {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_platform")
            .fetch_one(&state.pool)
            .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM wvp_platform WHERE name LIKE ? OR server_gb_id LIKE ? OR device_gb_id LIKE ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&state.pool)
        .await?
    };

    #[cfg(feature = "postgres")]
    let raw_list = if search.is_empty() {
        sqlx::query_as::<_, Platform>("SELECT * FROM wvp_platform ORDER BY id DESC LIMIT $1 OFFSET $2")
            .bind(count as i64)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Platform>(
            "SELECT * FROM wvp_platform WHERE name LIKE $1 OR server_gb_id LIKE $1 OR device_gb_id LIKE $1 ORDER BY id DESC LIMIT $2 OFFSET $3",
        )
        .bind(&like)
        .bind(count as i64)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?
    };
    #[cfg(feature = "mysql")]
    let raw_list = if search.is_empty() {
        sqlx::query_as::<_, Platform>("SELECT * FROM wvp_platform ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(count as i64)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Platform>(
            "SELECT * FROM wvp_platform WHERE name LIKE ? OR server_gb_id LIKE ? OR device_gb_id LIKE ? ORDER BY id DESC LIMIT ? OFFSET ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(count as i64)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?
    };

    let mut list = Vec::with_capacity(raw_list.len());
    for item in raw_list {
        let channel_count = platform_channel::count_by_platform_id(&state.pool, item.id as i64)
            .await
            .unwrap_or(0);
        list.push(serde_json::json!({
            "id": item.id,
            "enable": item.enable.unwrap_or(false),
            "name": item.name,
            "serverGBId": item.server_gb_id,
            "serverGBDomain": item.server_gb_domain,
            "serverIp": item.server_ip,
            "serverPort": item.server_port,
            "deviceGBId": item.device_gb_id,
            "deviceIp": item.device_ip,
            "devicePort": item.device_port,
            "username": item.username,
            "password": item.password,
            "expires": item.expires,
            "keepTimeout": item.keep_timeout,
            "transport": item.transport,
            "civilCode": item.civil_code,
            "manufacturer": item.manufacturer,
            "model": item.model,
            "address": item.address,
            "characterSet": item.character_set,
            "ptz": item.ptz.unwrap_or(false),
            "rtcp": item.rtcp.unwrap_or(false),
            "status": item.status.unwrap_or(false),
            "catalogGroup": item.catalog_group,
            "registerWay": item.register_way,
            "secrecy": item.secrecy,
            "createTime": item.create_time,
            "updateTime": item.update_time,
            "asMessageChannel": item.as_message_channel.unwrap_or(false),
            "catalogWithPlatform": item.catalog_with_platform.unwrap_or(0),
            "catalogWithGroup": item.catalog_with_group.unwrap_or(0),
            "catalogWithRegion": item.catalog_with_region.unwrap_or(0),
            "autoPushChannel": item.auto_push_channel.unwrap_or(false),
            "sendStreamIp": item.send_stream_ip,
            "serverId": item.server_id,
            "channelCount": channel_count,
            "alarmSubscribe": item.as_message_channel.unwrap_or(false) && item.enable.unwrap_or(false),
            "catalogSubscribe": item.enable.unwrap_or(false)
                && (item.catalog_with_platform.unwrap_or(0) > 0
                    || item.catalog_with_group.unwrap_or(0) > 0
                    || item.catalog_with_region.unwrap_or(0) > 0),
            "mobilePositionSubscribe": item.enable.unwrap_or(false) && item.status.unwrap_or(false)
        }));
    }
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total as u64,
        "list": list,
        "page": page as u64,
        "size": count as u64
    }))))
}

/// GET /api/platform/server_config
pub async fn platform_server_config(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let sip = state.config.sip.as_ref();
    let device_ip = sip
        .map(|cfg| cfg.ip.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    Json(WVPResult::success(serde_json::json!({
        "id": null,
        "name": "本地平台",
        "serverGBId": sip.as_ref().map(|cfg| cfg.realm.clone()).unwrap_or_else(|| "34020000002000000001".to_string()),
        "serverGBDomain": sip.as_ref().map(|cfg| cfg.realm.clone()).unwrap_or_else(|| "3402000000".to_string()),
        "serverHost": device_ip,
        "serverIp": sip.as_ref().map(|cfg| cfg.ip.clone()).unwrap_or_else(|| "127.0.0.1".to_string()),
        "serverPort": sip.as_ref().map(|cfg| cfg.port as i32).unwrap_or(5060),
        "deviceIp": sip.as_ref().map(|cfg| cfg.ip.clone()).unwrap_or_else(|| "127.0.0.1".to_string()),
        "devicePort": sip.as_ref().map(|cfg| cfg.port.to_string()).unwrap_or_else(|| "5060".to_string()),
        "username": sip.as_ref().map(|cfg| cfg.device_id.clone()).unwrap_or_else(|| "34020000001320000001".to_string()),
        "password": "",
        "transport": "UDP",
        "sendStreamIp": sip.as_ref().map(|cfg| cfg.ip.clone()).unwrap_or_else(|| "127.0.0.1".to_string())
    })))
}

// ========== 平台通道相关 ==========

#[derive(Debug, Deserialize)]
pub struct PlatformChannelQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    #[serde(alias = "platformId")]
    pub platform_id: Option<i64>,
    pub query: Option<String>,
    pub online: Option<String>,
    #[serde(alias = "channelType")]
    pub channel_type: Option<String>,
    #[serde(alias = "hasShare")]
    pub has_share: Option<String>,
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
    
    let offset = (page.saturating_sub(1) * count) as i64;
    let search = q.query.as_deref().unwrap_or("").trim().to_string();
    let like = format!("%{}%", search);
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let channel_type = q.channel_type.as_deref().and_then(|v| v.parse::<i32>().ok());
    let has_share = q.has_share.as_deref().unwrap_or("false");

    #[cfg(feature = "postgres")]
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.channel_type, d.manufacturer,
               pc.id as platform_channel_id, pc.custom_device_id, pc.custom_name
        FROM wvp_device_channel c
        LEFT JOIN wvp_device d ON c.device_id = d.device_id
        LEFT JOIN wvp_platform_channel pc
               ON pc.device_channel_id = c.id AND pc.platform_id = $1
        WHERE ($2 = '' OR c.name LIKE $3 OR c.gb_device_id LIKE $3)
          AND ($4::bool IS NULL OR d.on_line = $4)
          AND ($5::int IS NULL OR c.channel_type = $5)
          AND (($6 = 'true' AND pc.id IS NOT NULL) OR ($6 != 'true' AND pc.id IS NULL))
        ORDER BY c.id DESC
        LIMIT $7 OFFSET $8
        "#,
    )
    .bind(platform_id)
    .bind(&search)
    .bind(&like)
    .bind(online)
    .bind(channel_type)
    .bind(has_share)
    .bind(count as i64)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.channel_type, d.manufacturer,
               pc.id as platform_channel_id, pc.custom_device_id, pc.custom_name
        FROM wvp_device_channel c
        LEFT JOIN wvp_device d ON c.device_id = d.device_id
        LEFT JOIN wvp_platform_channel pc
               ON pc.device_channel_id = c.id AND pc.platform_id = ?
        WHERE (? = '' OR c.name LIKE ? OR c.gb_device_id LIKE ?)
          AND (? IS NULL OR d.on_line = ?)
          AND (? IS NULL OR c.channel_type = ?)
          AND ((? = 'true' AND pc.id IS NOT NULL) OR (? <> 'true' AND pc.id IS NULL))
        ORDER BY c.id DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(platform_id)
    .bind(&search)
    .bind(&like)
    .bind(&like)
    .bind(online)
    .bind(online)
    .bind(channel_type)
    .bind(channel_type)
    .bind(has_share)
    .bind(has_share)
    .bind(count as i64)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    #[cfg(feature = "postgres")]
    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM wvp_device_channel c
        LEFT JOIN wvp_device d ON c.device_id = d.device_id
        LEFT JOIN wvp_platform_channel pc
               ON pc.device_channel_id = c.id AND pc.platform_id = $1
        WHERE ($2 = '' OR c.name LIKE $3 OR c.gb_device_id LIKE $3)
          AND ($4::bool IS NULL OR d.on_line = $4)
          AND ($5::int IS NULL OR c.channel_type = $5)
          AND (($6 = 'true' AND pc.id IS NOT NULL) OR ($6 != 'true' AND pc.id IS NULL))
        "#,
    )
    .bind(platform_id)
    .bind(&search)
    .bind(&like)
    .bind(online)
    .bind(channel_type)
    .bind(has_share)
    .fetch_one(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM wvp_device_channel c
        LEFT JOIN wvp_device d ON c.device_id = d.device_id
        LEFT JOIN wvp_platform_channel pc
               ON pc.device_channel_id = c.id AND pc.platform_id = ?
        WHERE (? = '' OR c.name LIKE ? OR c.gb_device_id LIKE ?)
          AND (? IS NULL OR d.on_line = ?)
          AND (? IS NULL OR c.channel_type = ?)
          AND ((? = 'true' AND pc.id IS NOT NULL) OR (? <> 'true' AND pc.id IS NULL))
        "#,
    )
    .bind(platform_id)
    .bind(&search)
    .bind(&like)
    .bind(&like)
    .bind(online)
    .bind(online)
    .bind(channel_type)
    .bind(channel_type)
    .bind(has_share)
    .bind(has_share)
    .fetch_one(&state.pool)
    .await?;

    let rows: Vec<serde_json::Value> = rows.iter().map(|r| {
        serde_json::json!({
            "id": r.try_get::<i64, _>("platform_channel_id").unwrap_or_default(),
            "platformId": platform_id,
            "gbId": r.try_get::<Option<String>, _>("gb_device_id").ok().flatten(),
            "gbDeviceId": r.try_get::<Option<String>, _>("gb_device_id").ok().flatten(),
            "gbName": r.try_get::<Option<String>, _>("name").ok().flatten(),
            "gbManufacturer": r.try_get::<Option<String>, _>("manufacturer").ok().flatten(),
            "gbStatus": r.try_get::<Option<String>, _>("status").ok().flatten().unwrap_or_else(|| "OFF".to_string()),
            "dataType": r.try_get::<Option<i32>, _>("channel_type").ok().flatten().unwrap_or(0),
            "deviceChannelId": r.try_get::<i64, _>("id").unwrap_or_default(),
            "customDeviceId": r.try_get::<Option<String>, _>("custom_device_id").ok().flatten(),
            "customName": r.try_get::<Option<String>, _>("custom_name").ok().flatten()
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
    #[serde(alias = "id")]
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
    
    let server_gb_id = match platform.server_gb_id.clone() {
        Some(id) => id,
        None => {
            return Ok(Json(WVPResult::success(serde_json::json!({
                "message": "平台国标ID未设置",
                "code": 1
            }))));
        }
    };
    
    if platform.enable.unwrap_or(false) && !platform.status.unwrap_or(false) {
        let _ = sync_platform_registration(&state, &platform).await?;
    }

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
    
    let channel_ids = if let Some(channel_id_list) = &q.channel_id_list {
        channel_id_list
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
    } else {
        #[cfg(feature = "postgres")]
        let rows = sqlx::query(
            r#"SELECT c.gb_device_id
               FROM wvp_platform_channel pc
               INNER JOIN wvp_device_channel c ON c.id = pc.device_channel_id
               WHERE pc.platform_id = $1"#,
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        #[cfg(feature = "mysql")]
        let rows = sqlx::query(
            r#"SELECT c.gb_device_id
               FROM wvp_platform_channel pc
               INNER JOIN wvp_device_channel c ON c.id = pc.device_channel_id
               WHERE pc.platform_id = ?"#,
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        rows.into_iter()
            .filter_map(|row| row.try_get::<Option<String>, _>("gb_device_id").ok().flatten())
            .collect::<Vec<_>>()
    };

    for channel_id in channel_ids {
        let channel_id = channel_id.trim();
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
    
    refresh_platform_catalog(&state, platform_id).await?;

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
    #[serde(alias = "serverGBId")]
    pub server_gb_id: Option<String>,
    #[serde(alias = "serverIp", alias = "serverHost")]
    pub server_host: Option<String>,
    #[serde(alias = "serverPort")]
    pub server_port: Option<i32>,
    pub transport: Option<String>,
    pub password: Option<String>,
    // 扩展字段
    #[serde(alias = "serverGBDomain")]
    pub server_gb_domain: Option<String>,
    #[serde(alias = "deviceGBId")]
    pub device_gb_id: Option<String>,
    #[serde(alias = "deviceIp")]
    pub device_ip: Option<String>,
    #[serde(alias = "devicePort")]
    pub device_port: Option<String>,
    pub username: Option<String>,
    #[serde(alias = "civilCode")]
    pub civil_code: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub address: Option<String>,
    pub ptz: Option<bool>,
    pub rtcp: Option<bool>,
    #[serde(alias = "characterSet")]
    pub character_set: Option<String>,
    #[serde(alias = "catalogGroup")]
    pub catalog_group: Option<i32>,
    pub secrecy: Option<i32>,
    #[serde(alias = "asMessageChannel")]
    pub as_message_channel: Option<bool>,
    #[serde(alias = "autoPushChannel")]
    pub auto_push_channel: Option<bool>,
    #[serde(alias = "catalogWithPlatform")]
    pub catalog_with_platform: Option<i32>,
    #[serde(alias = "catalogWithGroup")]
    pub catalog_with_group: Option<i32>,
    #[serde(alias = "catalogWithRegion")]
    pub catalog_with_region: Option<i32>,
    #[serde(alias = "sendStreamIp")]
    pub send_stream_ip: Option<String>,
    pub enable: Option<bool>,
    pub expires: Option<String>,
    #[serde(alias = "keepTimeout")]
    pub keep_timeout: Option<String>,
}

/// POST /api/platform/add
pub async fn platform_add(
    State(state): State<AppState>,
    Json(body): Json<PlatformAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let name = body.name.clone().unwrap_or_default();
    let server_gb_id = body.server_gb_id.clone().unwrap_or_default();
    let server_ip = body.server_host.clone().unwrap_or_default();
    let server_port = body.server_port.unwrap_or(5060);
    let device_gb_id = body.device_gb_id.clone().unwrap_or_default();
    let transport = body.transport.clone().unwrap_or_else(|| "TCP".to_string());
    let username = body.username.clone().unwrap_or_default();
    let password = body.password.clone().unwrap_or_default();
    
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

    #[cfg(feature = "postgres")]
    sqlx::query(
        r#"UPDATE wvp_platform SET
           server_gb_domain = COALESCE($1, server_gb_domain),
           device_ip = COALESCE($2, device_ip),
           device_port = COALESCE($3, device_port),
           civil_code = COALESCE($4, civil_code),
           manufacturer = COALESCE($5, manufacturer),
           model = COALESCE($6, model),
           address = COALESCE($7, address),
           ptz = COALESCE($8, ptz),
           rtcp = COALESCE($9, rtcp),
           character_set = COALESCE($10, character_set),
           catalog_group = COALESCE($11, catalog_group),
           secrecy = COALESCE($12, secrecy),
           as_message_channel = COALESCE($13, as_message_channel),
           auto_push_channel = COALESCE($14, auto_push_channel),
           catalog_with_platform = COALESCE($15, catalog_with_platform),
           catalog_with_group = COALESCE($16, catalog_with_group),
           catalog_with_region = COALESCE($17, catalog_with_region),
           send_stream_ip = COALESCE($18, send_stream_ip),
           enable = COALESCE($19, enable),
           expires = COALESCE($20, expires),
           keep_timeout = COALESCE($21, keep_timeout)
           WHERE server_gb_id = $22"#,
    )
    .bind(body.server_gb_domain.as_deref())
    .bind(body.device_ip.as_deref())
    .bind(body.device_port.as_deref())
    .bind(body.civil_code.as_deref())
    .bind(body.manufacturer.as_deref())
    .bind(body.model.as_deref())
    .bind(body.address.as_deref())
    .bind(body.ptz)
    .bind(body.rtcp)
    .bind(body.character_set.as_deref())
    .bind(body.catalog_group)
    .bind(body.secrecy)
    .bind(body.as_message_channel)
    .bind(body.auto_push_channel)
    .bind(body.catalog_with_platform)
    .bind(body.catalog_with_group)
    .bind(body.catalog_with_region)
    .bind(body.send_stream_ip.as_deref())
    .bind(body.enable)
    .bind(body.expires.as_deref())
    .bind(body.keep_timeout.as_deref())
    .bind(&server_gb_id)
    .execute(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    sqlx::query(
        r#"UPDATE wvp_platform SET
           server_gb_domain = COALESCE(?, server_gb_domain),
           device_ip = COALESCE(?, device_ip),
           device_port = COALESCE(?, device_port),
           civil_code = COALESCE(?, civil_code),
           manufacturer = COALESCE(?, manufacturer),
           model = COALESCE(?, model),
           address = COALESCE(?, address),
           ptz = COALESCE(?, ptz),
           rtcp = COALESCE(?, rtcp),
           character_set = COALESCE(?, character_set),
           catalog_group = COALESCE(?, catalog_group),
           secrecy = COALESCE(?, secrecy),
           as_message_channel = COALESCE(?, as_message_channel),
           auto_push_channel = COALESCE(?, auto_push_channel),
           catalog_with_platform = COALESCE(?, catalog_with_platform),
           catalog_with_group = COALESCE(?, catalog_with_group),
           catalog_with_region = COALESCE(?, catalog_with_region),
           send_stream_ip = COALESCE(?, send_stream_ip),
           enable = COALESCE(?, enable),
           expires = COALESCE(?, expires),
           keep_timeout = COALESCE(?, keep_timeout)
           WHERE server_gb_id = ?"#,
    )
    .bind(body.server_gb_domain.as_deref())
    .bind(body.device_ip.as_deref())
    .bind(body.device_port.as_deref())
    .bind(body.civil_code.as_deref())
    .bind(body.manufacturer.as_deref())
    .bind(body.model.as_deref())
    .bind(body.address.as_deref())
    .bind(body.ptz)
    .bind(body.rtcp)
    .bind(body.character_set.as_deref())
    .bind(body.catalog_group)
    .bind(body.secrecy)
    .bind(body.as_message_channel)
    .bind(body.auto_push_channel)
    .bind(body.catalog_with_platform)
    .bind(body.catalog_with_group)
    .bind(body.catalog_with_region)
    .bind(body.send_stream_ip.as_deref())
    .bind(body.enable)
    .bind(body.expires.as_deref())
    .bind(body.keep_timeout.as_deref())
    .bind(&server_gb_id)
    .execute(&state.pool)
    .await?;

    if let Some(platform) = platform_db::get_by_server_gb_id(&state.pool, &server_gb_id).await? {
        let registered = sync_platform_registration(&state, &platform).await?;
        if registered && platform.auto_push_channel.unwrap_or(false) {
            refresh_platform_catalog(&state, platform.id as i64).await?;
        }
    }
    
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

    #[cfg(feature = "postgres")]
    sqlx::query(
        r#"UPDATE wvp_platform SET
           server_gb_domain = COALESCE($1, server_gb_domain),
           device_ip = COALESCE($2, device_ip),
           device_port = COALESCE($3, device_port),
           civil_code = COALESCE($4, civil_code),
           manufacturer = COALESCE($5, manufacturer),
           model = COALESCE($6, model),
           address = COALESCE($7, address),
           ptz = COALESCE($8, ptz),
           rtcp = COALESCE($9, rtcp),
           character_set = COALESCE($10, character_set),
           catalog_group = COALESCE($11, catalog_group),
           secrecy = COALESCE($12, secrecy),
           as_message_channel = COALESCE($13, as_message_channel),
           auto_push_channel = COALESCE($14, auto_push_channel),
           catalog_with_platform = COALESCE($15, catalog_with_platform),
           catalog_with_group = COALESCE($16, catalog_with_group),
           catalog_with_region = COALESCE($17, catalog_with_region),
           send_stream_ip = COALESCE($18, send_stream_ip),
           enable = COALESCE($19, enable),
           expires = COALESCE($20, expires),
           keep_timeout = COALESCE($21, keep_timeout),
           update_time = $22
           WHERE id = $23"#,
    )
    .bind(body.server_gb_domain.as_deref())
    .bind(body.device_ip.as_deref())
    .bind(body.device_port.as_deref())
    .bind(body.civil_code.as_deref())
    .bind(body.manufacturer.as_deref())
    .bind(body.model.as_deref())
    .bind(body.address.as_deref())
    .bind(body.ptz)
    .bind(body.rtcp)
    .bind(body.character_set.as_deref())
    .bind(body.catalog_group)
    .bind(body.secrecy)
    .bind(body.as_message_channel)
    .bind(body.auto_push_channel)
    .bind(body.catalog_with_platform)
    .bind(body.catalog_with_group)
    .bind(body.catalog_with_region)
    .bind(body.send_stream_ip.as_deref())
    .bind(body.enable)
    .bind(body.expires.as_deref())
    .bind(body.keep_timeout.as_deref())
    .bind(&now)
    .bind(id)
    .execute(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    sqlx::query(
        r#"UPDATE wvp_platform SET
           server_gb_domain = COALESCE(?, server_gb_domain),
           device_ip = COALESCE(?, device_ip),
           device_port = COALESCE(?, device_port),
           civil_code = COALESCE(?, civil_code),
           manufacturer = COALESCE(?, manufacturer),
           model = COALESCE(?, model),
           address = COALESCE(?, address),
           ptz = COALESCE(?, ptz),
           rtcp = COALESCE(?, rtcp),
           character_set = COALESCE(?, character_set),
           catalog_group = COALESCE(?, catalog_group),
           secrecy = COALESCE(?, secrecy),
           as_message_channel = COALESCE(?, as_message_channel),
           auto_push_channel = COALESCE(?, auto_push_channel),
           catalog_with_platform = COALESCE(?, catalog_with_platform),
           catalog_with_group = COALESCE(?, catalog_with_group),
           catalog_with_region = COALESCE(?, catalog_with_region),
           send_stream_ip = COALESCE(?, send_stream_ip),
           enable = COALESCE(?, enable),
           expires = COALESCE(?, expires),
           keep_timeout = COALESCE(?, keep_timeout),
           update_time = ?
           WHERE id = ?"#,
    )
    .bind(body.server_gb_domain.as_deref())
    .bind(body.device_ip.as_deref())
    .bind(body.device_port.as_deref())
    .bind(body.civil_code.as_deref())
    .bind(body.manufacturer.as_deref())
    .bind(body.model.as_deref())
    .bind(body.address.as_deref())
    .bind(body.ptz)
    .bind(body.rtcp)
    .bind(body.character_set.as_deref())
    .bind(body.catalog_group)
    .bind(body.secrecy)
    .bind(body.as_message_channel)
    .bind(body.auto_push_channel)
    .bind(body.catalog_with_platform)
    .bind(body.catalog_with_group)
    .bind(body.catalog_with_region)
    .bind(body.send_stream_ip.as_deref())
    .bind(body.enable)
    .bind(body.expires.as_deref())
    .bind(body.keep_timeout.as_deref())
    .bind(&now)
    .bind(id)
    .execute(&state.pool)
    .await?;

    if let Some(platform) = platform_db::get_by_id(&state.pool, id).await? {
        let registered = sync_platform_registration(&state, &platform).await?;
        if registered && platform.auto_push_channel.unwrap_or(false) {
            refresh_platform_catalog(&state, platform.id as i64).await?;
        }
    }
    
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
        if let Some(platform) = platform_db::get_by_id(&state.pool, id).await? {
            if platform.status.unwrap_or(false) {
                let _ = sync_platform_registration(
                    &state,
                    &Platform {
                        enable: Some(false),
                        ..platform.clone()
                    },
                )
                .await;
            }
        }
        let _ = platform_channel::batch_delete_by_platform(&state.pool, id).await;
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
    let maybe_platform = platform_db::get_by_device_gb_id(&state.pool, &device_gb_id).await;
    let exists = matches!(maybe_platform, Ok(Some(_)));
    Json(WVPResult::success(serde_json::json!(exists)))
}

// ========== 平台通道操作 ==========

/// POST /api/platform/channel/add
#[derive(Debug, Deserialize)]
pub struct PlatformChannelAddBody {
    #[serde(alias = "platformId")]
    pub platform_id: Option<i64>,
    #[serde(alias = "channelIds")]
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
    
    let platform = platform_db::get_by_id(&state.pool, platform_id).await?;
    let mut added_count = 0;
    let mut pushed_channels = Vec::new();
    if body.all == Some(true) {
        #[cfg(feature = "postgres")]
        let rows = sqlx::query(
            "SELECT id, gb_device_id FROM wvp_device_channel WHERE id NOT IN (SELECT device_channel_id FROM wvp_platform_channel WHERE platform_id = $1)"
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        #[cfg(feature = "mysql")]
        let rows = sqlx::query(
            "SELECT id, gb_device_id FROM wvp_device_channel WHERE id NOT IN (SELECT device_channel_id FROM wvp_platform_channel WHERE platform_id = ?)"
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        for row in rows {
            let channel_db_id = row.try_get::<i64, _>("id").unwrap_or_default();
            if channel_db_id > 0 && platform_channel::add(&state.pool, platform_id, channel_db_id).await.is_ok() {
                added_count += 1;
                if let Some(gb_device_id) = row.try_get::<Option<String>, _>("gb_device_id").ok().flatten() {
                    pushed_channels.push(gb_device_id);
                }
            }
        }
    } else if let Some(channel_ids) = body.channel_ids.clone() {
        for channel_id_str in channel_ids {
            #[cfg(feature = "postgres")]
            let row = sqlx::query("SELECT id, gb_device_id FROM wvp_device_channel WHERE gb_device_id = $1 OR CAST(id AS TEXT) = $1 LIMIT 1")
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "mysql")]
            let row = sqlx::query("SELECT id, gb_device_id FROM wvp_device_channel WHERE gb_device_id = ? OR CAST(id AS CHAR) = ? LIMIT 1")
                .bind(&channel_id_str)
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            if let Some(row) = row {
                let channel_db_id = row.try_get::<i64, _>("id").unwrap_or_default();
                if channel_db_id > 0 && platform_channel::add(&state.pool, platform_id, channel_db_id).await.is_ok() {
                    added_count += 1;
                    pushed_channels.push(
                        row.try_get::<Option<String>, _>("gb_device_id")
                            .ok()
                            .flatten()
                            .unwrap_or(channel_id_str),
                    );
                }
            }
        }
    }

    if let Some(platform) = platform {
        if platform.enable.unwrap_or(false) && !platform.status.unwrap_or(false) {
            let _ = sync_platform_registration(&state, &platform).await?;
        }
        if platform.auto_push_channel.unwrap_or(false) {
            let _ = push_platform_channels(&state, &platform, &pushed_channels).await?;
        } else if added_count > 0 {
            refresh_platform_catalog(&state, platform_id).await?;
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "addedCount": added_count,
        "message": "通道添加成功",
        "code": 0
    }))))
}

/// POST /api/platform/channel/device/add - 添加设备的所有通道
#[derive(Debug, Deserialize)]
pub struct PlatformChannelDeviceBody {
    #[serde(alias = "platformId")]
    pub platform_id: Option<i64>,
    #[serde(alias = "deviceIds")]
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
    let platform = platform_db::get_by_id(&state.pool, platform_id).await?;
    let mut added_count = 0;
    let mut pushed_channels = Vec::new();
    
    for device_id in device_ids {
        let channels = db_device::list_channels_for_device(&state.pool, &device_id).await?;
        
        for channel in channels {
            let channel_db_id = channel.id as i64;
            if channel_db_id > 0 {
                if platform_channel::add(&state.pool, platform_id, channel_db_id).await.is_ok() {
                    added_count += 1;
                    pushed_channels.push(channel.gb_device_id.clone().unwrap_or_else(|| channel_db_id.to_string()));
                }
            }
        }
    }

    if let Some(platform) = platform {
        if platform.enable.unwrap_or(false) && !platform.status.unwrap_or(false) {
            let _ = sync_platform_registration(&state, &platform).await?;
        }
        if platform.auto_push_channel.unwrap_or(false) {
            let _ = push_platform_channels(&state, &platform, &pushed_channels).await?;
        } else if added_count > 0 {
            refresh_platform_catalog(&state, platform_id).await?;
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
            let channel_db_id = channel.id as i64;
            if channel_db_id > 0 && platform_channel::delete_by_device_channel_id(&state.pool, platform_id, channel_db_id).await.is_ok() {
                removed_count += 1;
            }
        }
    }

    if removed_count > 0 {
        refresh_platform_catalog(&state, platform_id).await?;
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
    
    let mut removed_count = 0;
    if body.all == Some(true) {
        removed_count = platform_channel::batch_delete_by_platform(&state.pool, platform_id).await? as i32;
    } else if let Some(channel_ids) = body.channel_ids {
        for channel_id_str in channel_ids {
            #[cfg(feature = "postgres")]
            let row = sqlx::query("SELECT id FROM wvp_device_channel WHERE gb_device_id = $1 OR CAST(id AS TEXT) = $1 LIMIT 1")
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "mysql")]
            let row = sqlx::query("SELECT id FROM wvp_device_channel WHERE gb_device_id = ? OR CAST(id AS CHAR) = ? LIMIT 1")
                .bind(&channel_id_str)
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            if let Some(row) = row {
                let channel_id = row.try_get::<i64, _>("id").unwrap_or_default();
                removed_count += platform_channel::delete_by_device_channel_id(&state.pool, platform_id, channel_id).await? as i32;
            }
        }
    }

    if removed_count > 0 {
        refresh_platform_catalog(&state, platform_id).await?;
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "removedCount": removed_count,
        "message": "通道移除成功",
        "code": 0
    }))))
}

/// POST /api/platform/channel/custom/update
#[derive(Debug, Deserialize)]
pub struct PlatformChannelCustomUpdate {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(alias = "customName")]
    pub custom_name: Option<String>,
    #[serde(alias = "customDeviceId")]
    pub custom_device_id: Option<String>,
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
    
    let custom_name = body.custom_name.as_deref().or(body.name.as_deref());
    let custom_info = body.custom_info.as_deref();
    
    platform_channel::update(&state.pool, id, custom_name, custom_info).await?;
    if let Some(custom_device_id) = body.custom_device_id.as_deref() {
        #[cfg(feature = "postgres")]
        sqlx::query("UPDATE wvp_platform_channel SET custom_device_id = $1 WHERE id = $2")
            .bind(custom_device_id)
            .bind(id)
            .execute(&state.pool)
            .await?;
        #[cfg(feature = "mysql")]
        sqlx::query("UPDATE wvp_platform_channel SET custom_device_id = ? WHERE id = ?")
            .bind(custom_device_id)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }
    if let Ok(Some(ch)) = platform_channel::get_by_id(&state.pool, id).await {
        if let Some(platform_id) = ch.platform_id {
            refresh_platform_catalog(&state, platform_id).await?;
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
    if platform_id > 0 {
        let _ = refresh_platform_catalog(&state, platform_id).await;
    }
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
    if platform_id > 0 {
        let _ = refresh_platform_catalog(&state, platform_id).await;
    }
    Json(serde_json::json!({ "code": 0, "msg": "目录编辑成功" }))
}
