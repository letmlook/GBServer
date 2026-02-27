//! 国标设备与通道表 wvp_device, wvp_device_channel

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Device {
    pub id: i64,
    pub device_id: String,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub stream_mode: Option<String>,
    pub on_line: Option<bool>,
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub media_server_id: Option<String>,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DeviceChannel {
    pub id: i64,
    pub device_id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "channelId")]
    pub gb_device_id: Option<String>,
    pub status: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub sub_count: Option<i32>,
    pub has_audio: Option<bool>,
    pub channel_type: Option<i32>,
}

pub async fn list_devices_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
    status: Option<bool>,
) -> sqlx::Result<Vec<Device>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    let offset = offset as i64;
    let q = query.unwrap_or("").trim();
    let has_query = !q.is_empty();
    let like = format!("%{q}%");

    #[cfg(feature = "mysql")]
    let rows = if has_query && status.is_some() {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE (device_id LIKE ? OR name LIKE ?) AND on_line = ? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(&like).bind(&like).bind(status.unwrap()).bind(limit).bind(offset)
        .fetch_all(pool).await?
    } else if has_query {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE (device_id LIKE ? OR name LIKE ?) ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(&like).bind(&like).bind(limit).bind(offset)
        .fetch_all(pool).await?
    } else if status.is_some() {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE on_line = ? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(status.unwrap()).bind(limit).bind(offset)
        .fetch_all(pool).await?
    } else {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(limit).bind(offset)
        .fetch_all(pool).await?
    };
    #[cfg(feature = "postgres")]
    let rows = if has_query && status.is_some() {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE (device_id LIKE $1 OR name LIKE $2) AND on_line = $3 ORDER BY id LIMIT $4 OFFSET $5",
        )
        .bind(&like).bind(&like).bind(status.unwrap()).bind(limit).bind(offset)
        .fetch_all(pool).await?
    } else if has_query {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE (device_id LIKE $1 OR name LIKE $2) ORDER BY id LIMIT $3 OFFSET $4",
        )
        .bind(&like).bind(&like).bind(limit).bind(offset)
        .fetch_all(pool).await?
    } else if status.is_some() {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE on_line = $1 ORDER BY id LIMIT $2 OFFSET $3",
        )
        .bind(status.unwrap()).bind(limit).bind(offset)
        .fetch_all(pool).await?
    } else {
        sqlx::query_as::<_, Device>(
            "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device ORDER BY id LIMIT $1 OFFSET $2",
        )
        .bind(limit).bind(offset)
        .fetch_all(pool).await?
    };
    Ok(rows)
}

pub async fn count_devices(
    pool: &Pool,
    query: Option<&str>,
    status: Option<bool>,
) -> sqlx::Result<i64> {
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();
    #[cfg(feature = "mysql")]
    {
        if has_query && status.is_some() {
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device WHERE (device_id LIKE ? OR name LIKE ?) AND on_line = ?")
                .bind(&like).bind(&like).bind(status.unwrap()).fetch_one(pool).await;
        }
        if has_query {
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device WHERE (device_id LIKE ? OR name LIKE ?)")
                .bind(&like).bind(&like).fetch_one(pool).await;
        }
        if status.is_some() {
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device WHERE on_line = ?")
                .bind(status.unwrap()).fetch_one(pool).await;
        }
        return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device").fetch_one(pool).await;
    }
    #[cfg(feature = "postgres")]
    {
        if has_query && status.is_some() {
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device WHERE (device_id LIKE $1 OR name LIKE $2) AND on_line = $3")
                .bind(&like).bind(&like).bind(status.unwrap()).fetch_one(pool).await;
        }
        if has_query {
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device WHERE (device_id LIKE $1 OR name LIKE $2)")
                .bind(&like).bind(&like).fetch_one(pool).await;
        }
        if status.is_some() {
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device WHERE on_line = $1")
                .bind(status.unwrap()).fetch_one(pool).await;
        }
        return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device").fetch_one(pool).await;
    }
}

