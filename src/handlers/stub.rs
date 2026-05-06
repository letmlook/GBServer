//! 原占位接口改为真实实现：角色、区域、分组、日志、API Key、录像计划等

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Datelike;
use serde::Deserialize;

use crate::db::{
    count_common_channels, list_common_channels_paged, group, record_plan, region, role,
    user_api_key, DeviceChannel, Group, Region, Role,
};
use crate::db::position_history as ph;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;
use std::collections::HashSet;
use sqlx::Row;

fn normalize_record_time_ms(value: &str) -> i64 {
    if let Ok(ts) = value.parse::<i64>() {
        if ts > 1_000_000_000_000 {
            return ts;
        }
        if ts > 1_000_000_000 {
            return ts * 1000;
        }
    }

    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .map(|dt| dt.and_utc().timestamp_millis())
        .unwrap_or_default()
}

fn record_duration_ms(duration: Option<f64>) -> i64 {
    duration
        .map(|value| (value.max(0.0) * 1000.0).round() as i64)
        .unwrap_or(0)
}

fn build_cloud_record_id(media_server_id: &str, app: &str, stream: &str, file_name: &str) -> String {
    format!("{media_server_id}::{app}::{stream}::{file_name}")
}

fn parse_cloud_record_id(record_id: &str) -> Option<(String, String, String, String)> {
    let mut parts = record_id.splitn(4, "::");
    let media_server_id = parts.next()?.to_string();
    let app = parts.next()?.to_string();
    let stream = parts.next()?.to_string();
    let file_name = parts.next()?.to_string();
    Some((media_server_id, app, stream, file_name))
}

fn build_cloud_record_urls(
    state: &AppState,
    media_server_id: &str,
    app: &str,
    stream: &str,
    fallback_path: Option<&str>,
) -> serde_json::Value {
    let config_server = state
        .config
        .zlm
        .as_ref()
        .and_then(|cfg| cfg.servers.iter().find(|sv| sv.id == media_server_id));
    let server_ip = config_server
        .map(|sv| sv.ip.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let http_port = config_server.map(|sv| sv.http_port as i32).unwrap_or(80);
    let https_port = config_server
        .and_then(|sv| sv.https_port.map(|port| port as i32))
        .unwrap_or(443);
    let ws_port = http_port;
    let wss_port = https_port;
    let rtsp_port = 554;

    let http_flv = format!("http://{}:{}/{}/{}.live.flv", server_ip, http_port, app, stream);
    let https_flv = format!("https://{}:{}/{}/{}.live.flv", server_ip, https_port, app, stream);
    let ws_flv = format!("ws://{}:{}/{}/{}.live.flv", server_ip, ws_port, app, stream);
    let wss_flv = format!("wss://{}:{}/{}/{}.live.flv", server_ip, wss_port, app, stream);
    let rtsp = format!("rtsp://{}:{}/{}/{}", server_ip, rtsp_port, app, stream);

    serde_json::json!({
        "httpPath": fallback_path.unwrap_or(&http_flv),
        "httpsPath": fallback_path.unwrap_or(&https_flv),
        "http_flv": http_flv,
        "https_flv": https_flv,
        "ws_flv": ws_flv,
        "wss_flv": wss_flv,
        "rtsp": rtsp
    })
}

// ========== common channel ==========
#[derive(Debug, Deserialize)]
pub struct CommonChannelListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub online: Option<String>,
    pub channelType: Option<String>,
    pub hasRecordPlan: Option<String>,
    pub civilCode: Option<String>,
    pub parentDeviceId: Option<String>,
    #[serde(alias = "planId")]
    pub plan_id: Option<i32>,
    #[serde(alias = "hasLink")]
    pub has_link: Option<String>,
}

/// GET /api/common/channel/list — 通用通道列表，返回 JSON 避免未匹配时落到静态 index.html
pub async fn common_channel_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let query = q.query.as_deref().filter(|s| !s.is_empty());
    let online = match q.online.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let channel_type = q
        .channelType
        .as_deref()
        .and_then(|s| s.parse::<i32>().ok());

    let list: Vec<DeviceChannel> = list_common_channels_paged(
        &state.pool,
        page,
        count,
        query,
        online,
        channel_type,
    )
    .await?;
    let total = count_common_channels(&state.pool, query, online, channel_type).await?;

    let rows: Vec<serde_json::Value> = list
        .into_iter()
        .map(|c| {
            let gb_id = c.gb_device_id.clone().unwrap_or_default();
            let ptz_type: String = c
                .channel_type
                .map(|t: i32| t.to_string())
                .unwrap_or_else(|| "".to_string());
            serde_json::json!({
                "id": c.id,
                "deviceId": c.device_id,
                "name": c.name,
                "channelId": c.gb_device_id,
                "gbId": gb_id,
                "status": c.status,
                "longitude": c.longitude,
                "latitude": c.latitude,
                "createTime": c.create_time,
                "updateTime": c.update_time,
                "subCount": c.sub_count,
                "hasAudio": c.has_audio,
                "channelType": c.channel_type,
                "ptzType": ptz_type,
            })
        })
        .collect();

    let data: serde_json::Value = serde_json::json!({
        "list": rows,
        "total": total,
    });
    Ok(Json(WVPResult::success(data)))
}

// ========== role ==========
/// GET /api/role/all
pub async fn role_all(State(state): State<AppState>) -> Result<Json<WVPResult<Vec<Role>>>, AppError> {
    let list = role::list_all(&state.pool).await?;
    Ok(Json(WVPResult::success(list)))
}

// ========== region ==========
#[derive(Debug, Deserialize)]
pub struct RegionQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub id: Option<i32>,
    pub device_id: Option<String>,
}

