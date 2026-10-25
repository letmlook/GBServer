#![cfg(feature = "postgres")]

/// PostgreSQL 回归：**方言判据必须看 pool 自己的类型**，不能看进程级全局。
///
/// ## 为什么必须有这个测试
///
/// 改造后方言分支从「编译期 `#[cfg(feature)]`」变成「运行期按 pool 的类型
/// 分发」。判据必须是**池自身的类型** —— 进程级全局 `dialect::current()` 只在
/// 「一个进程一个库」时与 pool 一致。
///
/// 本测试**直接构造** `db::Pool::Postgres(..)`（不经过 `create_pool`），
/// 于是进程级全局仍是默认值 sqlite：
///
/// - 修复前：`db::audit_log::delete_before` 用的是读全局方言的
///   `days_ago_expr`，在 postgres 上生成 sqlite 专属的
///   `datetime('now', '-90 days')` → postgres 直接报
///   `function datetime(unknown, unknown) does not exist`
///   → **审计日志保留策略在 postgres 上 100% 失效**（每次清理都失败，
///   `gb_audit_log` 无限增长）；
/// - 修复后：按 pool 自己的方言生成 `NOW() - INTERVAL '90 days'` → 正常执行。
///
/// 只有**真的连上 postgres** 才能暴露 —— sqlite 下 `datetime()` 合法，
/// 单测全绿也证明不了任何事。
///
/// ```bash
/// export TEST_POSTGRES_URL='postgres://postgres:postgrespw@127.0.0.1:5432/gbserver_pg_test'
/// cargo test --locked --test postgres_dialect_judgment -- --ignored
/// ```
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn audit_log_retention_uses_the_pools_own_dialect() {
    let url = std::env::var("TEST_POSTGRES_URL").expect("TEST_POSTGRES_URL is required");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect PostgreSQL");
    // 直接构造 → 进程级全局方言仍是默认 sqlite
    let db_pool = gbserver::db::Pool::Postgres(pool.clone());

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS gb_audit_log (\
            id BIGSERIAL PRIMARY KEY, \
            username VARCHAR(100), \
            action VARCHAR(200), \
            resource VARCHAR(200), \
            method VARCHAR(10), \
            path VARCHAR(500), \
            ip VARCHAR(50), \
            status_code INT, \
            request_body TEXT, \
            elapsed_ms BIGINT, \
            create_time TIMESTAMP DEFAULT NOW())",
    )
    .execute(&pool)
    .await
    .expect("create gb_audit_log");

    // 关键断言：这条语句必须能在 postgres 上执行
    gbserver::db::audit_log::delete_before(&db_pool, 90)
        .await
        .expect("审计日志保留策略在 postgres 上必须能执行（判据必须是 pool 自己的方言）");
}
