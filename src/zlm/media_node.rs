//! MediaNode 健康检查（Phase 4.4）
//!
//! 负责：扫描所有 ZLM 媒体节点，把超过 `DEFAULT_KEEPALIVE_TIMEOUT_SECS` 秒
//! 没有 keepalive 的节点自动切为 offline。
//!
//! 设计要点：
//! - `MediaNode` trait 是一个只读视图接口（id + last_keepalive），用于让
//!   `is_online` 这种纯函数可以脱离数据库单元测试。
//! - `run_health_check_once` 是核心函数，被 `health_check_loop` 周期调用，
//!   也可以被其它逻辑（例如集成测试）直接调用。
//! - `health_check_loop` 用 `tokio::time::interval` 跑，间隔 10s，错误仅 warn
//!   不 panic，确保一次失败不会让后台任务消失。

use std::time::Duration;

use chrono::{DateTime, Utc};

/// Keepalive 超时阈值：默认 30 秒
pub const DEFAULT_KEEPALIVE_TIMEOUT_SECS: i64 = 30;

/// 后台 loop 间隔：10 秒
pub const HEALTH_CHECK_INTERVAL_SECS: u64 = 10;

/// keepalive_grace_count：连续 N 次健康检查都判定超时才真正切 offline。
/// 默认 3 次，对应 30s 超时 × 3 = ~90s 真实离线判定窗口。
/// R2 缓解：避免弱网下瞬时丢包就被切 offline。
pub const DEFAULT_KEEPALIVE_GRACE_COUNT: i32 = 3;

/// 媒体节点只读视图（让 `is_online` 可以在 trait 内纯函数实现）
pub trait MediaNode: Send + Sync {
    fn id(&self) -> &str;
    fn last_keepalive(&self) -> Option<DateTime<Utc>>;

    /// 当前是否在线：last_keepalive 在阈值内视为在线；无 keepalive 视为离线
    fn is_online(&self) -> bool {
        match self.last_keepalive() {
            Some(t) => (Utc::now() - t).num_seconds() < DEFAULT_KEEPALIVE_TIMEOUT_SECS,
            None => false,
        }
    }
}

/// 一次性扫描并把超时节点标 offline。
///
/// 返回被标记为 offline 的节点数。
pub async fn run_health_check_once(
    pool: &crate::db::Pool,
) -> anyhow::Result<usize> {
    run_health_check_once_with_grace(pool, DEFAULT_KEEPALIVE_GRACE_COUNT).await
}

/// 与 `run_health_check_once` 相同，但显式指定 grace count（测试用）。
///
/// 行为（两步原子 SQL）：
/// 1. 递增过期的在线节点的 `consecutive_misses`
/// 2. 切那些 `consecutive_misses >= grace_count` 的节点为 offline
/// 3. 重置健康节点的 `consecutive_misses = 0`
///
/// 返回**新切为 offline**的节点数（不是被递增的）。
pub async fn run_health_check_once_with_grace(
    pool: &crate::db::Pool,
    grace_count: i32,
) -> anyhow::Result<usize> {
    use crate::db::media_server;
    let offline_threshold = Utc::now() - chrono::Duration::seconds(DEFAULT_KEEPALIVE_TIMEOUT_SECS);
    let threshold_str = offline_threshold.to_rfc3339();

    // Step 1: 递增过期节点的连续丢失计数
    let _ = media_server::increment_miss_count_if_expired(
        pool, &threshold_str,
    ).await?;

    // Step 2: 切连续丢失达到阈值的节点为 offline
    let affected = media_server::mark_offline_if_miss_count_exceeded(
        pool, grace_count,
    ).await?;

    // Step 3: 重置健康节点的 miss 计数（keepalive 恢复）
    let reset_count = media_server::reset_miss_count_for_fresh_nodes(
        pool, &threshold_str,
    ).await?;

    if affected > 0 {
        tracing::info!(
            "Marked {} media nodes offline (keepalive timeout × {} grace)",
            affected, grace_count
        );
    }
    if reset_count > 0 {
        tracing::debug!(
            "Reset consecutive_misses for {} media nodes (keepalive recovered)",
            reset_count
        );
    }
    Ok(affected as usize)
}

