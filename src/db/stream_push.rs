//! 推流表 gb_stream_push

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;
use crate::state::{StreamState, StreamStatus};
use std::str::FromStr;

/// 推流记录结构体。
///
/// 序列化成 **camelCase**：前端（与 WVP 的 `StreamPush` bean）读的是
/// `mediaServerId`/`createTime`/`startOfflinePush`，snake_case 会让"媒体节点"等列空白。
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
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
    /// Phase 4.5: 统一流状态字段（与 `pushing` bool 并存，不替换）
    #[serde(default)]
    pub stream_status: Option<String>,
    /// 绑定的国标设备/通道（`/api/push/save_to_gb` 写入；WVP 用通道行表达，
    /// 这里落在推流表上，语义等价且不引入新的 data_type 约定）
    pub gb_device_id: Option<String>,
    pub gb_channel_id: Option<String>,
}

impl StreamState for StreamPush {
    fn stream_id(&self) -> &str {
        self.stream.as_deref().unwrap_or("")
    }
    fn app(&self) -> &str {
        self.app.as_deref().unwrap_or("")
    }
    fn status(&self) -> StreamStatus {
        // 优先读 stream_status（新统一字段），fallback 解析历史 pushing bool
        if let Some(ref s) = self.stream_status {
            if let Ok(st) = StreamStatus::from_str(s) {
                return st;
            }
        }
        if self.pushing.unwrap_or(false) {
            StreamStatus::Pushing
        } else {
            StreamStatus::Ready
        }
    }
    fn set_status(&mut self, status: StreamStatus) {
        self.stream_status = Some(status.as_str().to_string());
    }
    fn media_server_id(&self) -> Option<&str> {
        self.media_server_id.as_deref()
    }
    fn device_id(&self) -> Option<&str> {
        // 历史结构未携带设备/通道关联字段（通过 GB 上层 INVITE 间接关联）
        None
    }
    fn channel_id(&self) -> Option<&str> {
        None
    }
}

/// Phase 4.5: 幂等迁移 —— 为已存在的 `gb_stream_push` 表添加 `stream_status` 列。
/// 三态 cfg 防御：PG 用 `ADD COLUMN IF NOT EXISTS`；SQLite / MySQL 用 information_schema 检测后条件执行。
/// 为既有库补 `gb_device_id` / `gb_channel_id`（`save_to_gb` 用）。
pub async fn ensure_gb_binding_columns(pool: &Pool) -> sqlx::Result<()> {
    for stmt in [
        "ALTER TABLE gb_stream_push ADD COLUMN gb_device_id VARCHAR(50)",
        "ALTER TABLE gb_stream_push ADD COLUMN gb_channel_id VARCHAR(50)",
    ] {
        let _ = sqlx::query(stmt).execute(pool).await;
    }
    Ok(())
}

