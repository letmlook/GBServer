//! Position history table: gb_position_history
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
    // 三个驱动**同时**编进一个二进制之后，编译期 `#[cfg(feature = ...)]`
    // 的三份 CREATE TABLE 会**全部执行** —— 先落地的那份决定表结构，
    // 于是 sqlite 上建出 `id BIGINT AUTO_INCREMENT PRIMARY KEY` 的表
    // （不是 rowid 别名），后续 INSERT 报 "datatype mismatch"。
    // 改成**运行期**按当前方言三选一。
    let sql = match crate::db::dialect::current() {
        crate::db::Dialect::MySql => "CREATE TABLE IF NOT EXISTS gb_position_history (\n\n  id BIGINT AUTO_INCREMENT PRIMARY KEY,\n  device_id VARCHAR(50) NOT NULL,\n  timestamp VARCHAR(50) NOT NULL,\n  longitude DOUBLE,\n  latitude DOUBLE,\n  altitude DOUBLE,\n  speed DOUBLE,\n  direction DOUBLE\n)",
        crate::db::Dialect::Postgres => "CREATE TABLE IF NOT EXISTS gb_position_history (\n\n  id BIGSERIAL PRIMARY KEY,\n  device_id VARCHAR(50) NOT NULL,\n  timestamp VARCHAR(50) NOT NULL,\n  longitude DOUBLE PRECISION,\n  latitude DOUBLE PRECISION,\n  altitude DOUBLE PRECISION,\n  speed DOUBLE PRECISION,\n  direction DOUBLE PRECISION\n)",
        crate::db::Dialect::Sqlite => "CREATE TABLE IF NOT EXISTS gb_position_history (\n\n  id INTEGER PRIMARY KEY AUTOINCREMENT,\n  device_id VARCHAR(50) NOT NULL,\n  timestamp VARCHAR(50) NOT NULL,\n  longitude REAL,\n  latitude REAL,\n  altitude REAL,\n  speed REAL,\n  direction REAL\n)",
    };

    crate::dispatch_exec!(pool, |p| { sqlx::query(sql).execute(p) })?;
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
    let sql = crate::db::dialect::current_sql("INSERT INTO gb_position_history (device_id, timestamp, longitude, latitude, altitude, speed, direction) VALUES (?, ?, ?, ?, ?, ?, ?)").into_owned();

    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(device_id)
            .bind(timestamp)
            .bind(longitude)
            .bind(latitude)
            .bind(altitude)
            .bind(speed)
            .bind(direction)
            .execute(p)
    })?;

    Ok(r.rows_affected())
}

pub async fn list_by_device_and_time(
    pool: &Pool,
    device_id: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> sqlx::Result<Vec<PositionHistory>> {
    if let (Some(s), Some(e)) = (start, end) {
        let sql = crate::db::dialect::current_sql("SELECT id, device_id, timestamp, longitude, latitude, altitude, speed, direction FROM gb_position_history WHERE device_id = ? AND timestamp >= ? AND timestamp <= ? ORDER BY timestamp ASC").into_owned();
        return crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, PositionHistory>(&sql)
                .bind(device_id)
                .bind(s)
                .bind(e)
                .fetch_all(p)
        });
    } else {
        let sql = crate::db::dialect::current_sql("SELECT id, device_id, timestamp, longitude, latitude, altitude, speed, direction FROM gb_position_history WHERE device_id = ? ORDER BY timestamp ASC").into_owned();
        return crate::dispatch!(pool, |p| {
            sqlx::query_as::<_, PositionHistory>(&sql)
                .bind(device_id)
                .fetch_all(p)
        });
    }
}
