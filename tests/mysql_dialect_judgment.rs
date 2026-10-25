#![cfg(feature = "mysql")]

/// MySQL 回归：**方言判据必须看 pool 自己的类型**，不能看进程级全局。
///
/// 与 `postgres_dialect_judgment.rs` 同理：本测试直接构造
/// `db::Pool::MySql(..)`（不经过 `create_pool`），进程级全局仍是默认 sqlite。
///
/// 修复前 `db::audit_log::delete_before` 生成的是 sqlite 专属的
/// `datetime('now', '-90 days')` → MySQL 报
/// `FUNCTION gbserver.datetime does not exist` → **审计日志保留策略在 MySQL 上
/// 100% 失效**。sqlite 下 `datetime()` 合法，单测全绿也证明不了任何事。
///
/// ```bash
/// export TEST_MYSQL_URL='mysql://root:rootpw@127.0.0.1:3306/gbserver_mysql_test'
/// cargo test --locked --test mysql_dialect_judgment -- --ignored
/// ```
#[tokio::test]
#[ignore = "requires TEST_MYSQL_URL and an isolated MySQL database"]
async fn audit_log_retention_uses_the_pools_own_dialect() {
    let url = std::env::var("TEST_MYSQL_URL").expect("TEST_MYSQL_URL is required");
    let pool = sqlx::MySqlPool::connect(&url).await.expect("connect MySQL");
    // 直接构造 → 进程级全局方言仍是默认 sqlite
    let db_pool = gbserver::db::Pool::MySql(pool.clone());

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS gb_audit_log (\
            id BIGINT AUTO_INCREMENT PRIMARY KEY, \
            username VARCHAR(100), \
            action VARCHAR(200), \
            resource VARCHAR(200), \
            method VARCHAR(10), \
            path VARCHAR(500), \
            ip VARCHAR(50), \
            status_code INT, \
            request_body TEXT, \
            elapsed_ms BIGINT, \
            create_time DATETIME DEFAULT CURRENT_TIMESTAMP)",
    )
    .execute(&pool)
    .await
    .expect("create gb_audit_log");

    // 关键断言：这条语句必须能在 MySQL 上执行
    gbserver::db::audit_log::delete_before(&db_pool, 90)
        .await
        .expect("审计日志保留策略在 MySQL 上必须能执行（判据必须是 pool 自己的方言）");
}