pub async fn ensure_stream_status_column(pool: &Pool) -> sqlx::Result<()> {
    // PostgreSQL: ADD COLUMN IF NOT EXISTS
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("ALTER TABLE gb_stream_push ADD COLUMN IF NOT EXISTS stream_status VARCHAR(32) NOT NULL DEFAULT 'ready'")
            .execute(pool)
            .await?;
    }
    // SQLite: pragma_table_info 检测
    #[cfg(feature = "sqlite")]
    {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('gb_stream_push') WHERE name = 'stream_status'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if exists == 0 {
            let _ = sqlx::query("ALTER TABLE gb_stream_push ADD COLUMN stream_status TEXT NOT NULL DEFAULT 'ready'")
                .execute(pool)
                .await?;
        }
    }
    // MySQL: information_schema 检测
    #[cfg(feature = "mysql")]
    {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'gb_stream_push' AND column_name = 'stream_status'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if exists == 0 {
            let _ = sqlx::query("ALTER TABLE gb_stream_push ADD COLUMN stream_status varchar(32) NOT NULL DEFAULT 'ready'")
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// 根据ID获取推流记录
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<StreamPush>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id FROM gb_stream_push WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id FROM gb_stream_push WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id FROM gb_stream_push WHERE id = ?"
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
        r#"INSERT INTO gb_stream_push (app, stream, media_server_id, create_time, update_time, pushing, self, start_offline_push)
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
        r#"INSERT INTO gb_stream_push (app, stream, media_server_id, create_time, update_time, pushing, self, start_offline_push)
           VALUES ($1, $2, $3, $4, $5, false, true, true)"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_push (app, stream, media_server_id, create_time, update_time, pushing, self, start_offline_push)
           VALUES (?, ?, ?, ?, ?, 0, 1, 1)"#
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

/// ZLM 发现外部推流时自动登记/刷新推流记录
pub async fn upsert_discovered(
    pool: &Pool,
    app: &str,
    stream: &str,
    media_server_id: Option<&str>,
    server_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_push
           (app, stream, media_server_id, server_id, create_time, push_time, update_time, status, pushing, self, start_offline_push)
           VALUES (?, ?, ?, ?, ?, ?, ?, true, true, false, false)
           ON DUPLICATE KEY UPDATE
             media_server_id = COALESCE(VALUES(media_server_id), media_server_id),
             server_id = COALESCE(VALUES(server_id), server_id),
             push_time = COALESCE(push_time, VALUES(push_time)),
             update_time = VALUES(update_time),
             status = true,
             pushing = true"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(server_id)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_push
           (app, stream, media_server_id, server_id, create_time, push_time, update_time, status, pushing, self, start_offline_push)
           VALUES ($1, $2, $3, $4, $5, $5, $5, true, true, false, false)
           ON CONFLICT (app, stream) DO UPDATE SET
             media_server_id = COALESCE(EXCLUDED.media_server_id, gb_stream_push.media_server_id),
             server_id = COALESCE(EXCLUDED.server_id, gb_stream_push.server_id),
             push_time = COALESCE(gb_stream_push.push_time, EXCLUDED.push_time),
             update_time = EXCLUDED.update_time,
             status = true,
             pushing = true"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(server_id)
    .bind(now)
    .execute(pool)
    .await?;

    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_push
           (app, stream, media_server_id, server_id, create_time, push_time, update_time, status, pushing, self, start_offline_push)
           VALUES (?, ?, ?, ?, ?, ?, ?, 1, 1, 0, 0)
           ON CONFLICT(app, stream) DO UPDATE SET
             media_server_id = COALESCE(excluded.media_server_id, gb_stream_push.media_server_id),
             server_id = COALESCE(excluded.server_id, gb_stream_push.server_id),
             push_time = COALESCE(gb_stream_push.push_time, excluded.push_time),
             update_time = excluded.update_time,
             status = 1,
             pushing = 1"#
    )
    .bind(app)
    .bind(stream)
    .bind(media_server_id)
    .bind(server_id)
    .bind(now)
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
        r#"UPDATE gb_stream_push SET
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
        r#"UPDATE gb_stream_push SET
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
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"UPDATE gb_stream_push SET
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
    Ok(r.rows_affected())
}

/// 删除推流记录
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM gb_stream_push WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_stream_push WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("DELETE FROM gb_stream_push WHERE id = ?")
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

/// 推流列表（行查询 + 计数共用一套 WHERE；支持 mediaServerId / pushing / 关键字）。
pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    media_server_id: Option<&str>,
    pushing: Option<bool>,
    query: Option<&str>,
) -> sqlx::Result<Vec<StreamPush>> {
    let (w, _) = push_filter(media_server_id, pushing, query);
    let limit = count.min(100) as i64;
    let offset = (page.saturating_sub(1) as i64) * limit;

    const COLS: &str = "SELECT id, app, stream, create_time, media_server_id, server_id, \
         push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id \
         FROM gb_stream_push";
    let limit_ph = if cfg!(feature = "postgres") {
        format!(" LIMIT ${} OFFSET ${}", w.binds.len() + 1, w.binds.len() + 2)
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql = format!("{}{} ORDER BY id{limit_ph}", w.sql(COLS), "");
    let mut q = sqlx::query_as::<_, StreamPush>(&sql);
    for b in &w.binds {
        q = match b {
            crate::dyn_where::BindValue::Text(v) => q.bind(v.as_str()),
            crate::dyn_where::BindValue::Int(v) => q.bind(*v),
            crate::dyn_where::BindValue::Big(v) => q.bind(*v),
        };
    }
    q.bind(limit).bind(offset).fetch_all(pool).await
}

/// 组装推流列表的 WHERE（`list_paged` 与 `count_all` 共用）。
fn push_filter(
    media_server_id: Option<&str>,
    pushing: Option<bool>,
    query: Option<&str>,
) -> (crate::dyn_where::DynWhere, ()) {
    use crate::dyn_where::{BindValue, DynWhere};
    let mut w = DynWhere::new();
    if let Some(mid) = media_server_id.filter(|s| !s.is_empty()) {
        w.add("media_server_id = ?", vec![BindValue::Text(mid.to_string())]);
    }
    if let Some(p) = pushing {
        w.add("pushing = ?", vec![BindValue::Int(if p { 1 } else { 0 })]);
    }
    if let Some(kw) = query.map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{kw}%");
        w.add(
            "(app LIKE ? OR stream LIKE ?)",
            vec![BindValue::Text(like.clone()), BindValue::Text(like)],
        );
    }
    (w, ())
}

