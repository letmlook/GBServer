//! 录像计划表 gb_record_plan, gb_record_plan_item

use serde::{Deserialize, Serialize};
#[cfg(feature = "postgres")]
use sqlx::Row;
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
    #[serde(alias = "planItemList")]
    pub plan_item_list: Option<Vec<RecordPlanItemPayload>>,
}

#[derive(Debug, Deserialize)]
pub struct RecordPlanUpdate {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub snap: Option<bool>,
    #[serde(alias = "planItemList")]
    pub plan_item_list: Option<Vec<RecordPlanItemPayload>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordPlanItemPayload {
    pub start: Option<i32>,
    pub stop: Option<i32>,
    #[serde(alias = "weekDay")]
    pub week_day: Option<i32>,
    #[serde(alias = "planId")]
    pub plan_id: Option<i64>,
}

/// 计划条目里 `start`/`stop` 的合法范围：**当天第几分钟**（0..1440）。
///
/// 与 WVP 一致：`RecordPlanItem.start/stop` 是分钟数（`hour*60+minute`），
/// 不是秒。`stop` 允许等于 1440（24:00），此时与"全天"等价。
pub const MAX_MINUTE_OF_DAY: i32 = 1440;

impl RecordPlanItemPayload {
    /// 校验一个计划条目，返回中文错误信息（`None` = 合法）。
    ///
    /// 这里比 WVP **更严**：WVP 的 `update()` 会静默丢弃字段不全的条目，
    /// 而 `start > stop` 的条目会被存下来但**永远不会触发**（SQL 是
    /// `start <= index and stop >= index`）——那正是"保存成功却从不录像"
    /// 这类静默失败。前端也不会给你反着的区间，所以直接报错更有用。
    pub fn validate(&self) -> Option<String> {
        let (Some(start), Some(stop), Some(day)) = (self.start, self.stop, self.week_day) else {
            return Some("录制计划时段必须同时包含 start/stop/weekDay".to_string());
        };
        if !(1..=7).contains(&day) {
            // WVP 用 ISO 口径：LocalDateTime.getDayOfWeek().getValue() → 1=周一 … 7=周日
            return Some(format!("weekDay 必须是 1(周一)~7(周日)，收到 {day}"));
        }
        if !(0..=MAX_MINUTE_OF_DAY).contains(&start) {
            return Some(format!(
                "start 必须是当天第几分钟（0~{}），收到 {start}",
                MAX_MINUTE_OF_DAY
            ));
        }
        if !(0..=MAX_MINUTE_OF_DAY).contains(&stop) {
            return Some(format!(
                "stop 必须是当天第几分钟（0~{}），收到 {stop}",
                MAX_MINUTE_OF_DAY
            ));
        }
        if start > stop {
            return Some(format!(
                "时段起点 {start} 大于终点 {stop}（不支持跨天，请拆成两条时段）"
            ));
        }
        None
    }

    /// WVP `update()` 的行为：三个字段有一个为空就丢弃该条目。
    pub fn is_complete(&self) -> bool {
        self.start.is_some() && self.stop.is_some() && self.week_day.is_some()
    }
}

pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<RecordPlan>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM gb_record_plan WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM gb_record_plan WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM gb_record_plan WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

/// 分页查询计划；`query` 为名称模糊匹配（WVP `/api/record/plan/query` 的
/// `query` 参数，`escape '/'` 语义在本实现里用参数化 `LIKE` 直接表达）。
pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
) -> sqlx::Result<Vec<RecordPlan>> {
    let offset = (page.saturating_sub(1)) * count;
    let like = query.map(|q| format!("%{}%", q));
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM gb_record_plan \
         WHERE (? IS NULL OR name LIKE ?) ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(like.as_deref())
    .bind(like.as_deref())
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM gb_record_plan \
         WHERE ($1::text IS NULL OR name ILIKE $2) ORDER BY id LIMIT $3 OFFSET $4",
    )
    .bind(like.as_deref())
    .bind(like.as_deref())
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, RecordPlan>(
        "SELECT id, snap, name, create_time, update_time FROM gb_record_plan \
         WHERE (? IS NULL OR name LIKE ?) ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(like.as_deref())
    .bind(like.as_deref())
    .bind(count as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_all(pool: &Pool, query: Option<&str>) -> sqlx::Result<i64> {
    let like = query.map(|q| format!("%{}%", q));
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_record_plan WHERE (? IS NULL OR name LIKE ?)",
    )
    .bind(like.as_deref())
    .bind(like.as_deref())
    .fetch_one(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_record_plan WHERE ($1::text IS NULL OR name ILIKE $2)",
    )
    .bind(like.as_deref())
    .bind(like.as_deref())
    .fetch_one(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_record_plan WHERE (? IS NULL OR name LIKE ?)",
    )
    .bind(like.as_deref())
    .bind(like.as_deref())
    .fetch_one(pool)
    .await;
}

/// 计划关联的通道数。
///
/// WVP `/api/record/plan/query` 返回 `channelCount`（SQL 里的
/// `(select count(1) from wvp_device_channel where record_plan_id = wrp.id)`），
/// 前端"关联通道"那一列直接显示它。
pub async fn count_linked_channels(pool: &Pool, plan_id: i64) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_device_channel WHERE record_plan_id = ?",
    )
    .bind(plan_id as i32)
    .fetch_one(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_device_channel WHERE record_plan_id = $1",
    )
    .bind(plan_id as i32)
    .fetch_one(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_device_channel WHERE record_plan_id = ?",
    )
    .bind(plan_id as i32)
    .fetch_one(pool)
    .await;
}

