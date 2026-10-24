//! 平台与区域关系表 gb_platform_region

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

/// 平台区域关系结构体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlatformRegion {
    pub id: i64,
    #[serde(alias = "platformId")]
    pub platform_id: Option<i32>,
    #[serde(alias = "regionId")]
    pub region_id: Option<i32>,
}

/// 添加平台区域关系
pub async fn add(pool: &Pool, platform_id: i32, region_id: i32) -> sqlx::Result<i64> {
    // `RETURNING id` 只有 PostgreSQL / SQLite(≥3.35) 支持，**MySQL 不支持**
    // —— MySQL 走 `execute()` 的自增 id。
    //
    // 改造前这里是三个 `#[cfg(feature = ...)]` 分支（编译期三选一），
    // 现在按**运行期方言**分支。
    if crate::db::dialect::current().is_mysql() {
        let sql = crate::db::dialect::current_sql(
            "INSERT INTO gb_platform_region (platform_id, region_id) VALUES (?, ?)",
        )
        .into_owned();
        let r = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
                .bind(platform_id)
                .bind(region_id)
                .execute(p)
        })?;
        // MySQL 的自增 id 走 `last_insert_id()`（方言专有 API）
        return Ok(r.last_insert_id().unwrap_or(0) as i64);
    }

    let sql = crate::db::dialect::current_sql(
        "INSERT INTO gb_platform_region (platform_id, region_id) VALUES (?, ?) RETURNING id",
    )
    .into_owned();
    let row: (i64,) = crate::dispatch!(pool, |p| {
        sqlx::query_as(&sql)
            .bind(platform_id)
            .bind(region_id)
            .fetch_one(p)
    })?;
    Ok(row.0)
}

pub async fn list_by_platform(pool: &Pool, platform_id: i32) -> sqlx::Result<Vec<PlatformRegion>> {
    let sql = crate::db::dialect::current_sql(
        "SELECT id, platform_id, region_id FROM gb_platform_region WHERE platform_id = ?",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, PlatformRegion>(&sql)
            .bind(platform_id)
            .fetch_all(p)
    });
}

/// 删除平台区域关系
pub async fn delete(pool: &Pool, platform_id: i32, region_id: i32) -> sqlx::Result<bool> {
    let sql = crate::db::dialect::current_sql(
        "DELETE FROM gb_platform_region WHERE platform_id = ? AND region_id = ?",
    )
    .into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(platform_id)
            .bind(region_id)
            .execute(p)
    })?;

    Ok(result.rows_affected() > 0)
}

/// 删除平台的所有区域关系
pub async fn delete_by_platform(pool: &Pool, platform_id: i32) -> sqlx::Result<u64> {
    let sql =
        crate::db::dialect::current_sql("DELETE FROM gb_platform_region WHERE platform_id = ?")
            .into_owned();

    let result =
        crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(platform_id).execute(p) })?;

    Ok(result.rows_affected())
}

/// 批量添加平台区域关系
pub async fn batch_add(pool: &Pool, platform_id: i32, region_ids: &[i32]) -> sqlx::Result<u64> {
    let mut count = 0u64;
    for region_id in region_ids {
        if add(pool, platform_id, *region_id).await.is_ok() {
            count += 1;
        }
    }
    Ok(count)
}
