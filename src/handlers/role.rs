//! Role management handlers
//!
//! 权限：`/api/role/*` 原先只挂了 JWT 中间件，**任何已登录用户**都能创建/删除
//! 角色（实测普通用户可创建 `authority="0"` 的管理员级角色，等于自助提权）。
//! 现三个入口都由 `handlers::authz` 强制管理员。

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::db;
use crate::error::{AppError, ErrorCode};
use crate::handlers::authz;
use crate::response::ApiResult;
use crate::AppState;

/// POST /api/role/add
#[utoipa::path(
    post,
    path = "/api/role/add",
    tag = "user",
    operation_id = "role_add",
    request_body = RoleAddBody,
    responses(
        (status = 200, description = "新增成功（返回新角色 id + name）",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"id":2,"name":"操作员"}})),
        (status = 400, description = "缺少 name / 角色名过长 / 角色名已存在"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn role_add(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RoleAddBody>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    authz::require_admin(&state, &headers).await?;

    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 name"));
    }
    if name.chars().count() > 50 {
        return Err(AppError::business(
            ErrorCode::Error400,
            "角色名过长（最多 50 字符）",
        ));
    }
    if db::role::get_by_name(&state.pool, name).await?.is_some() {
        return Err(AppError::business(ErrorCode::Error400, "角色名已存在"));
    }

    let role = db::role::RoleCreate {
        name: name.to_string(),
        authority: body.authority.clone().unwrap_or_default(),
        create_time: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    let id = db::role::add(&state.pool, &role).await?;
    Ok(Json(ApiResult::success(serde_json::json!({
        "id": id,
        "name": role.name,
    }))))
}

/// DELETE /api/role/delete?id=...
///
/// 删除前做两项检查（此前都没有，实测可删掉仍在被引用的角色）：
///
/// 1. **内置管理员角色不可删** —— `id = 1` 是种子里的 admin，删掉会让所有
///    管理员失去身份，而且 `authz::is_admin_role` 会随之失去兜底。
/// 2. **仍被用户引用不可删** —— 此前删掉角色后，那些用户的 `role_id` 变成悬空值。
///    `gb_user` 与 `gb_user_role` 之间没有外键约束，所以库里会留下指向不存在
///    角色的用户；用户管理列表当时又是 `INNER JOIN`，这些用户直接被 SQL 丢掉 ——
///    表现为"数据库里有 4 个用户，页面只有 3 行，且翻页也找不回"。
#[utoipa::path(
    delete,
    path = "/api/role/delete",
    tag = "user",
    operation_id = "role_delete",
    params(DeleteRole),
    responses(
        (status = 200, description = "删除成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "内置管理员角色不可删除 / 该角色仍有用户引用"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
        (status = 404, description = "角色不存在"),
    ),
    security(("access_token" = [])),
)]
pub async fn role_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<DeleteRole>,
) -> Result<Json<ApiResult<()>>, AppError> {
    authz::require_admin(&state, &headers).await?;

    if q.id == authz::BUILTIN_ADMIN_ROLE_ID {
        return Err(AppError::business(
            ErrorCode::Error400,
            "内置管理员角色不可删除",
        ));
    }

    let in_use = db::count_users_by_role(&state.pool, q.id).await?;
    if in_use > 0 {
        return Err(AppError::business(
            ErrorCode::Error400,
            format!("该角色仍有 {} 个用户在使用，请先调整这些用户的角色", in_use),
        ));
    }

    match db::role::delete(&state.pool, q.id).await {
        Ok(true) => Ok(Json(ApiResult::<()>::success_empty())),
        Ok(false) => Err(AppError::business(ErrorCode::Error404, "角色不存在")),
        Err(e) => Err(AppError::business(
            ErrorCode::Error100,
            format!("删除失败: {}", e),
        )),
    }
}

/// GET /api/role/all
#[utoipa::path(
    get,
    path = "/api/role/all",
    tag = "user",
    operation_id = "role_list",
    responses(
        (status = 200, description = "全部角色列表（含 authority / createTime / updateTime）",
         body = ApiResult<Vec<db::role::Role>>,
         example = json!({"code":0,"msg":"成功","data":[{"id":1,"name":"管理员","authority":"0","createTime":"2026-01-01 00:00:00","updateTime":"2026-01-01 00:00:00"}]})),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn role_all(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResult<Vec<db::role::Role>>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let roles = db::role::list_all(&state.pool).await?;
    Ok(Json(ApiResult::success(roles)))
}

#[derive(Deserialize, ToSchema)]
pub struct RoleAddBody {
    /// 角色名（≤ 50 字符，不允许重复）
    pub name: String,
    /// 权限标识：`"0"` = 管理员；其他值由调用方定义
    pub authority: Option<String>,
}

#[derive(Deserialize, IntoParams)]
pub struct DeleteRole {
    /// 要删除的角色 ID
    pub id: i32,
}
