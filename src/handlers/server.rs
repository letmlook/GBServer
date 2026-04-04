//! 流媒体服务器与系统配置 API，与前端 server.js 对应

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{get_media_server_by_id, list_media_servers, media_server, MediaServer};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;

use crate::AppState;

use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use std::collections::HashMap;
use std::str::FromStr;
use crate::db as db;
use crate::db::position_history as ph;

// Helper functions to read /proc based system metrics (no new deps)
use std::fs::File;
use std::io::{Read as _Read};
use std::time::Duration;
use tokio::time::sleep;

async fn read_cpu_usage() -> Option<f64> {
    let (t1, id1) = read_cpu_times()?;
    sleep(Duration::from_millis(60)).await;
    let (t2, id2) = read_cpu_times()?;
    let dt = t2.saturating_sub(t1) as f64;
    let di = id2.saturating_sub(id1) as f64;
    if dt <= 0.0 {
        return Some(0.0);
    }
    Some(((dt - di) / dt) * 100.0)
}

fn read_cpu_times() -> Option<(u64, u64)> {
    let mut s = String::new();
    File::open("/proc/stat").ok()?.read_to_string(&mut s).ok()?;
    for line in s.lines() {
        if line.starts_with("cpu ") {
            let mut parts = line.split_whitespace();
            // skip the first token 'cpu'
            parts.next();
            let mut vals = Vec::new();
            for p in parts { if let Ok(n) = p.parse::<u64>() { vals.push(n); } }
            let user = *vals.get(0).unwrap_or(&0);
            let nice = *vals.get(1).unwrap_or(&0);
            let system = *vals.get(2).unwrap_or(&0);
            let idle = *vals.get(3).unwrap_or(&0);
            let iowait = *vals.get(4).unwrap_or(&0);
            let total = user + nice + system + idle + iowait + *vals.get(5).unwrap_or(&0) + *vals.get(6).unwrap_or(&0) + *vals.get(7).unwrap_or(&0);
            return Some((total, idle + iowait));
        }
    }
    None
}

fn read_memory_info() -> Option<(u64, u64, u64)> {
    let mut s = String::new();
    File::open("/proc/meminfo").ok()?.read_to_string(&mut s).ok()?;
    let mut mem_total: Option<u64> = None;
    let mut mem_available: Option<u64> = None;
    for line in s.lines() {
        if line.starts_with("MemTotal:") {
            mem_total = line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok());
        } else if line.starts_with("MemAvailable:") {
            mem_available = line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok());
        }
    }
    let total = mem_total.unwrap_or(0);
    let avail = mem_available.unwrap_or(0);
    let used = total.saturating_sub(avail);
    Some((total, avail, used))
}

fn read_disk_usage() -> Option<(u64, u64, u64)> {
    // Use df -k / to approximate root disk usage
    use std::process::Command;
    let output = Command::new("df").arg("-k").arg("/").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let out = String::from_utf8_lossy(&output.stdout);
    for (idx, line) in out.lines().enumerate() {
        if idx == 1 { // second line contains root
            let mut it = line.split_whitespace();
            // Filesystem 1K-blocks Used Available Use%
            let _fs = it.next();
            let total = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let used = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let _avail = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            return Some((total, used, 0));
        }
    }
    None
}

fn read_uptime() -> Option<f64> {
    let mut s = String::new();
    File::open("/proc/uptime").ok()?.read_to_string(&mut s).ok()?;
    let mut parts = s.split_whitespace();
    let up = parts.next().and_then(|v| v.parse::<f64>().ok())?;
    Some(up)
}

/// GET /api/server/media_server/list
pub async fn media_server_list(State(state): State<AppState>) -> Result<Json<WVPResult<Vec<MediaServer>>>, AppError> {
    let list = list_media_servers(&state.pool).await?;
    Ok(Json(WVPResult::success(list)))
}

/// GET /api/server/media_server/online/list — 与 list 同结构，可过滤在线（当前返回全部）
pub async fn media_server_online_list(State(state): State<AppState>) -> Result<Json<WVPResult<Vec<MediaServer>>>, AppError> {
    let list = list_media_servers(&state.pool).await?;
    Ok(Json(WVPResult::success(list)))
}

