//! 级联平台 wvp_platform

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 完整的平台信息结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Platform {
    pub id: i64,
    pub enable: Option<bool>,
    pub name: Option<String>,
    pub server_gb_id: Option<String>,
    pub server_gb_domain: Option<String>,
    pub server_ip: Option<String>,
    pub server_port: Option<i32>,
    pub device_gb_id: Option<String>,
    pub device_ip: Option<String>,
    pub device_port: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub expires: Option<String>,
    pub keep_timeout: Option<String>,
    pub transport: Option<String>,
    pub civil_code: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub address: Option<String>,
    pub character_set: Option<String>,
    pub ptz: Option<bool>,
    pub rtcp: Option<bool>,
    pub status: Option<bool>,
    pub catalog_group: Option<i32>,
    pub register_way: Option<i32>,
    pub secrecy: Option<i32>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub as_message_channel: Option<bool>,
    pub catalog_with_platform: Option<i32>,
    pub catalog_with_group: Option<i32>,
    pub catalog_with_region: Option<i32>,
    pub auto_push_channel: Option<bool>,
    pub send_stream_ip: Option<String>,
    pub server_id: Option<String>,
}

/// 分页列表查询
pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<Vec<Platform>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Platform>(
        "SELECT * FROM wvp_platform ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Platform>(
        "SELECT * FROM wvp_platform ORDER BY id LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset as i64)
    .fetch_all(pool)
    .await;
}

/// 统计总数
pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_platform")
        .fetch_one(pool)
        .await
}

/// 根据ID获取平台
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<Platform>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Platform>(
        "SELECT * FROM wvp_platform WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Platform>(
        "SELECT * FROM wvp_platform WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

/// 根据国标ID获取平台
pub async fn get_by_server_gb_id(pool: &Pool, server_gb_id: &str) -> sqlx::Result<Option<Platform>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, Platform>(
        "SELECT * FROM wvp_platform WHERE server_gb_id = ?",
    )
    .bind(server_gb_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, Platform>(
        "SELECT * FROM wvp_platform WHERE server_gb_id = $1",
    )
    .bind(server_gb_id)
    .fetch_optional(pool)
    .await;
}

/// 添加平台
pub async fn add(
    pool: &Pool,
    name: &str,
    server_gb_id: &str,
    server_ip: &str,
    server_port: i32,
    device_gb_id: &str,
    transport: &str,
    username: &str,
    password: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_platform (enable, name, server_gb_id, server_ip, server_port, device_gb_id, 
           transport, username, password, expires, keep_timeout, status, create_time, update_time, auto_push_channel)
           VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, '3600', '60', 0, ?, ?, true)"#,
    )
    .bind(name)
    .bind(server_gb_id)
    .bind(server_ip)
    .bind(server_port)
    .bind(device_gb_id)
    .bind(transport)
    .bind(username)
    .bind(password)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_platform (enable, name, server_gb_id, server_ip, server_port, device_gb_id, 
           transport, username, password, expires, keep_timeout, status, create_time, update_time, auto_push_channel)
           VALUES (true, $1, $2, $3, $4, $5, $6, $7, $8, '3600', '60', false, $9, $10, true)"#,
    )
    .bind(name)
    .bind(server_gb_id)
    .bind(server_ip)
    .bind(server_port)
    .bind(device_gb_id)
    .bind(transport)
    .bind(username)
    .bind(password)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 更新平台
pub async fn update(
    pool: &Pool,
    id: i64,
    name: Option<&str>,
    server_gb_id: Option<&str>,
    server_ip: Option<&str>,
    server_port: Option<i32>,
    device_gb_id: Option<&str>,
    transport: Option<&str>,
    username: Option<&str>,
    password: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE wvp_platform SET 
           name = COALESCE(?, name),
           server_gb_id = COALESCE(?, server_gb_id),
           server_ip = COALESCE(?, server_ip),
           server_port = COALESCE(?, server_port),
           device_gb_id = COALESCE(?, device_gb_id),
           transport = COALESCE(?, transport),
           username = COALESCE(?, username),
           password = COALESCE(?, password),
           update_time = ?
           WHERE id = ?"#,
    )
    .bind(name)
    .bind(server_gb_id)
    .bind(server_ip)
    .bind(server_port)
    .bind(device_gb_id)
    .bind(transport)
    .bind(username)
    .bind(password)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE wvp_platform SET 
           name = COALESCE($1, name),
           server_gb_id = COALESCE($2, server_gb_id),
           server_ip = COALESCE($3, server_ip),
           server_port = COALESCE($4, server_port),
           device_gb_id = COALESCE($5, device_gb_id),
           transport = COALESCE($6, transport),
           username = COALESCE($7, username),
           password = COALESCE($8, password),
           update_time = $9
           WHERE id = $10"#,
    )
    .bind(name)
    .bind(server_gb_id)
    .bind(server_ip)
    .bind(server_port)
    .bind(device_gb_id)
    .bind(transport)
    .bind(username)
    .bind(password)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 删除平台
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_platform WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_platform WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}