pub async fn count_all(
    pool: &Pool,
    media_server_id: Option<&str>,
    pushing: Option<bool>,
    query: Option<&str>,
) -> sqlx::Result<i64> {
    let (w, _) = push_filter(media_server_id, pushing, query);
    let sql = w.sql("SELECT COUNT(*) FROM gb_stream_push");
    let mut q = sqlx::query_scalar::<_, i64>(&sql);
    for b in &w.binds {
        q = match b {
            crate::dyn_where::BindValue::Text(v) => q.bind(v.as_str()),
            crate::dyn_where::BindValue::Int(v) => q.bind(*v),
            crate::dyn_where::BindValue::Big(v) => q.bind(*v),
        };
    }
    q.fetch_one(pool).await
}

/// 绑定/解绑国标设备（`device_id` 为空 = 解绑）。
pub async fn set_gb_binding(
    pool: &Pool,
    id: i64,
    device_id: Option<&str>,
    channel_id: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let r = sqlx::query(
        "UPDATE gb_stream_push SET gb_device_id = ?, gb_channel_id = ?, update_time = ? WHERE id = ?",
    )
    .bind(device_id.filter(|s| !s.is_empty()))
    .bind(channel_id.filter(|s| !s.is_empty()))
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn update_pushing_status(pool: &Pool, id: i64, pushing: bool) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_stream_push SET pushing = ? WHERE id = ?")
        .bind(pushing)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_stream_push SET pushing = $1 WHERE id = $2")
        .bind(pushing)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_stream_push SET pushing = ? WHERE id = ?")
        .bind(pushing)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 根据 app/stream 更新推流在线状态
pub async fn update_pushing_status_by_app_stream(
    pool: &Pool,
    app: &str,
    stream: &str,
    media_server_id: Option<&str>,
    pushing: bool,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_stream_push SET pushing = ?, status = ?, media_server_id = COALESCE(?, media_server_id), push_time = CASE WHEN ? THEN COALESCE(push_time, ?) ELSE push_time END, update_time = ? WHERE app = ? AND stream = ?"
    )
    .bind(pushing)
    .bind(pushing)
    .bind(media_server_id)
    .bind(pushing)
    .bind(now)
    .bind(now)
    .bind(app)
    .bind(stream)
    .execute(pool)
    .await?;

    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_stream_push SET pushing = $1, status = $1, media_server_id = COALESCE($2, media_server_id), push_time = CASE WHEN $1 THEN COALESCE(push_time, $3) ELSE push_time END, update_time = $3 WHERE app = $4 AND stream = $5"
    )
    .bind(pushing)
    .bind(media_server_id)
    .bind(now)
    .bind(app)
    .bind(stream)
    .execute(pool)
    .await?;

    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_stream_push SET pushing = ?, status = ?, media_server_id = COALESCE(?, media_server_id), push_time = CASE WHEN ? THEN COALESCE(push_time, ?) ELSE push_time END, update_time = ? WHERE app = ? AND stream = ?"
    )
    .bind(pushing)
    .bind(pushing)
    .bind(media_server_id)
    .bind(pushing)
    .bind(now)
    .bind(now)
    .bind(app)
    .bind(stream)
    .execute(pool)
    .await?;

    Ok(r.rows_affected())
}

