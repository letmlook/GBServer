use crate::db::Pool;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JtTerminal {
    pub id: i32,
    pub phone_number: String,
    pub terminal_id: Option<String>,
    /// 库里这两列是 **TEXT**（行政区划码按字符串存）。早期结构体声明成 i32:
    /// NULL 还能解码，一旦真写进值（TEXT 存储类）整行解码就失败 ——
    /// `/terminal/list` 会直接 500。改回字符串与库一致。
    pub province_id: Option<String>,
    pub province_text: Option<String>,
    pub city_id: Option<String>,
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
            "SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else if has_query {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) ORDER BY id LIMIT ? OFFSET ?"
        } else if online.is_some() {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal ORDER BY id LIMIT ? OFFSET ?"
        };
        let rows = if has_query && online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else if has_query {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(&like)
                .bind(&like)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else if online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        };
        Ok(rows)
    }

    #[cfg(feature = "postgres")]
    {
        let sql = if has_query && online.is_some() {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) AND status = $3 ORDER BY id LIMIT $4 OFFSET $5"
        } else if has_query {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE (phone_number LIKE $1 OR plate_no LIKE $2) ORDER BY id LIMIT $3 OFFSET $4"
        } else if online.is_some() {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE status = $1 ORDER BY id LIMIT $2 OFFSET $3"
        } else {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal ORDER BY id LIMIT $1 OFFSET $2"
        };
        let rows = if has_query && online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else if has_query {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(&like)
                .bind(&like)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else if online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        };
        Ok(rows)
    }

    #[cfg(feature = "sqlite")]
    {
        let sql = if has_query && online.is_some() {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) AND status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else if has_query {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE (phone_number LIKE ? OR plate_no LIKE ?) ORDER BY id LIMIT ? OFFSET ?"
        } else if online.is_some() {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE status = ? ORDER BY id LIMIT ? OFFSET ?"
        } else {
            "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal ORDER BY id LIMIT ? OFFSET ?"
        };
        let rows = if has_query && online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(&like)
                .bind(&like)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else if has_query {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(&like)
                .bind(&like)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else if online.is_some() {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(online.unwrap())
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, JtTerminal>(sql)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
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
                .bind(online.unwrap())
                .fetch_one(pool)
                .await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal")
                .fetch_one(pool)
                .await
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
                .bind(online.unwrap())
                .fetch_one(pool)
                .await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal")
                .fetch_one(pool)
                .await
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
                .bind(online.unwrap())
                .fetch_one(pool)
                .await
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal")
                .fetch_one(pool)
                .await
        }
    }
}

pub async fn get_terminal_by_phone(pool: &Pool, phone: &str) -> sqlx::Result<Option<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>(
        "SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, \
         province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, \
         plate_color, plate_no, longitude, latitude, status, register_time, update_time, \
         create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code \
         FROM gb_jt_terminal WHERE phone_number = ? ORDER BY id LIMIT 1",
    )
    .bind(phone)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>(
        "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, \
         province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, \
         plate_color, plate_no, longitude, latitude, status, register_time, update_time, \
         create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code \
         FROM gb_jt_terminal WHERE phone_number = $1 ORDER BY id LIMIT 1",
    )
    .bind(phone)
    .fetch_optional(pool)
    .await;
}

/// 根据终端 ID（JT/T 808 终端号，非手机号）查询终端。
///
/// `/api/jt1078/terminal/query?deviceId=` 的历史语义既可能是手机号也可能是
/// 终端号，只按手机号查会静默返回 null。
pub async fn get_terminal_by_terminal_id(
    pool: &Pool,
    terminal_id: &str,
) -> sqlx::Result<Option<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>(
        "SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, \
         province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, \
         plate_color, plate_no, longitude, latitude, status, register_time, update_time, \
         create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code \
         FROM gb_jt_terminal WHERE terminal_id = ?",
    )
    .bind(terminal_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>(
        "SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, \
         province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, \
         plate_color, plate_no, longitude, latitude, status, register_time, update_time, \
         create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code \
         FROM gb_jt_terminal WHERE terminal_id = $1",
    )
    .bind(terminal_id)
    .fetch_optional(pool)
    .await;
}

/// 根据ID查询终端
pub async fn get_terminal_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>("SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE id = ?")
        .bind(id).fetch_optional(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>("SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE id = $1")
        .bind(id).fetch_optional(pool).await;
}

/// 根据ID查询通道
pub async fn get_channel_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<JtChannel>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM gb_jt_channel WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtChannel>("SELECT * FROM gb_jt_channel WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await;
}

/// 获取所有在线终端
pub async fn get_online_terminals(pool: &Pool) -> sqlx::Result<Vec<JtTerminal>> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_as::<_, JtTerminal>("SELECT id, phone_number, terminal_id, CAST(province_id AS CHAR) AS province_id, province_text, CAST(city_id AS CHAR) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE status = 1 ORDER BY id")
        .fetch_all(pool).await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtTerminal>("SELECT id, phone_number, terminal_id, CAST(province_id AS TEXT) AS province_id, province_text, CAST(city_id AS TEXT) AS city_id, city_text, maker_id, model, plate_color, plate_no, longitude, latitude, status, register_time, update_time, create_time, geo_coord_sys, media_server_id, sdp_ip, auth_code FROM gb_jt_terminal WHERE status = true ORDER BY id")
        .fetch_all(pool).await;
}