/// GET /api/region/tree/list
pub async fn region_tree_list(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<Vec<serde_json::Value>>>, AppError> {
    let list: Vec<Region> = region::list_all(&state.pool).await?;
    let tree = build_region_tree(&list, None);
    Ok(Json(WVPResult::success(tree)))
}

fn build_region_tree(list: &[Region], parent_id: Option<i32>) -> Vec<serde_json::Value> {
    list.iter()
        .filter(|r| r.parent_id == parent_id)
        .map(|r| {
            let children = build_region_tree(list, Some(r.id as i32));
            serde_json::json!({
                "id": r.id,
                "deviceId": r.device_id,
                "name": r.name,
                "parentId": r.parent_id,
                "parentDeviceId": r.parent_device_id,
                "createTime": r.create_time,
                "updateTime": r.update_time,
                "children": children
            })
        })
        .collect()
}

/// DELETE /api/region/delete?id= 或 deviceId=
pub async fn region_delete(
    State(state): State<AppState>,
    Query(q): Query<RegionQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    if let Some(id) = q.id {
        region::delete_by_id(&state.pool, id).await?;
    } else if let Some(ref device_id) = q.device_id {
        region::delete_by_device_id(&state.pool, device_id).await?;
    } else {
        return Err(AppError::business(ErrorCode::Error400, "缺少 id 或 deviceId"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/region/description?id=
pub async fn region_description(
    State(state): State<AppState>,
    Query(q): Query<RegionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let r: Option<Region> = region::get_by_id(&state.pool, id).await?;
    Ok(Json(WVPResult::success(
        r.map(|x| {
            serde_json::json!({
                "id": x.id,
                "deviceId": x.device_id,
                "name": x.name,
                "parentId": x.parent_id,
                "parentDeviceId": x.parent_device_id,
                "createTime": x.create_time,
                "updateTime": x.update_time
            })
        })
        .unwrap_or(serde_json::Value::Null),
    )))
}

/// GET /api/region/addByCivilCode
#[derive(Debug, Deserialize)]
pub struct RegionCivilCodeQuery {
    pub civil_code: Option<String>,
}

pub async fn region_add_by_civil_code(
    State(state): State<AppState>,
    Query(q): Query<RegionCivilCodeQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let civil_code = q.civil_code.as_deref().unwrap_or("").trim();
    if civil_code.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 civilCode"));
    }

    // If region already exists with this civil_code as device_id, do nothing
    if let Ok(Some(_existing)) = region::get_by_device_id(&state.pool, civil_code).await {
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    // Create a new region with auto-generated device_id and name derived from civil_code
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let name = format!("区域 {}", civil_code);
    region::add(&state.pool, civil_code, &name, None, None, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/region/queryChildListInBase?parentId=
#[derive(Debug, Deserialize)]
pub struct RegionChildQuery {
    pub parent_id: Option<i32>,
}

pub async fn region_query_child(
    State(state): State<AppState>,
    Query(q): Query<RegionChildQuery>,
) -> Result<Json<WVPResult<Vec<serde_json::Value>>>, AppError> {
    let parent_id = q.parent_id.unwrap_or(0);
    let list: Vec<Region> = region::list_children(&state.pool, parent_id).await?;
    let out: Vec<serde_json::Value> = list
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "deviceId": r.device_id,
                "name": r.name,
                "parentId": r.parent_id,
                "parentDeviceId": r.parent_device_id,
                "createTime": r.create_time,
                "updateTime": r.update_time
            })
        })
        .collect();
    Ok(Json(WVPResult::success(out)))
}

/// GET /api/region/base/child/list
pub async fn region_base_child_list(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<Vec<serde_json::Value>>>, AppError> {
    let list: Vec<Region> = region::list_children(&state.pool, 0).await?;
    let out: Vec<serde_json::Value> = list
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "deviceId": r.device_id,
                "name": r.name,
                "parentId": r.parent_id,
                "parentDeviceId": r.parent_device_id,
                "createTime": r.create_time,
                "updateTime": r.update_time
            })
        })
        .collect();
    Ok(Json(WVPResult::success(out)))
}

/// POST /api/region/update
pub async fn region_update(
    State(state): State<AppState>,
    Json(body): Json<region::RegionUpdate>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    region::update(
        &state.pool,
        id,
        body.device_id.as_deref(),
        body.name.as_deref(),
        body.parent_id,
        body.parent_device_id.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/region/add
pub async fn region_add(
    State(state): State<AppState>,
    Json(body): Json<region::RegionAdd>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    let name = body.name.as_deref().unwrap_or("").trim();
    if device_id.is_empty() || name.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "deviceId 与 name 必填"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    region::add(
        &state.pool,
        device_id,
        name,
        body.parent_id,
        body.parent_device_id.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/region/path?id=（可省略，若省略则返回空路径）
pub async fn region_path(
    State(state): State<AppState>,
    Query(q): Query<RegionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    // 允许缺少 id，若未传则返回空路径，提升友好度
    let id = q.id.unwrap_or(0);
    let all: Vec<Region> = region::list_all(&state.pool).await?;
    let mut path = Vec::new();
    let mut current_id: Option<i32> = Some(id);
    while let Some(cid) = current_id {
        if let Some(r) = all.iter().find(|x| x.id == cid) {
            path.push(serde_json::json!({
                "id": r.id,
                "deviceId": r.device_id,
                "name": r.name
            }));
            current_id = r.parent_id;
        } else {
            break;
        }
    }
    path.reverse();
    Ok(Json(WVPResult::success(serde_json::Value::Array(path))))
}

/// GET /api/region/tree/query
#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
}

pub async fn region_tree_query(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list: Vec<Region> = region::list_all(&state.pool).await?;
    let total = list.len() as u64;
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let start = ((page - 1) * count) as usize;
    let end = (start + count as usize).min(list.len());
    let list: Vec<serde_json::Value> = list[start..end]
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "deviceId": r.device_id,
                "name": r.name,
                "parentId": r.parent_id,
                "parentDeviceId": r.parent_device_id,
                "createTime": r.create_time,
                "updateTime": r.update_time
            })
        })
        .collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

// ========== group ==========
fn build_group_tree(list: &[Group], parent_id: Option<i32>) -> Vec<serde_json::Value> {
    list.iter()
        .filter(|g| g.parent_id == parent_id)
        .map(|g| {
            let children = build_group_tree(list, Some(g.id as i32));
            serde_json::json!({
                "id": g.id,
                "deviceId": g.device_id,
                "name": g.name,
                "parentId": g.parent_id,
                "parentDeviceId": g.parent_device_id,
                "businessGroup": g.business_group,
                "createTime": g.create_time,
                "updateTime": g.update_time,
                "civilCode": g.civil_code,
                "children": children
            })
        })
        .collect()
}

