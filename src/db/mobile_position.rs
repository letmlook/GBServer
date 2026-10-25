//! 移动位置上报数据表 gb_device_mobile_position

use serde::{Deserialize, Serialize};

use super::Pool;

/// 移动位置信息结构体
///
/// 序列化成 **camelCase**：与 `/api/position/*` 的对外契约一致
/// （`deviceId` / `channelId` / `deviceName` /
/// `reportSource` / `createTime`）。此前是 snake_case，对外会暴露 `channel_id`
/// 这类键，与契约不一致。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MobilePosition {
    pub id: i64,
    #[serde(alias = "deviceId")]
    pub device_id: String,
    #[serde(alias = "channelId")]
    pub channel_id: String,
    #[serde(alias = "deviceName")]
    pub device_name: Option<String>,
    pub time: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub direction: Option<f64>,
    #[serde(alias = "reportSource")]
    pub report_source: Option<String>,
    #[serde(alias = "createTime")]
    pub create_time: Option<String>,
}

/// 插入移动位置参数
#[derive(Debug, Clone, Default)]
pub struct MobilePositionInsert {
    pub device_id: String,
    pub channel_id: String,
    pub device_name: Option<String>,
    pub time: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub direction: Option<f64>,
    pub report_source: Option<String>,
    pub create_time: String,
}

/// 插入移动位置记录
pub async fn insert(pool: &Pool, pos: &MobilePositionInsert) -> sqlx::Result<u64> {
    let sql = pool.sql("INSERT INTO gb_device_mobile_position \
             (device_id, channel_id, device_name, time, longitude, latitude, altitude, speed, direction, report_source, create_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)").into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(&pos.device_id)
            .bind(&pos.channel_id)
            .bind(&pos.device_name)
            .bind(&pos.time)
            .bind(pos.longitude)
            .bind(pos.latitude)
            .bind(pos.altitude)
            .bind(pos.speed)
            .bind(pos.direction)
            .bind(&pos.report_source)
            .bind(&pos.create_time)
            .execute(p)
    })?;

    Ok(result.rows_affected())
}

/// 分页查询移动位置列表
pub async fn list_paged(
    pool: &Pool,
    device_id: &str,
    channel_id: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
    page: i64,
    count: i64,
) -> sqlx::Result<Vec<MobilePosition>> {
    let offset = crate::pagination::offset_i64(page, count);
    // 同 `count`：三个方言的占位符数量不同，各分支各带自己的 SQL 与 bind 链。
    crate::dispatch_dialect!(
        pool,
        |p| {
            let sql = pool.sql(
                "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
                 altitude, speed, direction, report_source, create_time \
                 FROM gb_device_mobile_position \
                 WHERE device_id = $1 \
                   AND ($2::text IS NULL OR channel_id = $2) \
                   AND ($3::text IS NULL OR time >= $3) \
                   AND ($4::text IS NULL OR time <= $4) \
                 ORDER BY time DESC \
                 LIMIT $5 OFFSET $6",
            );
            sqlx::query_as::<_, MobilePosition>(&sql)
                .bind(device_id)
                .bind(channel_id)
                .bind(start_time)
                .bind(end_time)
                .bind(count)
                .bind(offset)
                .fetch_all(p)
                .await
        },
        |p| {
            let sql = pool.sql(
                "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
                 altitude, speed, direction, report_source, create_time \
                 FROM gb_device_mobile_position \
                 WHERE device_id = ? \
                   AND (? IS NULL OR channel_id = ?) \
                   AND (? IS NULL OR time >= ?) \
                   AND (? IS NULL OR time <= ?) \
                 ORDER BY time DESC \
                 LIMIT ? OFFSET ?",
            );
            sqlx::query_as::<_, MobilePosition>(&sql)
                .bind(device_id)
                .bind(channel_id)
                .bind(channel_id)
                .bind(start_time)
                .bind(start_time)
                .bind(end_time)
                .bind(end_time)
                .bind(count)
                .bind(offset)
                .fetch_all(p)
                .await
        },
        |p| {
            let sql = pool.sql(
                "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
                 altitude, speed, direction, report_source, create_time \
                 FROM gb_device_mobile_position \
                 WHERE device_id = ? \
                   AND (? IS NULL OR channel_id = ?) \
                   AND (? IS NULL OR time >= ?) \
                   AND (? IS NULL OR time <= ?) \
                 ORDER BY time DESC \
                 LIMIT ? OFFSET ?",
            );
            sqlx::query_as::<_, MobilePosition>(&sql)
                .bind(device_id)
                .bind(channel_id)
                .bind(channel_id)
                .bind(start_time)
                .bind(start_time)
                .bind(end_time)
                .bind(end_time)
                .bind(count)
                .bind(offset)
                .fetch_all(p)
                .await
        },
    )
}

