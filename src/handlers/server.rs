//! 流媒体服务器与系统配置 API，与前端 server.js 对应

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::db::{get_media_server_by_id, list_media_servers, media_server, MediaServer};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;

use crate::AppState;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use std::str::FromStr;
use crate::db as db;

// Helper functions to read system metrics
use std::fs::File;
use std::io::{Read as _Read};
use std::time::Duration;
use tokio::time::sleep;

pub async fn zlm_proxy(
    method: Method,
    State(state): State<AppState>,
    Path((media_server_id, path)): Path<(String, String)>,
    Query(mut params): Query<HashMap<String, String>>,
    body: Bytes,
) -> Response {
    let Some(client) = state.get_zlm_client(Some(&media_server_id)) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "code": 404,
                "msg": format!("媒体服务不存在: {}", media_server_id),
                "data": null
            })),
        )
            .into_response();
    };

    params
        .entry("secret".to_string())
        .or_insert_with(|| client.secret.clone());

    let target = format!("{}/{}", client.base_url().trim_end_matches('/'), path);
    let http = reqwest::Client::new();
    let req = if method == Method::POST {
        http.post(&target).query(&params).body(body)
    } else {
        http.get(&target).query(&params)
    };
    let resp = match req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("ZLM proxy request failed: {} -> {}", target, e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "code": 502,
                    "msg": format!("ZLM请求失败: {}", e),
                    "data": null
                })),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| header::HeaderValue::from_str(value).ok())
        .unwrap_or_else(|| header::HeaderValue::from_static("application/json"));
    let body = match resp.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "code": 502,
                    "msg": format!("读取ZLM响应失败: {}", e),
                    "data": null
                })),
            )
                .into_response();
        }
    };

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

async fn configure_zlm_hooks(
    state: &AppState,
    media_server_id: &str,
    client: &crate::zlm::ZlmClient,
) -> Vec<String> {
    let hook_url = state
        .config
        .zlm
        .as_ref()
        .and_then(|cfg| {
            cfg.servers
                .iter()
                .find(|server| server.id == media_server_id)
                .and_then(|server| server.hook_url.clone())
                .or_else(|| Some(cfg.hook_url.clone()))
        })
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| format!("http://127.0.0.1:{}/api/zlm/hook", state.config.server.port));

    let config_items = [
        ("hook.enable", "1".to_string()),
        ("hook.on_server_started", hook_url.clone()),
        ("hook.on_server_keepalive", hook_url.clone()),
        ("hook.on_stream_changed", hook_url.clone()),
        ("hook.on_stream_not_found", hook_url.clone()),
        ("hook.on_record_mp4", hook_url.clone()),
        ("hook.on_record_hls", hook_url.clone()),
        ("hook.on_publish", hook_url.clone()),
        ("hook.on_play", hook_url.clone()),
        ("hook.on_flow_report", hook_url.clone()),
        ("hook.on_rtp_server_timeout", hook_url.clone()),
    ];

    let mut errors = Vec::new();
    for (key, value) in config_items {
        if let Err(e) = client.set_server_config(&client.secret, key, &value).await {
            let msg = format!("{}={}: {}", key, value, e);
            tracing::warn!("Failed to configure ZLM hook {}", msg);
            errors.push(msg);
        }
    }
    errors
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "windows")]
async fn read_cpu_usage() -> Option<f64> {
    // Return a dummy value for Windows
    Some(5.0)
}

#[cfg(target_os = "windows")]
fn read_cpu_times() -> Option<(u64, u64)> {
    None
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "windows")]
fn read_memory_info() -> Option<(u64, u64, u64)> {
    // Return dummy values for Windows
    Some((16 * 1024 * 1024, 8 * 1024 * 1024, 8 * 1024 * 1024))
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "windows")]
fn read_disk_usage() -> Option<(u64, u64, u64)> {
    // Return dummy values for Windows
    Some((100 * 1024 * 1024, 50 * 1024 * 1024, 50 * 1024 * 1024))
}

