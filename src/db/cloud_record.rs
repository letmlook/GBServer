//! 云端录像记录表 wvp_cloud_record

use serde::{Deserialize, Serialize};

use super::Pool;

/// 云端录像记录结构体
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CloudRecord {
    pub id: i64,
    pub app: String,
    pub stream: String,
    pub call_id: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub media_server_id: Option<String>,
    pub server_id: Option<String>,
    pub file_name: Option<String>,
    pub folder: Option<String>,
    pub file_path: Option<String>,
    pub collect: Option<bool>,
    pub file_size: Option<i64>,
    pub time_len: Option<f64>,
}

/// 插入云端录像参数
pub struct CloudRecordInsert {
    pub app: String,
    pub stream: String,
    pub call_id: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub media_server_id: Option<String>,
    pub server_id: Option<String>,
    pub file_name: Option<String>,
    pub folder: Option<String>,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub time_len: Option<f64>,
}

/// 更新云端录像参数
pub struct CloudRecordUpdate {
    pub id: i64,
    pub end_time: Option<i64>,
    pub file_size: Option<i64>,
    pub time_len: Option<f64>,
}

/// 插入云端录像记录
pub async fn insert(pool: &Pool, record: &CloudRecordInsert) -> sqlx::Result<i64> {
    #[cfg(feature = "postgres")]
    {
        let result: (i64,) = sqlx::query_as(
            "INSERT INTO wvp_cloud_record \
             (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
              file_name, folder, file_path, file_size, time_len) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
             RETURNING id"
        )
        .bind(&record.app)
        .bind(&record.stream)
        .bind(&record.call_id)
        .bind(record.start_time)
        .bind(record.end_time)
        .bind(&record.media_server_id)
        .bind(&record.server_id)
        .bind(&record.file_name)
        .bind(&record.folder)
        .bind(&record.file_path)
        .bind(record.file_size)
        .bind(record.time_len)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "INSERT INTO wvp_cloud_record \
             (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
              file_name, folder, file_path, file_size, time_len) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&record.app)
        .bind(&record.stream)
        .bind(&record.call_id)
        .bind(record.start_time)
        .bind(record.end_time)
        .bind(&record.media_server_id)
        .bind(&record.server_id)
        .bind(&record.file_name)
        .bind(&record.folder)
        .bind(&record.file_path)
        .bind(record.file_size)
        .bind(record.time_len)
        .execute(pool)
        .await?;

        Ok(result.last_insert_id() as i64)
    }
}

/// 根据ID查询云端录像
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<CloudRecord>> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM wvp_cloud_record WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    #[cfg(feature = "mysql")]
    {
        sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM wvp_cloud_record WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }
}

/// 分页查询云端录像列表
pub async fn list_paged(
    pool: &Pool,
    app: Option<&str>,
    stream: Option<&str>,
    media_server_id: Option<&str>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    page: i64,
    count: i64,
) -> sqlx::Result<Vec<CloudRecord>> {
    let offset = (page - 1) * count;

    #[cfg(feature = "postgres")]
    {
        let query = sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM wvp_cloud_record \
             WHERE ($1::text IS NULL OR app = $1) \
               AND ($2::text IS NULL OR stream = $2) \
               AND ($3::text IS NULL OR media_server_id = $3) \
               AND ($4::bigint IS NULL OR start_time >= $4) \
               AND ($5::bigint IS NULL OR end_time <= $5) \
             ORDER BY start_time DESC \
             LIMIT $6 OFFSET $7"
        )
        .bind(app)
        .bind(stream)
        .bind(media_server_id)
        .bind(start_time)
        .bind(end_time)
        .bind(count)
        .bind(offset);

        query.fetch_all(pool).await
    }

    #[cfg(feature = "mysql")]
    {
        let query = sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM wvp_cloud_record \
             WHERE (? IS NULL OR app = ?) \
               AND (? IS NULL OR stream = ?) \
               AND (? IS NULL OR media_server_id = ?) \
               AND (? IS NULL OR start_time >= ?) \
               AND (? IS NULL OR end_time <= ?) \
             ORDER BY start_time DESC \
             LIMIT ? OFFSET ?"
        )
        .bind(app)
        .bind(app)
        .bind(stream)
        .bind(stream)
        .bind(media_server_id)
        .bind(media_server_id)
        .bind(start_time)
        .bind(start_time)
        .bind(end_time)
        .bind(end_time)
        .bind(count)
        .bind(offset);

        query.fetch_all(pool).await
    }
}

