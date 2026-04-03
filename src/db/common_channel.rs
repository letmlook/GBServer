use sqlx::{FromRow, Row};
use crate::db::Pool;
use crate::db::device::DeviceChannel;

pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<DeviceChannel>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

pub async fn update(
    pool: &Pool,
    id: i64,
    name: Option<&str>,
    channel_id: Option<&str>,
    civil_code: Option<&str>,
    parent_id: Option<i64>,
    business_group: Option<&str>,
    ptz_type: Option<i32>,
    custom_name: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(
            r#"UPDATE wvp_device_channel SET 
               name = COALESCE(?, name),
               gb_device_id = COALESCE(?, gb_device_id),
               civil_code = COALESCE(?, civil_code),
               parent_id = COALESCE(?, parent_id),
               business_group = COALESCE(?, business_group),
               ptz_type = COALESCE(?, ptz_type),
               custom_name = COALESCE(?, custom_name),
               update_time = ?
               WHERE id = ?"#,
        )
        .bind(name).bind(channel_id).bind(civil_code).bind(parent_id)
        .bind(business_group).bind(ptz_type).bind(custom_name).bind(now).bind(id)
        .execute(pool)
        .await?;
        Ok(r.rows_affected())
    }
    #[cfg(feature = "postgres")]
    {
        let r = sqlx::query(
            r#"UPDATE wvp_device_channel SET 
               name = COALESCE($1, name),
               gb_device_id = COALESCE($2, gb_device_id),
               civil_code = COALESCE($3, civil_code),
               parent_id = COALESCE($4, parent_id),
               business_group = COALESCE($5, business_group),
               ptz_type = COALESCE($6, ptz_type),
               custom_name = COALESCE($7, custom_name),
               update_time = $8
               WHERE id = $9"#,
        )
        .bind(name).bind(channel_id).bind(civil_code).bind(parent_id)
        .bind(business_group).bind(ptz_type).bind(custom_name).bind(now).bind(id)
        .execute(pool)
        .await?;
        Ok(r.rows_affected())
    }
}

pub async fn reset(pool: &Pool, id: i64, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET sub_count = 0, update_time = ? WHERE id = ?")
        .bind(now).bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET sub_count = 0, update_time = $1 WHERE id = $2")
        .bind(now).bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn add(
    pool: &Pool,
    device_id: &str,
    name: &str,
    channel_id: &str,
    civil_code: Option<&str>,
    parent_id: Option<i64>,
    business_group: Option<&str>,
    ptz_type: Option<i32>,
    custom_name: Option<&str>,
    now: &str,
) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            r#"INSERT INTO wvp_device_channel (device_id, name, gb_device_id, civil_code, parent_id, business_group, ptz_type, custom_name, create_time, update_time)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(device_id).bind(name).bind(channel_id).bind(civil_code).bind(parent_id)
        .bind(business_group).bind(ptz_type).bind(custom_name).bind(now).bind(now)
        .execute(pool)
        .await?;
        Ok(result.last_insert_id() as i64)
    }
    #[cfg(feature = "postgres")]
    {
        let row = sqlx::query(
            r#"INSERT INTO wvp_device_channel (device_id, name, gb_device_id, civil_code, parent_id, business_group, ptz_type, custom_name, create_time, update_time)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id"#,
        )
        .bind(device_id).bind(name).bind(channel_id).bind(civil_code).bind(parent_id)
        .bind(business_group).bind(ptz_type).bind(custom_name).bind(now).bind(now)
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i32, _>("id") as i64)
    }
}

pub async fn get_unusual_civilcode(pool: &Pool, page: u32, count: u32) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = (page.saturating_sub(1)) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE civil_code IS NULL OR civil_code = '' ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(count as i64).bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE civil_code IS NULL OR civil_code = '' ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(count as i64).bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_unusual_civilcode(pool: &Pool) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_device_channel WHERE civil_code IS NULL OR civil_code = ''",
    )
    .fetch_one(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_device_channel WHERE civil_code IS NULL OR civil_code = ''",
    )
    .fetch_one(pool)
    .await;
}

