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
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;

/// Deserialize a port field that may be either a JSON number (Vue's default)
/// or a JSON string (some legacy frontend forms). Returns the value as a String.
fn deserialize_port<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        Value::Number(n) => Ok(Some(n.to_string())),
        // Anything else: try to take its raw text representation
        other => Ok(Some(other.to_string())),
    }
}

use crate::AppState;

/// 内部工具 — 按 feature 分发不同 SQL；sqlite 路径下部分参数仅在 cfg(postgres/mysql) 中使用
#[allow(unused_variables)]
async fn update_platform_status(pool: &crate::db::Pool, id: i64, status: bool) -> Result<(), sqlx::Error> {
    #[cfg(feature = "postgres")]
    sqlx::query("UPDATE gb_platform SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "mysql")]
    sqlx::query("UPDATE gb_platform SET status = ? WHERE id = ?")
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

    let sip = &*sip_server;

    // 级联注册的唯一实现是 `CascadeRegistrar`：它持有注册状态、401 挑战、
    // 周期重试与 keepalive。这里只负责"把 DB 变更同步给它 + 立即触发一次"。
    let Some(registrar) = sip.cascade_registrar() else {
        tracing::warn!("级联注册器未就绪，无法同步平台 {} 的注册状态", server_gb_id);
        update_platform_status(&state.pool, id, false).await?;
        return Ok(false);
    };

    let local_device_id = sip.config().device_id.clone();
    let realm = sip.config().realm.clone();

    let result = if enable {
        match registrar
            .upsert_platform_from_db(server_gb_id, &local_device_id, &realm)
            .await
        {
            Ok(()) => registrar
                .register_now(server_gb_id)
                .await
                .map_err(|e| anyhow::anyhow!(e)),
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    } else {
        registrar
            .unregister_and_remove(server_gb_id, 0)
            .await
            .map_err(|e| anyhow::anyhow!(e))
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

    let sip = &*sip_server;
    let mut pushed_count = 0;
    for channel_id in channel_ids {
        let channel_id = channel_id.trim();
        if channel_id.is_empty() {
            continue;
        }
        // 为这一路级联取流分配 ZLM 收流端口，并写进 INVITE 的 m=video。
        // 修正：此前固定传 0，而 SDP 中 m=video 0 表示该媒体流被禁用，
        // 上级平台没有可推流的目标地址，级联取流不可能建立。
        let Some(zlm) = state.zlm_client.as_ref() else {
            tracing::error!("ZLM 未配置，无法为级联取流分配收流端口");
            break;
        };
        let stream_id = format!("cascade_{}_{}", server_gb_id, channel_id);
        let media_port = match zlm
            .open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                secret: zlm.secret.clone(),
                stream_id: stream_id.clone(),
                port: Some(0),
                use_tcp: Some(false),
                rtp_type: Some(0),
                recv_port: None,
            })
            .await
        {
            Ok(info) => info.port,
            Err(e) => {
                tracing::error!("openRtpServer for cascade {} failed: {}", stream_id, e);
                continue;
            }
        };
        if sip
            .send_platform_invite(server_gb_id, channel_id, media_port)
            .await
            .is_ok()
        {
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
    let sip = &*sip_server;
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
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_platform")
            .fetch_one(&state.pool)
            .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM gb_platform WHERE name LIKE $1 OR server_gb_id LIKE $1 OR device_gb_id LIKE $1",
        )
        .bind(&like)
        .fetch_one(&state.pool)
        .await?
    };
    #[cfg(feature = "mysql")]
    let total = if search.is_empty() {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_platform")
            .fetch_one(&state.pool)
            .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM gb_platform WHERE name LIKE ? OR server_gb_id LIKE ? OR device_gb_id LIKE ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&state.pool)
        .await?
    };
    #[cfg(feature = "sqlite")]
    let total = if search.is_empty() {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_platform")
            .fetch_one(&state.pool)
            .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM gb_platform WHERE name LIKE ? OR server_gb_id LIKE ? OR device_gb_id LIKE ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&state.pool)
        .await?
    };

    #[cfg(feature = "postgres")]
    let raw_list = if search.is_empty() {
        sqlx::query_as::<_, Platform>("SELECT * FROM gb_platform ORDER BY id DESC LIMIT $1 OFFSET $2")
            .bind(count as i64)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Platform>(
            "SELECT * FROM gb_platform WHERE name LIKE $1 OR server_gb_id LIKE $1 OR device_gb_id LIKE $1 ORDER BY id DESC LIMIT $2 OFFSET $3",
        )
        .bind(&like)
        .bind(count as i64)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?
    };
    #[cfg(feature = "mysql")]
    let raw_list = if search.is_empty() {
        sqlx::query_as::<_, Platform>("SELECT * FROM gb_platform ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(count as i64)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Platform>(
            "SELECT * FROM gb_platform WHERE name LIKE ? OR server_gb_id LIKE ? OR device_gb_id LIKE ? ORDER BY id DESC LIMIT ? OFFSET ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(count as i64)
        .bind(offset)
        .fetch_all(&state.pool)
            .await?
    };
    #[cfg(feature = "sqlite")]
    let raw_list = if search.is_empty() {
        sqlx::query_as::<_, Platform>("SELECT * FROM gb_platform ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(count as i64)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Platform>(
            "SELECT * FROM gb_platform WHERE name LIKE ? OR server_gb_id LIKE ? OR device_gb_id LIKE ? ORDER BY id DESC LIMIT ? OFFSET ?",
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
        list.push(platform_row_json(&item, channel_count));
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
    // `serverGBId` 是**本平台的 20 位国标编码**（sip.device_id），
    // `serverGBDomain` 才是域（sip.realm）。此前两者都填 realm
    // —— 拿它去上级平台登记会用错编号。
    let local_gb_id = sip
        .map(|cfg| cfg.device_id.clone())
        .unwrap_or_else(|| "34020000002000000001".to_string());
    let realm = sip
        .map(|cfg| cfg.realm.clone())
        .unwrap_or_else(|| "3402000000".to_string());
    Json(WVPResult::success(serde_json::json!({
        "id": null,
        "name": "本地平台",
        "serverGBId": local_gb_id,
        "serverGBDomain": realm,
        // 旧前端/旧接口声明的字段名
        "realm": realm,
        "ip": sip.as_ref().map(|cfg| cfg.ip.clone()).unwrap_or_else(|| "127.0.0.1".to_string()),
        "port": sip.as_ref().map(|cfg| cfg.port as i32).unwrap_or(5060),
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
        FROM gb_device_channel c
        LEFT JOIN gb_device d ON c.device_id = d.device_id
        LEFT JOIN gb_platform_channel pc
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
        FROM gb_device_channel c
        LEFT JOIN gb_device d ON c.device_id = d.device_id
        LEFT JOIN gb_platform_channel pc
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
    #[cfg(feature = "sqlite")]
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.channel_type, d.manufacturer,
               pc.id as platform_channel_id, pc.custom_device_id, pc.custom_name
        FROM gb_device_channel c
        LEFT JOIN gb_device d ON c.device_id = d.device_id
        LEFT JOIN gb_platform_channel pc
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
        FROM gb_device_channel c
        LEFT JOIN gb_device d ON c.device_id = d.device_id
        LEFT JOIN gb_platform_channel pc
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
        FROM gb_device_channel c
        LEFT JOIN gb_device d ON c.device_id = d.device_id
        LEFT JOIN gb_platform_channel pc
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
    #[cfg(feature = "sqlite")]
    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM gb_device_channel c
        LEFT JOIN gb_device d ON c.device_id = d.device_id
        LEFT JOIN gb_platform_channel pc
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
    #[serde(alias = "platformId")]
    pub platform_id: Option<i64>,
    #[serde(alias = "channelIdList")]
    pub channel_id_list: Option<String>,
    #[serde(alias = "deviceIdList")]
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
    
    let sip = &*sip_server;
    
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
               FROM gb_platform_channel pc
               INNER JOIN gb_device_channel c ON c.id = pc.device_channel_id
               WHERE pc.platform_id = $1"#,
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        let rows = sqlx::query(
            r#"SELECT c.gb_device_id
               FROM gb_platform_channel pc
               INNER JOIN gb_device_channel c ON c.id = pc.device_channel_id
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

        // 同 push_platform_channels：INVITE 里的 m=video 必须是真实可推流端口
        let Some(zlm) = state.zlm_client.as_ref() else {
            errors.push("ZLM 未配置，无法分配收流端口".to_string());
            break;
        };
        let stream_id = format!("cascade_{}_{}", server_gb_id, channel_id);
        let media_port = match zlm
            .open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                secret: zlm.secret.clone(),
                stream_id: stream_id.clone(),
                port: Some(0),
                use_tcp: Some(false),
                rtp_type: Some(0),
                recv_port: None,
            })
            .await
        {
            Ok(info) => info.port,
            Err(e) => {
                tracing::error!("openRtpServer for cascade {} failed: {}", stream_id, e);
                errors.push(format!("{}: ZLM 收流端口分配失败: {}", channel_id, e));
                continue;
            }
        };

        match sip
            .send_platform_invite(&server_gb_id, channel_id, media_port)
            .await
        {
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

/// 数字列（`expires` / `keep_timeout`）在体里可能是 `3600` 也可能是 `"3600"`。
///
/// WVP 的 Java 端是 `int`，Vue3 的 `el-input-number` 输出 number，而本仓库这两列
/// 是 varchar —— 只认字符串会在反序列化阶段 422（请求根本进不到 handler），
/// 只认数字又会让老客户端挂。这里两者都收，统一存成字符串。
fn deserialize_opt_int_string<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::String(s)) => Some(s),
        Some(other) => {
            return Err(serde::de::Error::custom(format!(
                "需要数字或字符串，收到 {other}"
            )))
        }
    })
}

/// 回给前端的数值：能解析成整数就给整数（与 WVP 的 `int expires` 一致），
/// 否则原样回字符串。
fn int_or_string(v: &Option<String>) -> serde_json::Value {
    match v.as_deref().map(str::trim) {
        None | Some("") => serde_json::Value::Null,
        Some(s) => match s.parse::<i64>() {
            Ok(n) => serde_json::Value::from(n),
            Err(_) => serde_json::Value::from(s),
        },
    }
}

/// 平台行的统一 JSON —— 列表与详情**共用同一份**，避免两处字段漂移
/// （此前 `/info/:id` 只回 10 个字段，`serverGBDomain`/`expires`/`keepTimeout`
/// 等全缺，任何按完整类型取值的调用方都会拿到 undefined）。
fn platform_row_json(item: &Platform, channel_count: i64) -> serde_json::Value {
    serde_json::json!({
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
        "expires": int_or_string(&item.expires),
        // 只回 WVP 的 `keepTimeout`，**不要**再回一个 `heartBeatInterval` 同义键：
        // 前端会把整行原样提交回 /platform/update，而 DTO 两个键都收
        // → serde 报 "duplicate field"，更新稳定 422。
        "keepTimeout": int_or_string(&item.keep_timeout),
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
    })
}

/// POST /api/platform/add 请求体
#[derive(Debug, Deserialize)]
pub struct PlatformAddBody {
    pub id: Option<i64>,
    pub name: Option<String>,
    // 前端（与 WVP 的 `Platform.java`）用的是 `serverGBId`；`serverGbId` 是
    // 本仓库 Vue3 前端历史上的错误拼写，仍然收下，避免旧客户端静默写空串。
    #[serde(alias = "serverGBId", alias = "serverGbId")]
    pub server_gb_id: Option<String>,
    #[serde(alias = "serverIp", alias = "serverHost")]
    pub server_host: Option<String>,
    #[serde(alias = "serverPort")]
    pub server_port: Option<i32>,
    pub transport: Option<String>,
    pub password: Option<String>,
    // 扩展字段
    #[serde(alias = "serverGBDomain", alias = "realm")]
    pub server_gb_domain: Option<String>,
    #[serde(alias = "deviceGBId")]
    pub device_gb_id: Option<String>,
    #[serde(alias = "deviceIp")]
    pub device_ip: Option<String>,
    // `deserialize_with` 会**去掉** `Option<T>` 字段的隐式 default，
    // 于是这个本该可选的字段变成了必填 —— 前端平台表单从不提交
    // `devicePort`，`POST /api/platform/add` 因此必然 422
    // "missing field `device_port`"，新建平台完全不可用。
    #[serde(default, alias = "devicePort", deserialize_with = "deserialize_port")]
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
    #[serde(default, deserialize_with = "deserialize_opt_int_string")]
    pub expires: Option<String>,
    // 心跳周期：WVP 叫 `keepTimeout`，旧前端叫 `heartBeatInterval`，两者都收
    #[serde(
        default,
        alias = "keepTimeout",
        alias = "heartBeatInterval",
        deserialize_with = "deserialize_opt_int_string"
    )]
    pub keep_timeout: Option<String>,
}

/// POST /api/platform/add
pub async fn platform_add(
    State(state): State<AppState>,
    Json(body): Json<PlatformAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    // 必填校验：此前 `server_gb_id` 绑不上（键名 serverGbId 被忽略）时会写空串，
    // 接口照样回「平台添加成功」，但库里 `server_gb_id` 为空 ⇒ 级联注册、
    // `get_by_server_gb_id` 全部以空串为键，平台**实际不可用**。
    let name = body.name.clone().unwrap_or_default().trim().to_string();
    let server_gb_id = body.server_gb_id.clone().unwrap_or_default().trim().to_string();
    let server_ip = body.server_host.clone().unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "平台名称不能为空"));
    }
    if server_gb_id.is_empty() {
        return Err(AppError::business(
            ErrorCode::Error400,
            "国标ID(serverGBId)不能为空",
        ));
    }
    if server_ip.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "平台 IP 不能为空"));
    }
    if platform_db::get_by_server_gb_id(&state.pool, &server_gb_id)
        .await?
        .is_some()
    {
        return Err(AppError::business(
            ErrorCode::Error400,
            format!("平台国标ID已存在: {server_gb_id}"),
        ));
    }
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
        r#"UPDATE gb_platform SET
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
        r#"UPDATE gb_platform SET
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
    #[cfg(feature = "sqlite")]
    sqlx::query(
        r#"UPDATE gb_platform SET
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

    let mut created_id: Option<i32> = None;
    if let Some(platform) = platform_db::get_by_server_gb_id(&state.pool, &server_gb_id).await? {
        created_id = Some(platform.id);
        let registered = sync_platform_registration(&state, &platform).await?;
        if registered && platform.auto_push_channel.unwrap_or(false) {
            refresh_platform_catalog(&state, platform.id as i64).await?;
        }
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        // 回传新建平台的标识：前端拿到后可直接定位/刷新该行，
        // 也便于脚本化验证（此前只回 name）。
        "id": created_id,
        "serverGBId": server_gb_id,
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
        r#"UPDATE gb_platform SET
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
        r#"UPDATE gb_platform SET
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
    #[cfg(feature = "sqlite")]
    sqlx::query(
        r#"UPDATE gb_platform SET
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
        // 修正：级联删除失败被 `let _ =` 吞掉时，平台行会被删掉但 gb_platform_channel
        // 里还留着一批指向不存在平台的孤儿行。必须传播。
        platform_channel::batch_delete_by_platform(&state.pool, id)
            .await
            .map_err(|e| {
                AppError::business(ErrorCode::Error500, format!("删除平台通道关联失败: {}", e))
            })?;
        platform_db::delete_by_id(&state.pool, id).await?;
    }
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "平台删除成功",
        "code": 0
    }))))
}

