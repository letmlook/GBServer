//! 原占位接口改为真实实现：角色、区域、分组、日志、API Key、录像计划等
//!
//! ## 角色定位 (Phase 2.5)
//!
//! 本模块保留作为前端兼容性 shim：
//! - 部分 handler 在 Phase 1/2 推进后已切到 `device_control.rs` / `playback.rs` 等真实实现模块
//! - 仍挂载在前端依赖的 `/api/...` 路径上以保证向后兼容
//! - 新代码请优先使用 `crate::handlers::device_control` / `playback` 等模块
//! - 当前所有 entry 函数未标 `#[deprecated]`，是为了不影响前端调用；待前端
//!   切到新 API 后再做 deprecation 警告 + 隔离

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
#[allow(non_snake_case)]
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
            let ptz_type_text = c.channel_type.map(|v| match v {
                1 => "球机".to_string(),
                2 => "半球".to_string(),
                3 => "固定枪机".to_string(),
                4 => "遥控枪机".to_string(),
                _ => "未知".to_string(),
            });
            // Phase 5: 同时输出 camelCase 与 gb_* 前缀字段,兼容前端
            // /channel 页面读取 `gbName`/`gbDeviceId`/`gbStatus` 等历史命名,
            // 否则通道列表 / 地图信息窗显示空白。
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
                "ptzTypeText": ptz_type_text,
                "gbName": c.name,
                "gbDeviceId": c.gb_device_id,
                "gbManufacturer": c.manufacturer,
                "gbModel": c.model,
                "gbOwner": c.owner,
                "gbCivilCode": c.civil_code,
                "gbAddress": c.address,
                "gbParental": c.parental,
                "gbParentId": c.parent_id,
                "gbStatus": c.status,
                "gbLongitude": c.longitude,
                "gbLatitude": c.latitude,
                "streamId": c.stream_id,
                "streamIdentification": c.stream_identification,
                "manufacturer": c.manufacturer,
                "model": c.model,
                "owner": c.owner,
                "civilCode": c.civil_code,
                "address": c.address,
                "parental": c.parental,
                "parentId": c.parent_id,
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
    #[serde(alias = "deviceId")]
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
        // A region's "root" status is `parent_id == None` (canonical) OR
        // `parent_id == Some(-1)` (legacy / "ROOT" sentinel used by the
        // frontend form). Treat both as roots so tree_list returns the
        // top-level nodes the user just inserted.
        .filter(|r| match parent_id {
            None => r.parent_id.is_none() || r.parent_id == Some(-1),
            Some(target) => r.parent_id == Some(target),
        })
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
        .filter(|g| match parent_id {
            None => g.parent_id.is_none() || g.parent_id == Some(-1),
            Some(target) => g.parent_id == Some(target),
        })
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
    pub page: Option<u32>,
    pub count: Option<u32>,
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
    /// 级别过滤（前端 historyLog 使用 `level` 参数）
    pub level: Option<String>,
}

/// GET /api/log/list — 查询系统日志（可按 message / level / 时间范围过滤）
///
/// 2026-09-12 重写。此前实现有双重问题：
/// 1. 查询的 `gb_log` 表**从未在任何 schema 中创建**，且错误被吞成空列表
///    → 该端点永远返回空，「实时日志 / 历史日志」页面实质不可用；
/// 2. 返回的是**文件行**（name/type/create_time），而前端 `historyLog.vue`
///    渲染的是日志**条目**（time/level/logger/message/thread）—— 契约不匹配。
///
/// 现按前端契约返回结构化日志条目，数据来自 `tracing` 采集层（见 `crate::logging`）。
pub async fn log_list(
    State(state): State<AppState>,
    Query(q): Query<LogListQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15).clamp(1, 500);
    // 前端 historyLog 用 level 过滤；旧结构体里叫 log_type，这里两者都接受
    let level = q.level.as_deref().or(q.log_type.as_deref());

    match crate::db::log::list_paged(
        &state.pool,
        q.query.as_deref(),
        level,
        q.start_time.as_deref(),
        q.end_time.as_deref(),
        page,
        count,
    )
    .await
    {
        Ok((total, list)) => Json(WVPResult::success(serde_json::json!({
            "total": total,
            "list": list,
            "page": page,
            "count": count,
        }))),
        Err(e) => {
            // 不再把错误吞成"空列表"：明确返回错误，让调用方知道查询失败
            tracing::error!("查询系统日志失败: {}", e);
            Json(WVPResult::error(format!("查询系统日志失败: {}", e)))
        }
    }
}

/// GET /api/log/file/{fileName} - 下载日志文件
///
/// 两种用法：
/// 1. **导出结构化日志**（真正可用的路径）：
///    `gbserver-log.csv` / `gbserver-log.json`
///    —— 直接导出 `gb_log` 表内容，支持 `query` / `level` / `startTime` /
///    `endTime` 过滤。
///    本进程**不写日志文件**（tracing 采集层直接把结构化日志落到 `gb_log`），
///    所以此前固定去 `./logs/<fileName>` 找文件是必然 404 的死路径。
/// 2. 运维自行放置/挂载在 `./logs/` 下的真实文件；文件名经过严格校验
///    （见 [`safe_log_file_name`]）。
pub async fn log_file_download(
    State(state): State<AppState>,
    Path(file_name): Path<String>,
    Query(q): Query<LogExportQuery>,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::http::{header, StatusCode};
    use axum::response::Response;

    // ---- 1) 结构化日志导出 ----
    let lower = file_name.to_ascii_lowercase();
    if lower == "gbserver-log.csv" || lower == "gbserver-log.json" {
        let rows = crate::db::log::export(
            &state.pool,
            q.query.as_deref(),
            q.level.as_deref(),
            q.start_time.as_deref(),
            q.end_time.as_deref(),
            50_000,
        )
        .await?;

        let (body, content_type) = if lower.ends_with(".csv") {
            (logs_to_csv(&rows), "text/csv; charset=utf-8")
        } else {
            (
                serde_json::to_vec_pretty(&rows)
                    .map_err(|e| AppError::business(ErrorCode::Error500, format!("序列化日志失败: {}", e)))?,
                "application/json; charset=utf-8",
            )
        };

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file_name),
            )
            .body(Body::from(body))
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("构造响应失败: {}", e)));
    }

    // ---- 2) 运维放置的真实日志文件 ----
    let Some(safe_name) = safe_log_file_name(&file_name) else {
        // 明确拒绝而不是默默返回 404：这是安全边界，不该被当成"文件不存在"
        return Err(AppError::business(
            ErrorCode::Error400,
            "非法日志文件名（只允许字母、数字、点、横线、下划线，且不允许以点开头）",
        ));
    };

    let file_path = std::path::PathBuf::from("./logs").join(safe_name);
    if !file_path.is_file() {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("日志文件不存在: {}", file_name),
        ));
    }

    // 修正：此前把所有读取错误都映射成 404（`map_err(|_| ...404)`），
    // 权限不足 / IO 错误会被伪装成"文件不存在"。现在如实区分。
    let data = tokio::fs::read(&file_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::business(ErrorCode::Error404, format!("日志文件不存在: {}", file_name))
        } else {
            AppError::business(
                ErrorCode::Error500,
                format!("日志文件读取失败 {}: {}", file_name, e),
            )
        }
    })?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(Body::from(data))
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("构造响应失败: {}", e)))
}

