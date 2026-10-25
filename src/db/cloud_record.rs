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
    if pool.dialect().is_mysql() {
        let sql = pool
            .sql(
                "INSERT INTO gb_cloud_record \
                     (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
                      file_name, folder, file_path, file_size, time_len) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .into_owned();

        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
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
                .execute(p)
        })?;

        Ok(result.last_insert_id().unwrap_or(0) as i64)
    } else if pool.dialect().is_postgres() {
        let sql = pool
            .sql(
                "INSERT INTO gb_cloud_record \
                     (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
                      file_name, folder, file_path, file_size, time_len) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
                     RETURNING id",
            )
            .into_owned();

        let result: (i64,) = crate::dispatch!(pool, |p| {
            sqlx::query_as(&sql)
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
                .fetch_one(p)
        })?;

        Ok(result.0)
    } else {
        let sql = pool
            .sql(
                "INSERT INTO gb_cloud_record \
                     (app, stream, call_id, start_time, end_time, media_server_id, server_id, \
                      file_name, folder, file_path, file_size, time_len) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .into_owned();

        let result = crate::dispatch_exec!(pool, |p| {
            sqlx::query(&sql)
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
                .execute(p)
        })?;

        Ok(result.last_insert_id().unwrap_or(0) as i64)
    }
}

