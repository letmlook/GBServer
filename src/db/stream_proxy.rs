//! 拉流代理表 gb_stream_proxy

use serde::Serialize;
use sqlx::FromRow;

use super::Pool;
use crate::state::{StreamState, StreamStatus};
use std::str::FromStr;

/// 拉流代理结构体。
///
/// 序列化成 **camelCase**：前端（与 WVP 的 `StreamProxy` bean）读的是
/// `srcUrl`/`mediaServerId`/`streamStatus`，snake_case 会让"源 URL""媒体节点"整列空白。
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StreamProxy {
    pub id: i32,
    #[serde(rename = "type")]
    pub r#type: Option<String>,
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
    /// Phase 4.5: 统一流状态字段（与 `pulling` bool 并存，不替换）
    #[serde(default)]
    pub stream_status: Option<String>,
}

impl StreamState for StreamProxy {
    fn stream_id(&self) -> &str {
        self.stream.as_deref().unwrap_or("")
    }
    fn app(&self) -> &str {
        self.app.as_deref().unwrap_or("")
    }
    fn status(&self) -> StreamStatus {
        if let Some(ref s) = self.stream_status {
            if let Ok(st) = StreamStatus::from_str(s) {
                return st;
            }
        }
        if self.pulling.unwrap_or(false) {
            StreamStatus::Active
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
        // 拉流代理通常无直接 device 关联
        None
    }
    fn channel_id(&self) -> Option<&str> {
        None
    }
}

/// Phase 4.5: 幂等迁移 —— 为已存在的 `gb_stream_proxy` 表添加 `stream_status` 列。
pub async fn ensure_stream_status_column(pool: &Pool) -> sqlx::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let _ = sqlx::query("ALTER TABLE gb_stream_proxy ADD COLUMN IF NOT EXISTS stream_status VARCHAR(32) NOT NULL DEFAULT 'ready'")
            .execute(pool)
            .await?;
    }
    #[cfg(feature = "sqlite")]
    {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('gb_stream_proxy') WHERE name = 'stream_status'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if exists == 0 {
            let _ = sqlx::query("ALTER TABLE gb_stream_proxy ADD COLUMN stream_status TEXT NOT NULL DEFAULT 'ready'")
                .execute(pool)
                .await?;
        }
    }
    #[cfg(feature = "mysql")]
    {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'gb_stream_proxy' AND column_name = 'stream_status'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if exists == 0 {
            let _ = sqlx::query("ALTER TABLE gb_stream_proxy ADD COLUMN stream_status varchar(32) NOT NULL DEFAULT 'ready'")
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// 根据ID获取拉流代理
pub async fn get_by_id(pool: &Pool, id: i64) -> sqlx::Result<Option<StreamProxy>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await;
}

/// 拉流代理的可写字段（新增 / 更新共用）。
///
/// 此前 `add` 的形参只有 5 个字符串，`type` / `timeout` / `ffmpeg_cmd_key` /
/// `rtsp_type` / `enable*` 全部无处安放 —— 用户在新弹窗里选的协议类型、超时、
/// FFmpeg 模板、拉流方式、启用开关**在插入时就被丢掉**；`update` 连 `enable`
/// 都没有。换成结构体后新增/更新共享同一份字段清单。
#[derive(Debug, Default, Clone)]
pub struct StreamProxyWrite<'a> {
    pub app: Option<&'a str>,
    pub stream: Option<&'a str>,
    pub src_url: Option<&'a str>,
    pub media_server_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub r#type: Option<&'a str>,
    pub timeout: Option<i32>,
    pub ffmpeg_cmd_key: Option<&'a str>,
    pub rtsp_type: Option<&'a str>,
    pub enable: Option<bool>,
    pub enable_audio: Option<bool>,
    pub enable_mp4: Option<bool>,
    pub enable_disable_none_reader: Option<bool>,
    pub relates_media_server_id: Option<&'a str>,
}