/// GET /api/group/tree/list
pub async fn group_tree_list(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<Vec<serde_json::Value>>>, AppError> {
    let list: Vec<Group> = group::list_all(&state.pool).await?;
    let tree = build_group_tree(&list, None);
    Ok(Json(WVPResult::success(tree)))
}

/// POST /api/group/add
pub async fn group_add(
    State(state): State<AppState>,
    Json(body): Json<group::GroupAdd>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body.device_id.as_deref().unwrap_or("").trim();
    let name = body.name.as_deref().unwrap_or("").trim();
    let business_group = body.business_group.as_deref().unwrap_or("0");
    if device_id.is_empty() || name.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "deviceId 与 name 必填"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    group::add(
        &state.pool,
        device_id,
        name,
        body.parent_id,
        body.parent_device_id.as_deref(),
        business_group,
        &now,
        body.civil_code.as_deref(),
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/group/update
pub async fn group_update(
    State(state): State<AppState>,
    Json(body): Json<group::GroupUpdate>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    group::update(
        &state.pool,
        id,
        body.device_id.as_deref(),
        body.name.as_deref(),
        body.parent_id,
        body.parent_device_id.as_deref(),
        body.business_group.as_deref(),
        body.civil_code.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// DELETE /api/group/delete?id=
#[derive(Debug, Deserialize)]
pub struct IdQuery {
    pub id: Option<i32>,
    #[serde(alias = "planId")]
    pub plan_id: Option<i32>,
}

pub async fn group_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    group::delete_by_id(&state.pool, id).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/group/path?id=
pub async fn group_path(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.unwrap_or(0);
    let all: Vec<Group> = group::list_all(&state.pool).await?;
    let mut path = Vec::new();
    let mut current_id: Option<i32> = Some(id);
    while let Some(cid) = current_id {
        if let Some(g) = all.iter().find(|x| x.id == cid) {
            path.push(serde_json::json!({
                "id": g.id,
                "deviceId": g.device_id,
                "name": g.name
            }));
            current_id = g.parent_id;
        } else {
            break;
        }
    }
    path.reverse();
    Ok(Json(WVPResult::success(serde_json::Value::Array(path))))
}

/// GET /api/group/tree/query
pub async fn group_tree_query(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list: Vec<Group> = group::list_all(&state.pool).await?;
    let total = list.len() as u64;
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let start = ((page - 1) * count) as usize;
    let end = (start + count as usize).min(list.len());
    let list: Vec<serde_json::Value> = list[start..end]
        .iter()
        .map(|g| {
            serde_json::json!({
                "id": g.id,
                "deviceId": g.device_id,
                "name": g.name,
                "parentId": g.parent_id,
                "parentDeviceId": g.parent_device_id,
                "businessGroup": g.business_group,
                "createTime": g.create_time,
                "updateTime": g.update_time,
                "civilCode": g.civil_code
            })
        })
        .collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

// ========== log ==========
#[derive(Debug, Deserialize)]
pub struct LogListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    #[serde(alias = "type")]
    pub log_type: Option<String>,
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
}

/// GET /api/log/list（若存在 wvp_log 表则查询，否则返回空列表）
pub async fn log_list(
    State(state): State<AppState>,
    Query(q): Query<LogListQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    #[derive(sqlx::FromRow)]
    struct LogRow {
        id: i64,
        name: Option<String>,
        r#type: Option<String>,
        create_time: Option<String>,
    }
    
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15).min(100);
    let offset = (page - 1) * count;
    
    let search = q.query.as_deref().unwrap_or("").trim();
    let log_type = q.log_type.as_deref().unwrap_or("").trim();
    let start_time = q.start_time.as_deref().unwrap_or("").trim();
    let end_time = q.end_time.as_deref().unwrap_or("").trim();
    
    let has_filter = !search.is_empty() || !log_type.is_empty() || !start_time.is_empty() || !end_time.is_empty();
    
    if !has_filter {
        let total = match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_log")
            .fetch_one(&state.pool)
            .await
        {
            Ok(n) => n,
            _ => return Json(WVPResult::success(serde_json::json!({ "total": 0, "list": [] }))),
        };
        
        #[cfg(feature = "postgres")]
        let rows: Result<Vec<LogRow>, _> = sqlx::query_as(
            "SELECT id, name, type, create_time FROM wvp_log ORDER BY id DESC LIMIT $1 OFFSET $2",
        )
        .bind(count as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await;
        
        #[cfg(feature = "mysql")]
        let rows: Result<Vec<LogRow>, _> = sqlx::query_as(
            "SELECT id, name, type, create_time FROM wvp_log ORDER BY id DESC LIMIT ? OFFSET ?",
        )
        .bind(count as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await;
        
        let list = match rows {
            Ok(rows) => rows
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "name": r.name,
                        "type": r.r#type,
                        "createTime": r.create_time
                    })
                })
                .collect::<Vec<_>>(),
            _ => vec![],
        };
        
        return Json(WVPResult::success(serde_json::json!({ "total": total, "list": list })));
    }
    
    let like_search = format!("%{}%", search);
    
    #[cfg(feature = "postgres")]
    {
        let mut conditions = String::new();
        let mut binds: Vec<String> = Vec::new();
        
        if !search.is_empty() {
            conditions.push_str(" AND (name ILIKE $1 OR type ILIKE $1)");
            binds.push(like_search.clone());
        }
        if !log_type.is_empty() {
            let idx = binds.len() + 1;
            conditions.push_str(&format!(" AND type = ${}", idx));
            binds.push(log_type.to_string());
        }
        if !start_time.is_empty() {
            let idx = binds.len() + 1;
            conditions.push_str(&format!(" AND create_time >= ${}", idx));
            binds.push(start_time.to_string());
        }
        if !end_time.is_empty() {
            let idx = binds.len() + 1;
            conditions.push_str(&format!(" AND create_time <= ${}", idx));
            binds.push(end_time.to_string());
        }
        
        let count_sql = format!("SELECT COUNT(*) FROM wvp_log WHERE 1=1{}", conditions);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        for bind in &binds {
            count_query = count_query.bind(bind);
        }
        let total: i64 = count_query.fetch_one(&state.pool).await.unwrap_or(0);
        
        let data_sql = format!("SELECT id, name, type, create_time FROM wvp_log WHERE 1=1{} ORDER BY id DESC LIMIT ${} OFFSET ${}", 
            conditions, binds.len() + 1, binds.len() + 2);
        let mut data_query = sqlx::query_as::<_, LogRow>(&data_sql);
        for bind in &binds {
            data_query = data_query.bind(bind);
        }
        data_query = data_query.bind(count as i64).bind(offset as i64);
        
        let rows: Vec<LogRow> = data_query.fetch_all(&state.pool).await.unwrap_or_default();
        
        let list: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "name": r.name,
                    "type": r.r#type,
                    "createTime": r.create_time
                })
            })
            .collect();
        
        return Json(WVPResult::success(serde_json::json!({ "total": total, "list": list })));
    }
    
    #[cfg(feature = "mysql")]
    {
        let mut conditions = String::new();
        let mut binds: Vec<String> = Vec::new();
        
        if !search.is_empty() {
            conditions.push_str(" AND (name LIKE ? OR type LIKE ?)");
            binds.push(like_search.clone());
            binds.push(like_search.clone());
        }
        if !log_type.is_empty() {
            conditions.push_str(" AND type = ?");
            binds.push(log_type.to_string());
        }
        if !start_time.is_empty() {
            conditions.push_str(" AND create_time >= ?");
            binds.push(start_time.to_string());
        }
        if !end_time.is_empty() {
            conditions.push_str(" AND create_time <= ?");
            binds.push(end_time.to_string());
        }
        
        let count_sql = format!("SELECT COUNT(*) FROM wvp_log WHERE 1=1{}", conditions);
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        for bind in &binds {
            count_query = count_query.bind(bind);
        }
        let total: i64 = count_query.fetch_one(&state.pool).await.unwrap_or(0);
        
        let data_sql = format!("SELECT id, name, type, create_time FROM wvp_log WHERE 1=1{} ORDER BY id DESC LIMIT ? OFFSET ?", conditions);
        let mut data_query = sqlx::query_as::<_, LogRow>(&data_sql);
        for bind in &binds {
            data_query = data_query.bind(bind);
        }
        data_query = data_query.bind(count as i64).bind(offset as i64);
        
        let rows: Vec<LogRow> = data_query.fetch_all(&state.pool).await.unwrap_or_default();
        
        let list: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "name": r.name,
                    "type": r.r#type,
                    "createTime": r.create_time
                })
            })
            .collect();
        
        return Json(WVPResult::success(serde_json::json!({ "total": total, "list": list })));
    }
}

