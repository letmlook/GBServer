//! 云端录像记录表 gb_cloud_record

use serde::{Deserialize, Serialize};

use super::Pool;

/// 云端录像记录结构体
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CloudRecord {
    pub id: i64,
    pub app: String,
    pub stream: String,
    #[serde(alias = "callId")]
    pub call_id: Option<String>,
    #[serde(alias = "startTime")]
    pub start_time: Option<i64>,
    #[serde(alias = "endTime")]
    pub end_time: Option<i64>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    #[serde(alias = "serverId")]
    pub server_id: Option<String>,
    #[serde(alias = "fileName")]
    pub file_name: Option<String>,
    pub folder: Option<String>,
    #[serde(alias = "filePath")]
    pub file_path: Option<String>,
    pub collect: Option<bool>,
    #[serde(alias = "fileSize")]
    pub file_size: Option<i64>,
    #[serde(alias = "timeLen")]
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
            "INSERT INTO gb_cloud_record \
             (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
              file_name, folder, file_path, file_size, time_len) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
             RETURNING id",
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
            "INSERT INTO gb_cloud_record \
             (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
              file_name, folder, file_path, file_size, time_len) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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

    #[cfg(feature = "sqlite")]
    {
        let result = sqlx::query(
            "INSERT INTO gb_cloud_record \
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

        Ok(result.last_insert_rowid() as i64)
    }
}