/// 统计在线终端数量
pub async fn count_online_terminals(pool: &Pool) -> sqlx::Result<i64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = 1")
        .fetch_one(pool)
        .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_terminal WHERE status = true")
        .fetch_one(pool)
        .await;
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
    return sqlx::query_as::<_, JtChannel>(
        "SELECT * FROM gb_jt_channel WHERE terminal_db_id = ? ORDER BY channel_id",
    )
    .bind(terminal_db_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, JtChannel>(
        "SELECT * FROM gb_jt_channel WHERE terminal_db_id = $1 ORDER BY channel_id",
    )
    .bind(terminal_db_id)
    .fetch_all(pool)
    .await;
}

/// 终端的可写字段。
///
/// 之前是 8 个位置参数、且**没有** province_id/city_id/name 等列 ——
/// 前端编辑框里的省域/市域编码填了也进不来。
#[derive(Debug, Default, Clone)]
pub struct JtTerminalWrite<'a> {
    pub terminal_id: Option<&'a str>,
    pub plate_no: Option<&'a str>,
    pub plate_color: Option<i32>,
    pub maker_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub media_server_id: Option<&'a str>,
    pub province_id: Option<&'a str>,
    pub city_id: Option<&'a str>,
}

pub async fn insert_terminal(
    pool: &Pool,
    phone_number: &str,
    f: &JtTerminalWrite<'_>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query(
        "INSERT INTO gb_jt_terminal (phone_number, terminal_id, plate_no, plate_color, maker_id, model, media_server_id, province_id, city_id, status, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(phone_number)
    .bind(f.terminal_id)
    .bind(f.plate_no)
    .bind(f.plate_color)
    .bind(f.maker_id)
    .bind(f.model)
    .bind(f.media_server_id)
    .bind(f.province_id)
    .bind(f.city_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO gb_jt_terminal (phone_number, terminal_id, plate_no, plate_color, maker_id, model, media_server_id, province_id, city_id, status, create_time, update_time) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, false, $10, $11)",
    )
    .bind(phone_number)
    .bind(f.terminal_id)
    .bind(f.plate_no)
    .bind(f.plate_color)
    .bind(f.maker_id)
    .bind(f.model)
    .bind(f.media_server_id)
    .bind(f.province_id)
    .bind(f.city_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn update_terminal(
    pool: &Pool,
    phone_number: &str,
    f: &JtTerminalWrite<'_>,
    now: &str,
) -> sqlx::Result<u64> {
    let set =
        "SET terminal_id = COALESCE({p1}, terminal_id), plate_no = COALESCE({p2}, plate_no), \
               plate_color = COALESCE({p3}, plate_color), maker_id = COALESCE({p4}, maker_id), \
               model = COALESCE({p5}, model), media_server_id = COALESCE({p6}, media_server_id), \
               province_id = COALESCE({p7}, province_id), city_id = COALESCE({p8}, city_id), \
               update_time = {p9} WHERE phone_number = {p10}";

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = {
        let sql: String = (1..=10).fold(set.to_string(), |acc, i| {
            acc.replace(&format!("{{p{i}}}"), "?")
        });
        sqlx::query(&format!("UPDATE gb_jt_terminal {sql}"))
            .bind(f.terminal_id)
            .bind(f.plate_no)
            .bind(f.plate_color)
            .bind(f.maker_id)
            .bind(f.model)
            .bind(f.media_server_id)
            .bind(f.province_id)
            .bind(f.city_id)
            .bind(now)
            .bind(phone_number)
            .execute(pool)
            .await?
    };
    #[cfg(feature = "postgres")]
    let r = {
        let sql: String = (1..=10).fold(set.to_string(), |acc, i| {
            acc.replace(&format!("{{p{i}}}"), &format!("${i}"))
        });
        sqlx::query(&format!("UPDATE gb_jt_terminal {sql}"))
            .bind(f.terminal_id)
            .bind(f.plate_no)
            .bind(f.plate_color)
            .bind(f.maker_id)
            .bind(f.model)
            .bind(f.media_server_id)
            .bind(f.province_id)
            .bind(f.city_id)
            .bind(now)
            .bind(phone_number)
            .execute(pool)
            .await?
    };
    Ok(r.rows_affected())
}

pub async fn delete_terminal_by_phone(pool: &Pool, phone_number: &str) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query("DELETE FROM gb_jt_terminal WHERE phone_number = ?")
        .bind(phone_number)
        .execute(pool)
        .await
        .map(|r| r.rows_affected());
    #[cfg(feature = "postgres")]
    return sqlx::query("DELETE FROM gb_jt_terminal WHERE phone_number = $1")
        .bind(phone_number)
        .execute(pool)
        .await
        .map(|r| r.rows_affected());
}

