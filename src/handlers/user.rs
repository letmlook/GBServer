use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use md5::{Digest, Md5};
use serde::Deserialize;

use crate::auth::JwtKeys;
use crate::db::{self, LoginUserResponse, RoleInfo, UserListRow};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// 前端已传 MD5(密码) 的 32 位小写十六进制，此处直接使用；若为明文则再计算 MD5
fn password_for_db(password: &str) -> String {
    let s = password.trim();
    if s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        s.to_lowercase()
    } else {
        md5_hex(s)
    }
}

/// GET/POST /api/user/login?username=xx&password=xx  前端传的 password 已为 32 位 MD5 十六进制
pub async fn login(
    State(state): State<AppState>,
    Query(params): Query<LoginParams>,
) -> Result<impl IntoResponse, AppError> {
    let username = params.username.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 username"))?;
    let password = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 password"))?;
    let password_md5 = password_for_db(password);
    let mut user = db::find_by_username_password(&state.pool, username, &password_md5)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "用户名或密码错误"))?;
    user.for_login();

    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    let token = keys
        .create_token(username, state.config.jwt.expiration_minutes)
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "生成 Token 失败"))?;

    let role_id = user.role_id.unwrap_or(0);
    let login_user = LoginUserResponse {
        id: user.id,
        username: user.username.clone(),
        role: RoleInfo {
            id: role_id,
            name: user.role_name.clone(),
            authority: user.role_authority.clone(),
        },
        push_key: user.push_key.clone(),
        access_token: Some(token.clone()),
        server_id: state.config.user_settings.as_ref().and_then(|u| u.server_id.clone()),
    };

    let mut response = (StatusCode::OK, Json(WVPResult::success(login_user))).into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("access-token"),
        axum::http::HeaderValue::from_str(&token).unwrap_or(axum::http::HeaderValue::from_static("")),
    );
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// GET /api/user/logout  仅返回 200
pub async fn logout() -> impl IntoResponse {
    (StatusCode::OK, Json(WVPResult::<()>::success_empty()))
}

/// POST /api/user/userInfo  需 access-token，返回当前用户信息
pub async fn user_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WVPResult<LoginUserResponse>>, AppError> {
    let token = crate::auth::extract_token_from_headers(&headers).ok_or(AppError::Unauthorized)?;
    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    let claims = keys.verify_token(&token).ok_or(AppError::Unauthorized)?;
    let mut user = db::find_by_username(&state.pool, &claims.userName)
        .await?
        .ok_or(AppError::Unauthorized)?;
    user.for_login();

    let role_id = user.role_id.unwrap_or(0);
    let login_user = LoginUserResponse {
        id: user.id,
        username: user.username.clone(),
        role: RoleInfo {
            id: role_id,
            name: user.role_name.clone(),
            authority: user.role_authority.clone(),
        },
        push_key: user.push_key.clone(),
        access_token: None,
        server_id: state.config.user_settings.as_ref().and_then(|u| u.server_id.clone()),
    };
    Ok(Json(WVPResult::success(login_user)))
}

#[derive(Debug, Deserialize)]
pub struct UsersQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
}

/// GET /api/user/users?page=1&count=10
pub async fn users(
    State(state): State<AppState>,
    Query(q): Query<UsersQuery>,
) -> Result<Json<WVPResult<PageUsers>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let list = db::get_users_paged(&state.pool, page, count).await?;
    let total = db::count_users(&state.pool).await?;
    let rows: Vec<UserListRow> = list
        .into_iter()
        .map(|u| {
            let role_id = u.role_id.unwrap_or(0);
            UserListRow {
                id: u.id,
                username: u.username.clone(),
                push_key: u.push_key.clone(),
                role: RoleInfo {
                    id: role_id,
                    name: u.role_name.clone(),
                    authority: u.role_authority.clone(),
                },
                create_time: u.create_time.clone(),
                update_time: u.update_time.clone(),
            }
        })
        .collect();
    let out = PageUsers {
        list: rows,
        total: total as u64,
        page: page as u64,
        size: count as u64,
    };
    Ok(Json(WVPResult::success(out)))
}

