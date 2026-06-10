//! Role management handlers

use axum::{extract::{Query, State}, Json};
use serde::Deserialize;

use crate::db;
use crate::response::WVPResult;
use crate::AppState;

/// POST /api/role/add
pub async fn role_add(
    State(state): State<AppState>,
    Json(body): Json<RoleAddBody>,
) -> Json<WVPResult<serde_json::Value>> {
    if body.name.is_empty() {
        return Json(WVPResult::error("缺少 name"));
    }
    let role = db::role::RoleCreate {
        name: body.name.clone(),
        authority: body.authority.clone().unwrap_or_default(),
        create_time: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    match db::role::add(&state.pool, &role).await {
        Ok(id) => Json(WVPResult::success(serde_json::json!({
            "id": id,
            "name": role.name,
        }))),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

/// DELETE /api/role/delete?id=...
pub async fn role_delete(
    State(state): State<AppState>,
    Query(q): Query<DeleteRole>,
) -> Json<WVPResult<()>> {
    match db::role::delete(&state.pool, q.id).await {
        Ok(true) => Json(WVPResult::<()>::success_empty()),
        Ok(false) => Json(WVPResult::error("Role not found")),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

#[derive(Deserialize)]
pub struct RoleAddBody {
    pub name: String,
    pub authority: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteRole {
    pub id: i32,
}
