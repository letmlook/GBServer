//! 级联平台 wvp_platform

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Platform {
    pub id: i64,
    pub enable: Option<bool>,
    pub name: Option<String>,
    pub server_gb_id: Option<String>,
    pub server_ip: Option<String>,
    pub server_port: Option<i32>,
    pub device_gb_id: Option<String>,
    pub status: Option<bool>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<Platform>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Platform>(
        "SELECT id, enable, name, server_gb_id, server_ip, server_port, device_gb_id, status, create_time, update_time FROM wvp_platform ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Platform>(
        "SELECT id, enable, name, server_gb_id, server_ip, server_port, device_gb_id, status, create_time, update_time FROM wvp_platform ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_platform")
        .fetch_one(pool)
        .await
}
