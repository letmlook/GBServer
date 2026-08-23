use serde::Serialize;
use sqlx::FromRow;
use crate::db::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JtTerminal {
    pub id: i32,
    pub phone_number: String,
    pub terminal_id: Option<String>,
    pub province_id: Option<i32>,
    pub province_text: Option<String>,
    pub city_id: Option<i32>,
    pub city_text: Option<String>,
    pub maker_id: Option<String>,
    pub model: Option<String>,
    pub plate_color: Option<i32>,
    pub plate_no: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub status: Option<bool>,
    pub register_time: Option<String>,
    pub update_time: Option<String>,
    pub create_time: Option<String>,
    pub geo_coord_sys: Option<String>,
    pub media_server_id: Option<String>,
    pub sdp_ip: Option<String>,
    /// Phase 6.1: authentication code for terminal register response (0x8100).
    /// Read from DB; matched against incoming 0x0100 register body.
    pub auth_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JtChannel {
    pub id: i32,
    pub terminal_db_id: i32,
    pub channel_id: i32,
    pub has_audio: Option<bool>,
    pub name: Option<String>,
    pub update_time: Option<String>,
    pub create_time: Option<String>,
}

pub async fn list_terminals_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    query: Option<&str>,
    online: Option<bool>,
) -> sqlx::Result<Vec<JtTerminal>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    let offset = offset as i64;
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();

    #[cfg(feature = "mysql")]
    {
        let sql = if has_query && online.is_some() {
            "SELECT * FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else if has_query {
            "SELECT * FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) ORDER BY id LIMIT ? OFFSET ?"
        } else if online.is_some() {
            "SELECT * FROM gb_jt_terminal WHERE status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else {
            "SELECT * FROM gb_jt_terminal ORDER BY id LIMIT ? OFFSET ?"
        };
        let rows = if has_query && online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql).bind(&like).bind(&like).bind(online.unwrap()).bind(limit).bind(offset).fetch_all(pool).await?
        } else if has_query {
            sqlx::query_as::<_, JtTerminal>(sql).bind(&like).bind(&like).bind(limit).bind(offset).fetch_all(pool).await?
        } else if online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql).bind(online.unwrap()).bind(limit).bind(offset).fetch_all(pool).await?
        } else {
            sqlx::query_as::<_, JtTerminal>(sql).bind(limit).bind(offset).fetch_all(pool).await?
        };
        Ok(rows)
    }

    #[cfg(feature = "postgres")]
    {
        let sql = if has_query && online.is_some() {
            "SELECT * FROM gb_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) AND status = $3 ORDER BY id LIMIT $4 OFFSET $5"
        } else if has_query {
            "SELECT * FROM gb_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) ORDER BY id LIMIT $3 OFFSET $4"
        } else if online.is_some() {
            "SELECT * FROM gb_jt_terminal WHERE status = $1 ORDER BY id LIMIT $2 OFFSET $3"
        } else {
            "SELECT * FROM gb_jt_terminal ORDER BY id LIMIT $1 OFFSET $2"
        };
        let rows = if has_query && online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql).bind(&like).bind(&like).bind(online.unwrap()).bind(limit).bind(offset).fetch_all(pool).await?
        } else if has_query {
            sqlx::query_as::<_, JtTerminal>(sql).bind(&like).bind(&like).bind(limit).bind(offset).fetch_all(pool).await?
        } else if online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql).bind(online.unwrap()).bind(limit).bind(offset).fetch_all(pool).await?
        } else {
            sqlx::query_as::<_, JtTerminal>(sql).bind(limit).bind(offset).fetch_all(pool).await?
        };
        Ok(rows)
    }

    #[cfg(feature = "sqlite")]
    {
        let sql = if has_query && online.is_some() {
            "SELECT * FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else if has_query {
            "SELECT * FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) ORDER BY id LIMIT ? OFFSET ?"
        } else if online.is_some() {
            "SELECT * FROM gb_jt_terminal WHERE status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else {
            "SELECT * FROM gb_jt_terminal ORDER BY id LIMIT ? OFFSET ?"
        };
        let rows = if has_query && online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql).bind(&like).bind(&like).bind(online.unwrap()).bind(limit).bind(offset).fetch_all(pool).await?
        } else if has_query {
            sqlx::query_as::<_, JtTerminal>(sql).bind(&like).bind(&like).bind(limit).bind(offset).fetch_all(pool).await?
        } else if online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql).bind(online.unwrap()).bind(limit).bind(offset).fetch_all(pool).await?
        } else {
            sqlx::query_as::<_, JtTerminal>(sql).bind(limit).bind(offset).fetch_all(pool).await?
        };
        Ok(rows)
    }
}