pub async fn get_unusual_parent(pool: &Pool, page: u32, count: u32) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = (page.saturating_sub(1)) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE parent_id IS NULL OR parent_id = 0 ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(count as i64).bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE parent_id IS NULL OR parent_id = 0 ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(count as i64).bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_unusual_parent(pool: &Pool) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_device_channel WHERE parent_id IS NULL OR parent_id = 0",
    )
    .fetch_one(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_device_channel WHERE parent_id IS NULL OR parent_id = 0",
    )
    .fetch_one(pool)
    .await;
}

pub async fn clear_unusual_civilcode(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = NULL WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn clear_unusual_parent(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = 0 WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn get_parent_channels(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = (page.saturating_sub(1)) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE parent_id IS NOT NULL AND parent_id != 0 ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(count as i64).bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE parent_id IS NOT NULL AND parent_id != 0 ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(count as i64).bind(offset as i64)
    .fetch_all(pool)
    .await;
}

pub async fn count_parent_channels(
    pool: &Pool,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_device_channel WHERE parent_id IS NOT NULL AND parent_id != 0",
    )
    .fetch_one(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM wvp_device_channel WHERE parent_id IS NOT NULL AND parent_id != 0",
    )
    .fetch_one(pool)
    .await;
}

pub async fn update_civil_code(pool: &Pool, id: i64, civil_code: &str, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = ?, update_time = ? WHERE id = ?")
        .bind(civil_code).bind(now).bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = $1, update_time = $2 WHERE id = $3")
        .bind(civil_code).bind(now).bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn clear_civil_code(pool: &Pool, id: i64, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = NULL, update_time = ? WHERE id = ?")
        .bind(now).bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = NULL, update_time = $1 WHERE id = $2")
        .bind(now).bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn update_device_civil_code(pool: &Pool, device_id: &str, civil_code: &str, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = ?, update_time = ? WHERE device_id = ?")
        .bind(civil_code).bind(now).bind(device_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = $1, update_time = $2 WHERE device_id = $3")
        .bind(civil_code).bind(now).bind(device_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn clear_device_civil_code(pool: &Pool, device_id: &str, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = NULL, update_time = ? WHERE device_id = ?")
        .bind(now).bind(device_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET civil_code = NULL, update_time = $1 WHERE device_id = $2")
        .bind(now).bind(device_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn update_group(pool: &Pool, id: i64, parent_id: i64, business_group: &str, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = ?, business_group = ?, update_time = ? WHERE id = ?")
        .bind(parent_id).bind(business_group).bind(now).bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = $1, business_group = $2, update_time = $3 WHERE id = $4")
        .bind(parent_id).bind(business_group).bind(now).bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn clear_group(pool: &Pool, id: i64, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = 0, business_group = NULL, update_time = ? WHERE id = ?")
        .bind(now).bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = 0, business_group = NULL, update_time = $1 WHERE id = $2")
        .bind(now).bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn update_device_group(pool: &Pool, device_id: &str, parent_id: i64, business_group: &str, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = ?, business_group = ?, update_time = ? WHERE device_id = ?")
        .bind(parent_id).bind(business_group).bind(now).bind(device_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = $1, business_group = $2, update_time = $3 WHERE device_id = $4")
        .bind(parent_id).bind(business_group).bind(now).bind(device_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn clear_device_group(pool: &Pool, device_id: &str, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = 0, business_group = NULL, update_time = ? WHERE device_id = ?")
        .bind(now).bind(device_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_device_channel SET parent_id = 0, business_group = NULL, update_time = $1 WHERE device_id = $2")
        .bind(now).bind(device_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn get_channels_for_map(
    pool: &Pool,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<Vec<DeviceChannel>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE longitude IS NOT NULL AND latitude IS NOT NULL ORDER BY id LIMIT 1000",
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE longitude IS NOT NULL AND latitude IS NOT NULL ORDER BY id LIMIT 1000",
    )
    .fetch_all(pool)
    .await;
}