/// 根据ID查询云端录像
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<CloudRecord>> {
    #[cfg(feature = "postgres")]
    {
        sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM gb_cloud_record WHERE id = $1"
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
             FROM gb_cloud_record WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    #[cfg(feature = "sqlite")]
    {
        sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM gb_cloud_record WHERE id = ?"
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
             FROM gb_cloud_record \
             WHERE ($1::text IS NULL OR app = $1) \
               AND ($2::text IS NULL OR stream = $2) \
               AND ($3::text IS NULL OR media_server_id = $3) \
               AND ($4::bigint IS NULL OR start_time >= $4) \
               AND ($5::bigint IS NULL OR end_time <= $5) \
             ORDER BY start_time DESC \
             LIMIT $6 OFFSET $7",
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
             FROM gb_cloud_record \
             WHERE (? IS NULL OR app = ?) \
               AND (? IS NULL OR stream = ?) \
               AND (? IS NULL OR media_server_id = ?) \
               AND (? IS NULL OR start_time >= ?) \
               AND (? IS NULL OR end_time <= ?) \
             ORDER BY start_time DESC \
             LIMIT ? OFFSET ?",
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

    #[cfg(feature = "sqlite")]
    {
        let query = sqlx::query_as::<_, CloudRecord>(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM gb_cloud_record \
             WHERE (?1 IS NULL OR app = ?1) \
               AND (?2 IS NULL OR stream = ?2) \
               AND (?3 IS NULL OR media_server_id = ?3) \
               AND (?4 IS NULL OR start_time >= ?4) \
               AND (?5 IS NULL OR end_time <= ?5) \
             ORDER BY start_time DESC \
             LIMIT ?6 OFFSET ?7"
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
}

/// 云端录像列表的筛选条件（全部可选；`None` = 不过滤）。
#[derive(Debug, Default, Clone)]
pub struct CloudRecordFilter<'a> {
    pub app: Option<&'a str>,
    pub stream: Option<&'a str>,
    pub media_server_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    /// 关键字：文件名/流名/App 模糊匹配
    pub query: Option<&'a str>,
    /// 国标设备号 / 通道号 —— 本平台的录像流名是 `{deviceId}_{channelId}`，
    /// 因此按 `stream` 前缀匹配（`deviceId_%` / `%_channelId`）。
    pub device_id: Option<&'a str>,
    pub channel_id: Option<&'a str>,
    /// 起始/结束时间（毫秒）
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

impl<'a> CloudRecordFilter<'a> {
    fn to_where(&self) -> crate::dyn_where::DynWhere {
        use crate::dyn_where::{BindValue, DynWhere};
        let mut w = DynWhere::new();
        if let Some(v) = self.app.filter(|s| !s.is_empty()) {
            w.add("app = ?", vec![BindValue::Text(v.to_string())]);
        }
        if let Some(v) = self.stream.filter(|s| !s.is_empty()) {
            w.add("stream = ?", vec![BindValue::Text(v.to_string())]);
        }
        if let Some(v) = self.media_server_id.filter(|s| !s.is_empty()) {
            w.add("media_server_id = ?", vec![BindValue::Text(v.to_string())]);
        }
        if let Some(v) = self.call_id.filter(|s| !s.is_empty()) {
            w.add("call_id LIKE ?", vec![BindValue::Text(format!("%{v}%"))]);
        }
        // 用 substr 做**精确**的前缀/后缀匹配：LIKE 里的 `_` 是通配符，
        // `dev_%` 会连 `devX...` 一起匹配上，不是我们想要的语义。
        if let Some(v) = self.device_id.filter(|s| !s.is_empty()) {
            w.add(
                "substr(stream, 1, length(?)) = ?",
                vec![BindValue::Text(v.to_string()), BindValue::Text(v.to_string())],
            );
        }
        if let Some(v) = self.channel_id.filter(|s| !s.is_empty()) {
            w.add(
                "substr(stream, length(stream) - length(?) + 1) = ?",
                vec![BindValue::Text(v.to_string()), BindValue::Text(v.to_string())],
            );
        }
        if let Some(v) = self.query.map(str::trim).filter(|s| !s.is_empty()) {
            let like = format!("%{v}%");
            w.add(
                "(file_name LIKE ? OR stream LIKE ? OR app LIKE ?)",
                vec![
                    BindValue::Text(like.clone()),
                    BindValue::Text(like.clone()),
                    BindValue::Text(like),
                ],
            );
        }
        if let Some(v) = self.start_time {
            // 与录像有交集：录像结束时间 >= 查询起点
            w.add(
                "COALESCE(end_time, start_time) >= ?",
                vec![BindValue::Big(v)],
            );
        }
        if let Some(v) = self.end_time {
            w.add("start_time <= ?", vec![BindValue::Big(v)]);
        }
        w
    }
}

/// 按筛选条件分页查询云端录像（行查询 + 计数共用同一套 WHERE）。
pub async fn list_filtered(
    pool: &Pool,
    f: &CloudRecordFilter<'_>,
    page: i64,
    count: i64,
) -> sqlx::Result<(Vec<CloudRecord>, i64)> {
    let w = f.to_where();
    let offset = (page.max(1) - 1) * count;

    const COLS: &str = "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
         server_id, file_name, folder, file_path, collect, file_size, time_len \
         FROM gb_cloud_record";
    const COUNT_BASE: &str = "SELECT COUNT(*) FROM gb_cloud_record";

    let limit_ph = if cfg!(feature = "postgres") {
        format!(" LIMIT ${} OFFSET ${}", w.binds.len() + 1, w.binds.len() + 2)
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql_rows = format!("{}{} ORDER BY start_time DESC{limit_ph}", w.sql(COLS), "");
    let mut q = sqlx::query_as::<_, CloudRecord>(&sql_rows);
    for b in &w.binds {
        q = match b {
            crate::dyn_where::BindValue::Text(v) => q.bind(v.as_str()),
            crate::dyn_where::BindValue::Int(v) => q.bind(*v),
            crate::dyn_where::BindValue::Big(v) => q.bind(*v),
            crate::dyn_where::BindValue::Bool(v) => q.bind(*v),
        };
    }
    let rows = q.bind(count).bind(offset).fetch_all(pool).await?;

    let count_sql = w.sql(COUNT_BASE);
    let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &w.binds {
        cq = match b {
            crate::dyn_where::BindValue::Text(v) => cq.bind(v.as_str()),
            crate::dyn_where::BindValue::Int(v) => cq.bind(*v),
            crate::dyn_where::BindValue::Big(v) => cq.bind(*v),
            crate::dyn_where::BindValue::Bool(v) => cq.bind(*v),
        };
    }
    let total = cq.fetch_one(pool).await?;
    Ok((rows, total))
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
            "SELECT COUNT(*) FROM gb_cloud_record \
             WHERE ($1::text IS NULL OR app = $1) \
               AND ($2::text IS NULL OR stream = $2) \
               AND ($3::text IS NULL OR media_server_id = $3) \
               AND ($4::bigint IS NULL OR start_time >= $4) \
               AND ($5::bigint IS NULL OR end_time <= $5)",
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
            "SELECT COUNT(*) FROM gb_cloud_record \
             WHERE (? IS NULL OR app = ?) \
               AND (? IS NULL OR stream = ?) \
               AND (? IS NULL OR media_server_id = ?) \
               AND (? IS NULL OR start_time >= ?) \
               AND (? IS NULL OR end_time <= ?)",
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

    #[cfg(feature = "sqlite")]
    {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM gb_cloud_record \
             WHERE (?1 IS NULL OR app = ?1) \
               AND (?2 IS NULL OR stream = ?2) \
               AND (?3 IS NULL OR media_server_id = ?3) \
               AND (?4 IS NULL OR start_time >= ?4) \
               AND (?5 IS NULL OR end_time <= ?5)"
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
}

/// 更新云端录像
pub async fn update(pool: &Pool, record: &CloudRecordUpdate) -> sqlx::Result<bool> {
    #[cfg(feature = "postgres")]
    {
        let result = sqlx::query(
            "UPDATE gb_cloud_record SET \
             end_time = COALESCE($2, end_time), \
             file_size = COALESCE($3, file_size), \
             time_len = COALESCE($4, time_len) \
             WHERE id = $1",
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
            "UPDATE gb_cloud_record SET \
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

    #[cfg(feature = "sqlite")]
    {
        let result = sqlx::query(
            "UPDATE gb_cloud_record SET \
             end_time = COALESCE(?, end_time), \
             file_size = COALESCE(?, file_size), \
             time_len = COALESCE(?, time_len) \
             WHERE id = ?",
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
        let result = sqlx::query("DELETE FROM gb_cloud_record WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("DELETE FROM gb_cloud_record WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "sqlite")]
    {
        let result = sqlx::query("DELETE FROM gb_cloud_record WHERE id = ?")
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
            "DELETE FROM gb_cloud_record WHERE id IN ({})",
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
            "DELETE FROM gb_cloud_record WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }

    #[cfg(feature = "sqlite")]
    {
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "DELETE FROM gb_cloud_record WHERE id IN ({})",
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
        let result = sqlx::query("UPDATE gb_cloud_record SET collect = $2 WHERE id = $1")
            .bind(id)
            .bind(collect)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "mysql")]
    {
        let result = sqlx::query("UPDATE gb_cloud_record SET collect = ? WHERE id = ?")
            .bind(collect)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(feature = "sqlite")]
    {
        let result = sqlx::query("UPDATE gb_cloud_record SET collect = ? WHERE id = ?")
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
        "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE call_id = $1"
    )
    .bind(call_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE call_id = ?"
    )
    .bind(call_id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE call_id = ?"
    )
    .bind(call_id)
    .fetch_optional(pool)
    .await;
}

/// 删除指定流的录像
pub async fn delete_by_app_stream(pool: &Pool, app: &str, stream: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_cloud_record WHERE app = $1 AND stream = $2")
        .bind(app)
        .bind(stream)
        .execute(pool)
        .await?;
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM gb_cloud_record WHERE app = ? AND stream = ?")
        .bind(app)
        .bind(stream)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("DELETE FROM gb_cloud_record WHERE app = ? AND stream = ?")
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
        "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE collect = true ORDER BY create_time DESC"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE collect = 1 ORDER BY create_time DESC"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE collect = 1 ORDER BY create_time DESC"
    )
    .fetch_all(pool)
    .await;
}

/// 删除指定时间之前的录像
pub async fn delete_before_time(pool: &Pool, before_time: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_cloud_record WHERE end_time < $1")
        .bind(before_time)
        .execute(pool)
        .await?;
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM gb_cloud_record WHERE end_time < ?")
        .bind(before_time)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("DELETE FROM gb_cloud_record WHERE end_time < ?")
        .bind(before_time)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// Phase 4.1: 从 ZLM on_record_file hook 插入录像记录
pub async fn insert_from_hook(
    pool: &Pool,
    stream_id: &str,
    file_path: &str,
    duration_secs: i64,
) -> sqlx::Result<i64> {
    let now = chrono::Utc::now().timestamp();
    let start = now - duration_secs;
    let record = CloudRecordInsert {
        app: "record".to_string(),
        stream: stream_id.to_string(),
        call_id: None,
        start_time: Some(start),
        end_time: Some(now),
        media_server_id: None,
        server_id: None,
        file_name: std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string()),
        folder: std::path::Path::new(file_path)
            .parent()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string()),
        file_path: Some(file_path.to_string()),
        file_size: None,
        time_len: Some(duration_secs as f64),
    };
    insert(pool, &record).await
}

/// ABL `on_record_progress` 钩子专用：按 stream_id 更新最新一次录像的
/// 累计 duration / file_size，供前端轮询实时展示。
pub async fn update_recording_progress(
    pool: &Pool,
    stream_id: &str,
    app: &str,
    current_duration_secs: f64,
    current_size_bytes: u64,
) -> sqlx::Result<u64> {
    let now = chrono::Utc::now().timestamp();
    let duration_secs = current_duration_secs as i64;
    #[cfg(feature = "postgres")]
    {
        sqlx::query(
            r#"UPDATE gb_cloud_record
               SET time_len = $3,
                   file_size = $4,
                   end_time = $5
               WHERE id = (
                   SELECT id FROM gb_cloud_record
                   WHERE stream = $1 AND app = $2
                   ORDER BY start_time DESC NULLS LAST
                   LIMIT 1
               )"#,
        )
        .bind(stream_id)
        .bind(app)
        .bind(duration_secs)
        .bind(current_size_bytes as i64)
        .bind(now)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
    }
    #[cfg(feature = "mysql")]
    {
        sqlx::query(
            r#"UPDATE gb_cloud_record
               SET time_len = ?, file_size = ?, end_time = ?
               WHERE id = (
                   SELECT id FROM (
                       SELECT id FROM gb_cloud_record
                       WHERE stream = ? AND app = ?
                       ORDER BY (start_time IS NULL), start_time DESC
                       LIMIT 1
                   ) AS t
               )"#,
        )
        .bind(duration_secs)
        .bind(current_size_bytes as i64)
        .bind(now)
        .bind(stream_id)
        .bind(app)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
    }
    #[cfg(feature = "sqlite")]
    {
        sqlx::query(
            r#"UPDATE gb_cloud_record
               SET time_len = ?, file_size = ?, end_time = ?
               WHERE id = (
                   SELECT id FROM gb_cloud_record
                   WHERE stream = ? AND app = ?
                   ORDER BY (start_time IS NULL), start_time DESC
                   LIMIT 1
               )"#,
        )
        .bind(duration_secs)
        .bind(current_size_bytes as i64)
        .bind(now)
        .bind(stream_id)
        .bind(app)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
    }
}

/// Phase 3.3: 把 RecordInfo 响应里的多段录像项写入 gb_cloud_record
///
/// `device_id` + `channel_id` 组合作为 stream 字段；
/// `start_time` / `end_time` 用 ISO 字符串解析为 epoch 秒。
/// 重复插入由 file_path + start_time 唯一索引去重（ON CONFLICT DO NOTHING）。
pub async fn insert_records(
    pool: &Pool,
    device_id: &str,
    channel_id: &str,
    _query_start: &str,
    _query_end: &str,
    items: &[crate::sip::server::RecordInfoItem],
) -> sqlx::Result<usize> {
    let stream = format!("{}/{}", device_id, channel_id);
    let mut inserted = 0usize;

    for item in items {
        let start_ts = item
            .start_time
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp())
            .or_else(|| {
                // 退化：尝试 GB28181 常见格式 "2026-06-10T10:00:00"
                chrono::NaiveDateTime::parse_from_str(item.start_time.as_deref()?, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .map(|nd| nd.and_utc().timestamp())
            });
        let end_ts = item
            .end_time
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp())
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(item.end_time.as_deref()?, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .map(|nd| nd.and_utc().timestamp())
            });
        let time_len = match (start_ts, end_ts) {
            (Some(s), Some(e)) if e >= s => Some((e - s) as f64),
            _ => None,
        };

        let file_name = item.name.clone().or_else(|| {
            item.file_path
                .as_deref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        });

        let record = CloudRecordInsert {
            app: "record_info".to_string(),
            stream: stream.clone(),
            call_id: None,
            start_time: start_ts,
            end_time: end_ts,
            media_server_id: None,
            server_id: None,
            file_name,
            folder: None,
            file_path: item.file_path.clone(),
            file_size: None,
            time_len,
        };

        if insert(pool, &record).await.is_ok() {
            inserted += 1;
        }
    }

    Ok(inserted)
}

/// Phase 3.3: 按 device_id + channel_id + 时间窗口分页查询录像
pub async fn query_by_device_channel(
    pool: &Pool,
    device_id: &str,
    channel_id: &str,
    start_time: Option<i64>,
    end_time: Option<i64>,
    page: i64,
    count: i64,
) -> sqlx::Result<Vec<CloudRecord>> {
    let stream = format!("{}/{}", device_id, channel_id);
    list_paged(
        pool,
        Some("record_info"),
        Some(&stream),
        None,
        start_time,
        end_time,
        page,
        count,
    )
    .await
}

/// 查最近一条 `stream` 中包含 `needle` 的录像记录（按 `end_time` 倒序取 1 条）。
///
/// 用于「设备/通道最近一次抓拍或录像」这类查询：GB28181 上报的 stream 名通常包含
/// 设备或通道编码（如 `34020000001320000001_34020000001310000001`），
/// 因此用子串匹配即可把设备与它的录像关联起来。
pub async fn find_latest_by_stream_like(
    pool: &Pool,
    needle: &str,
) -> sqlx::Result<Option<CloudRecord>> {
    let pattern = format!("%{}%", needle);
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT * FROM gb_cloud_record WHERE stream LIKE ? ORDER BY end_time DESC, id DESC LIMIT 1",
    )
    .bind(&pattern)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT * FROM gb_cloud_record WHERE stream LIKE $1 ORDER BY end_time DESC, id DESC LIMIT 1",
    )
    .bind(&pattern)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, CloudRecord>(
        "SELECT * FROM gb_cloud_record WHERE stream LIKE ? ORDER BY end_time DESC, id DESC LIMIT 1",
    )
    .bind(&pattern)
    .fetch_optional(pool)
    .await;
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::test_support::sqlite_pool_with_schema;

    async fn insert_rec(pool: &Pool, stream: &str, file_name: &str, end_time: i64) -> i64 {
        insert(
            pool,
            &CloudRecordInsert {
                app: "record".into(),
                stream: stream.into(),
                call_id: None,
                start_time: Some(end_time - 10),
                end_time: Some(end_time),
                media_server_id: None,
                server_id: None,
                file_name: Some(file_name.into()),
                folder: None,
                file_path: Some(format!("/tmp/{}", file_name)),
                file_size: Some(1),
                time_len: Some(10.0),
            },
        )
        .await
        .expect("insert record")
    }

    /// `find_latest_by_stream_like` 用于「设备最近一次抓拍/录像」查询（alarm_snap）。
    #[tokio::test]
    async fn test_find_latest_by_stream_like() {
        let pool = sqlite_pool_with_schema().await;
        let dev = "34020000001320000001";

        insert_rec(&pool, &format!("{}_34020000001310000001", dev), "older.mp4", 100).await;
        insert_rec(&pool, &format!("{}_34020000001310000002", dev), "newer.mp4", 200).await;
        // 另一个设备，不应命中
        insert_rec(&pool, "34020000001320000099_34020000001310000001", "other.mp4", 300).await;

        let latest = find_latest_by_stream_like(&pool, dev)
            .await
            .unwrap()
            .expect("应找到记录");
        assert_eq!(
            latest.file_name.as_deref(),
            Some("newer.mp4"),
            "应取 end_time 最大的那条"
        );

        // 无匹配必须是 None，而不是退化成「返回全部中随便一条」
        assert!(find_latest_by_stream_like(&pool, "no-such-device")
            .await
            .unwrap()
            .is_none());
    }
}
