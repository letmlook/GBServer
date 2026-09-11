//! 系统日志表 `gb_log`
//!
//! 2026-09-12 新增。此前 `handlers/stub.rs::log_list` 一直在查询 `gb_log`，但该表
//! **从未在任何 schema 中创建**，且查询错误被吞成空列表 —— 于是
//! `GET /api/log/list` 永远返回空，前端的「实时日志 / 历史日志」两个页面实质不可用。
//!
//! 前端 `historyLog.vue` 渲染的是日志**条目**（time / level / logger / message / thread），
//! 而旧实现返回的是**文件行**（name / type / create_time）—— 契约本身也不匹配。
//! 本模块按前端契约提供结构化日志的写入与查询。

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;

/// 一条系统日志
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub time: String,
    pub level: String,
    pub logger: Option<String>,
    pub thread: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
}

/// 采集层使用的待写入日志（无 id）
#[derive(Debug, Clone, Default)]
pub struct NewLogEntry {
    pub time: String,
    pub level: String,
    pub logger: Option<String>,
    pub thread: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
}

impl From<NewLogEntry> for LogEntry {
    fn from(n: NewLogEntry) -> Self {
        Self {
            id: 0,
            time: n.time,
            level: n.level,
            logger: n.logger,
            thread: n.thread,
            message: n.message,
            source: n.source,
        }
    }
}

/// 批量写入（后台 writer 每秒调用一次）。空批次直接返回。
pub async fn insert_batch(pool: &Pool, rows: &[NewLogEntry]) -> sqlx::Result<u64> {
    if rows.is_empty() {
        return Ok(0);
    }
    let mut affected = 0u64;
    // 逐条插入：日志表写入频率低（秒级批量），且可避免三种方言的批量语法差异
    for r in rows {
        let sql = if cfg!(feature = "postgres") {
            "INSERT INTO gb_log (time, level, logger, thread, message, source) VALUES ($1, $2, $3, $4, $5, $6)"
        } else {
            "INSERT INTO gb_log (time, level, logger, thread, message, source) VALUES (?, ?, ?, ?, ?, ?)"
        };
        affected += sqlx::query(sql)
            .bind(&r.time)
            .bind(&r.level)
            .bind(r.logger.as_deref())
            .bind(r.thread.as_deref())
            .bind(r.message.as_deref())
            .bind(r.source.as_deref())
            .execute(pool)
            .await?
            .rows_affected();
    }
    Ok(affected)
}

/// 分页查询日志，返回 `(总数, 当页数据)`。
///
/// 过滤条件：`query`（message 模糊）、`level`（等值）、`start` / `end`（time 闭区间）。
/// 方言占位符：postgres 用 `$n`，sqlite/mysql 用 `?`。
fn ph(i: usize) -> String {
    if cfg!(feature = "postgres") {
        format!("${}", i)
    } else {
        "?".to_string()
    }
}

/// 依据过滤条件拼出 `WHERE` 子句与绑定值。
/// `list_paged` 与 `export` 共用，避免两处条件写法漂移。
fn build_filter(
    query: Option<&str>,
    level: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
) -> (String, Vec<String>) {
    let mut conds: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    if let Some(q) = query.map(str::trim).filter(|s| !s.is_empty()) {
        // 子串匹配：必须带通配符，否则 LIKE 退化为等值比较
        binds.push(format!("%{}%", q));
        conds.push(format!("message LIKE {}", ph(binds.len())));
    }
    if let Some(l) = level.map(str::trim).filter(|s| !s.is_empty()) {
        binds.push(l.to_string());
        conds.push(format!("level = {}", ph(binds.len())));
    }
    if let Some(s) = start.map(str::trim).filter(|s| !s.is_empty()) {
        binds.push(s.to_string());
        conds.push(format!("time >= {}", ph(binds.len())));
    }
    if let Some(e) = end.map(str::trim).filter(|s| !s.is_empty()) {
        binds.push(e.to_string());
        conds.push(format!("time <= {}", ph(binds.len())));
    }
    let where_clause = if conds.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conds.join(" AND "))
    };
    (where_clause, binds)
}

