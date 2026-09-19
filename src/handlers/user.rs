use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use md5::{Digest, Md5};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::auth::JwtKeys;
use crate::db::{self, LoginUserResponse, RoleInfo, UserListRow};
use crate::error::{AppError, ErrorCode};
use crate::response::ApiResult;
use crate::AppState;

fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// POST /api/user/login?username=xx&password=xx
///
/// **仅 POST**：路由由 `utoipa_axum::routes!` 依据下面的注解生成。此前注解写了
/// `get, post`，但该宏只为 POST 生成路由 —— 规范里也只有 `post`，而实际路由
/// 对 GET 返回 405。注解与行为不一致会误导前端（前端曾因此一直发 GET，
/// 导致**登录完全不可用**），故这里只保留真实存在的方法。
///
/// Phase 7.6: Password is verified via Argon2 (preferred) with MD5 fallback
/// to the historical plaintext/MD5 passwords still present in some DBs.
/// Frontend is expected to send the MD5 hex of the plaintext password
/// (matches the historical behavior). The handler additionally supports
/// direct plaintext for migration convenience — passwords stored as
/// Argon2 hashes (`$argon2id$...`) are validated against the plaintext;
/// legacy MD5/plaintext passwords are validated by re-hashing the input.
#[utoipa::path(
    post,
    path = "/api/user/login",
    tag = "user",
    operation_id = "user_login",
    params(LoginParams),
    responses(
        (status = 200, description = "登录成功：返回用户信息 + JWT；JWT 同时写入 `access-token` 响应头",
         body = ApiResult<LoginUserResponse>,
         example = json!({"code":0,"msg":"成功","data":{"id":1,"username":"admin","role":{"id":1,"name":"管理员","authority":"0"},"pushKey":"xxx","accessToken":"<jwt>","serverId":null}})),
        (status = 400, description = "缺少 username / password"),
        (status = 401, description = "用户名或密码错误"),
    ),
)]
pub async fn login(
    State(state): State<AppState>,
    Query(params): Query<LoginParams>,
) -> Result<impl IntoResponse, AppError> {
    let username = params.username.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 username"))?;
    let password = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 password"))?;
    let mut user = db::find_by_username(&state.pool, username)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "用户名或密码错误"))?;
    // 兼容校验：Argon2id ↔ 旧版 MD5 ↔ 明文（见 auth::verify_password_compat）
    let stored = user.password.clone().unwrap_or_default();
    if !crate::auth::verify_password_flexible(password, &stored) {
        return Err(AppError::business(ErrorCode::Error100, "用户名或密码错误"));
    }
    // 机会式升级：旧格式（MD5 / 明文）在登录成功后自动替换为 Argon2id，用户无感。
    // 这样种子 admin 的弱 MD5 会在首次登录后自行变强，无需人工改密。
    if !crate::auth::is_argon2_hash(&stored) {
        match crate::auth::hash_password(&crate::auth::password_secret(password)) {
            Ok(new_hash) => match db::change_password(&state.pool, user.id, &new_hash).await {
                Ok(_) => tracing::info!("用户 {} 的口令哈希已升级为 Argon2id", username),
                Err(e) => tracing::warn!("口令哈希升级失败（不影响本次登录）: {}", e),
            },
            Err(e) => tracing::warn!("生成 Argon2id 哈希失败: {}", e),
        }
    }
    user.for_login();

    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    // token 有效期跟着「记住我」走，与前端 cookie 的 expires 保持一致：
    //   * 勾选 → remember_expiration_minutes（默认 7 天）
    //   * 不勾 → expiration_minutes（普通会话，默认 12 小时）
    //
    // 此前无论勾不勾都只发 `expiration_minutes`（当时配的是 30 分钟），
    // 于是"7 天免登录"名不副实：cookie 活 7 天但 token 半小时就过期，
    // 之后每个请求 401 → 前端弹"登录已到期"。用户常在重启后端后第一次
    // 发请求时撞上这个时间点，误以为是重启导致登录失效。
    let ttl_minutes = if params.remember.unwrap_or(false) {
        state.config.jwt.remember_expiration_minutes
    } else {
        state.config.jwt.expiration_minutes
    };
    let token = keys
        .create_token(username, ttl_minutes)
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

    let mut response = (StatusCode::OK, Json(ApiResult::success(login_user))).into_response();
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("access-token"),
        axum::http::HeaderValue::from_str(&token).unwrap_or(axum::http::HeaderValue::from_static("")),
    );
    Ok(response)
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct LoginParams {
    /// 用户名
    pub username: Option<String>,
    /// 密码（前端发送 MD5(明文) 或明文均可；后端按 Argon2 ↔ MD5 ↔ 明文兼容校验）
    pub password: Option<String>,
    /// 「7 天免登录」勾选状态。true → 发长效 token（默认 7 天），
    /// 与前端写 7 天 cookie 的行为对齐；缺省/false → 普通会话 token。
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub remember: Option<bool>,
}

