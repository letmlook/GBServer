//! 拉流代理表 wvp_stream_proxy

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StreamProxy {
    pub id: i64,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub src_url: Option<String>,
    pub media_server_id: Option<String>,
    pub pulling: Option<bool>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub name: Option<String>,
}

pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    media_server_id: Option<&str>,
    pulling: Option<bool>,
) -> sqlx::Result<Vec<StreamProxy>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    if let Some(mid) = media_server_id {
        if let Some(p) = pulling {
            #[cfg(feature = "mysql")]
            return sqlx::query_as::<_, StreamProxy>(
                "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy WHERE media_server_id = ? AND pulling = ? ORDER BY id LIMIT ? OFFSET ?",
            )
            .bind(mid)
            .bind(p)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_as::<_, StreamProxy>(
                "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy WHERE media_server_id = $1 AND pulling = $2 ORDER BY id LIMIT $3 OFFSET $4",
            )
            .bind(mid)
            .bind(p)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
        } else {
            #[cfg(feature = "mysql")]
            return sqlx::query_as::<_, StreamProxy>(
                "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy WHERE media_server_id = ? ORDER BY id LIMIT ? OFFSET ?",
            )
            .bind(mid)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_as::<_, StreamProxy>(
                "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy WHERE media_server_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(mid)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
        }
    } else if let Some(p) = pulling {
        #[cfg(feature = "mysql")]
        return sqlx::query_as::<_, StreamProxy>(
            "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy WHERE pulling = ? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(p)
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
        #[cfg(feature = "postgres")]
        return sqlx::query_as::<_, StreamProxy>(
            "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy WHERE pulling = $1 ORDER BY id LIMIT $2 OFFSET $3",
        )
        .bind(p)
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
    } else {
        #[cfg(feature = "mysql")]
        return sqlx::query_as::<_, StreamProxy>(
            "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
        #[cfg(feature = "postgres")]
        return sqlx::query_as::<_, StreamProxy>(
            "SELECT id, app, stream, src_url, media_server_id, pulling, create_time, update_time, name FROM wvp_stream_proxy ORDER BY id LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
    }
}

pub async fn count_all(
    pool: &Pool,
    media_server_id: Option<&str>,
    pulling: Option<bool>,
) -> sqlx::Result<i64> {
    if let Some(mid) = media_server_id {
        if let Some(p) = pulling {
            #[cfg(feature = "mysql")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy WHERE media_server_id = ? AND pulling = ?")
                .bind(mid)
                .bind(p)
                .fetch_one(pool)
                .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy WHERE media_server_id = $1 AND pulling = $2")
                .bind(mid)
                .bind(p)
                .fetch_one(pool)
                .await;
        } else {
            #[cfg(feature = "mysql")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy WHERE media_server_id = ?")
                .bind(mid)
                .fetch_one(pool)
                .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy WHERE media_server_id = $1")
                .bind(mid)
                .fetch_one(pool)
                .await;
        }
    } else if let Some(p) = pulling {
        #[cfg(feature = "mysql")]
        return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy WHERE pulling = ?")
            .bind(p)
            .fetch_one(pool)
            .await;
        #[cfg(feature = "postgres")]
        return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy WHERE pulling = $1")
            .bind(p)
            .fetch_one(pool)
            .await;
    } else {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_proxy")
            .fetch_one(pool)
            .await
    }
}