pub async fn get_device_by_device_id(pool: &Pool, device_id: &str) -> sqlx::Result<Option<Device>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Device>(
        "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Device>(
        "SELECT id, device_id, name, manufacturer, model, transport, stream_mode, on_line, ip, port, create_time, update_time, media_server_id, custom_name FROM wvp_device WHERE device_id = $1",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await;
}

pub async fn list_channels_paged(
    pool: &Pool,
    device_id: &str,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = (page.saturating_sub(1)) * count;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = ? ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(device_id).bind(count as i64).bind(offset as i64)
    .fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
    )
    .bind(device_id).bind(count as i64).bind(offset as i64)
    .fetch_all(pool).await;
}

pub async fn count_channels(pool: &Pool, device_id: &str) -> sqlx::Result<i64> {
    #[cfg(feature = "mysql")]
    return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel WHERE device_id = ?")
        .bind(device_id).fetch_one(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel WHERE device_id = $1")
        .bind(device_id).fetch_one(pool).await;
}

pub async fn get_channel_by_device_and_channel_id(
    pool: &Pool,
    device_id: &str,
    channel_id: &str,
) -> sqlx::Result<Option<DeviceChannel>> {
    let id_val = channel_id.parse::<i64>().unwrap_or(0);
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = ? AND (gb_device_id = ? OR id = ?) LIMIT 1",
    )
    .bind(device_id).bind(channel_id).bind(id_val)
    .fetch_optional(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = $1 AND (gb_device_id = $2 OR id = $3) LIMIT 1",
    )
    .bind(device_id).bind(channel_id).bind(id_val)
    .fetch_optional(pool).await;
}

pub async fn list_channels_by_parent(
    pool: &Pool,
    device_id: &str,
    parent_channel_id: &str,
) -> sqlx::Result<Vec<DeviceChannel>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = ? AND (parent_id = ? OR gb_parent_id = ?) ORDER BY id",
    )
    .bind(device_id).bind(parent_channel_id).bind(parent_channel_id)
    .fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = $1 AND (parent_id = $2 OR gb_parent_id = $3) ORDER BY id",
    )
    .bind(device_id).bind(parent_channel_id).bind(parent_channel_id)
    .fetch_all(pool).await;
}

pub async fn list_channels_for_device(
    pool: &Pool,
    device_id: &str,
) -> sqlx::Result<Vec<DeviceChannel>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = ? ORDER BY id",
    )
    .bind(device_id).fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, DeviceChannel>(
        "SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel WHERE device_id = $1 ORDER BY id",
    )
    .bind(device_id).fetch_all(pool).await;
}

