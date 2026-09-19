//! 管理员权限判定。
//!
//! 背景：`require_admin` 原先内联在 `handlers/user.rs`，实现是**硬编码
//! `role_id == 1`**，且只被 4 个端点调用 —— `/api/role/*`、`/api/userApiKey/*`、
//! `/api/user/all` 等管理端点只挂了 JWT 中间件，**任何已登录用户都能调**（实测
//! 普通用户可创建 authority=0 的管理员级角色）。
//!
//! 这里集中两个概念，避免各 handler 各自为政：
//!
//! * [`is_admin`]：**判定规则**。`authority == "0"` 为管理员；`role_id == 1`
//!   （内置 admin 角色）始终视为管理员，兼容种子里 `authority` 缺失的历史数据。
//!   之所以不能只看 `role_id == 1`：`/api/role/add` 允许创建 `authority="0"` 的
//!   角色，若判定只认 id，那些角色的成员会被误判为非管理员。
//! * [`require_admin`]：**强制入口**。拒绝时返回 `ErrorCode::Error403`
//!   （此前用的是 400，语义不对）。

use axum::http::HeaderMap;

use crate::auth::{extract_token_from_headers, Claims, JwtKeys};
use crate::db::{self, User};
use crate::error::{AppError, ErrorCode};
use crate::AppState;

/// 内置管理员角色 id（种子数据里 `gb_user_role.id = 1` 恒为 admin）。
pub const BUILTIN_ADMIN_ROLE_ID: i32 = 1;

/// 角色是否为管理员。
///
/// `authority == "0"` 即管理员（与 WVP 的 `Role.authority` 口径一致）；
/// 内置 admin 角色（id=1）无论 authority 为何都视为管理员。
pub fn is_admin_role(role_id: Option<i32>, authority: Option<&str>) -> bool {
    if role_id == Some(BUILTIN_ADMIN_ROLE_ID) {
        return true;
    }
    matches!(authority.map(str::trim), Some("0"))
}

/// 当前请求主体对应的用户（从 `access-token` 解析）。
///
/// 用户已被删除时返回 401，而不是当成"无权限"—— 否则被删用户会看到 403
/// 而误以为是自己权限不够。
pub async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<(Claims, User), AppError> {
    let token = extract_token_from_headers(headers).ok_or(AppError::Unauthorized)?;
    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    let claims = keys.verify_token(&token).ok_or(AppError::Unauthorized)?;
    let user = db::find_by_username(&state.pool, &claims.userName)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok((claims, user))
}

/// 当前请求主体是否为管理员。
pub async fn is_admin(state: &AppState, headers: &HeaderMap) -> Result<bool, AppError> {
    let (_, user) = current_user(state, headers).await?;
    Ok(is_admin_role(user.role_id, user.role_authority.as_deref()))
}

/// 要求管理员身份；返回请求主体。
///
/// 非管理员 → `ErrorCode::Error403`（HTTP 200 + code 403，与本仓库既有约定一致）。
pub async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let (_, user) = current_user(state, headers).await?;
    if !is_admin_role(user.role_id, user.role_authority.as_deref()) {
        return Err(AppError::business(ErrorCode::Error403, "用户无权限"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 判定口径必须同时覆盖两种数据形态：
    /// * `authority = "0"` —— WVP 口径的管理员（不依赖 id）
    /// * 内置角色 id = 1 —— 历史种子数据里 authority 可能缺失
    #[test]
    fn admin_judgement_covers_authority_and_builtin_role() {
        // 内置 admin 角色：即使 authority 缺失或异常也算管理员
        assert!(is_admin_role(Some(BUILTIN_ADMIN_ROLE_ID), None));
        assert!(is_admin_role(Some(BUILTIN_ADMIN_ROLE_ID), Some("1")));

        // 非内置角色：只看 authority
        assert!(is_admin_role(Some(7), Some("0")));
        assert!(is_admin_role(Some(7), Some(" 0 ")), "首尾空白应被忽略");
        assert!(!is_admin_role(Some(7), Some("1")));
        assert!(!is_admin_role(Some(7), None));
        assert!(!is_admin_role(None, None));
    }

    /// 回归保护：修复前判定是硬编码 `role_id == 1`，于是
    /// `/api/role/add` 建出来的 authority="0" 角色成员会被误判为非管理员，
    /// 出现"有管理员角色却什么都干不了"。
    #[test]
    fn custom_admin_role_is_not_mistaken_for_normal_user() {
        assert!(is_admin_role(Some(9), Some("0")));
    }
}