/// 日志导出过滤条件。
#[derive(Debug, Deserialize)]
pub struct LogExportQuery {
    pub query: Option<String>,
    pub level: Option<String>,
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
}

/// 校验日志文件名，防目录穿越。
///
/// 修正前直接 `PathBuf::from("./logs").join(file_name)`：axum 的路径参数会做
/// 百分号解码，`..%2f` 之类会被还原成 `../`，因此 `GET
/// /api/log/file/..%2f..%2fetc%2fpasswd` 可以读到仓库外的任意文件。
/// 现在只接受"单段、纯字母数字与 `._-`、不以点开头、不含 `..`"的文件名。
fn safe_log_file_name(name: &str) -> Option<&str> {
    let ok = !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    ok.then_some(name)
}

/// 把日志导出成 CSV。字段一律加引号并转义内部引号，
/// 避免日志正文里的逗号/换行/引号破坏表格结构。
fn logs_to_csv(rows: &[crate::db::log::LogEntry]) -> Vec<u8> {
    fn esc(v: Option<&str>) -> String {
        let s = v.unwrap_or("");
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    let mut out = String::from("id,time,level,logger,thread,source,message\n");
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            r.id,
            esc(Some(r.time.as_str())),
            esc(Some(r.level.as_str())),
            esc(r.logger.as_deref()),
            esc(r.thread.as_deref()),
            esc(r.source.as_deref()),
            esc(r.message.as_deref()),
        ));
    }
    out.into_bytes()
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

#[derive(Debug, Deserialize)]
pub struct CloudRecordCollectQuery {
    #[serde(alias = "recordId")]
    pub record_id: Option<String>,
    #[serde(alias = "cloudRecordId")]
    pub cloud_record_id: Option<String>,
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    pub name: Option<String>,
}

