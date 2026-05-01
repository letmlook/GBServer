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

#[cfg(all(feature = "mysql", not(feature = "postgres")))]
pub type Pool = sqlx::MySqlPool;

#[cfg(all(feature = "postgres", not(feature = "mysql")))]
pub type Pool = sqlx::PgPool;

// 默认使用postgres（当没有明确指定feature时）
#[cfg(all(not(feature = "mysql"), not(feature = "postgres")))]
pub type Pool = sqlx::PgPool;

pub async fn create_pool(cfg: &AppConfig) -> anyhow::Result<Pool> {
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