/// 按数据库主键删除（前端终端列表传的就是主键）。
pub async fn delete_terminal_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    return sqlx::query("DELETE FROM gb_jt_terminal WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected());
    #[cfg(feature = "postgres")]
    return sqlx::query("DELETE FROM gb_jt_terminal WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected());
}

/// 更新终端在线状态
pub async fn update_terminal_status(
    pool: &Pool,
    phone_number: &str,
    status: bool,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r =
        sqlx::query("UPDATE gb_jt_terminal SET status = ?, update_time = ? WHERE phone_number = ?")
            .bind(status)
            .bind(&now)
            .bind(phone_number)
            .execute(pool)
            .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_jt_terminal SET status = $1, update_time = $2 WHERE phone_number = $3",
    )
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
        "SELECT COUNT(*) FROM gb_jt_channel WHERE terminal_db_id = $1",
    )
    .bind(terminal_db_id)
    .fetch_one(pool)
    .await?;
    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_jt_channel WHERE terminal_db_id = ?")
            .bind(terminal_db_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

// =================== Phase 6.1: Terminal authentication code lookup ===================

/// Look up the auth_code assigned to a terminal by phone number.
/// Returns None if terminal not found OR if auth_code column is NULL.
pub async fn get_auth_code_by_phone(pool: &Pool, phone: &str) -> sqlx::Result<Option<String>> {
    // `LIMIT 1` + `ORDER BY`：`phone_number` 在三方言里都**没有**唯一约束
    // （唯一索引是 `(id, phone_number)`，而 id 是主键），同一 phone 出现重复行时
    // **取到哪一行是不确定的** —— 三个方言都不保证返回顺序，postgres 返回的是
    // 物理扫描顺序的第一行，而心跳每次都会重写这些行（行版本搬移）→ **取到的行会翻转**。
    //
    // 若恰好取到一条 `auth_code` 为空的行，`stored_auth_code` 就返回 None →
    // `issue_auth_code` 生成随机码 → 而 `update_auth_code` 的 `WHERE phone_number = ?`
    // 会**更新所有**重复行 → 运维配好的码被静默抹掉，终端随后全部鉴权失败。
    //
    // 所以这里**优先取真配了码的那条**（`auth_code` 非空），再按 id 取最早的 ——
    // 重复行是历史脏数据，不该让鉴权彻底失效，也不该让它变得不确定。
    let row: Option<(Option<String>,)> = sqlx::query_as(&crate::dyn_where::dialect_sql(
        "SELECT auth_code FROM gb_jt_terminal \
         WHERE phone_number = ? AND auth_code IS NOT NULL AND auth_code <> '' \
         ORDER BY id LIMIT 1",
    ))
    .bind(phone)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| r.0))
}

/// Update the auth_code for a terminal (admin operation).
pub async fn update_auth_code(pool: &Pool, phone: &str, auth_code: &str) -> sqlx::Result<u64> {
    let result = sqlx::query(&crate::dyn_where::dialect_sql(
        "UPDATE gb_jt_terminal SET auth_code = ?, update_time = ? WHERE phone_number = ?",
    ))
    .bind(auth_code)
    .bind(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(phone)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// 解析 `gb_jt_terminal.update_time`，返回 **epoch 秒**。
///
/// **必须兼容历史遗留的两种旧格式** —— 这一列历史上被写过三种形状：
/// 1. `%Y-%m-%d %H:%M:%S`（**UTC** 墙钟，当前写入用的就是它）；
/// 2. **RFC3339**（`2026-10-24T04:04:11.123+00:00`，旧版 `update_auth_code` /
///    `update_last_position` 写的）；
/// 3. `%Y-%m-%d %H:%M:%S` 但基准是**本地时间**（旧版 `manager.rs` 用的
///    `chrono::Local::now()`）。
///
/// 1 与 3 **格式相同、基准不同**，无法从字符串区分 —— 本函数按 UTC 解释（与当前
/// 写入一致）。那一批历史行会被读成"未来 8 小时"，因而永远判不出过期；
/// 彻底订正需要一次数据订正（见 `docs/OPEN_ISSUES.md` 的 B21）。
/// 2 则是**确定性**可还原的，这里直接兼容。
///
/// 为什么要在 **Rust 里**解析、而不是在 SQL 里比字符串：
/// RFC3339 的第 10 位是 `'T'`（0x54），纯格式的第 10 位是 `' '`（0x20）——
/// 同一个日历日内，RFC3339 的值**恒大于**纯格式的值。于是"这行比 deadline 新"
/// 这个判断对历史行**恒成立**，条件写等于没写 → 那些终端会**永远显示在线**。
fn parse_update_time(raw: &str) -> Option<i64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    // ① 纯格式（UTC 墙钟）
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp());
    }
    // ② RFC3339（带偏移或 Z）
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Some(dt.timestamp());
    }
    // ③ 再试一次带毫秒的纯格式（部分路径写过 "%Y-%m-%d %H:%M:%S%.3f"）
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc().timestamp());
    }
    None
}

