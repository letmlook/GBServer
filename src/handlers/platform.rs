//! 级联平台 /api/platform，对应前端 platform.js

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::platform as platform_db;
use crate::db::Platform;
use crate::error::AppError;
use crate::response::WVPResult;

use crate::AppState;

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
) -> Result<Json<WVPResult<PlatformPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = platform_db::count_all(&state.pool).await?;
    let list = platform_db::list_paged(&state.pool, page, count).await?;
    Ok(Json(WVPResult::success(PlatformPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    })))
}

#[derive(Debug, serde::Serialize)]
pub struct PlatformPage {
    pub total: u64,
    pub list: Vec<Platform>,
    pub page: u64,
    pub size: u64,
}

/// GET /api/platform/server_config
pub async fn platform_server_config() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({})))
}
/// GET /api/platform/channel/list
pub async fn platform_channel_list() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({ "total": 0, "list": [] })))
}
/// GET /api/platform/channel/push
pub async fn platform_channel_push() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// POST /api/platform/add, update
pub async fn platform_add() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn platform_update() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// DELETE /api/platform/delete
pub async fn platform_delete() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// GET /api/platform/exit/:deviceGbId
pub async fn platform_exit() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

// ---------- 占位：前端 channel 相关 ----------
/// POST /api/platform/channel/add
pub async fn platform_channel_add() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// POST /api/platform/channel/device/add
pub async fn platform_channel_device_add() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// POST /api/platform/channel/device/remove
pub async fn platform_channel_device_remove() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// DELETE /api/platform/channel/remove
pub async fn platform_channel_remove() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// POST /api/platform/channel/custom/update
pub async fn platform_channel_custom_update() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