/// 根据ID查询云端录像
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<CloudRecord>> {
    let sql = pool
        .sql(
            "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
             server_id, file_name, folder, file_path, collect, file_size, time_len \
             FROM gb_cloud_record WHERE id = ?",
        )
        .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, CloudRecord>(&sql)
            .bind(id)
            .fetch_optional(p)
    });
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
    let offset = crate::pagination::offset_i64(page, count);

    // 三个方言的**占位符数量不同**（这决定 bind 链的长度）：
    // * PostgreSQL：`$1` 复用 —— 「`$1 IS NULL` 或 `app = $1`」共用同一个参数（7 个 bind）；
    // * MySQL：每个条件两个 `?` —— 「`? IS NULL` 或 `app = ?`」是两个参数（12 个 bind）；
    // * SQLite：`?1` 编号占位符，同样复用参数（7 个 bind）。
    //
    // 所以这三个分支必须各带自己的 SQL 与 bind 链，不能合成一条。
    crate::dispatch_dialect!(
        pool,
        |p| {
            let sql = pool.sql(
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
            );
            sqlx::query_as::<_, CloudRecord>(&sql)
                .bind(app)
                .bind(stream)
                .bind(media_server_id)
                .bind(start_time)
                .bind(end_time)
                .bind(count)
                .bind(offset)
                .fetch_all(p)
                .await
        },
        |p| {
            let sql = pool.sql(
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
            );
            sqlx::query_as::<_, CloudRecord>(&sql)
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
                .bind(offset)
                .fetch_all(p)
                .await
        },
        |p| {
            let sql = pool.sql(
                "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
                 server_id, file_name, folder, file_path, collect, file_size, time_len \
                 FROM gb_cloud_record \
                 WHERE (?1 IS NULL OR app = ?1) \
                   AND (?2 IS NULL OR stream = ?2) \
                   AND (?3 IS NULL OR media_server_id = ?3) \
                   AND (?4 IS NULL OR start_time >= ?4) \
                   AND (?5 IS NULL OR end_time <= ?5) \
                 ORDER BY start_time DESC \
                 LIMIT ?6 OFFSET ?7",
            );
            sqlx::query_as::<_, CloudRecord>(&sql)
                .bind(app)
                .bind(stream)
                .bind(media_server_id)
                .bind(start_time)
                .bind(end_time)
                .bind(count)
                .bind(offset)
                .fetch_all(p)
                .await
        },
    )
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
        //
        // **必须带分隔符边界**，但分隔符**不止 `_` 一个**。
        //
        // 仓库往 `gb_cloud_record.stream` 写的流名有**四种**形状
        // （见 `sip/gb28181/stream_reconnect.rs` 的 `parse_stream_id` —— 那是
        // 仓库自称"全局唯一"的流名解析器，它认的分隔符集合就是 `_` / `$` / `/`）：
        //   * `{device}_{channel}` —— `handlers/play.rs` 的 `openRtpServer` 规范写法；
        //   * `{device}/{channel}` —— `db/cloud_record.rs::insert_records` 落
        //     `record_info` 时用的（设备通过 GB28181 RecordInfo 上报的历史录像，
        //     **活路径**：`sip/server.rs` 应答 RecordInfo 查询时调用它）；
        //   * `{device}${channel}` —— 部分早期实现 / 上级平台下发的写法；
        //   * `{phone}` —— JT1078 通道号为 0 时**没有分隔符**
        //     （`handlers/jt1078.rs`）。
        //
        // 边界只认 `_` 的话，后两种形状会被**静默整段过滤掉** —— 平台 UI 的
        // "设备/通道"输入框和第三方对接方的 list-url 都会"少查数据但总数看着正常"，
        // 全程无报错。
        //
        // 分隔符集合用**字面量** IN 列表，不新增占位符（省得三个方言的
        // 占位符改写再出一次幺蛾子）。
        if let Some(v) = self.device_id.filter(|s| !s.is_empty()) {
            w.add(
                "(substr(stream, 1, length(?)) = ? AND substr(stream, length(?) + 1, 1) IN ('_', '$', '/')) \
                 OR stream = ?",
                vec![
                    BindValue::Text(v.to_string()),
                    BindValue::Text(v.to_string()),
                    BindValue::Text(v.to_string()),
                    BindValue::Text(v.to_string()),
                ],
            );
        }
        // 通道号是流名的**最后一段**，同理要求它前面紧邻分隔符，
        // 否则短通道号会命中以它结尾的别的通道（"…_11" 被 "…_1" 命中）。
        // 纯号码流（JT1078 通道号=0）没有"最后一段"，不参与通道过滤 —— 正确。
        if let Some(v) = self.channel_id.filter(|s| !s.is_empty()) {
            w.add(
                "substr(stream, length(stream) - length(?), 1) IN ('_', '$', '/') \
                 AND substr(stream, length(stream) - length(?) + 1) = ?",
                vec![
                    BindValue::Text(v.to_string()),
                    BindValue::Text(v.to_string()),
                    BindValue::Text(v.to_string()),
                ],
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
    let offset = crate::pagination::offset_i64(page, count);

    const COLS: &str = "SELECT id, app, stream, call_id, start_time, end_time, media_server_id, \
         server_id, file_name, folder, file_path, collect, file_size, time_len \
         FROM gb_cloud_record";
    const COUNT_BASE: &str = "SELECT COUNT(*) FROM gb_cloud_record";

    let limit_ph = if pool.dialect().is_postgres() {
        format!(
            " LIMIT ${} OFFSET ${}",
            w.binds.len() + 1,
            w.binds.len() + 2
        )
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql_rows = format!("{}{} ORDER BY start_time DESC{limit_ph}", w.sql(COLS), "");
    let rows = crate::dispatch!(pool, |p| {
        let mut q = sqlx::query_as::<_, CloudRecord>(&sql_rows);
        for b in &w.binds {
            q = match b {
                crate::dyn_where::BindValue::Text(v) => q.bind(v.as_str()),
                crate::dyn_where::BindValue::Int(v) => q.bind(*v),
                crate::dyn_where::BindValue::Big(v) => q.bind(*v),
                crate::dyn_where::BindValue::Bool(v) => q.bind(*v),
            };
        }
        q.bind(count).bind(offset).fetch_all(p)
    })?;

    let count_sql = w.sql(COUNT_BASE);
    let total = crate::dispatch!(pool, |p| {
        let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);
        for b in &w.binds {
            cq = match b {
                crate::dyn_where::BindValue::Text(v) => cq.bind(v.as_str()),
                crate::dyn_where::BindValue::Int(v) => cq.bind(*v),
                crate::dyn_where::BindValue::Big(v) => cq.bind(*v),
                crate::dyn_where::BindValue::Bool(v) => cq.bind(*v),
            };
        }
        cq.fetch_one(p)
    })?;
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
    let sql = match pool.dialect() {
        crate::db::Dialect::MySql => pool
            .sql(
                "SELECT COUNT(*) FROM gb_cloud_record \
             WHERE (? IS NULL OR app = ?) \
               AND (? IS NULL OR stream = ?) \
               AND (? IS NULL OR media_server_id = ?) \
               AND (? IS NULL OR start_time >= ?) \
               AND (? IS NULL OR end_time <= ?)",
            )
            .into_owned(),
        crate::db::Dialect::Sqlite => pool
            .sql(
                "SELECT COUNT(*) FROM gb_cloud_record \
             WHERE (?1 IS NULL OR app = ?1) \
               AND (?2 IS NULL OR stream = ?2) \
               AND (?3 IS NULL OR media_server_id = ?3) \
               AND (?4 IS NULL OR start_time >= ?4) \
               AND (?5 IS NULL OR end_time <= ?5)",
            )
            .into_owned(),
        _ => pool
            .sql(
                "SELECT COUNT(*) FROM gb_cloud_record \
             WHERE ($1::text IS NULL OR app = $1) \
               AND ($2::text IS NULL OR stream = $2) \
               AND ($3::text IS NULL OR media_server_id = $3) \
               AND ($4::bigint IS NULL OR start_time >= $4) \
               AND ($5::bigint IS NULL OR end_time <= $5)",
            )
            .into_owned(),
    };

    let result: (i64,) = crate::dispatch!(pool, |p| {
        sqlx::query_as(&sql)
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
            .fetch_one(p)
    })?;

    Ok(result.0)
}

/// 更新云端录像
pub async fn update(pool: &Pool, record: &CloudRecordUpdate) -> sqlx::Result<bool> {
    let sql = pool
        .sql(
            "UPDATE gb_cloud_record SET \
             end_time = COALESCE(?, end_time), \
             file_size = COALESCE(?, file_size), \
             time_len = COALESCE(?, time_len) \
             WHERE id = ?",
        )
        .into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(record.end_time)
            .bind(record.file_size)
            .bind(record.time_len)
            .bind(record.id)
            .execute(p)
    })?;

    Ok(result.rows_affected() > 0)
}

