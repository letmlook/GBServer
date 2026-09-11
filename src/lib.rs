pub mod config;
pub mod error;
pub mod response;
pub mod auth;
pub mod archive;
pub mod db;
#[cfg(all(test, feature = "sqlite"))]
pub(crate) mod test_support;
pub mod handlers;
pub mod router;
pub mod sip;
pub mod zlm;
pub mod jt1078;
pub mod scheduler;
pub mod cascade;
pub mod metrics;
pub mod rpc;
pub mod state_store;
pub mod state;
pub mod security;
pub mod cluster;
pub mod ws;
pub mod middleware;

use config::AppConfig;
use std::collections::HashMap;
use std::sync::Arc;

async fn init_db_tables(pool: &db::Pool) -> anyhow::Result<()> {
    db::position_history::ensure_table(pool).await?;
    db::audit_log::ensure_table(pool).await?;
    db::alarm::ensure_columns(pool).await?;
    db::common_channel::ensure_columns(pool).await?;
    // Phase 4.5: 幂等迁移 —— 流状态统一字段
    let _ = db::stream_push::ensure_stream_status_column(pool).await;
    let _ = db::stream_proxy::ensure_stream_status_column(pool).await;
    // 旧版 SQLite 库升级补建（幂等）：下列表曾缺失于 init-sqlite-2.7.4.sql，
    // 旧库核心表齐全、不会触发全量 init，需启动时单独补建（仅 SQLite 需要，
    // PG/MySQL init 脚本一直包含这些表）。
    #[cfg(feature = "sqlite")]
    ensure_sqlite_upgrade_tables(pool).await?;

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
        // Phase 7.6: 检测多个核心表确保 schema 完整；任一缺失即触发 init。
        let core_tables = ["gb_device", "gb_common_region", "gb_online_user", "gb_cluster_node"];
        let mut missing_any = false;
        for tbl in &core_tables {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)"
            )
            .bind(tbl)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
            if !exists {
                missing_any = true;
                tracing::warn!("[sqlite] missing core table: {}", tbl);
            }
        }

        if missing_any {
            tracing::info!("[sqlite] schema tables missing — running incremental init");
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
                // Only CREATE statements use IF NOT EXISTS so they're safe to
                // re-run; INSERT statements inside init-sqlite would only apply
                // to a fresh schema. We accept re-execution since core seeds
                // are idempotent (use INSERT OR IGNORE equivalent via WHERE NOT EXISTS).
                match sqlx::query(stmt).execute(pool).await {
                    Ok(_) => tracing::debug!("[sqlite] stmt #{} OK", idx),
                    Err(e) => tracing::warn!(
                        "[sqlite] stmt #{} SKIP: {} | error: {}",
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

/// 旧版 SQLite 库升级补建：gb_platform_channel / gb_platform_group /
/// gb_platform_region / gb_jt_channel 曾缺失于 init-sqlite-2.7.4.sql，
/// 已有库因核心表检测通过而跳过全量 init，这里幂等补齐（列定义与
/// PostgreSQL/MySQL 脚本对齐，见 db::platform_channel / db::jt1078）。
#[cfg(feature = "sqlite")]
async fn ensure_sqlite_upgrade_tables(pool: &db::Pool) -> anyhow::Result<()> {
    const STMTS: &[&str] = &[
        r#"CREATE TABLE IF NOT EXISTS gb_platform_channel (
            id                           INTEGER PRIMARY KEY AUTOINCREMENT,
            platform_id                  INTEGER,
            device_channel_id            INTEGER,
            custom_device_id             VARCHAR(50),
            custom_name                  VARCHAR(255),
            custom_manufacturer          VARCHAR(50),
            custom_model                 VARCHAR(50),
            custom_owner                 VARCHAR(50),
            custom_civil_code            VARCHAR(50),
            custom_block                 VARCHAR(50),
            custom_address               VARCHAR(50),
            custom_parental              INTEGER,
            custom_parent_id             VARCHAR(50),
            custom_safety_way            INTEGER,
            custom_register_way          INTEGER,
            custom_cert_num              VARCHAR(50),
            custom_certifiable           INTEGER,
            custom_err_code              INTEGER,
            custom_end_time              VARCHAR(50),
            custom_secrecy               INTEGER,
            custom_ip_address            VARCHAR(50),
            custom_port                  INTEGER,
            custom_password              VARCHAR(255),
            custom_status                VARCHAR(50),
            custom_longitude             REAL,
            custom_latitude              REAL,
            custom_ptz_type              INTEGER,
            custom_position_type         INTEGER,
            custom_room_type             INTEGER,
            custom_use_type              INTEGER,
            custom_supply_light_type     INTEGER,
            custom_direction_type        INTEGER,
            custom_resolution            VARCHAR(255),
            custom_business_group_id     VARCHAR(255),
            custom_download_speed        VARCHAR(255),
            custom_svc_space_support_mod INTEGER,
            custom_svc_time_support_mode INTEGER
        )"#,
        "CREATE UNIQUE INDEX IF NOT EXISTS uk_platform_gb_channel_platform_device \
         ON gb_platform_channel (platform_id, device_channel_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uk_platform_gb_channel_custom_device \
         ON gb_platform_channel (custom_device_id)",
        r#"CREATE TABLE IF NOT EXISTS gb_platform_group (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            platform_id INTEGER,
            group_id    INTEGER
        )"#,
        "CREATE UNIQUE INDEX IF NOT EXISTS uk_gb_platform_group_platform_id_group_id \
         ON gb_platform_group (platform_id, group_id)",
        r#"CREATE TABLE IF NOT EXISTS gb_platform_region (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            platform_id INTEGER,
            region_id   INTEGER
        )"#,
        "CREATE UNIQUE INDEX IF NOT EXISTS uk_gb_platform_region_platform_id_region_id \
         ON gb_platform_region (platform_id, region_id)",
        r#"CREATE TABLE IF NOT EXISTS gb_jt_channel (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            terminal_db_id INTEGER,
            channel_id     INTEGER,
            has_audio      INTEGER DEFAULT 0,
            name           VARCHAR(255),
            update_time    VARCHAR(50) NOT NULL,
            create_time    VARCHAR(50) NOT NULL
        )"#,
        "CREATE UNIQUE INDEX IF NOT EXISTS uk_jt_channel_terminal_channel \
         ON gb_jt_channel (terminal_db_id, channel_id)",
    ];
    for stmt in STMTS {
        if let Err(e) = sqlx::query(stmt).execute(pool).await {
            // 语句均带 IF NOT EXISTS，唯一可能的失败是磁盘/权限问题，记录后继续
            tracing::warn!("[sqlite] upgrade table stmt skip: {}", e);
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

    let mut sip_server = if let Some(ref sip_config) = cfg.sip {
        if sip_config.enabled {
            let mut server = sip::SipServer::new(sip_config.clone(), pool.clone(), cfg.database.sqlite_max_devices);
            server.set_ws_state(ws_state.clone()).await;
            Some(Arc::new(server))
        } else {
            None
        }
    } else {
        None
    };

    // Construct ZLM clients FIRST so we can wire them into the SipServer
    // before any other Arc clones of SipServer exist.
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

    // Phase 4.4 + follow-up: 媒体节点 keepalive 超时检测 — 每 N 秒扫描一次，
    // 把超过 timeout_secs 秒无 keepalive 的节点切 offline
    // (timeout_secs / grace_count / check_interval_secs 来自 [zlm.keepalive])
    {
        let health_pool = pool.clone();
        let keepalive_cfg = cfg.zlm.as_ref()
            .map(|z| z.keepalive.clone())
            .unwrap_or_default();
        let health_config = zlm::media_node::HealthCheckConfig {
            timeout_secs: keepalive_cfg.timeout_secs,
            grace_count: keepalive_cfg.grace_count,
            check_interval_secs: keepalive_cfg.check_interval_secs,
        };
        tracing::info!(
            "MediaNode health check started: timeout={}s, grace={}, interval={}s",
            health_config.timeout_secs, health_config.grace_count, health_config.check_interval_secs
        );
        tokio::spawn(async move {
            zlm::media_node::health_check_loop(health_pool, health_config).await;
        });
    }

    // Wire cascade registrar, ZLM client, start, and spawn run() task ALL
    // here, before sip_server is cloned into AppState. We need every mutator
    // to run while Arc::strong_count(&sip_server) == 1.
    {
        let registrar = Arc::new(crate::cascade::CascadeRegistrar::new());
        let local_device_id = cfg.sip.as_ref()
            .map(|s| s.device_id.clone())
            .unwrap_or_else(|| "local_device".to_string());
        let realm = cfg.sip.as_ref()
            .map(|s| s.realm.clone())
            .unwrap_or_else(|| "GBServerRealm".to_string());

        if let Some(sip_srv) = sip_server.as_mut() {
            let sip_mut = Arc::get_mut(sip_srv)
                .expect("SipServer Arc must be unique at this point (no clones yet)");
            // set_cascade_registrar
            sip_mut.set_cascade_registrar(registrar.clone());
            // set_zlm_client (uses selected zlm_client)
            sip_mut.set_zlm_client(zlm_client.clone());
            // start() binds UDP/TCP listeners; safe to call now
            sip_mut.start().await?;
        }
        registrar.set_pool(pool.clone()).await;

        // Clone the Arc into the registrar — this bumps strong count to 2,
        // but no further mutators run on the SipServer.
        if let Some(sip_srv) = sip_server.as_ref() {
            registrar.set_sip_server(sip_srv.clone()).await;
        }

        // Spawn the SIP server run() task. Strong count is now 3 (binding +
        // registrar + spawned task). All future operations are reads.
        if let Some(srv) = sip_server.as_ref() {
            let srv_clone = srv.clone();
            tokio::spawn(async move {
                if let Err(e) = srv_clone.run().await {
                    tracing::error!("SIP Server error: {}", e);
                }
            });
        }

        // Spawn the cascade periodic tasks and registration loop.
        let reg = registrar.clone();
        let local_device_id_for_reg = local_device_id.clone();
        let realm_for_reg = realm.clone();
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            reg.load_platforms_from_db(&pool_clone, &local_device_id_for_reg, &realm_for_reg).await;
            reg.run_registration_loop().await;
        });

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

    let mut state = AppState {
        config: Arc::new(cfg.clone()),
        pool,
        sip_server: sip_server.clone(),
        zlm_client,
        zlm_clients,
        playback_manager: Some(playback_manager),
        download_manager: Some(download_manager),
        ws_state,
        // Phase 7.3: cluster-aware WS hub. Receives RPC `ws_broadcast` from
        // sibling nodes and fans out to local clients.
        // (constructed below with the router reference)
        ws_hub: Arc::new(crate::ws::WsHub::new(cfg.cluster.node_id.clone(), None)),
        redis: redis_conn.clone(),
        // Phase 4.6: construct StateStore — Redis backend if available, else in-memory.
        state_store: {
            let store = match &cfg.redis {
                Some(rc) => crate::state_store::StateStore::redis(&rc.url),
                None => crate::state_store::StateStore::in_memory(),
            };
            Arc::new(store)
        },
        // Phase 7.1: business-level repository facade over StateStore.
        state_repo: {
            let store = match &cfg.redis {
                Some(rc) => crate::state_store::StateStore::redis(&rc.url),
                None => crate::state_store::StateStore::in_memory(),
            };
            Arc::new(crate::state::StateStoreRepository::new(Arc::new(store)))
        },
        jt1078_manager: jt1078_manager.clone(),
        rpc_router: Some(Arc::new(crate::rpc::RpcRouter::new())),
        // Phase 7.2: cluster registry. In single_node_mode without Redis, returns
        // only the local node from list_active(); otherwise reads from Redis SET/ZSET.
        cluster_registry: {
            let redis_conn = redis_conn.clone();
            let cluster_cfg = crate::cluster::ClusterConfig {
                enabled: cfg.cluster.enabled,
                single_node_mode: cfg.cluster.single_node_mode,
                node_id: cfg.cluster.node_id.clone(),
                addr: cfg.cluster.addr.clone(),
                role: cfg.cluster.role.clone(),
                heartbeat_interval: cfg.cluster.heartbeat_interval(),
                heartbeat_ttl: cfg.cluster.heartbeat_ttl(),
            };
            let redis_arc = redis_conn.map(|c| Arc::new(tokio::sync::Mutex::new(c)));
            Arc::new(crate::cluster::ClusterRegistry::new(cluster_cfg, redis_arc))
        },
    };

    // Phase 7.3: 注册 ws_broadcast RPC handler — 接收其他节点 fanout 来的
    // WS 事件并分发到本节点客户端（出站方向由 WsHub::broadcast_event 经
    // outbound 传输完成；from_node == 本节点时忽略，防回环）。
    if let Some(ref router) = state.rpc_router {
        let hub_clone = state.ws_hub.clone();
        let self_node = state.cluster_registry.config().node_id.clone();
        struct WsBroadcastHandler { hub: Arc<crate::ws::WsHub>, self_node: String }
        impl crate::rpc::RpcHandler for WsBroadcastHandler {
            fn name(&self) -> &str { "ws_broadcast" }
            fn handle(&self, payload: serde_json::Value) -> crate::rpc::RpcResponse {
                if payload.get("from_node").and_then(|v| v.as_str()) == Some(&self.self_node) {
                    return crate::rpc::RpcResponse { ok: true, result: None, error: None };
                }
                let hub = self.hub.clone();
                // Spawn async dispatch — RpcHandler is sync; use tokio::spawn.
                tokio::spawn(async move {
                    hub.handle_rpc_broadcast(payload).await;
                });
                crate::rpc::RpcResponse { ok: true, result: None, error: None }
            }
        }
        router.register(WsBroadcastHandler { hub: hub_clone, self_node }).await;
    }

    // Phase 7.3: 把 state.rpc_router 注入 ws_hub（让 hub 能 cluster 广播）
    // WsHub::set_router 只能通过 Arc::get_mut 调用（要求 strong_count==1）；
    // run() 是唯一构造 ws_hub 的地方，所以此处一定为 1。
    if let Some(router) = state.rpc_router.clone() {
        if let Some(hub_mut) = Arc::get_mut(&mut state.ws_hub) {
            hub_mut.set_router(Some(router));
        }
    }

    // Phase 7.3+: WsState 广播统一委托给 WsHub（事件过滤 + 集群外发）。
    // 生产方（sip/zlm/jt1078 等）保持调用 ws_state.broadcast 不变。
    state.ws_state.set_hub(state.ws_hub.clone()).await;

    // E2: [rpc].peer_endpoints 非空时启用 HttpRpc 出站传输（POST 对端 /api/rpc）
    if let Some(ref router) = state.rpc_router {
        if !cfg.rpc.peer_endpoints.is_empty() {
            let node_id = cfg.rpc.node_id.clone()
                .unwrap_or_else(|| state.cluster_registry.config().node_id.clone());
            let http_rpc = crate::rpc::HttpRpc::new(&node_id, crate::rpc::HttpRpcConfig {
                peer_endpoints: cfg.rpc.peer_endpoints.clone(),
                timeout_secs: cfg.rpc.timeout_secs.unwrap_or(5),
                secret: cfg.rpc.secret.clone(),
            });
            router.register_outbound(Arc::new(http_rpc)).await;
            if cfg.rpc.secret.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
                tracing::warn!(
                    "多节点 RPC 已启用（{} 个对端）但未配置 [rpc].secret —— 入站 /api/rpc 将不校验来源，                     建议设置共享密钥",
                    cfg.rpc.peer_endpoints.len()
                );
            }
            tracing::info!("HttpRpc outbound enabled: {} peers", cfg.rpc.peer_endpoints.len());
        }
    }

    // Phase 7.2: 启动 cluster heartbeat task（自动 evict 过期节点）
    {
        let registry = state.cluster_registry.clone();
        let _hb = registry.start_heartbeat_task();
    }

    // Phase 7.2: 启动 Redis RPC（如果 Redis 已配置）——同时作为入站 subscriber
    // 和出站 broadcast 传输
    if let (Some(ref router), Some(redis_manager)) = (&state.rpc_router, &state.redis) {
        let cfg_url = state.config.redis.as_ref().map(|r| r.url.clone());
        if let Some(url) = cfg_url {
            let client = match redis::Client::open(url.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("RedisRpcTransport: cannot open client: {}", e);
                    return Ok(());
                }
            };
            let transport = Arc::new(crate::rpc::RedisRpcTransport::new(
                state.cluster_registry.config().node_id.clone(),
                Arc::new(tokio::sync::Mutex::new(redis_manager.clone())),
                client,
            ));
            let transport_clone = transport.clone();
            let router_clone = router.clone();
            let _sub = transport.clone().start_subscriber().await;
            tokio::spawn(async move {
                use crate::rpc::RpcTransport;
                let mut rx = transport_clone.receive();
                while let Ok(req) = rx.recv().await {
                    let _ = router_clone.route(&req).await;
                }
            });
            router.register_outbound(transport as Arc<dyn crate::rpc::RpcTransport>).await;
            tracing::info!("RedisRpcTransport subscriber started (inbound + outbound)");
        }
    }

    // Defer the SIP server run() spawn until AFTER the cascade registrar
    // is wired up, so the run() task is the second Arc clone (and after
    // cascade init, no other mutators run on the SipServer).

    // Start RecordPlanScheduler
    {
        let scheduler_pool = state.pool.clone();
        let scheduler_zlm = state.zlm_client.clone();
        let scheduler_clients = state.zlm_clients.clone();
        tokio::spawn(async move {
            let mut scheduler = crate::scheduler::record_plan::RecordPlanScheduler::new(
                scheduler_pool,
                scheduler_zlm,
            );
            scheduler.set_zlm_clients(scheduler_clients);
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
    pub sip_server: Option<Arc<sip::SipServer>>,
    pub zlm_client: Option<Arc<zlm::ZlmClient>>,
    pub zlm_clients: HashMap<String, Arc<zlm::ZlmClient>>,
    pub playback_manager: Option<Arc<crate::handlers::playback::PlaybackManager>>,
    pub download_manager: Option<Arc<crate::handlers::playback::DownloadManager>>,
    pub ws_state: Arc<crate::handlers::websocket::WsState>,
    /// Phase 7.3: cluster-aware WebSocket fanout hub.
    pub ws_hub: Arc<crate::ws::WsHub>,
    pub redis: Option<redis::aio::ConnectionManager>,
    /// Phase 4.6: unified state store (Redis or in-memory). Drives
    /// `select_least_loaded_server_filtered` so offline nodes are skipped
    /// even when the Redis stream-count cache is empty.
    pub state_store: Arc<crate::state_store::StateStore>,
    /// Phase 7.1: business-level repository facade over StateStore.
    pub state_repo: Arc<crate::state::StateStoreRepository>,
    pub jt1078_manager: Arc<tokio::sync::RwLock<Option<Arc<crate::jt1078::manager::Jt1078Manager>>>>,
    pub rpc_router: Option<Arc<crate::rpc::RpcRouter>>,
    /// Phase 7.2: cluster registry (Redis-backed node discovery).
    pub cluster_registry: Arc<crate::cluster::ClusterRegistry>,
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
    /// 回退链（从优到次）：
    /// 1. 优先过滤 offline 节点（基于 `gb_media_server.status`），再用 `StateStore`
    ///    的流计数（Redis zset 或 in-memory）选最少负载的；
    /// 2. state_store 无流计数 / 选择失败 → 取 online 列表的第一个；
    /// 3. DB 拿不到 online 列表 → 用 ZLM 实时 `get_active_stream_count` 选最少负载的；
    /// 4. 全部信号失败 → 兜底取 `zlm_clients.iter().next()` 兼容。
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

        // Step C: 查询 ZLM API 获取实际流数（最终 tie-breaker）
        // 设计取舍：offline 过滤在 Step A/B 完成；仅当 DB 拿不到 online 列表
        // （online_ids 为空）时才走到这里，用 ZLM 实时 `get_active_stream_count`
        // 选最少负载的节点。如果某个被检测 offline 但 ZLM 实际可达的节点在此处胜出，
        // 也不会造成数据/连接错误（业务上只是把流推到该节点），并且会被
        // `health_check_loop`（media_node.rs）10s 内重新标记为 online / offline
        // 收敛到 Step A 的路径。Safety net 在所有上游信号失败时仍会兜底返回首个节点。
        //
        // 历史：此处原有一步「读 Redis `gb:ms:streams:*` 计数」的次级 fallback。
        // 该计数由 `zlm/hook.rs` 维护，与 StateStore 完全重复（Phase 7.1 起 hook
        // 已同时写 StateStore），且 Step A 的 StateStore 查询已覆盖它；删除后
        // 唯一状态源收敛为 StateStore，本步用 ZLM 实时数据兜底。
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

        // Safety net: if all upstream signals fail (ZLM unreachable or all offline in DB),
        // return the first configured client rather than leaving callers with None.
        self.zlm_clients.iter().next().map(|(id, c)| (id.clone(), c.clone()))
    }
}

// Phase 5.6: 这些测试是 SQLite-specific（使用 SqlitePoolOptions / SQLite DDL），
// 在 --features postgres / mysql 下编译会失败。整组测试 cfg-gate。
#[cfg(all(test, feature = "sqlite"))]
mod tests {
    //! Phase 4.6 — `select_least_loaded` integration with `state_store` +
    //! `db::media_server::list_online_servers`.
    //!
    //! Three SQLite-backed tests cover:
    //! - offline node is skipped, least-loaded online is picked
    //! - the actual least-loaded (smallest stream_count) wins
    //! - when all are offline, falls through to iter().next() safety net (not None)
    // Phase 5.6: 修复 PG/MySQL 编译失败 — 该测试仅在 sqlite feature 下有意义
    #[cfg(feature = "sqlite")]
    use sqlx::sqlite::SqlitePoolOptions;
    use super::*;
    use chrono::Utc;
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
            server: ServerConfig {
                port: 18080,
                download_dir: None,
                record_root: None,
            },
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
            cluster: crate::config::ClusterAppConfig::default(),
            audit: crate::config::AuditConfig::default(),
            rpc: crate::config::RpcAppConfig::default(),
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
            ws_hub: Arc::new(crate::ws::WsHub::new("node-test".to_string(), None)),
            redis: None,
            state_store: state_store.clone(),
            state_repo: Arc::new(crate::state::StateStoreRepository::new(state_store)),
            jt1078_manager: Arc::new(tokio::sync::RwLock::new(None)),
            rpc_router: None,
            cluster_registry: Arc::new(crate::cluster::ClusterRegistry::new(
                crate::cluster::ClusterConfig::default(),
                None,
            )),
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