/// 统计移动位置数量
pub async fn count(
    pool: &Pool,
    device_id: &str,
    channel_id: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> sqlx::Result<i64> {
    // 三个方言的**占位符数量不同**（决定 bind 链长度）：
    // * PostgreSQL / SQLite：`$1`（或 `?1`）**复用**参数 —— 5 个 bind；
    // * MySQL：每个条件两个 `?` —— 7 个 bind。
    let result: (i64,) = crate::dispatch_dialect!(
        pool,
        |p| {
            let sql = pool.sql(
                "SELECT COUNT(*) FROM gb_device_mobile_position \
                 WHERE device_id = $1 \
                   AND ($2::text IS NULL OR channel_id = $2) \
                   AND ($3::text IS NULL OR time >= $3) \
                   AND ($4::text IS NULL OR time <= $4)",
            );
            sqlx::query_as::<_, (i64,)>(&sql)
                .bind(device_id)
                .bind(channel_id)
                .bind(start_time)
                .bind(end_time)
                .fetch_one(p)
                .await
        },
        |p| {
            let sql = pool.sql(
                "SELECT COUNT(*) FROM gb_device_mobile_position \
                 WHERE device_id = ? \
                   AND (? IS NULL OR channel_id = ?) \
                   AND (? IS NULL OR time >= ?) \
                   AND (? IS NULL OR time <= ?)",
            );
            sqlx::query_as::<_, (i64,)>(&sql)
                .bind(device_id)
                .bind(channel_id)
                .bind(channel_id)
                .bind(start_time)
                .bind(start_time)
                .bind(end_time)
                .bind(end_time)
                .fetch_one(p)
                .await
        },
        |p| {
            let sql = pool.sql(
                "SELECT COUNT(*) FROM gb_device_mobile_position \
                 WHERE device_id = ?1 \
                   AND (?2 IS NULL OR channel_id = ?2) \
                   AND (?3 IS NULL OR time >= ?3) \
                   AND (?4 IS NULL OR time <= ?4)",
            );
            sqlx::query_as::<_, (i64,)>(&sql)
                .bind(device_id)
                .bind(channel_id)
                .bind(start_time)
                .bind(end_time)
                .fetch_one(p)
                .await
        },
    )?;

    Ok(result.0)
}

/// 删除指定时间之前的位置记录
pub async fn delete_before_time(pool: &Pool, before_time: &str) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_device_mobile_position WHERE time < ?")
        .into_owned();

    let result =
        crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(before_time).execute(p) })?;

    Ok(result.rows_affected())
}

/// 获取设备最新位置
pub async fn get_latest_position(
    pool: &Pool,
    device_id: &str,
    channel_id: Option<&str>,
) -> sqlx::Result<Option<MobilePosition>> {
    // 三个方言的占位符**数量不同**（pg 的 `$2` 复用同一个参数，mysql/sqlite
    // 的 `?` 要两个），而且 pg 那支需要 `$2::text` —— 否则 postgres 推不出
    // `$2` 的类型（`could not determine data type of parameter $2`）。
    // 所以三支各带自己的 SQL 与 bind 链。
    crate::dispatch_dialect!(
        pool,
        |p| {
            sqlx::query_as::<_, MobilePosition>(
                "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
                 altitude, speed, direction, report_source, create_time \
                 FROM gb_device_mobile_position \
                 WHERE device_id = $1 AND ($2::text IS NULL OR channel_id = $2) \
                 ORDER BY time DESC LIMIT 1",
            )
            .bind(device_id)
            .bind(channel_id)
            .fetch_optional(p)
            .await
        },
        |p| {
            sqlx::query_as::<_, MobilePosition>(&pool.sql(
                "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
                     altitude, speed, direction, report_source, create_time \
                     FROM gb_device_mobile_position \
                     WHERE device_id = ? AND (? IS NULL OR channel_id = ?) \
                     ORDER BY time DESC LIMIT 1",
            ))
            .bind(device_id)
            .bind(channel_id)
            .bind(channel_id)
            .fetch_optional(p)
            .await
        },
        |p| {
            sqlx::query_as::<_, MobilePosition>(&pool.sql(
                "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
                     altitude, speed, direction, report_source, create_time \
                     FROM gb_device_mobile_position \
                     WHERE device_id = ? AND (? IS NULL OR channel_id = ?) \
                     ORDER BY time DESC LIMIT 1",
            ))
            .bind(device_id)
            .bind(channel_id)
            .bind(channel_id)
            .fetch_optional(p)
            .await
        },
    )
}

/// 删除设备的所有位置记录
pub async fn delete_by_device(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_device_mobile_position WHERE device_id = ?")
        .into_owned();

    let result = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(device_id).execute(p) })?;

    Ok(result.rows_affected())
}

/// 根据ID查询单条记录
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<MobilePosition>> {
    let sql = pool.sql("SELECT id, device_id, channel_id, device_name, time, longitude, latitude, altitude, speed, direction, report_source, create_time FROM gb_device_mobile_position WHERE id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, MobilePosition>(&sql)
            .bind(id)
            .fetch_optional(p)
    });
}

/// 把最新上报的位置**回写到通道行**，供地图与通道列表使用。
///
/// ## 为什么需要这一步
///
/// 设备上报的移动位置落在 `gb_device_mobile_position`（以及历史表
/// `gb_position_history`），而**地图与通道列表读的是
/// `gb_device_channel.longitude/latitude`** —— 两张表互不相通。结果是
/// 「设备明明在持续上报位置，地图上却什么都看不到」。
///
/// ## 覆盖范围
///
/// 设备级上报时 `gb_device_mobile_position.channel_id` 等于 device_id，
/// 因此这里按 `gb_device_channel.device_id = <设备国标编号>` 更新**该设备的所有通道**。
///
/// ## 守卫
///
/// 经纬度同时为 0（设备未带该字段 / 解析失败）时**不写库**：否则会把已有的
/// 正确坐标覆盖成 (0,0)，地图上设备直接消失（前端会把 0,0 视为无效点过滤掉）。
pub async fn sync_channel_coords(
    pool: &Pool,
    device_id: &str,
    longitude: f64,
    latitude: f64,
) -> sqlx::Result<u64> {
    if longitude == 0.0 && latitude == 0.0 {
        return Ok(0);
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&pool.sql(
            "UPDATE gb_device_channel SET longitude = ?, latitude = ?, update_time = ? \
         WHERE device_id = ?",
        ))
        .bind(longitude)
        .bind(latitude)
        .bind(&now)
        .bind(device_id)
        .execute(p)
    })?;
    Ok(result.rows_affected())
}
