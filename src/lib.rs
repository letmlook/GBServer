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
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn init_db_tables(pool: &db::Pool) -> anyhow::Result<()> {
    db::position_history::ensure_table(pool).await?;
    
    // Check if core WVP tables exist; if not, run full schema init
    #[cfg(feature = "postgres")]
    {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'wvp_device')"
        )
        .fetch_one(pool)
        .await.unwrap_or(false);
        
        if !table_exists {
            tracing::info!("WVP schema tables not found, initializing from SQL script...");
            let sql = include_str!("../database/init-postgresql-2.7.4.sql");
            // Execute each statement separately (split by semicolons, skip comments)
            for stmt in sql.split(';') {
                let stmt = stmt.trim();
                if stmt.is_empty() || stmt.starts_with("--") || stmt.starts_with("/*") {
                    continue;
                }
                // Skip non-DML/DDL statements
                if !stmt.starts_with("CREATE") && !stmt.starts_with("INSERT") && 
                   !stmt.starts_with("ALTER") && !stmt.starts_with("COMMENT") &&
                   !stmt.starts_with("DROP") && !stmt.starts_with("DO") {
                    continue;
                }
                let _ = sqlx::query(stmt).execute(pool).await;
            }
            tracing::info!("WVP schema initialization complete");
        }
    }
    
    #[cfg(feature = "mysql")]
    {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'wvp_device')"
        )
        .fetch_one(pool)
        .await.unwrap_or(false);
        
        if !table_exists {
            tracing::info!("WVP schema tables not found, initializing from SQL script...");
            let sql = include_str!("../database/init-mysql-2.7.4.sql");
            for stmt in sql.split(';') {
                let stmt = stmt.trim();
                if stmt.is_empty() || stmt.starts_with("--") || stmt.starts_with("/*") {
                    continue;
                }
                if !stmt.starts_with("CREATE") && !stmt.starts_with("INSERT") && 
                   !stmt.starts_with("ALTER") && !stmt.starts_with("DROP") {
                    continue;
                }
                let _ = sqlx::query(stmt).execute(pool).await;
            }
            tracing::info!("WVP schema initialization complete");
        }
    }
    
    Ok(())
}

pub async fn run(cfg: AppConfig) -> anyhow::Result<()> {
    let pool = db::create_pool(&cfg).await?;
    let ws_state = Arc::new(crate::handlers::websocket::WsState::new());

    // Initialize required DB tables on startup
    init_db_tables(&pool).await?;

    let sip_server = if let Some(ref sip_config) = cfg.sip {
        if sip_config.enabled {
            let mut server = sip::SipServer::new(sip_config.clone(), pool.clone());
            server.set_ws_state(ws_state.clone());
            server.start().await?;
            Some(Arc::new(RwLock::new(server)))
        } else {
            None
        }
    } else {
        None
    };

    let mut zlm_clients: HashMap<String, Arc<zlm::ZlmClient>> = HashMap::new();
    let mut zlm_client: Option<Arc<zlm::ZlmClient>> = None;
    
    if let Some(ref zlm_config) = cfg.zlm {
        for server in &zlm_config.servers {
            if server.enabled {
                let client = Arc::new(zlm::ZlmClient::from_config(server));
                zlm_clients.insert(server.id.clone(), client.clone());
                tracing::info!("ZLM client initialized: {} ({}:{})", server.id, server.ip, server.http_port);
                
                if zlm_client.is_none() {
                    zlm_client = Some(client);
                }
            }
        }
    }

    let download_manager = Arc::new(crate::handlers::playback::DownloadManager::new());

    let state = AppState {
        config: Arc::new(cfg.clone()),
        pool,
        sip_server: sip_server.clone(),
        zlm_client,
        zlm_clients,
        download_manager: Some(download_manager),
        ws_state,
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
    let app = router::app(state.clone()).with_state(state);
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
    pub zlm_clients: HashMap<String, Arc<zlm::ZlmClient>>,
    pub download_manager: Option<Arc<crate::handlers::playback::DownloadManager>>,
    pub ws_state: Arc<crate::handlers::websocket::WsState>,
}

impl AppState {
    pub fn get_zlm_client(&self, media_server_id: Option<&str>) -> Option<Arc<zlm::ZlmClient>> {
        if let Some(id) = media_server_id {
            self.zlm_clients.get(id).cloned()
        } else {
            self.zlm_client.clone()
        }
    }
    
    pub fn list_zlm_servers(&self) -> Vec<String> {
        self.zlm_clients.keys().cloned().collect()
    }
}
