//! 用户 API Key 表 wvp_user_api_key

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UserApiKey {
    pub id: i32,
    pub user_id: Option<i64>,
    pub app: Option<String>,
    pub api_key: Option<String>,
    pub expired_at: Option<i64>,
    pub remark: Option<String>,
    pub enable: Option<bool>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserApiKeyAdd {
    #[serde(alias = "userId")]
    pub user_id: Option<i64>,
    pub app: Option<String>,
    #[serde(alias = "expiresAt")]
    pub expired_at: Option<i64>,
    pub enable: Option<bool>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserApiKeyRemark {
    pub id: Option<i64>,
    pub remark: Option<String>,
}

pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<UserApiKey>> {
    let offset = (page.saturating_sub(1)) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, UserApiKey>(
        "SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM wvp_user_api_key ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, UserApiKey>(
        "SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM wvp_user_api_key ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_user_api_key")
        .fetch_one(pool)
        .await
}

pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<UserApiKey>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, UserApiKey>(
        "SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM wvp_user_api_key WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, UserApiKey>(
        "SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM wvp_user_api_key WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

pub async fn add(
    pool: &Pool,
    user_id: i64,
    app: &str,
    api_key: &str,
    expired_at: Option<i64>,
    enable: bool,
    remark: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO wvp_user_api_key (user_id, app, api_key, expired_at, remark, enable, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(app)
    .bind(api_key)
    .bind(expired_at)
    .bind(remark)
    .bind(enable)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO wvp_user_api_key (user_id, app, api_key, expired_at, remark, enable, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(user_id)
    .bind(app)
    .bind(api_key)
    .bind(expired_at)
    .bind(remark)
    .bind(enable)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn update_remark(
    pool: &Pool,
    id: i64,
    remark: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_user_api_key SET remark = ?, update_time = ? WHERE id = ?")
        .bind(remark)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_user_api_key SET remark = $1, update_time = $2 WHERE id = $3")
        .bind(remark)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn set_enable(pool: &Pool, id: i32, enable: bool, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_user_api_key SET enable = ?, update_time = ? WHERE id = ?")
        .bind(enable)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_user_api_key SET enable = $1, update_time = $2 WHERE id = $3")
        .bind(enable)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn reset_api_key(
    pool: &Pool,
    id: i32,
    new_key: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_user_api_key SET api_key = ?, update_time = ? WHERE id = ?")
        .bind(new_key)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_user_api_key SET api_key = $1, update_time = $2 WHERE id = $3")
        .bind(new_key)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_user_api_key WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_user_api_key WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}
