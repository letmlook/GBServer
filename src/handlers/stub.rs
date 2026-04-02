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
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

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
pub async fn region_add_by_civil_code() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
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

/// GET /api/region/path?id=
pub async fn region_path(
    State(state): State<AppState>,
    Query(q): Query<RegionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
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
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
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

pub async fn playback_resume(Path(_stream_id): Path<String>) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn playback_pause(Path(_stream_id): Path<String>) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn playback_speed(
    Path((_stream_id, _speed)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
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

pub async fn cloud_record_play_path() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
}

pub async fn cloud_record_date_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![]))
}

pub async fn cloud_record_load() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![]))
}

pub async fn cloud_record_seek() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_speed() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_task_add() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_task_list() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "list": [] })))
}

pub async fn cloud_record_delete() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

pub async fn cloud_record_list() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "total": 0, "list": [] })))
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
