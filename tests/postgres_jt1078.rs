#![cfg(feature = "postgres")]

/// PostgreSQL 专属回归测试：**JT1078「会话超时 ⇒ 终端置离线」在 postgres 上必须真的生效**。
///
/// ## 为什么必须有这个测试
///
/// 这条链路（`jt1078::manager::cleanup_once` → `db::jt1078::mark_terminal_offline_if_stale`）
/// 在 sqlite / mysql 上一直是好的，**只有 postgres 会炸**，而且炸的原因有两个，
/// 各自独立、都能让整条链路 100% 失效：
///
/// 1. **裸 `?` 占位符**：postgres 的预编译语句协议只认 `$1`，
///    直接发 `?` → `syntax error at end of input`；
/// 2. **`id` 解码成 `i64` 而列是 INT4**：postgres 的 `gb_jt_terminal.id` 是
///    `serial` → **integer(4)**，而 sqlx-postgres 的类型检查是**严格**的：
///    `i64`（INT8）解 INT4 列直接报
///    `mismatched types; Rust type i64 (as SQL type INT8) is not compatible with SQL type INT4`。
///
/// 两个错误都被 `cleanup_once` 吞成一条 `tracing::warn!`（见
/// `src/jt1078/manager.rs`），于是：**终端永远显示在线，平台照样给它下发命令**，
/// 而「超时 ⇒ 真离线 ⇒ 需重新注册 + 重新鉴权」这条闭环彻底失效。
///
/// ## 为什么单测抓不到
///
/// `cargo test` 默认跑 sqlite —— `?` 合法、`INTEGER` 兼容 `i64`，**两个坑都不存在**，
/// 单测全绿也证明不了任何事。sqlite / mysql 的方言冒烟同样全绿。
/// 只有**真的连上 postgres** 才能暴露。所以本文件默认 `#[ignore]`，
/// 需要显式跑（**逐个 test target 指定**，不要用 `cargo test -- --ignored` ——
/// 那会把标记为 ignore 的 doc-test 伪代码示例也拉出来跑）：
///
/// ```bash
/// export TEST_POSTGRES_URL='postgres://postgres:postgres@127.0.0.1:5432/gbserver_test'
/// cargo test --locked --test postgres_jt1078 -- --ignored
/// ```
use std::time::Duration;

/// 造一个**干净**的隔离库（schema 来自 init-postgresql-2.7.4.sql）。
async fn test_pool() -> sqlx::PgPool {
    let url = std::env::var("TEST_POSTGRES_URL").expect("TEST_POSTGRES_URL is required");
    sqlx::PgPool::connect(&url)
        .await
        .expect("connect PostgreSQL")
}

/// 插入一行终端，`update_time` 由调用方给定。
async fn insert_terminal(pool: &sqlx::PgPool, phone: &str, update_time: &str) -> i64 {
    // postgres 用 `RETURNING id` 取自增主键（`last_insert_id()` 是 mysql/sqlite 的）。
    sqlx::query_scalar::<_, i32>(
        "INSERT INTO gb_jt_terminal (phone_number, status, create_time, update_time) \
         VALUES ($1, true, $2, $2) RETURNING id",
    )
    .bind(phone)
    .bind(update_time)
    .fetch_one(pool)
    .await
    .expect("insert fixture") as i64
}

