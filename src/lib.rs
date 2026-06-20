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
pub mod rpc;
pub mod state_store;
pub mod state;
pub mod security;

use config::AppConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn init_db_tables(pool: &db::Pool) -> anyhow::Result<()> {
    db::position_history::ensure_table(pool).await?;
    db::audit_log::ensure_table(pool).await?;
    // Phase 4.5: 幂等迁移 —— 流状态统一字段
    let _ = db::stream_push::ensure_stream_status_column(pool).await;
    let _ = db::stream_proxy::ensure_stream_status_column(pool).await;

    // Check if core tables exist; if not, run full schema init
    #[cfg(feature = "postgres")]
    {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'gb_device')"
        )
        .fetch_one(pool)
        .await.unwrap_or(false);

        if !table_exists {
            tracing::info!("Schema tables not found, initializing from SQL script...");
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
            tracing::info!("Schema initialization complete");
        }
    }

    #[cfg(feature = "mysql")]
    {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'gb_device')"
        )
        .fetch_one(pool)
        .await.unwrap_or(false);

        if !table_exists {
            tracing::info!("Schema tables not found, initializing from SQL script...");
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
            tracing::info!("Schema initialization complete");
        }
    }

    #[cfg(feature = "sqlite")]
    {
        // SQLite 用 sqlite_master 检测；Phase 1 仅覆盖核心 6 表最小集合
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='gb_device')"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !table_exists {
            tracing::info!("[sqlite] schema tables not found, initializing...");
            let sql = include_str!("../database/init-sqlite-2.7.4.sql");
            // 逐行去除 `--` 行注释和空行，然后按 `;` 切分执行
            let cleaned: String = sql
                .lines()
                .filter(|line| {
                    let t = line.trim_start();
                    !t.is_empty() && !t.starts_with("--")
                })
                .collect::<Vec<_>>()
                .join("\n");
            for (idx, raw_stmt) in cleaned.split(';').enumerate() {
                let stmt = raw_stmt.trim();
                if stmt.is_empty() {
                    continue;
                }
                let upper = stmt.to_uppercase();
                if !upper.starts_with("CREATE") && !upper.starts_with("INSERT") &&
                   !upper.starts_with("ALTER") && !upper.starts_with("DROP") {
                    continue;
                }
                match sqlx::query(stmt).execute(pool).await {
                    Ok(_) => tracing::debug!("[sqlite] stmt #{} OK", idx),
                    Err(e) => tracing::error!(
                        "[sqlite] stmt #{} FAILED: {} | error: {}",
                        idx,
                        stmt.chars().take(120).collect::<String>(),
                        e
                    ),
                }
            }
            tracing::info!("[sqlite] schema initialization complete");
        }
    }

    Ok(())
}