/// 添加拉流代理（写入全部可写列）。
///
/// `pulling` 是运行期状态，恒从 false 起步；`enable` 由调用方给出（缺省 false）。
pub async fn add(pool: &Pool, f: &StreamProxyWrite<'_>, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_proxy
           (app, stream, src_url, media_server_id, name, type, timeout, ffmpeg_cmd_key, rtsp_type,
            enable_audio, enable_mp4, enable_disable_none_reader, relates_media_server_id,
            enable, pulling, create_time, update_time)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, false, ?, ?)"#,
    )
    .bind(f.app.unwrap_or("proxy"))
    .bind(f.stream.unwrap_or(""))
    .bind(f.src_url.unwrap_or(""))
    .bind(f.media_server_id.unwrap_or("auto"))
    .bind(f.name.unwrap_or(""))
    .bind(f.r#type)
    .bind(f.timeout)
    .bind(f.ffmpeg_cmd_key)
    .bind(f.rtsp_type)
    .bind(f.enable_audio.unwrap_or(false))
    .bind(f.enable_mp4.unwrap_or(false))
    .bind(f.enable_disable_none_reader.unwrap_or(false))
    .bind(f.relates_media_server_id)
    .bind(f.enable.unwrap_or(false))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_proxy
           (app, stream, src_url, media_server_id, name, type, timeout, ffmpeg_cmd_key, rtsp_type,
            enable_audio, enable_mp4, enable_disable_none_reader, relates_media_server_id,
            enable, pulling, create_time, update_time)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, false, $15, $16)"#,
    )
    .bind(f.app.unwrap_or("proxy"))
    .bind(f.stream.unwrap_or(""))
    .bind(f.src_url.unwrap_or(""))
    .bind(f.media_server_id.unwrap_or("auto"))
    .bind(f.name.unwrap_or(""))
    .bind(f.r#type)
    .bind(f.timeout)
    .bind(f.ffmpeg_cmd_key)
    .bind(f.rtsp_type)
    .bind(f.enable_audio.unwrap_or(false))
    .bind(f.enable_mp4.unwrap_or(false))
    .bind(f.enable_disable_none_reader.unwrap_or(false))
    .bind(f.relates_media_server_id)
    .bind(f.enable.unwrap_or(false))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"INSERT INTO gb_stream_proxy
           (app, stream, src_url, media_server_id, name, type, timeout, ffmpeg_cmd_key, rtsp_type,
            enable_audio, enable_mp4, enable_disable_none_reader, relates_media_server_id,
            enable, pulling, create_time, update_time)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)"#,
    )
    .bind(f.app.unwrap_or("proxy"))
    .bind(f.stream.unwrap_or(""))
    .bind(f.src_url.unwrap_or(""))
    .bind(f.media_server_id.unwrap_or("auto"))
    .bind(f.name.unwrap_or(""))
    .bind(f.r#type)
    .bind(f.timeout)
    .bind(f.ffmpeg_cmd_key)
    .bind(f.rtsp_type)
    .bind(f.enable_audio.unwrap_or(false))
    .bind(f.enable_mp4.unwrap_or(false))
    .bind(f.enable_disable_none_reader.unwrap_or(false))
    .bind(f.relates_media_server_id)
    .bind(f.enable.unwrap_or(false))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 更新拉流代理；`None` 的字段保持原值（COALESCE）。
///
/// 注意 `pulling` 不在可写字段里 —— 它是运行期状态，只能由
/// `update_play_state` 改写，否则"编辑弹窗保存"会把正在拉流的标记抹掉。
pub async fn update(pool: &Pool, id: i64, f: &StreamProxyWrite<'_>, now: &str) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        r#"UPDATE gb_stream_proxy SET
           app = COALESCE(?, app),
           stream = COALESCE(?, stream),
           src_url = COALESCE(?, src_url),
           media_server_id = COALESCE(?, media_server_id),
           name = COALESCE(?, name),
           type = COALESCE(?, type),
           timeout = COALESCE(?, timeout),
           ffmpeg_cmd_key = COALESCE(?, ffmpeg_cmd_key),
           rtsp_type = COALESCE(?, rtsp_type),
           enable_audio = COALESCE(?, enable_audio),
           enable_mp4 = COALESCE(?, enable_mp4),
           enable_disable_none_reader = COALESCE(?, enable_disable_none_reader),
           relates_media_server_id = COALESCE(?, relates_media_server_id),
           enable = COALESCE(?, enable),
           update_time = ?
           WHERE id = ?"#,
    )
    .bind(f.app)
    .bind(f.stream)
    .bind(f.src_url)
    .bind(f.media_server_id)
    .bind(f.name)
    .bind(f.r#type)
    .bind(f.timeout)
    .bind(f.ffmpeg_cmd_key)
    .bind(f.rtsp_type)
    .bind(f.enable_audio)
    .bind(f.enable_mp4)
    .bind(f.enable_disable_none_reader)
    .bind(f.relates_media_server_id)
    .bind(f.enable)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        r#"UPDATE gb_stream_proxy SET
           app = COALESCE($1, app),
           stream = COALESCE($2, stream),
           src_url = COALESCE($3, src_url),
           media_server_id = COALESCE($4, media_server_id),
           name = COALESCE($5, name),
           type = COALESCE($6, type),
           timeout = COALESCE($7, timeout),
           ffmpeg_cmd_key = COALESCE($8, ffmpeg_cmd_key),
           rtsp_type = COALESCE($9, rtsp_type),
           enable_audio = COALESCE($10, enable_audio),
           enable_mp4 = COALESCE($11, enable_mp4),
           enable_disable_none_reader = COALESCE($12, enable_disable_none_reader),
           relates_media_server_id = COALESCE($13, relates_media_server_id),
           enable = COALESCE($14, enable),
           update_time = $15
           WHERE id = $16"#,
    )
    .bind(f.app)
    .bind(f.stream)
    .bind(f.src_url)
    .bind(f.media_server_id)
    .bind(f.name)
    .bind(f.r#type)
    .bind(f.timeout)
    .bind(f.ffmpeg_cmd_key)
    .bind(f.rtsp_type)
    .bind(f.enable_audio)
    .bind(f.enable_mp4)
    .bind(f.enable_disable_none_reader)
    .bind(f.relates_media_server_id)
    .bind(f.enable)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        r#"UPDATE gb_stream_proxy SET
           app = COALESCE(?, app),
           stream = COALESCE(?, stream),
           src_url = COALESCE(?, src_url),
           media_server_id = COALESCE(?, media_server_id),
           name = COALESCE(?, name),
           type = COALESCE(?, type),
           timeout = COALESCE(?, timeout),
           ffmpeg_cmd_key = COALESCE(?, ffmpeg_cmd_key),
           rtsp_type = COALESCE(?, rtsp_type),
           enable_audio = COALESCE(?, enable_audio),
           enable_mp4 = COALESCE(?, enable_mp4),
           enable_disable_none_reader = COALESCE(?, enable_disable_none_reader),
           relates_media_server_id = COALESCE(?, relates_media_server_id),
           enable = COALESCE(?, enable),
           update_time = ?
           WHERE id = ?"#,
    )
    .bind(f.app)
    .bind(f.stream)
    .bind(f.src_url)
    .bind(f.media_server_id)
    .bind(f.name)
    .bind(f.r#type)
    .bind(f.timeout)
    .bind(f.ffmpeg_cmd_key)
    .bind(f.rtsp_type)
    .bind(f.enable_audio)
    .bind(f.enable_mp4)
    .bind(f.enable_disable_none_reader)
    .bind(f.relates_media_server_id)
    .bind(f.enable)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// 更新拉流运行状态：同时落 `pulling`（bool）与 `stream_status`（Phase 4.5 文本）。
pub async fn update_play_state(
    pool: &Pool,
    id: i64,
    pulling: bool,
    status: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET pulling = ?, stream_status = ? WHERE id = ?")
        .bind(pulling)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET pulling = $1, stream_status = $2 WHERE id = $3")
        .bind(pulling)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET pulling = ?, stream_status = ? WHERE id = ?")
        .bind(pulling)
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 删除拉流代理
pub async fn delete_by_id(pool: &Pool, id: i64) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("DELETE FROM gb_stream_proxy WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_stream_proxy WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("DELETE FROM gb_stream_proxy WHERE id = ?")
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

/// 组装代理列表的 WHERE（`list_paged` 与 `count_all` 共用）。
///
/// `query` 此前被 DTO 收下却从未使用 —— 搜索框里输入什么都返回同一批数据。
fn proxy_filter(
    media_server_id: Option<&str>,
    pulling: Option<bool>,
    query: Option<&str>,
) -> crate::dyn_where::DynWhere {
    use crate::dyn_where::{BindValue, DynWhere};
    let mut w = DynWhere::new();
    if let Some(mid) = media_server_id.filter(|s| !s.is_empty()) {
        w.add("media_server_id = ?", vec![BindValue::Text(mid.to_string())]);
    }
    if let Some(p) = pulling {
        // 不能绑 Int：postgres 的 `pulling` 是 bool 列，`boolean = integer` 会 500。
        w.add("pulling = ?", vec![BindValue::Bool(p)]);
    }
    if let Some(kw) = query.map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{kw}%");
        w.add(
            "(app LIKE ? OR stream LIKE ? OR name LIKE ? OR src_url LIKE ? OR type LIKE ?)",
            vec![
                BindValue::Text(like.clone()),
                BindValue::Text(like.clone()),
                BindValue::Text(like.clone()),
                BindValue::Text(like.clone()),
                BindValue::Text(like),
            ],
        );
    }
    w
}

const PROXY_COLS: &str = "id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, \
     media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, \
     stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status";
const PROXY_FROM: &str = " FROM gb_stream_proxy";

pub async fn list_paged(
    pool: &Pool,
    page: u32,
    count: u32,
    media_server_id: Option<&str>,
    pulling: Option<bool>,
    query: Option<&str>,
) -> sqlx::Result<Vec<StreamProxy>> {
    let offset = (page.saturating_sub(1)) * count;
    let limit = count.min(100) as i64;
    let w = proxy_filter(media_server_id, pulling, query);
    // 多方言共用同一份 SQL 文本：`DynWhere::sql` 把 `?` 改写成 `$n`。
    // 列名显式列出，不用 `SELECT *` —— 少了列时 `SELECT *` 不报错，只会让字段恒空。
    let limit_ph = if cfg!(feature = "postgres") {
        format!(" LIMIT ${} OFFSET ${}", w.binds.len() + 1, w.binds.len() + 2)
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql = format!(
        "SELECT {PROXY_COLS}{PROXY_FROM}{} ORDER BY id{limit_ph}",
        w.sql("")
    );
    let mut q = sqlx::query_as::<_, StreamProxy>(&sql);
    for b in &w.binds {
        q = match b {
            crate::dyn_where::BindValue::Text(v) => q.bind(v.as_str()),
            crate::dyn_where::BindValue::Int(v) => q.bind(*v),
            crate::dyn_where::BindValue::Big(v) => q.bind(*v),
            crate::dyn_where::BindValue::Bool(v) => q.bind(*v),
        };
    }
    q.bind(limit).bind(offset as i64).fetch_all(pool).await
}

pub async fn count_all(
    pool: &Pool,
    media_server_id: Option<&str>,
    pulling: Option<bool>,
    query: Option<&str>,
) -> sqlx::Result<i64> {
    let w = proxy_filter(media_server_id, pulling, query);
    let sql = format!("SELECT COUNT(*){PROXY_FROM}{}", w.sql(""));
    let mut cq = sqlx::query_scalar::<_, i64>(&sql);
    for b in &w.binds {
        cq = match b {
            crate::dyn_where::BindValue::Text(v) => cq.bind(v.as_str()),
            crate::dyn_where::BindValue::Int(v) => cq.bind(*v),
            crate::dyn_where::BindValue::Big(v) => cq.bind(*v),
            crate::dyn_where::BindValue::Bool(v) => cq.bind(*v),
        };
    }
    cq.fetch_one(pool).await
}

/// 更新拉流状态
pub async fn update_pulling_status(pool: &Pool, id: i64, pulling: bool) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET pulling = ? WHERE id = ?")
        .bind(pulling)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET pulling = $1 WHERE id = $2")
        .bind(pulling)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET pulling = ? WHERE id = ?")
        .bind(pulling)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 根据 app/stream 更新代理拉流状态
pub async fn update_pulling_status_by_app_stream(
    pool: &Pool,
    app: &str,
    stream: &str,
    media_server_id: Option<&str>,
    pulling: bool,
    now: &str,
) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query(
        "UPDATE gb_stream_proxy SET pulling = ?, media_server_id = COALESCE(?, media_server_id), update_time = ? WHERE app = ? AND stream = ?"
    )
    .bind(pulling)
    .bind(media_server_id)
    .bind(now)
    .bind(app)
    .bind(stream)
    .execute(pool)
    .await?;

    #[cfg(feature = "postgres")]
    let r = sqlx::query(
        "UPDATE gb_stream_proxy SET pulling = $1, media_server_id = COALESCE($2, media_server_id), update_time = $3 WHERE app = $4 AND stream = $5"
    )
    .bind(pulling)
    .bind(media_server_id)
    .bind(now)
    .bind(app)
    .bind(stream)
    .execute(pool)
    .await?;

    #[cfg(feature = "sqlite")]
    let r = sqlx::query(
        "UPDATE gb_stream_proxy SET pulling = ?, media_server_id = COALESCE(?, media_server_id), update_time = ? WHERE app = ? AND stream = ?"
    )
    .bind(pulling)
    .bind(media_server_id)
    .bind(now)
    .bind(app)
    .bind(stream)
    .execute(pool)
    .await?;

    Ok(r.rows_affected())
}

/// 更新启用状态
pub async fn update_enable_status(pool: &Pool, id: i64, enable: bool) -> sqlx::Result<u64> {
    #[cfg(feature = "mysql")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET enable = ? WHERE id = ?")
        .bind(enable)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "postgres")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET enable = $1 WHERE id = $2")
        .bind(enable)
        .bind(id)
        .execute(pool)
        .await?;
    #[cfg(feature = "sqlite")]
    let r = sqlx::query("UPDATE gb_stream_proxy SET enable = ? WHERE id = ?")
        .bind(enable)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// 根据 app 和 stream 查询代理
pub async fn get_by_app_stream(pool: &Pool, app: &str, stream: &str) -> sqlx::Result<Option<StreamProxy>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE app = ? AND stream = ?"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE app = $1 AND stream = $2"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE app = ? AND stream = ?"
    )
    .bind(app)
    .bind(stream)
    .fetch_optional(pool)
    .await;
}

/// 获取所有启用的代理
pub async fn get_all_enabled_proxies(pool: &Pool) -> sqlx::Result<Vec<StreamProxy>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE enable = 1 ORDER BY id"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE enable = true ORDER BY id"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE enable = 1 ORDER BY id"
    )
    .fetch_all(pool)
    .await;
}