/// 导出过滤后的日志（按时间正序，供下载）。
///
/// `limit` 上限 50000：导出是给人看的诊断文件，不是数据迁移通道；
/// 无上限会让一次请求把整张表读进内存。
pub async fn export(
    pool: &Pool,
    query: Option<&str>,
    level: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    limit: u32,
) -> sqlx::Result<Vec<LogEntry>> {
    let (where_clause, binds) = build_filter(query, level, start, end);
    let limit = limit.clamp(1, 50_000);
    // 正序：日志文件按时间阅读
    let sql = format!(
        "SELECT id, time, level, logger, thread, message, source FROM gb_log{} ORDER BY id ASC LIMIT {}",
        where_clause,
        ph(binds.len() + 1)
    );
    let mut q = sqlx::query_as::<_, LogEntry>(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    q.bind(limit as i64).fetch_all(pool).await
}

pub async fn list_paged(
    pool: &Pool,
    query: Option<&str>,
    level: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    page: u32,
    count: u32,
) -> sqlx::Result<(i64, Vec<LogEntry>)> {
    let page = page.max(1);
    let count = count.clamp(1, 500);
    let offset = ((page - 1) * count) as i64;

    let (where_clause, binds) = build_filter(query, level, start, end);

    let count_sql = format!("SELECT COUNT(*) FROM gb_log{}", where_clause);
    let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &binds {
        cq = cq.bind(b);
    }
    let total = cq.fetch_one(pool).await?;

    let list_sql = format!(
        "SELECT id, time, level, logger, thread, message, source FROM gb_log{} ORDER BY id DESC LIMIT {} OFFSET {}",
        where_clause,
        ph(binds.len() + 1),
        ph(binds.len() + 2)
    );
    let mut lq = sqlx::query_as::<_, LogEntry>(&list_sql);
    for b in &binds {
        lq = lq.bind(b);
    }
    let list = lq
        .bind(count as i64)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok((total, list))
}

/// 保留策略：删除 `time < before` 的日志，返回删除行数。
pub async fn delete_before(pool: &Pool, before: &str) -> sqlx::Result<u64> {
    let sql = if cfg!(feature = "postgres") {
        "DELETE FROM gb_log WHERE time < $1"
    } else {
        "DELETE FROM gb_log WHERE time < ?"
    };
    Ok(sqlx::query(sql)
        .bind(before)
        .execute(pool)
        .await?
        .rows_affected())
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::test_support::sqlite_pool_with_schema;

    fn entry(time: &str, level: &str, msg: &str) -> NewLogEntry {
        NewLogEntry {
            time: time.into(),
            level: level.into(),
            logger: Some("gbserver::test".into()),
            thread: Some("t1".into()),
            message: Some(msg.into()),
            source: None,
        }
    }

    #[tokio::test]
    async fn test_insert_and_query_with_filters() {
        let pool = sqlite_pool_with_schema().await;
        insert_batch(
            &pool,
            &[
                entry("2026-01-01 10:00:00", "INFO", "device registered"),
                entry("2026-01-01 10:00:01", "WARN", "stream reconnect"),
                entry("2026-01-01 10:00:02", "ERROR", "invite failed"),
            ],
        )
        .await
        .expect("日志写入应成功");

        let (total, all) = list_paged(&pool, None, None, None, None, 1, 10).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(all.len(), 3);
        // 倒序：最新在前
        assert_eq!(all[0].level, "ERROR");

        // 按级别过滤
        let (n, rows) = list_paged(&pool, None, Some("WARN"), None, None, 1, 10).await.unwrap();
        assert_eq!(n, 1);
        assert_eq!(rows[0].message.as_deref(), Some("stream reconnect"));

        // 按关键字过滤
        let (n, _) = list_paged(&pool, Some("invite"), None, None, None, 1, 10).await.unwrap();
        assert_eq!(n, 1);

        // 时间范围过滤
        let (n, _) = list_paged(
            &pool,
            None,
            None,
            Some("2026-01-01 10:00:01"),
            Some("2026-01-01 10:00:01"),
            1,
            10,
        )
        .await
        .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn test_pagination_and_retention() {
        let pool = sqlite_pool_with_schema().await;
        let rows: Vec<NewLogEntry> = (0..5)
            .map(|i| entry(&format!("2026-01-01 10:00:0{}", i), "INFO", &format!("m{}", i)))
            .collect();
        insert_batch(&pool, &rows).await.unwrap();

        let (total, page1) = list_paged(&pool, None, None, None, None, 1, 2).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 2);
        let (_, page3) = list_paged(&pool, None, None, None, None, 3, 2).await.unwrap();
        assert_eq!(page3.len(), 1);

        // 保留策略：删除 10:00:03 之前的记录（应删 3 条）
        let removed = delete_before(&pool, "2026-01-01 10:00:03").await.unwrap();
        assert_eq!(removed, 3);
        let (total, _) = list_paged(&pool, None, None, None, None, 1, 10).await.unwrap();
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_empty_batch_is_noop() {
        let pool = sqlite_pool_with_schema().await;
        assert_eq!(insert_batch(&pool, &[]).await.unwrap(), 0);
    }
}