#[cfg(target_os = "linux")]
fn read_uptime() -> Option<f64> {
    let mut s = String::new();
    File::open("/proc/uptime").ok()?.read_to_string(&mut s).ok()?;
    let mut parts = s.split_whitespace();
    let up = parts.next().and_then(|v| v.parse::<f64>().ok())?;
    Some(up)
}

#[cfg(target_os = "windows")]
fn read_uptime() -> Option<f64> {
    // Return dummy value for Windows
    Some(3600.0)
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
        json!({
            "enabled": s.enabled,
            "ip": s.ip,
            "port": s.port,
            "tcpPort": s.tcp_port,
            "deviceId": s.device_id,
            "realm": s.realm,
            "keepaliveTimeout": s.keepalive_timeout,
            "registerTimeout": s.register_timeout,
            "charset": s.charset,
        })
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
    // Simplified implementation for Windows
    let now = "2026-04-04 11:00:00";

    // Format data as arrays for frontend charts
    let cpu_data = vec![
        serde_json::json!([now, 0.05])
    ];

    let mem_data = vec![
        serde_json::json!([now, 0.5])
    ];

    let disk_data = vec![
        serde_json::json!("总空间"),
        serde_json::json!(100),
        serde_json::json!("已用"),
        serde_json::json!(50),
        serde_json::json!("可用"),
        serde_json::json!(50)
    ];

    let net_data = vec![
        serde_json::json!([now, 1.0]), // 1MB/s
        serde_json::json!([now, 0.5])  // 0.5MB/s
    ];

    let net_total_data = vec![
        serde_json::json!("入网"),
        serde_json::json!("出网")
    ];

    let data = serde_json::json!({
        "cpu": cpu_data,
        "mem": mem_data,
        "disk": disk_data,
        "net": net_data,
        "netTotal": net_total_data,
        "uptime": 3600,
        "cpu_usage": 5.0,
        "mem_usage": 50.0,
        "disk_usage": 50.0,
    });
    Json(WVPResult::success(data))
}

/// GET /api/server/map/config
pub async fn map_config(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let map_cfg = &state.config.map;
    Json(WVPResult::success(serde_json::json!({
        "tiandituKey": map_cfg.as_ref().and_then(|m| m.tianditu_key.clone()).unwrap_or_default(),
        "centerLng": map_cfg.as_ref().and_then(|m| m.center_lng).unwrap_or(116.397428),
        "centerLat": map_cfg.as_ref().and_then(|m| m.center_lat).unwrap_or(39.90923),
        "zoom": map_cfg.as_ref().and_then(|m| m.zoom).unwrap_or(12),
        "coordSys": map_cfg.as_ref().and_then(|m| m.coord_sys.clone()).unwrap_or_else(|| "WGS84".to_string()),
    })))
}