/// 兼容 `?remember=true` / `1` / `yes` / `on` 等写法。
/// 前端 query 里传布尔值必须序列化成字符串，这里统一解析。
fn deserialize_boolish<'de, D>(de: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(de)?;
    Ok(raw.map(|s| {
        matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    }))
}

/// GET /api/user/logout  仅返回 200
#[utoipa::path(
    get,
    path = "/api/user/logout",
    tag = "user",
    operation_id = "user_logout",
    responses(
        (status = 200, description = "登出（前端清空 token）",
         body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn logout() -> impl IntoResponse {
    (StatusCode::OK, Json(ApiResult::<()>::success_empty()))
}

/// POST /api/user/userInfo  需 access-token，返回当前用户信息
#[utoipa::path(
    get,
    post,
    path = "/api/user/userInfo",
    tag = "user",
    operation_id = "user_info",
    responses(
        (status = 200, description = "当前登录用户信息",
         body = ApiResult<LoginUserResponse>,
         example = json!({"code":0,"msg":"成功","data":{"id":1,"username":"admin","role":{"id":1,"name":"管理员","authority":"0"},"pushKey":"xxx","serverId":"node-1"}})),
        (status = 401, description = "未鉴权或 token 失效"),
    ),
    security(("access_token" = [])),
)]
pub async fn user_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResult<LoginUserResponse>>, AppError> {
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
    Ok(Json(ApiResult::success(login_user)))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UsersQuery {
    /// 页码，从 1 开始（默认 1）
    pub page: Option<u32>,
    /// 每页条数（默认 10，最大 100）
    pub count: Option<u32>,
    /// 用户名模糊搜索。此前该字段不存在，前端 `UserQueryParams.query` 声明了却
    /// 传了个寂寞（serde 静默忽略，搜索框看着能输、实际不过滤）。
    pub query: Option<String>,
}

/// GET /api/user/users?page=1&count=10&query=xx
///
/// 需要管理员：返回的是**全部用户**（含每人 pushKey），普通用户不应看到。
#[utoipa::path(
    get,
    path = "/api/user/users",
    tag = "user",
    operation_id = "user_list",
    params(UsersQuery),
    responses(
        (status = 200, description = "分页用户列表（含每人的 pushKey）",
         body = ApiResult<PageUsers>,
         example = json!({"code":0,"msg":"成功","data":{"list":[],"total":0,"page":1,"size":10}})),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<UsersQuery>,
) -> Result<Json<ApiResult<PageUsers>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(10).clamp(1, 100);
    let query = q.query.as_deref();
    let list = db::get_users_paged(&state.pool, page, count, query).await?;
    let total = db::count_users(&state.pool, query).await?;
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
                    // LEFT JOIN 后角色可能已不存在（悬空 role_id）—— 给显式占位，
                    // 而不是让前端显示空白/undefined。
                    name: u.role_name.clone().or_else(|| {
                        Some(if u.role_id.is_some() {
                            "(角色已删除)".to_string()
                        } else {
                            "(未分配)".to_string()
                        })
                    }),
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
    Ok(Json(ApiResult::success(out)))
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct PageUsers {
    pub list: Vec<UserListRow>,
    pub total: u64,
    pub page: u64,
    pub size: u64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UpdateUserParams {
    /// 要修改的用户 ID（必填）
    #[serde(alias = "userId")]
    pub user_id: Option<i32>,
    /// 新用户名（可选；非空且 ≤ 64 字符）
    pub username: Option<String>,
    /// 新角色 ID（可选；必须存在）
    #[serde(alias = "roleId")]
    pub role_id: Option<i32>,
}

/// POST /api/user/update?userId=2&username=xx&roleId=2
///
/// 补齐「编辑用户 / 改角色」。此前既无该端点，`db::update_user_role` /
/// `db::update_username` 也已沦为零调用死函数 —— 用户管理页只能新增和删除，
/// 连改个角色都做不到。只更新传入的字段。
#[utoipa::path(
    post,
    path = "/api/user/update",
    tag = "user",
    operation_id = "user_update",
    params(UpdateUserParams),
    responses(
        (status = 200, description = "更新成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "缺少 userId / 用户名为空 / 用户名重复 / 角色不存在 / 没有要更新的字段"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<UpdateUserParams>,
) -> Result<Json<ApiResult<()>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let user_id = params
        .user_id
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 userId"))?;

    let username = match params.username.as_deref() {
        Some(raw) => {
            let name = raw.trim();
            if name.is_empty() {
                return Err(AppError::business(ErrorCode::Error400, "用户名不可为空"));
            }
            if name.chars().count() > 64 {
                return Err(AppError::business(
                    ErrorCode::Error400,
                    "用户名过长（最多 64 字符）",
                ));
            }
            // 唯一索引 uk_user_username 会抛原始 SQL 错误（500）—— 先拦成 400。
            if db::username_taken(&state.pool, name, Some(user_id)).await? {
                return Err(AppError::business(ErrorCode::Error400, "用户名已存在"));
            }
            Some(name)
        }
        None => None,
    };

    if let Some(role_id) = params.role_id {
        if !db::role_exists(&state.pool, role_id).await? {
            return Err(AppError::business(ErrorCode::Error400, "角色不存在"));
        }
    }

    if username.is_none() && params.role_id.is_none() {
        return Err(AppError::business(ErrorCode::Error400, "没有要更新的字段"));
    }

    let n = db::update_user(&state.pool, user_id, username, params.role_id).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "用户不存在或更新失败"));
    }
    Ok(Json(ApiResult::<()>::success_empty()))
}