#[derive(Debug, serde::Serialize)]
pub struct PageUsers {
    pub list: Vec<UserListRow>,
    pub total: u64,
    pub page: u64,
    pub size: u64,
}

/// POST /api/user/add?username=xx&password=xx&roleId=1
pub async fn add_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AddUserParams>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let _ = require_admin(&state, &headers).await?;
    let username = params.username.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "参数不可为空"))?;
    let password = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "参数不可为空"))?;
    let role_id = params
        .role_id
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 roleId"))?;

    let exists = db::role_exists(&state.pool, role_id).await?;
    if !exists {
        return Err(AppError::business(ErrorCode::Error400, "角色不存在"));
    }

    let password_md5 = md5_hex(password);
    let push_key = md5_hex(&format!("{}{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(), password));
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = db::add_user(&state.pool, username, &password_md5, role_id, &push_key, &now).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "添加失败"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize)]
pub struct AddUserParams {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(alias = "roleId")]
    pub role_id: Option<i32>,
}

/// DELETE /api/user/delete?id=1
pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<DeleteQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let _ = require_admin(&state, &headers).await?;
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let n = db::delete_user(&state.pool, id).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "删除失败"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize)]
pub struct DeleteQuery {
    pub id: Option<i32>,
}

/// POST /api/user/changePassword?oldPassword=xx&password=xx
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChangePasswordParams>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let token = crate::auth::extract_token_from_headers(&headers).ok_or(AppError::Unauthorized)?;
    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    let claims = keys.verify_token(&token).ok_or(AppError::Unauthorized)?;
    let old_md5 = params.old_password.or(params.oldPassword).ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 oldPassword"))?;
    let new_pwd = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 password"))?;

    let user = db::find_by_username(&state.pool, &claims.userName).await?.ok_or(AppError::Unauthorized)?;
    let current_md5 = user.password.as_deref().unwrap_or("");
    if current_md5 != old_md5.as_str() {
        return Err(AppError::business(ErrorCode::Error100, "旧密码错误"));
    }
    let new_md5 = md5_hex(new_pwd);
    let n = db::change_password(&state.pool, user.id, &new_md5).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "修改失败"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordParams {
    pub old_password: Option<String>,
    #[serde(rename = "oldPassword")]
    pub oldPassword: Option<String>,
    pub password: Option<String>,
}

/// POST /api/user/changePasswordForAdmin?userId=2&password=xx
pub async fn change_password_for_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChangePasswordForAdminParams>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let _claims = require_admin(&state, &headers).await?;
    let user_id = params.user_id.or(params.userId).ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 userId"))?;
    let password = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 password"))?;
    let new_md5 = md5_hex(password);
    let n = db::change_password(&state.pool, user_id, &new_md5).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "修改失败"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordForAdminParams {
    pub user_id: Option<i32>,
    #[serde(rename = "userId")]
    pub userId: Option<i32>,
    pub password: Option<String>,
}

/// POST /api/user/changePushKey?userId=2&pushKey=xx
pub async fn change_push_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChangePushKeyParams>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let _ = require_admin(&state, &headers).await?;
    let user_id = params.user_id.or(params.userId).ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 userId"))?;
    let push_key = params.push_key.or(params.pushKey).ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 pushKey"))?;
    let n = db::change_push_key(&state.pool, user_id, &push_key).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "修改失败"));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize)]
pub struct ChangePushKeyParams {
    pub user_id: Option<i32>,
    #[serde(rename = "userId")]
    pub userId: Option<i32>,
    pub push_key: Option<String>,
    #[serde(rename = "pushKey")]
    pub pushKey: Option<String>,
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<crate::auth::Claims, AppError> {
    let token = crate::auth::extract_token_from_headers(headers).ok_or(AppError::Unauthorized)?;
    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    let claims = keys.verify_token(&token).ok_or(AppError::Unauthorized)?;
    let user = db::find_by_username(&state.pool, &claims.userName).await?.ok_or(AppError::Unauthorized)?;
    if user.role_id.unwrap_or(0) != 1 {
        return Err(AppError::business(ErrorCode::Error400, "用户无权限"));
    }
    Ok(claims)
}
