use super::Pool;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 报警信息结构体
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Alarm {
    pub id: i64,
    #[serde(alias = "deviceId")]
    pub device_id: String,
    #[serde(alias = "channelId")]
    pub channel_id: String,
    #[serde(alias = "alarmPriority")]
    pub alarm_priority: Option<String>,
    #[serde(alias = "alarmMethod")]
    pub alarm_method: Option<String>,
    #[serde(alias = "alarmTime")]
    pub alarm_time: Option<String>,
    #[serde(alias = "alarmDescription")]
    pub alarm_description: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    #[serde(alias = "alarmType")]
    pub alarm_type: Option<String>,
    #[serde(alias = "createTime")]
    pub create_time: String,
}

/// 插入报警信息结构体
pub struct AlarmInsert {
    pub device_id: String,
    pub channel_id: String,
    pub alarm_priority: Option<String>,
    pub alarm_method: Option<String>,
    pub alarm_time: Option<String>,
    pub alarm_description: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub alarm_type: Option<String>,
    pub create_time: String,
}

/// 插入报警记录
pub async fn insert_alarm(pool: &Pool, alarm: &AlarmInsert) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("INSERT INTO gb_device_alarm (device_id, channel_id, alarm_priority, alarm_method, alarm_time, alarm_description, longitude, latitude, alarm_type, create_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)").into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(&alarm.device_id)
            .bind(&alarm.channel_id)
            .bind(&alarm.alarm_priority)
            .bind(&alarm.alarm_method)
            .bind(&alarm.alarm_time)
            .bind(&alarm.alarm_description)
            .bind(alarm.longitude)
            .bind(alarm.latitude)
            .bind(&alarm.alarm_type)
            .bind(&alarm.create_time)
            .execute(p)
    })?;

    Ok(result.rows_affected())
}

/// 分页查询报警列表
pub async fn list_alarms_paged(
    pool: &Pool,
    device_id: Option<&str>,
    alarm_type: Option<&str>,
    alarm_priority: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
    page: i64,
    count: i64,
) -> sqlx::Result<Vec<Alarm>> {
    let offset = crate::pagination::offset_i64(page, count);

    let sql = match crate::db::dialect::current() {
        crate::db::Dialect::MySql => crate::db::dialect::current_sql(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM gb_device_alarm \
             WHERE (? IS NULL OR device_id LIKE CONCAT('%', ?, '%')) \
               AND (? IS NULL OR alarm_type = ?) \
               AND (? IS NULL OR alarm_priority = ?) \
               AND (? IS NULL OR alarm_time >= ?) \
               AND (? IS NULL OR alarm_time <= ?) \
             ORDER BY create_time DESC \
             LIMIT ? OFFSET ?",
        )
        .into_owned(),
        crate::db::Dialect::Sqlite => crate::db::dialect::current_sql(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM gb_device_alarm \
             WHERE (? IS NULL OR device_id LIKE '%' || ? || '%') \
               AND (? IS NULL OR alarm_type = ?) \
               AND (? IS NULL OR alarm_priority = ?) \
               AND (? IS NULL OR alarm_time >= ?) \
               AND (? IS NULL OR alarm_time <= ?) \
             ORDER BY create_time DESC \
             LIMIT ? OFFSET ?",
        )
        .into_owned(),
        _ => crate::db::dialect::current_sql(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM gb_device_alarm \
             WHERE ($1::text IS NULL OR device_id LIKE '%' || $1 || '%') \
               AND ($2::text IS NULL OR alarm_type = $2) \
               AND ($3::text IS NULL OR alarm_priority = $3) \
               AND ($4::text IS NULL OR alarm_time >= $4) \
               AND ($5::text IS NULL OR alarm_time <= $5) \
             ORDER BY create_time DESC \
             LIMIT $6 OFFSET $7",
        )
        .into_owned(),
    };
    return crate::dispatch!(pool, |p| {
        let query = sqlx::query_as::<_, Alarm>(&sql)
            .bind(device_id)
            .bind(device_id)
            .bind(alarm_type)
            .bind(alarm_type)
            .bind(alarm_priority)
            .bind(alarm_priority)
            .bind(start_time)
            .bind(start_time)
            .bind(end_time)
            .bind(end_time)
            .bind(count)
            .bind(offset);

        query.fetch_all(p)
    });
}

