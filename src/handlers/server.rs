//! 流媒体服务器与系统配置 API，与前端 server.js 对应

use axum::{
    extract::{Path, State},
    Json,
};
use crate::db::{get_media_server_by_id, list_media_servers, MediaServer};
use crate::error::AppError;
use crate::response::WVPResult;

use crate::AppState;

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
pub async fn system_config_info() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({})))
}

/// GET /api/server/system/info
pub async fn system_info() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "cpu": null,
        "memory": null,
        "network": null
    })))
}

/// GET /api/server/map/config
pub async fn map_config() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({})))
}

/// GET /api/server/info
pub async fn server_info() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({})))
}

/// GET /api/server/resource/info
pub async fn resource_info() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "total": 0,
        "used": 0
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
/// POST /api/server/media_server/save
pub async fn media_server_save() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// DELETE /api/server/media_server/delete
pub async fn media_server_delete() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// GET /api/server/media_server/media_info
pub async fn media_server_media_info() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
}
/// GET /api/server/media_server/load
pub async fn media_server_load() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
}
/// GET /api/server/map/model-icon/list
pub async fn map_model_icon_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![]))
}