pub async fn count_terminals(
    pool: &Pool,
    query: Option<&str>,
    online: Option<bool>,
) -> sqlx::Result<i64> {
    let q = query.unwrap_or("").trim();
    let like = format!("%{q}%");
    let has_query = !q.is_empty();

    #[cfg(feature = "mysql")]
    {
        if has_query && online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ?")
                .bind(&like).bind(&like).bind(online.unwrap()).fetch_one(pool).await
        } else if has_query {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?)")
                .bind(&like).bind(&like).fetch_one(pool).await
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = ?")
                .bind(online.unwrap()).fetch_one(pool).await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal").fetch_one(pool).await
        }
    }

    #[cfg(feature = "postgres")]
    {
        if has_query && online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) AND status = $3")
                .bind(&like).bind(&like).bind(online.unwrap()).fetch_one(pool).await
        } else if has_query {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2)")
                .bind(&like).bind(&like).fetch_one(pool).await
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = $1")
                .bind(online.unwrap()).fetch_one(pool).await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal").fetch_one(pool).await
        }
    }

    #[cfg(feature = "sqlite")]
    {
        if has_query && online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ?")
                .bind(&like).bind(&like).bind(online.unwrap()).fetch_one(pool).await
        } else if has_query {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?)")
                .bind(&like).bind(&like).fetch_one(pool).await
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = ?")
                .bind(online.unwrap()).fetch_one(pool).await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal").fetch_one(pool).await
        }
    }
}

pub async fn get_terminal_by_phone(pool: &Pool, phone: &str) -> sqlx::Result<Option<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM gb_jt_terminal WHERE phone_number = ?")
        .bind(phone).fetch_optional(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM gb_jt_terminal WHERE phone_number = $1")
        .bind(phone).fetch_optional(pool).await;
}

/// 根据ID查询终端
pub async fn get_terminal_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM gb_jt_terminal WHERE id = ?")
        .bind(id).fetch_optional(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM gb_jt_terminal WHERE id = $1")
        .bind(id).fetch_optional(pool).await;
}

/// 根据ID查询通道
pub async fn get_channel_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<JtChannel>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM gb_jt_channel WHERE id = ?")
        .bind(id).fetch_optional(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM gb_jt_channel WHERE id = $1")
        .bind(id).fetch_optional(pool).await;
}

/// 获取所有在线终端
pub async fn get_online_terminals(pool: &Pool) -> sqlx::Result<Vec<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM gb_jt_terminal WHERE status = 1 ORDER BY id")
        .fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM gb_jt_terminal WHERE status = true ORDER BY id")
        .fetch_all(pool).await;
}

/// 统计在线终端数量
pub async fn count_online_terminals(pool: &Pool) -> sqlx::Result<i64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = 1")
        .fetch_one(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = true")
        .fetch_one(pool).await;
}

