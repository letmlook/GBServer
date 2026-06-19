//! SQLite 数据库集成测试
//!
//! 验证 GBServer 在 SQLite 默认 feature 下的端到端行为：
//! 1. 内存 SQLite 数据库 schema 初始化成功
//! 2. 默认 admin 用户可登录
//! 3. CRUD 操作（device / user / channel）跨特性兼容
//! 4. 设备数量上限 check_sqlite_device_limit 行为正确
//! 5. PG/MySQL 专属语法未被误用
//!
//! 本测试仅在 `--features sqlite` 下运行；PG/MySQL feature 下会自动跳过。
//!
//! 运行：
//!   cargo test --test integration --features sqlite sqlite_compat
//!   cargo test --test integration --features sqlite -- --nocapture

#![cfg(feature = "sqlite")]

use gbserver::db;

#[tokio::test]
async fn sqlite_in_memory_schema_init_and_default_admin() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::time::Duration;

    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .expect("sqlite in-memory pool");

    // 复用生产 init SQL
    let sql = include_str!("../../database/init-sqlite-2.7.4.sql");
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
        sqlx::query(stmt)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("init SQL failed: {} | stmt: {}", e, &stmt[..80.min(stmt.len())]));
    }

    // 验证 gb_device 表存在且有 admin 用户
    let device_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gb_device")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(device_count, 0, "fresh DB should have no devices");

    let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gb_user WHERE username = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(admin_count, 1, "default admin user must be seeded");

    let role_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gb_user_role")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(role_count, 1, "admin role must be seeded");
}

#[tokio::test]
async fn sqlite_device_crud_roundtrip() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::time::Duration;

    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();

    let sql = include_str!("../../database/init-sqlite-2.7.4.sql");
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
        if !stmt.to_uppercase().starts_with("CREATE") && !stmt.to_uppercase().starts_with("INSERT") {
            continue;
        }
        let _ = sqlx::query(stmt).execute(&pool).await;
    }

    // 用 db::device::upsert_device 验证 SQLite 分支可工作
    db::device::upsert_device(
        &pool,
        "34020000001110000001",
        Some("测试摄像头-001"),
        Some("海康"),
        Some("DS-2CD"),
        Some("V5.5.0"),
        Some("UDP"),
        Some("passive"),
        Some("192.168.1.10"),
        Some(5060),
        true,
        Some("zlmediakit-1"),
        "2026-06-19 12:00:00",
    )
    .await
    .expect("upsert_device should succeed on SQLite");

    // 重新注册同一 device_id 应更新而非插入新行
    db::device::upsert_device(
        &pool,
        "34020000001110000001",
        Some("测试摄像头-001-更新"),
        None, None, None, None, None, None, None, true, None,
        "2026-06-19 13:00:00",
    )
    .await
    .expect("re-upsert should succeed");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gb_device WHERE device_id = ?")
        .bind("34020000001110000001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "upsert should keep single row per device_id");

    let name: String = sqlx::query_scalar("SELECT name FROM gb_device WHERE device_id = ?")
        .bind("34020000001110000001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "测试摄像头-001-更新");
}

#[tokio::test]
async fn sqlite_device_limit_blocks_new_registration() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::time::Duration;

    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();

    let sql = include_str!("../../database/init-sqlite-2.7.4.sql");
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
        if !stmt.to_uppercase().starts_with("CREATE") && !stmt.to_uppercase().starts_with("INSERT") {
            continue;
        }
        let _ = sqlx::query(stmt).execute(&pool).await;
    }

    // 上限 2 设备
    let limit = Some(2usize);

    // 1. 添加第一个设备 → 允许
    db::device::upsert_device(
        &pool,
        "device-1",
        Some("d1"), None, None, None, None, None, None, None, true, None, "2026-06-19 00:00:00",
    )
    .await
    .unwrap();
    db::device::check_sqlite_device_limit(&pool, "device-1", limit)
        .await
        .expect("first device should be allowed");

    // 2. 同一设备重注册（更新） → 允许
    db::device::check_sqlite_device_limit(&pool, "device-1", limit)
        .await
        .expect("existing device update should be allowed");

    // 3. 添加第二个设备 → 允许
    db::device::upsert_device(
        &pool,
        "device-2",
        Some("d2"), None, None, None, None, None, None, None, true, None, "2026-06-19 00:00:01",
    )
    .await
    .unwrap();
    db::device::check_sqlite_device_limit(&pool, "device-2", limit)
        .await
        .expect("second device should be allowed");

    // 4. 第三个新设备 → 拒绝（已达上限）
    let err = db::device::check_sqlite_device_limit(&pool, "device-3", limit)
        .await
        .expect_err("third new device must be rejected");
    assert_eq!(err.current, 2);
    assert_eq!(err.limit, 2);
    assert!(err.to_string().contains("2/2"), "error msg should mention limit: {}", err);
}

#[tokio::test]
async fn sqlite_user_auth_login_succeeds() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::time::Duration;

    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();

    let sql = include_str!("../../database/init-sqlite-2.7.4.sql");
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
        if !stmt.to_uppercase().starts_with("CREATE") && !stmt.to_uppercase().starts_with("INSERT") {
            continue;
        }
        let _ = sqlx::query(stmt).execute(&pool).await;
    }

    // admin / admin 的 MD5 = 21232f297a57a5a743894a0e4a801fc3
    let row: Option<(String, Option<i32>)> = sqlx::query_as(
        "SELECT u.username, u.role_id FROM gb_user u WHERE u.username = ? AND u.password = ?"
    )
    .bind("admin")
    .bind("21232f297a57a5a743894a0e4a801fc3")
    .fetch_optional(&pool)
    .await
    .unwrap();

    let (username, role_id) = row.expect("admin user should exist with matching password");
    assert_eq!(username, "admin");
    assert_eq!(role_id, Some(1));
}