/// POST /api/user/add?username=xx&password=xx&roleId=1
#[utoipa::path(
    post,
    path = "/api/user/add",
    tag = "user",
    operation_id = "user_add",
    params(AddUserParams),
    responses(
        (status = 200, description = "新增成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "用户名/密码为空、用户名过长、用户名已存在、缺少 roleId、角色不存在"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn add_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AddUserParams>,
) -> Result<Json<ApiResult<()>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let username = params
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "用户名不可为空"))?;
    if username.chars().count() > 64 {
        return Err(AppError::business(
            ErrorCode::Error400,
            "用户名过长（最多 64 字符）",
        ));
    }
    let password = params
        .password
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "密码不可为空"))?;
    let role_id = params
        .role_id
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 roleId"))?;

    let exists = db::role_exists(&state.pool, role_id).await?;
    if !exists {
        return Err(AppError::business(ErrorCode::Error400, "角色不存在"));
    }

    // 先查重名：`gb_user` 上有唯一索引 `uk_user_username`，此前不预检，
    // 用户会收到 500 + 原始 SQL 报错（"UNIQUE constraint failed: gb_user.username"），
    // 既泄漏库内部信息、前端也无法给出友好提示。
    if db::username_taken(&state.pool, username, None).await? {
        return Err(AppError::business(ErrorCode::Error400, "用户名已存在"));
    }

    // Phase 7.6: store password as Argon2id hash instead of plaintext MD5.
    // 2026-09-12: 先统一成"客户端登录时会送的那个秘密值"（md5(明文)）再哈希，
    // 否则库里是 Argon2(明文) 而登录送 md5(明文)，新建用户永远登不上。
    let password_hash = crate::auth::hash_password(&crate::auth::password_secret(password))
        .map_err(|e| AppError::business(ErrorCode::Error100, format!("hash failed: {}", e)))?;
    let push_key = md5_hex(&format!("{}{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(), password));
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = db::add_user(&state.pool, username, &password_hash, role_id, &push_key, &now).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "添加失败"));
    }
    Ok(Json(ApiResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct AddUserParams {
    /// 用户名（≤ 64 字符，不允许重复）
    pub username: Option<String>,
    /// 密码（前端可送明文或 MD5；服务端以 Argon2id 存储）
    pub password: Option<String>,
    /// 角色 ID（必须已存在）
    #[serde(alias = "roleId")]
    pub role_id: Option<i32>,
}

/// DELETE /api/user/delete?id=1
#[utoipa::path(
    delete,
    path = "/api/user/delete",
    tag = "user",
    operation_id = "user_delete",
    params(DeleteQuery),
    responses(
        (status = 200, description = "删除成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "缺少 id / 不能删除当前登录账号"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<DeleteQuery>,
) -> Result<Json<ApiResult<()>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let id = q.id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 id"))?;
    let (_, me) = authz::current_user(&state, &headers).await?;
    if me.id == id {
        // 明确拦自己：此前靠 `db::delete_user` 里 SQL 的 `WHERE id != 1` 兜住，
        // 报的还是含义不明的"删除失败"，且只保护了 id=1 这一个账号。
        return Err(AppError::business(ErrorCode::Error400, "不能删除当前登录账号"));
    }
    let n = db::delete_user(&state.pool, id).await?;
    if n == 0 {
        return Err(AppError::business(
            ErrorCode::Error100,
            "删除失败（用户不存在，或为受保护的内置账号）",
        ));
    }
    Ok(Json(ApiResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteQuery {
    /// 要删除的用户 ID
    pub id: Option<i32>,
}

/// POST /api/user/changePassword?oldPassword=xx&password=xx
#[utoipa::path(
    post,
    path = "/api/user/changePassword",
    tag = "user",
    operation_id = "user_change_password",
    params(ChangePasswordParams),
    responses(
        (status = 200, description = "修改成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "缺少 oldPassword / password"),
        (status = 401, description = "未鉴权 / 旧密码错误"),
    ),
    security(("access_token" = [])),
)]
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChangePasswordParams>,
) -> Result<Json<ApiResult<()>>, AppError> {
    let token = crate::auth::extract_token_from_headers(&headers).ok_or(AppError::Unauthorized)?;
    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    let claims = keys.verify_token(&token).ok_or(AppError::Unauthorized)?;
    let old_md5 = params.old_password.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 oldPassword"))?;
    let new_pwd = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 password"))?;

    let user = db::find_by_username(&state.pool, &claims.userName).await?.ok_or(AppError::Unauthorized)?;
    // 旧口令同样走兼容校验：
    // 此前这里是 `current_md5 != old_md5` 的明文比较，导致存 Argon2id 的新用户
    // **永远无法修改自己的密码**（Argon2 串不可能等于 MD5 十六进制）。
    let stored = user.password.clone().unwrap_or_default();
    if !crate::auth::verify_password_flexible(old_md5.as_str(), &stored) {
        return Err(AppError::business(ErrorCode::Error100, "旧密码错误"));
    }
    // 新口令存 Argon2id（此前写 MD5，等于把新建时的 Argon2id 降级）
    let new_hash = crate::auth::hash_password(&crate::auth::password_secret(new_pwd))
        .map_err(|e| AppError::business(ErrorCode::Error100, format!("hash failed: {}", e)))?;
    let n = db::change_password(&state.pool, user.id, &new_hash).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "修改失败"));
    }
    Ok(Json(ApiResult::<()>::success_empty()))
}

