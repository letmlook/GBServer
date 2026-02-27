pub mod config;
pub mod error;
pub mod response;
pub mod auth;
pub mod db;
pub mod handlers;
pub mod router;

use config::AppConfig;
use std::sync::Arc;

pub async fn run(cfg: AppConfig) -> anyhow::Result<()> {
    let pool = db::create_pool(&cfg).await?;
    let state = AppState {
        config: Arc::new(cfg.clone()),
        pool,
    };

    let port = cfg.server.port;
    let app = router::app(state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("WVP GB28181 后端启动: http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pool: db::Pool,
}
