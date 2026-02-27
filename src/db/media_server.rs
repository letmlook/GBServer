//! 流媒体服务器 wvp_media_server

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MediaServer {
    pub id: String,
    pub ip: Option<String>,
    pub hook_ip: Option<String>,
    pub http_port: Option<i32>,
    pub rtmp_port: Option<i32>,
    pub rtsp_port: Option<i32>,
    pub flv_port: Option<i32>,
    pub secret: Option<String>,
    pub rtp_enable: Option<bool>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

pub async fn list_media_servers(pool: &Pool) -> sqlx::Result<Vec<MediaServer>> {
    sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, http_port, rtmp_port, rtsp_port, flv_port, secret, rtp_enable, create_time, update_time FROM wvp_media_server ORDER BY id",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_media_server_by_id(pool: &Pool, id: &str) -> sqlx::Result<Option<MediaServer>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, http_port, rtmp_port, rtsp_port, flv_port, secret, rtp_enable, create_time, update_time FROM wvp_media_server WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, MediaServer>(
        "SELECT id, ip, hook_ip, http_port, rtmp_port, rtsp_port, flv_port, secret, rtp_enable, create_time, update_time FROM wvp_media_server WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}