/// GET /api/server/info
pub async fn server_info(State(_state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // Persist a simple start time reference via a static OnceLock
    use std::sync::OnceLock;
    static START_TIME: OnceLock<SystemTime> = OnceLock::new();
    let start = START_TIME.get_or_init(|| SystemTime::now());
    let start_ts = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    // Simple uptime calculation from start time
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let uptime = now.saturating_sub(start_ts);
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

    // Format data as array for frontend
    let resource_data = vec![
        serde_json::json!("总设备数"),
        serde_json::json!(total_devices),
        serde_json::json!("在线设备数"),
        serde_json::json!(online_devices),
        serde_json::json!("总通道数"),
        serde_json::json!(total_channels),
        serde_json::json!("在线通道数"),
        serde_json::json!(online_channels),
        serde_json::json!("活跃流数"),
        serde_json::json!(active_streams)
    ];

    Json(WVPResult::success(serde_json::Value::Array(resource_data)))
}

// ---------- 占位：前端调用避免 404 ----------
/// GET /api/server/media_server/check
#[derive(Debug, Deserialize)]
pub struct MediaServerCheckQuery {
    pub ip: Option<String>,
    #[serde(alias = "httpPort")]
    pub port: Option<i32>,
    pub secret: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
}

pub async fn media_server_check(
    State(state): State<AppState>,
    Query(q): Query<MediaServerCheckQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let ip = q.ip.unwrap_or_else(|| "127.0.0.1".to_string());
    let http_port = q.port.unwrap_or(80);
    let secret = q.secret.unwrap_or_default();
    let type_ = q.type_.unwrap_or_else(|| "zlm".to_string());
    let temp_client = crate::zlm::ZlmClient::new(&ip, http_port as u16, &secret);

    let mut payload = serde_json::json!({
        "ip": ip,
        "httpPort": http_port,
        "secret": secret,
        "type": type_,
        "autoConfig": true,
        "rtpEnable": false,
        "rtpProxyPort": 30000,
        "rtpPortRange": "30000,30500",
        "sendRtpPortRange": "50000,60000"
    });

    if let Ok(configs) = temp_client.get_server_config().await {
        if let Some(obj) = payload.as_object_mut() {
            let get_i32 = |key: &str| configs.get(key).and_then(|v| i32::from_str(v).ok());
            let get_bool = |key: &str| {
                configs
                    .get(key)
                    .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            };
            obj.insert(
                "hookIp".to_string(),
                json!(configs.get("hook.hookIp").cloned().unwrap_or_default()),
            );
            obj.insert(
                "sdpIp".to_string(),
                json!(configs.get("rtp_proxy.sdp_ip").cloned().unwrap_or_default()),
            );
            obj.insert(
                "streamIp".to_string(),
                json!(configs.get("general.streamNoneReaderDelayMS").cloned().unwrap_or_default()),
            );
            obj.insert("httpSSlPort".to_string(), json!(get_i32("http.sslport").unwrap_or(443)));
            obj.insert("rtmpPort".to_string(), json!(get_i32("rtmp.port").unwrap_or(1935)));
            obj.insert("rtmpSSlPort".to_string(), json!(get_i32("rtmp.sslport").unwrap_or(0)));
            obj.insert("rtspPort".to_string(), json!(get_i32("rtsp.port").unwrap_or(554)));
            obj.insert("rtspSSLPort".to_string(), json!(get_i32("rtsp.sslport").unwrap_or(0)));
            obj.insert(
                "recordAssistPort".to_string(),
                json!(get_i32("record.port").unwrap_or(0)),
            );
            obj.insert(
                "rtpEnable".to_string(),
                json!(get_bool("rtp_proxy.port_range").unwrap_or(false)),
            );
        }
    }

    if let Some(obj) = payload.as_object_mut() {
        if obj
            .get("streamIp")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .is_empty()
        {
            obj.insert(
                "streamIp".to_string(),
                json!(
                    state
                        .config
                        .zlm
                        .as_ref()
                        .and_then(|cfg| cfg.servers.first().map(|sv| sv.ip.clone()))
                        .unwrap_or_else(|| "127.0.0.1".to_string())
                ),
            );
        }
    }

    Json(WVPResult::success(payload))
}

/// GET /api/server/media_server/record/check
#[derive(Debug, Deserialize)]
pub struct MediaServerRecordCheckQuery {
    pub ip: Option<String>,
    pub port: Option<i32>,
}

pub async fn media_server_record_check(
    State(state): State<AppState>,
    Query(q): Query<MediaServerRecordCheckQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let reachable = state
        .zlm_client
        .as_ref()
        .map(|_| true)
        .unwrap_or(false);
    Json(WVPResult::success(serde_json::json!({
        "success": reachable,
        "ip": q.ip,
        "port": q.port,
        "recordAssistPort": q.port.unwrap_or(0),
    })))
}

/// POST /api/server/media_server/save - 添加或更新媒体服务器
#[derive(Debug, Deserialize)]
pub struct MediaServerSaveBody {
    pub id: Option<String>,
    pub ip: Option<String>,
    #[serde(alias = "hookIp")]
    pub hook_ip: Option<String>,
    #[serde(alias = "sdpIp")]
    pub sdp_ip: Option<String>,
    #[serde(alias = "streamIp")]
    pub stream_ip: Option<String>,
    #[serde(alias = "httpPort")]
    pub http_port: Option<i32>,
    #[serde(alias = "httpSSlPort")]
    pub http_ssl_port: Option<i32>,
    pub secret: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(alias = "autoConfig")]
    pub auto_config: Option<bool>,
    #[serde(alias = "rtmpPort")]
    pub rtmp_port: Option<i32>,
    #[serde(alias = "rtmpSSlPort")]
    pub rtmp_ssl_port: Option<i32>,
    #[serde(alias = "rtspPort")]
    pub rtsp_port: Option<i32>,
    #[serde(alias = "rtspSSLPort")]
    pub rtsp_ssl_port: Option<i32>,
    #[serde(alias = "rtpEnable")]
    pub rtp_enable: Option<bool>,
    #[serde(alias = "rtpPortRange")]
    pub rtp_port_range: Option<String>,
    #[serde(alias = "sendRtpPortRange")]
    pub send_rtp_port_range: Option<String>,
    #[serde(alias = "rtpProxyPort")]
    pub rtp_proxy_port: Option<i32>,
    #[serde(alias = "recordAssistPort")]
    pub record_assist_port: Option<i32>,
    #[serde(alias = "defaultServer")]
    pub default_server: Option<bool>,
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

    #[cfg(feature = "postgres")]
    sqlx::query(
        r#"UPDATE wvp_media_server SET
           hook_ip = COALESCE($1, hook_ip),
           sdp_ip = COALESCE($2, sdp_ip),
           stream_ip = COALESCE($3, stream_ip),
           http_ssl_port = COALESCE($4, http_ssl_port),
           secret = COALESCE($5, secret),
           type = COALESCE($6, type),
           auto_config = COALESCE($7, auto_config),
           rtmp_port = COALESCE($8, rtmp_port),
           rtmp_ssl_port = COALESCE($9, rtmp_ssl_port),
           rtsp_port = COALESCE($10, rtsp_port),
           rtsp_ssl_port = COALESCE($11, rtsp_ssl_port),
           rtp_enable = COALESCE($12, rtp_enable),
           rtp_port_range = COALESCE($13, rtp_port_range),
           send_rtp_port_range = COALESCE($14, send_rtp_port_range),
           rtp_proxy_port = COALESCE($15, rtp_proxy_port),
           record_assist_port = COALESCE($16, record_assist_port),
           default_server = COALESCE($17, default_server),
           update_time = $18
           WHERE id = $19"#,
    )
    .bind(body.hook_ip.as_deref())
    .bind(body.sdp_ip.as_deref())
    .bind(body.stream_ip.as_deref())
    .bind(body.http_ssl_port)
    .bind(body.secret.as_deref())
    .bind(body.type_.as_deref())
    .bind(body.auto_config)
    .bind(body.rtmp_port)
    .bind(body.rtmp_ssl_port)
    .bind(body.rtsp_port)
    .bind(body.rtsp_ssl_port)
    .bind(body.rtp_enable)
    .bind(body.rtp_port_range.as_deref())
    .bind(body.send_rtp_port_range.as_deref())
    .bind(body.rtp_proxy_port)
    .bind(body.record_assist_port)
    .bind(body.default_server)
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    sqlx::query(
        r#"UPDATE wvp_media_server SET
           hook_ip = COALESCE(?, hook_ip),
           sdp_ip = COALESCE(?, sdp_ip),
           stream_ip = COALESCE(?, stream_ip),
           http_ssl_port = COALESCE(?, http_ssl_port),
           secret = COALESCE(?, secret),
           type = COALESCE(?, type),
           auto_config = COALESCE(?, auto_config),
           rtmp_port = COALESCE(?, rtmp_port),
           rtmp_ssl_port = COALESCE(?, rtmp_ssl_port),
           rtsp_port = COALESCE(?, rtsp_port),
           rtsp_ssl_port = COALESCE(?, rtsp_ssl_port),
           rtp_enable = COALESCE(?, rtp_enable),
           rtp_port_range = COALESCE(?, rtp_port_range),
           send_rtp_port_range = COALESCE(?, send_rtp_port_range),
           rtp_proxy_port = COALESCE(?, rtp_proxy_port),
           record_assist_port = COALESCE(?, record_assist_port),
           default_server = COALESCE(?, default_server),
           update_time = ?
           WHERE id = ?"#,
    )
    .bind(body.hook_ip.as_deref())
    .bind(body.sdp_ip.as_deref())
    .bind(body.stream_ip.as_deref())
    .bind(body.http_ssl_port)
    .bind(body.secret.as_deref())
    .bind(body.type_.as_deref())
    .bind(body.auto_config)
    .bind(body.rtmp_port)
    .bind(body.rtmp_ssl_port)
    .bind(body.rtsp_port)
    .bind(body.rtsp_ssl_port)
    .bind(body.rtp_enable)
    .bind(body.rtp_port_range.as_deref())
    .bind(body.send_rtp_port_range.as_deref())
    .bind(body.rtp_proxy_port)
    .bind(body.record_assist_port)
    .bind(body.default_server)
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    let auto_config = body.auto_config.unwrap_or(true);
    let mut zlm_hook_errors = Vec::new();
    if auto_config {
        let client = state
            .get_zlm_client(Some(&id))
            .unwrap_or_else(|| std::sync::Arc::new(crate::zlm::ZlmClient::new(
                &ip,
                http_port as u16,
                body.secret.as_deref().unwrap_or_default(),
            )));
        zlm_hook_errors = configure_zlm_hooks(&state, &id, &client).await;
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "autoConfig": auto_config,
        "hookConfigured": auto_config && zlm_hook_errors.is_empty(),
        "hookErrors": zlm_hook_errors,
        "message": "保存成功"
    }))))
}