async fn ensure_cloud_record_task_table(pool: &crate::db::Pool) {
    #[cfg(feature = "postgres")]
    let query = r#"
        CREATE TABLE IF NOT EXISTS gb_cloud_record_task (
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
        CREATE TABLE IF NOT EXISTS gb_cloud_record_task (
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
    #[cfg(feature = "sqlite")]
    let query = r#"
        CREATE TABLE IF NOT EXISTS gb_cloud_record_task (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app TEXT,
            stream TEXT,
            media_server_id TEXT,
            start_time TEXT,
            end_time TEXT,
            status TEXT,
            progress REAL DEFAULT 0,
            create_time TEXT,
            update_time TEXT
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
        r#"INSERT INTO gb_cloud_record_task
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
        r#"INSERT INTO gb_cloud_record_task
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
    #[cfg(feature = "sqlite")]
    let result = sqlx::query(
        r#"INSERT INTO gb_cloud_record_task
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
        "SELECT id, app, stream, media_server_id, start_time, end_time, status, progress, create_time, update_time FROM gb_cloud_record_task ORDER BY id DESC",
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

/// ============================================================================
/// 云录像收藏
/// ============================================================================

/// 确保收藏表存在
async fn ensure_record_collect_table(pool: &crate::db::Pool) {
    let _ = sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS wvp_record_collect (
            id SERIAL PRIMARY KEY,
            record_id VARCHAR(255) NOT NULL UNIQUE,
            device_id VARCHAR(64),
            channel_id VARCHAR(64),
            name VARCHAR(255),
            create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )"#
    ).execute(pool).await;
}

/// GET /api/cloud/record/collect/add
/// 收藏录像
pub async fn cloud_record_collect_add(
    State(state): State<AppState>,
    Query(q): Query<CloudRecordCollectQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    ensure_record_collect_table(&state.pool).await;
    
    let record_id = q.record_id.clone().or(q.cloud_record_id).unwrap_or_default();
    if record_id.is_empty() {
        return Json(WVPResult::error("record_id is required"));
    }
    
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let name = q.name.clone().unwrap_or_else(|| "收藏录像".to_string());
    
    let result = sqlx::query(
        "INSERT INTO wvp_record_collect (record_id, device_id, channel_id, name) VALUES ($1, $2, $3, $4) ON CONFLICT (record_id) DO NOTHING"
    )
    .bind(&record_id)
    .bind(&device_id)
    .bind(&channel_id)
    .bind(&name)
    .execute(&state.pool)
    .await;
    
    match result {
        Ok(r) if r.rows_affected() > 0 => {
            Json(WVPResult::success(serde_json::json!({"recordId": record_id, "status": "collected"})))
        }
        _ => {
            Json(WVPResult::success(serde_json::json!({"recordId": record_id, "status": "already_collected"})))
        }
    }
}

/// DELETE /api/cloud/record/collect/delete
/// 取消收藏录像
pub async fn cloud_record_collect_delete(
    State(state): State<AppState>,
    Json(body): Json<CloudRecordCollectQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    ensure_record_collect_table(&state.pool).await;
    
    let record_id = body.record_id.clone().or(body.cloud_record_id).unwrap_or_default();
    if record_id.is_empty() {
        return Json(WVPResult::error("record_id is required"));
    }
    
    sqlx::query("DELETE FROM wvp_record_collect WHERE record_id = $1")
        .bind(&record_id)
        .execute(&state.pool)
        .await
        .ok();
    
    Json(WVPResult::success(serde_json::json!({"recordId": record_id, "status": "deleted"})))
}

/// GET /api/cloud/record/collect/list
/// 获取收藏列表
pub async fn cloud_record_collect_list(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    ensure_record_collect_table(&state.pool).await;
    
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(20).min(100);
    let offset = (page.saturating_sub(1) * count) as i64;
    
    let records: Vec<(i32, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, record_id, device_id, channel_id, name, create_time::text FROM wvp_record_collect ORDER BY id DESC LIMIT $1 OFFSET $2"
    )
    .bind(count as i64)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    
    let list: Vec<serde_json::Value> = records.into_iter().map(|r| {
        serde_json::json!({
            "id": r.0,
            "recordId": r.1,
            "deviceId": r.2,
            "channelId": r.3,
            "name": r.4,
            "createTime": r.5
        })
    }).collect();
    
    Json(WVPResult::success(serde_json::json!({"total": list.len(), "list": list})))
}

// ========== record_plan ==========

/// WVP `DateUtil.getNow()` 用的是本地时间；录像计划的时段本身也是本地时间，
/// 时间戳跟着本地走，前端显示才不会差 8 小时。
fn local_now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 校验一组计划时段，出错直接返回 400。
///
/// 比 WVP 严的一点：`start > stop` 会被拒绝。WVP 允许保存但那条时段
/// 永远不会命中（SQL `start <= index and stop >= index`），属于静默失败。
fn validate_record_plan_items(
    items: &[record_plan::RecordPlanItemPayload],
) -> Result<(), AppError> {
    if items.is_empty() {
        return Err(AppError::business(
            ErrorCode::Error400,
            "录制计划时段不可为空",
        ));
    }
    for (idx, item) in items.iter().enumerate() {
        if let Some(err) = item.validate() {
            return Err(AppError::business(
                ErrorCode::Error400,
                format!("第 {} 个时段不合法: {}", idx + 1, err),
            ));
        }
    }
    Ok(())
}

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
    let channel_count = record_plan::count_linked_channels(&state.pool, id as i64).await?;
    let out = match plan {
        Some(p) => serde_json::json!({
            "id": p.id,
            "snap": p.snap,
            "name": p.name,
            "channelCount": channel_count,
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
    let name = body.name.as_deref().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "计划名称不可为空"))?;
    // WVP `RecordPlanController.add()`：planItemList 为空直接报错
    // "添加录制计划时，录制计划不可为空"。此前我们静默建了一条**没有时段**的
    // 计划：列表里看得见，调度器永远不会命中 —— 保存成功但功能为零。
    let items = body
        .plan_item_list
        .as_ref()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "添加录制计划时，录制计划不可为空"))?;
    validate_record_plan_items(items)?;
    let snap = body.snap.unwrap_or(false);
    let now = local_now_str();
    let plan_id = record_plan::add_with_id(&state.pool, name, snap, &now).await?;
    record_plan::replace_items(&state.pool, plan_id, items, &now).await?;
    crate::scheduler::record_plan::wake_record_plan_scheduler();
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/record/plan/update
pub async fn record_plan_update(
    State(state): State<AppState>,
    Json(body): Json<record_plan::RecordPlanUpdate>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    if id == 0 {
        return Err(AppError::business(ErrorCode::Error400, "计划 ID 不可为空"));
    }
    if record_plan::get_by_id(&state.pool, id as i32).await?.is_none() {
        return Err(AppError::business(
            ErrorCode::Error400,
            format!("录制计划不存在: {id}"),
        ));
    }
    if let Some(ref items) = body.plan_item_list {
        validate_record_plan_items(items)?;
    }
    let now = local_now_str();
    record_plan::update(&state.pool, id, body.name.as_deref(), body.snap, &now).await?;
    if let Some(ref items) = body.plan_item_list {
        // WVP `update()`：先清后写；空列表等价于"清空所有时段"
        record_plan::replace_items(&state.pool, id, items, &now).await?;
    }
    crate::scheduler::record_plan::wake_record_plan_scheduler();
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/record/plan/query 的查询参数（WVP: page/count/query）
#[derive(Debug, Deserialize)]
pub struct RecordPlanQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
}

/// GET /api/record/plan/query
pub async fn record_plan_query(
    State(state): State<AppState>,
    Query(q): Query<RecordPlanQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15).min(100);
    let search = q
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let plans: Vec<crate::db::RecordPlan> =
        record_plan::list_paged(&state.pool, page, count, search).await?;
    let total: i64 = record_plan::count_all(&state.pool, search).await?;
    let mut list: Vec<serde_json::Value> = Vec::with_capacity(plans.len());
    for p in &plans {
        let channel_count = record_plan::count_linked_channels(&state.pool, p.id as i64).await?;
        let items = record_plan::list_items(&state.pool, p.id as i64).await?;
        list.push(serde_json::json!({
            "id": p.id,
            "snap": p.snap,
            "name": p.name,
            "channelCount": channel_count,
            "planItemList": items.iter().map(|item| serde_json::json!({
                "id": item.id,
                "start": item.start,
                "stop": item.stop,
                "weekDay": item.week_day,
                "planId": item.plan_id
            })).collect::<Vec<_>>(),
            "createTime": p.create_time,
            "updateTime": p.update_time
        }));
    }
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
    // WVP 的参数名是 **planId**（`RecordPlanController.delete(Integer planId)`），
    // `IdQuery` 已经 alias 到 `plan_id`。早期实现只读 `id`，
    // 于是前端按 WVP 契约传 planId 时稳定得到 400「缺少 id」——删除功能不可用。
    let id = q
        .id
        .or(q.plan_id)
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 planId"))?;
    // WVP `delete()`：计划不存在时报 "录制计划不存在"，而不是静默成功
    if record_plan::get_by_id(&state.pool, id as i32).await?.is_none() {
        return Err(AppError::business(
            ErrorCode::Error400,
            format!("录制计划不存在: {id}"),
        ));
    }
    record_plan::delete_by_id(&state.pool, id as i32).await?;
    crate::scheduler::record_plan::wake_record_plan_scheduler();
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// 动态 WHERE 构造器（录像计划通道列表用）。
///
/// 统一用 `?` 写条件，postgres 下按顺序把第 k 个 `?` 改写成 `$k`——同一份 SQL
/// 文本即可服务三种方言。早期实现把整条 SQL（含全部条件分支）抄了 6 遍
/// （3 方言 × 行查询/计数），改一个条件要改 6 处，正是"改了这里忘了那里"的温床。
struct DynWhere {
    conds: Vec<String>,
    binds: Vec<BindValue>,
}

#[derive(Clone)]
enum BindValue {
    Text(String),
    Int(i32),
}

impl DynWhere {
    fn new() -> Self {
        Self {
            conds: Vec::new(),
            binds: Vec::new(),
        }
    }

    /// 追加一个条件；`?` 的个数必须与 `values` 个数一致。
    fn add(&mut self, cond: impl Into<String>, values: Vec<BindValue>) {
        let cond = cond.into();
        debug_assert_eq!(cond.matches('?').count(), values.len());
        self.conds.push(cond);
        self.binds.extend(values);
    }

    /// 按方言生成最终 SQL（postgres 把 `?` 换成 `$1..$n`）。
    fn sql(&self, base: &str) -> String {
        self.sql_for(base, cfg!(feature = "postgres"))
    }

    /// `sql` 的可测版本：`postgres` 显式传入，便于在 sqlite 构建下也能
    /// 验证占位符改写（否则这段逻辑只在 postgres 构建里才跑到）。
    fn sql_for(&self, base: &str, postgres: bool) -> String {
        let raw = if self.conds.is_empty() {
            base.to_string()
        } else {
            format!("{} WHERE {}", base, self.conds.join(" AND "))
        };
        if postgres {
            let mut out = String::with_capacity(raw.len() + 16);
            let mut n = 0usize;
            for ch in raw.chars() {
                if ch == '?' {
                    n += 1;
                    out.push_str(&format!("${n}"));
                } else {
                    out.push(ch);
                }
            }
            out
        } else {
            raw
        }
    }
}

/// 录像计划通道列表的一行。
///
/// `gb_*` 列在目录同步路径下是空的（数据写在老列 `name`/`status` 上），
/// 所以 SQL 里统一 `coalesce`。
#[derive(Debug, sqlx::FromRow)]
struct RecordPlanChannelRow {
    id: i64,
    gb_device_id: Option<String>,
    gb_name: Option<String>,
    gb_manufacturer: Option<String>,
    gb_model: Option<String>,
    gb_status: Option<String>,
    data_type: Option<i32>,
    record_plan_id: Option<i32>,
}

impl RecordPlanChannelRow {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            // WVP: `wdc.id as gb_id` —— 前端 link 时把 gbId 当 channelIds 回传，
            // 所以这里必须是**通道主键**，不能是国标编号。
            "id": self.id,
            "gbId": self.id,
            "gbDeviceId": self.gb_device_id,
            "gbName": self.gb_name,
            "gbManufacturer": self.gb_manufacturer,
            "gbModel": self.gb_model,
            "gbStatus": self.gb_status.clone().unwrap_or_else(|| "OFF".to_string()),
            "dataType": self.data_type.unwrap_or(0),
            "recordPlanId": self.record_plan_id,
        })
    }
}

