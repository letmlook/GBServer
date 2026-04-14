//! 移动位置上报数据表 wvp_device_mobile_position

use serde::{Deserialize, Serialize};

use super::Pool;

/// 移动位置信息结构体
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MobilePosition {
    pub id: i64,
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
    pub create_time: Option<String>,
}

/// 插入移动位置参数
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
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_device_mobile_position \
             (device_id, channel_id, device_name, time, longitude, latitude, altitude, speed, direction, report_source, create_time) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
        )
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
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_device_mobile_position \
             (device_id, channel_id, device_name, time, longitude, latitude, altitude, speed, direction, report_source, create_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
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
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
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
    let offset = (page - 1) * count;

    #[cfg(feature = "postgres")]
    {
        let query = sqlx::query_as::<_, MobilePosition>(
            "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
             altitude, speed, direction, report_source, create_time \
             FROM wvp_device_mobile_position \
             WHERE device_id = $1 \
               AND ($2::text IS NULL OR channel_id = $2) \
               AND ($3::text IS NULL OR time >= $3) \
               AND ($4::text IS NULL OR time <= $4) \
             ORDER BY time DESC \
             LIMIT $5 OFFSET $6"
        )
        .bind(device_id)
        .bind(channel_id)
        .bind(start_time)
        .bind(end_time)
        .bind(count)
        .bind(offset);

        query.fetch_all(pool).await
    }

    #[cfg(feature = "mysql")]
    {
        let query = sqlx::query_as::<_, MobilePosition>(
            "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
             altitude, speed, direction, report_source, create_time \
             FROM wvp_device_mobile_position \
             WHERE device_id = ? \
               AND (? IS NULL OR channel_id = ?) \
               AND (? IS NULL OR time >= ?) \
               AND (? IS NULL OR time <= ?) \
             ORDER BY time DESC \
             LIMIT ? OFFSET ?"
        )
        .bind(device_id)
        .bind(channel_id)
        .bind(channel_id)
        .bind(start_time)
        .bind(start_time)
        .bind(end_time)
        .bind(end_time)
        .bind(count)
        .bind(offset);

        query.fetch_all(pool).await
    }
}

/// 统计移动位置数量
pub async fn count(
    pool: &Pool,
    device_id: &str,
    channel_id: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> sqlx::Result<i64> {
    #[cfg(feature = "postgres")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wvp_device_mobile_position \
             WHERE device_id = $1 \
               AND ($2::text IS NULL OR channel_id = $2) \
               AND ($3::text IS NULL OR time >= $3) \
               AND ($4::text IS NULL OR time <= $4)"
        )
        .bind(device_id)
        .bind(channel_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    #[cfg(feature = "mysql")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wvp_device_mobile_position \
             WHERE device_id = ? \
               AND (? IS NULL OR channel_id = ?) \
               AND (? IS NULL OR time >= ?) \
               AND (? IS NULL OR time <= ?)"
        )
        .bind(device_id)
        .bind(channel_id)
        .bind(channel_id)
        .bind(start_time)
        .bind(start_time)
        .bind(end_time)
        .bind(end_time)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }
}

/// 删除指定时间之前的位置记录
pub async fn delete_before_time(pool: &Pool, before_time: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("DELETE FROM wvp_device_mobile_position WHERE time < $1")
            .bind(before_time)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM wvp_device_mobile_position WHERE time < ?")
            .bind(before_time)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// 获取设备最新位置
pub async fn get_latest_position(pool: &Pool, device_id: &str, channel_id: Option<&str>) -> sqlx::Result<Option<MobilePosition>> {
    #[cfg(feature = "postgres")]
    {
        let query = sqlx::query_as::<_, MobilePosition>(
            "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
             altitude, speed, direction, report_source, create_time \
             FROM wvp_device_mobile_position \
             WHERE device_id = $1 AND ($2::text IS NULL OR channel_id = $2) \
             ORDER BY time DESC LIMIT 1"
        )
        .bind(device_id)
        .bind(channel_id);
        query.fetch_optional(pool).await
    }

    #[cfg(feature = "mysql")]
    {
        let query = sqlx::query_as::<_, MobilePosition>(
            "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, \
             altitude, speed, direction, report_source, create_time \
             FROM wvp_device_mobile_position \
             WHERE device_id = ? AND (? IS NULL OR channel_id = ?) \
             ORDER BY time DESC LIMIT 1"
        )
        .bind(device_id)
        .bind(channel_id)
        .bind(channel_id);
        query.fetch_optional(pool).await
    }
}

/// 删除设备的所有位置记录
pub async fn delete_by_device(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("DELETE FROM wvp_device_mobile_position WHERE device_id = $1")
            .bind(device_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM wvp_device_mobile_position WHERE device_id = ?")
            .bind(device_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// 根据ID查询单条记录
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<MobilePosition>> {
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MobilePosition>(
        "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, altitude, speed, direction, report_source, create_time FROM wvp_device_mobile_position WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MobilePosition>(
        "SELECT id, device_id, channel_id, device_name, time, longitude, latitude, altitude, speed, direction, report_source, create_time FROM wvp_device_mobile_position WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}
