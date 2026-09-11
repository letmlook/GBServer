//! 共享测试支撑（仅 `cfg(test)`）
//!
//! 多个 handler 的单元测试都需要「已跑过生产 schema 的内存 SQLite」。
//! 这里集中提供，避免每个测试模块各写一份、且容易与生产 schema 漂移。
//!
//! 仅在 `sqlite` feature 下可用：schema 直接复用
//! `database/init-sqlite-2.7.4.sql`（与生产启动时 `include_str!` 的是同一份）。

#![cfg(all(test, feature = "sqlite"))]

use crate::db;

/// 建一个跑过生产 schema 的内存 SQLite 连接池。
///
/// 与 `tests/integration/sqlite_compat.rs` 的做法一致：只执行 CREATE / INSERT
/// 语句，跳过事务控制与 PRAGMA，避免驱动差异。
pub(crate) async fn sqlite_pool_with_schema() -> db::Pool {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("valid sqlite url")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("sqlite in-memory pool");

    let sql = include_str!("../database/init-sqlite-2.7.4.sql");
    let cleaned: String = sql
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.is_empty() && !t.starts_with("--")
        })
        .collect::<Vec<_>>()
        .join("\n");

    for raw in cleaned.split(';') {
        let stmt = raw.trim();
        if stmt.is_empty() {
            continue;
        }
        let upper = stmt.to_uppercase();
        if !upper.starts_with("CREATE") && !upper.starts_with("INSERT") {
            continue;
        }
        sqlx::query(stmt).execute(&pool).await.unwrap_or_else(|e| {
            panic!(
                "init SQL failed: {} | stmt: {}",
                e,
                &stmt[..80.min(stmt.len())]
            )
        });
    }
    pool
}


/// 最小可用 `AppConfig`（只填充测试关心的字段，其余为默认）
pub(crate) fn app_config() -> crate::config::AppConfig {
    use crate::config::{
        AppConfig, AuditConfig, ClusterAppConfig, DatabaseConfig, JwtConfig, RpcAppConfig,
        ServerConfig,
    };
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
        cluster: ClusterAppConfig::default(),
        audit: AuditConfig::default(),
        rpc: RpcAppConfig::default(),
    }
}

/// 构造一个自洽的 `AppState`：内存 SQLite（含生产 schema）+ 内存 StateStore +
/// 无 SIP / 无 ZLM / 无 Redis。
///
/// 用于 handler 级与 router 级测试 —— 例如验证「路由能成功构建」（可捕获
/// 历史上的 `Overlapping method route` 启动 panic）。
pub(crate) async fn app_state() -> crate::AppState {
    use std::sync::Arc;

    let pool = sqlite_pool_with_schema().await;
    let state_store = Arc::new(crate::state_store::StateStore::in_memory());

    crate::AppState {
        config: Arc::new(app_config()),
        pool,
        sip_server: None,
        zlm_client: None,
        zlm_clients: std::collections::HashMap::new(),
        playback_manager: None,
        download_manager: None,
        ws_state: Arc::new(crate::handlers::websocket::WsState::new()),
        ws_hub: Arc::new(crate::ws::WsHub::new("test-node".to_string(), None)),
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
