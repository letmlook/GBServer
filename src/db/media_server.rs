//! 流媒体服务器 gb_media_server

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
        "SELECT * FROM gb_media_server ORDER BY id"
    )
    .fetch_all(pool)
    .await
}

/// 根据ID获取媒体服务器
pub async fn get_media_server_by_id(pool: &Pool, id: &str) -> sqlx::Result<Option<MediaServer>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT * FROM gb_media_server WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT * FROM gb_media_server WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT * FROM gb_media_server WHERE id = ?"
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
        r#"INSERT INTO gb_media_server (id, ip, http_port, create_time, update_time, auto_config, rtp_enable, default_server)
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
        r#"INSERT INTO gb_media_server (id, ip, http_port, create_time, update_time, auto_config, rtp_enable, default_server)
           VALUES ($1, $2, $3, $4, $5, false, false, false)"#
    )
    .bind(id)
    .bind(ip)
    .bind(http_port)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"INSERT INTO gb_media_server (id, ip, http_port, create_time, update_time, auto_config, rtp_enable, default_server)
           VALUES (?, ?, ?, ?, ?, 0, 0, 0)"#
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
        r#"UPDATE gb_media_server SET
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
        r#"UPDATE gb_media_server SET
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
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"UPDATE gb_media_server SET
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
    Ok(r.rows_affected())
}

/// 删除媒体服务器
pub async fn delete_by_id(pool: &Pool, id: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM gb_media_server WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_media_server WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("DELETE FROM gb_media_server WHERE id = ?")
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
        r#"INSERT INTO gb_media_server (id, ip, http_port, secret, create_time, update_time, auto_config, rtp_enable, default_server, server_id, type)
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
        r#"INSERT INTO gb_media_server (id, ip, http_port, secret, create_time, update_time, auto_config, rtp_enable, default_server, server_id, type)
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
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"INSERT INTO gb_media_server (id, ip, http_port, secret, create_time, update_time, auto_config, rtp_enable, default_server, server_id, type)
           VALUES (?, ?, ?, ?, ?, ?, 0, 0, 1, ?, 'zlm')
           ON CONFLICT(id) DO UPDATE SET ip = excluded.ip, http_port = excluded.http_port, secret = excluded.secret, update_time = excluded.update_time"#
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
    Ok(r.rows_affected())
}

/// 获取默认媒体服务器
pub async fn get_default_server(pool: &Pool) -> sqlx::Result<Option<MediaServer>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM gb_media_server WHERE default_server = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM gb_media_server WHERE default_server = true LIMIT 1",
    )
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM gb_media_server WHERE default_server = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await;
}

/// 统计媒体服务器数量
pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_media_server")
        .fetch_one(pool)
        .await
}

/// 更新服务器状态
pub async fn update_status(pool: &Pool, id: &str, status: bool, last_keepalive: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_media_server SET status = ?, last_keepalive_time = ? WHERE id = ?")
        .bind(status)
        .bind(last_keepalive)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_media_server SET status = $1, last_keepalive_time = $2 WHERE id = $3")
        .bind(status)
        .bind(last_keepalive)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_media_server SET status = ?, last_keepalive_time = ? WHERE id = ?")
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
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM gb_media_server WHERE status = 1 ORDER BY id",
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM gb_media_server WHERE status = true ORDER BY id",
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, sdp_ip, stream_ip, http_port, http_ssl_port, rtmp_port, rtsp_port, rtsp_ssl_port, flv_port, flv_ssl_port, ws_port, wss_port, rtp_proxy_port, secret, rtp_enable, default_server, record_assist_port, record_day, record_transcode, create_time, update_time, status, last_keepalive_time FROM gb_media_server WHERE status = 1 ORDER BY id",
    )
    .fetch_all(pool)
    .await;
}

