//! WVP position history table: wvp_position_history
use serde::Serialize;
use sqlx::FromRow;

use crate::db::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PositionHistory {
    pub id: i64,
    pub device_id: String,
    pub timestamp: String,
    pub longitude: f64,
    pub latitude: f64,
    pub altitude: f64,
    pub speed: f64,
    pub direction: f64,
}

pub async fn ensure_table(pool: &Pool) -> sqlx::Result<()> {
    #[cfg(feature = "mysql")]
    {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wvp_position_history (\n\n  id BIGINT AUTO_INCREMENT PRIMARY KEY,\n  device_id VARCHAR(50) NOT NULL,\n  timestamp VARCHAR(50) NOT NULL,\n  longitude DOUBLE,\n  latitude DOUBLE,\n  altitude DOUBLE,\n  speed DOUBLE,\n  direction DOUBLE\n)"
        ).execute(pool).await?;
    }
    #[cfg(feature = "postgres")]
    {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wvp_position_history (\n\n  id BIGSERIAL PRIMARY KEY,\n  device_id VARCHAR(50) NOT NULL,\n  timestamp VARCHAR(50) NOT NULL,\n  longitude DOUBLE PRECISION,\n  latitude DOUBLE PRECISION,\n  altitude DOUBLE PRECISION,\n  speed DOUBLE PRECISION,\n  direction DOUBLE PRECISION\n)"
        ).execute(pool).await?;
    }
    Ok(())
}

pub async fn insert_position(
    pool: &Pool,
    device_id: &str,
    timestamp: &str,
    longitude: f64,
    latitude: f64,
    altitude: f64,
    speed: f64,
    direction: f64,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(
            "INSERT INTO wvp_position_history (device_id, timestamp, longitude, latitude, altitude, speed, direction) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(device_id)
        .bind(timestamp)
        .bind(longitude)
        .bind(latitude)
        .bind(altitude)
        .bind(speed)
        .bind(direction)
        .execute(pool)
        .await?;
        Ok(r.rows_affected())
    }
    #[cfg(feature = "postgres")]
    {
        let r = sqlx::query(
            "INSERT INTO wvp_position_history (device_id, timestamp, longitude, latitude, altitude, speed, direction) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(device_id)
        .bind(timestamp)
        .bind(longitude)
        .bind(latitude)
        .bind(altitude)
        .bind(speed)
        .bind(direction)
        .execute(pool)
        .await?;
        Ok(r.rows_affected())
    }
}

pub async fn list_by_device_and_time(
    pool: &Pool,
    device_id: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> sqlx::Result<Vec<PositionHistory>> {
    if let (Some(s), Some(e)) = (start, end) {
        #[cfg(feature = "mysql")]
        {
            sqlx::query_as::<_, PositionHistory>(
                "SELECT id, device_id, timestamp, longitude, latitude, altitude, speed, direction FROM wvp_position_history WHERE device_id = ? AND timestamp >= ? AND timestamp <= ? ORDER BY timestamp ASC",
            )
            .bind(device_id).bind(s).bind(e)
            .fetch_all(pool).await
        }
        #[cfg(feature = "postgres")]
        {
            sqlx::query_as::<_, PositionHistory>(
                "SELECT id, device_id, timestamp, longitude, latitude, altitude, speed, direction FROM wvp_position_history WHERE device_id = $1 AND timestamp >= $2 AND timestamp <= $3 ORDER BY timestamp ASC",
            )
            .bind(device_id).bind(s).bind(e)
            .fetch_all(pool).await
        }
    } else {
        #[cfg(feature = "mysql")]
        {
            sqlx::query_as::<_, PositionHistory>(
                "SELECT id, device_id, timestamp, longitude, latitude, altitude, speed, direction FROM wvp_position_history WHERE device_id = ? ORDER BY timestamp ASC",
            )
            .bind(device_id)
            .fetch_all(pool).await
        }
        #[cfg(feature = "postgres")]
        {
            sqlx::query_as::<_, PositionHistory>(
                "SELECT id, device_id, timestamp, longitude, latitude, altitude, speed, direction FROM wvp_position_history WHERE device_id = $1 ORDER BY timestamp ASC",
            )
            .bind(device_id)
            .fetch_all(pool).await
        }
    }
}