/// 后台 loop：每 10s 跑一次 `run_health_check_once`
///
/// 错误只 warn，不退出 — 因为 keepalive 超时检测是辅助功能，不能因为单次
/// 数据库错误就让整个后台任务消失。
pub async fn health_check_loop(pool: crate::db::Pool) {
    let mut interval = tokio::time::interval(Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS));
    loop {
        interval.tick().await;
        if let Err(e) = run_health_check_once(&pool).await {
            tracing::warn!("MediaNode health check failed: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    /// 用于 trait 单元测试的简单 fake node
    struct FakeNode {
        id: String,
        last_keepalive: Option<DateTime<Utc>>,
    }
    impl MediaNode for FakeNode {
        fn id(&self) -> &str {
            &self.id
        }
        fn last_keepalive(&self) -> Option<DateTime<Utc>> {
            self.last_keepalive
        }
    }

    #[test]
    fn test_is_online_with_recent_keepalive() {
        let node = FakeNode {
            id: "zlm-1".to_string(),
            last_keepalive: Some(Utc::now() - ChronoDuration::seconds(5)),
        };
        assert!(
            node.is_online(),
            "keepalive 5s ago should be online (threshold={})",
            DEFAULT_KEEPALIVE_TIMEOUT_SECS
        );
    }

    #[test]
    fn test_is_online_with_old_keepalive() {
        let node = FakeNode {
            id: "zlm-2".to_string(),
            last_keepalive: Some(Utc::now() - ChronoDuration::seconds(120)),
        };
        assert!(
            !node.is_online(),
            "keepalive 120s ago should be offline (threshold={})",
            DEFAULT_KEEPALIVE_TIMEOUT_SECS
        );
    }

    #[test]
    fn test_is_online_no_keepalive() {
        let node = FakeNode {
            id: "zlm-3".to_string(),
            last_keepalive: None,
        };
        assert!(!node.is_online(), "no keepalive should be offline");
    }

    /// 集成测试：在内存 SQLite 上建表，插一行过期的在线节点，
    /// 跑 `run_health_check_once`，断言该行被切到 status=0。
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_run_health_check_once_marks_expired() {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")
            .expect("lazy pool");

        // 建最小 gb_media_server 表（包含 status + last_keepalive_time + consecutive_misses）
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_media_server (
                id VARCHAR(255) PRIMARY KEY,
                ip VARCHAR(50),
                http_port INTEGER,
                status INTEGER NOT NULL DEFAULT 0,
                last_keepalive_time VARCHAR(50),
                consecutive_misses INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .execute(&pool)
        .await
        .expect("create table");

        // 插两行：
        //  - expired: status=1, keepalive 是 2 分钟前 → 应该被切 offline
        //  - fresh:   status=1, keepalive 是 5 秒前 → 应该保持 online
        let expired_ts =
            (Utc::now() - ChronoDuration::seconds(120)).to_rfc3339();
        let fresh_ts = (Utc::now() - ChronoDuration::seconds(5)).to_rfc3339();

        sqlx::query(
            "INSERT INTO gb_media_server (id, ip, http_port, status, last_keepalive_time, consecutive_misses) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("zlm-expired")
        .bind("10.0.0.1")
        .bind(8080_i32)
        .bind(1_i32)
        .bind(&expired_ts)
        .bind(0_i32)
        .execute(&pool)
        .await
        .expect("insert expired");

        sqlx::query(
            "INSERT INTO gb_media_server (id, ip, http_port, status, last_keepalive_time, consecutive_misses) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("zlm-fresh")
        .bind("10.0.0.2")
        .bind(8080_i32)
        .bind(1_i32)
        .bind(&fresh_ts)
        .bind(0_i32)
        .execute(&pool)
        .await
        .expect("insert fresh");

        // 跑一次 health check（用 grace_count=1 保持原始语义：单次超时立即切 offline）
        let affected = run_health_check_once_with_grace(&pool, 1)
            .await
            .expect("health check");
        assert_eq!(affected, 1, "exactly one row should be marked offline");

        // 验证 expired 已被切到 status=0
        let expired_status: i32 =
            sqlx::query_scalar("SELECT status FROM gb_media_server WHERE id = ?")
                .bind("zlm-expired")
                .fetch_one(&pool)
                .await
                .expect("select expired");
        assert_eq!(expired_status, 0, "expired node should be offline");

        // 验证 fresh 仍为 status=1
        let fresh_status: i32 =
            sqlx::query_scalar("SELECT status FROM gb_media_server WHERE id = ?")
                .bind("zlm-fresh")
                .fetch_one(&pool)
                .await
                .expect("select fresh");
        assert_eq!(fresh_status, 1, "fresh node should stay online");
    }

    /// Phase 4 follow-up: keepalive_grace_count 容错
    /// 第一次健康检查发现 expired → consecutive_misses=1，但 status 仍为 1
    /// （因为 grace_count=3 还不到）
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_grace_count_does_not_mark_offline_on_first_miss() {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")
            .expect("lazy pool");

        // 包含 consecutive_misses 列
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_media_server (
                id VARCHAR(255) PRIMARY KEY,
                ip VARCHAR(50),
                http_port INTEGER,
                status INTEGER NOT NULL DEFAULT 0,
                last_keepalive_time VARCHAR(50),
                consecutive_misses INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .execute(&pool)
        .await
        .expect("create table");

        // 插一行已过期的在线节点
        let expired_ts = (Utc::now() - ChronoDuration::seconds(120)).to_rfc3339();
        sqlx::query(
            "INSERT INTO gb_media_server (id, ip, http_port, status, last_keepalive_time, consecutive_misses) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("zlm-gc1")
        .bind("10.0.0.1")
        .bind(8080_i32)
        .bind(1_i32)
        .bind(&expired_ts)
        .bind(0_i32)
        .execute(&pool)
        .await
        .expect("insert");

        // 跑 health check with grace_count=3
        let affected = run_health_check_once_with_grace(&pool, 3)
            .await
            .expect("health check");
        assert_eq!(affected, 0, "第一次未达 grace_count 不应切 offline");

        // consecutive_misses 应该 +1 到 1，但 status 仍 1
        let status: i32 = sqlx::query_scalar("SELECT status FROM gb_media_server WHERE id = ?")
            .bind("zlm-gc1")
            .fetch_one(&pool)
            .await
            .expect("select status");
        assert_eq!(status, 1, "第一次未达 grace_count 仍应 online");

        let misses: i32 = sqlx::query_scalar("SELECT consecutive_misses FROM gb_media_server WHERE id = ?")
            .bind("zlm-gc1")
            .fetch_one(&pool)
            .await
            .expect("select misses");
        assert_eq!(misses, 1, "consecutive_misses 应该 +1");
    }

    /// Phase 4 follow-up: 连续 grace_count 次都判定超时才真正切 offline
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_grace_count_marks_offline_after_threshold() {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")
            .expect("lazy pool");

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_media_server (
                id VARCHAR(255) PRIMARY KEY,
                ip VARCHAR(50),
                http_port INTEGER,
                status INTEGER NOT NULL DEFAULT 0,
                last_keepalive_time VARCHAR(50),
                consecutive_misses INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .execute(&pool)
        .await
        .expect("create table");

        // 插一行已过期的在线节点，consecutive_misses=2（差一次就达 3）
        let expired_ts = (Utc::now() - ChronoDuration::seconds(120)).to_rfc3339();
        sqlx::query(
            "INSERT INTO gb_media_server (id, ip, http_port, status, last_keepalive_time, consecutive_misses) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("zlm-gc2")
        .bind("10.0.0.2")
        .bind(8080_i32)
        .bind(1_i32)
        .bind(&expired_ts)
        .bind(2_i32)
        .execute(&pool)
        .await
        .expect("insert");

        // 跑 health check with grace_count=3
        let affected = run_health_check_once_with_grace(&pool, 3)
            .await
            .expect("health check");
        assert_eq!(affected, 1, "达到 grace_count 应该切 offline");

        let status: i32 = sqlx::query_scalar("SELECT status FROM gb_media_server WHERE id = ?")
            .bind("zlm-gc2")
            .fetch_one(&pool)
            .await
            .expect("select status");
        assert_eq!(status, 0, "达到 grace_count 后应 offline");
    }

    /// Phase 4 follow-up: keepalive 到达时重置 consecutive_misses
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_update_last_keepalive_resets_miss_count() {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy("sqlite::memory:")
            .expect("lazy pool");

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS gb_media_server (
                id VARCHAR(255) PRIMARY KEY,
                ip VARCHAR(50),
                http_port INTEGER,
                status INTEGER NOT NULL DEFAULT 0,
                last_keepalive_time VARCHAR(50),
                consecutive_misses INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .execute(&pool)
        .await
        .expect("create table");

        let now_ts = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO gb_media_server (id, ip, http_port, status, last_keepalive_time, consecutive_misses) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("zlm-gc3")
        .bind("10.0.0.3")
        .bind(8080_i32)
        .bind(1_i32)
        .bind("2020-01-01T00:00:00+00:00") // 过期
        .bind(2_i32)
        .execute(&pool)
        .await
        .expect("insert");

        // 模拟 keepalive 到达
        let updated = crate::db::media_server::update_last_keepalive(
            &pool, "zlm-gc3", &now_ts,
        )
        .await
        .expect("update");
        assert_eq!(updated, 1);

        // consecutive_misses 应被重置为 0
        let misses: i32 = sqlx::query_scalar("SELECT consecutive_misses FROM gb_media_server WHERE id = ?")
            .bind("zlm-gc3")
            .fetch_one(&pool)
            .await
            .expect("select misses");
        assert_eq!(misses, 0, "keepalive 到达应重置 consecutive_misses");
    }
}