/// Update media server ports from ZLM on_server_started hook
pub async fn update_ports(
    pool: &Pool,
    id: &str,
    http_port: i32,
    http_ssl_port: Option<i32>,
    rtsp_port: Option<i32>,
    rtmp_port: Option<i32>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE gb_media_server SET
           http_port = COALESCE(?, http_port),
           http_ssl_port = COALESCE(?, http_ssl_port),
           rtsp_port = COALESCE(?, rtsp_port),
           rtmp_port = COALESCE(?, rtmp_port),
           status = 1,
           update_time = ?
           WHERE id = ?"#
    )
    .bind(http_port)
    .bind(http_ssl_port)
    .bind(rtsp_port)
    .bind(rtmp_port)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE gb_media_server SET
           http_port = COALESCE($1, http_port),
           http_ssl_port = COALESCE($2, http_ssl_port),
           rtsp_port = COALESCE($3, rtsp_port),
           rtmp_port = COALESCE($4, rtmp_port),
           status = true,
           update_time = $5
           WHERE id = $6"#
    )
    .bind(http_port)
    .bind(http_ssl_port)
    .bind(rtsp_port)
    .bind(rtmp_port)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"UPDATE gb_media_server SET
           http_port = COALESCE(?, http_port),
           http_ssl_port = COALESCE(?, http_ssl_port),
           rtsp_port = COALESCE(?, rtsp_port),
           rtmp_port = COALESCE(?, rtmp_port),
           status = 1,
           update_time = ?
           WHERE id = ?"#
    )
    .bind(http_port)
    .bind(http_ssl_port)
    .bind(rtsp_port)
    .bind(rtmp_port)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Update last keepalive time
///
/// Phase 4 follow-up: 同时 reset `consecutive_misses = 0`，避免 grace count
/// 在节点恢复后还保留旧的丢失计数。
pub async fn update_last_keepalive(
    pool: &Pool,
    id: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET last_keepalive_time = ?, status = 1, consecutive_misses = 0 WHERE id = ?"
    )
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET last_keepalive_time = $1, status = true, consecutive_misses = 0 WHERE id = $2"
    )
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET last_keepalive_time = ?, status = 1, consecutive_misses = 0 WHERE id = ?"
    )
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Mark media servers as offline if their last keepalive is older than `before_time`.
///
/// Only rows currently with `status = 1` are eligible; the timestamp is
/// compared against `last_keepalive_time` (stored as RFC3339 string).
///
/// Returns the number of rows updated (i.e. newly-offline nodes).
pub async fn mark_offline_if_expired(
    pool: &Pool,
    before_time: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET status = 0 WHERE status = 1 AND last_keepalive_time < ?"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET status = 0 WHERE status = 1 AND last_keepalive_time < $1"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET status = 0 WHERE status = 1 AND last_keepalive_time < ?"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Phase 4 follow-up: keepalive_grace_count 容错版本（拆分两个原子 SQL）
///
/// 步骤 1：`increment_miss_count_if_expired` —
/// 对每个 status=1 且 `last_keepalive_time < before_time` 的节点，
/// 仅 `consecutive_misses += 1`。返回被递增的节点数。
pub async fn increment_miss_count_if_expired(
    pool: &Pool,
    before_time: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET consecutive_misses = consecutive_misses + 1 \
         WHERE status = 1 AND last_keepalive_time < ?"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET consecutive_misses = consecutive_misses + 1 \
         WHERE status = true AND last_keepalive_time < $1"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET consecutive_misses = consecutive_misses + 1 \
         WHERE status = 1 AND last_keepalive_time < ?"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Phase 4 follow-up: keepalive_grace_count 容错版本（步骤 2）
///
/// 对每个 status=1 且 `consecutive_misses >= grace_count` 的节点：
/// - `status = false` (offline)
/// - reset `consecutive_misses = 0`
///
/// 返回被新切为 offline 的节点数。
pub async fn mark_offline_if_miss_count_exceeded(
    pool: &Pool,
    grace_count: i32,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET status = 0, consecutive_misses = 0 \
         WHERE status = 1 AND consecutive_misses >= ?"
    )
    .bind(grace_count)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET status = false, consecutive_misses = 0 \
         WHERE status = true AND consecutive_misses >= $1"
    )
    .bind(grace_count)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET status = 0, consecutive_misses = 0 \
         WHERE status = 1 AND consecutive_misses >= ?"
    )
    .bind(grace_count)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Phase 4 follow-up: 重置健康节点的 `consecutive_misses = 0`（keepalive 恢复）