/// Insert a new JT1078 channel for a terminal identified by phone_number.
/// Returns number of rows affected.
pub async fn insert_channel(
    pool: &Pool,
    phone_number: &str,
    channel_id: i32,
    name: Option<&str>,
) -> sqlx::Result<u64> {
    // Resolve terminal first
    if let Some(term) = get_terminal_by_phone(pool, phone_number).await? {
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        {
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let r = sqlx::query(
                "INSERT INTO gb_jt_channel (terminal_db_id, channel_id, name, create_time, update_time) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(term.id)
            .bind(channel_id)
            .bind(name)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
            Ok(r.rows_affected())
        }
        #[cfg(feature = "postgres")]
        {
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let r = sqlx::query(
                "INSERT INTO gb_jt_channel (terminal_db_id, channel_id, name, create_time, update_time) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(term.id)
            .bind(channel_id)
            .bind(name)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
            Ok(r.rows_affected())
        }
    } else {
        // Terminal not found, no insert
        Ok(0)
    }
}

/// Update an existing JT1078 channel by its DB id.
/// Allows updating of name and channel_id fields.
pub async fn update_channel(
    pool: &Pool,
    id: i64,
    name: Option<&str>,
    channel_id: Option<i32>,
) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let r = sqlx::query(
            "UPDATE gb_jt_channel SET name = COALESCE(?, name), channel_id = COALESCE(?, channel_id), update_time = ? WHERE id = ?",
        )
        .bind(name)
        .bind(channel_id)
        .bind(now)
        .bind(id as i64)
        .execute(pool)
        .await?;
        Ok(r.rows_affected())
    }
    #[cfg(feature = "postgres")]
    {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let r = sqlx::query(
            "UPDATE gb_jt_channel SET name = COALESCE($1, name), channel_id = COALESCE($2, channel_id), update_time = $3 WHERE id = $4",
        )
        .bind(name)
        .bind(channel_id)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(r.rows_affected())
    }
}

pub async fn list_channels_by_terminal(
    pool: &Pool,
    terminal_db_id: i32,
) -> sqlx::Result<Vec<JtChannel>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM gb_jt_channel WHERE terminal_db_id = ? ORDER BY channel_id")
        .bind(terminal_db_id).fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM gb_jt_channel WHERE terminal_db_id = $1 ORDER BY channel_id")
        .bind(terminal_db_id).fetch_all(pool).await;
}

pub async fn insert_terminal(
    pool: &Pool,
    phone_number: &str,
    terminal_id: Option<&str>,
    plate_no: Option<&str>,
    plate_color: Option<i32>,
    maker_id: Option<&str>,
    model: Option<&str>,
    media_server_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query(
        "INSERT INTO gb_jt_terminal (phone_number, terminal_id, plate_no, plate_color, maker_id, model, media_server_id, status, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    ).bind(phone_number).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(now)
    .execute(pool).await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO gb_jt_terminal (phone_number, terminal_id, plate_no, plate_color, maker_id, model, media_server_id, status, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8, $9)",
    ).bind(phone_number).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(now)
    .execute(pool).await?;
    Ok(r.rows_affected())
}

pub async fn update_terminal(
    pool: &Pool,
    phone_number: &str,
    terminal_id: Option<&str>,
    plate_no: Option<&str>,
    plate_color: Option<i32>,
    maker_id: Option<&str>,
    model: Option<&str>,
    media_server_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query(
        "UPDATE gb_jt_terminal SET terminal_id = COALESCE(?, terminal_id), plate_no = COALESCE(?, plate_no), plate_color = COALESCE(?, plate_color), maker_id = COALESCE(?, maker_id), model = COALESCE(?, model), media_server_id = COALESCE(?, media_server_id), update_time = ? WHERE phone_number = ?",
    ).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(phone_number)
    .execute(pool).await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_jt_terminal SET terminal_id = COALESCE($1, terminal_id), plate_no = COALESCE($2, plate_no), plate_color = COALESCE($3, plate_color), maker_id = COALESCE($4, maker_id), model = COALESCE($5, model), media_server_id = COALESCE($6, media_server_id), update_time = $7 WHERE phone_number = $8",
    ).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(phone_number)
    .execute(pool).await?;
    Ok(r.rows_affected())
}

pub async fn delete_terminal_by_phone(pool: &Pool, phone_number: &str) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query("DELETE FROM gb_jt_terminal WHERE phone_number = ?").bind(phone_number).execute(pool).await.map(|r| r.rows_affected());
    #[cfg(feature = "postgres")]
    return sqlx::query("DELETE FROM gb_jt_terminal WHERE phone_number = $1").bind(phone_number).execute(pool).await.map(|r| r.rows_affected());
}

/// 更新终端在线状态
pub async fn update_terminal_status(pool: &Pool, phone_number: &str, status: bool) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("UPDATE gb_jt_terminal SET status = ?, update_time = ? WHERE phone_number = ?")
        .bind(status)
        .bind(&now)
        .bind(phone_number)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_jt_terminal SET status = $1, update_time = $2 WHERE phone_number = $3")
        .bind(status)
        .bind(&now)
        .bind(phone_number)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 删除终端通道
