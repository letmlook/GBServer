pub mod config;
pub mod error;
pub mod response;
pub mod auth;
pub mod db;
pub mod handlers;
pub mod router;
pub mod sip;
pub mod zlm;
pub mod jt1078;
pub mod cache;
pub mod scheduler;
pub mod cascade;
pub mod metrics;

use config::AppConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn init_db_tables(pool: &db::Pool) -> anyhow::Result<()> {
    db::position_history::ensure_table(pool).await?;
    db::audit_log::ensure_table(pool).await?;
    
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
            server.set_ws_state(ws_state.clone()).await;
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
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for server in &zlm_config.servers {
            if server.enabled {
                let client = Arc::new(zlm::ZlmClient::from_config(server));
                zlm_clients.insert(server.id.clone(), client.clone());
                tracing::info!("ZLM client initialized: {} ({}:{})", server.id, server.ip, server.http_port);
                
                let _ = db::media_server::sync_from_config(
                    &pool,
                    &server.id,
                    &server.ip,
                    server.http_port as i32,
                    Some(server.secret.as_str()),
                    &now,
                ).await;
                
                if zlm_client.is_none() {
                    zlm_client = Some(client);
                }
            }
        }
    }

    // Start ZLM health checker and register clients
    if !zlm_clients.is_empty() {
        let mut health_checker = zlm::ZlmHealthChecker::new(30);
        health_checker.set_pool(pool.clone());
        for (id, client) in zlm_clients.iter() {
            health_checker.add_client(id, client.clone()).await;
        }
        tokio::spawn(async move {
            health_checker.run_health_check_loop().await;
        });
    }

    if let Some(ref server) = sip_server {
        let mut server = server.write().await;
        server.set_zlm_client(zlm_client.clone());
        server.start().await?;
    }

    let download_manager = Arc::new(crate::handlers::playback::DownloadManager::new());
    let playback_manager = Arc::new(crate::handlers::playback::PlaybackManager::new());

    // Initialize Redis (optional)
    let redis_conn = if let Some(ref redis_cfg) = cfg.redis {
        match redis::Client::open(redis_cfg.url.as_str()) {
            Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                Ok(cm) => {
                    tracing::info!("Redis 连接成功: {}", redis_cfg.url);
                    Some(cm)
                }
                Err(e) => {
                    tracing::warn!("Redis 连接失败，将以无缓存模式运行: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Redis 客户端创建失败: {}", e);
                None
            }
        }
    } else {
        tracing::info!("未配置 Redis，以无缓存模式运行");
        None
    };

    let state = AppState {
        config: Arc::new(cfg.clone()),
        pool,
        sip_server: sip_server.clone(),
        zlm_client,
        zlm_clients,
        playback_manager: Some(playback_manager),
        download_manager: Some(download_manager),
        ws_state,
        redis: redis_conn,
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

    // Start Cascade registrar for platform-level SIP cascade
    {
        let registrar = Arc::new(crate::cascade::CascadeRegistrar::new());
        let local_device_id = state.config.sip.as_ref().map(|s| s.device_id.clone()).unwrap_or_else(|| "local_device".to_string());
        let realm = state.config.sip.as_ref().map(|s| s.realm.clone()).unwrap_or_else(|| "GBServerRealm".to_string());
        let pool_clone = state.pool.clone();

        if let Some(ref sip_srv) = state.sip_server {
            // Wire registrar into SipServer for response routing
            {
                let mut srv = sip_srv.write().await;
                srv.set_cascade_registrar(registrar.clone());
            }
            // Wire SipServer into registrar for sending REGISTER requests
            registrar.set_sip_server(sip_srv.clone()).await;
        }
        registrar.set_pool(state.pool.clone()).await;

        let reg = registrar.clone();
        tokio::spawn(async move {
            reg.load_platforms_from_db(&pool_clone, &local_device_id, &realm).await;
            reg.run_registration_loop().await;
        });
    }

    // Start RecordPlanScheduler
    {
        let scheduler_pool = state.pool.clone();
        let scheduler_zlm = state.zlm_client.clone();
        tokio::spawn(async move {
            let scheduler = crate::scheduler::record_plan::RecordPlanScheduler::new(
                scheduler_pool,
                scheduler_zlm,
            );
            scheduler.run().await;
        });
    }

    // Start JT1078 server (skeleton) in background
    {
        // If app config provides JT1078 tuning, set environment variables consumed by jt1078::server
        if let Some(jcfg) = cfg.jt1078.as_ref() {
            if let Some(timeout_ms) = jcfg.timeout_ms {
                std::env::set_var("WVP__JT1078__TIMEOUT_MS", timeout_ms.to_string());
            }
            if let Some(rw) = jcfg.retransmit_wait_ms {
                std::env::set_var("WVP__JT1078__RETRANSMIT_WAIT_MS", rw.to_string());
            }
            if let Some(ref hook) = jcfg.retransmit_hook_url {
                std::env::set_var("WVP__JT1078__RETRANSMIT_HOOK", hook.clone());
            }
            // allow enabling direct device send via config env var
            if jcfg.retransmit_hook_url.is_some() {
                // default to not sending to device unless configured explicitly
            }
        }

        let jt = crate::jt1078::Jt1078Server::new();
        let jcfg = cfg.jt1078.clone();
        tokio::spawn(async move {
            if let Err(e) = jt.start(jcfg).await {
                tracing::warn!("JT1078 server failed to start: {}", e);
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
    pub playback_manager: Option<Arc<crate::handlers::playback::PlaybackManager>>,
    pub download_manager: Option<Arc<crate::handlers::playback::DownloadManager>>,
    pub ws_state: Arc<crate::handlers::websocket::WsState>,
    pub redis: Option<redis::aio::ConnectionManager>,
}

impl AppState {
    /// 获取 ZLM 客户端。media_server_id 为 None 或 "auto" 时自动选择负载最低的节点。
    pub async fn get_zlm_client_auto(&self, media_server_id: Option<&str>) -> Option<(String, Arc<zlm::ZlmClient>)> {
        match media_server_id {
            Some(id) if id != "auto" && !id.is_empty() => {
                self.zlm_clients.get(id).map(|c| (id.to_string(), c.clone()))
            }
            _ => self.select_least_loaded().await,
        }
    }

    pub fn get_zlm_client(&self, media_server_id: Option<&str>) -> Option<Arc<zlm::ZlmClient>> {
        if let Some(id) = media_server_id {
            if id != "auto" && !id.is_empty() {
                return self.zlm_clients.get(id).cloned();
            }
        }
        self.zlm_client.clone()
    }
    
    pub fn list_zlm_servers(&self) -> Vec<String> {
        self.zlm_clients.keys().cloned().collect()
    }

    /// 选择流数量最少的 ZLM 节点（最少连接数策略）
    async fn select_least_loaded(&self) -> Option<(String, Arc<zlm::ZlmClient>)> {
        if self.zlm_clients.is_empty() {
            return None;
        }
        if self.zlm_clients.len() == 1 {
            let (id, client) = self.zlm_clients.iter().next()?;
            return Some((id.clone(), client.clone()));
        }

        // 优先从 Redis 读取各节点流计数
        if let Some(ref redis) = self.redis {
            let mut min_count = i64::MAX;
            let mut best: Option<(String, Arc<zlm::ZlmClient>)> = None;
            for (id, client) in &self.zlm_clients {
                let count = cache::get_media_server_stream_count(redis, id).await;
                if count < min_count {
                    min_count = count;
                    best = Some((id.clone(), client.clone()));
                }
            }
            if best.is_some() {
                return best;
            }
        }

        // 无 Redis 时查询 ZLM API 获取实际流数
        let mut min_count = usize::MAX;
        let mut best: Option<(String, Arc<zlm::ZlmClient>)> = None;
        for (id, client) in &self.zlm_clients {
            let count = client.get_active_stream_count().await.unwrap_or(usize::MAX);
            if count < min_count {
                min_count = count;
                best = Some((id.clone(), client.clone()));
            }
        }
        best
    }
}