/// 通用通道列表：全量分页，支持 query(名称/通道号)、online(设备在线)、channel_type
pub async fn list_common_channels_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<Vec<DeviceChannel>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    let offset = offset as i64;
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();

    #[cfg(feature = "mysql")]
    {
        let rows = if has_query && online.is_some() && channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ? AND c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(&like).bind(&like).bind(online.unwrap()).bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if has_query && online.is_some() {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ? ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(&like).bind(&like).bind(online.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if has_query && channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(&like).bind(&like).bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if has_query {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(&like).bind(&like).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if online.is_some() && channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = ? AND c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(online.unwrap()).bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if online.is_some() {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = ? ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(online.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c WHERE c.channel_type = ? ORDER BY c.id LIMIT ? OFFSET ?")
                .bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else {
            sqlx::query_as::<_, DeviceChannel>("SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel ORDER BY id LIMIT ? OFFSET ?")
                .bind(limit).bind(offset)
                .fetch_all(pool).await?
        };
        Ok(rows)
    }

    #[cfg(feature = "postgres")]
    {
        let (sql, _): (_, Vec<i64>) = if has_query && online.is_some() && channel_type.is_some() {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) AND d.on_line = $3 AND c.channel_type = $4 ORDER BY c.id LIMIT $5 OFFSET $6", vec![])
        } else if has_query && online.is_some() {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) AND d.on_line = $3 ORDER BY c.id LIMIT $4 OFFSET $5", vec![])
        } else if has_query && channel_type.is_some() {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) AND c.channel_type = $3 ORDER BY c.id LIMIT $4 OFFSET $5", vec![])
        } else if has_query {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) ORDER BY c.id LIMIT $3 OFFSET $4", vec![])
        } else if online.is_some() && channel_type.is_some() {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = $1 AND c.channel_type = $2 ORDER BY c.id LIMIT $3 OFFSET $4", vec![])
        } else if online.is_some() {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = $1 ORDER BY c.id LIMIT $2 OFFSET $3", vec![])
        } else if channel_type.is_some() {
            ("SELECT c.id, c.device_id, c.name, c.gb_device_id, c.status, c.longitude, c.latitude, c.create_time, c.update_time, c.sub_count, c.has_audio, c.channel_type FROM wvp_device_channel c WHERE c.channel_type = $1 ORDER BY c.id LIMIT $2 OFFSET $3", vec![])
        } else {
            ("SELECT id, device_id, name, gb_device_id, status, longitude, latitude, create_time, update_time, sub_count, has_audio, channel_type FROM wvp_device_channel ORDER BY id LIMIT $1 OFFSET $2", vec![])
        };
        // Build query with bind - use raw to avoid dynamic param count
        let rows = if has_query && online.is_some() && channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(&like).bind(&like).bind(online.unwrap()).bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if has_query && online.is_some() {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(&like).bind(&like).bind(online.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if has_query && channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(&like).bind(&like).bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if has_query {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(&like).bind(&like).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if online.is_some() && channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(online.unwrap()).bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if online.is_some() {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(online.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else if channel_type.is_some() {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(channel_type.unwrap()).bind(limit).bind(offset)
                .fetch_all(pool).await?
        } else {
            sqlx::query_as::<_, DeviceChannel>(sql)
                .bind(limit).bind(offset)
                .fetch_all(pool).await?
        };
        Ok(rows)
    }
}

pub async fn count_common_channels(
    pool: &Pool,
    query: Option<&str>,
    online: Option<bool>,
    channel_type: Option<i32>,
) -> sqlx::Result<i64> {
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();

    #[cfg(feature = "mysql")]
    {
        let row = if has_query && online.is_some() && channel_type.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ? AND c.channel_type = ?")
                .bind(&like).bind(&like).bind(online.unwrap()).bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else if has_query && online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND d.on_line = ?")
                .bind(&like).bind(&like).bind(online.unwrap())
                .fetch_one(pool).await?
        } else if has_query && channel_type.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?) AND c.channel_type = ?")
                .bind(&like).bind(&like).bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else if has_query {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c WHERE (c.name LIKE ? OR c.gb_device_id LIKE ?)")
                .bind(&like).bind(&like)
                .fetch_one(pool).await?
        } else if online.is_some() && channel_type.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = ? AND c.channel_type = ?")
                .bind(online.unwrap()).bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = ?")
                .bind(online.unwrap())
                .fetch_one(pool).await?
        } else if channel_type.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel c WHERE c.channel_type = ?")
                .bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_device_channel")
                .fetch_one(pool).await?
        };
        Ok(row)
    }

    #[cfg(feature = "postgres")]
    {
        let sql = if has_query && online.is_some() && channel_type.is_some() {
            "SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) AND d.on_line = $3 AND c.channel_type = $4"
        } else if has_query && online.is_some() {
            "SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) AND d.on_line = $3"
        } else if has_query && channel_type.is_some() {
            "SELECT COUNT(*) FROM wvp_device_channel c WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2) AND c.channel_type = $3"
        } else if has_query {
            "SELECT COUNT(*) FROM wvp_device_channel c WHERE (c.name LIKE $1 OR c.gb_device_id LIKE $2)"
        } else if online.is_some() && channel_type.is_some() {
            "SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = $1 AND c.channel_type = $2"
        } else if online.is_some() {
            "SELECT COUNT(*) FROM wvp_device_channel c INNER JOIN wvp_device d ON c.device_id = d.device_id WHERE d.on_line = $1"
        } else if channel_type.is_some() {
            "SELECT COUNT(*) FROM wvp_device_channel c WHERE c.channel_type = $1"
        } else {
            "SELECT COUNT(*) FROM wvp_device_channel"
        };
        let row = if has_query && online.is_some() && channel_type.is_some() {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(&like).bind(&like).bind(online.unwrap()).bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else if has_query && online.is_some() {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(&like).bind(&like).bind(online.unwrap())
                .fetch_one(pool).await?
        } else if has_query && channel_type.is_some() {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(&like).bind(&like).bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else if has_query {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(&like).bind(&like)
                .fetch_one(pool).await?
        } else if online.is_some() && channel_type.is_some() {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(online.unwrap()).bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(online.unwrap())
                .fetch_one(pool).await?
        } else if channel_type.is_some() {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(channel_type.unwrap())
                .fetch_one(pool).await?
        } else {
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(pool).await?
        };
        Ok(row)
    }
}

pub async fn delete_device_cascade(pool: &Pool, device_id: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    {
        let _ = sqlx::query("DELETE FROM wvp_device_channel WHERE device_id = ?").bind(device_id).execute(pool).await?;
        let r = sqlx::query("DELETE FROM wvp_device WHERE device_id = ?").bind(device_id).execute(pool).await?;
        return Ok(r.rows_affected());
    }
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("DELETE FROM wvp_device_channel WHERE device_id = $1").bind(device_id).execute(pool).await?;
        let r = sqlx::query("DELETE FROM wvp_device WHERE device_id = $1").bind(device_id).execute(pool).await?;
        return Ok(r.rows_affected());
    }
}

pub async fn insert_device(
    pool: &Pool,
    device_id: &str,
    name: Option<&str>,
    manufacturer: Option<&str>,
    model: Option<&str>,
    transport: Option<&str>,
    stream_mode: Option<&str>,
    media_server_id: Option<&str>,
    custom_name: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let mid = media_server_id.unwrap_or("auto");
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_device (device_id, name, manufacturer, model, transport, stream_mode, on_line, create_time, update_time, media_server_id, custom_name)
           VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?)"#,
    )
    .bind(device_id).bind(name).bind(manufacturer).bind(model).bind(transport).bind(stream_mode)
    .bind(now).bind(now).bind(mid).bind(custom_name)
    .execute(pool).await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_device (device_id, name, manufacturer, model, transport, stream_mode, on_line, create_time, update_time, media_server_id, custom_name)
           VALUES ($1, $2, $3, $4, $5, $6, false, $7, $8, $9, $10)"#,
    )
    .bind(device_id).bind(name).bind(manufacturer).bind(model).bind(transport).bind(stream_mode)
    .bind(now).bind(now).bind(mid).bind(custom_name)
    .execute(pool).await?;
    Ok(r.rows_affected())
}

