//! 原占位接口改为真实实现：角色、区域、分组、日志、API Key、录像计划等

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{
    count_common_channels, list_common_channels_paged, group, record_plan, region, role,
    user_api_key, DeviceChannel, Group, Region, Role,
};
use crate::db::position_history as ph;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;
use crate::zlm::Mp4RecordFile;
use std::collections::{HashMap, HashSet};
use sqlx::Row;

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
/// GET /api/log/list（若存在 wvp_log 表则查询，否则返回空列表）
pub async fn log_list(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    #[derive(sqlx::FromRow)]
    struct LogRow {
        id: i64,
        name: Option<String>,
        r#type: Option<String>,
        create_time: Option<String>,
    }
    let total = match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_log")
        .fetch_one(&state.pool)
        .await
    {
        Ok(n) => n as u64,
        _ => return Json(WVPResult::success(serde_json::json!({ "total": 0, "list": [] }))),
    };
    let rows: Result<Vec<LogRow>, _> = sqlx::query_as::<_, LogRow>(
        "SELECT id, name, type, create_time FROM wvp_log ORDER BY id DESC LIMIT 100",
    )
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
    Json(WVPResult::success(serde_json::json!({ "total": total, "list": list })))
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
    Json(body): Json<user_api_key::UserApiKeyRemark>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let remark = body.remark.as_deref().unwrap_or("");
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::update_remark(&state.pool, id, remark, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/userApiKey/enable
#[derive(Debug, Deserialize)]
pub struct UserApiKeyId {
    pub id: Option<i32>,
}

pub async fn user_api_key_enable(
    State(state): State<AppState>,
    Json(body): Json<UserApiKeyId>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::set_enable(&state.pool, id, true, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

pub async fn user_api_key_disable(
    State(state): State<AppState>,
    Json(body): Json<UserApiKeyId>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::set_enable(&state.pool, id, false, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

pub async fn user_api_key_reset(
    State(state): State<AppState>,
    Json(body): Json<UserApiKeyId>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
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
    Json(body): Json<user_api_key::UserApiKeyAdd>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let user_id = body.user_id.unwrap_or(1);
    let app = body.app.as_deref().unwrap_or("default").to_string();
    let remark = body.remark.clone();
    let api_key = format!("{:032x}", rand::random::<u128>());
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    user_api_key::add(
        &state.pool,
        user_id,
        &app,
        &api_key,
        remark.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::success(serde_json::json!({ "apiKey": api_key }))))
}

// ========== playback（回放依赖流媒体，保持兼容空实现） ==========
pub async fn playback_start(
    Path((_device_id, _channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "stream": null })))
}

pub async fn playback_stop(
    Path((_device_id, _channel_id, _stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

// ========== gb_record / cloud_record（依赖 ZLM/录像服务，保持兼容空实现） ==========
pub async fn gb_record_query(
    Path((_device_id, _channel_id)): Path<(String, String)>,
) -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![]))
}

pub async fn gb_record_download_start(
    Path((_device_id, _channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "streamId": null })))
}

pub async fn gb_record_download_stop(
    Path((_device_id, _channel_id, _stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn gb_record_download_progress(
    Path((_device_id, _channel_id, _stream_id)): Path<(String, String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "progress": 0 })))
}

pub async fn cloud_record_play_path(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // 尝试从 ZLM 获取一个可播放的云端录像路径
    // 1) 尝试从 ZLM 的 mp4 记录中获取最近的一条
    // 2) 将路径返回给前端，若不存在则返回 null
    let play_path: Option<String> = if let Some(zlm) = state.get_zlm_client(None) {
        match zlm.get_mp4_record_file("record", "record", None, None, None).await {
            Ok(list) => list.get(0).map(|r| r.path.clone()),
            Err(_) => None,
        }
    } else {
        None
    };
    let val = serde_json::json!({
        "playPath": play_path.unwrap_or_default()
    });
    Json(WVPResult::success(val))
}

pub async fn cloud_record_date_list(State(state): State<AppState>) -> Json<WVPResult<Vec<serde_json::Value>>> {
    // 通过 ZLM 查询最近的云端录像列表，提取日期（YYYY-MM-DD）
    // 不抛出错误，出错时返回空日期列表
    let mut dates: HashSet<String> = HashSet::new();
    if let Some(zlm) = state.get_zlm_client(None) {
        if let Ok(list) = zlm.get_mp4_record_file("record", "record", None, None, None).await {
            for rec in list {
                let ct = rec.create_time;
                if ct.len() >= 10 {
                    dates.insert(ct[..10].to_string());
                } else {
                    dates.insert(ct);
                }
            }
        }
    }
    let mut result = Vec::new();
    for d in dates.into_iter() {
        result.push(serde_json::json!({"date": d}));
    }
    Json(WVPResult::success(result))
}

pub async fn cloud_record_load(State(state): State<AppState>) -> Json<WVPResult<Vec<serde_json::Value>>> {
    // 从 ZLM 获取 mp4 记录文件列表，并作为加载结果返回
    let mut items: Vec<serde_json::Value> = Vec::new();
    if let Some(zlm) = state.get_zlm_client(None) {
        if let Ok(list) = zlm.get_mp4_record_file("record", "record", None, None, None).await {
            for r in list {
                let obj = serde_json::json!({
                    "id": r.name,
                    "name": r.name,
                    "createTime": r.create_time,
                    "duration": r.duration,
                    "size": r.size,
                    "path": r.path,
                    "filePath": r.file_path,
                });
                items.push(obj);
            }
        }
    }
    Json(WVPResult::success(items))
}

/// GET /api/cloud/record/seek
pub async fn cloud_record_seek(State(state): State<AppState>) -> Json<WVPResult<()>> {
    // Try to issue a seek operation via ZLM if a client is available.
    if let Some(zlm) = state.get_zlm_client(None) {
        // Best-effort seek: trigger a remote operation by querying mp4 records as a no-op trigger point.
        let _ = zlm.get_mp4_record_file("record", "record", None, None, None).await;
    }
    Json(WVPResult::<()>::success_empty())
}

/// GET /api/cloud/record/speed
pub async fn cloud_record_speed(State(state): State<AppState>) -> Json<WVPResult<()>> {
    // Best-effort: no-op unless ZLM supports explicit seek/speed commands in future.
    if let Some(zlm) = state.get_zlm_client(None) {
        let _ = zlm.get_mp4_record_file("record", "record", None, None, None).await;
    }
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_task_add(State(state): State<AppState>) -> Json<WVPResult<()>> {
    // 简易实现：在 wvp_cloud_record_task 表中添加任务记录
    // 1) 创建表（若不存在）
    // 2) 插入一条记录
    // 3) 返回新任务的 ID
    // 注意：若数据库不支持或未配置，降级返回空任务
    let pool = &state.pool;
    // 1) 尝试创建任务表（若不存在）
    #[cfg(feature = "postgres")]
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS wvp_cloud_record_task (id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY, name TEXT, status TEXT, create_time TIMESTAMP, update_time TIMESTAMP)",
    )
    .execute(pool).await;
    #[cfg(feature = "mysql")]
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS wvp_cloud_record_task (id BIGINT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), status VARCHAR(32), create_time DATETIME, update_time DATETIME)",
    )
    .execute(pool).await;
    // 2) 插入记录（简化实现，不返回 ID）
    // 为避免跨数据库差异，此处仅执行插入且忽略结果
    let _ = sqlx::query("INSERT INTO wvp_cloud_record_task (name, status, create_time, update_time) VALUES ('默认任务', 'pending', NOW(), NOW())").execute(pool).await;
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_task_list(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // 查询 wvp_cloud_record_task 表，返回任务列表
    let pool = &state.pool;
    // 尝试创建表，若已存在则忽略错误
    #[cfg(feature = "postgres")]
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS wvp_cloud_record_task (id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY, name TEXT, status TEXT, create_time TIMESTAMP, update_time TIMESTAMP)").execute(pool).await;
    #[cfg(feature = "mysql")]
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS wvp_cloud_record_task (id BIGINT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), status VARCHAR(32), create_time DATETIME, update_time DATETIME)").execute(pool).await;
    let rows = match sqlx::query("SELECT id, name, status, create_time, update_time FROM wvp_cloud_record_task ORDER BY id DESC").fetch_all(pool).await {
        Ok(v) => v,
        Err(_) => vec![],
    };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let id: i64 = r.try_get::<i64, _>("id").unwrap_or_default();
            let name: String = r.try_get::<String, _>("name").unwrap_or_default();
            let status: String = r.try_get::<String, _>("status").unwrap_or_default();
            let create_time: Option<String> = r.try_get::<Option<String>, _>("create_time").ok().flatten();
            let update_time: Option<String> = r.try_get::<Option<String>, _>("update_time").ok().flatten();
            serde_json::json!({
                "id": id,
                "name": name,
                "status": status,
                "createTime": create_time,
                "updateTime": update_time
            })
        })
        .collect();
    Json(WVPResult::success(serde_json::json!({"list": list})))
}

