//! 推流 /api/push 与拉流代理 /api/proxy，对应前端 streamPush.js / streamProxy.js

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{stream_push, stream_proxy, StreamPush, StreamProxy};
use crate::error::AppError;
use crate::response::WVPResult;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PushListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub pushing: Option<bool>,
    pub mediaServerId: Option<String>,
}

/// GET /api/push/list
pub async fn push_list(
    State(state): State<AppState>,
    Query(q): Query<PushListQuery>,
) -> Result<Json<WVPResult<PushListPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = stream_push::count_all(
        &state.pool,
        q.mediaServerId.as_deref(),
        q.pushing,
    )
    .await?;
    let list = stream_push::list_paged(
        &state.pool,
        page,
        count,
        q.mediaServerId.as_deref(),
        q.pushing,
    )
    .await?;
    Ok(Json(WVPResult::success(PushListPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    })))
}

#[derive(Debug, serde::Serialize)]
pub struct PushListPage {
    pub total: u64,
    pub list: Vec<StreamPush>,
    pub page: u64,
    pub size: u64,
}

/// POST /api/push/add、update、remove、start 等暂返回成功，后续接 ZLM/业务
pub async fn push_add() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn push_update() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn push_remove() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn push_start() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
}
pub async fn push_batch_remove() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn push_save_to_gb() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn push_remove_form_gb() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}

#[derive(Debug, Deserialize)]
pub struct ProxyListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub pulling: Option<bool>,
    pub mediaServerId: Option<String>,
}

/// GET /api/proxy/list
pub async fn proxy_list(
    State(state): State<AppState>,
    Query(q): Query<ProxyListQuery>,
) -> Result<Json<WVPResult<ProxyListPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = stream_proxy::count_all(
        &state.pool,
        q.mediaServerId.as_deref(),
        q.pulling,
    )
    .await?;
    let list = stream_proxy::list_paged(
        &state.pool,
        page,
        count,
        q.mediaServerId.as_deref(),
        q.pulling,
    )
    .await?;
    Ok(Json(WVPResult::success(ProxyListPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    })))
}

#[derive(Debug, serde::Serialize)]
pub struct ProxyListPage {
    pub total: u64,
    pub list: Vec<StreamProxy>,
    pub page: u64,
    pub size: u64,
}

/// GET /api/proxy/ffmpeg_cmd/list
pub async fn proxy_ffmpeg_cmd_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![]))
}
pub async fn proxy_add() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn proxy_update() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn proxy_save() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn proxy_start() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!(null)))
}
pub async fn proxy_stop() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
pub async fn proxy_delete() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