pub async fn update_device(
    pool: &Pool,
    device_id: &str,
    name: Option<&str>,
    manufacturer: Option<&str>,
    model: Option<&str>,
    transport: Option<&str>,
    stream_mode: Option<&str>,
    media_server_id: Option<&str>,
    custom_name: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE wvp_device SET name = COALESCE(?, name), manufacturer = COALESCE(?, manufacturer), model = COALESCE(?, model),
           transport = COALESCE(?, transport), stream_mode = COALESCE(?, stream_mode), media_server_id = COALESCE(?, media_server_id),
           custom_name = COALESCE(?, custom_name), update_time = ? WHERE device_id = ?"#,
    )
    .bind(name).bind(manufacturer).bind(model).bind(transport).bind(stream_mode).bind(media_server_id).bind(custom_name).bind(now).bind(device_id)
    .execute(pool).await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE wvp_device SET name = COALESCE($1, name), manufacturer = COALESCE($2, manufacturer), model = COALESCE($3, model),
           transport = COALESCE($4, transport), stream_mode = COALESCE($5, stream_mode), media_server_id = COALESCE($6, media_server_id),
           custom_name = COALESCE($7, custom_name), update_time = $8 WHERE device_id = $9"#,
    )
    .bind(name).bind(manufacturer).bind(model).bind(transport).bind(stream_mode).bind(media_server_id).bind(custom_name).bind(now).bind(device_id)
    .execute(pool).await?;
    Ok(r.rows_affected())
}