/// GET /api/server/media_server/one/:id
pub async fn media_server_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WVPResult<MediaServer>>, AppError> {
    let one = get_media_server_by_id(&state.pool, &id).await?;
    let one = one.ok_or_else(|| crate::error::AppError::business(crate::error::ErrorCode::Error404, "流媒体不存在"))?;
    Ok(Json(WVPResult::success(one)))
}

/// GET /api/server/system/configInfo
pub async fn system_config_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let cfg = &state.config;
    // SIP config
    let sip_cfg = cfg.sip.as_ref();
    let sip_json = if let Some(s) = sip_cfg {
        serde_json::to_value(s).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    // ZLM config (simplified representation)
    let zlm_json = if let Some(z) = &cfg.zlm {
        let servers: Vec<_> = z.servers.iter().map(|sv| {
            json!({
                "id": sv.id,
                "ip": sv.ip,
                "http_port": sv.http_port,
                "https_port": sv.https_port,
                "enabled": sv.enabled,
            })
        }).collect();
        json!({
            "servers": servers,
            "stream_timeout": z.stream_timeout,
            "hook_enabled": z.hook_enabled,
            "hook_url": z.hook_url,
        })
    } else {
        serde_json::Value::Null
    };

    // Database type from URL
    let db_type = {
        let url = &cfg.database.url;
        if url.starts_with("postgres") { "postgres" } else if url.starts_with("mysql") { "mysql" } else { "unknown" }
    };

    let data = json!({
        "sip": sip_json,
        "zlm": zlm_json,
        "database": {"type": db_type},
        "version": env!("CARGO_PKG_VERSION"),
        "build": env!("CARGO_PKG_NAME"),
    });
    Json(WVPResult::success(data))
}

/// GET /api/server/system/info
pub async fn system_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // CPU usage: read /proc/stat twice to compute usage
    let cpu_usage = match read_cpu_usage().await {
        Some(v) => v,
        None => 0.0,
    };

    // Memory usage from /proc/meminfo
    let mem = read_memory_info().unwrap_or((0u64, 0u64, 0u64));
    let mem_total_kb = mem.0;
    let mem_available_kb = mem.1;
    let mem_used_kb = if mem_total_kb > mem_available_kb { mem_total_kb - mem_available_kb } else { 0 };
    let mem_usage_pct = if mem_total_kb > 0 { (mem_used_kb as f64 / mem_total_kb as f64) * 100.0 } else { 0.0 };

    // Disk usage for root
    let disk_usage = read_disk_usage().unwrap_or((0u64, 0u64, 0u64));
    let (disk_total_kb, disk_used_kb) = (disk_usage.0, disk_usage.1);
    let disk_usage_pct = if disk_total_kb > 0 { (disk_used_kb as f64 / disk_total_kb as f64) * 100.0 } else { 0.0 };

    // Uptime in seconds from /proc/uptime
    let uptime_secs = read_uptime().unwrap_or(0.0) as u64;

    Json(WVPResult::success(serde_json::json!({
        "cpu": {"usage_percent": cpu_usage},
        "memory": {
            "total_kb": mem_total_kb,
            "used_kb": mem_used_kb,
            "usage_percent": mem_usage_pct
        },
        "disk": { "root": { "total_kb": disk_total_kb, "used_kb": disk_used_kb, "usage_percent": disk_usage_pct } },
        "uptime_seconds": uptime_secs
    })))
}

/// GET /api/server/map/config
pub async fn map_config() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({})))
}

/// GET /api/server/info
pub async fn server_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // Persist a simple start time reference via a static OnceLock
    use std::sync::OnceLock;
    static START_TIME: OnceLock<SystemTime> = OnceLock::new();
    let start = START_TIME.get_or_init(|| SystemTime::now());
    let start_ts = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let uptime = read_uptime().unwrap_or(0.0) as u64;
    Json(WVPResult::success(serde_json::json!({
        "start_time": start_ts,
        "uptime_seconds": uptime,
        "version": env!("CARGO_PKG_VERSION"),
        "build": env!("CARGO_PKG_NAME"),
    })))
}

/// GET /api/server/resource/info
pub async fn resource_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // Simple counts from DB
    let total_devices = db::count_devices(&state.pool, None, None).await.unwrap_or(0);
    let online_devices = db::count_devices(&state.pool, None, Some(true)).await.unwrap_or(0);
    let total_channels = db::count_all_channels(&state.pool).await.unwrap_or(0);
    let online_channels = db::count_online_channels(&state.pool).await.unwrap_or(0);
    let active_streams = 
        db::stream_proxy::count_all(&state.pool, None, Some(true)).await.unwrap_or(0);

    Json(WVPResult::success(serde_json::json!({
        "total_devices": total_devices,
        "online_devices": online_devices,
        "total_channels": total_channels,
        "online_channels": online_channels,
        "active_streams": active_streams,
    })))
}