/// GET /api/log/file/{fileName} - 下载指定日志文件
/// Returns a binary stream with Content-Disposition header for download.
pub async fn log_file_download(
    State(_state): State<AppState>,
    Path(file_name): Path<String>,
) -> Result<axum::response::Response, AppError> {
    use axum::http::{header, StatusCode};
    use axum::response::Response;
    use axum::body::Body;
    use std::path::PathBuf;

    // 日志文件目录：项目根目录 logs/，以便与前端一致的日志文件位置
    let base_dir = PathBuf::from("./logs");
    let file_path = base_dir.join(&file_name);

    // 验证文件是否存在
    if !file_path.exists() {
        return Err(AppError::business(ErrorCode::Error404, format!("日志文件不存在: {}", file_name)));
    }

    // 读取文件内容
    let data = tokio::fs::read(&file_path)
        .await
        .map_err(|_| AppError::business(ErrorCode::Error404, format!("日志文件读取失败: {}", file_name)))?;

    // 构造响应：下载流
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(Body::from(data))
        .unwrap();
    Ok(resp)
}

// ========== userApiKey ==========
#[derive(Debug, Deserialize)]
pub struct UserApiKeyQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UserApiKeyMutateQuery {
    pub id: Option<i32>,
    #[serde(alias = "userId")]
    pub user_id: Option<i64>,
    pub app: Option<String>,
    pub enable: Option<bool>,
    #[serde(alias = "expiresAt")]
    pub expires_at: Option<String>,
    pub remark: Option<String>,
}

fn parse_expired_at(raw: Option<&str>) -> Option<i64> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp())
        .or_else(|| raw.parse::<i64>().ok())
}

