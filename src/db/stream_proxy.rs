//! 拉流代理表 wvp_stream_proxy

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 拉流代理结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StreamProxy {
    pub id: i64,
    pub type_: Option<String>,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub src_url: Option<String>,
    pub timeout: Option<i32>,
    pub ffmpeg_cmd_key: Option<String>,
    pub rtsp_type: Option<String>,
    pub media_server_id: Option<String>,
    pub enable_audio: Option<bool>,
    pub enable_mp4: Option<bool>,
    pub pulling: Option<bool>,
    pub enable: Option<bool>,
    pub create_time: Option<String>,
    pub name: Option<String>,
    pub update_time: Option<String>,
    pub stream_key: Option<String>,
    pub server_id: Option<String>,
    pub enable_disable_none_reader: Option<bool>,
    pub relates_media_server_id: Option<String>,
}

/// 根据ID获取拉流代理
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<StreamProxy>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT * FROM wvp_stream_proxy WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id FROM wvp_stream_proxy WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

/// 添加拉流代理
pub async fn add(
    pool: &Pool,
    app: &str,
    stream: &str,
    src_url: &str,
    media_server_id: &str,
    name: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_stream_proxy (app, stream, src_url, media_server_id, name, create_time, update_time, enable, pulling)
           VALUES (?, ?, ?, ?, ?, ?, ?, false, false)"#
    )
    .bind(app)
    .bind(stream)
    .bind(src_url)
    .bind(media_server_id)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_stream_proxy (app, stream, src_url, media_server_id, name, create_time, update_time, enable, pulling)
           VALUES ($1, $2, $3, $4, $5, $6, $7, false, false)"#
    )
    .bind(app)
    .bind(stream)
    .bind(src_url)
    .bind(media_server_id)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 更新拉流代理
pub async fn update(
    pool: &Pool,
    id: i64,
    app: Option<&str>,
    stream: Option<&str>,
    src_url: Option<&str>,
    media_server_id: Option<&str>,
    name: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE wvp_stream_proxy SET
           app = COALESCE(?, app),
           stream = COALESCE(?, stream),
           src_url = COALESCE(?, src_url),
           media_server_id = COALESCE(?, media_server_id),
           name = COALESCE(?, name),
           update_time = ?
           WHERE id = ?"#
    )
    .bind(app)
    .bind(stream)
    .bind(src_url)
    .bind(media_server_id)
    .bind(name)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE wvp_stream_proxy SET
           app = COALESCE($1, app),
           stream = COALESCE($2, stream),
           src_url = COALESCE($3, src_url),
           media_server_id = COALESCE($4, media_server_id),
           name = COALESCE($5, name),
           update_time = $6
           WHERE id = $7"#
    )
    .bind(app)
    .bind(stream)
    .bind(src_url)
    .bind(media_server_id)
    .bind(name)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 删除拉流代理
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_stream_proxy WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_stream_proxy WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 批量删除拉流代理
pub async fn batch_delete(pool: &Pool, ids: &[i64]) -> sqlx::Result<u64> {
    let mut total: u64 = 0;
    for id in ids {
        let r = delete_by_id(pool, *id).await?;
        total += r;
    }
    Ok(total)
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