/// 统计云端录像数量
pub async fn count_all(
    pool: &Pool,
    app: Option<&str>,
    stream: Option<&str>,
    media_server_id: Option<&str>,
    start_time: Option<i64>,
    end_time: Option<i64>,
) -> sqlx::Result<i64> {
    #[cfg(feature = "postgres")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wvp_cloud_record \
             WHERE ($1::text IS NULL OR app = $1) \
               AND ($2::text IS NULL OR stream = $2) \
               AND ($3::text IS NULL OR media_server_id = $3) \
               AND ($4::bigint IS NULL OR start_time >= $4) \
               AND ($5::bigint IS NULL OR end_time <= $5)"
        )
        .bind(app)
        .bind(stream)
        .bind(media_server_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    #[cfg(feature = "mysql")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM wvp_cloud_record \
             WHERE (? IS NULL OR app = ?) \
               AND (? IS NULL OR stream = ?) \
               AND (? IS NULL OR media_server_id = ?) \
               AND (? IS NULL OR start_time >= ?) \
               AND (? IS NULL OR end_time <= ?)"
        )
        .bind(app)
        .bind(app)
        .bind(stream)
        .bind(stream)
        .bind(media_server_id)
        .bind(media_server_id)
        .bind(start_time)
        .bind(start_time)
        .bind(end_time)
        .bind(end_time)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }
}

/// 更新云端录像
pub async fn update(pool: &Pool, record: &CloudRecordUpdate) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query(
            "UPDATE wvp_cloud_record SET \
             end_time = COALESCE($2, end_time), \
             file_size = COALESCE($3, file_size), \
             time_len = COALESCE($4, time_len) \
             WHERE id = $1"
        )
        .bind(record.id)
        .bind(record.end_time)
        .bind(record.file_size)
        .bind(record.time_len)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query(
            "UPDATE wvp_cloud_record SET \
             end_time = COALESCE(?, end_time), \
             file_size = COALESCE(?, file_size), \
             time_len = COALESCE(?, time_len) \
             WHERE id = ?"
        )
        .bind(record.end_time)
        .bind(record.file_size)
        .bind(record.time_len)
        .bind(record.id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

/// 删除云端录像
pub async fn delete(pool: &Pool, id: i64) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("DELETE FROM wvp_cloud_record WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM wvp_cloud_record WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// 批量删除云端录像
pub async fn batch_delete(pool: &Pool, ids: &[i64]) -> sqlx::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }

    #[cfg(feature = "postgres")]
    {
        let placeholders: Vec<String> = ids.iter().map(|_| "$1".to_string()).collect();
        let sql = format!(
            "DELETE FROM wvp_cloud_record WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "mysql")]
    {
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM wvp_cloud_record WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }
}

/// 设置/取消收藏
pub async fn set_collect(pool: &Pool, id: i64, collect: bool) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query("UPDATE wvp_cloud_record SET collect = $2 WHERE id = $1")
            .bind(id)
            .bind(collect)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("UPDATE wvp_cloud_record SET collect = ? WHERE id = ?")
            .bind(collect)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// 根据 call_id 查询录像
pub async fn get_by_call_id(pool: &Pool, call_id: &str) -> sqlx::Result<Option<CloudRecord>> {
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, duration, media_server_id, file_name, file_path, file_size, create_time, collect FROM wvp_cloud_record WHERE call_id = $1"
    )
    .bind(call_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, duration, media_server_id, file_name, file_path, file_size, create_time, collect FROM wvp_cloud_record WHERE call_id = ?"
    )
    .bind(call_id)
    .fetch_optional(pool)
    .await;
}

/// 删除指定流的录像
pub async fn delete_by_app_stream(pool: &Pool, app: &str, stream: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_cloud_record WHERE app = $1 AND stream = $2")
        .bind(app)
        .bind(stream)
        .execute(pool)
        .await?;
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_cloud_record WHERE app = ? AND stream = ?")
        .bind(app)
        .bind(stream)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 获取收藏的录像
pub async fn get_collect_records(pool: &Pool) -> sqlx::Result<Vec<CloudRecord>> {
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, duration, media_server_id, file_name, file_path, file_size, create_time, collect FROM wvp_cloud_record WHERE collect = true ORDER BY create_time DESC"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, duration, media_server_id, file_name, file_path, file_size, create_time, collect FROM wvp_cloud_record WHERE collect = 1 ORDER BY create_time DESC"
    )
    .fetch_all(pool)
    .await;
}

/// 删除指定时间之前的录像
pub async fn delete_before_time(pool: &Pool, before_time: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM wvp_cloud_record WHERE end_time < $1")
        .bind(before_time)
        .execute(pool)
        .await?;
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM wvp_cloud_record WHERE end_time < ?")
        .bind(before_time)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}