/// 删除云端录像
pub async fn delete(pool: &Pool, id: i64) -> sqlx::Result<bool> {
    let sql = pool
        .sql("DELETE FROM gb_cloud_record WHERE id = ?")
        .into_owned();

    let result = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;

    Ok(result.rows_affected() > 0)
}

/// 批量删除云端录像
pub async fn batch_delete(pool: &Pool, ids: &[i64]) -> sqlx::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }

    // 同 `batch_delete_alarms`：构造在宏外，执行在宏内。
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!(
        "DELETE FROM gb_cloud_record WHERE id IN ({})",
        placeholders.join(", ")
    );
    let result = crate::dispatch_exec!(pool, |p| {
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.execute(p)
    })?;
    Ok(result.rows_affected())
}

/// 设置/取消收藏
pub async fn set_collect(pool: &Pool, id: i64, collect: bool) -> sqlx::Result<bool> {
    let sql = pool
        .sql("UPDATE gb_cloud_record SET collect = ? WHERE id = ?")
        .into_owned();

    let result = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(collect).bind(id).execute(p)
    })?;

    Ok(result.rows_affected() > 0)
}

/// 根据 call_id 查询录像
pub async fn get_by_call_id(pool: &Pool, call_id: &str) -> sqlx::Result<Option<CloudRecord>> {
    let sql = pool.sql("SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE call_id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, CloudRecord>(&sql)
            .bind(call_id)
            .fetch_optional(p)
    });
}

/// 删除指定流的录像
pub async fn delete_by_app_stream(pool: &Pool, app: &str, stream: &str) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_cloud_record WHERE app = ? AND stream = ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(app).bind(stream).execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 删除"某条流 + 某个日期目录"下的**全部**录像记录。
///
/// 为什么需要它：ZLM 的 `deleteRecordDirectory` 是**按目录**删的 —— 只给
/// `vhost/app/stream` 就会删掉该流的整个录像目录；**即使传了 `period` 与
/// `file_name`，实测依然把该日期目录下的所有 mp4 一起删掉**
/// （响应里回的就是目录路径）。因此平台侧如果只删被点的那一行，
/// 同目录下的其它行就成了"列表里还在、点开必 404"的孤儿记录。
pub async fn delete_by_app_stream_period(
    pool: &Pool,
    app: &str,
    stream: &str,
    period: &str,
) -> sqlx::Result<u64> {
    let name_like = format!("{period}%");
    let path_like = format!("%/{period}/%");
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&pool.sql(
            "DELETE FROM gb_cloud_record WHERE app = ? AND stream = ? \
         AND (file_name LIKE ? OR file_path LIKE ?)",
        ))
        .bind(app)
        .bind(stream)
        .bind(&name_like)
        .bind(&path_like)
        .execute(p)
    })?;
    Ok(r.rows_affected())
}

