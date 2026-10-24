//! 级联平台 gb_platform

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 完整的平台信息结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Platform {
    pub id: i32,
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

/// `get_all_online_platforms` 用的显式列清单。
///
/// 不能用 `SELECT *`：三个方言的列顺序/数量不一致（`SELECT *` 在结构体列
/// 清单漂移时不会报错，只在真正解码到行时炸 —— 见 `docs/DB_DIALECT_NOTES.md` 规则 2）。
const ONLINE_COLUMNS: &str = "id, enable, name, server_gb_id, server_gb_domain, server_ip, \
     server_port, device_gb_id, device_ip, device_port, username, password, expires, \
     keep_timeout, transport, character_set, ptz, rtcp, status, catalog_group, \
     auto_push_channel, send_stream_ip, create_time, update_time, civil_code, manufacturer, \
     model, address, register_way, secrecy, as_message_channel, catalog_with_platform, \
     catalog_with_group, catalog_with_region, server_id";

/// 分页列表查询
pub async fn list_paged(pool: &Pool, page: u32, count: u32) -> sqlx::Result<Vec<Platform>> {
    let limit = count.min(100);
    let offset = crate::pagination::offset(page, limit);
    let sql = pool.sql("SELECT * FROM gb_platform ORDER BY id LIMIT ? OFFSET ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql)
            .bind(limit as i64)
            .bind(offset)
            .fetch_all(p)
    })
}

/// 统计总数
pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    let sql = pool.sql("SELECT COUNT(*) FROM gb_platform");
    crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    })
}

/// 根据ID获取平台
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<Platform>> {
    let sql = pool.sql("SELECT * FROM gb_platform WHERE id = ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql)
            .bind(id)
            .fetch_optional(p)
    })
}

/// 根据国标ID获取平台
pub async fn get_by_server_gb_id(
    pool: &Pool,
    server_gb_id: &str,
) -> sqlx::Result<Option<Platform>> {
    let sql = pool.sql("SELECT * FROM gb_platform WHERE server_gb_id = ?");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql)
            .bind(server_gb_id)
            .fetch_optional(p)
    })
}

/// Get platform by device_gb_id (the device's GB ID within the platform context)
pub async fn get_by_device_gb_id(
    pool: &Pool,
    device_gb_id: &str,
) -> sqlx::Result<Option<Platform>> {
    let sql = pool.sql("SELECT * FROM gb_platform WHERE device_gb_id = ? LIMIT 1");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql)
            .bind(device_gb_id)
            .fetch_optional(p)
    })
}

/// 添加平台
///
/// **布尔列一律用参数绑定**（`enable` / `status` / `auto_push_channel`），
/// 不在 SQL 文本里写 `true` / `1`：
/// * PostgreSQL 的 `BOOL` 列只接受 `true` / `false`；
/// * MySQL 的 `BOOL` 就是 `TINYINT(1)`，SQLite 的布尔列是 `INTEGER`。
///
/// 绑定一个 Rust `bool` 参数，三种方言都能落到自己那一族的列上
/// （pg 走 BOOLEAN、mysql 走 TINYINT、sqlite 走 INTEGER），不必分叉 SQL。
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
    let sql = pool.sql(
        r#"INSERT INTO gb_platform (enable, name, server_gb_id, server_ip, server_port, device_gb_id,
           transport, username, password, expires, keep_timeout, status, create_time, update_time, auto_push_channel)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, '3600', '60', ?, ?, ?, ?)"#,
    );
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(true)
            .bind(name)
            .bind(server_gb_id)
            .bind(server_ip)
            .bind(server_port)
            .bind(device_gb_id)
            .bind(transport)
            .bind(username)
            .bind(password)
            .bind(false)
            .bind(now)
            .bind(now)
            .bind(true)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 更新平台
///
/// 三个方言的 SQL **只有占位符不同**（mysql / sqlite 用 `?`，postgres 用 `$N`），
/// 统一成 `?`，由 `pool.sql()` 在运行期改写。
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
    let sql = pool.sql(
        r#"UPDATE gb_platform SET
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
    );
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
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
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 删除平台
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    let sql = pool.sql("DELETE FROM gb_platform WHERE id = ?");
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 更新平台注册状态
pub async fn update_status(pool: &Pool, id: i64, status: bool) -> sqlx::Result<u64> {
    let sql = pool.sql("UPDATE gb_platform SET status = ? WHERE id = ?");
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(status).bind(id).execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 更新平台启用状态
pub async fn update_enable(pool: &Pool, id: i64, enable: bool) -> sqlx::Result<u64> {
    let sql = pool.sql("UPDATE gb_platform SET enable = ? WHERE id = ?");
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(enable).bind(id).execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 获取所有启用的平台
pub async fn get_all_enabled_platforms(pool: &Pool) -> sqlx::Result<Vec<Platform>> {
    let sql = pool.sql("SELECT * FROM gb_platform WHERE enable = ? ORDER BY id");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql).bind(true).fetch_all(p)
    })
}

/// 获取所有平台
pub async fn list_platforms(pool: &Pool) -> sqlx::Result<Vec<Platform>> {
    let sql = pool.sql("SELECT * FROM gb_platform ORDER BY id");
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql).fetch_all(p)
    })
}

/// 获取所有在线平台
pub async fn get_all_online_platforms(pool: &Pool) -> sqlx::Result<Vec<Platform>> {
    // `ONLINE_COLUMNS` 是运行期拼进去的列名；`?` 是占位符 —— 占位符形态由
    // **pool 自己的方言** 决定（postgres 只认 `$1`），所以先把模板交给
    // `pool.sql(..)` 改写，再把列名填进去（顺序不能反）。
    let sql = pool
        .sql("SELECT {ONLINE_COLUMNS} FROM gb_platform WHERE status = ? AND enable = ? ORDER BY id")
        .replace("{ONLINE_COLUMNS}", ONLINE_COLUMNS);
    crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, Platform>(&sql)
            .bind(true)
            .bind(true)
            .fetch_all(p)
    })
}