/// GET /api/platform/exit/:serverGBId —— 向上级平台发送 **Expires: 0** 的注销 REGISTER
///
/// 此前这里按 `device_gb_id` 查库、只回一个"是否存在"的布尔值，**没有任何注销动作**；
/// 而前端按钮写的是「注销」并始终弹「注销请求已发送」，参数传的又是 `serverGBId`，
/// 两列对不上 ⇒ 恒为 false、功能完全对不上号。
///
/// WVP 的同名接口确实只做"国标ID是否已存在"的校验（前端用它防重复），但本平台的
/// 按钮语义就是真注销，因此这里做实事：发注销报文 + 把 `enable`/`status` 落成 false
/// —— 只发报文不改 `enable` 的话，下一个注册周期会立刻把它注册回去，
/// 用户看到"注销成功"却仍然在线。
pub async fn platform_exit(
    State(state): State<AppState>,
    Path(server_gb_id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let server_gb_id = server_gb_id.trim().to_string();
    let platform = platform_db::get_by_server_gb_id(&state.pool, &server_gb_id)
        .await?
        .ok_or_else(|| {
            AppError::business(
                ErrorCode::Error404,
                format!("平台不存在: {server_gb_id}"),
            )
        })?;

    let mut sip_warning: Option<String> = None;
    match state.sip_server.as_ref().and_then(|sip| sip.cascade_registrar()) {
        Some(registrar) => {
            if let Err(e) = registrar.unregister_and_remove(&server_gb_id, 0).await {
                tracing::warn!("向 {} 发送注销 REGISTER 失败（仍置为停用）: {}", server_gb_id, e);
                sip_warning = Some(e);
            }
        }
        None => {
            tracing::warn!("级联注册器未就绪，{} 仅置为停用", server_gb_id);
            sip_warning = Some("级联注册器未就绪".to_string());
        }
    }

    // 停用 + 离线：不用 `sync_platform_registration` 是因为它会在 enable=false 时
    // 再调一次 unregister（我们已经发过了），这里只需要落库状态。
    sqlx::query(&crate::dyn_where::dialect_sql(
        "UPDATE gb_platform SET enable = ?, status = ? WHERE id = ?",
    ))
    .bind(false)
    .bind(false)
    .bind(platform.id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::business(ErrorCode::Error500, format!("更新平台状态失败: {e}")))?;

    Ok(Json(WVPResult::success(serde_json::json!({
        "id": platform.id,
        "serverGBId": server_gb_id,
        "exited": true,
        "sipWarning": sip_warning,
        "message": "注销请求已发送，平台已置为停用"
    }))))
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
            "SELECT id, gb_device_id FROM gb_device_channel WHERE id NOT IN (SELECT device_channel_id FROM gb_platform_channel WHERE platform_id = $1)"
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        #[cfg(feature = "mysql")]
        let rows = sqlx::query(
            "SELECT id, gb_device_id FROM gb_device_channel WHERE id NOT IN (SELECT device_channel_id FROM gb_platform_channel WHERE platform_id = ?)"
        )
        .bind(platform_id)
        .fetch_all(&state.pool)
        .await?;
        #[cfg(feature = "sqlite")]
        let rows = sqlx::query(
            "SELECT id, gb_device_id FROM gb_device_channel WHERE id NOT IN (SELECT device_channel_id FROM gb_platform_channel WHERE platform_id = ?)"
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
            let row = sqlx::query("SELECT id, gb_device_id FROM gb_device_channel WHERE gb_device_id = $1 OR CAST(id AS TEXT) = $1 LIMIT 1")
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "mysql")]
            let row = sqlx::query("SELECT id, gb_device_id FROM gb_device_channel WHERE gb_device_id = ? OR CAST(id AS CHAR) = ? LIMIT 1")
                .bind(&channel_id_str)
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "sqlite")]
            let row = sqlx::query("SELECT id, gb_device_id FROM gb_device_channel WHERE gb_device_id = ? OR CAST(id AS TEXT) = ? LIMIT 1")
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
            let row = sqlx::query("SELECT id FROM gb_device_channel WHERE gb_device_id = $1 OR CAST(id AS TEXT) = $1 LIMIT 1")
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "mysql")]
            let row = sqlx::query("SELECT id FROM gb_device_channel WHERE gb_device_id = ? OR CAST(id AS CHAR) = ? LIMIT 1")
                .bind(&channel_id_str)
                .bind(&channel_id_str)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "sqlite")]
            let row = sqlx::query("SELECT id FROM gb_device_channel WHERE gb_device_id = ? OR CAST(id AS TEXT) = ? LIMIT 1")
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