/// GET /api/record/plan/channel/list
///
/// 与 WVP `CommonGBChannelMapper.queryForRecordPlanForWebList` 对齐：
///
/// * **只列国标通道**（`channel_type = 0`）—— 录像计划要能真的拉起设备流，
///   把推流/代理/车载通道放进来只会得到一条永远录不到东西的计划；
/// * `gbId` 是通道**主键**（`gb_device_channel.id`）；
/// * `hasLink=true` → `record_plan_id = planId`；`hasLink=false` → `IS NULL`
///   （WVP 语义：未关联 = 不属于任何计划）；
/// * 名称/编号/在线状态都走 `coalesce(gb_xxx, xxx)`。
pub async fn record_plan_channel_list(
    State(state): State<AppState>,
    Query(q): Query<CommonChannelListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15).clamp(1, 500);
    let offset = ((page - 1) * count) as i64;
    let plan_id = q.plan_id;
    let has_link = q.has_link.as_deref();
    let online = q.online.as_deref();
    let channel_type = q.channelType.as_deref().and_then(|v| v.parse::<i32>().ok());
    let search = q.query.as_deref().unwrap_or("").trim().to_string();

    let mut w = DynWhere::new();
    if !search.is_empty() {
        let like = format!("%{}%", search);
        w.add(
            "(coalesce(c.gb_device_id, c.device_id) LIKE ? OR coalesce(c.gb_name, c.name) LIKE ?)",
            vec![BindValue::Text(like.clone()), BindValue::Text(like)],
        );
    }
    match online {
        Some("true") => w.add("coalesce(c.gb_status, c.status) = ?", vec![BindValue::Text("ON".into())]),
        Some("false") => w.add("coalesce(c.gb_status, c.status) = ?", vec![BindValue::Text("OFF".into())]),
        _ => {}
    }
    if let Some(t) = channel_type {
        w.add("c.data_type = ?", vec![BindValue::Int(t)]);
    }
    // 只列国标设备通道（WVP 同样硬编码 channel_type = 0）
    w.add("c.channel_type = 0", vec![]);
    if let Some(p) = plan_id {
        match has_link {
            Some("true") => w.add("c.record_plan_id = ?", vec![BindValue::Int(p)]),
            Some("false") => w.add("c.record_plan_id IS NULL", vec![]),
            _ => {}
        }
    }

    const BASE_COLS: &str = "SELECT c.id, \
        coalesce(c.gb_device_id, c.device_id) AS gb_device_id, \
        coalesce(c.gb_name, c.name) AS gb_name, \
        coalesce(c.gb_manufacturer, c.manufacturer) AS gb_manufacturer, \
        coalesce(c.gb_model, c.model) AS gb_model, \
        coalesce(c.gb_status, c.status) AS gb_status, \
        c.data_type, c.record_plan_id \
        FROM gb_device_channel c";
    const BASE_COUNT: &str = "SELECT COUNT(*) FROM gb_device_channel c";

    let limit_ph = if cfg!(feature = "postgres") {
        format!(" LIMIT ${} OFFSET ${}", w.binds.len() + 1, w.binds.len() + 2)
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql_rows = format!("{}{}", w.sql(BASE_COLS), format!(" ORDER BY c.id DESC{limit_ph}"));
    let mut q_rows = sqlx::query_as::<_, RecordPlanChannelRow>(&sql_rows);
    for b in &w.binds {
        q_rows = match b {
            BindValue::Text(v) => q_rows.bind(v.as_str()),
            BindValue::Int(v) => q_rows.bind(*v),
        };
    }
    let rows: Vec<RecordPlanChannelRow> = q_rows.bind(count as i64).bind(offset).fetch_all(&state.pool).await?;

    let sql_count = w.sql(BASE_COUNT);
    let mut q_count = sqlx::query_scalar::<_, i64>(&sql_count);
    for b in &w.binds {
        q_count = match b {
            BindValue::Text(v) => q_count.bind(v.as_str()),
            BindValue::Int(v) => q_count.bind(*v),
        };
    }
    let total: i64 = q_count.fetch_one(&state.pool).await?;

    let list: Vec<serde_json::Value> = rows.iter().map(|r| r.to_json()).collect();
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
    /// WVP 前端传的是通道**主键**（`CommonGBChannel.gbId`，即
    /// `gb_device_channel.id`）；早期调用方传的是国标编号字符串。
    /// 两种都收（见 `resolve_channel_ids`）。
    #[serde(alias = "channelIds")]
    pub channel_ids: Option<Vec<serde_json::Value>>,
    #[serde(alias = "deviceDbIds")]
    pub device_db_ids: Option<Vec<i64>>,
    #[serde(alias = "allLink")]
    pub all_link: Option<bool>,
}

