//! 录像计划表 wvp_record_plan, wvp_record_plan_item

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RecordPlan {
    pub id: i32,
    pub snap: Option<bool>,
    pub name: String,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RecordPlanItem {
    pub id: i32,
    pub start: Option<i32>,
    pub stop: Option<i32>,
    pub week_day: Option<i32>,
    pub plan_id: Option<i32>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordPlanAdd {
    pub name: Option<String>,
    pub snap: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RecordPlanUpdate {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub snap: Option<bool>,
}

pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<RecordPlan>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM wvp_record_plan WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM wvp_record_plan WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<RecordPlan>> {
    let offset = (page.saturating_sub(1)) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM wvp_record_plan ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM wvp_record_plan ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_record_plan")
        .fetch_one(pool)
        .await
}

pub async fn list_items(pool: &Pool, plan_id: i64) -> sqlx::Result<Vec<RecordPlanItem>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, RecordPlanItem>(
        "SELECT id, start, stop, week_day, plan_id, create_time, update_time FROM wvp_record_plan_item WHERE plan_id = ? ORDER BY id",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, RecordPlanItem>(
        "SELECT id, start, stop, week_day, plan_id, create_time, update_time FROM wvp_record_plan_item WHERE plan_id = $1 ORDER BY id",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await;
}

pub async fn add(pool: &Pool, name: &str, snap: bool, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO wvp_record_plan (name, snap, create_time, update_time) VALUES (?, ?, ?, ?)",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO wvp_record_plan (name, snap, create_time, update_time) VALUES ($1, $2, $3, $4)",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn update(
    pool: &Pool,
    id: i64,
    name: Option<&str>,
    snap: Option<bool>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE wvp_record_plan SET name = COALESCE(?, name), snap = COALESCE(?, snap), update_time = ? WHERE id = ?",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE wvp_record_plan SET name = COALESCE($1, name), snap = COALESCE($2, snap), update_time = $3 WHERE id = $4",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    {
        let _ = sqlx::query("DELETE FROM wvp_record_plan_item WHERE plan_id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        let r = sqlx::query("DELETE FROM wvp_record_plan WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(r.rows_affected());
    }
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("DELETE FROM wvp_record_plan_item WHERE plan_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        let r = sqlx::query("DELETE FROM wvp_record_plan WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(r.rows_affected());
    }
}

pub async fn link_channel(
    pool: &Pool,
    channel_id: i64,
    plan_id: Option<i64>,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET record_plan_id = ? WHERE id = ?")
        .bind(plan_id.map(|x| x as i32))
        .bind(channel_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET record_plan_id = $1 WHERE id = $2")
        .bind(plan_id.map(|x| x as i32))
        .bind(channel_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}