/// 获取收藏的录像
pub async fn get_collect_records(pool: &Pool) -> sqlx::Result<Vec<CloudRecord>> {
    let sql = match pool.dialect() {
        crate::db::Dialect::Postgres => pool.sql("SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE collect = true ORDER BY create_time DESC").into_owned(),
        _ => pool.sql("SELECT id, app, stream, call_id, start_time, end_time, media_server_id, file_name, file_path, file_size, collect, server_id, folder, time_len FROM gb_cloud_record WHERE collect = 1 ORDER BY create_time DESC").into_owned(),
    };
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, CloudRecord>(&sql).fetch_all(p)
    });
}

/// 删除指定时间之前的录像
pub async fn delete_before_time(pool: &Pool, before_time: i64) -> sqlx::Result<u64> {
    let sql = pool
        .sql("DELETE FROM gb_cloud_record WHERE end_time < ?")
        .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(before_time).execute(p) })?;
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
    let sql = match pool.dialect() {
        crate::db::Dialect::MySql => pool
            .sql(
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
            .into_owned(),
        crate::db::Dialect::Sqlite => pool
            .sql(
                r#"UPDATE gb_cloud_record
               SET time_len = ?, file_size = ?, end_time = ?
               WHERE id = (
                   SELECT id FROM gb_cloud_record
                   WHERE stream = ? AND app = ?
                   ORDER BY (start_time IS NULL), start_time DESC
                   LIMIT 1
               )"#,
            )
            .into_owned(),
        _ => pool
            .sql(
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
            .into_owned(),
    };
    crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(duration_secs)
            .bind(current_size_bytes as i64)
            .bind(now)
            .bind(stream_id)
            .bind(app)
            .execute(p)
    })
    .map(|r| r.rows_affected())
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
                chrono::NaiveDateTime::parse_from_str(
                    item.start_time.as_deref()?,
                    "%Y-%m-%dT%H:%M:%S",
                )
                .ok()
                .map(|nd| nd.and_utc().timestamp())
            });
        let end_ts = item
            .end_time
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp())
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(
                    item.end_time.as_deref()?,
                    "%Y-%m-%dT%H:%M:%S",
                )
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
    let sql = pool.sql(
        "SELECT * FROM gb_cloud_record WHERE stream LIKE ? ORDER BY end_time DESC, id DESC LIMIT 1",
    )
    .into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, CloudRecord>(&sql)
            .bind(&pattern)
            .fetch_optional(p)
    });
}

