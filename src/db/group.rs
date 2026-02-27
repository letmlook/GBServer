//! 分组表 wvp_common_group

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Group {
    pub id: i64,
    pub device_id: String,
    pub name: String,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
    pub business_group: String,
    pub create_time: String,
    pub update_time: String,
    pub civil_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GroupAdd {
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
    pub business_group: Option<String>,
    pub civil_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GroupUpdate {
    pub id: Option<i64>,
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
    pub business_group: Option<String>,
    pub civil_code: Option<String>,
}

pub async fn list_all(pool: &Pool) -> sqlx::Result<Vec<Group>> {
    sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group ORDER BY id",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<Group>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

pub async fn get_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<Option<Group>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group WHERE device_id = $1",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await;
}

pub async fn list_children(pool: &Pool, parent_id: i32) -> sqlx::Result<Vec<Group>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group WHERE parent_id = ? ORDER BY id",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Group>(
        "SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM wvp_common_group WHERE parent_id = $1 ORDER BY id",
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
    business_group: &str,
    now: &str,
    civil_code: Option<&str>,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO wvp_common_group (device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(business_group)
    .bind(now)
    .bind(now)
    .bind(civil_code)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO wvp_common_group (device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(business_group)
    .bind(now)
    .bind(now)
    .bind(civil_code)
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
    business_group: Option<&str>,
    civil_code: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE wvp_common_group SET device_id = COALESCE(?, device_id), name = COALESCE(?, name), parent_id = ?, parent_device_id = COALESCE(?, parent_device_id), business_group = COALESCE(?, business_group), civil_code = COALESCE(?, civil_code), update_time = ? WHERE id = ?",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(business_group)
    .bind(civil_code)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE wvp_common_group SET device_id = COALESCE($1, device_id), name = COALESCE($2, name), parent_id = $3, parent_device_id = COALESCE($4, parent_device_id), business_group = COALESCE($5, business_group), civil_code = COALESCE($6, civil_code), update_time = $7 WHERE id = $8",
    )
    .bind(device_id)
    .bind(name)
    .bind(parent_id)
    .bind(parent_device_id)
    .bind(business_group)
    .bind(civil_code)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_common_group WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_common_group WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}