/// GET /api/userApiKey/userApiKeys
pub async fn user_api_key_list(
    State(state): State<AppState>,
    Query(q): Query<UserApiKeyQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let list: Vec<crate::db::UserApiKey> = user_api_key::list_paged(&state.pool, page, count).await?;
    let total: i64 = user_api_key::count_all(&state.pool).await?;
    let list: Vec<serde_json::Value> = list
        .iter()
        .map(|k| {
            serde_json::json!({
                "id": k.id,
                "userId": k.user_id,
                "app": k.app,
                "apiKey": k.api_key.as_ref().map(|_| "******"),
                "expiredAt": k.expired_at,
                "remark": k.remark,
                "enable": k.enable,
                "createTime": k.create_time,
                "updateTime": k.update_time
            })
        })
        .collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

/// POST /api/userApiKey/remark
pub async fn user_api_key_remark(
    State(state): State<AppState>,
    Query(q): Query<UserApiKeyMutateQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let remark = q.remark.as_deref().unwrap_or("");
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::update_remark(&state.pool, id as i64, remark, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/userApiKey/enable
#[derive(Debug, Deserialize)]
pub struct UserApiKeyId {
    pub id: Option<i32>,
}

pub async fn user_api_key_enable(
    State(state): State<AppState>,
    Query(q): Query<UserApiKeyMutateQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::set_enable(&state.pool, id, true, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

pub async fn user_api_key_disable(
    State(state): State<AppState>,
    Query(q): Query<UserApiKeyMutateQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::set_enable(&state.pool, id, false, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

pub async fn user_api_key_reset(
    State(state): State<AppState>,
    Query(q): Query<UserApiKeyMutateQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let new_key = format!("{:032x}", rand::random::<u128>());
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::reset_api_key(&state.pool, id, &new_key, &now).await?;
    Ok(Json(WVPResult::success(serde_json::json!({ "apiKey": new_key }))))
}

/// DELETE /api/userApiKey/delete?id=
pub async fn user_api_key_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    user_api_key::delete_by_id(&state.pool, id).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/userApiKey/add
pub async fn user_api_key_add(
    State(state): State<AppState>,
    Query(q): Query<UserApiKeyMutateQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let user_id = q.user_id.unwrap_or(1);
    let app = q.app.as_deref().unwrap_or("default").to_string();
    let remark = q.remark.clone();
    let enable = q.enable.unwrap_or(true);
    let expired_at = parse_expired_at(q.expires_at.as_deref());
    let api_key = format!("{:032x}", rand::random::<u128>());
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::add(
        &state.pool,
        user_id,
        &app,
        &api_key,
        expired_at,
        enable,
        remark.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "apiKey": api_key,
        "enable": enable,
        "expiredAt": expired_at
    }))))
}

// ========== cloud_record（已实现，查询 ZLM + DB） ==========

#[derive(Debug, Deserialize)]
pub struct CloudRecordQuery {
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(alias = "recordId")]
    pub record_id: Option<String>,
    #[serde(alias = "cloudRecordId")]
    pub cloud_record_id: Option<String>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub query: Option<String>,
    #[serde(alias = "callId")]
    pub call_id: Option<String>,
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub page: Option<u32>,
    pub count: Option<u32>,
    #[serde(alias = "ascOrder")]
    pub asc_order: Option<bool>,
    #[serde(alias = "isEnd")]
    pub is_end: Option<bool>,
    pub schema: Option<String>,
    pub seek: Option<i64>,
    pub speed: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CloudRecordDeleteBody {
    pub ids: Option<Vec<String>>,
}

async fn ensure_cloud_record_task_table(pool: &crate::db::Pool) {
    #[cfg(feature = "postgres")]
    let query = r#"
        CREATE TABLE IF NOT EXISTS wvp_cloud_record_task (
            id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            app TEXT,
            stream TEXT,
            media_server_id TEXT,
            start_time TEXT,
            end_time TEXT,
            status TEXT,
            progress DOUBLE PRECISION DEFAULT 0,
            create_time TEXT,
            update_time TEXT
        )
    "#;
    #[cfg(feature = "mysql")]
    let query = r#"
        CREATE TABLE IF NOT EXISTS wvp_cloud_record_task (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            app VARCHAR(255),
            stream VARCHAR(255),
            media_server_id VARCHAR(255),
            start_time VARCHAR(64),
            end_time VARCHAR(64),
            status VARCHAR(32),
            progress DOUBLE DEFAULT 0,
            create_time VARCHAR(64),
            update_time VARCHAR(64)
        )
    "#;
    let _ = sqlx::query(query).execute(pool).await;
}

pub async fn cloud_record_play_path(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let record_id = q.record_id.or(q.cloud_record_id).unwrap_or_default();
    let Some((media_server_id, app, stream, file_name)) = parse_cloud_record_id(&record_id) else {
        return Json(WVPResult::success(serde_json::json!({
            "playPath": "",
            "httpPath": "",
            "httpsPath": ""
        })));
    };

    let mut payload = build_cloud_record_urls(&state, &media_server_id, &app, &stream, None);
    if let Some(zlm) = state.get_zlm_client(Some(&media_server_id)) {
        if let Ok(records) = zlm.get_mp4_record_file(&app, &stream, None, None, None).await {
            if let Some(record) = records.into_iter().find(|item| item.name == file_name) {
                payload = build_cloud_record_urls(
                    &state,
                    &media_server_id,
                    &app,
                    &stream,
                    Some(record.path.as_str()),
                );
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("playPath".to_string(), serde_json::json!(record.path));
                    obj.insert("filePath".to_string(), serde_json::json!(record.file_path));
                    obj.insert("fileName".to_string(), serde_json::json!(record.name));
                    obj.insert("stream".to_string(), serde_json::json!(stream));
                    obj.insert("app".to_string(), serde_json::json!(app));
                    obj.insert("mediaServerId".to_string(), serde_json::json!(media_server_id));
                }
                if let Some(ref playback_manager) = state.playback_manager {
                    playback_manager.create(crate::handlers::playback::PlaybackSession {
                        stream_id: record_id.clone(),
                        device_id: media_server_id.clone(),
                        channel_id: file_name.clone(),
                        app: app.clone(),
                        stream: stream.clone(),
                        media_server_id: Some(media_server_id.clone()),
                        schema: q.schema.clone().unwrap_or_else(|| "fmp4".to_string()),
                        start_time: record.create_time.clone(),
                        end_time: None,
                        current_time: record.create_time,
                        speed: 1.0,
                        paused: false,
                        source: "cloud_record".to_string(),
                    }).await;
                }
            }
        }
    }
    Json(WVPResult::success(payload))
}

pub async fn cloud_record_date_list(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<Vec<String>>> {
    let app = q.app.clone().unwrap_or_else(|| "record".to_string());
    let stream = q.stream.clone().unwrap_or_else(|| "record".to_string());
    let media_server_ids = if let Some(id) = q.media_server_id.clone() {
        vec![id]
    } else {
        let ids = state.list_zlm_servers();
        if ids.is_empty() {
            vec!["default".to_string()]
        } else {
            ids
        }
    };

    let mut dates: HashSet<String> = HashSet::new();
    for media_server_id in media_server_ids {
        if let Some(zlm) = state.get_zlm_client(Some(&media_server_id)) {
            if let Ok(list) = zlm.get_mp4_record_file(&app, &stream, None, None, None).await {
                for rec in list {
                    let date = rec.create_time.chars().take(10).collect::<String>();
                    if !date.is_empty() {
                        if let (Some(year), Some(month)) = (q.year, q.month) {
                            if let Ok(parsed) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                                if parsed.year() != year || parsed.month() != month {
                                    continue;
                                }
                            }
                        }
                        dates.insert(date);
                    }
                }
            }
        }
    }
    let mut result = dates.into_iter().collect::<Vec<_>>();
    result.sort();
    Json(WVPResult::success(result))
}

pub async fn cloud_record_load(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let record_id = q.cloud_record_id.unwrap_or_default();
    let Some((media_server_id, app, stream, file_name)) = parse_cloud_record_id(&record_id) else {
        return Json(WVPResult::success(serde_json::json!({})));
    };

    if let Some(zlm) = state.get_zlm_client(Some(&media_server_id)) {
        if let Ok(records) = zlm.get_mp4_record_file(&app, &stream, None, None, None).await {
            if let Some(record) = records.into_iter().find(|item| item.name == file_name) {
                let start_time = normalize_record_time_ms(&record.create_time);
                let duration = record_duration_ms(record.duration);
                let end_time = start_time + duration;
                let mut payload =
                    build_cloud_record_urls(&state, &media_server_id, &app, &stream, Some(record.path.as_str()));
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("id".to_string(), serde_json::json!(record_id));
                    obj.insert("key".to_string(), serde_json::json!(file_name));
                    obj.insert("app".to_string(), serde_json::json!(app));
                    obj.insert("stream".to_string(), serde_json::json!(stream));
                    obj.insert("mediaServerId".to_string(), serde_json::json!(media_server_id));
                    obj.insert("duration".to_string(), serde_json::json!(duration.max(1)));
                    obj.insert("startTime".to_string(), serde_json::json!(start_time));
                    obj.insert("endTime".to_string(), serde_json::json!(end_time));
                    obj.insert("filePath".to_string(), serde_json::json!(record.file_path));
                    obj.insert("playPath".to_string(), serde_json::json!(record.path));
                }
                if let Some(ref playback_manager) = state.playback_manager {
                    playback_manager.create(crate::handlers::playback::PlaybackSession {
                        stream_id: record_id.clone(),
                        device_id: media_server_id.clone(),
                        channel_id: file_name,
                        app: app.clone(),
                        stream: stream.clone(),
                        media_server_id: Some(media_server_id.clone()),
                        schema: q.schema.clone().unwrap_or_else(|| "fmp4".to_string()),
                        start_time: record.create_time.clone(),
                        end_time: Some(end_time.to_string()),
                        current_time: record.create_time,
                        speed: 1.0,
                        paused: false,
                        source: "cloud_record".to_string(),
                    }).await;
                }
                return Json(WVPResult::success(payload));
            }
        }
    }
    Json(WVPResult::success(serde_json::json!({})))
}

pub async fn cloud_record_seek(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let record_id = q.record_id.clone().or(q.cloud_record_id.clone()).unwrap_or_default();
    if let Some(ref playback_manager) = state.playback_manager {
        if !record_id.is_empty() {
            playback_manager
                .update_current_time(&record_id, q.seek.unwrap_or_default().to_string())
                .await;
        }
    }
    Json(WVPResult::success(serde_json::json!({
        "id": record_id,
        "mediaServerId": q.media_server_id,
        "app": q.app,
        "stream": q.stream,
        "schema": q.schema.unwrap_or_else(|| "fmp4".to_string()),
        "seek": q.seek.unwrap_or_default()
    })))
}

pub async fn cloud_record_speed(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let record_id = q.record_id.clone().or(q.cloud_record_id.clone()).unwrap_or_default();
    let speed = q.speed.unwrap_or(1.0);
    if let Some(ref playback_manager) = state.playback_manager {
        if !record_id.is_empty() {
            playback_manager.update_speed(&record_id, speed).await;
        }
    }
    Json(WVPResult::success(serde_json::json!({
        "id": record_id,
        "mediaServerId": q.media_server_id,
        "app": q.app,
        "stream": q.stream,
        "schema": q.schema.unwrap_or_else(|| "fmp4".to_string()),
        "speed": speed
    })))
}

pub async fn cloud_record_task_add(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    ensure_cloud_record_task_table(&state.pool).await;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let app = q.app.unwrap_or_else(|| "record".to_string());
    let stream = q.stream.unwrap_or_else(|| "record".to_string());
    let media_server_id = q.media_server_id.unwrap_or_else(|| "default".to_string());
    let start_time = q.start_time.unwrap_or_default();
    let end_time = q.end_time.unwrap_or_default();

    #[cfg(feature = "postgres")]
    let result = sqlx::query(
        r#"INSERT INTO wvp_cloud_record_task
           (app, stream, media_server_id, start_time, end_time, status, progress, create_time, update_time)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(&app)
    .bind(&stream)
    .bind(&media_server_id)
    .bind(&start_time)
    .bind(&end_time)
    .bind("pending")
    .bind(0.0_f64)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await;
    #[cfg(feature = "mysql")]
    let result = sqlx::query(
        r#"INSERT INTO wvp_cloud_record_task
           (app, stream, media_server_id, start_time, end_time, status, progress, create_time, update_time)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&app)
    .bind(&stream)
    .bind(&media_server_id)
    .bind(&start_time)
    .bind(&end_time)
    .bind("pending")
    .bind(0.0_f64)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await;

    let inserted = result.ok().map(|res| res.rows_affected()).unwrap_or_default();
    Json(WVPResult::success(serde_json::json!({
        "app": app,
        "stream": stream,
        "mediaServerId": media_server_id,
        "startTime": start_time,
        "endTime": end_time,
        "added": inserted
    })))
}

pub async fn cloud_record_task_list(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    ensure_cloud_record_task_table(&state.pool).await;
    let rows = sqlx::query(
        "SELECT id, app, stream, media_server_id, start_time, end_time, status, progress, create_time, update_time FROM wvp_cloud_record_task ORDER BY id DESC",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .filter_map(|r| {
            let status = r.try_get::<String, _>("status").unwrap_or_default();
            if let Some(want_end) = q.is_end {
                let finished = matches!(status.as_str(), "done" | "completed" | "failed");
                if finished != want_end {
                    return None;
                }
            }
            Some(serde_json::json!({
                "id": r.try_get::<i64, _>("id").unwrap_or_default(),
                "app": r.try_get::<Option<String>, _>("app").ok().flatten(),
                "stream": r.try_get::<Option<String>, _>("stream").ok().flatten(),
                "mediaServerId": r.try_get::<Option<String>, _>("media_server_id").ok().flatten(),
                "startTime": r.try_get::<Option<String>, _>("start_time").ok().flatten(),
                "endTime": r.try_get::<Option<String>, _>("end_time").ok().flatten(),
                "status": status,
                "progress": r.try_get::<Option<f64>, _>("progress").ok().flatten().unwrap_or_default(),
                "createTime": r.try_get::<Option<String>, _>("create_time").ok().flatten(),
                "updateTime": r.try_get::<Option<String>, _>("update_time").ok().flatten()
            }))
        })
        .collect();
    let total = list.len();
    Json(WVPResult::success(serde_json::json!({"total": total, "list": list})))
}

pub async fn cloud_record_delete(
    State(state): State<AppState>,
    Json(body): Json<CloudRecordDeleteBody>,
) -> Json<WVPResult<serde_json::Value>> {
    let ids = body.ids.unwrap_or_default();
    let mut deleted = Vec::new();
    let mut failed = Vec::new();

    for record_id in ids {
        let Some((media_server_id, app, stream, file_name)) = parse_cloud_record_id(&record_id) else {
            failed.push(record_id);
            continue;
        };
        let Some(zlm) = state.get_zlm_client(Some(&media_server_id)) else {
            failed.push(record_id);
            continue;
        };
        match zlm.get_mp4_record_file(&app, &stream, None, None, None).await {
            Ok(records) => {
                if let Some(record) = records.into_iter().find(|item| item.name == file_name) {
                    let target = record.file_path.unwrap_or(record.path);
                    if zlm.delete_mp4_file(&target).await.is_ok() {
                        deleted.push(record_id);
                    } else {
                        failed.push(record_id);
                    }
                } else {
                    failed.push(record_id);
                }
            }
            Err(_) => failed.push(record_id),
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "deleted": deleted,
        "failed": failed
    })))
}

pub async fn cloud_record_list(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let app = q.app.clone().unwrap_or_else(|| "record".to_string());
    let stream = q.stream.clone().unwrap_or_else(|| "record".to_string());
    let media_server_ids = if let Some(id) = q.media_server_id.clone() {
        vec![id]
    } else {
        let ids = state.list_zlm_servers();
        if ids.is_empty() {
            vec!["default".to_string()]
        } else {
            ids
        }
    };
    let search = q.query.clone().unwrap_or_default().to_lowercase();
    let call_id = q.call_id.clone().unwrap_or_default().to_lowercase();
    let start_filter = q.start_time.as_deref().map(normalize_record_time_ms);
    let end_filter = q.end_time.as_deref().map(normalize_record_time_ms);
    let mut list = Vec::new();

    for media_server_id in media_server_ids {
        if let Some(zlm) = state.get_zlm_client(Some(&media_server_id)) {
            if let Ok(records) = zlm.get_mp4_record_file(&app, &stream, None, None, None).await {
                for record in records {
                    let start_time = normalize_record_time_ms(&record.create_time);
                    let time_len = record_duration_ms(record.duration);
                    let end_time = start_time + time_len;
                    if let Some(filter_start) = start_filter {
                        if end_time < filter_start {
                            continue;
                        }
                    }
                    if let Some(filter_end) = end_filter {
                        if start_time > filter_end {
                            continue;
                        }
                    }
                    let file_name = record.name.clone();
                    let file_name_lc = file_name.to_lowercase();
                    if !search.is_empty() && !file_name_lc.contains(&search) {
                        continue;
                    }
                    if !call_id.is_empty() && !file_name_lc.contains(&call_id) {
                        continue;
                    }
                    let record_id = build_cloud_record_id(&media_server_id, &app, &stream, &file_name);
                    let mut payload = build_cloud_record_urls(
                        &state,
                        &media_server_id,
                        &app,
                        &stream,
                        Some(record.path.as_str()),
                    );
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert("id".to_string(), serde_json::json!(record_id));
                        obj.insert("app".to_string(), serde_json::json!(app));
                        obj.insert("stream".to_string(), serde_json::json!(stream));
                        obj.insert("callId".to_string(), serde_json::json!(file_name));
                        obj.insert("startTime".to_string(), serde_json::json!(start_time));
                        obj.insert("endTime".to_string(), serde_json::json!(end_time));
                        obj.insert("timeLen".to_string(), serde_json::json!(time_len));
                        obj.insert("fileName".to_string(), serde_json::json!(file_name));
                        obj.insert("createTime".to_string(), serde_json::json!(record.create_time));
                        obj.insert("size".to_string(), serde_json::json!(record.size));
                        obj.insert("mediaServerId".to_string(), serde_json::json!(media_server_id));
                        obj.insert("filePath".to_string(), serde_json::json!(record.file_path));
                    }
                    list.push(payload);
                }
            }
        }
    }

    list.sort_by(|a, b| {
        let av = a.get("startTime").and_then(|v| v.as_i64()).unwrap_or_default();
        let bv = b.get("startTime").and_then(|v| v.as_i64()).unwrap_or_default();
        av.cmp(&bv)
    });
    if q.asc_order != Some(true) {
        list.reverse();
    }

    let total = list.len();
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(1000);
    let start = (page.saturating_sub(1) * count) as usize;
    let end = (start + count as usize).min(total);
    let paged = if start >= total {
        Vec::new()
    } else {
        list[start..end].to_vec()
    };

    Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": paged
    })))
}