/// 内部工具 — 按 feature 分发不同 SQL；sqlite 路径下部分参数仅在 cfg(postgres/mysql) 中使用
#[allow(unused_variables)]
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
        sqlx::query("UPDATE gb_platform_channel SET custom_device_id = $1 WHERE id = $2")
            .bind(custom_device_id)
            .bind(id)
            .execute(&state.pool)
            .await?;
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        sqlx::query("UPDATE gb_platform_channel SET custom_device_id = ? WHERE id = ?")
            .bind(custom_device_id)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }
    if let Ok(Some(ch)) = platform_channel::get_by_id(&state.pool, id).await {
        if let Some(platform_id) = ch.platform_id {
            refresh_platform_catalog(&state, platform_id as i64).await?;
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
#[serde(rename_all = "camelCase")]
pub struct CatalogAddBody {
    pub id: Option<i64>,
    pub name: Option<String>,
    /// 前端（含归档的 legacy 前端）提交的是 `parentId`
    #[serde(alias = "parent")]
    pub parent_id: Option<String>,
    pub civil_code: Option<String>,
    pub business_group: Option<String>,
    pub platform_id: Option<i64>,
}

/// POST /api/platform/catalog/add
///
/// 2026-09-12 修复（此前有三重缺陷，等于**必然失败却报告成功**）：
/// 1. `gb_platform_catalog` 表在**三份 schema 中都不存在**
/// 2. **没有 `sqlite` 分支** —— 默认 SQLite 部署下该端点什么都不做
/// 3. INSERT 的错误被 `let _ =` 忽略，却始终返回「目录添加成功」
pub async fn catalog_add(
    State(state): State<AppState>,
    Json(body): Json<CatalogAddBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let name = body.name.clone().unwrap_or_default();
    let parent = body.parent_id.clone().unwrap_or_default();
    let civil_code = body.civil_code.clone().unwrap_or_default();
    let business_group = body.business_group.clone().unwrap_or_default();
    let platform_id = body.platform_id.unwrap_or(0);
    if name.trim().is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 name"));
    }
    tracing::info!(
        "platform catalog add: name={:?}, parent={}, civil_code={}, platform_id={}",
        name,
        parent,
        civil_code,
        platform_id
    );
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let sql = if cfg!(feature = "postgres") {
        "INSERT INTO gb_platform_catalog (name, parent, civil_code, business_group, platform_id, create_time, update_time) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)"
    } else {
        "INSERT INTO gb_platform_catalog (name, parent, civil_code, business_group, platform_id, create_time, update_time) \
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    };
    let affected = sqlx::query(sql)
        .bind(&name)
        .bind(&parent)
        .bind(&civil_code)
        .bind(&business_group)
        .bind(platform_id)
        .bind(&now)
        .bind(&now)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("目录写入失败: {}", e)))?
        .rows_affected();

    // 推送上级平台属尽力而为：本地已落库，推送失败只告警不失败
    if platform_id > 0 {
        if let Err(e) = refresh_platform_catalog(&state, platform_id).await {
            tracing::warn!("目录已写入，但刷新上级平台目录失败: {}", e);
        }
    }
    Ok(Json(serde_json::json!({
        "code": 0,
        "msg": "目录添加成功",
        "affected": affected,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAddBodyEdit {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(alias = "parent")]
    pub parent_id: Option<String>,
    pub civil_code: Option<String>,
    pub business_group: Option<String>,
    pub platform_id: Option<i64>,
}

/// POST /api/platform/catalog/edit (used in catalogEdit.vue, commonChannelEditDialog.vue)
///
/// 与 `catalog_add` 同样的问题已一并修复：补 sqlite 分支、传播错误、不再空转报成功。
#[allow(unused_variables)]
pub async fn catalog_edit(
    State(state): State<AppState>,
    Json(body): Json<CatalogAddBodyEdit>,
) -> Result<Json<serde_json::Value>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        // 无 id 时退化为新增（保持既有语义），但**必须传播错误**
        let add_body = CatalogAddBody {
            id: None,
            name: body.name.clone(),
            parent_id: body.parent_id.clone(),
            civil_code: body.civil_code.clone(),
            business_group: body.business_group.clone(),
            platform_id: body.platform_id,
        };
        let Json(_) = catalog_add(State(state.clone()), Json(add_body)).await?;
        return Ok(Json(serde_json::json!({ "code": 0, "msg": "目录编辑成功" })));
    }
    // 保留 Option：`unwrap_or_default()` 会把没传的字段变成**空串**，而空串不是
    // NULL —— `COALESCE(?, col)` 于是把库里已有的值清空（编辑弹窗只要没带
    // `parentId` 就把上级目录清掉）。
    let name = body.name.clone();
    let parent = body.parent_id.clone();
    let civil_code = body.civil_code.clone();
    let business_group = body.business_group.clone();
    let platform_id = body.platform_id;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let sql = if cfg!(feature = "postgres") {
        "UPDATE gb_platform_catalog SET name = COALESCE($1, name), parent = COALESCE($2, parent), \
         civil_code = COALESCE($3, civil_code), business_group = COALESCE($4, business_group), \
         platform_id = COALESCE($5, platform_id), update_time = $6 WHERE id = $7"
    } else {
        "UPDATE gb_platform_catalog SET name = COALESCE(?, name), parent = COALESCE(?, parent), \
         civil_code = COALESCE(?, civil_code), business_group = COALESCE(?, business_group), \
         platform_id = COALESCE(?, platform_id), update_time = ? WHERE id = ?"
    };
    let affected = sqlx::query(sql)
        .bind(name.as_deref())
        .bind(parent.as_deref())
        .bind(civil_code.as_deref())
        .bind(business_group.as_deref())
        .bind(platform_id)
        .bind(&now)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("目录更新失败: {}", e)))?
        .rows_affected();

    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("目录不存在: id={}", id),
        ));
    }
    if let Some(pid) = platform_id.filter(|v| *v > 0) {
        if let Err(e) = refresh_platform_catalog(&state, pid).await {
            tracing::warn!("目录已更新，但刷新上级平台目录失败: {}", e);
        }
    }
    Ok(Json(serde_json::json!({
        "code": 0,
        "msg": "目录编辑成功",
        "affected": affected,
    })))
}