#[cfg(test)]
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

    /// ZLM 的 `deleteRecordDirectory` 是**按目录**删的（传 file_name 也删整个日期目录），
    /// 所以平台必须把同 (app, stream, 日期) 下的记录一起删；别的日期目录不受影响。
    #[tokio::test]
    async fn delete_by_app_stream_period_only_touches_that_date() {
        let pool = sqlite_pool_with_schema().await;
        insert_rec(&pool, "ch1", "2026-09-14-10-00-00-0.mp4", 10).await;
        insert_rec(&pool, "ch1", "2026-09-14-10-05-00-0.mp4", 20).await;
        insert_rec(&pool, "ch1", "2026-09-15-09-00-00-0.mp4", 30).await;

        let n = delete_by_app_stream_period(&pool, "record", "ch1", "2026-09-14")
            .await
            .expect("按目录删除");
        assert_eq!(n, 2, "同一天的两条应一起删除");

        let left: Vec<String> = crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, String>("SELECT file_name FROM gb_cloud_record ORDER BY id")
                .fetch_all(p)
        })
        .unwrap();
        assert_eq!(left, vec!["2026-09-15-09-00-00-0.mp4".to_string()]);
    }

    /// `find_latest_by_stream_like` 用于「设备最近一次抓拍/录像」查询（alarm_snap）。
    #[tokio::test]
    async fn test_find_latest_by_stream_like() {
        let pool = sqlite_pool_with_schema().await;
        let dev = "34020000001320000001";

        insert_rec(
            &pool,
            &format!("{}_34020000001310000001", dev),
            "older.mp4",
            100,
        )
        .await;
        insert_rec(
            &pool,
            &format!("{}_34020000001310000002", dev),
            "newer.mp4",
            200,
        )
        .await;
        // 另一个设备，不应命中
        insert_rec(
            &pool,
            "34020000001320000099_34020000001310000001",
            "other.mp4",
            300,
        )
        .await;

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

    /// 回归：按设备/通道过滤必须**带分隔符边界**，但分隔符**不止 `_`**。
    ///
    /// 此前是纯前缀/后缀匹配：19 位的截断设备号会同时命中
    /// `…_34020000001320000001` 和 `…_34020000001320000002` 两条**不同设备**的
    /// 录像 —— 静默多查，对接方拿到的是别人的录像。
    ///
    /// 而如果边界只认 `_`，仓库**自己**写的另外几种流名形状
    /// （`{dev}/{ch}` 的 record_info、`{dev}${ch}`、JT1078 的纯 `{phone}`）
    /// 又会被**整段静默过滤掉** —— 少查数据但总数看着正常，全程无报错。
    /// 两种错法都要挡住，所以这里把四种形状都测一遍。
    #[tokio::test]
    async fn device_and_channel_filters_respect_separator_boundary() {
        let pool = sqlite_pool_with_schema().await;
        let dev_a = "34020000001320000001";
        let dev_b = "34020000001320000002";
        let ch_a = "34020000001310000001";
        let ch_b = "34020000001310000002";

        // 四种形状各来一条（分属两个设备）
        insert_rec(&pool, &format!("{dev_a}_{ch_a}"), "underscore.mp4", 10).await;
        insert_rec(&pool, &format!("{dev_a}/{ch_a}"), "slash.mp4", 20).await;
        insert_rec(&pool, &format!("{dev_a}${ch_a}"), "dollar.mp4", 30).await;
        insert_rec(&pool, &format!("{dev_b}_{ch_b}"), "other-device.mp4", 40).await;
        // JT1078：通道号为 0 时没有分隔符，流名就是手机号本身
        insert_rec(&pool, "13800138000", "jt-no-sep.mp4", 50).await;

        // 完整设备号：命中它的全部三种分隔符形状 + 别的设备不命中
        let (rows, _t) = list_filtered(
            &pool,
            &CloudRecordFilter {
                device_id: Some(dev_a),
                ..Default::default()
            },
            0,
            15,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 3, "{rows:?}");
        let names: Vec<&str> = rows.iter().filter_map(|r| r.file_name.as_deref()).collect();
        assert!(names.contains(&"underscore.mp4"), "{names:?}");
        assert!(names.contains(&"slash.mp4"), "{names:?}");
        assert!(names.contains(&"dollar.mp4"), "{names:?}");

        // 纯号码流（JT1078）：设备过滤要能命中它自己
        let (rows, _t) = list_filtered(
            &pool,
            &CloudRecordFilter {
                device_id: Some("13800138000"),
                ..Default::default()
            },
            0,
            15,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].file_name.as_deref(), Some("jt-no-sep.mp4"));

        // **截断**的设备号（19 位）不得命中两个设备
        let prefix = &dev_a[..19];
        let (rows, _t) = list_filtered(
            &pool,
            &CloudRecordFilter {
                device_id: Some(prefix),
                ..Default::default()
            },
            0,
            15,
        )
        .await
        .unwrap();
        assert!(rows.is_empty(), "截断的设备号不得命中别的设备: {rows:?}");

        // 完整通道号：三种分隔符形状都要命中
        let (rows, _t) = list_filtered(
            &pool,
            &CloudRecordFilter {
                channel_id: Some(ch_a),
                ..Default::default()
            },
            0,
            15,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 3, "{rows:?}");

        // **截断**的通道号（末 19 位）不得命中 —— 它也是流名的后缀，
        // 但它前面紧邻的不是分隔符，说明那不是完整的最后一段。
        let ch_tail = &ch_a[1..];
        let (rows, _t) = list_filtered(
            &pool,
            &CloudRecordFilter {
                channel_id: Some(ch_tail),
                ..Default::default()
            },
            0,
            15,
        )
        .await
        .unwrap();
        assert!(rows.is_empty(), "截断的通道号不得命中: {rows:?}");
    }
}