/// 把一个"通道标识"解析成通道主键。
///
/// 支持两种输入：
/// 1. 数字/数字字符串 → 通道主键（WVP 语义）；
/// 2. 其它字符串 → `gb_device_id` 反查主键。
///
/// 显式区分两者很重要：早期实现**只**把输入当国标编号去查
/// `gb_device_id`，而前端传的是主键数字，于是每个通道都查不到，
/// 循环体一次都没进，接口却返回"成功"——关联操作完全无效且无任何提示。
async fn resolve_channel_id(
    pool: &crate::db::Pool,
    raw: &serde_json::Value,
) -> Result<Option<i64>, AppError> {
    if let Some(n) = raw.as_i64() {
        if record_plan::channel_exists(pool, n).await? {
            return Ok(Some(n));
        }
        // 数字但主键不存在：再按国标编号试一次（编号可能纯数字）
        if let Some(id) = record_plan::channel_id_by_gb_id(pool, &n.to_string()).await? {
            return Ok(Some(id));
        }
        return Ok(None);
    }
    if let Some(text) = raw.as_str() {
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        if let Ok(n) = text.parse::<i64>() {
            if record_plan::channel_exists(pool, n).await? {
                return Ok(Some(n));
            }
        }
        return Ok(record_plan::channel_id_by_gb_id(pool, text).await?);
    }
    Ok(None)
}