/// GET /api/platform/info/:id
pub async fn platform_info(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    let pid = id.parse::<i64>().unwrap_or(0);
    match crate::db::platform::get_by_id(&state.pool, pid).await {
        // 与列表共用同一份 JSON（此前只回 10 个字段，编辑弹窗按完整类型取值时
        // serverGBDomain / expires / keepTimeout 等全是 undefined）
        Ok(Some(p)) => {
            let channel_count = platform_channel::count_by_platform_id(&state.pool, p.id as i64)
                .await
                .unwrap_or(0);
            Json(WVPResult::success(platform_row_json(&p, channel_count)))
        }
        Ok(None) => Json(WVPResult::error("Platform not found")),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

#[cfg(test)]
mod platform_contract_tests {
    use super::*;
    use crate::test_support::app_state;

    fn add_body(v: serde_json::Value) -> PlatformAddBody {
        serde_json::from_value(v).expect("PlatformAddBody 反序列化")
    }

    /// 前端（与 WVP）用的是 `serverGBId`；历史上还出现过 `serverGbId` 的错误拼写。
    /// 三种写法都必须能绑上，否则库里写的是空串 —— 接口报成功、平台却不可用。
    #[test]
    fn add_body_binds_server_gb_id_and_realm_and_heartbeat() {
        let b = add_body(serde_json::json!({
            "name": "上级", "serverGBId": "34020000002000000009",
            "serverGBDomain": "3402000000", "serverIp": "10.0.0.9", "serverPort": 5060
        }));
        assert_eq!(b.server_gb_id.as_deref(), Some("34020000002000000009"));
        assert_eq!(b.server_gb_domain.as_deref(), Some("3402000000"));

        // 旧错误拼写仍然收下
        let legacy = add_body(serde_json::json!({
            "name": "x", "serverGbId": "34020000002000000008", "realm": "3402000001"
        }));
        assert_eq!(legacy.server_gb_id.as_deref(), Some("34020000002000000008"));
        assert_eq!(legacy.server_gb_domain.as_deref(), Some("3402000001"));

        // 心跳周期：WVP 的 keepTimeout 与旧前端的 heartBeatInterval 都要认
        let hb = add_body(serde_json::json!({"name": "x", "heartBeatInterval": 45}));
        assert_eq!(hb.keep_timeout.as_deref(), Some("45"));
        let kt = add_body(serde_json::json!({"name": "x", "keepTimeout": "90"}));
        assert_eq!(kt.keep_timeout.as_deref(), Some("90"));
    }

    /// `expires` 在 WVP 里是 int，Vue3 的 el-input-number 输出 number，
    /// 而这一列是 varchar —— 只认字符串会 422（请求进不到 handler）。
    #[test]
    fn add_body_accepts_numeric_and_string_expires() {
        let numeric = add_body(serde_json::json!({"name": "x", "expires": 3600}));
        assert_eq!(numeric.expires.as_deref(), Some("3600"));
        let stringy = add_body(serde_json::json!({"name": "x", "expires": "1800"}));
        assert_eq!(stringy.expires.as_deref(), Some("1800"));
        let missing = add_body(serde_json::json!({"name": "x"}));
        assert_eq!(missing.expires, None);

        let bad = serde_json::from_value::<PlatformAddBody>(serde_json::json!({
            "name": "x", "expires": {"nope": 1}
        }));
        assert!(bad.is_err(), "对象既不是数字也不是字符串，应报错而不是静默丢弃");
    }

    #[test]
    fn int_or_string_emits_numbers_for_numeric_columns() {
        assert_eq!(int_or_string(&Some("3600".to_string())), serde_json::json!(3600));
        assert_eq!(
            int_or_string(&Some("weird".to_string())),
            serde_json::json!("weird")
        );
        assert_eq!(int_or_string(&None), serde_json::Value::Null);
    }

    /// 新增：必填校验 + 重复国标ID 拒绝（此前空 server_gb_id 也回"成功"）。
    #[tokio::test]
    async fn platform_add_validates_and_persists_real_fields() {
        let state = app_state().await;

        let missing_gb = platform_add(
            State(state.clone()),
            Json(add_body(serde_json::json!({"name": "没有国标ID", "serverIp": "10.0.0.1"}))),
        )
        .await
        .expect_err("缺国标ID 应报错");
        assert!(matches!(missing_gb, AppError::Business(_, _)));

        let missing_ip = platform_add(
            State(state.clone()),
            Json(add_body(serde_json::json!({
                "name": "x", "serverGBId": "34020000002000000009"
            }))),
        )
        .await
        .expect_err("缺 IP 应报错");
        assert!(matches!(missing_ip, AppError::Business(_, _)));

        let created = platform_add(
            State(state.clone()),
            Json(add_body(serde_json::json!({
                "name": "上级平台", "serverGBId": "34020000002000000009",
                "serverGBDomain": "3402000000", "serverIp": "10.0.0.9", "serverPort": 5060,
                "deviceGBId": "34020000001320000001", "username": "u", "password": "p",
                "transport": "TCP", "expires": 1800, "keepTimeout": 45,
                "civilCode": "340200", "enable": true, "autoPushChannel": true
            }))),
        )
        .await
        .expect("新增平台");
        assert_eq!(created.0.code, 0, "msg={}", created.0.msg);

        let row = platform_db::get_by_server_gb_id(&state.pool, "34020000002000000009")
            .await
            .unwrap()
            .expect("库里有行");
        assert_eq!(row.server_gb_domain.as_deref(), Some("3402000000"));
        assert_eq!(row.server_ip.as_deref(), Some("10.0.0.9"));
        assert_eq!(row.device_gb_id.as_deref(), Some("34020000001320000001"));
        assert_eq!(row.transport.as_deref(), Some("TCP"));
        assert_eq!(row.expires.as_deref(), Some("1800"));
        assert_eq!(row.keep_timeout.as_deref(), Some("45"));
        assert_eq!(row.civil_code.as_deref(), Some("340200"));
        assert_eq!(row.enable, Some(true));

        let dup = platform_add(
            State(state.clone()),
            Json(add_body(serde_json::json!({
                "name": "重复", "serverGBId": "34020000002000000009", "serverIp": "10.0.0.2"
            }))),
        )
        .await
        .expect_err("重复国标ID 应报错");
        assert!(matches!(dup, AppError::Business(_, _)));
    }

    /// 列表与详情必须给出同一套 camelCase 键（此前详情只有 10 个字段）。
    #[tokio::test]
    async fn platform_list_and_info_share_the_same_keys() {
        let state = app_state().await;
        let _ = platform_add(
            State(state.clone()),
            Json(add_body(serde_json::json!({
                "name": "p1", "serverGBId": "34020000002000000007",
                "serverGBDomain": "3402000000", "serverIp": "10.0.0.7",
                "expires": 900, "keepTimeout": 30
            }))),
        )
        .await
        .unwrap();

        let id = platform_db::get_by_server_gb_id(&state.pool, "34020000002000000007")
            .await
            .unwrap()
            .unwrap()
            .id;
        let info = platform_info(State(state.clone()), Path(id.to_string())).await;
        let data = info.0.data.expect("info data");
        assert_eq!(data["serverGBId"], "34020000002000000007");
        assert_eq!(data["serverGBDomain"], "3402000000");
        // expires / keepTimeout 回的是数字（与 WVP 的 int 字段一致）
        assert_eq!(data["expires"], 900);
        assert_eq!(data["keepTimeout"], 30);
        // 不能同时回同义键，否则前端回提交时会 "duplicate field" 422
        assert!(data.get("heartBeatInterval").is_none());
        assert!(data.get("channelCount").is_some());

        let list = platform_query(
            State(state.clone()),
            Query(PlatformQuery { page: Some(1), count: Some(10), query: None }),
        )
        .await
        .unwrap();
        let body = list.0.data.unwrap();
        let first = &body["list"][0];
        for key in [
            "serverGBId",
            "serverGBDomain",
            "expires",
            "keepTimeout",
            "channelCount",
            "civilCode",
            "autoPushChannel",
        ] {
            assert_eq!(first.get(key), data.get(key), "列表与详情的 {key} 应一致");
        }

        // 回提交护栏：编辑弹窗会把整行原样 POST 回 /platform/update。
        // 只要响应用了两个互为别名的键（例如 keepTimeout + heartBeatInterval），
        // serde 就会报 "duplicate field" → 更新稳定 422。
        serde_json::from_value::<PlatformAddBody>(first.clone()).expect(
            "列表行必须能原样反序列化成 PlatformAddBody（否则编辑保存必 422）",
        );
    }

    /// 注销：按 serverGBId 定位（此前按 device_gb_id 查、只回布尔，且无任何动作）。
    #[tokio::test]
    async fn platform_exit_unregisters_by_server_gb_id() {
        let state = app_state().await;
        let _ = platform_add(
            State(state.clone()),
            Json(add_body(serde_json::json!({
                "name": "p2", "serverGBId": "34020000002000000006",
                "serverIp": "10.0.0.6", "enable": true
            }))),
        )
        .await
        .unwrap();

        let missing = platform_exit(State(state.clone()), Path("99999999999999999999".to_string()))
            .await
            .expect_err("不存在的平台应 404");
        assert!(matches!(missing, AppError::Business(_, _)));

        let exited = platform_exit(
            State(state.clone()),
            Path("34020000002000000006".to_string()),
        )
        .await
        .expect("注销");
        let data = exited.0.data.unwrap();
        assert_eq!(data["exited"], true);
        assert_eq!(data["serverGBId"], "34020000002000000006");

        let row = platform_db::get_by_server_gb_id(&state.pool, "34020000002000000006")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.enable, Some(false), "注销后必须停用，否则下一个周期会注册回来");
        assert_eq!(row.status, Some(false));
    }

    /// 目录：前端提交的是 `platformId`/`parentId`/`civilCode`/`businessGroup`；
    /// 编辑时**未提供的字段不能被空串清掉**。
    #[tokio::test]
    async fn catalog_add_edit_use_frontend_field_names_without_clobbering() {
        let state = app_state().await;
        let add: CatalogAddBody = serde_json::from_value(serde_json::json!({
            "platformId": 7, "name": "目录A", "parentId": "0",
            "civilCode": "340200", "businessGroup": "1"
        }))
        .unwrap();
        assert_eq!(add.platform_id, Some(7));
        assert_eq!(add.parent_id.as_deref(), Some("0"));
        assert_eq!(add.civil_code.as_deref(), Some("340200"));
        assert_eq!(add.business_group.as_deref(), Some("1"));

        let _ = catalog_add(State(state.clone()), Json(add)).await.unwrap();
        let catalog_id: i64 = sqlx::query_scalar("SELECT id FROM gb_platform_catalog LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();

        // 只改名字：parent / civil_code / business_group 应保持原值
        let edit: CatalogAddBodyEdit = serde_json::from_value(serde_json::json!({
            "id": catalog_id, "name": "目录A-改名"
        }))
        .unwrap();
        assert_eq!(edit.parent_id, None);
        let _ = catalog_edit(State(state.clone()), Json(edit)).await.unwrap();

        let (name, parent, civil): (String, String, String) = sqlx::query_as(
            "SELECT name, parent, civil_code FROM gb_platform_catalog WHERE id = ?",
        )
        .bind(catalog_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(name, "目录A-改名");
        assert_eq!(parent, "0", "未提供的 parent 不能被空串覆盖");
        assert_eq!(civil, "340200", "未提供的 civil_code 不能被空串覆盖");
    }
}
