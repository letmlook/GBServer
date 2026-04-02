pub mod config;
pub mod error;
pub mod response;
pub mod auth;
pub mod db;
pub mod handlers;
pub mod router;
pub mod sip;
pub mod zlm;

use config::AppConfig;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run(cfg: AppConfig) -> anyhow::Result<()> {
    let pool = db::create_pool(&cfg).await?;

    let sip_server = if let Some(ref sip_config) = cfg.sip {
        if sip_config.enabled {
            let mut server = sip::SipServer::new(sip_config.clone(), pool.clone());
            server.start().await?;
            Some(Arc::new(RwLock::new(server)))
        } else {
            None
        }
    } else {
        None
    };

    let zlm_client = if let Some(ref zlm_config) = cfg.zlm {
        if let Some(server) = zlm_config.servers.first() {
            if server.enabled {
                Some(Arc::new(zlm::ZlmClient::from_config(server)))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let state = AppState {
        config: Arc::new(cfg.clone()),
        pool,
        sip_server: sip_server.clone(),
        zlm_client,
    };

    if let Some(ref server) = sip_server {
        let srv = server.clone();
        tokio::spawn(async move {
            let mut server = srv.write().await;
            if let Err(e) = server.run().await {
                tracing::error!("SIP Server error: {}", e);
            }
        });
    }

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
    pub sip_server: Option<Arc<RwLock<sip::SipServer>>>,
    pub zlm_client: Option<Arc<zlm::ZlmClient>>,
}
