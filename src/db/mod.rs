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

pub use user::*;
pub use device::*;
pub use media_server::*;
pub use role::*;
pub use region::*;
pub use group::*;
pub use user_api_key::*;
pub use record_plan::*;
pub use stream_push::StreamPush;
pub use stream_proxy::StreamProxy;
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
        use sqlx::postgres::PgPoolOptions;
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&cfg.database.url)
            .await?;
        Ok(pool)
    }
}