/// 获取所有正在拉流的代理
pub async fn get_all_pulling_proxies(pool: &Pool) -> sqlx::Result<Vec<StreamProxy>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE pulling = 1 ORDER BY id"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE pulling = true ORDER BY id"
    )
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE pulling = 1 ORDER BY id"
    )
    .fetch_all(pool)
    .await;
}

/// 根据媒体服务器ID获取代理列表
pub async fn list_by_media_server(pool: &Pool, media_server_id: &str) -> sqlx::Result<Vec<StreamProxy>> {
    #[cfg(feature = "mysql")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE media_server_id = ? ORDER BY id"
    )
    .bind(media_server_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "postgres")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE media_server_id = $1 ORDER BY id"
    )
    .bind(media_server_id)
    .fetch_all(pool)
    .await;
    #[cfg(feature = "sqlite")]
    return sqlx::query_as::<_, StreamProxy>(
        "SELECT id, type, app, stream, src_url, timeout, ffmpeg_cmd_key, rtsp_type, media_server_id, enable_audio, enable_mp4, pulling, enable, create_time, name, update_time, stream_key, server_id, enable_disable_none_reader, relates_media_server_id, stream_status FROM gb_stream_proxy WHERE media_server_id = ? ORDER BY id"
    )
    .bind(media_server_id)
    .fetch_all(pool)
    .await;
}