pub async fn delete_channel(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("DELETE FROM gb_jt_channel WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_jt_channel WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 删除终端的所有通道
pub async fn delete_channels_by_terminal(pool: &Pool, terminal_db_id: i32) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("DELETE FROM gb_jt_channel WHERE terminal_db_id = ?")
        .bind(terminal_db_id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_jt_channel WHERE terminal_db_id = $1")
        .bind(terminal_db_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 更新终端位置信息
pub async fn update_terminal_position(
    pool: &Pool,
    phone_number: &str,
    longitude: f64,
    latitude: f64,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("UPDATE gb_jt_terminal SET longitude = ?, latitude = ?, update_time = ? WHERE phone_number = ?")
        .bind(longitude)
        .bind(latitude)
        .bind(&now)
        .bind(phone_number)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_jt_terminal SET longitude = $1, latitude = $2, update_time = $3 WHERE phone_number = $4")
        .bind(longitude)
        .bind(latitude)
        .bind(&now)
        .bind(phone_number)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// Count channels for a terminal by terminal DB id
pub async fn count_channels_by_terminal(pool: &Pool, terminal_db_id: i32) -> sqlx::Result<i64> {
    #[cfg(feature = "postgres")]
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_jt_channel WHERE terminal_db_id = $1"
    )
    .bind(terminal_db_id)
    .fetch_one(pool)
    .await?;
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gb_jt_channel WHERE terminal_db_id = ?"
    )
    .bind(terminal_db_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

// =================== Phase 6.1: Terminal authentication code lookup ===================

/// Look up the auth_code assigned to a terminal by phone number.
/// Returns None if terminal not found OR if auth_code column is NULL.
pub async fn get_auth_code_by_phone(pool: &Pool, phone: &str) -> sqlx::Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT auth_code FROM gb_jt_terminal WHERE phone_number = ?")
        .bind(phone)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|r| r.0))
}

/// Update the auth_code for a terminal (admin operation).
pub async fn update_auth_code(pool: &Pool, phone: &str, auth_code: &str) -> sqlx::Result<u64> {
    let result = sqlx::query("UPDATE gb_jt_terminal SET auth_code = ?, update_time = ? WHERE phone_number = ?")
        .bind(auth_code)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(phone)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Phase 6.5: Update last reported position (longitude, latitude, time).
pub async fn update_last_position(
    pool: &Pool,
    phone: &str,
    longitude: f64,
    latitude: f64,
    time: chrono::DateTime<chrono::Utc>,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE gb_jt_terminal SET longitude = ?, latitude = ?, register_time = ?, update_time = ? WHERE phone_number = ?"
    )
    .bind(longitude)
    .bind(latitude)
    .bind(time.to_rfc3339())
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(phone)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

// =================== Phase 6.4: Media item persistence ===================

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JtMediaItem {
    pub id: i32,
    pub phone_number: String,
    pub channel_id: i32,
    pub media_id: i64,
    pub media_type: Option<i32>,
    pub media_format: Option<i32>,
    pub event_code: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub file_path: Option<String>,
    pub create_time: String,
}

/// Insert a JT/T 1078 media item (returned from 0x8802 media search).
pub async fn insert_media_item(
    pool: &Pool,
    phone: &str,
    channel_id: i32,
    media_id: u32,
    media_type: i32,
    media_format: i32,
    event_code: i32,
    start_time: &str,
    end_time: &str,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "INSERT INTO gb_jt_media_item (phone_number, channel_id, media_id, media_type, media_format, event_code, start_time, end_time, create_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(phone)
    .bind(channel_id)
    .bind(media_id as i64)
    .bind(media_type)
    .bind(media_format)
    .bind(event_code)
    .bind(start_time)
    .bind(end_time)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// List media items for a terminal within optional time range.
pub async fn list_media_items_by_terminal(
    pool: &Pool,
    phone: &str,
    start_time: Option<&str>,
    end_time: Option<&str>,
    limit: i32,
) -> sqlx::Result<Vec<JtMediaItem>> {
    let limit_64 = limit as i64;
    let rows: Vec<JtMediaItem> = match (start_time, end_time) {
        (Some(s), Some(e)) => {
            sqlx::query_as::<_, JtMediaItem>(
                "SELECT * FROM gb_jt_media_item WHERE phone_number = ? AND start_time >= ? AND end_time <= ? ORDER BY start_time DESC LIMIT ?"
            )
            .bind(phone).bind(s).bind(e).bind(limit_64)
            .fetch_all(pool).await?
        }
        _ => {
            sqlx::query_as::<_, JtMediaItem>(
                "SELECT * FROM gb_jt_media_item WHERE phone_number = ? ORDER BY start_time DESC LIMIT ?"
            )
            .bind(phone).bind(limit_64)
            .fetch_all(pool).await?
        }
    };
    Ok(rows)
}

// ============================================================================
// Phase 6.5: 区域/路线 持久化（GBServer 扩展，JT/T 808/1078 围栏管理）
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct JtAreaCircle {
    pub id: i64,
    pub phone_number: String,
    pub label: Option<String>,
    pub center_lat: f64,
    pub center_lon: f64,
    pub radius_m: i32,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct JtAreaPolygon {
    pub id: i64,
    pub phone_number: String,
    pub label: Option<String>,
    pub points_json: String,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct JtAreaRectangle {
    pub id: i64,
    pub phone_number: String,
    pub label: Option<String>,
    pub left_top_lat: f64,
    pub left_top_lon: f64,
    pub right_bottom_lat: f64,
    pub right_bottom_lon: f64,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct JtRoute {
    pub id: i64,
    pub phone_number: String,
    pub label: Option<String>,
    pub waypoints_json: String,
    pub create_time: String,
    pub update_time: String,
}

pub async fn insert_area_circle(
    pool: &Pool,
    phone_number: &str,
    label: Option<&str>,
    center_lat: f64,
    center_lon: f64,
    radius_m: i32,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO gb_jt_area_circle (phone_number, label, center_lat, center_lon, radius_m, create_time, update_time)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id"
    )
    .bind(phone_number).bind(label).bind(center_lat).bind(center_lon)
    .bind(radius_m).bind(&now).bind(&now)
    .fetch_one(pool).await?;
    Ok(row.0)
}

pub async fn update_area_circle(
    pool: &Pool, id: i64,
    label: Option<&str>, center_lat: f64, center_lon: f64, radius_m: i32,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let n = sqlx::query(
        "UPDATE gb_jt_area_circle SET label = ?, center_lat = ?, center_lon = ?, radius_m = ?, update_time = ? WHERE id = ?"
    )
    .bind(label).bind(center_lat).bind(center_lon).bind(radius_m).bind(&now).bind(id)
    .execute(pool).await?
    .rows_affected();
    Ok(n)
}

pub async fn delete_area_circle(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query("DELETE FROM gb_jt_area_circle WHERE id = ?")
        .bind(id)
        .execute(pool).await?
        .rows_affected();
    Ok(n)
}

pub async fn list_area_circles_by_phone(
    pool: &Pool, phone_number: &str,
) -> sqlx::Result<Vec<JtAreaCircle>> {
    let rows = sqlx::query_as::<_, JtAreaCircle>(
        "SELECT * FROM gb_jt_area_circle WHERE phone_number = ? ORDER BY id DESC"
    )
    .bind(phone_number)
    .fetch_all(pool).await?;
    Ok(rows)
}

pub async fn insert_area_polygon(
    pool: &Pool, phone_number: &str, label: Option<&str>, points_json: &str,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO gb_jt_area_polygon (phone_number, label, points_json, create_time, update_time)
         VALUES (?, ?, ?, ?, ?) RETURNING id"
    )
    .bind(phone_number).bind(label).bind(points_json).bind(&now).bind(&now)
    .fetch_one(pool).await?;
    Ok(row.0)
}

pub async fn delete_area_polygon(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query("DELETE FROM gb_jt_area_polygon WHERE id = ?")
        .bind(id)
        .execute(pool).await?
        .rows_affected();
    Ok(n)
}

pub async fn list_area_polygons_by_phone(
    pool: &Pool, phone_number: &str,
) -> sqlx::Result<Vec<JtAreaPolygon>> {
    let rows = sqlx::query_as::<_, JtAreaPolygon>(
        "SELECT * FROM gb_jt_area_polygon WHERE phone_number = ? ORDER BY id DESC"
    )
    .bind(phone_number)
    .fetch_all(pool).await?;
    Ok(rows)
}

pub async fn insert_area_rectangle(
    pool: &Pool, phone_number: &str, label: Option<&str>,
    lt_lat: f64, lt_lon: f64, rb_lat: f64, rb_lon: f64,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO gb_jt_area_rectangle (phone_number, label, left_top_lat, left_top_lon, right_bottom_lat, right_bottom_lon, create_time, update_time)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
    )
    .bind(phone_number).bind(label).bind(lt_lat).bind(lt_lon).bind(rb_lat).bind(rb_lon)
    .bind(&now).bind(&now)
    .fetch_one(pool).await?;
    Ok(row.0)
}

pub async fn update_area_rectangle(
    pool: &Pool, id: i64, label: Option<&str>,
    lt_lat: f64, lt_lon: f64, rb_lat: f64, rb_lon: f64,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let n = sqlx::query(
        "UPDATE gb_jt_area_rectangle SET label = ?, left_top_lat = ?, left_top_lon = ?, right_bottom_lat = ?, right_bottom_lon = ?, update_time = ? WHERE id = ?"
    )
    .bind(label).bind(lt_lat).bind(lt_lon).bind(rb_lat).bind(rb_lon).bind(&now).bind(id)
    .execute(pool).await?
    .rows_affected();
    Ok(n)
}

pub async fn delete_area_rectangle(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query("DELETE FROM gb_jt_area_rectangle WHERE id = ?")
        .bind(id)
        .execute(pool).await?
        .rows_affected();
    Ok(n)
}

pub async fn list_area_rectangles_by_phone(
    pool: &Pool, phone_number: &str,
) -> sqlx::Result<Vec<JtAreaRectangle>> {
    let rows = sqlx::query_as::<_, JtAreaRectangle>(
        "SELECT * FROM gb_jt_area_rectangle WHERE phone_number = ? ORDER BY id DESC"
    )
    .bind(phone_number)
    .fetch_all(pool).await?;
    Ok(rows)
}

pub async fn insert_route(
    pool: &Pool, phone_number: &str, label: Option<&str>, waypoints_json: &str,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO gb_jt_route (phone_number, label, waypoints_json, create_time, update_time)
         VALUES (?, ?, ?, ?, ?) RETURNING id"
    )
    .bind(phone_number).bind(label).bind(waypoints_json).bind(&now).bind(&now)
    .fetch_one(pool).await?;
    Ok(row.0)
}

pub async fn delete_route(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query("DELETE FROM gb_jt_route WHERE id = ?")
        .bind(id)
        .execute(pool).await?
        .rows_affected();
    Ok(n)
}

pub async fn list_routes_by_phone(
    pool: &Pool, phone_number: &str,
) -> sqlx::Result<Vec<JtRoute>> {
    let rows = sqlx::query_as::<_, JtRoute>(
        "SELECT * FROM gb_jt_route WHERE phone_number = ? ORDER BY id DESC"
    )
    .bind(phone_number)
    .fetch_all(pool).await?;
    Ok(rows)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        use std::time::Duration;

        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .unwrap();

        // 初始化本测试关心的 4 张表（不依赖 init-sqlite 的整 schema）
        for sql in [
            "CREATE TABLE gb_jt_area_circle (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, phone_number TEXT NOT NULL, label TEXT, \
                center_lat REAL NOT NULL, center_lon REAL NOT NULL, radius_m INTEGER NOT NULL, \
                create_time TEXT NOT NULL, update_time TEXT NOT NULL)",
            "CREATE TABLE gb_jt_area_polygon (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, phone_number TEXT NOT NULL, label TEXT, \
                points_json TEXT NOT NULL, create_time TEXT NOT NULL, update_time TEXT NOT NULL)",
            "CREATE TABLE gb_jt_area_rectangle (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, phone_number TEXT NOT NULL, label TEXT, \
                left_top_lat REAL NOT NULL, left_top_lon REAL NOT NULL, \
                right_bottom_lat REAL NOT NULL, right_bottom_lon REAL NOT NULL, \
                create_time TEXT NOT NULL, update_time TEXT NOT NULL)",
            "CREATE TABLE gb_jt_route (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, phone_number TEXT NOT NULL, label TEXT, \
                waypoints_json TEXT NOT NULL, create_time TEXT NOT NULL, update_time TEXT NOT NULL)",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn area_circle_crud_roundtrip() {
        let pool = make_pool().await;
        let id = insert_area_circle(&pool, "13800000001", Some("工厂围栏"), 31.5, 121.4, 500).await.unwrap();
        assert!(id > 0);

        let items = list_area_circles_by_phone(&pool, "13800000001").await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].center_lat, 31.5);
        assert_eq!(items[0].center_lon, 121.4);
        assert_eq!(items[0].radius_m, 500);
        assert_eq!(items[0].label.as_deref(), Some("工厂围栏"));

        // update
        let n = update_area_circle(&pool, id, Some("新围栏"), 31.6, 121.5, 800).await.unwrap();
        assert_eq!(n, 1);
        let items = list_area_circles_by_phone(&pool, "13800000001").await.unwrap();
        assert_eq!(items[0].label.as_deref(), Some("新围栏"));
        assert_eq!(items[0].radius_m, 800);

        // delete
        let n = delete_area_circle(&pool, id).await.unwrap();
        assert_eq!(n, 1);
        let items = list_area_circles_by_phone(&pool, "13800000001").await.unwrap();
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn area_polygon_crud_roundtrip() {
        let pool = make_pool().await;
        let pts = r#"[{"lat":31.5,"lon":121.4},{"lat":31.6,"lon":121.4}]"#;
        let id = insert_area_polygon(&pool, "13800000002", Some("poly1"), pts).await.unwrap();
        assert!(id > 0);

        let items = list_area_polygons_by_phone(&pool, "13800000002").await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].points_json.contains("31.5"));
        assert_eq!(items[0].label.as_deref(), Some("poly1"));

        assert_eq!(delete_area_polygon(&pool, id).await.unwrap(), 1);
        assert_eq!(list_area_polygons_by_phone(&pool, "13800000002").await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn area_rectangle_crud_roundtrip() {
        let pool = make_pool().await;
        let id = insert_area_rectangle(&pool, "13800000003", Some("rect1"), 31.5, 121.0, 31.0, 121.5)
            .await.unwrap();
        let items = list_area_rectangles_by_phone(&pool, "13800000003").await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].left_top_lat, 31.5);
        assert_eq!(items[0].right_bottom_lon, 121.5);

        let n = update_area_rectangle(&pool, id, Some("rect1-updated"), 31.6, 121.1, 31.1, 121.6)
            .await.unwrap();
        assert_eq!(n, 1);
        let items = list_area_rectangles_by_phone(&pool, "13800000003").await.unwrap();
        assert_eq!(items[0].label.as_deref(), Some("rect1-updated"));

        assert_eq!(delete_area_rectangle(&pool, id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn route_crud_roundtrip() {
        let pool = make_pool().await;
        let wps = r#"[{"lat":31.5,"lon":121.4},{"lat":31.6,"lon":121.5}]"#;
        let id = insert_route(&pool, "13800000004", Some("线路A"), wps).await.unwrap();
        let items = list_routes_by_phone(&pool, "13800000004").await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].waypoints_json.contains("31.5"));
        assert_eq!(items[0].label.as_deref(), Some("线路A"));

        assert_eq!(delete_route(&pool, id).await.unwrap(), 1);
        assert_eq!(list_routes_by_phone(&pool, "13800000004").await.unwrap().len(), 0);
    }

    /// 验证不同 phone_number 的围栏互不干扰（边界）
    #[tokio::test]
    async fn areas_are_scoped_by_phone() {
        let pool = make_pool().await;
        insert_area_circle(&pool, "13800000010", None, 1.0, 1.0, 100).await.unwrap();
        insert_area_circle(&pool, "13800000011", None, 2.0, 2.0, 200).await.unwrap();

        assert_eq!(list_area_circles_by_phone(&pool, "13800000010").await.unwrap().len(), 1);
        assert_eq!(list_area_circles_by_phone(&pool, "13800000011").await.unwrap().len(), 1);
        assert_eq!(list_area_circles_by_phone(&pool, "13800000099").await.unwrap().len(), 0);
    }
}