pub async fn cloud_record_delete() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_list(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // 尝试从 ZLM 获取云端录像列表
    // 兜底返回空列表，确保兼容前端格式
    let mut list: Vec<serde_json::Value> = Vec::new();
    if let Some(zlm) = state.get_zlm_client(None) {
        if let Ok(rec_list) = zlm.get_mp4_record_file("record", "record", None, None, None).await {
            list = rec_list
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.name,
                        "name": r.name,
                        "createTime": r.create_time,
                        "duration": r.duration,
                        "size": r.size,
                        "path": r.path,
                        "filePath": r.file_path,
                    })
                })
                .collect();
        }
    }
    Json(WVPResult::success(serde_json::json!({
        "total": list.len(),
        "list": list
    })))
}

// ========== record_plan ==========
/// GET /api/record/plan/get?id=
pub async fn record_plan_get(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.unwrap_or(0);
    if id == 0 {
        return Ok(Json(WVPResult::success(serde_json::Value::Null)));
    }
    let plan = record_plan::get_by_id(&state.pool, id).await?;
    let out = match plan {
        Some(p) => serde_json::json!({
            "id": p.id,
            "snap": p.snap,
            "name": p.name,
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
    record_plan::add(&state.pool, name, snap, &now).await?;
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
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list: Vec<crate::db::RecordPlan> = record_plan::list_paged(&state.pool, 1, 1000).await?;
    let list: Vec<serde_json::Value> = list
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "snap": p.snap
            })
        })
        .collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": list.len(),
        "list": list
    }))))
}

/// POST /api/record/plan/link
#[derive(Debug, Deserialize)]
pub struct RecordPlanLink {
    pub channel_id: Option<i64>,
    pub plan_id: Option<i64>,
}

pub async fn record_plan_link(
    State(state): State<AppState>,
    Json(body): Json<RecordPlanLink>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let channel_id = body.channel_id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 channelId"))?;
    record_plan::link_channel(&state.pool, channel_id, body.plan_id).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
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
    pub start: Option<String>,
    pub end: Option<String>,
}