/// 旧写法是「再声明一个 camelCase 同义字段 + `#[serde(rename)]`」，
/// 会出现两个字段映射到同一个 JSON 键（serde 报 unreachable pattern），
/// 且调用方要靠 `a.or(b)` 兜。现已统一为 snake_case 主名 + camelCase alias。
#[derive(Debug, Deserialize, IntoParams)]
pub struct ChangePasswordParams {
    /// 旧密码（前端送 MD5(明文)，与登录口径一致）
    #[serde(alias = "oldPassword")]
    pub old_password: Option<String>,
    /// 新密码（明文或 MD5，服务端统一以 Argon2id 存储）
    pub password: Option<String>,
}

/// POST /api/user/changePasswordForAdmin?userId=2&password=xx
#[utoipa::path(
    post,
    path = "/api/user/changePasswordForAdmin",
    tag = "user",
    operation_id = "user_change_password_admin",
    params(ChangePasswordForAdminParams),
    responses(
        (status = 200, description = "重置成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "缺少 userId / password"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn change_password_for_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChangePasswordForAdminParams>,
) -> Result<Json<ApiResult<()>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let user_id = params.user_id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 userId"))?;
    let password = params.password.as_deref().ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 password"))?;
    // 管理员重置口令也存 Argon2id（此前写 MD5）；秘密值同样取 md5(明文)，
    // 与登录侧（送 md5）保持一致，否则被重置的账号从此登录不上。
    let new_hash = crate::auth::hash_password(&crate::auth::password_secret(password))
        .map_err(|e| AppError::business(ErrorCode::Error100, format!("hash failed: {}", e)))?;
    let n = db::change_password(&state.pool, user_id, &new_hash).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "修改失败"));
    }
    Ok(Json(ApiResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ChangePasswordForAdminParams {
    /// 目标用户 ID
    #[serde(alias = "userId")]
    pub user_id: Option<i32>,
    /// 新密码（明文或 MD5，服务端统一以 Argon2id 存储）
    pub password: Option<String>,
}

/// POST /api/user/changePushKey?userId=2&pushKey=xx
#[utoipa::path(
    post,
    path = "/api/user/changePushKey",
    tag = "user",
    operation_id = "user_change_push_key",
    params(ChangePushKeyParams),
    responses(
        (status = 200, description = "修改成功",
         body = ApiResult<serde_json::Value>),
        (status = 400, description = "缺少 userId / pushKey"),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn change_push_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ChangePushKeyParams>,
) -> Result<Json<ApiResult<()>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    let user_id = params.user_id.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 userId"))?;
    let push_key = params.push_key.ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 pushKey"))?;
    let n = db::change_push_key(&state.pool, user_id, &push_key).await?;
    if n == 0 {
        return Err(AppError::business(ErrorCode::Error100, "修改失败"));
    }
    Ok(Json(ApiResult::<()>::success_empty()))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ChangePushKeyParams {
    /// 目标用户 ID
    #[serde(alias = "userId")]
    pub user_id: Option<i32>,
    /// 新 pushKey（用于推流鉴权）
    #[serde(alias = "pushKey")]
    pub push_key: Option<String>,
}

// 权限判定集中在 `handlers::authz`：原先这里内联的 `require_admin` 只认
// `role_id == 1`，而 `/api/role/add` 允许创建 `authority="0"` 的角色 —— 那些
// 角色的成员会被误判为非管理员。现按 `authority` 判定，内置角色 id=1 仍兜底。
use crate::handlers::authz;

#[cfg(all(test, feature = "sqlite"))]
mod password_flow_tests {
    use super::*;
    use crate::test_support::app_state;

    async fn admin_headers(state: &AppState) -> HeaderMap {
        // 种子里 admin/admin 是 MD5 行，登录送 md5(明文)
        let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = keys.create_token("admin", 60).expect("token");
        let mut h = HeaderMap::new();
        h.insert("access-token", token.parse().expect("header value"));
        h
    }

    fn md5(s: &str) -> String {
        crate::auth::legacy_md5(s)
    }

    async fn stored(name: &str, state: &AppState) -> String {
        db::find_by_username(&state.pool, name)
            .await
            .unwrap()
            .unwrap()
            .password
            .unwrap_or_default()
    }

    /// 回归保护：界面「新增用户」送**明文**，登录送 **md5(明文)** —— 两边必须对得上。
    ///
    /// 修复前库里是 `Argon2(明文)`，登录送 md5，新建用户永远登录不上。
    #[tokio::test]
    async fn test_add_then_login_with_md5() {
        let state = app_state().await;
        let headers = admin_headers(&state).await;

        let _ = add_user(
            State(state.clone()),
            headers.clone(),
            Query(AddUserParams {
                username: Some("u1".into()),
                password: Some("plain123".into()),
                role_id: Some(1),
            }),
        )
        .await
        .expect("新增用户应成功");

        let s = stored("u1", &state).await;
        assert!(crate::auth::is_argon2_hash(&s), "必须是 Argon2id: {s}");
        // 登录页面/JS 送的是 md5
        assert!(crate::auth::verify_password_flexible(&md5("plain123"), &s));
        // 外部接口直接送明文也应通过
        assert!(crate::auth::verify_password_flexible("plain123", &s));
        assert!(!crate::auth::verify_password_flexible("wrong", &s));
    }

    /// 管理员「重置」后，被重置的账号必须能用新口令登录。
    #[tokio::test]
    async fn test_admin_reset_then_login() {
        let state = app_state().await;
        let headers = admin_headers(&state).await;
        let _ = add_user(
            State(state.clone()),
            headers.clone(),
            Query(AddUserParams {
                username: Some("u1".into()),
                password: Some("plain123".into()),
                role_id: Some(1),
            }),
        )
        .await
        .unwrap();
        let uid = db::find_by_username(&state.pool, "u1").await.unwrap().unwrap().id;

        let _ = change_password_for_admin(
            State(state.clone()),
            headers.clone(),
            Query(ChangePasswordForAdminParams {
                user_id: Some(uid),
                password: Some("reset999".into()),
            }),
        )
        .await
        .expect("重置应成功");

        let s = stored("u1", &state).await;
        assert!(crate::auth::verify_password_flexible(&md5("reset999"), &s), "重置后必须能用新口令登录");
        assert!(!crate::auth::verify_password_flexible(&md5("plain123"), &s), "旧口令必须失效");
    }

    /// 自助改密：`oldPassword` 走登录口径（md5），改完必须能用新口令登录。
    #[tokio::test]
    async fn test_self_change_password_then_login() {
        let state = app_state().await;
        let headers = admin_headers(&state).await;
        let _ = add_user(
            State(state.clone()),
            headers.clone(),
            Query(AddUserParams {
                username: Some("u1".into()),
                password: Some("plain123".into()),
                role_id: Some(1),
            }),
        )
        .await
        .unwrap();

        // 用 u1 自己的 token（前端登录后保存的）
        let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = keys.create_token("u1", 60).unwrap();
        let mut h = HeaderMap::new();
        h.insert("access-token", token.parse().unwrap());

        // 前端 api 层会把 oldPassword md5 后再发
        let _ = change_password(
            State(state.clone()),
            h,
            Query(ChangePasswordParams {
                old_password: Some(md5("plain123")),
                password: Some("brandnew1".into()),
            }),
        )
        .await
        .expect("改密应成功");

        let s = stored("u1", &state).await;
        assert!(crate::auth::verify_password_flexible(&md5("brandnew1"), &s));
        assert!(!crate::auth::verify_password_flexible(&md5("plain123"), &s));
    }

    /// 旧密码错误必须被拒绝（且不改动已有口令）。
    #[tokio::test]
    async fn test_self_change_password_rejects_wrong_old() {
        let state = app_state().await;
        let headers = admin_headers(&state).await;
        let _ = add_user(
            State(state.clone()),
            headers.clone(),
            Query(AddUserParams {
                username: Some("u1".into()),
                password: Some("plain123".into()),
                role_id: Some(1),
            }),
        )
        .await
        .unwrap();
        let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = keys.create_token("u1", 60).unwrap();
        let mut h = HeaderMap::new();
        h.insert("access-token", token.parse().unwrap());

        let err = change_password(
            State(state.clone()),
            h,
            Query(ChangePasswordParams {
                old_password: Some(md5("nope")),
                password: Some("brandnew1".into()),
            }),
        )
        .await
        .expect_err("旧密码错误应报错");
        assert!(matches!(err, AppError::Business(_, _)));
        let s = stored("u1", &state).await;
        assert!(crate::auth::verify_password_flexible(&md5("plain123"), &s), "口令不应被改动");
    }
}

/// `GET /api/user/all` —— 不分页返回全部用户，
/// 供「角色/分组分配」等下拉框使用。
#[utoipa::path(
    get,
    path = "/api/user/all",
    tag = "user",
    operation_id = "user_list_all",
    responses(
        (status = 200, description = "全部用户 `{list,total}`，含 pushKey",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"list":[{"id":1,"username":"admin","roleId":1,"createTime":"2026-01-01 00:00:00","updateTime":"2026-01-01 00:00:00","pushKey":"xxx"}],"total":1}})),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "需要管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn all_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    // 同样涉及 pushKey 等敏感字段，仅管理员可取。
    authz::require_admin(&state, &headers).await?;
    let users = db::get_all_users(&state.pool).await?;
    let rows: Vec<serde_json::Value> = users
        .iter()
        .map(|u| {
            serde_json::json!({
                "id": u.id,
                "username": u.username,
                "roleId": u.role_id,
                "createTime": u.create_time,
                "updateTime": u.update_time,
                "pushKey": u.push_key,
            })
        })
        .collect();
    Ok(Json(ApiResult::success(serde_json::json!({
        "list": rows,
        "total": rows.len(),
    }))))
}
