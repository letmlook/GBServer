//! 推流表 wvp_stream_push

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 推流记录结构体
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StreamPush {
    pub id: i32,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub create_time: Option<String>,
    pub media_server_id: Option<String>,
    pub server_id: Option<String>,
    pub push_time: Option<String>,
    pub status: Option<bool>,
    pub update_time: Option<String>,
    pub pushing: Option<bool>,
    // Map to the database column 'self'. Some environments store this as 'self',
    // while others may use 'self_push'. By keeping the field named 'self_push'
    // and removing the rename, we rely on the actual column name in the target DB
    // (or alias it in SELECT queries if needed in the future).
    pub self_push: Option<bool>,
    pub start_offline_push: Option<bool>,
}

/// 根据ID获取推流记录
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<StreamPush>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

/// 添加推流记录
pub async fn add(
    pool: &Pool,
    app: &str,
    stream: &str,
    media_server_id: &str,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_stream_push (app, stream, media_server_id, create_time, update_time, pushing, self, start_offline_push)
           VALUES (?, ?, ?, ?, ?, false, true, true)"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO wvp_stream_push (app, stream, media_server_id, create_time, update_time, pushing, self, start_offline_push)
           VALUES ($1, $2, $3, $4, $5, false, true, true)"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 更新推流记录
pub async fn update(
    pool: &Pool,
    id: i64,
    app: Option<&str>,
    stream: Option<&str>,
    media_server_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE wvp_stream_push SET
           app = COALESCE(?, app),
           stream = COALESCE(?, stream),
           media_server_id = COALESCE(?, media_server_id),
           update_time = ?
           WHERE id = ?"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE wvp_stream_push SET
           app = COALESCE($1, app),
           stream = COALESCE($2, stream),
           media_server_id = COALESCE($3, media_server_id),
           update_time = $4
           WHERE id = $5"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 删除推流记录
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_stream_push WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_stream_push WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 批量删除推流记录
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
    pushing: Option<bool>,
) -> sqlx::Result<Vec<StreamPush>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    if let Some(mid) = media_server_id {
        if let Some(p) = pushing {
            #[cfg(feature = "mysql")]
            return sqlx::query_as::<_, StreamPush>(
                "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE media_server_id = ? AND pushing = ? ORDER BY id LIMIT ? OFFSET ?",
            )
            .bind(mid)
            .bind(p)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_as::<_, StreamPush>(
                "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE media_server_id = $1 AND pushing = $2 ORDER BY id LIMIT $3 OFFSET $4",
            )
            .bind(mid)
            .bind(p)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
        } else {
            #[cfg(feature = "mysql")]
            return sqlx::query_as::<_, StreamPush>(
                "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE media_server_id = ? ORDER BY id LIMIT ? OFFSET ?",
            )
            .bind(mid)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_as::<_, StreamPush>(
                "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE media_server_id = $1 ORDER BY id LIMIT $2 OFFSET $3",
            )
            .bind(mid)
            .bind(limit)
            .bind(offset as i64)
            .fetch_all(pool)
            .await;
        }
    } else if let Some(p) = pushing {
        #[cfg(feature = "mysql")]
        return sqlx::query_as::<_, StreamPush>(
            "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE pushing = ? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(p)
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
        #[cfg(feature = "postgres")]
        return sqlx::query_as::<_, StreamPush>(
            "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE pushing = $1 ORDER BY id LIMIT $2 OFFSET $3",
        )
        .bind(p)
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
    } else {
        #[cfg(feature = "mysql")]
        return sqlx::query_as::<_, StreamPush>(
            "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset as i64)
        .fetch_all(pool)
        .await;
        #[cfg(feature = "postgres")]
        return sqlx::query_as::<_, StreamPush>(
            "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push ORDER BY id LIMIT $1 OFFSET $2",
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
    pushing: Option<bool>,
) -> sqlx::Result<i64> {
    if let Some(mid) = media_server_id {
        if let Some(p) = pushing {
            #[cfg(feature = "mysql")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push WHERE media_server_id = ? AND pushing = ?")
                .bind(mid)
                .bind(p)
                .fetch_one(pool)
                .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push WHERE media_server_id = $1 AND pushing = $2")
                .bind(mid)
                .bind(p)
                .fetch_one(pool)
                .await;
        } else {
            #[cfg(feature = "mysql")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push WHERE media_server_id = ?")
                .bind(mid)
                .fetch_one(pool)
                .await;
            #[cfg(feature = "postgres")]
            return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push WHERE media_server_id = $1")
                .bind(mid)
                .fetch_one(pool)
                .await;
        }
    } else if let Some(p) = pushing {
        #[cfg(feature = "mysql")]
        return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push WHERE pushing = ?")
            .bind(p)
            .fetch_one(pool)
            .await;
        #[cfg(feature = "postgres")]
        return sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push WHERE pushing = $1")
            .bind(p)
            .fetch_one(pool)
            .await;
    } else {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM wvp_stream_push")
            .fetch_one(pool)
            .await
    }
}

/// 更新推流状态
pub async fn update_pushing_status(pool: &Pool, id: i64, pushing: bool) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_stream_push SET pushing = ? WHERE id = ?")
        .bind(pushing)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_stream_push SET pushing = $1 WHERE id = $2")
        .bind(pushing)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 更新推流状态（status字段）
pub async fn update_status(pool: &Pool, id: i64, status: bool) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE wvp_stream_push SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE wvp_stream_push SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 根据app和stream查询推流记录
pub async fn get_by_app_stream(pool: &Pool, app: &str, stream: &str) -> sqlx::Result<Option<StreamPush>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE app = ? AND stream = ?"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push FROM wvp_stream_push WHERE app = $1 AND stream = $2"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
}