// ========== record_plan ==========
/// GET /api/record/plan/get?id=
pub async fn record_plan_get(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.or(q.plan_id).unwrap_or(0);
    if id == 0 {
        return Ok(Json(WVPResult::success(serde_json::Value::Null)));
    }
    let plan = record_plan::get_by_id(&state.pool, id).await?;
    let items = record_plan::list_items(&state.pool, id as i64).await?;
    let out = match plan {
        Some(p) => serde_json::json!({
            "id": p.id,
            "snap": p.snap,
            "name": p.name,
            "planItemList": items.iter().map(|item| serde_json::json!({
                "id": item.id,
                "start": item.start,
                "stop": item.stop,
                "weekDay": item.week_day,
                "planId": item.plan_id,
                "createTime": item.create_time,
                "updateTime": item.update_time
            })).collect::<Vec<_>>(),
            "createTime": p.create_time,
            "updateTime": p.update_time
        }),
        None => serde_json::Value::Null,
    };
    Ok(Json(WVPResult::success(out)))
}

/// POST /api/record/plan/add
pub async fn record_plan_add(
    State(state): State<AppState>,
    Json(body): Json<record_plan::RecordPlanAdd>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let name = body.name.as_deref().unwrap_or("默认计划");
    let snap = body.snap.unwrap_or(false);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let plan_id = record_plan::add_with_id(&state.pool, name, snap, &now).await?;
    if let Some(ref items) = body.plan_item_list {
        record_plan::replace_items(&state.pool, plan_id, items, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/record/plan/update
pub async fn record_plan_update(
    State(state): State<AppState>,
    Json(body): Json<record_plan::RecordPlanUpdate>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    record_plan::update(
        &state.pool,
        id,
        body.name.as_deref(),
        body.snap,
        &now,
    )
    .await?;
    if let Some(ref items) = body.plan_item_list {
        record_plan::replace_items(&state.pool, id, items, &now).await?;
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/record/plan/query
pub async fn record_plan_query(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let list: Vec<crate::db::RecordPlan> = record_plan::list_paged(&state.pool, page, count).await?;
    let total: i64 = record_plan::count_all(&state.pool).await?;
    let list: Vec<serde_json::Value> = list
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "snap": p.snap,
                "name": p.name,
                "createTime": p.create_time,
                "updateTime": p.update_time
            })
        })
        .collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

/// DELETE /api/record/plan/delete?id=
pub async fn record_plan_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    record_plan::delete_by_id(&state.pool, id).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/record/plan/channel/list
pub async fn record_plan_channel_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let offset = (page.saturating_sub(1) * count) as i64;
    let plan_id = q.plan_id;
    let has_link = q.has_link.as_deref();
    let online = match q.online.as_deref() {
        Some("true") => Some("ON"),
        Some("false") => Some("OFF"),
        _ => None,
    };
    let channel_type = q.channelType.as_deref().and_then(|v| v.parse::<i32>().ok());
    let search = q.query.as_deref().unwrap_or("").trim();
    let like = format!("%{}%", search);

    #[cfg(feature = "postgres")]
    let rows = sqlx::query(
        r#"
        SELECT id, name, gb_device_id, manufacturer, status, data_type, record_plan_id
        FROM wvp_device_channel
        WHERE ($1::text = '' OR name ILIKE $2 OR gb_device_id ILIKE $2)
          AND ($3::text IS NULL OR status = $3)
          AND ($4::int IS NULL OR data_type = $4)
          AND (
                $5::int IS NULL
                OR ($6::text = 'true' AND record_plan_id = $5)
                OR ($6::text = 'false' AND (record_plan_id IS NULL OR record_plan_id != $5))
              )
        ORDER BY id DESC
        LIMIT $7 OFFSET $8
        "#,
    )
    .bind(search)
    .bind(&like)
    .bind(online)
    .bind(channel_type)
    .bind(plan_id)
    .bind(has_link)
    .bind(count as i64)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    let rows = sqlx::query(
        r#"
        SELECT id, name, gb_device_id, manufacturer, status, data_type, record_plan_id
        FROM wvp_device_channel
        WHERE (? = '' OR name LIKE ? OR gb_device_id LIKE ?)
          AND (? IS NULL OR status = ?)
          AND (? IS NULL OR data_type = ?)
          AND (
                ? IS NULL
                OR (? = 'true' AND record_plan_id = ?)
                OR (? = 'false' AND (record_plan_id IS NULL OR record_plan_id != ?))
              )
        ORDER BY id DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(search)
    .bind(&like)
    .bind(&like)
    .bind(online)
    .bind(online)
    .bind(channel_type)
    .bind(channel_type)
    .bind(plan_id)
    .bind(has_link)
    .bind(plan_id)
    .bind(has_link)
    .bind(plan_id)
    .bind(count as i64)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    #[cfg(feature = "postgres")]
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM wvp_device_channel
        WHERE ($1::text = '' OR name ILIKE $2 OR gb_device_id ILIKE $2)
          AND ($3::text IS NULL OR status = $3)
          AND ($4::int IS NULL OR data_type = $4)
          AND (
                $5::int IS NULL
                OR ($6::text = 'true' AND record_plan_id = $5)
                OR ($6::text = 'false' AND (record_plan_id IS NULL OR record_plan_id != $5))
              )
        "#,
    )
    .bind(search)
    .bind(&like)
    .bind(online)
    .bind(channel_type)
    .bind(plan_id)
    .bind(has_link)
    .fetch_one(&state.pool)
    .await?;
    #[cfg(feature = "mysql")]
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM wvp_device_channel
        WHERE (? = '' OR name LIKE ? OR gb_device_id LIKE ?)
          AND (? IS NULL OR status = ?)
          AND (? IS NULL OR data_type = ?)
          AND (
                ? IS NULL
                OR (? = 'true' AND record_plan_id = ?)
                OR (? = 'false' AND (record_plan_id IS NULL OR record_plan_id != ?))
              )
        "#,
    )
    .bind(search)
    .bind(&like)
    .bind(&like)
    .bind(online)
    .bind(online)
    .bind(channel_type)
    .bind(channel_type)
    .bind(plan_id)
    .bind(has_link)
    .bind(plan_id)
    .bind(has_link)
    .bind(plan_id)
    .fetch_one(&state.pool)
    .await?;

    let list: Vec<serde_json::Value> = rows.iter().map(|r| {
        let gb_id: Option<String> = r.try_get("gb_device_id").ok();
        serde_json::json!({
            "id": r.try_get::<i64, _>("id").unwrap_or_default(),
            "gbId": gb_id,
            "gbDeviceId": gb_id,
            "gbName": r.try_get::<Option<String>, _>("name").ok().flatten(),
            "gbManufacturer": r.try_get::<Option<String>, _>("manufacturer").ok().flatten(),
            "gbStatus": r.try_get::<Option<String>, _>("status").ok().flatten().unwrap_or_else(|| "OFF".to_string()),
            "dataType": r.try_get::<Option<i32>, _>("data_type").ok().flatten().unwrap_or(0),
            "recordPlanId": r.try_get::<Option<i32>, _>("record_plan_id").ok().flatten(),
        })
    }).collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

/// POST /api/record/plan/link
#[derive(Debug, Deserialize)]
pub struct RecordPlanLink {
    #[serde(alias = "channelId")]
    pub channel_id: Option<i64>,
    #[serde(alias = "planId")]
    pub plan_id: Option<i64>,
    #[serde(alias = "channelIds")]
    pub channel_ids: Option<Vec<String>>,
    #[serde(alias = "deviceDbIds")]
    pub device_db_ids: Option<Vec<i64>>,
    #[serde(alias = "allLink")]
    pub all_link: Option<bool>,
}

pub async fn record_plan_link(
    State(state): State<AppState>,
    Json(body): Json<RecordPlanLink>,
) -> Result<Json<WVPResult<()>>, AppError> {
    if let Some(channel_id) = body.channel_id {
        record_plan::link_channel(&state.pool, channel_id, body.plan_id).await?;
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    if let Some(ref channel_ids) = body.channel_ids {
        for gb_id in channel_ids {
            #[cfg(feature = "postgres")]
            let row = sqlx::query("SELECT id FROM wvp_device_channel WHERE gb_device_id = $1")
                .bind(gb_id)
                .fetch_optional(&state.pool)
                .await?;
            #[cfg(feature = "mysql")]
            let row = sqlx::query("SELECT id FROM wvp_device_channel WHERE gb_device_id = ?")
                .bind(gb_id)
                .fetch_optional(&state.pool)
                .await?;
            if let Some(row) = row {
                let channel_id: i64 = row.try_get::<i32, _>("id").map(|v| v as i64)
                    .or_else(|_| row.try_get::<i64, _>("id"))
                    .unwrap_or_default();
                record_plan::link_channel(&state.pool, channel_id, body.plan_id).await?;
            }
        }
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    if let Some(ref device_db_ids) = body.device_db_ids {
        for device_db_id in device_db_ids {
            #[cfg(feature = "postgres")]
            let rows = sqlx::query("SELECT id FROM wvp_device_channel WHERE data_device_id = $1")
                .bind(*device_db_id as i32)
                .fetch_all(&state.pool)
                .await?;
            #[cfg(feature = "mysql")]
            let rows = sqlx::query("SELECT id FROM wvp_device_channel WHERE data_device_id = ?")
                .bind(*device_db_id as i32)
                .fetch_all(&state.pool)
                .await?;
            for row in rows {
                let channel_id: i64 = row.try_get::<i32, _>("id").map(|v| v as i64)
                    .or_else(|_| row.try_get::<i64, _>("id"))
                    .unwrap_or_default();
                record_plan::link_channel(&state.pool, channel_id, body.plan_id).await?;
            }
        }
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    if body.all_link.is_some() {
        #[cfg(feature = "postgres")]
        let rows = sqlx::query("SELECT id FROM wvp_device_channel")
            .fetch_all(&state.pool)
            .await?;
        #[cfg(feature = "mysql")]
        let rows = sqlx::query("SELECT id FROM wvp_device_channel")
            .fetch_all(&state.pool)
            .await?;
        let target_plan_id = if body.all_link == Some(true) { body.plan_id } else { None };
        for row in rows {
            let channel_id: i64 = row.try_get::<i32, _>("id").map(|v| v as i64)
                .or_else(|_| row.try_get::<i64, _>("id"))
                .unwrap_or_default();
            record_plan::link_channel(&state.pool, channel_id, target_plan_id).await?;
        }
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    return Err(AppError::business(ErrorCode::Error400, "缺少关联参数"));
}

/// GET /api/position/history/:deviceId (used in queryTrace.vue, map/queryTrace.vue)
pub async fn position_history(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Query(q): Query<PositionHistoryQuery>,
) -> Json<serde_json::Value> {
    let start = q.start.clone().unwrap_or_default();
    let end = q.end.clone().unwrap_or_default();
    tracing::info!("position history: device={}, start={}, end={}", device_id, start, end);
    // Fetch from DB
    let list = ph::list_by_device_and_time(&state.pool, &device_id, Some(&start), Some(&end)).await.unwrap_or_default();
    Json(serde_json::json!({
        "code": 0,
        "msg": "查询成功",
        "data": list
    }))
}

#[derive(Debug, Deserialize)]
pub struct PositionHistoryQuery {
    #[serde(alias = "startTime")]
    pub start: Option<String>,
    #[serde(alias = "endTime")]
    pub end: Option<String>,
}