/// **条件**置离线：只有当该终端**所有**行在 `deadline` 之后**都没有**被更新过，
/// 才置 `status = 0`。
///
/// 用于"会话超时 ⇒ 终端离线"这条语义。清理分两段（锁内摘会话、锁外写库），
/// 两段之间存在窗口：终端若在这期间发过心跳，`persist_terminal_online` 会把
/// `update_time` 刷新成更新的值 —— 此时**不能**再把它标成离线，否则活着的终端
/// 在库里被显示成"离线"，而平台其实仍能给它下发命令。
///
/// 返回被置为离线的行数（0 = 期间这行被更新过，没动它）。
///
/// `deadline` 是**会话被摘掉的那一刻**（UTC 墙钟纯格式）。
/// 比较用**严格小于**：`update_time` 是秒级精度，同一秒内到达的心跳其
/// `update_time` 会**等于** deadline —— 用 `<=` 会把活着的终端误置离线。
///
/// **为什么在 Rust 里比较、而不是交给 SQL 比字符串**：
/// - `update_time` 这一列**历史上被写过三种格式**（见 [`parse_update_time`]），
///   其中 RFC3339 的第 10 位是 `'T'`，**恒大于**纯格式的 `' '` → 用 SQL 字符串
///   比较，历史行会被判成"比 deadline 新"，条件恒成立 → 那些终端**永远显示在线**。
/// - `phone_number` **没有唯一约束**，同一 phone 可能有多行；逐行判定会出现
///   "一半置离线、一半仍在线"的**半离线**状态（在线统计仍把死终端算在线）。
///   故这里取全部行、逐行解析，**全部都过期才置离线**（保守方向：宁可晚点离线，
///   也不能把活着的终端标成离线）。
pub async fn mark_terminal_offline_if_stale(
    pool: &Pool,
    phone_number: &str,
    deadline: &str,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let deadline_ts = parse_update_time(deadline).unwrap_or(0);

    // 取该 phone 的**所有**行（phone_number 无唯一约束，可能有重复行）
    let rows: Vec<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, update_time FROM gb_jt_terminal WHERE phone_number = ?")
            .bind(phone_number)
            .fetch_all(pool)
            .await?;
    if rows.is_empty() {
        return Ok(0);
    }

    // 逐行解析：只要有**任何一行**比 deadline 新（= 这期间有活动），就整体不动。
    //
    // **解析不出来**（历史遗留的未知格式 / 被写坏的值）时按「**还活跃**」处理 ——
    // 方向与整段设计一致：**宁可暂时不置离线，也不能把活着的终端误标成离线**。
    // 代价是那种行永远不会被自动置离线（属于数据问题，需要一次性订正）。
    let mut all_stale = true;
    for (_, ts) in &rows {
        let fresh = ts
            .as_deref()
            .and_then(parse_update_time)
            .map(|t| t >= deadline_ts)
            .unwrap_or(true);
        if fresh {
            all_stale = false;
            break;
        }
    }
    if !all_stale {
        return Ok(0);
    }

    // 全部过期 → 按 **id** 精确置离线（而不是按 phone 批量改）
    let mut affected = 0u64;
    for (id, _) in &rows {
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        let r = sqlx::query("UPDATE gb_jt_terminal SET status = ?, update_time = ? WHERE id = ?")
            .bind(false)
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await?;
        #[cfg(feature = "postgres")]
        let r =
            sqlx::query("UPDATE gb_jt_terminal SET status = $1, update_time = $2 WHERE id = $3")
                .bind(false)
                .bind(&now)
                .bind(id)
                .execute(pool)
                .await?;
        affected += r.rows_affected();
    }
    Ok(affected)
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
        &crate::dyn_where::dialect_sql("UPDATE gb_jt_terminal SET longitude = ?, latitude = ?, register_time = ?, update_time = ? WHERE phone_number = ?")
    )
    .bind(longitude)
    .bind(latitude)
    .bind(time.to_rfc3339())
    .bind(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
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
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let result = sqlx::query(
        &crate::dyn_where::dialect_sql("INSERT INTO gb_jt_media_item (phone_number, channel_id, media_id, media_type, media_format, event_code, start_time, end_time, create_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
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
                &crate::dyn_where::dialect_sql("SELECT * FROM gb_jt_media_item WHERE phone_number = ? AND start_time >= ? AND end_time <= ? ORDER BY start_time DESC LIMIT ?")
            )
            .bind(phone).bind(s).bind(e).bind(limit_64)
            .fetch_all(pool).await?
        }
        _ => {
            sqlx::query_as::<_, JtMediaItem>(
                &crate::dyn_where::dialect_sql("SELECT * FROM gb_jt_media_item WHERE phone_number = ? ORDER BY start_time DESC LIMIT ?")
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

/// 序列化成 **camelCase**：前端读的是 `phoneNumber`/
/// `centerLat`/`radiusM`/`pointsJson` 等，snake_case 会让表格整列空白。
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
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

/// 序列化成 **camelCase**：前端读的是 `phoneNumber`/
/// `centerLat`/`radiusM`/`pointsJson` 等，snake_case 会让表格整列空白。
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct JtAreaPolygon {
    pub id: i64,
    pub phone_number: String,
    pub label: Option<String>,
    pub points_json: String,
    pub create_time: String,
    pub update_time: String,
}

/// 序列化成 **camelCase**：前端读的是 `phoneNumber`/
/// `centerLat`/`radiusM`/`pointsJson` 等，snake_case 会让表格整列空白。
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
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

/// 序列化成 **camelCase**：前端读的是 `phoneNumber`/
/// `centerLat`/`radiusM`/`pointsJson` 等，snake_case 会让表格整列空白。
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
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
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = "INSERT INTO gb_jt_area_circle (phone_number, label, center_lat, center_lon, \
               radius_m, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?)";
    // `RETURNING` 只有 PostgreSQL / SQLite(≥3.35) 支持，**MySQL 不支持**
    // （会报 1064 语法错误）—— MySQL 走 `execute` + `last_insert_id()`。
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(sql)
            .bind(phone_number)
            .bind(label)
            .bind(center_lat)
            .bind(center_lon)
            .bind(radius_m)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        return Ok(r.last_insert_id() as i64);
    }
    #[cfg(not(feature = "mysql"))]
    {
        let with_returning = format!("{sql} RETURNING id");
        let row: (i64,) = sqlx::query_as(&crate::dyn_where::dialect_sql(&with_returning))
            .bind(phone_number)
            .bind(label)
            .bind(center_lat)
            .bind(center_lon)
            .bind(radius_m)
            .bind(&now)
            .bind(&now)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

pub async fn update_area_circle(
    pool: &Pool,
    id: i64,
    label: Option<&str>,
    center_lat: f64,
    center_lon: f64,
    radius_m: i32,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = sqlx::query(
        &crate::dyn_where::dialect_sql("UPDATE gb_jt_area_circle SET label = ?, center_lat = ?, center_lon = ?, radius_m = ?, update_time = ? WHERE id = ?")
    )
    .bind(label).bind(center_lat).bind(center_lon).bind(radius_m).bind(&now).bind(id)
    .execute(pool).await?
    .rows_affected();
    Ok(n)
}

pub async fn delete_area_circle(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query(&crate::dyn_where::dialect_sql(
        "DELETE FROM gb_jt_area_circle WHERE id = ?",
    ))
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

pub async fn list_area_circles_by_phone(
    pool: &Pool,
    phone_number: &str,
) -> sqlx::Result<Vec<JtAreaCircle>> {
    let rows = sqlx::query_as::<_, JtAreaCircle>(&crate::dyn_where::dialect_sql(
        "SELECT * FROM gb_jt_area_circle WHERE phone_number = ? ORDER BY id DESC",
    ))
    .bind(phone_number)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_area_polygon(
    pool: &Pool,
    phone_number: &str,
    label: Option<&str>,
    points_json: &str,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = "INSERT INTO gb_jt_area_polygon (phone_number, label, points_json, create_time, \
               update_time) VALUES (?, ?, ?, ?, ?)";
    // 同 `insert_area_circle`：MySQL 没有 `RETURNING`。
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(sql)
            .bind(phone_number)
            .bind(label)
            .bind(points_json)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        return Ok(r.last_insert_id() as i64);
    }
    #[cfg(not(feature = "mysql"))]
    {
        let with_returning = format!("{sql} RETURNING id");
        let row: (i64,) = sqlx::query_as(&crate::dyn_where::dialect_sql(&with_returning))
            .bind(phone_number)
            .bind(label)
            .bind(points_json)
            .bind(&now)
            .bind(&now)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

pub async fn delete_area_polygon(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query(&crate::dyn_where::dialect_sql(
        "DELETE FROM gb_jt_area_polygon WHERE id = ?",
    ))
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

pub async fn list_area_polygons_by_phone(
    pool: &Pool,
    phone_number: &str,
) -> sqlx::Result<Vec<JtAreaPolygon>> {
    let rows = sqlx::query_as::<_, JtAreaPolygon>(&crate::dyn_where::dialect_sql(
        "SELECT * FROM gb_jt_area_polygon WHERE phone_number = ? ORDER BY id DESC",
    ))
    .bind(phone_number)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_area_rectangle(
    pool: &Pool,
    phone_number: &str,
    label: Option<&str>,
    lt_lat: f64,
    lt_lon: f64,
    rb_lat: f64,
    rb_lon: f64,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = "INSERT INTO gb_jt_area_rectangle (phone_number, label, left_top_lat, left_top_lon, \
               right_bottom_lat, right_bottom_lon, create_time, update_time) \
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
    // 同 `insert_area_circle`：MySQL 没有 `RETURNING`。
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(sql)
            .bind(phone_number)
            .bind(label)
            .bind(lt_lat)
            .bind(lt_lon)
            .bind(rb_lat)
            .bind(rb_lon)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        return Ok(r.last_insert_id() as i64);
    }
    #[cfg(not(feature = "mysql"))]
    {
        let with_returning = format!("{sql} RETURNING id");
        let row: (i64,) = sqlx::query_as(&crate::dyn_where::dialect_sql(&with_returning))
            .bind(phone_number)
            .bind(label)
            .bind(lt_lat)
            .bind(lt_lon)
            .bind(rb_lat)
            .bind(rb_lon)
            .bind(&now)
            .bind(&now)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

pub async fn update_area_rectangle(
    pool: &Pool,
    id: i64,
    label: Option<&str>,
    lt_lat: f64,
    lt_lon: f64,
    rb_lat: f64,
    rb_lon: f64,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = sqlx::query(
        &crate::dyn_where::dialect_sql("UPDATE gb_jt_area_rectangle SET label = ?, left_top_lat = ?, left_top_lon = ?, right_bottom_lat = ?, right_bottom_lon = ?, update_time = ? WHERE id = ?")
    )
    .bind(label).bind(lt_lat).bind(lt_lon).bind(rb_lat).bind(rb_lon).bind(&now).bind(id)
    .execute(pool).await?
    .rows_affected();
    Ok(n)
}

pub async fn delete_area_rectangle(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query(&crate::dyn_where::dialect_sql(
        "DELETE FROM gb_jt_area_rectangle WHERE id = ?",
    ))
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

pub async fn list_area_rectangles_by_phone(
    pool: &Pool,
    phone_number: &str,
) -> sqlx::Result<Vec<JtAreaRectangle>> {
    let rows = sqlx::query_as::<_, JtAreaRectangle>(&crate::dyn_where::dialect_sql(
        "SELECT * FROM gb_jt_area_rectangle WHERE phone_number = ? ORDER BY id DESC",
    ))
    .bind(phone_number)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_route(
    pool: &Pool,
    phone_number: &str,
    label: Option<&str>,
    waypoints_json: &str,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = "INSERT INTO gb_jt_route (phone_number, label, waypoints_json, create_time, \
               update_time) VALUES (?, ?, ?, ?, ?)";
    // 同 `insert_area_circle`：MySQL 没有 `RETURNING`。
    #[cfg(feature = "mysql")]
    {
        let r = sqlx::query(sql)
            .bind(phone_number)
            .bind(label)
            .bind(waypoints_json)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        return Ok(r.last_insert_id() as i64);
    }
    #[cfg(not(feature = "mysql"))]
    {
        let with_returning = format!("{sql} RETURNING id");
        let row: (i64,) = sqlx::query_as(&crate::dyn_where::dialect_sql(&with_returning))
            .bind(phone_number)
            .bind(label)
            .bind(waypoints_json)
            .bind(&now)
            .bind(&now)
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

pub async fn delete_route(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let n = sqlx::query(&crate::dyn_where::dialect_sql(
        "DELETE FROM gb_jt_route WHERE id = ?",
    ))
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

pub async fn list_routes_by_phone(pool: &Pool, phone_number: &str) -> sqlx::Result<Vec<JtRoute>> {
    let rows = sqlx::query_as::<_, JtRoute>(&crate::dyn_where::dialect_sql(
        "SELECT * FROM gb_jt_route WHERE phone_number = ? ORDER BY id DESC",
    ))
    .bind(phone_number)
    .fetch_all(pool)
    .await?;
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
        let id = insert_area_circle(&pool, "13800000001", Some("工厂围栏"), 31.5, 121.4, 500)
            .await
            .unwrap();
        assert!(id > 0);

        let items = list_area_circles_by_phone(&pool, "13800000001")
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].center_lat, 31.5);
        assert_eq!(items[0].center_lon, 121.4);
        assert_eq!(items[0].radius_m, 500);
        assert_eq!(items[0].label.as_deref(), Some("工厂围栏"));

        // update
        let n = update_area_circle(&pool, id, Some("新围栏"), 31.6, 121.5, 800)
            .await
            .unwrap();
        assert_eq!(n, 1);
        let items = list_area_circles_by_phone(&pool, "13800000001")
            .await
            .unwrap();
        assert_eq!(items[0].label.as_deref(), Some("新围栏"));
        assert_eq!(items[0].radius_m, 800);

        // delete
        let n = delete_area_circle(&pool, id).await.unwrap();
        assert_eq!(n, 1);
        let items = list_area_circles_by_phone(&pool, "13800000001")
            .await
            .unwrap();
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn area_polygon_crud_roundtrip() {
        let pool = make_pool().await;
        let pts = r#"[{"lat":31.5,"lon":121.4},{"lat":31.6,"lon":121.4}]"#;
        let id = insert_area_polygon(&pool, "13800000002", Some("poly1"), pts)
            .await
            .unwrap();
        assert!(id > 0);

        let items = list_area_polygons_by_phone(&pool, "13800000002")
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].points_json.contains("31.5"));
        assert_eq!(items[0].label.as_deref(), Some("poly1"));

        assert_eq!(delete_area_polygon(&pool, id).await.unwrap(), 1);
        assert_eq!(
            list_area_polygons_by_phone(&pool, "13800000002")
                .await
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn area_rectangle_crud_roundtrip() {
        let pool = make_pool().await;
        let id = insert_area_rectangle(
            &pool,
            "13800000003",
            Some("rect1"),
            31.5,
            121.0,
            31.0,
            121.5,
        )
        .await
        .unwrap();
        let items = list_area_rectangles_by_phone(&pool, "13800000003")
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].left_top_lat, 31.5);
        assert_eq!(items[0].right_bottom_lon, 121.5);

        let n = update_area_rectangle(&pool, id, Some("rect1-updated"), 31.6, 121.1, 31.1, 121.6)
            .await
            .unwrap();
        assert_eq!(n, 1);
        let items = list_area_rectangles_by_phone(&pool, "13800000003")
            .await
            .unwrap();
        assert_eq!(items[0].label.as_deref(), Some("rect1-updated"));

        assert_eq!(delete_area_rectangle(&pool, id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn route_crud_roundtrip() {
        let pool = make_pool().await;
        let wps = r#"[{"lat":31.5,"lon":121.4},{"lat":31.6,"lon":121.5}]"#;
        let id = insert_route(&pool, "13800000004", Some("线路A"), wps)
            .await
            .unwrap();
        let items = list_routes_by_phone(&pool, "13800000004").await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].waypoints_json.contains("31.5"));
        assert_eq!(items[0].label.as_deref(), Some("线路A"));

        assert_eq!(delete_route(&pool, id).await.unwrap(), 1);
        assert_eq!(
            list_routes_by_phone(&pool, "13800000004")
                .await
                .unwrap()
                .len(),
            0
        );
    }

    /// 验证不同 phone_number 的围栏互不干扰（边界）
    #[tokio::test]
    async fn areas_are_scoped_by_phone() {
        let pool = make_pool().await;
        insert_area_circle(&pool, "13800000010", None, 1.0, 1.0, 100)
            .await
            .unwrap();
        insert_area_circle(&pool, "13800000011", None, 2.0, 2.0, 200)
            .await
            .unwrap();

        assert_eq!(
            list_area_circles_by_phone(&pool, "13800000010")
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_area_circles_by_phone(&pool, "13800000011")
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_area_circles_by_phone(&pool, "13800000099")
                .await
                .unwrap()
                .len(),
            0
        );
    }
}

/// 「会话超时 ⇒ 置离线」的条件写的专项测试。
#[cfg(test)]
mod offline_tests {
    use super::*;
    use chrono::Duration;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        use std::time::Duration as StdDuration;

        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true)
            .busy_timeout(StdDuration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE gb_jt_terminal (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, phone_number TEXT, status INTEGER, \
                update_time TEXT, create_time TEXT)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    async fn insert_row(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        phone: &str,
        status: i64,
        ts: &str,
    ) -> i64 {
        sqlx::query(
            "INSERT INTO gb_jt_terminal (phone_number, status, update_time, create_time) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(phone)
        .bind(status)
        .bind(ts)
        .bind(ts)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query_scalar::<_, i64>("SELECT MAX(id) FROM gb_jt_terminal WHERE phone_number = ?")
            .bind(phone)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn status_of(pool: &sqlx::Pool<sqlx::Sqlite>, id: i64) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT status FROM gb_jt_terminal WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn plain(ago: Duration) -> String {
        (chrono::Utc::now() - ago)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }

    /// 正常形态：**超时的终端**（最后一次上报早于 deadline）必须被置离线。
    #[tokio::test]
    async fn stale_terminal_is_marked_offline() {
        let pool = make_pool().await;
        // 生产超时是 60s，用同量级的时间差（此前测试回拨 2 分钟，绕开了真实形态）
        let id = insert_row(&pool, "13800000001", 1, &plain(Duration::seconds(60))).await;
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let n = mark_terminal_offline_if_stale(&pool, "13800000001", &deadline)
            .await
            .unwrap();
        assert_eq!(n, 1, "超时的终端应被置离线");
        assert_eq!(status_of(&pool, id).await, 0);
    }

    /// 刚活跃过的终端**不能**被置离线（同一秒也算活跃 → 严格小于）。
    #[tokio::test]
    async fn active_terminal_is_not_marked_offline() {
        let pool = make_pool().await;
        let id = insert_row(&pool, "13800000002", 1, &plain(Duration::seconds(0))).await;
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let n = mark_terminal_offline_if_stale(&pool, "13800000002", &deadline)
            .await
            .unwrap();
        assert_eq!(n, 0, "刚活跃过的终端不该被置离线");
        assert_eq!(status_of(&pool, id).await, 1);
    }

    /// 回归：**历史遗留的 RFC3339 格式**也必须能正确判过期。
    ///
    /// 这一列历史上被写过 RFC3339（`2026-10-24T04:04:11.123+00:00`）。用 SQL
    /// 直接比字符串时，同一日内 RFC3339 的第 10 位是 `'T'`（0x54）而纯格式是
    /// `' '`（0x20）→ 历史行**恒被判成"比 deadline 新"** → 条件恒成立 →
    /// 那些终端**永远显示在线**。现改为在 Rust 里解析成 epoch 再比。
    #[tokio::test]
    async fn legacy_rfc3339_timestamp_is_parsed_and_can_be_marked_offline() {
        let pool = make_pool().await;
        // 60 秒前的 RFC3339 时刻
        let rfc = (chrono::Utc::now() - Duration::seconds(60)).to_rfc3339();
        let id = insert_row(&pool, "13800000003", 1, &rfc).await;
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let n = mark_terminal_offline_if_stale(&pool, "13800000003", &deadline)
            .await
            .unwrap();
        assert_eq!(n, 1, "RFC3339 格式的历史行也必须能判出过期");
        assert_eq!(status_of(&pool, id).await, 0);
    }

    /// 回归：**重复行**（同一 phone 多行）不得出现"半离线"。
    ///
    /// `phone_number` 没有唯一约束。逐行判定会把"一行旧、一行新"的 phone 判成
    /// 过期（那一行被置离线），而新行仍在线 → 在线统计仍把死终端算在线。
    /// 现在的语义是：**全部行都过期才置离线**。
    #[tokio::test]
    async fn duplicate_rows_are_not_left_half_offline() {
        let pool = make_pool().await;
        let old_id = insert_row(&pool, "13800000004", 1, &plain(Duration::seconds(600))).await;
        let fresh_id = insert_row(&pool, "13800000004", 1, &plain(Duration::seconds(0))).await;
        assert_ne!(old_id, fresh_id, "应为两行");
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let n = mark_terminal_offline_if_stale(&pool, "13800000004", &deadline)
            .await
            .unwrap();
        assert_eq!(n, 0, "只要有一行还活跃，整个 phone 都不该被置离线");
        assert_eq!(status_of(&pool, old_id).await, 1, "旧行也不该被单独置离线");
        assert_eq!(status_of(&pool, fresh_id).await, 1);
    }

    /// 回归：重复行**全部**过期时，两行都要被置离线。
    #[tokio::test]
    async fn duplicate_rows_all_stale_are_all_marked_offline() {
        let pool = make_pool().await;
        let a = insert_row(&pool, "13800000005", 1, &plain(Duration::seconds(600))).await;
        let b = insert_row(&pool, "13800000005", 1, &plain(Duration::seconds(300))).await;
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let n = mark_terminal_offline_if_stale(&pool, "13800000005", &deadline)
            .await
            .unwrap();
        assert_eq!(n, 2, "全部过期的重复行都应被置离线");
        assert_eq!(status_of(&pool, a).await, 0);
        assert_eq!(status_of(&pool, b).await, 0);
    }

    /// 回归：**刚活跃的 RFC3339 历史行**不能被误置离线。
    ///
    /// 这是上面那条的**另一半**：解析不出来时按「还活跃」处理（安全方向），
    /// 所以一个刚活跃的 RFC3339 行必须**解析成功**才能正确判出"活跃" ——
    /// 若解析支被去掉，它会被当成"过期"而误置离线。
    #[tokio::test]
    async fn fresh_legacy_rfc3339_timestamp_is_not_marked_offline() {
        let pool = make_pool().await;
        let rfc = chrono::Utc::now().to_rfc3339();
        let id = insert_row(&pool, "13800000006", 1, &rfc).await;
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let n = mark_terminal_offline_if_stale(&pool, "13800000006", &deadline)
            .await
            .unwrap();
        assert_eq!(n, 0, "刚活跃的 RFC3339 行不该被置离线");
        assert_eq!(status_of(&pool, id).await, 1);
    }

    /// `parse_update_time` 的三种形状：纯格式（UTC 墙钟）、RFC3339、带毫秒的纯格式。
    #[test]
    fn update_time_parser_handles_legacy_formats() {
        let plain_ts = "2026-10-24 04:04:11";
        let parsed = parse_update_time(plain_ts).expect("纯格式应能解析");
        assert_eq!(
            parsed,
            chrono::NaiveDateTime::parse_from_str(plain_ts, "%Y-%m-%d %H:%M:%S")
                .unwrap()
                .and_utc()
                .timestamp()
        );

        let rfc_ts = "2026-10-24T04:04:11.123+00:00";
        assert_eq!(
            parse_update_time(rfc_ts),
            Some(
                chrono::DateTime::parse_from_rfc3339(rfc_ts)
                    .unwrap()
                    .timestamp()
            )
        );

        let millis = "2026-10-24 04:04:11.123";
        assert!(
            parse_update_time(millis).is_some(),
            "带毫秒的纯格式也应能解析"
        );

        assert!(parse_update_time("").is_none());
        assert!(parse_update_time("not-a-time").is_none());
    }
}