async fn status_of(pool: &sqlx::PgPool, id: i64) -> Option<bool> {
    sqlx::query_scalar::<_, bool>("SELECT status FROM gb_jt_terminal WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("read status")
}

async fn cleanup(pool: &sqlx::PgPool, phone: &str) {
    sqlx::query("DELETE FROM gb_jt_terminal WHERE phone_number = $1")
        .bind(phone)
        .execute(pool)
        .await
        .expect("clean fixture");
}

/// **核心回归**：过期的终端在 postgres 上必须被置成离线。
///
/// 旧实现（裸 `?` + `i64` 解 INT4）在这里会直接返回 `Err` → 断言失败。
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn stale_terminal_is_marked_offline_on_postgres() {
    let pool = test_pool().await;
    // `db::jt1078::*` 现在接受运行期多态的 `db::Pool`（原生 PgPool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::Postgres(pool.clone());
    let phone = "13900000001";
    cleanup(&pool, phone).await;

    // 10 分钟前上报 —— 早于下面的 deadline（用真实量级，不写「此刻」，
    // 否则与 deadline 同秒会踩边界）。
    let old = chrono::Utc::now() - Duration::from_secs(600);
    let old = old.format("%Y-%m-%d %H:%M:%S").to_string();
    let id = insert_terminal(&pool, phone, &old).await;

    let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let affected = gbserver::db::jt1078::mark_terminal_offline_if_stale(&db_pool, phone, &deadline)
        .await
        .expect("mark_terminal_offline_if_stale 必须在 postgres 上成功执行");

    assert_eq!(affected, 1, "过期的终端应当被置离线");
    assert_eq!(
        status_of(&pool, id).await,
        Some(false),
        "status 应当变成离线"
    );

    cleanup(&pool, phone).await;
}

/// 活跃的终端（心跳比 deadline 新）**不能**被置离线。
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn fresh_terminal_is_not_marked_offline_on_postgres() {
    let pool = test_pool().await;
    // `db::jt1078::*` 现在接受运行期多态的 `db::Pool`（原生 PgPool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::Postgres(pool.clone());
    let phone = "13900000002";
    cleanup(&pool, phone).await;

    // **先算 deadline，再插入** —— `update_time` 必须**不早于** deadline，
    // 才能表达「这期间终端还在活动」。
    //
    // 反过来写（先取 now 再算 deadline）会踩边界：两次取时钟之间若跨过一秒，
    // `update_time` 就比 deadline **早**一秒 → 被判成过期 → 用例变成 flaky。
    // 真实场景里「有活动」的心跳本来就发生在会话被摘掉**之后**。
    let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = insert_terminal(&pool, phone, &deadline).await;

    let affected = gbserver::db::jt1078::mark_terminal_offline_if_stale(&db_pool, phone, &deadline)
        .await
        .expect("mark_terminal_offline_if_stale 必须在 postgres 上成功执行");

    assert_eq!(affected, 0, "活跃的终端不应被置离线");
    assert_eq!(
        status_of(&pool, id).await,
        Some(true),
        "status 应当仍是在线"
    );

    cleanup(&pool, phone).await;
}

/// 同一 `phone_number` 有多行时：**全部**过期才置离线（第七轮修的「半离线」）。
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn duplicate_phone_rows_are_not_left_half_offline_on_postgres() {
    let pool = test_pool().await;
    // `db::jt1078::*` 现在接受运行期多态的 `db::Pool`（原生 PgPool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::Postgres(pool.clone());
    let phone = "13900000003";
    cleanup(&pool, phone).await;

    // 同上：deadline 先算；「旧行」比 deadline 早 10 分钟（真实量级），
    // 「新行」等于 deadline（= 这期间还有活动）。
    let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let old = (chrono::Utc::now() - Duration::from_secs(600))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let id_old = insert_terminal(&pool, phone, &old).await;
    let id_new = insert_terminal(&pool, phone, &deadline).await;

    let affected = gbserver::db::jt1078::mark_terminal_offline_if_stale(&db_pool, phone, &deadline)
        .await
        .expect("mark_terminal_offline_if_stale 必须在 postgres 上成功执行");

    assert_eq!(affected, 0, "还有一行活跃 → 整体不动");
    assert_eq!(status_of(&pool, id_old).await, Some(true));
    assert_eq!(status_of(&pool, id_new).await, Some(true));

    cleanup(&pool, phone).await;
}

/// **全部**过期时，按 id 精确把**每一行**都置离线（第七轮修的「半离线」的另一半）。
#[tokio::test]
#[ignore = "requires TEST_POSTGRES_URL and an isolated PostgreSQL database"]
async fn duplicate_phone_rows_all_stale_are_all_marked_offline_on_postgres() {
    let pool = test_pool().await;
    // `db::jt1078::*` 现在接受运行期多态的 `db::Pool`（原生 PgPool 仍用于直连 sqlx 查询）。
    let db_pool = gbserver::db::Pool::Postgres(pool.clone());
    let phone = "13900000004";
    cleanup(&pool, phone).await;

    let old = (chrono::Utc::now() - Duration::from_secs(600))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let id_a = insert_terminal(&pool, phone, &old).await;
    let id_b = insert_terminal(&pool, phone, &old).await;

    let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let affected = gbserver::db::jt1078::mark_terminal_offline_if_stale(&db_pool, phone, &deadline)
        .await
        .expect("mark_terminal_offline_if_stale 必须在 postgres 上成功执行");

    assert_eq!(affected, 2, "两行都过期 → 两行都置离线");
    assert_eq!(status_of(&pool, id_a).await, Some(false));
    assert_eq!(status_of(&pool, id_b).await, Some(false));

    cleanup(&pool, phone).await;
}
