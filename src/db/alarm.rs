use super::Pool;

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