pub async fn run(cfg: AppConfig) -> anyhow::Result<()> {
    // F3: validate JWT secret at startup
    // 默认仅 warn；如果设置了 GBSERVER__SECURITY__STRICT=1 则 fail-fast 退出
    match crate::security::validate_jwt_secret(&cfg.jwt.secret) {
        Ok(()) => tracing::info!("JWT secret OK (len={})", cfg.jwt.secret.len()),
        Err(e) => {
            let strict = std::env::var("GBSERVER__SECURITY__STRICT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if strict {
                tracing::error!("❌ JWT secret validation failed (STRICT mode): {}", e);
                tracing::error!("Generate a fresh secret with: openssl rand -hex 32");
                return Err(anyhow::anyhow!("JWT secret rejected in STRICT mode"));
            }
            tracing::warn!("⚠️  JWT secret validation failed: {}", e);
            tracing::warn!("Set GBSERVER__JWT__SECRET to a ≥32-char random hex string before production.");
        }
    }
    let pool = db::create_pool(&cfg).await?;
    let ws_state = Arc::new(crate::handlers::websocket::WsState::new());

    // SQLite 启动期设备数检查
    #[cfg(feature = "sqlite")]
    if let Some(limit) = cfg.database.sqlite_max_devices {
        match db::device::count_devices(&pool, None, None).await {
            Ok(current) => {
                let cur = current as usize;
                if cur >= limit {
                    tracing::error!(
                        "🚨 SQLite 设备数量已达上限 ({}/{}); 新设备注册将被拒绝。请迁移到 PostgreSQL。",
                        cur, limit
                    );
                } else if cur * 5 >= limit * 4 {
                    tracing::warn!(
                        "⚠️  SQLite 设备数量已用 {}% ({}/{}); 接近上限，请规划迁移到 PostgreSQL。",
                        cur * 100 / limit, cur, limit
                    );
                } else {
                    tracing::info!("SQLite device count: {}/{}", cur, limit);
                }
            }
            Err(e) => tracing::warn!("启动期设备数检查失败: {}", e),
        }
    }

    // Initialize required DB tables on startup
    init_db_tables(&pool).await?;

    let sip_server = if let Some(ref sip_config) = cfg.sip {
        if sip_config.enabled {
            let mut server = sip::SipServer::new(sip_config.clone(), pool.clone(), cfg.database.sqlite_max_devices);
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

    // Phase 4.4: 媒体节点 keepalive 超时检测 — 每 10s 扫描一次，
    // 把超过 DEFAULT_KEEPALIVE_TIMEOUT_SECS 秒无 keepalive 的节点切 offline
    {
        let health_pool = pool.clone();
        tokio::spawn(async move {
            zlm::media_node::health_check_loop(health_pool).await;
        });
    }

    if let Some(ref server) = sip_server {
        let mut server = server.write().await;
        server.set_zlm_client(zlm_client.clone());
        server.start().await?;
    }

    let download_manager = Arc::new(crate::handlers::playback::DownloadManager::new());
    let playback_manager = Arc::new(crate::handlers::playback::PlaybackManager::new());
    let jt1078_manager: Arc<tokio::sync::RwLock<Option<Arc<crate::jt1078::manager::Jt1078Manager>>>> = Arc::new(tokio::sync::RwLock::new(None));

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
        redis: redis_conn.clone(),
        // Phase 4.6: construct StateStore — Redis backend if available, else in-memory.
        state_store: {
            let store = match &cfg.redis {
                Some(rc) => crate::state_store::StateStore::redis(&rc.url),
                None => crate::state_store::StateStore::in_memory(),
            };
            Arc::new(store)
        },
        jt1078_manager: jt1078_manager.clone(),
        rpc_router: Some(Arc::new(crate::rpc::RpcRouter::new())),
    };

    // E2: 注册标准 RPC 处理器（device_control / play_stop / cloud_record_sync）
    if let Some(ref router) = state.rpc_router {
        crate::rpc::register_standard_handlers(router).await;
    }

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
        let local_device_id_for_reg = local_device_id.clone();
        let realm_for_reg = realm.clone();
        tokio::spawn(async move {
            reg.load_platforms_from_db(&pool_clone, &local_device_id_for_reg, &realm_for_reg).await;
            reg.run_registration_loop().await;
        });

        // C3: 启动 keepalive / reload / 超时检测 三个周期任务
        let periodic_reg = registrar.clone();
        let local_device_id_for_periodic = local_device_id.clone();
        let realm_for_periodic = realm.clone();
        tokio::spawn(async move {
            crate::cascade::register::cascade_periodic_tasks(
                periodic_reg,
                local_device_id_for_periodic,
                realm_for_periodic,
            )
            .await;
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

    // Start JT1078 server in background
    {
        if let Some(jcfg) = cfg.jt1078.as_ref() {
            if let Some(timeout_ms) = jcfg.timeout_ms {
                std::env::set_var("GBSERVER__JT1078__TIMEOUT_MS", timeout_ms.to_string());
            }
            if let Some(rw) = jcfg.retransmit_wait_ms {
                std::env::set_var("GBSERVER__JT1078__RETRANSMIT_WAIT_MS", rw.to_string());
            }
            if let Some(ref hook) = jcfg.retransmit_hook_url {
                std::env::set_var("GBSERVER__JT1078__RETRANSMIT_HOOK", hook.clone());
            }
        }

        let jt = crate::jt1078::Jt1078Server::new();
        let jcfg = cfg.jt1078.clone();
        let jt_mgr_for_state = jt1078_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = jt.start(jcfg).await {
                tracing::warn!("JT1078 server failed to start: {}", e);
            }
            // After start, copy manager from server to AppState so handlers can access it
            if let Some(mgr) = jt.get_manager().await {
                *jt_mgr_for_state.write().await = Some(mgr);
            }
        });
    }

    let port = cfg.server.port;
    let app = router::app(state.clone()).with_state(state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("GBServer 启动: http://{}", addr);
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
    /// Phase 4.6: unified state store (Redis or in-memory). Drives
    /// `select_least_loaded_server_filtered` so offline nodes are skipped
    /// even when the Redis stream-count cache is empty.
    pub state_store: Arc<crate::state_store::StateStore>,
    pub jt1078_manager: Arc<tokio::sync::RwLock<Option<Arc<crate::jt1078::manager::Jt1078Manager>>>>,
    pub rpc_router: Option<Arc<crate::rpc::RpcRouter>>,
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
    ///
    /// Phase 4.6：优先过滤 offline 节点（基于 `gb_media_server.status`），
    /// 然后用 `StateStore` 的流计数（Redis zset 或 in-memory）选最少负载的；
    /// state_store 不可用 / 无流计数时 fallback 到 online 列表的第一个；
    /// 全 offline 时（无可选 online）直接取 `zlm_clients.iter().next()` 兼容。
    async fn select_least_loaded(&self) -> Option<(String, Arc<zlm::ZlmClient>)> {
        if self.zlm_clients.is_empty() {
            return None;
        }
        if self.zlm_clients.len() == 1 {
            let (id, client) = self.zlm_clients.iter().next()?;
            return Some((id.clone(), client.clone()));
        }

        // Phase 4.6: 优先过滤 online 节点（last_keepalive < 30s → status=0）。
        // 若 DB 拿不到 online 列表（schema 未扩展 / DB 不可用），退化为不过滤。
        let online_ids: Vec<String> = match crate::db::media_server::list_online_servers(&self.pool).await {
            Ok(rows) => rows.into_iter().map(|s| s.id).collect(),
            Err(e) => {
                tracing::warn!("list_online_servers failed, falling back to unfiltered: {}", e);
                self.zlm_clients.keys().cloned().collect()
            }
        };

        // Step A: state_store 选 least-load among online
        if !online_ids.is_empty() {
            if let Some(id) = self.state_store.select_least_loaded_server_filtered(&online_ids) {
                if let Some(client) = self.zlm_clients.get(&id) {
                    return Some((id, client.clone()));
                }
            }

            // Step B: state_store 不可用 / 全部节点都没有流计数 → 取 online 列表第一个
            for id in &online_ids {
                if let Some(client) = self.zlm_clients.get(id) {
                    return Some((id.clone(), client.clone()));
                }
            }
        }

        // Step C: Redis 在线计数（兼容旧 fallback，未经过 offline 过滤）
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

        // Step D: 查询 ZLM API 获取实际流数（最后兼容 fallback）
        let mut min_count = usize::MAX;
        let mut best: Option<(String, Arc<zlm::ZlmClient>)> = None;
        for (id, client) in &self.zlm_clients {
            let count = client.get_active_stream_count().await.unwrap_or(usize::MAX);
            if count < min_count {
                min_count = count;
                best = Some((id.clone(), client.clone()));
            }
        }
        if best.is_some() {
            return best;
        }

        // Safety net: if all upstream signals fail (Redis/ZLM unreachable or all offline in DB),
        // return the first configured client rather than leaving callers with None.
        self.zlm_clients.iter().next().map(|(id, c)| (id.clone(), c.clone()))
    }
}

#[cfg(test)]
mod tests {
    //! Phase 4.6 — `select_least_loaded` integration with `state_store` +
    //! `db::media_server::list_online_servers`.
    //!
    //! Three SQLite-backed tests cover:
    //! - offline node is skipped, least-loaded online is picked
    //! - the actual least-loaded (smallest stream_count) wins
    //! - when all are offline, falls through to iter().next() safety net (not None)
    use super::*;
    use chrono::Utc;
    use sqlx::sqlite::SqlitePoolOptions;
    use crate::config::{ServerConfig, DatabaseConfig, JwtConfig};

    /// Build a minimal `gb_media_server` table on an in-memory SQLite pool.
    /// Returns the pool and a `status=1`/`status=0` inserter closure.
    async fn make_pool_with_servers(
        rows: &[(&str, i32)], // (id, status 0/1)
    ) -> db::Pool {
        let pool: db::Pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")
            .expect("lazy pool");

        // Minimal schema covering list_online_servers columns.
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_media_server (
                id VARCHAR(255) PRIMARY KEY,
                ip VARCHAR(50),
                hook_ip VARCHAR(50),
                sdp_ip VARCHAR(50),
                stream_ip VARCHAR(50),
                http_port INTEGER,
                http_ssl_port INTEGER,
                rtmp_port INTEGER,
                rtmp_ssl_port INTEGER,
                rtp_proxy_port INTEGER,
                rtsp_port INTEGER,
                rtsp_ssl_port INTEGER,
                flv_port INTEGER,
                flv_ssl_port INTEGER,
                mp4_port INTEGER,
                mp4_ssl_port INTEGER,
                ws_flv_port INTEGER,
                ws_flv_ssl_port INTEGER,
                jtt_proxy_port INTEGER,
                auto_config INTEGER,
                secret VARCHAR(255),
                type VARCHAR(50),
                rtp_enable INTEGER,
                rtp_port_range VARCHAR(50),
                send_rtp_port_range VARCHAR(50),
                record_assist_port INTEGER,
                default_server INTEGER,
                create_time VARCHAR(50),
                update_time VARCHAR(50),
                hook_alive_interval INTEGER,
                record_path VARCHAR(255),
                record_day INTEGER,
                transcode_suffix VARCHAR(50),
                server_id VARCHAR(255),
                ws_port INTEGER,
                wss_port INTEGER,
                record_transcode INTEGER,
                status INTEGER NOT NULL DEFAULT 0,
                last_keepalive_time VARCHAR(50),
                total_bytes BIGINT,
                active_stream_count INTEGER
            )"#,
        )
        .execute(&pool)
        .await
        .expect("create table");

        for (id, status) in rows {
            sqlx::query(
                "INSERT INTO gb_media_server (id, status) VALUES (?, ?)",
            )
            .bind(id)
            .bind(status)
            .execute(&pool)
            .await
            .expect("insert");
        }
        pool
    }

    /// Build a minimal `AppConfig` for tests. Only fields used by
    /// `select_least_loaded` are populated.
    fn make_app_config() -> AppConfig {
        AppConfig {
            server: ServerConfig { port: 18080 },
            database: DatabaseConfig {
                url: "sqlite::memory:".into(),
                sqlite_max_devices: None,
            },
            redis: None,
            jwt: JwtConfig {
                secret: "test-secret-test-secret-test-secret-1234".into(),
                expiration_minutes: 60,
            },
            static_dir: None,
            user_settings: None,
            sip: None,
            zlm: None,
            map: None,
            jt1078: None,
        }
    }

    /// Build an AppState with only the fields `select_least_loaded` reads.
    fn make_app_state(
        pool: db::Pool,
        state_store: Arc<crate::state_store::StateStore>,
        zlm_clients: HashMap<String, Arc<zlm::ZlmClient>>,
    ) -> AppState {
        AppState {
            config: Arc::new(make_app_config()),
            pool,
            sip_server: None,
            zlm_client: None,
            zlm_clients,
            playback_manager: None,
            download_manager: None,
            ws_state: Arc::new(crate::handlers::websocket::WsState::new()),
            redis: None,
            state_store,
            jt1078_manager: Arc::new(tokio::sync::RwLock::new(None)),
            rpc_router: None,
        }
    }

    #[test]
    fn test_select_least_loaded_skips_offline() {
        // 3 zlm_clients; zlm-c is offline in DB.
        // state_store sets a=10, b=3, c=1 (c is offline → must be skipped).
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let pool = rt.block_on(make_pool_with_servers(&[
            ("zlm-a", 1),
            ("zlm-b", 1),
            ("zlm-c", 0), // offline
        ]));
        let store = Arc::new(crate::state_store::StateStore::in_memory());
        // Stream counts: a=10, b=3 → state_store should pick b.
        store.set_media_server("zlm-a", crate::state_store::MediaServerLoad {
            server_id: "zlm-a".into(), stream_count: 10, rtp_server_count: 0,
            online: true, last_keepalive: Utc::now(),
        });
        store.set_media_server("zlm-b", crate::state_store::MediaServerLoad {
            server_id: "zlm-b".into(), stream_count: 3, rtp_server_count: 0,
            online: true, last_keepalive: Utc::now(),
        });
        // zlm-c has stream_count but is offline; should be skipped.
        store.set_media_server("zlm-c", crate::state_store::MediaServerLoad {
            server_id: "zlm-c".into(), stream_count: 1, rtp_server_count: 0,
            online: false, last_keepalive: Utc::now() - chrono::Duration::seconds(120),
        });

        let mut zlm_clients = HashMap::new();
        for id in ["zlm-a", "zlm-b", "zlm-c"] {
            zlm_clients.insert(
                id.to_string(),
                Arc::new(zlm::ZlmClient::new("127.0.0.1", 80, "")),
            );
        }
        let state = make_app_state(pool, store, zlm_clients);

        let (picked_id, _) = rt.block_on(state.select_least_loaded()).expect("should pick one");
        assert_eq!(picked_id, "zlm-b", "must skip offline zlm-c (lowest count)");
    }

    #[test]
    fn test_select_least_loaded_picks_least_loaded() {
        // 3 online servers; stream counts 5 / 3 / 10. Expect zlm-b (count=3).
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let pool = rt.block_on(make_pool_with_servers(&[
            ("zlm-a", 1),
            ("zlm-b", 1),
            ("zlm-c", 1),
        ]));
        let store = Arc::new(crate::state_store::StateStore::in_memory());
        for (id, count) in [("zlm-a", 5_i64), ("zlm-b", 3_i64), ("zlm-c", 10_i64)] {
            store.set_media_server(id, crate::state_store::MediaServerLoad {
                server_id: id.into(), stream_count: count, rtp_server_count: 0,
                online: true, last_keepalive: Utc::now(),
            });
        }

        let mut zlm_clients = HashMap::new();
        for id in ["zlm-a", "zlm-b", "zlm-c"] {
            zlm_clients.insert(
                id.to_string(),
                Arc::new(zlm::ZlmClient::new("127.0.0.1", 80, "")),
            );
        }
        let state = make_app_state(pool, store, zlm_clients);

        let (picked_id, _) = rt.block_on(state.select_least_loaded()).expect("should pick one");
        assert_eq!(picked_id, "zlm-b", "zlm-b has lowest stream_count=3");
    }

    #[test]
    fn test_select_least_loaded_iter_fallback_when_filter_exhausted() {
        // All servers are offline in the DB, so list_online_servers returns [].
        // Steps A-D all fail (no Redis, ZLM API unreachable).
        // The implementation falls through to the iter().next() safety net
        // rather than returning None — this preserves backward compatibility
        // with single-node deployments and multi-node setups where the DB
        // keepalive status may lag behind actual reachability.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let pool = rt.block_on(make_pool_with_servers(&[
            ("zlm-a", 0),
            ("zlm-b", 0),
        ]));
        let store = Arc::new(crate::state_store::StateStore::in_memory());

        let mut zlm_clients = HashMap::new();
        for id in ["zlm-a", "zlm-b"] {
            zlm_clients.insert(
                id.to_string(),
                Arc::new(zlm::ZlmClient::new("127.0.0.1", 80, "")),
            );
        }
        let state = make_app_state(pool, store, zlm_clients);

        let picked = rt.block_on(state.select_least_loaded());
        // iter().next() safety net always returns Some when at least one
        // client is configured, even when all are DB-offline and upstream
        // signals are unreachable.
        assert!(picked.is_some(), "fallback should still return a configured zlm_client");
        let (picked_id, _) = picked.unwrap();
        assert!(
            picked_id == "zlm-a" || picked_id == "zlm-b",
            "picked id must be one of the registered clients (got {})",
            picked_id
        );
    }
}