pub async fn record_plan_link(
    State(state): State<AppState>,
    Json(body): Json<RecordPlanLink>,
) -> Result<Json<WVPResult<()>>, AppError> {
    // WVP `link()`：channelIds 为空直接报错
    if body.channel_ids.as_ref().is_some_and(|v| v.is_empty())
        && body.channel_id.is_none()
        && body.device_db_ids.as_ref().is_none_or(|v| v.is_empty())
        && body.all_link.is_none()
    {
        return Err(AppError::business(ErrorCode::Error400, "通道编号必须存在"));
    }
    // 关联到某个计划前，先确认计划真的存在（否则会在通道上留一个野 plan_id）
    if let Some(plan_id) = body.plan_id {
        if plan_id > 0 && record_plan::get_by_id(&state.pool, plan_id as i32).await?.is_none() {
            return Err(AppError::business(
                ErrorCode::Error400,
                format!("录制计划不存在: {plan_id}"),
            ));
        }
    }

    // 1) 单个通道
    if let Some(channel_id) = body.channel_id {
        let affected = record_plan::link_channel(&state.pool, channel_id, body.plan_id).await?;
        if affected == 0 {
            return Err(AppError::business(
                ErrorCode::Error400,
                format!("通道不存在: {channel_id}"),
            ));
        }
        crate::scheduler::record_plan::wake_record_plan_scheduler();
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    // 2) 通道列表
    if let Some(ref raw_ids) = body.channel_ids {
        let mut resolved = Vec::new();
        let mut unknown = Vec::new();
        for raw in raw_ids {
            match resolve_channel_id(&state.pool, raw).await? {
                Some(id) => resolved.push(id),
                None => unknown.push(raw.to_string()),
            }
        }
        if !unknown.is_empty() {
            return Err(AppError::business(
                ErrorCode::Error400,
                format!("以下通道不存在: {}", unknown.join(", ")),
            ));
        }
        for id in &resolved {
            record_plan::link_channel(&state.pool, *id, body.plan_id).await?;
        }
        // 取消关联用 null；关联传 planId（WVP 的 `link(channelIds, null)` 语义）
        tracing::info!(
            "record_plan_link: {} 个通道 {} 计划 {:?}",
            resolved.len(),
            if body.plan_id.is_some() { "关联到" } else { "取消关联" },
            body.plan_id
        );
        crate::scheduler::record_plan::wake_record_plan_scheduler();
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    // 3) 按设备关联（设备下所有通道）
    if let Some(ref device_db_ids) = body.device_db_ids {
        if device_db_ids.is_empty() {
            return Err(AppError::business(ErrorCode::Error400, "设备 ID 不可为空"));
        }
        let mut total = 0usize;
        for device_db_id in device_db_ids {
            let ids = record_plan::channel_ids_by_device_db_id(&state.pool, *device_db_id).await?;
            if ids.is_empty() {
                tracing::warn!("record_plan_link: 设备 {} 下没有通道", device_db_id);
            }
            for id in &ids {
                record_plan::link_channel(&state.pool, *id, body.plan_id).await?;
                total += 1;
            }
        }
        if total == 0 {
            return Err(AppError::business(
                ErrorCode::Error400,
                "所选设备下没有可关联的通道",
            ));
        }
        crate::scheduler::record_plan::wake_record_plan_scheduler();
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    // 4) 全部关联 / 全部取消关联
    if let Some(all_link) = body.all_link {
        // WVP `linkAll(planId)` / `cleanAll(planId)` 两个分支都要 planId
        let plan_id = body
            .plan_id
            .ok_or_else(|| AppError::business(ErrorCode::Error400, "全部关联/取消关联时必须提供 planId"))?;
        if all_link {
            let ids = record_plan::all_channel_ids(&state.pool).await?;
            for id in &ids {
                record_plan::link_channel(&state.pool, *id, Some(plan_id)).await?;
            }
            tracing::info!("record_plan_link: 全部 {} 个通道关联到计划 {}", ids.len(), plan_id);
        } else {
            let n = record_plan::unlink_all_channels(&state.pool, plan_id).await?;
            tracing::info!("record_plan_link: 计划 {} 移除全部关联（{} 个通道）", plan_id, n);
        }
        crate::scheduler::record_plan::wake_record_plan_scheduler();
        return Ok(Json(WVPResult::<()>::success_empty()));
    }

    Err(AppError::business(ErrorCode::Error400, "缺少关联参数"))
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

// ============================================================================
// Phase 7.3: 运维 API 路由处理器
// ============================================================================


/// /api/server/shutdown — 服务关闭
// 注：这里曾有 `server_shutdown`，返回 "Shutdown signal sent. Server will stop
// gracefully."，但**没有任何实现** —— 既不发信号、也不停服务，而路由也早已
// 被移除（见 router.rs 中 "Removing legacy public stubs for
// /api/server/{shutdown,version,config}" 的说明；version/config 有真实实现，
// shutdown 没有）。它是一个"只会说谎"的孤儿函数，已删除。
// 若将来要做远程关停，应端到端实现（鉴权 + axum graceful shutdown + SIP/ZLM 清理），
// 而不是恢复这个空壳。

#[cfg(test)]
mod log_export_tests {
    use super::*;

    /// 目录穿越必须被拒绝：axum 路径参数会做百分号解码，
    /// 修正前 `..%2f..%2fetc%2fpasswd` 会被还原成 `../../etc/passwd` 并读到仓库外文件。
    #[test]
    fn safe_log_file_name_rejects_traversal_and_separators() {
        for bad in [
            "../etc/passwd",
            "..",
            "a/../../b",
            "a/b.log",
            "a\\b.log",
            ".hidden",
            "",
            "with space.log",
            "semi;colon.log",
            "%2e%2e%2fetc",
            "x.log\0",
        ] {
            assert!(
                safe_log_file_name(bad).is_none(),
                "{:?} 应被拒绝",
                bad
            );
        }
    }

    #[test]
    fn safe_log_file_name_accepts_plain_names() {
        for good in ["gbserver-log.csv", "app.log", "2026-09-12_01-00.log", "a1.b2-c3_d4"] {
            assert_eq!(safe_log_file_name(good), Some(good), "{:?} 应被接受", good);
        }
    }

    /// 日志正文里的逗号 / 引号 / 换行不能破坏 CSV 结构。
    #[test]
    fn logs_to_csv_escapes_delimiters() {
        let rows = vec![crate::db::log::LogEntry {
            id: 1,
            time: "2026-09-12 01:00:00.000".to_string(),
            level: "WARN".to_string(),
            logger: Some("gbserver::x".to_string()),
            thread: Some("tokio-runtime-worker".to_string()),
            message: Some("a,\"quoted\" line\nsecond line".to_string()),
            source: Some("src/x.rs:1".to_string()),
        }];
        let csv = String::from_utf8(logs_to_csv(&rows)).unwrap();
        assert!(csv.starts_with("id,time,level,logger,thread,source,message\n"));
        // 引号被转义成两个引号，且整段仍包在一对引号里
        assert!(csv.contains("\"a,\"\"quoted\"\" line\nsecond line\""), "{csv}");
        // 数据行数 = 表头 + 1；正文里的 \n 在引号内，不应被当成新行分隔
        assert_eq!(csv.matches("2026-09-12 01:00:00.000").count(), 1);
    }
}

#[cfg(test)]
mod dyn_where_tests {
    use super::*;

    #[test]
    fn test_postgres_placeholder_rewrite() {
        let mut w = DynWhere::new();
        assert_eq!(w.sql_for("SELECT * FROM t", true), "SELECT * FROM t");
        assert_eq!(w.sql_for("SELECT * FROM t", false), "SELECT * FROM t");

        w.add("a LIKE ? OR b LIKE ?", vec![BindValue::Text("%x%".into()), BindValue::Text("%x%".into())]);
        w.add("c.channel_type = 0", vec![]);
        w.add("d = ?", vec![BindValue::Int(7)]);

        // sqlite/mysql：保持 ? 原样
        assert_eq!(
            w.sql_for("SELECT * FROM t", false),
            "SELECT * FROM t WHERE a LIKE ? OR b LIKE ? AND c.channel_type = 0 AND d = ?"
        );
        // postgres：按顺序编号 $1..$3
        assert_eq!(
            w.sql_for("SELECT * FROM t", true),
            "SELECT * FROM t WHERE a LIKE $1 OR b LIKE $2 AND c.channel_type = 0 AND d = $3"
        );
        assert_eq!(w.binds.len(), 3);
    }

    /// 计数查询与行查询共用同一个 WHERE，但行查询在后面追加 LIMIT/OFFSET，
    /// 编号必须接在 WHERE 参数之后（否则 postgres 下会串位）。
    #[test]
    fn test_postgres_limit_placeholders_follow_binds() {
        let mut w = DynWhere::new();
        w.add("a = ?", vec![BindValue::Text("x".into())]);
        let n = w.binds.len();
        let sql = format!("{} LIMIT ${} OFFSET ${}", w.sql_for("SELECT 1 FROM t", true), n + 1, n + 2);
        assert_eq!(sql, "SELECT 1 FROM t WHERE a = $1 LIMIT $2 OFFSET $3");
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod record_plan_handler_tests {
    use super::*;
    use crate::db::RecordPlanUpdate;
    use crate::test_support::app_state;

    fn id_query(id: i32) -> IdQuery {
        IdQuery {
            id: Some(id),
            plan_id: None,
            page: None,
            count: None,
        }
    }

    fn channel_query(has_link: Option<&str>, plan_id: Option<i32>) -> CommonChannelListQuery {
        CommonChannelListQuery {
            page: Some(1),
            count: Some(50),
            query: None,
            online: None,
            channelType: None,
            hasRecordPlan: None,
            civilCode: None,
            parentDeviceId: None,
            plan_id,
            has_link: has_link.map(|s| s.to_string()),
        }
    }


    fn item(start: i32, stop: i32, day: i32) -> crate::db::record_plan::RecordPlanItemPayload {
        crate::db::record_plan::RecordPlanItemPayload {
            start: Some(start),
            stop: Some(stop),
            week_day: Some(day),
            plan_id: None,
        }
    }

    fn add_body(
        name: Option<&str>,
        items: Option<Vec<crate::db::record_plan::RecordPlanItemPayload>>,
    ) -> crate::db::record_plan::RecordPlanAdd {
        crate::db::record_plan::RecordPlanAdd {
            name: name.map(|s| s.to_string()),
            snap: None,
            plan_item_list: items,
        }
    }

    async fn seed_channel(state: &AppState, gb_id: &str) -> i64 {
        let r = sqlx::query(
            "INSERT INTO gb_device_channel \
             (device_id, name, gb_device_id, status, data_type, data_device_id, channel_type, \
              create_time, update_time) \
             VALUES ('dev1', ?, ?, 'ON', 0, 1, 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .bind(format!("ch-{gb_id}"))
        .bind(gb_id)
        .execute(&state.pool)
        .await
        .expect("insert channel");
        r.last_insert_rowid()
    }

    /// WVP `RecordPlanController.add()` 对空 `planItemList` 直接报错。
    /// 早期实现会静默建一条**没有任何时段**的计划：列表里看得见，
    /// 调度器永远不命中 —— 保存成功但功能为零。
    #[tokio::test]
    async fn test_add_rejects_empty_plan_item_list() {
        let state = app_state().await;
        let err = record_plan_add(State(state.clone()), Json(add_body(Some("p"), None)))
            .await
            .expect_err("空时段必须被拒绝");
        match err {
            AppError::Business(ErrorCode::Error400, msg) => {
                assert!(msg.contains("不可为空"), "{msg}")
            }
            other => panic!("期望 400 业务错误，实际 {other:?}"),
        }

        let err = record_plan_add(State(state.clone()), Json(add_body(Some("p"), Some(vec![]))))
            .await
            .expect_err("空数组同样必须被拒绝");
        assert!(matches!(err, AppError::Business(ErrorCode::Error400, _)));

        // 校验失败时不得留下半条计划
        let total = crate::db::record_plan::count_all(&state.pool, None).await.unwrap();
        assert_eq!(total, 0, "校验失败不应写入任何计划");
    }

    #[tokio::test]
    async fn test_add_rejects_reversed_window() {
        let state = app_state().await;
        let err = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("p"), Some(vec![item(660, 600, 1)]))),
        )
        .await
        .expect_err("start > stop 必须被拒绝");
        assert!(matches!(err, AppError::Business(ErrorCode::Error400, _)));
    }

    /// add → get 往返：`planItemList` 与 `channelCount` 都要能读回来。
    #[tokio::test]
    async fn test_add_get_roundtrip_keeps_items_and_channel_count() {
        let state = app_state().await;
        let ch = seed_channel(&state, "34020000001310000001").await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("机房"), Some(vec![item(600, 660, 1), item(1200, 1260, 7)]))),
        )
        .await
        .expect("add 应成功");

        let resp = record_plan_get(State(state.clone()), Query(id_query(1)))
            .await
            .expect("get 应成功");
        let v = resp.0.data.as_ref().expect("data");
        assert_eq!(v["name"], "机房");
        assert_eq!(v["channelCount"], 0);
        let items = v["planItemList"].as_array().expect("planItemList");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["start"], 600);
        assert_eq!(items[0]["stop"], 660);
        assert_eq!(items[0]["weekDay"], 1);

        // 关联一个通道后 channelCount 应变成 1
        let _ = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: None,
                plan_id: Some(1),
                channel_ids: Some(vec![serde_json::json!(ch)]),
                device_db_ids: None,
                all_link: None,
            }),
        )
        .await
        .expect("link 应成功");

        let resp = record_plan_get(State(state.clone()), Query(id_query(1)))
            .await
            .unwrap();
        assert_eq!(resp.0.data.as_ref().unwrap()["channelCount"], 1);
    }

    #[tokio::test]
    async fn test_update_requires_existing_plan() {
        let state = app_state().await;
        let err = record_plan_update(
            State(state.clone()),
            Json(RecordPlanUpdate {
                id: Some(999),
                name: Some("x".into()),
                snap: None,
                plan_item_list: Some(vec![item(0, 60, 1)]),
            }),
        )
        .await
        .expect_err("更新不存在的计划必须报错");
        match err {
            AppError::Business(_, msg) => assert!(msg.contains("不存在"), "{msg}"),
            other => panic!("期望业务错误，实际 {other:?}"),
        }
    }

    /// WVP 契约：`DELETE /api/record/plan/delete?planId=`。
    /// 早期实现只读 `id`，前端按 WVP 传 planId 时稳定 400「缺少 id」。
    #[tokio::test]
    async fn test_delete_accepts_plan_id_param() {
        let state = app_state().await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("待删"), Some(vec![item(0, 1439, 1)]))),
        )
        .await
        .unwrap();

        let _ = record_plan_delete(
            State(state.clone()),
            Query(IdQuery {
                id: None,
                plan_id: Some(1),
                page: None,
                count: None,
            }),
        )
        .await
        .expect("按 planId 删除必须成功");
        assert_eq!(
            crate::db::record_plan::count_all(&state.pool, None).await.unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_delete_requires_existing_plan() {
        let state = app_state().await;
        let err = record_plan_delete(State(state.clone()), Query(id_query(7)))
            .await
            .expect_err("删除不存在的计划必须报错");
        match err {
            AppError::Business(_, msg) => assert!(msg.contains("不存在"), "{msg}"),
            other => panic!("期望业务错误，实际 {other:?}"),
        }
    }

    /// 关联通道：前端传的是**通道主键**（`gbId`）。
    /// 早期实现把 `channelIds` 一律当国标编号去查 `gb_device_id`，
    /// 每个通道都查不到 → 循环一次没进 → 接口仍返回成功。
    #[tokio::test]
    async fn test_link_by_numeric_channel_id_actually_links() {
        let state = app_state().await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("p"), Some(vec![item(0, 1439, 1)]))),
        )
        .await
        .unwrap();
        let ch = seed_channel(&state, "34020000001310000001").await;

        let _ = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: None,
                plan_id: Some(1),
                channel_ids: Some(vec![serde_json::json!(ch)]),
                device_db_ids: None,
                all_link: None,
            }),
        )
        .await
        .expect("按主键关联应成功");

        assert_eq!(
            crate::db::record_plan::count_linked_channels(&state.pool, 1)
                .await
                .unwrap(),
            1,
            "关联必须真的写进 record_plan_id"
        );
    }

    #[tokio::test]
    async fn test_link_rejects_unknown_channel_and_unknown_plan() {
        let state = app_state().await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("p"), Some(vec![item(0, 1439, 1)]))),
        )
        .await
        .unwrap();

        // 不存在的通道 → 报错（不能静默成功）
        let err = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: None,
                plan_id: Some(1),
                channel_ids: Some(vec![serde_json::json!(4242)]),
                device_db_ids: None,
                all_link: None,
            }),
        )
        .await
        .expect_err("未知通道必须报错");
        match err {
            AppError::Business(_, msg) => assert!(msg.contains("不存在"), "{msg}"),
            other => panic!("期望业务错误，实际 {other:?}"),
        }

        // 不存在的计划 → 报错
        let err = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: None,
                plan_id: Some(999),
                channel_ids: None,
                device_db_ids: None,
                all_link: None,
            }),
        )
        .await
        .expect_err("未知计划必须报错");
        assert!(matches!(err, AppError::Business(_, _)));
    }

    /// `hasLink=false` 表示"不属于任何计划"（WVP 语义）；
    /// `gbId` 必须是通道主键，前端 link 时原样回传。
    #[tokio::test]
    async fn test_channel_list_returns_numeric_gb_id_and_unlinked_semantics() {
        let state = app_state().await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("p"), Some(vec![item(0, 1439, 1)]))),
        )
        .await
        .unwrap();
        let ch = seed_channel(&state, "34020000001310000001").await;

        let q = channel_query;

        let resp = record_plan_channel_list(State(state.clone()), Query(q(Some("false"), Some(1))))
            .await
            .unwrap();
        let list = resp.0.data.as_ref().unwrap()["list"].as_array().unwrap();
        assert_eq!(list.len(), 1, "未关联列表应包含该通道");
        assert_eq!(list[0]["gbId"], ch, "gbId 必须是通道主键");
        assert_eq!(list[0]["gbDeviceId"], "34020000001310000001");
        assert_eq!(list[0]["gbName"], "ch-34020000001310000001");

        // 关联后：未关联列表为空，已关联列表有 1 条
        let _ = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: Some(ch),
                plan_id: Some(1),
                channel_ids: None,
                device_db_ids: None,
                all_link: None,
            }),
        )
        .await
        .unwrap();

        let resp = record_plan_channel_list(State(state.clone()), Query(q(Some("false"), Some(1))))
            .await
            .unwrap();
        assert_eq!(resp.0.data.as_ref().unwrap()["list"].as_array().unwrap().len(), 0);
        let resp = record_plan_channel_list(State(state.clone()), Query(q(Some("true"), Some(1))))
            .await
            .unwrap();
        assert_eq!(resp.0.data.as_ref().unwrap()["list"].as_array().unwrap().len(), 1);
        assert_eq!(resp.0.data.as_ref().unwrap()["total"], 1);
    }

    #[tokio::test]
    async fn test_link_all_and_clean_all() {
        let state = app_state().await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("p"), Some(vec![item(0, 1439, 1)]))),
        )
        .await
        .unwrap();
        seed_channel(&state, "34020000001310000001").await;
        seed_channel(&state, "34020000001310000002").await;

        let _ = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: None,
                plan_id: Some(1),
                channel_ids: None,
                device_db_ids: None,
                all_link: Some(true),
            }),
        )
        .await
        .expect("全部关联应成功");
        assert_eq!(
            crate::db::record_plan::count_linked_channels(&state.pool, 1)
                .await
                .unwrap(),
            2
        );

        let _ = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: None,
                plan_id: Some(1),
                channel_ids: None,
                device_db_ids: None,
                all_link: Some(false),
            }),
        )
        .await
        .expect("全部取消关联应成功");
        assert_eq!(
            crate::db::record_plan::count_linked_channels(&state.pool, 1)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_query_returns_channel_count_and_search() {
        let state = app_state().await;
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("机房全天"), Some(vec![item(0, 1439, 1)]))),
        )
        .await
        .unwrap();
        let _ = record_plan_add(
            State(state.clone()),
            Json(add_body(Some("大厅夜间"), Some(vec![item(1320, 1439, 3)]))),
        )
        .await
        .unwrap();
        let ch = seed_channel(&state, "34020000001310000001").await;
        let _ = record_plan_link(
            State(state.clone()),
            Json(RecordPlanLink {
                channel_id: Some(ch),
                plan_id: Some(1),
                channel_ids: None,
                device_db_ids: None,
                all_link: None,
            }),
        )
        .await
        .unwrap();

        let resp = record_plan_query(
            State(state.clone()),
            Query(RecordPlanQuery {
                page: Some(1),
                count: Some(10),
                query: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(resp.0.data.as_ref().unwrap()["total"], 2);
        let list = resp.0.data.as_ref().unwrap()["list"].as_array().unwrap();
        let first = list.iter().find(|p| p["name"] == "机房全天").unwrap();
        assert_eq!(first["channelCount"], 1);
        assert_eq!(first["planItemList"].as_array().unwrap().len(), 1);

        let resp = record_plan_query(
            State(state.clone()),
            Query(RecordPlanQuery {
                page: Some(1),
                count: Some(10),
                query: Some("大厅".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(resp.0.data.as_ref().unwrap()["total"], 1);
        assert_eq!(resp.0.data.as_ref().unwrap()["list"][0]["name"], "大厅夜间");
    }
}
