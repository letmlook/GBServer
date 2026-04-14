//! 平台与分组关系表 wvp_platform_group

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

/// 平台分组关系结构体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlatformGroup {
    pub id: i64,
    pub platform_id: Option<i32>,
    pub group_id: Option<i32>,
}

/// 添加平台分组关系
pub async fn add(pool: &Pool, platform_id: i32, group_id: i32) -> sqlx::Result<i64> {
    #[cfg(feature = "postgres")]
    {
        let result: (i64,) = sqlx::query_as(
            "INSERT INTO wvp_platform_group (platform_id, group_id) VALUES ($1, $2) RETURNING id"
        )
        .bind(platform_id)
        .bind(group_id)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_platform_group (platform_id, group_id) VALUES (?, ?)"
        )
        .bind(platform_id)
        .bind(group_id)
        .execute(pool)
        .await?;

        Ok(result.last_insert_id() as i64)
    }
}

/// 查询平台的所有分组
pub async fn list_by_platform(pool: &Pool, platform_id: i32) -> sqlx::Result<Vec<PlatformGroup>> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query_as::<_, PlatformGroup>(
            "SELECT id, platform_id, group_id FROM wvp_platform_group WHERE platform_id = $1"
        )
        .bind(platform_id)
        .fetch_all(pool)
        .await
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query_as::<_, PlatformGroup>(
            "SELECT id, platform_id, group_id FROM wvp_platform_group WHERE platform_id = ?"
        )
        .bind(platform_id)
        .fetch_all(pool)
        .await
    }
}

/// 删除平台分组关系
pub async fn delete(pool: &Pool, platform_id: i32, group_id: i32) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query(
            "DELETE FROM wvp_platform_group WHERE platform_id = $1 AND group_id = $2"
        )
        .bind(platform_id)
        .bind(group_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "DELETE FROM wvp_platform_group WHERE platform_id = ? AND group_id = ?"
        )
        .bind(platform_id)
        .bind(group_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// 删除平台的所有分组关系
pub async fn delete_by_platform(pool: &Pool, platform_id: i32) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("DELETE FROM wvp_platform_group WHERE platform_id = $1")
            .bind(platform_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM wvp_platform_group WHERE platform_id = ?")
            .bind(platform_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// 批量添加平台分组关系
pub async fn batch_add(pool: &Pool, platform_id: i32, group_ids: &[i32]) -> sqlx::Result<u64> {
    let mut count = 0u64;
    for group_id in group_ids {
        if add(pool, platform_id, *group_id).await.is_ok() {
            count += 1;
        }
    }
    Ok(count)
}