/// 统计报警数量
pub async fn count_alarms(
    pool: &Pool,
    device_id: Option<&str>,
    alarm_type: Option<&str>,
    alarm_priority: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> sqlx::Result<i64> {
    let sql = match crate::db::dialect::current() {
        crate::db::Dialect::MySql => crate::db::dialect::current_sql(
            "SELECT COUNT(*) FROM gb_device_alarm \
             WHERE (? IS NULL OR device_id LIKE CONCAT('%', ?, '%')) \
               AND (? IS NULL OR alarm_type = ?) \
               AND (? IS NULL OR alarm_priority = ?) \
               AND (? IS NULL OR alarm_time >= ?) \
               AND (? IS NULL OR alarm_time <= ?)",
        )
        .into_owned(),
        crate::db::Dialect::Sqlite => crate::db::dialect::current_sql(
            "SELECT COUNT(*) FROM gb_device_alarm \
             WHERE (?1 IS NULL OR device_id LIKE '%' || ?1 || '%') \
               AND (?2 IS NULL OR alarm_type = ?2) \
               AND (?3 IS NULL OR alarm_priority = ?3) \
               AND (?4 IS NULL OR alarm_time >= ?4) \
               AND (?5 IS NULL OR alarm_time <= ?5)",
        )
        .into_owned(),
        _ => crate::db::dialect::current_sql(
            "SELECT COUNT(*) FROM gb_device_alarm \
             WHERE ($1::text IS NULL OR device_id LIKE '%' || $1 || '%') \
               AND ($2::text IS NULL OR alarm_type = $2) \
               AND ($3::text IS NULL OR alarm_priority = $3) \
               AND ($4::text IS NULL OR alarm_time >= $4) \
               AND ($5::text IS NULL OR alarm_time <= $5)",
        )
        .into_owned(),
    };

    let result: (i64,) = crate::dispatch!(pool, |p| {
        sqlx::query_as(&sql)
            .bind(device_id)
            .bind(device_id)
            .bind(alarm_type)
            .bind(alarm_type)
            .bind(alarm_priority)
            .bind(alarm_priority)
            .bind(start_time)
            .bind(start_time)
            .bind(end_time)
            .bind(end_time)
            .fetch_one(p)
    })?;

    Ok(result.0)
}

/// 根据ID查询报警详情
pub async fn get_alarm_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<Alarm>> {
    let sql = crate::db::dialect::current_sql(
        "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM gb_device_alarm WHERE id = ?",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Alarm>(&sql).bind(id).fetch_optional(p)
    });
}

/// 删除报警记录
/// Delete ALL alarms (used by /api/alarm/clear). Returns count deleted.
pub async fn delete_all(pool: &Pool) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("DELETE FROM gb_device_alarm").into_owned();
    // `dispatch_exec!` 自己会 await 并把方言相关的 `QueryResult` 统一映射成
    // `ExecResult` —— 宏体里**只能**留未 await 的 `execute(p)`。
    crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).execute(p) }).map(|r| r.rows_affected())
}

pub async fn delete_alarm(pool: &Pool, id: i64) -> sqlx::Result<bool> {
    let sql =
        crate::db::dialect::current_sql("DELETE FROM gb_device_alarm WHERE id = ?").into_owned();

    let result = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;

    Ok(result.rows_affected() > 0)
}

/// 批量删除报警记录
pub async fn batch_delete_alarms(pool: &Pool, ids: &[i64]) -> sqlx::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }

    // 占位符数量随 ids 变化 → 拼出 `IN (?, ?, ...)`。
    // **构造**（占位符串、SQL 文本）不依赖执行器，可以提到 dispatch 外面；
    // `sqlx::query(&sql)` 的类型随方言变化，必须留在 dispatch 里面。
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!(
        "DELETE FROM gb_device_alarm WHERE id IN ({})",
        placeholders.join(", ")
    );
    let result = crate::dispatch_exec!(pool, |p| {
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.execute(p)
    })?;
    Ok(result.rows_affected())
}

/// Migration: ensure gb_device_alarm has handle columns.
/// These columns are missing from the original schema; without them,
/// `alarm_handle` cannot persist its updates (a regression vs. the historical
/// "gb_device_alarm表没有handled字段" comment in the handler).
/// Adding columns to an existing SQLite table is done via ALTER TABLE,
/// which fails with "duplicate column" if the column already exists — we
/// swallow that error so the migration is idempotent.
pub async fn ensure_columns(pool: &Pool) -> sqlx::Result<()> {
    // ADD COLUMN is feature-uniform: SQLite/MySQL/Postgres all support the
    // same SQL syntax for adding a single nullable column.
    for stmt in [
        "ALTER TABLE gb_device_alarm ADD COLUMN handled INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE gb_device_alarm ADD COLUMN handle_user VARCHAR(50)",
        "ALTER TABLE gb_device_alarm ADD COLUMN handle_time VARCHAR(50)",
        "ALTER TABLE gb_device_alarm ADD COLUMN handle_result VARCHAR(255)",
    ] {
        // Swallow "duplicate column" errors so the migration is idempotent.
        // **不能加 `?`** —— 加了就把「忽略重复列」变成了「遇到重复列就整体失败」。
        let _ = crate::dispatch_exec!(pool, |p| { sqlx::query(stmt).execute(p) });
    }
    Ok(())
}

/// Update the handle state of a single alarm. Returns the rows affected.
pub async fn set_handled(
    pool: &Pool,
    id: i64,
    handle_user: Option<&str>,
    handle_time: &str,
    handle_result: Option<&str>,
) -> sqlx::Result<u64> {
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&crate::dyn_where::dialect_sql(
            "UPDATE gb_device_alarm SET handled = 1, handle_user = ?, handle_time = ?, \
         handle_result = COALESCE(?, handle_result) WHERE id = ?",
        ))
        .bind(handle_user)
        .bind(handle_time)
        .bind(handle_result)
        .bind(id)
        .execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 按条件清空。`where_sql` 由 handler 用
/// `DynWhere` 生成并保证是参数化的；空条件表示清空整表。
pub async fn delete_where(pool: &Pool, where_sql: &str, binds: &[String]) -> sqlx::Result<u64> {
    let sql = format!("DELETE FROM gb_device_alarm{where_sql}");
    // `execute()` 的返回类型是**方言相关**的（`PgQueryResult` / `MySqlQueryResult`
    // / `SqliteQueryResult`），不能直接放进 `dispatch!` —— 用 `dispatch_exec!`
    // 统一映射成 `ExecResult`。
    let r = crate::dispatch_exec!(pool, |p| {
        let mut q = sqlx::query(&sql);
        for b in binds {
            q = q.bind(b.as_str());
        }
        q.execute(p)
    })?;
    Ok(r.rows_affected())
}