pub async fn reset_miss_count_for_fresh_nodes(
    pool: &Pool,
    before_time: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET consecutive_misses = 0 \
         WHERE status = 1 AND last_keepalive_time >= ? AND consecutive_misses > 0"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET consecutive_misses = 0 \
         WHERE status = true AND last_keepalive_time >= $1 AND consecutive_misses > 0"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET consecutive_misses = 0 \
         WHERE status = 1 AND last_keepalive_time >= ? AND consecutive_misses > 0"
    )
    .bind(before_time)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Update flow statistics from flow report webhook
pub async fn update_flow_stats(
    pool: &Pool,
    id: &str,
    total_bytes: i64,
    active_streams: i32,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET total_bytes = COALESCE(total_bytes, 0) + ?, active_stream_count = ?, update_time = ? WHERE id = ?"
    )
    .bind(total_bytes)
    .bind(active_streams)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET total_bytes = COALESCE(total_bytes, 0) + $1, active_stream_count = $2, update_time = $3 WHERE id = $4"
    )
    .bind(total_bytes)
    .bind(active_streams)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_media_server SET total_bytes = COALESCE(total_bytes, 0) + ?, active_stream_count = ?, update_time = ? WHERE id = ?"
    )
    .bind(total_bytes)
    .bind(active_streams)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

// =====================================================================
// IP 白名单（Phase 4.2 — hook secret 鉴权 + IP 校验）
//
// 每个媒体服务器可绑定一组 CIDR 网段，ZLM hook（on_play / on_publish）时
// 检查客户端 IP 是否落在白名单内。
// =====================================================================

/// 获取指定媒体服务器的全部白名单 CIDR
pub async fn get_white_list_cidrs(
    pool: &Pool,
    media_server_id: &str,
) -> sqlx::Result<Vec<String>> {
    #[cfg(feature = "mysql")]
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT cidr FROM gb_media_server_white_list WHERE media_server_id = ? ORDER BY id"
    )
    .bind(media_server_id)
    .fetch_all(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT cidr FROM gb_media_server_white_list WHERE media_server_id = $1 ORDER BY id"
    )
    .bind(media_server_id)
    .fetch_all(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT cidr FROM gb_media_server_white_list WHERE media_server_id = ? ORDER BY id"
    )
    .bind(media_server_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 新增一条白名单 CIDR
pub async fn add_white_list_cidr(
    pool: &Pool,
    media_server_id: &str,
    cidr: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "INSERT INTO gb_media_server_white_list (media_server_id, cidr, create_time) VALUES (?, ?, ?)"
    )
    .bind(media_server_id)
    .bind(cidr)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "INSERT INTO gb_media_server_white_list (media_server_id, cidr, create_time) VALUES ($1, $2, $3)"
    )
    .bind(media_server_id)
    .bind(cidr)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "INSERT INTO gb_media_server_white_list (media_server_id, cidr, create_time) VALUES (?, ?, ?)"
    )
    .bind(media_server_id)
    .bind(cidr)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 删除一条白名单 CIDR（按 media_server_id + cidr 唯一匹配）
pub async fn remove_white_list_cidr(
    pool: &Pool,
    media_server_id: &str,
    cidr: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "DELETE FROM gb_media_server_white_list WHERE media_server_id = ? AND cidr = ?"
    )
    .bind(media_server_id)
    .bind(cidr)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "DELETE FROM gb_media_server_white_list WHERE media_server_id = $1 AND cidr = $2"
    )
    .bind(media_server_id)
    .bind(cidr)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "DELETE FROM gb_media_server_white_list WHERE media_server_id = ? AND cidr = ?"
    )
    .bind(media_server_id)
    .bind(cidr)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}
