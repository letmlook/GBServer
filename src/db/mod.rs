mod user;
mod device;
mod media_server;
pub mod role;
pub mod region;
pub mod group;
pub mod user_api_key;
pub mod record_plan;
pub mod stream_push;
pub mod stream_proxy;
pub mod platform;

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

use crate::config::AppConfig;

#[cfg(feature = "mysql")]
pub type Pool = sqlx::MySqlPool;

#[cfg(feature = "postgres")]
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
