//! 行政区域表 gb_common_region

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Region {
    pub id: i32,
    pub device_id: String,
    pub name: String,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Deserialize)]
pub struct RegionAdd {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    pub name: Option<String>,
    #[serde(alias = "parentId")]
    pub parent_id: Option<i32>,
    #[serde(alias = "parentDeviceId")]
    pub parent_device_id: Option<String>,
}

/// 更新请求体。前端用 camelCase，
/// 这里必须与 `RegionAdd` 一样带别名 —— 否则 `deviceId`/`parentId`/`parentDeviceId`
/// 全部绑定失败，只有 `name` 生效，而 `parent_id` 会被写成 NULL
/// （`build_region_tree` 把 `parent_id.is_none()` 当根节点 → 节点被抬到根级）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionUpdate {
    pub id: Option<i64>,
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_device_id: Option<String>,
}

pub async fn list_all(pool: &Pool) -> sqlx::Result<Vec<Region>> {
    let sql = pool.sql("SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM gb_common_region ORDER BY id").into_owned();

    crate::dispatch!(pool, |p| { sqlx::query_as::<_, Region>(&sql).fetch_all(p) })
}

pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<Region>> {
    let sql = pool.sql("SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM gb_common_region WHERE id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Region>(&sql).bind(id).fetch_optional(p)
    });
}

pub async fn get_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<Option<Region>> {
    let sql = pool.sql("SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM gb_common_region WHERE device_id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Region>(&sql)
            .bind(device_id)
            .fetch_optional(p)
    });
}

pub async fn list_children(pool: &Pool, parent_id: i32) -> sqlx::Result<Vec<Region>> {
    let sql = pool.sql("SELECT id, device_id, name, parent_id, parent_device_id, create_time, update_time FROM gb_common_region WHERE parent_id = ? ORDER BY id").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Region>(&sql)
            .bind(parent_id)
            .fetch_all(p)
    });
}

pub async fn add(
    pool: &Pool,
    device_id: &str,
    name: &str,
    parent_id: Option<i32>,
    parent_device_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = pool.sql("INSERT INTO gb_common_region (device_id, name, parent_id, parent_device_id, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?)").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(device_id)
            .bind(name)
            .bind(parent_id)
            .bind(parent_device_id)
            .bind(now)
            .bind(now)
            .execute(p)
    })?;
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
    let sql = pool.sql("UPDATE gb_common_region SET device_id = COALESCE(?, device_id), name = COALESCE(?, name), parent_id = COALESCE(?, parent_id), parent_device_id = COALESCE(?, parent_device_id), update_time = ? WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(device_id)
            .bind(name)
            .bind(parent_id)
            .bind(parent_device_id)
            .bind(now)
            .bind(id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_common_region WHERE id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

pub async fn delete_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_common_region WHERE device_id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(device_id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 统计区域数量
pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    let sql = pool
        .sql("SELECT COUNT(*) FROM gb_common_region")
        .into_owned();

    crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    })
}

/// 获取子区域数量
pub async fn count_children(pool: &Pool, parent_id: i32) -> sqlx::Result<i64> {
    let sql = pool
        .sql("SELECT COUNT(*) FROM gb_common_region WHERE parent_id = ?")
        .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(parent_id)
            .fetch_one(p)
    });
}