// ---------- 占位：前端调用避免 404 ----------
/// GET /api/server/media_server/check
pub async fn media_server_check() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(true)))
}

/// GET /api/server/media_server/record/check
pub async fn media_server_record_check() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(true)))
}

/// POST /api/server/media_server/save - 添加或更新媒体服务器
#[derive(Debug, Deserialize)]
pub struct MediaServerSaveBody {
    pub id: Option<String>,
    pub ip: Option<String>,
    pub hook_ip: Option<String>,
    pub http_port: Option<i32>,
}

pub async fn media_server_save(
    State(state): State<AppState>,
    Json(body): Json<MediaServerSaveBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or_else(|| format!("media_server_{}", chrono::Utc::now().timestamp_millis()));
    let ip = body.ip.unwrap_or_else(|| "127.0.0.1".to_string());
    let http_port = body.http_port.unwrap_or(8080);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    // 检查是否已存在
    let existing = get_media_server_by_id(&state.pool, &id).await?;
    
    if existing.is_some() {
        media_server::update(
            &state.pool,
            &id,
            Some(&ip),
            body.hook_ip.as_deref(),
            Some(http_port),
            &now,
        ).await?;
    } else {
        // 添加
        media_server::add(
            &state.pool,
            &id,
            &ip,
            http_port,
            &now,
        ).await?;
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "保存成功"
    }))))
}

/// DELETE /api/server/media_server/delete
pub async fn media_server_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    media_server::delete_by_id(&state.pool, &id).await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "message": "删除成功"
    }))))
}

/// GET /api/server/media_server/media_info
#[derive(Debug, Deserialize)]
pub struct MediaInfoQuery {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub mediaServerId: Option<String>,
}

pub async fn media_server_media_info(
    State(state): State<AppState>,
    Query(q): Query<MediaInfoQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let app = q.app.as_deref().unwrap_or("");
    let stream = q.stream.as_deref().unwrap_or("");
    if app.is_empty() || stream.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 app 或 stream 参数"));
    }

    // 选择 ZLM 客户端
    let zlm_client = if let Some(ref ms_id) = q.mediaServerId {
        state.get_zlm_client(Some(ms_id)).clone()
    } else {
        state.get_zlm_client(None)
    };

    let client = zlm_client.ok_or_else(|| {
        AppError::business(ErrorCode::Error404, "未配置 ZLM 客户端或媒体服务器不存在")
    })?;

    // 常见默认参数：rtmp, __defaultVhost__
    let schema = "rtmp";
    let vhost = "__defaultVhost__";

    match client.get_media_info(schema, vhost, app, stream).await {
        Ok(Some(info)) => {
            let value = serde_json::to_value(info).unwrap_or(serde_json::Value::Null);
            Ok(Json(WVPResult::success(value)))
        }
        Ok(None) => Ok(Json(WVPResult::success(serde_json::Value::Null))),
        Err(e) => Err(AppError::business(ErrorCode::Error500, format!("ZLM 请求失败: {}", e))),
    }
}

/// GET /api/server/media_server/load
pub async fn media_server_load(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    if let Some(ref zlm) = state.zlm_client {
        let cfg_map = zlm.get_server_config().await.unwrap_or_default();
        let stats_map = zlm.get_server_stats().await.unwrap_or_default();
        let mut data = serde_json::Map::new();
        data.insert("config".to_string(), serde_json::to_value(cfg_map).unwrap_or(serde_json::Value::Null));
        data.insert("stats".to_string(), serde_json::to_value(stats_map).unwrap_or(serde_json::Value::Null));
        Json(WVPResult::success(serde_json::Value::Object(data)))
    } else {
        Json(WVPResult::success(serde_json::json!(null)))
    }
}

/// GET /api/server/map/model-icon/list
pub async fn map_model_icon_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    // 参考实现：WVP 在此接口通常返回图标配置。当前实现保持向后兼容，返回空列表。
    Json(WVPResult::success(vec![]))
}
