#[cfg(all(test, feature = "sqlite"))]
mod read_smoke;
pub mod log;
mod user;
pub mod device;
pub mod media_server;
pub mod role;
pub mod region;
pub mod group;
pub mod user_api_key;
pub mod record_plan;
pub mod stream_push;
pub mod stream_proxy;
pub mod platform;
pub mod platform_channel;
pub mod common_channel;
pub mod jt1078;
pub mod position_history;
pub mod alarm;
pub mod mobile_position;
pub mod cloud_record;
pub mod platform_group;
pub mod platform_region;
pub mod audit_log;

// 跨模块同名函数 re-export（如 `count_all`、`get_by_id` 等多模块共有）。
// 调用方写 `db::count_all(&pool)` 时使用"先 import 的优先"语义；触发 warning 但不编译失败。
// 业务层实际依赖调用路径明确（`db::device::get_by_id` vs `db::region::get_by_id`）。
#[allow(ambiguous_glob_reexports)]
pub use log::*;
#[allow(ambiguous_glob_reexports)]
pub use user::*;
#[allow(ambiguous_glob_reexports)]
pub use device::*;
#[allow(ambiguous_glob_reexports)]
pub use media_server::*;
#[allow(ambiguous_glob_reexports)]
pub use role::*;
#[allow(ambiguous_glob_reexports)]
pub use region::*;
#[allow(ambiguous_glob_reexports)]
pub use group::*;
#[allow(ambiguous_glob_reexports)]
pub use user_api_key::*;
#[allow(ambiguous_glob_reexports)]
pub use record_plan::*;
pub use stream_push::StreamPush;
pub use stream_proxy::{StreamProxy, StreamProxyWrite};
pub use platform::Platform;
pub use jt1078::{JtTerminal, JtChannel};
pub use position_history::PositionHistory;
pub use alarm::Alarm;
pub use mobile_position::MobilePosition;
pub use cloud_record::CloudRecord;

use crate::config::AppConfig;

// 数据库三选一：编译期通过 cargo feature 互斥确定 Pool 类型。
// 默认 SQLite；PG 用 --no-default-features --features postgres；MySQL 用 --no-default-features --features mysql。
#[cfg(feature = "sqlite")]
pub type Pool = sqlx::SqlitePool;

#[cfg(all(feature = "postgres", not(feature = "mysql"), not(feature = "sqlite")))]
pub type Pool = sqlx::PgPool;

#[cfg(all(feature = "mysql", not(feature = "postgres"), not(feature = "sqlite")))]
pub type Pool = sqlx::MySqlPool;

// 兜底：当三个 feature 同时未指定时，默认 PG（保留历史行为）
#[cfg(all(not(feature = "mysql"), not(feature = "postgres"), not(feature = "sqlite")))]
pub type Pool = sqlx::PgPool;

pub async fn create_pool(cfg: &AppConfig) -> anyhow::Result<Pool> {
    #[cfg(feature = "sqlite")]
    {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        // SQLite URL 兼容：接受 `sqlite://path`、`sqlite::memory:`，或裸文件路径
        // 裸路径自动补 `sqlite://` 前缀；绝对路径以 `/` 开头时，URL 用三斜杠 `sqlite:///abs/path`。
        let url = &cfg.database.url;
        let url_owned: String;
        let normalized: &str = if url.starts_with("sqlite:") || url.starts_with("file:") {
            url.as_str()
        } else if url == ":memory:" || url.is_empty() {
            "sqlite::memory:"
        } else {
            // 绝对路径补三斜杠（保留根 `/`），相对路径补双斜杠
            if url.starts_with('/') {
                url_owned = format!("sqlite:///{}", url.trim_start_matches('/'));
            } else {
                url_owned = format!("sqlite://{}", url);
            }
            url_owned.as_str()
        };
        let opts = SqliteConnectOptions::from_str(normalized)
            .map_err(|e| anyhow::anyhow!("解析 SQLite 连接串失败: {} (url={})", e, url))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(opts)
            .await?;
        tracing::info!("SQLite pool 初始化成功 (WAL mode): {}", normalized);
        return Ok(pool);
    }

    #[cfg(feature = "mysql")]
    {
        use sqlx::mysql::MySqlPoolOptions;
        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect(&cfg.database.url)
            .await?;
        Ok(pool)
    }

    #[cfg(feature = "postgres")]
    {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        use std::str::FromStr;

        // **必须关闭 prepared statement 缓存**（capacity = 0）。
        //
        // 原因（2026-09-12 postgres 运行时冒烟实测）：
        // sqlx-postgres 的语句缓存以 **SQL 文本**为 key，命中缓存时
        // **不校验参数类型**（见 sqlx-postgres `get_or_prepare`：
        // `if let Some(statement) = self.cache_statement.get_mut(sql) { return ... }`）。
        // 于是"同一份 SQL 文本被两个不同 Rust 类型绑定"的地方，
        // 第二次会按第一次 PARSE 时声明的 OID 发送二进制参数，服务端直接报：
        //   - `insufficient data left in message`
        //   - `incorrect binary data format in bind parameter 1`
        // 具体撞车实例（实测报 500 的接口）：
        //   `DELETE FROM gb_record_plan_item WHERE plan_id = $1`
        //     —— db/record_plan.rs 的 `delete_by_id` 绑 i32、`replace_items` 绑 i64
        //     → `/api/record/plan/delete`、`/api/record/plan/update` 500。
        //     （该处参数类型已一并统一为 i32，作为第二道防线。）
        // sqlite（动态类型）与 mysql（宽进严出）都不受影响，所以这个问题
        // 只在 postgres 构建里、且只在"先跑 A 再跑 B"时才炸，隐藏得很深。
        //
        // 关掉缓存后每次查询都带真实参数类型重新 PARSE，代价是每条语句多一个
        // 往返；对本项目（管理面 + 协议面，QPS 不高）换取的是**跨方言一致的
        // 正确性**。根治办法是把同文本 SQL 的参数类型统一，但那需要全仓约束，
        // 这里先用最可靠的开关兜住。
        let opts = PgConnectOptions::from_str(&cfg.database.url)
            .map_err(|e| anyhow::anyhow!("解析 PostgreSQL 连接串失败: {}", e))?
            .statement_cache_capacity(0);
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(opts)
            .await?;
        Ok(pool)
    }
}
