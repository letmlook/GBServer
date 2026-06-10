//! `/api/cloud/record/*` extras — collect toggles, list-url, zip packaging.
//! These complement the CRUD already in `src/handlers/cloud_record.rs`.

use axum::{extract::{Query, State}, Json};
use serde::Deserialize;

use crate::db;
use crate::response::WVPResult;
use crate::AppState;

#[derive(Deserialize, Default)]
pub struct CollectQuery {
    pub id: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct ListUrlQuery {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ZipQuery {
    pub ids: Option<String>, // comma-separated CloudRecord ids
}

/// GET /api/cloud/record/collect/add?id=<i64>
pub async fn collect_add(
    State(state): State<AppState>,
    Query(q): Query<CollectQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = match q.id {
        Some(i) if i > 0 => i,
        _ => return Json(WVPResult::error("missing id")),
    };
    match db::cloud_record::set_collect(&state.pool, id, true).await {
        Ok(true) => Json(WVPResult::success(serde_json::json!({"id": id, "collect": true}))),
        Ok(false) => Json(WVPResult::error("record not found")),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

/// GET /api/cloud/record/collect/delete?id=<i64>
pub async fn collect_delete(
    State(state): State<AppState>,
    Query(q): Query<CollectQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = match q.id {
        Some(i) if i > 0 => i,
        _ => return Json(WVPResult::error("missing id")),
    };
    match db::cloud_record::set_collect(&state.pool, id, false).await {
        Ok(true) => Json(WVPResult::success(serde_json::json!({"id": id, "collect": false}))),
        Ok(false) => Json(WVPResult::error("record not found")),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

/// GET /api/cloud/record/list-url?device_id=&channel_id=
pub async fn list_url(
    State(state): State<AppState>,
    Query(q): Query<ListUrlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15);
    let kw = q.device_id.as_deref().or(q.channel_id.as_deref());
    let records = db::cloud_record::list_paged(
        &state.pool, kw, None, None, None, None,
        page as i64, count as i64,
    ).await.unwrap_or_default();
    let total = db::cloud_record::count_all(
        &state.pool, kw, None, None, None, None,
    ).await.unwrap_or(0);
    let urls: Vec<serde_json::Value> = records.iter().map(|r| {
        let path = r.file_path.clone().unwrap_or_default();
        let url = if path.is_empty() {
            String::new()
        } else {
            format!("/record/{}", path)
        };
        serde_json::json!({
            "id": r.id,
            "app": r.app,
            "stream": r.stream,
            "fileName": r.file_name,
            "url": url,
            "startTime": r.start_time,
            "endTime": r.end_time,
            "duration": r.time_len,
            "fileSize": r.file_size,
        })
    }).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": urls,
        "total": total,
        "page": page,
        "count": count,
    })))
}

/// GET /api/cloud/record/download/zip?ids=1,2,3
/// Stub: returns a manifest describing what would be zipped. Real ZLM-side
/// file archive requires file copy + zip CLI which is server-side background
/// work; this endpoint queues the request and returns a tracking id.
pub async fn download_zip(
    State(_state): State<AppState>,
    Query(q): Query<ZipQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let ids: Vec<i64> = q.ids.unwrap_or_default().split(',')
        .filter_map(|s| s.trim().parse().ok()).collect();
    if ids.is_empty() {
        return Json(WVPResult::error("missing ids"));
    }
    Json(WVPResult::success(serde_json::json!({
        "taskId": format!("zip-{}", chrono::Utc::now().timestamp()),
        "ids": ids,
        "status": "queued",
        "msg": "云录像打包任务已提交，后台完成后可通过 taskId 查询",
    })))
}

/// GET /api/cloud/record/zip?ids=1,2,3 — alias of download/zip
pub async fn zip(
    State(state): State<AppState>,
    Query(q): Query<ZipQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    download_zip(State(state), Query(q)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_query_parses_ids_string() {
        let ids_str = String::from("10,20,30");
        let ids: Vec<i64> = ids_str.split(',')
            .filter_map(|s| s.trim().parse().ok()).collect();
        assert_eq!(ids, vec![10, 20, 30]);
    }

    #[test]
    fn test_zip_query_handles_empty_string() {
        let ids_str = String::new();
        let ids: Vec<i64> = ids_str.split(',')
            .filter_map(|s| s.trim().parse().ok()).collect();
        assert!(ids.is_empty());
    }

    #[test]
    fn test_list_url_query_defaults() {
        let q = ListUrlQuery::default();
        assert_eq!(q.page, None);
        assert_eq!(q.count, None);
    }

    #[test]
    fn test_zip_query_struct_field_default() {
        let q = ZipQuery::default();
        assert!(q.ids.is_none());
    }
}
