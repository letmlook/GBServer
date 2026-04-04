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
            "SELECT * FROM wvp_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else if has_query {
            "SELECT * FROM wvp_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) ORDER BY id LIMIT ? OFFSET ?"
        } else if online.is_some() {
            "SELECT * FROM wvp_jt_terminal WHERE status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else {
            "SELECT * FROM wvp_jt_terminal ORDER BY id LIMIT ? OFFSET ?"
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
            "SELECT * FROM wvp_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) AND status = $3 ORDER BY id LIMIT $4 OFFSET $5"
        } else if has_query {
            "SELECT * FROM wvp_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) ORDER BY id LIMIT $3 OFFSET $4"
        } else if online.is_some() {
            "SELECT * FROM wvp_jt_terminal WHERE status = $1 ORDER BY id LIMIT $2 OFFSET $3"
        } else {
            "SELECT * FROM wvp_jt_terminal ORDER BY id LIMIT $1 OFFSET $2"
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
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ?")
                .bind(&like).bind(&like).bind(online.unwrap()).fetch_one(pool).await
        } else if has_query {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?)")
                .bind(&like).bind(&like).fetch_one(pool).await
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal WHERE status = ?")
                .bind(online.unwrap()).fetch_one(pool).await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal").fetch_one(pool).await
        }
    }

    #[cfg(feature = "postgres")]
    {
        if has_query && online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) AND status = $3")
                .bind(&like).bind(&like).bind(online.unwrap()).fetch_one(pool).await
        } else if has_query {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2)")
                .bind(&like).bind(&like).fetch_one(pool).await
        } else if online.is_some() {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal WHERE status = $1")
                .bind(online.unwrap()).fetch_one(pool).await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_jt_terminal").fetch_one(pool).await
        }
    }
}

pub async fn get_terminal_by_phone(pool: &Pool, phone: &str) -> sqlx::Result<Option<JtTerminal>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM wvp_jt_terminal WHERE phone_number = ?")
        .bind(phone).fetch_optional(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>("SELECT * FROM wvp_jt_terminal WHERE phone_number = $1")
        .bind(phone).fetch_optional(pool).await;
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
        #[cfg(feature = "mysql")]
        {
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let r = sqlx::query(
                "INSERT INTO wvp_jt_channel (terminal_db_id, channel_id, name, create_time, update_time) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(term.id)
            .bind(channel_id)
            .bind(name)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
            Ok(r.rows_affected())
        }
        #[cfg(feature = "postgres")]
        {
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let r = sqlx::query(
                "INSERT INTO wvp_jt_channel (terminal_db_id, channel_id, name, create_time, update_time) VALUES ($1, $2, $3, $4, $5)",
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
    #[cfg(feature = "mysql")]
    {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let r = sqlx::query(
            "UPDATE wvp_jt_channel SET name = COALESCE(?, name), channel_id = COALESCE(?, channel_id), update_time = ? WHERE id = ?",
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
            "UPDATE wvp_jt_channel SET name = COALESCE($1, name), channel_id = COALESCE($2, channel_id), update_time = $3 WHERE id = $4",
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
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM wvp_jt_channel WHERE terminal_db_id = ? ORDER BY channel_id")
        .bind(terminal_db_id).fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM wvp_jt_channel WHERE terminal_db_id = $1 ORDER BY channel_id")
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
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO wvp_jt_terminal (phone_number, terminal_id, plate_no, plate_color, maker_id, model, media_server_id, status, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    ).bind(phone_number).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(now)
    .execute(pool).await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO wvp_jt_terminal (phone_number, terminal_id, plate_no, plate_color, maker_id, model, media_server_id, status, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8, $9)",
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
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE wvp_jt_terminal SET terminal_id = COALESCE(?, terminal_id), plate_no = COALESCE(?, plate_no), plate_color = COALESCE(?, plate_color), maker_id = COALESCE(?, maker_id), model = COALESCE(?, model), media_server_id = COALESCE(?, media_server_id), update_time = ? WHERE phone_number = ?",
    ).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(phone_number)
    .execute(pool).await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE wvp_jt_terminal SET terminal_id = COALESCE($1, terminal_id), plate_no = COALESCE($2, plate_no), plate_color = COALESCE($3, plate_color), maker_id = COALESCE($4, maker_id), model = COALESCE($5, model), media_server_id = COALESCE($6, media_server_id), update_time = $7 WHERE phone_number = $8",
    ).bind(terminal_id).bind(plate_no).bind(plate_color).bind(maker_id).bind(model).bind(media_server_id).bind(now).bind(phone_number)
    .execute(pool).await?;
    Ok(r.rows_affected())
}

pub async fn delete_terminal_by_phone(pool: &Pool, phone_number: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    return sqlx::query("DELETE FROM wvp_jt_terminal WHERE phone_number = ?").bind(phone_number).execute(pool).await.map(|r| r.rows_affected());
    #[cfg(feature = "postgres")]
    return sqlx::query("DELETE FROM wvp_jt_terminal WHERE phone_number = $1").bind(phone_number).execute(pool).await.map(|r| r.rows_affected());
}
