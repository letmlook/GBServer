//! 流媒体服务器 wvp_media_server

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 媒体服务器结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MediaServer {
    pub id: String,
    pub ip: Option<String>,
    pub hook_ip: Option<String>,
    pub sdp_ip: Option<String>,
    pub stream_ip: Option<String>,
    pub http_port: Option<i32>,
    pub http_ssl_port: Option<i32>,
    pub rtmp_port: Option<i32>,
    pub rtmp_ssl_port: Option<i32>,
    pub rtp_proxy_port: Option<i32>,
    pub rtsp_port: Option<i32>,
    pub rtsp_ssl_port: Option<i32>,
    pub flv_port: Option<i32>,
    pub flv_ssl_port: Option<i32>,
    pub mp4_port: Option<i32>,
    pub mp4_ssl_port: Option<i32>,
    pub ws_flv_port: Option<i32>,
    pub ws_flv_ssl_port: Option<i32>,
    pub jtt_proxy_port: Option<i32>,
    pub auto_config: Option<bool>,
    pub secret: Option<String>,
    #[sqlx(rename = "type")]
    pub type_: Option<String>,
    pub rtp_enable: Option<bool>,
    pub rtp_port_range: Option<String>,
    pub send_rtp_port_range: Option<String>,
    pub record_assist_port: Option<i32>,
    pub default_server: Option<bool>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub hook_alive_interval: Option<i32>,
    pub record_path: Option<String>,
    pub record_day: Option<i32>,
    pub transcode_suffix: Option<String>,
    pub server_id: Option<String>,
}

/// 获取所有媒体服务器
pub async fn list_media_servers(pool: &Pool) -> sqlx::Result<Vec<MediaServer>> {
    sqlx::query_as::<_, MediaServer>(
        "SELECT * FROM wvp_media_server ORDER BY id"
    )
    .fetch_all(pool)
    .await
}

/// 根据ID获取媒体服务器
pub async fn get_media_server_by_id(pool: &Pool, id: &str) -> sqlx::Result<Option<MediaServer>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT * FROM wvp_media_server WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT * FROM wvp_media_server WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

/// 添加媒体服务器
pub async fn add(
    pool: &Pool,
    id: &str,
    ip: &str,
    http_port: i32,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_media_server (id, ip, http_port, create_time, update_time, auto_config, rtp_enable, default_server)
           VALUES (?, ?, ?, ?, ?, false, false, false)"#
    )
    .bind(id)
    .bind(ip)
    .bind(http_port)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_media_server (id, ip, http_port, create_time, update_time, auto_config, rtp_enable, default_server)
           VALUES ($1, $2, $3, $4, $5, false, false, false)"#
    )
    .bind(id)
    .bind(ip)
    .bind(http_port)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 更新媒体服务器
pub async fn update(
    pool: &Pool,
    id: &str,
    ip: Option<&str>,
    hook_ip: Option<&str>,
    http_port: Option<i32>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE wvp_media_server SET
           ip = COALESCE(?, ip),
           hook_ip = COALESCE(?, hook_ip),
           http_port = COALESCE(?, http_port),
           update_time = ?
           WHERE id = ?"#
    )
    .bind(ip)
    .bind(hook_ip)
    .bind(http_port)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE wvp_media_server SET
           ip = COALESCE($1, ip),
           hook_ip = COALESCE($2, hook_ip),
           http_port = COALESCE($3, http_port),
           update_time = $4
           WHERE id = $5"#
    )
    .bind(ip)
    .bind(hook_ip)
    .bind(http_port)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 删除媒体服务器
pub async fn delete_by_id(pool: &Pool, id: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_media_server WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_media_server WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 同步配置文件中的媒体服务器到数据库（upsert）
pub async fn sync_from_config(
    pool: &Pool,
    id: &str,
    ip: &str,
    http_port: i32,
    secret: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_media_server (id, ip, http_port, secret, create_time, update_time, auto_config, rtp_enable, default_server, server_id, type)
           VALUES (?, ?, ?, ?, ?, ?, false, false, true, ?, 'zlm')
           ON DUPLICATE KEY UPDATE ip = VALUES(ip), http_port = VALUES(http_port), secret = VALUES(secret), update_time = VALUES(update_time)"#
    )
    .bind(id)
    .bind(ip)
    .bind(http_port)
    .bind(secret)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_media_server (id, ip, http_port, secret, create_time, update_time, auto_config, rtp_enable, default_server, server_id, type)
           VALUES ($1, $2, $3, $4, $5, $6, false, false, true, $1, 'zlm')
           ON CONFLICT (id) DO UPDATE SET ip = EXCLUDED.ip, http_port = EXCLUDED.http_port, secret = EXCLUDED.secret, update_time = EXCLUDED.update_time"#
    )
    .bind(id)
    .bind(ip)
    .bind(http_port)
    .bind(secret)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 获取默认媒体服务器
pub async fn get_default_server(pool: &Pool) -> sqlx::Result<Option<MediaServer>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM wvp_media_server WHERE default_server = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM wvp_media_server WHERE default_server = true LIMIT 1",
    )
    .fetch_optional(pool)
    .await;
}

/// 统计媒体服务器数量
pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_media_server")
        .fetch_one(pool)
        .await
}

/// 更新服务器状态
pub async fn update_status(pool: &Pool, id: &str, status: bool, last_keepalive: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_media_server SET status = ?, last_keepalive_time = ? WHERE id = ?")
        .bind(status)
        .bind(last_keepalive)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_media_server SET status = $1, last_keepalive_time = $2 WHERE id = $3")
        .bind(status)
        .bind(last_keepalive)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 获取所有在线的媒体服务器
pub async fn list_online_servers(pool: &Pool) -> sqlx::Result<Vec<MediaServer>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM wvp_media_server WHERE status = 1 ORDER BY id",
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM wvp_media_server WHERE status = true ORDER BY id",
    )
    .fetch_all(pool)
    .await;
}