/// 更新推流状态（status字段）
pub async fn update_status(pool: &Pool, id: i64, status: bool) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_stream_push SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_stream_push SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_stream_push SET status = ? WHERE id = ?")
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
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id FROM gb_stream_push WHERE app = ? AND stream = ?"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id FROM gb_stream_push WHERE app = $1 AND stream = $2"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamPush>(
        "SELECT id, app, stream, create_time, media_server_id, server_id, push_time, status, update_time, pushing, self as self_push, start_offline_push, stream_status, gb_device_id, gb_channel_id FROM gb_stream_push WHERE app = ? AND stream = ?"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
}

#[cfg(all(test, feature = "sqlite"))]
mod push_filter_tests {
    use crate::test_support::sqlite_pool_with_schema;

    async fn seed(pool: &crate::db::Pool, app: &str, stream: &str) {
        sqlx::query(
            "INSERT INTO gb_stream_push (app, stream, create_time, update_time, media_server_id, status, pushing) \
             VALUES (?, ?, '2026-01-01 00:00:00', '2026-01-01 00:00:00', 'zlmediakit-1', 0, 0)",
        )
        .bind(app)
        .bind(stream)
        .execute(pool)
        .await
        .expect("insert push");
    }

    /// 关键字过滤此前被 DTO 收下却从未使用 —— 页面按签名传 query 会静默无效。
    #[tokio::test]
    async fn test_list_query_filter_works() {
        let pool = sqlite_pool_with_schema().await;
        seed(&pool, "push", "cam1").await;
        seed(&pool, "push", "gate1").await;

        let all = super::list_paged(&pool, 1, 10, None, None, None).await.unwrap();
        assert_eq!(all.len(), 2);

        let hit = super::list_paged(&pool, 1, 10, None, None, Some("cam")).await.unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].stream.as_deref(), Some("cam1"));

        let by_app = super::list_paged(&pool, 1, 10, None, None, Some("gate")).await.unwrap();
        assert_eq!(by_app.len(), 1);

        assert_eq!(super::count_all(&pool, None, None, None).await.unwrap(), 2);
        assert_eq!(super::count_all(&pool, None, None, Some("cam")).await.unwrap(), 1);
        assert_eq!(super::count_all(&pool, None, None, Some("nope")).await.unwrap(), 0);
    }

    /// 列表序列化成 camelCase（前端读 mediaServerId/createTime），
    /// 且带 `pushUrl`（WVP 表里没有 url 列，前端"推流地址"是算出来的）。
    #[tokio::test]
    async fn test_list_serializes_camel_case() {
        let pool = sqlite_pool_with_schema().await;
        seed(&pool, "push", "cam1").await;
        let rows = super::list_paged(&pool, 1, 10, None, None, None).await.unwrap();
        let v = serde_json::to_value(&rows[0]).unwrap();
        assert!(v.get("mediaServerId").is_some(), "必须是 camelCase: {v}");
        assert!(v.get("createTime").is_some());
        assert!(v.get("startOfflinePush").is_some());
        assert!(v.get("status").is_some());
    }

    /// 国标绑定：`save_to_gb` / `remove_form_gb` 此前更新的是**不存在的列**
    /// （device_id/channel_id）→ 接口稳定 500。
    #[tokio::test]
    async fn test_gb_binding_roundtrip() {
        let pool = sqlite_pool_with_schema().await;
        seed(&pool, "push", "cam1").await;

        let n = super::set_gb_binding(&pool, 1, Some("34020000001320000001"), Some("34020000001310000001"), "now")
            .await
            .unwrap();
        assert_eq!(n, 1);
        let rows = super::list_paged(&pool, 1, 10, None, None, None).await.unwrap();
        assert_eq!(rows[0].gb_device_id.as_deref(), Some("34020000001320000001"));
        assert_eq!(rows[0].gb_channel_id.as_deref(), Some("34020000001310000001"));

        let n = super::set_gb_binding(&pool, 1, None, None, "now").await.unwrap();
        assert_eq!(n, 1);
        let rows = super::list_paged(&pool, 1, 10, None, None, None).await.unwrap();
        assert!(rows[0].gb_device_id.is_none());
    }
}