pub async fn list_items(pool: &Pool, plan_id: i64) -> sqlx::Result<Vec<RecordPlanItem>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, RecordPlanItem>(
        "SELECT id, start, stop, week_day, plan_id, create_time, update_time FROM gb_record_plan_item WHERE plan_id = ? ORDER BY id",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, RecordPlanItem>(
        "SELECT id, start, stop, week_day, plan_id, create_time, update_time FROM gb_record_plan_item WHERE plan_id = $1 ORDER BY id",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, RecordPlanItem>(
        "SELECT id, start, stop, week_day, plan_id, create_time, update_time FROM gb_record_plan_item WHERE plan_id = ? ORDER BY id",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await;
}

pub async fn add(pool: &Pool, name: &str, snap: bool, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO gb_record_plan (name, snap, create_time, update_time) VALUES (?, ?, ?, ?)",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO gb_record_plan (name, snap, create_time, update_time) VALUES ($1, $2, $3, $4)",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "INSERT INTO gb_record_plan (name, snap, create_time, update_time) VALUES (?, ?, ?, ?)",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn add_with_id(pool: &Pool, name: &str, snap: bool, now: &str) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(
            "INSERT INTO gb_record_plan (name, snap, create_time, update_time) VALUES (?, ?, ?, ?)",
        )
        .bind(name)
        .bind(snap)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        Ok(r.last_insert_id() as i64)
    }
    #[cfg(feature = "postgres")]
    {
        let row = sqlx::query(
            "INSERT INTO gb_record_plan (name, snap, create_time, update_time) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(name)
        .bind(snap)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i32, _>("id") as i64)
    }
    #[cfg(feature = "sqlite")]
    {
        let r = sqlx::query(
            "INSERT INTO gb_record_plan (name, snap, create_time, update_time) VALUES (?, ?, ?, ?)",
        )
        .bind(name)
        .bind(snap)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        Ok(r.last_insert_rowid())
    }
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
        "UPDATE gb_record_plan SET name = COALESCE(?, name), snap = COALESCE(?, snap), update_time = ? WHERE id = ?",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_record_plan SET name = COALESCE($1, name), snap = COALESCE($2, snap), update_time = $3 WHERE id = $4",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_record_plan SET name = COALESCE(?, name), snap = COALESCE(?, snap), update_time = ? WHERE id = ?",
    )
    .bind(name)
    .bind(snap)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 删除计划：**同时**清掉它下面的时段条目和通道关联。
///
/// WVP `delete()` 做三件事：`removeRecordPlanByPlanId`（清通道关联）、
/// `cleanItems`、`delete`。只删主表会留下指向已删计划的 `record_plan_id`，
/// 通道列表里就会显示"已关联到不存在的计划"。
pub async fn delete_by_id(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    {
        let _ = sqlx::query("UPDATE gb_device_channel SET record_plan_id = NULL WHERE record_plan_id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        let _ = sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        let r = sqlx::query("DELETE FROM gb_record_plan WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(r.rows_affected());
    }
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("UPDATE gb_device_channel SET record_plan_id = NULL WHERE record_plan_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        let _ = sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        let r = sqlx::query("DELETE FROM gb_record_plan WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(r.rows_affected());
    }
    #[cfg(feature = "sqlite")]
    {
        let _ = sqlx::query("UPDATE gb_device_channel SET record_plan_id = NULL WHERE record_plan_id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        let _ = sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        let r = sqlx::query("DELETE FROM gb_record_plan WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(r.rows_affected());
    }
}

/// 按**通道主键**（`gb_device_channel.id`，即 WVP 前端里的 `gbId`）关联/取消关联。
///
/// `plan_id = None` 表示取消关联（WVP `link(channelIds, null)`）。
pub async fn link_channel(
    pool: &Pool,
    channel_id: i64,
    plan_id: Option<i64>,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_device_channel SET record_plan_id = ? WHERE id = ?")
        .bind(plan_id.map(|x| x as i32))
        .bind(channel_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_device_channel SET record_plan_id = $1 WHERE id = $2")
        .bind(plan_id.map(|x| x as i32))
        .bind(channel_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_device_channel SET record_plan_id = ? WHERE id = ?")
        .bind(plan_id.map(|x| x as i32))
        .bind(channel_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 按**国标通道编号**（`gb_device_id`）解析出通道主键。
///
/// WVP 的 `link` 只认主键，但更早版本的调用方传的是国标编号字符串，
/// 所以这里两种都能解析（先在 handler 里尝试按主键命中，再退回这里）。
pub async fn channel_id_by_gb_id(pool: &Pool, gb_device_id: &str) -> sqlx::Result<Option<i64>> {
    #[cfg(feature = "mysql")]
    let row = sqlx::query("SELECT id FROM gb_device_channel WHERE gb_device_id = ?")
        .bind(gb_device_id)
        .fetch_optional(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let row = sqlx::query("SELECT id FROM gb_device_channel WHERE gb_device_id = $1")
        .bind(gb_device_id)
        .fetch_optional(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let row = sqlx::query("SELECT id FROM gb_device_channel WHERE gb_device_id = ?")
        .bind(gb_device_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| {
        use sqlx::Row;
        r.try_get::<i32, _>("id")
            .map(|v| v as i64)
            .or_else(|_| r.try_get::<i64, _>("id"))
            .unwrap_or_default()
    }))
}

/// 通道主键是否真实存在（防止"关联了一个不存在的通道"被静默成功）。
pub async fn channel_exists(pool: &Pool, channel_id: i64) -> sqlx::Result<bool> {
    #[cfg(feature = "mysql")]
    let n = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device_channel WHERE id = ?")
        .bind(channel_id)
        .fetch_one(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let n = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device_channel WHERE id = $1")
        .bind(channel_id)
        .fetch_one(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let n = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device_channel WHERE id = ?")
        .bind(channel_id)
        .fetch_one(pool)
        .await?;
    Ok(n > 0)
}

/// 某设备（`gb_device.id`，WVP 的 `deviceDbIds`）下的所有通道主键。
pub async fn channel_ids_by_device_db_id(pool: &Pool, device_db_id: i64) -> sqlx::Result<Vec<i64>> {
    #[cfg(feature = "mysql")]
    let rows = sqlx::query("SELECT id FROM gb_device_channel WHERE data_device_id = ?")
        .bind(device_db_id as i32)
        .fetch_all(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let rows = sqlx::query("SELECT id FROM gb_device_channel WHERE data_device_id = $1")
        .bind(device_db_id as i32)
        .fetch_all(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let rows = sqlx::query("SELECT id FROM gb_device_channel WHERE data_device_id = ?")
        .bind(device_db_id as i32)
        .fetch_all(pool)
        .await?;
    use sqlx::Row;
    Ok(rows
        .iter()
        .map(|r| {
            r.try_get::<i32, _>("id")
                .map(|v| v as i64)
                .or_else(|_| r.try_get::<i64, _>("id"))
                .unwrap_or_default()
        })
        .collect())
}

/// 全部通道的主键（`allLink` 用）。
pub async fn all_channel_ids(pool: &Pool) -> sqlx::Result<Vec<i64>> {
    let rows = sqlx::query("SELECT id FROM gb_device_channel")
        .fetch_all(pool)
        .await?;
    use sqlx::Row;
    Ok(rows
        .iter()
        .map(|r| {
            r.try_get::<i32, _>("id")
                .map(|v| v as i64)
                .or_else(|_| r.try_get::<i64, _>("id"))
                .unwrap_or_default()
        })
        .collect())
}

/// 清掉某个计划下所有通道的关联（WVP `cleanAll`）。
pub async fn unlink_all_channels(pool: &Pool, plan_id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_device_channel SET record_plan_id = NULL WHERE record_plan_id = ?")
        .bind(plan_id as i32)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_device_channel SET record_plan_id = NULL WHERE record_plan_id = $1")
        .bind(plan_id as i32)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_device_channel SET record_plan_id = NULL WHERE record_plan_id = ?")
        .bind(plan_id as i32)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 覆盖写入计划的全部时段条目。
///
/// 与 WVP `update()` 一致：`start`/`stop`/`weekDay` 任一为空的条目**跳过**
/// （WVP 里是 `continue`）。否则会写入一条永远不可能命中的记录。
pub async fn replace_items(
    pool: &Pool,
    plan_id: i64,
    items: &[RecordPlanItemPayload],
    now: &str,
) -> sqlx::Result<u64> {
    let items: Vec<&RecordPlanItemPayload> = items.iter().filter(|i| i.is_complete()).collect();
    #[cfg(feature = "mysql")]
    {
        let _ = sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = ?")
            .bind(plan_id)
            .execute(pool)
            .await?;
        let mut affected = 0;
        for item in items {
            let r = sqlx::query(
                "INSERT INTO gb_record_plan_item (`start`, stop, week_day, plan_id, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(item.start)
            .bind(item.stop)
            .bind(item.week_day)
            .bind(plan_id)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
            affected += r.rows_affected();
        }
        Ok(affected)
    }
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = $1")
            .bind(plan_id)
            .execute(pool)
            .await?;
        let mut affected = 0;
        for item in items {
            let r = sqlx::query(
                "INSERT INTO gb_record_plan_item (\"start\", stop, week_day, plan_id, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(item.start)
            .bind(item.stop)
            .bind(item.week_day)
            .bind(plan_id)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
            affected += r.rows_affected();
        }
        Ok(affected)
    }
    #[cfg(feature = "sqlite")]
    {
        let _ = sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = ?")
            .bind(plan_id)
            .execute(pool)
            .await?;
        let mut affected = 0;
        for item in items {
            let r = sqlx::query(
                "INSERT INTO gb_record_plan_item (\"start\", stop, week_day, plan_id, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(item.start)
            .bind(item.stop)
            .bind(item.week_day)
            .bind(plan_id)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
            affected += r.rows_affected();
        }
        Ok(affected)
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod plan_db_tests {
    use super::*;
    use crate::test_support::sqlite_pool_with_schema;

    async fn seed_channel(pool: &Pool, gb_id: &str) -> i64 {
        let row = sqlx::query(
            "INSERT INTO gb_device_channel \
             (device_id, name, gb_device_id, status, data_type, data_device_id, channel_type, \
              create_time, update_time) \
             VALUES ('dev1', ?, ?, 'ON', 0, 1, 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .bind(format!("ch-{gb_id}"))
        .bind(gb_id)
        .execute(pool)
        .await
        .expect("insert channel");
        row.last_insert_rowid()
    }

    fn payload(start: i32, stop: i32, day: i32) -> RecordPlanItemPayload {
        RecordPlanItemPayload {
            start: Some(start),
            stop: Some(stop),
            week_day: Some(day),
            plan_id: None,
        }
    }

    /// `start > stop` 的时段在 WVP 里能存下来但**永远不会触发**
    /// （SQL 是 `start <= index and stop >= index`）。保存成功却没有任何效果
    /// 正是本项目要消灭的静默失败，所以直接拒绝。
    #[test]
    fn test_item_validation_rejects_impossible_windows() {
        assert!(payload(600, 660, 1).validate().is_none());
        let err = payload(660, 600, 1).validate().expect("start>stop 必须报错");
        assert!(err.contains("大于"), "{err}");
        assert!(payload(0, 1440, 7).validate().is_none(), "stop=1440 合法");
        assert!(payload(-1, 60, 1).validate().is_some(), "负 start 非法");
        assert!(payload(0, 1441, 1).validate().is_some(), "超过 1440 非法");
        assert!(payload(0, 60, 0).validate().is_some(), "weekDay=0 非法（ISO 1..7）");
        assert!(payload(0, 60, 8).validate().is_some(), "weekDay=8 非法");
    }

    #[test]
    fn test_item_missing_field_is_not_complete() {
        let mut p = payload(0, 60, 1);
        assert!(p.is_complete());
        p.start = None;
        assert!(!p.is_complete());
        p.start = Some(0);
        p.week_day = None;
        assert!(!p.is_complete());
    }

    /// 回归保护：写入时应跳过字段不全的条目（WVP `update()` 的 `continue` 行为），
    /// 否则库里会留下永远命中的不了的记录。
    #[tokio::test]
    async fn test_replace_items_skips_incomplete() {
        let pool = sqlite_pool_with_schema().await;
        let now = "2026-01-01 00:00:00";
        let plan_id = add_with_id(&pool, "p1", false, now).await.unwrap();
        let items = vec![
            payload(0, 60, 1),
            RecordPlanItemPayload {
                start: None,
                stop: Some(120),
                week_day: Some(2),
                plan_id: None,
            },
            payload(120, 180, 3),
        ];
        let n = replace_items(&pool, plan_id, &items, now).await.unwrap();
        assert_eq!(n, 2, "只应写入 2 条完整条目");
        let stored = list_items(&pool, plan_id).await.unwrap();
        assert_eq!(stored.len(), 2);
    }

    /// 删除计划必须同时清掉：时段条目 + 通道上的 `record_plan_id`。
    /// 否则通道会一直指向一个不存在的计划（页面显示"已关联"但永远不录像）。
    #[tokio::test]
    async fn test_delete_plan_clears_items_and_channel_links() {
        let pool = sqlite_pool_with_schema().await;
        let now = "2026-01-01 00:00:00";
        let plan_id = add_with_id(&pool, "p1", false, now).await.unwrap();
        replace_items(&pool, plan_id, &[payload(0, 60, 1)], now)
            .await
            .unwrap();
        let ch = seed_channel(&pool, "34020000001310000001").await;
        link_channel(&pool, ch, Some(plan_id)).await.unwrap();
        assert_eq!(count_linked_channels(&pool, plan_id).await.unwrap(), 1);

        delete_by_id(&pool, plan_id as i32).await.unwrap();
        assert!(list_items(&pool, plan_id).await.unwrap().is_empty());
        assert_eq!(count_linked_channels(&pool, plan_id).await.unwrap(), 0);
        assert!(get_by_id(&pool, plan_id as i32).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_link_unlink_and_all_channels() {
        let pool = sqlite_pool_with_schema().await;
        let now = "2026-01-01 00:00:00";
        let plan_id = add_with_id(&pool, "p1", false, now).await.unwrap();
        let c1 = seed_channel(&pool, "34020000001310000001").await;
        let c2 = seed_channel(&pool, "34020000001310000002").await;

        assert_eq!(link_channel(&pool, c1, Some(plan_id)).await.unwrap(), 1);
        assert_eq!(count_linked_channels(&pool, plan_id).await.unwrap(), 1);
        assert!(channel_exists(&pool, c1).await.unwrap());
        assert!(!channel_exists(&pool, 9999).await.unwrap());

        assert_eq!(channel_id_by_gb_id(&pool, "34020000001310000002").await.unwrap(), Some(c2));
        assert_eq!(channel_id_by_gb_id(&pool, "nope").await.unwrap(), None);

        let all = all_channel_ids(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        let by_dev = channel_ids_by_device_db_id(&pool, 1).await.unwrap();
        assert_eq!(by_dev.len(), 2);

        assert_eq!(unlink_all_channels(&pool, plan_id).await.unwrap(), 1);
        assert_eq!(count_linked_channels(&pool, plan_id).await.unwrap(), 0);
        // 取消关联（plan_id = None）
        assert_eq!(link_channel(&pool, c2, None).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_list_paged_search_filters_by_name() {
        let pool = sqlite_pool_with_schema().await;
        let now = "2026-01-01 00:00:00";
        add_with_id(&pool, "机房全天", false, now).await.unwrap();
        add_with_id(&pool, "大厅夜间", false, now).await.unwrap();

        let all = list_paged(&pool, 1, 10, None).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(count_all(&pool, None).await.unwrap(), 2);

        let hit = list_paged(&pool, 1, 10, Some("机房")).await.unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].name, "机房全天");
        assert_eq!(count_all(&pool, Some("机房")).await.unwrap(), 1);
        assert!(list_paged(&pool, 1, 10, Some("不存在")).await.unwrap().is_empty());
    }
}
