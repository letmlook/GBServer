//! 行政区域表 wvp_common_region

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Region {
    pub id: i64,
    pub device_id: String,
    pub name: String,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Deserialize)]
pub struct RegionAdd {
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegionUpdate {
    pub id: Option<i64>,
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
}

pub async fn list_all(pool: &Pool) -> sqlx::Result<Vec<Region>> {
    sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region ORDER BY id",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<Region>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

pub async fn get_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<Option<Region>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region WHERE device_id = $1",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await;
}

pub async fn list_children(pool: &Pool, parent_id: i32) -> sqlx::Result<Vec<Region>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region WHERE parent_id = ? ORDER BY id",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Region>(
        "SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM wvp_common_region WHERE parent_id = $1 ORDER BY id",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await;
}

pub async fn add(
    pool: &Pool,
    device_id: &str,
    name: &str,
    parent_id: Option<i32>,
    parent_device_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO wvp_common_region (device_id, name, parent_id, parent_device_id, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO wvp_common_region (device_id, name, parent_id, parent_device_id, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn update(
    pool: &Pool,
    id: i64,
    device_id: Option<&str>,
    name: Option<&str>,
    parent_id: Option<i32>,
    parent_device_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE wvp_common_region SET device_id = COALESCE(?, device_id), name = COALESCE(?, name), parent_id = ?, parent_device_id = COALESCE(?, parent_device_id), update_time = ? WHERE id = ?",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE wvp_common_region SET device_id = COALESCE($1, device_id), name = COALESCE($2, name), parent_id = $3, parent_device_id = COALESCE($4, parent_device_id), update_time = $5 WHERE id = $6",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_common_region WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_common_region WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn delete_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_common_region WHERE device_id = ?")
        .bind(device_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_common_region WHERE device_id = $1")
        .bind(device_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}
