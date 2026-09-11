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

/// 建一个独立的临时目录，测试结束时请调用方自行清理。
pub(crate) fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gbserver-test-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