/// DELETE /api/server/media_server/delete
#[derive(Debug, Deserialize)]
pub struct MediaServerDeleteQuery {
    pub id: Option<String>,
}

pub async fn media_server_delete(
    State(state): State<AppState>,
    Query(q): Query<MediaServerDeleteQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q
        .id
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 id 参数"));
    }
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
    let mut server_loads = Vec::new();
    for server_id in state.list_zlm_servers() {
        if let Some(zlm) = state.get_zlm_client(Some(&server_id)) {
            let cfg_map = zlm.get_server_config().await.unwrap_or_default();
            let stats_map = zlm.get_server_stats().await.unwrap_or_default();
            let network_map = zlm.get_net_work_api().await.unwrap_or_default();
            server_loads.push(serde_json::json!({
                "id": server_id,
                "config": cfg_map,
                "stats": stats_map,
                "network": network_map
            }));
        }
    }
    // Return array directly for frontend
    Json(WVPResult::success(serde_json::Value::Array(server_loads)))
}

/// GET /api/server/map/model-icon/list
pub async fn map_model_icon_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![
        serde_json::json!({
            "id": "camera",
            "name": "标准枪机",
            "icon": "el-icon-video-camera"
        }),
        serde_json::json!({
            "id": "ptz",
            "name": "云台球机",
            "icon": "el-icon-camera"
        }),
        serde_json::json!({
            "id": "vehicle",
            "name": "车载终端",
            "icon": "el-icon-truck"
        }),
    ]))
}
