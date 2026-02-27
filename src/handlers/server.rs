//! 流媒体服务器与系统配置 API，与前端 server.js 对应

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::db::{get_media_server_by_id, list_media_servers, media_server, MediaServer};
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
