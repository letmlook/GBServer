//! Region handler stubs - 行政区码/区域/页表查询

use axum::{extract::{Query, State}, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

/// GET /api/region/one?id=...
pub async fn region_one(
    State(_state): State<AppState>,
    Query(q): Query<RegionOne>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "id": q.id,
        "name": format!("region-{}", q.id),
        "deviceCount": 0,
        "channelCount": 0,
    })))
}

/// GET /api/region/page/list?page=&count=
pub async fn region_page_list(
    State(_state): State<AppState>,
    Query(q): Query<PageList>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(15);
    Json(WVPResult::success(serde_json::json!({
        "list": [],
        "total": 0,
        "page": page,
        "count": count,
    })))
}

/// GET /api/region/sync
pub async fn region_sync(
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "synced": 0,
        "msg": "区域同步 ok",
    })))
}

#[derive(Deserialize)]
pub struct RegionOne {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct PageList {
    pub page: Option<u32>,
    pub count: Option<u32>,
}
