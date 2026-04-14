use super::Pool;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 报警信息结构体
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Alarm {
    pub id: i64,
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
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_device_alarm (device_id, channel_id, alarm_priority, alarm_method, alarm_time, alarm_description, longitude, latitude, alarm_type, create_time) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
        )
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
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_device_alarm (device_id, channel_id, alarm_priority, alarm_method, alarm_time, alarm_description, longitude, latitude, alarm_type, create_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
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
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
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
    let offset = (page - 1) * count;

    #[cfg(feature = "postgres")]
    {
        let query = sqlx::query_as::<_, Alarm>(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM wvp_device_alarm \
             WHERE ($1::text IS NULL OR device_id LIKE '%' || $1 || '%') \
               AND ($2::text IS NULL OR alarm_type = $2) \
               AND ($3::text IS NULL OR alarm_priority = $3) \
               AND ($4::text IS NULL OR alarm_time >= $4) \
               AND ($5::text IS NULL OR alarm_time <= $5) \
             ORDER BY create_time DESC \
             LIMIT $6 OFFSET $7"
        )
        .bind(device_id)
        .bind(alarm_type)
        .bind(alarm_priority)
        .bind(start_time)
        .bind(end_time)
        .bind(count)
        .bind(offset);

        query.fetch_all(pool).await
    }

    #[cfg(feature = "mysql")]
    {
        let query = sqlx::query_as::<_, Alarm>(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM wvp_device_alarm \
             WHERE (? IS NULL OR device_id LIKE CONCAT('%', ?, '%')) \
               AND (? IS NULL OR alarm_type = ?) \
               AND (? IS NULL OR alarm_priority = ?) \
               AND (? IS NULL OR alarm_time >= ?) \
               AND (? IS NULL OR alarm_time <= ?) \
             ORDER BY create_time DESC \
             LIMIT ? OFFSET ?"
        )
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

        query.fetch_all(pool).await
    }
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
    #[cfg(feature = "postgres")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wvp_device_alarm \
             WHERE ($1::text IS NULL OR device_id LIKE '%' || $1 || '%') \
               AND ($2::text IS NULL OR alarm_type = $2) \
               AND ($3::text IS NULL OR alarm_priority = $3) \
               AND ($4::text IS NULL OR alarm_time >= $4) \
               AND ($5::text IS NULL OR alarm_time <= $5)"
        )
        .bind(device_id)
        .bind(alarm_type)
        .bind(alarm_priority)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    #[cfg(feature = "mysql")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wvp_device_alarm \
             WHERE (? IS NULL OR device_id LIKE CONCAT('%', ?, '%')) \
               AND (? IS NULL OR alarm_type = ?) \
               AND (? IS NULL OR alarm_priority = ?) \
               AND (? IS NULL OR alarm_time >= ?) \
               AND (? IS NULL OR alarm_time <= ?)"
        )
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
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }
}

/// 根据ID查询报警详情
pub async fn get_alarm_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<Alarm>> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query_as::<_, Alarm>(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM wvp_device_alarm WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query_as::<_, Alarm>(
            "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_time, \
             alarm_description, longitude, latitude, alarm_type, create_time \
             FROM wvp_device_alarm WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }
}

/// 删除报警记录
pub async fn delete_alarm(pool: &Pool, id: i64) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("DELETE FROM wvp_device_alarm WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM wvp_device_alarm WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// 批量删除报警记录
pub async fn batch_delete_alarms(pool: &Pool, ids: &[i64]) -> sqlx::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }

    #[cfg(feature = "postgres")]
    {
        let placeholders: Vec<String> = ids.iter().map(|_| "$1".to_string()).collect();
        let sql = format!(
            "DELETE FROM wvp_device_alarm WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM wvp_device_alarm WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }
}
