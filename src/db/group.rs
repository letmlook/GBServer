//! 分组表 gb_common_group

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Group {
    pub id: i32,
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
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    pub name: Option<String>,
    #[serde(alias = "parentId")]
    pub parent_id: Option<i32>,
    #[serde(alias = "parentDeviceId")]
    pub parent_device_id: Option<String>,
    #[serde(alias = "businessGroup")]
    pub business_group: Option<String>,
    #[serde(alias = "civilCode")]
    pub civil_code: Option<String>,
}

/// 更新请求体（同 `RegionUpdate`：必须带 camelCase 别名，
/// 否则只有 `name` 生效、`parent_id` 被清成 NULL → 分组被抬到根级）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    let sql = crate::db::dialect::current_sql("SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM gb_common_group ORDER BY id").into_owned();

    crate::dispatch!(pool, |p| { sqlx::query_as::<_, Group>(&sql,).fetch_all(p) })
}

pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<Group>> {
    let sql = crate::db::dialect::current_sql("SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM gb_common_group WHERE id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Group>(&sql).bind(id).fetch_optional(p)
    });
}

pub async fn get_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<Option<Group>> {
    let sql = crate::db::dialect::current_sql("SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM gb_common_group WHERE device_id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Group>(&sql)
            .bind(device_id)
            .fetch_optional(p)
    });
}

pub async fn list_children(pool: &Pool, parent_id: i32) -> sqlx::Result<Vec<Group>> {
    let sql = crate::db::dialect::current_sql("SELECT id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code FROM gb_common_group WHERE parent_id = ? ORDER BY id").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Group>(&sql)
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
    business_group: &str,
    now: &str,
    civil_code: Option<&str>,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("INSERT INTO gb_common_group (device_id, name, parent_id, parent_device_id, business_group, create_time, update_time, civil_code) VALUES (?, ?, ?, ?, ?, ?, ?, ?)").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(device_id)
            .bind(name)
            .bind(parent_id)
            .bind(parent_device_id)
            .bind(business_group)
            .bind(now)
            .bind(now)
            .bind(civil_code)
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
    business_group: Option<&str>,
    civil_code: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("UPDATE gb_common_group SET device_id = COALESCE(?, device_id), name = COALESCE(?, name), parent_id = COALESCE(?, parent_id), parent_device_id = COALESCE(?, parent_device_id), business_group = COALESCE(?, business_group), civil_code = COALESCE(?, civil_code), update_time = ? WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(device_id)
            .bind(name)
            .bind(parent_id)
            .bind(parent_device_id)
            .bind(business_group)
            .bind(civil_code)
            .bind(now)
            .bind(id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    let sql =
        crate::db::dialect::current_sql("DELETE FROM gb_common_group WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 统计分组数量
pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql("SELECT COUNT(*) FROM gb_common_group").into_owned();

    crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    })
}

/// 根据设备ID删除分组
pub async fn delete_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("DELETE FROM gb_common_group WHERE device_id = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(device_id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 获取子分组数量
pub async fn count_children(pool: &Pool, parent_id: i32) -> sqlx::Result<i64> {
    let sql =
        crate::db::dialect::current_sql("SELECT COUNT(*) FROM gb_common_group WHERE parent_id = ?")
            .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(parent_id)
            .fetch_one(p)
    });
}
