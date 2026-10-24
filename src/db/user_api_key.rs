//! 用户 API Key 表 gb_user_api_key

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::Pool;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UserApiKey {
    pub id: i32,
    pub user_id: Option<i64>,
    pub app: Option<String>,
    pub api_key: Option<String>,
    pub expired_at: Option<i64>,
    pub remark: Option<String>,
    pub enable: Option<bool>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserApiKeyAdd {
    #[serde(alias = "userId")]
    pub user_id: Option<i64>,
    pub app: Option<String>,
    #[serde(alias = "expiresAt")]
    #[serde(alias = "expiredAt")]
    pub expired_at: Option<i64>,
    pub enable: Option<bool>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserApiKeyRemark {
    pub id: Option<i64>,
    pub remark: Option<String>,
}

/// 分页列出 API Key，**最新的排在最前**（`id` 倒序）。
///
/// 排序方向是产品契约，不是实现细节：这一页是"凭证台账"，运维刚建完
/// Key 就要确认它在那儿、把它停用或删掉。升序（`ORDER BY id`）会把新建的
/// Key 顶到**最后一页** —— 库里超过一页（前端默认 20 条）时，管理员建完
/// Key 回到列表页根本看不到自己刚建的那条，只能翻页去找。
///
/// 仓库里其它列表（设备、审计日志、系统日志、JT1078 终端）也都是"新的在前"，
/// 这里跟它们保持一致。
pub async fn list_paged(pool: &Pool, page: u32, count: u32) -> sqlx::Result<Vec<UserApiKey>> {
    let offset = crate::pagination::offset(page, count);
    let sql = crate::db::dialect::current_sql("SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM gb_user_api_key ORDER BY id DESC LIMIT ? OFFSET ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, UserApiKey>(&sql)
            .bind(count as i64)
            .bind(offset)
            .fetch_all(p)
    });
}

pub async fn count_all(pool: &Pool) -> sqlx::Result<i64> {
    let sql = crate::db::dialect::current_sql("SELECT COUNT(*) FROM gb_user_api_key").into_owned();

    crate::dispatch!(pool, |p| {
        sqlx::query_scalar::<_, i64>(&sql).fetch_one(p)
    })
}

pub async fn get_by_id(pool: &Pool, id: i32) -> sqlx::Result<Option<UserApiKey>> {
    let sql = crate::db::dialect::current_sql("SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM gb_user_api_key WHERE id = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, UserApiKey>(&sql)
            .bind(id)
            .fetch_optional(p)
    });
}

pub async fn add(
    pool: &Pool,
    user_id: i64,
    app: &str,
    api_key: &str,
    expired_at: Option<i64>,
    enable: bool,
    remark: Option<&str>,
    now: &str,
) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql("INSERT INTO gb_user_api_key (user_id, app, api_key, expired_at, remark, enable, create_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?)").into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(user_id)
            .bind(app)
            .bind(api_key)
            .bind(expired_at)
            .bind(remark)
            .bind(enable)
            .bind(now)
            .bind(now)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn update_remark(pool: &Pool, id: i64, remark: &str, now: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_user_api_key SET remark = ?, update_time = ? WHERE id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(remark).bind(now).bind(id).execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn set_enable(pool: &Pool, id: i32, enable: bool, now: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_user_api_key SET enable = ?, update_time = ? WHERE id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql).bind(enable).bind(now).bind(id).execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn reset_api_key(pool: &Pool, id: i32, new_key: &str, now: &str) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "UPDATE gb_user_api_key SET api_key = ?, update_time = ? WHERE id = ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| {
        sqlx::query(&sql)
            .bind(new_key)
            .bind(now)
            .bind(id)
            .execute(p)
    })?;
    Ok(r.rows_affected())
}

pub async fn delete_by_id(pool: &Pool, id: i32) -> sqlx::Result<u64> {
    let sql =
        crate::db::dialect::current_sql("DELETE FROM gb_user_api_key WHERE id = ?").into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(id).execute(p) })?;
    Ok(r.rows_affected())
}

/// 根据 API Key 查询（用于认证）
pub async fn get_by_api_key(pool: &Pool, api_key: &str) -> sqlx::Result<Option<UserApiKey>> {
    let sql = crate::db::dialect::current_sql("SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM gb_user_api_key WHERE api_key = ?").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, UserApiKey>(&sql)
            .bind(api_key)
            .fetch_optional(p)
    });
}

/// 根据用户ID查询 API Key 列表（**最新的排在最前**，与 `list_paged` 同口径）。
pub async fn list_by_user_id(pool: &Pool, user_id: i64) -> sqlx::Result<Vec<UserApiKey>> {
    let sql = crate::db::dialect::current_sql("SELECT id, user_id, app, api_key, expired_at, remark, enable, create_time, update_time FROM gb_user_api_key WHERE user_id = ? ORDER BY id DESC").into_owned();
    return crate::dispatch!(pool, |p| {
        sqlx::query_as::<_, UserApiKey>(&sql)
            .bind(user_id)
            .fetch_all(p)
    });
}

/// 删除过期的 API Key
pub async fn delete_expired_keys(pool: &Pool, now_ts: i64) -> sqlx::Result<u64> {
    let sql = crate::db::dialect::current_sql(
        "DELETE FROM gb_user_api_key WHERE expired_at IS NOT NULL AND expired_at < ?",
    )
    .into_owned();
    let r = crate::dispatch_exec!(pool, |p| { sqlx::query(&sql).bind(now_ts).execute(p) })?;
    Ok(r.rows_affected())
}

#[cfg(test)]
mod expiry_tests {
    use super::*;

    /// C4：过期记录必须能被清掉，未过期与"永不过期"的必须保留。
    #[tokio::test]
    async fn delete_expired_keys_removes_only_expired_rows() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        let now = 1_000_000i64;
        let now_str = "2026-01-01 00:00:00";

        // 已过期
        add(
            &pool,
            1,
            "expired-app",
            "key-expired",
            Some(now - 60),
            true,
            None,
            now_str,
        )
        .await
        .unwrap();
        // 未过期
        add(
            &pool,
            1,
            "live-app",
            "key-live",
            Some(now + 3600),
            true,
            None,
            now_str,
        )
        .await
        .unwrap();
        // 永不过期（expired_at = NULL）
        add(
            &pool,
            1,
            "forever-app",
            "key-forever",
            None,
            true,
            None,
            now_str,
        )
        .await
        .unwrap();

        let removed = delete_expired_keys(&pool, now).await.unwrap();
        assert_eq!(removed, 1, "只应删掉 1 条过期记录");

        let remaining = list_paged(&pool, 1, 100).await.unwrap();
        let keys: Vec<String> = remaining.iter().filter_map(|k| k.api_key.clone()).collect();
        assert!(keys.contains(&"key-live".to_string()), "{keys:?}");
        assert!(keys.contains(&"key-forever".to_string()), "{keys:?}");
        assert!(!keys.contains(&"key-expired".to_string()), "{keys:?}");
    }

    /// 没有过期记录时不应报错，删除数应为 0。
    #[tokio::test]
    async fn delete_expired_keys_is_a_noop_without_expired_rows() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        let now = 1_000_000i64;
        add(
            &pool,
            1,
            "live-app",
            "key-live",
            Some(now + 3600),
            true,
            None,
            "2026-01-01 00:00:00",
        )
        .await
        .unwrap();
        assert_eq!(delete_expired_keys(&pool, now).await.unwrap(), 0);
        assert_eq!(count_all(&pool).await.unwrap(), 1);
    }
}

#[cfg(test)]
mod ordering_tests {
    use super::*;

    const NOW: &str = "2026-01-01 00:00:00";

    /// 铺 `n` 条历史数据（都属于 user 1）。
    async fn seed(pool: &Pool, n: usize) {
        for i in 0..n {
            add(
                pool,
                1,
                &format!("app-{i:02}"),
                &format!("key-{i:02}"),
                None,
                true,
                None,
                NOW,
            )
            .await
            .unwrap();
        }
    }

    /// 回归：列表必须**最新的在最前**（`id` 倒序）。
    ///
    /// 这不是审美偏好，而是产品契约：这一页是"凭证台账"，管理员建完 Key 回到
    /// 列表页就要看见它、马上停用或删掉。升序（`ORDER BY id`）会把新建的 Key
    /// 顶到**最后一页** —— 库里超过一页（前端默认 20 条）时，页面上根本看不到
    /// 自己刚建的那条，只能靠翻页去找。
    ///
    /// 变异验证：把 `list_paged` 的 `ORDER BY id DESC` 改回 `ORDER BY id`，
    /// 本用例立刻失败（`just-created` 跑到了第 26 条、即第二页）。
    #[tokio::test]
    async fn list_paged_returns_newest_first() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed(&pool, 25).await;
        add(
            &pool,
            1,
            "just-created",
            "key-just-created",
            None,
            true,
            None,
            NOW,
        )
        .await
        .unwrap();

        let page1 = list_paged(&pool, 1, 20).await.unwrap();
        assert_eq!(page1.len(), 20, "第一页应当有 20 条");
        assert_eq!(
            page1[0].app.as_deref(),
            Some("just-created"),
            "刚创建的 Key 必须落在第一页第一行，而不是被顶到最后一页"
        );
        let ids: Vec<i32> = page1.iter().map(|k| k.id).collect();
        let mut desc = ids.clone();
        desc.sort_by(|a, b| b.cmp(a));
        assert_eq!(ids, desc, "整页必须按 id 倒序（新的在前），实际 {ids:?}");
    }

    /// 回归：倒序 + LIMIT/OFFSET 翻页必须**不重不漏**。
    #[tokio::test]
    async fn list_paged_pages_do_not_skip_or_duplicate() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed(&pool, 25).await;

        let first_page: Vec<i32> = list_paged(&pool, 1, 10)
            .await
            .unwrap()
            .iter()
            .map(|k| k.id)
            .collect();
        let second_page: Vec<i32> = list_paged(&pool, 2, 10)
            .await
            .unwrap()
            .iter()
            .map(|k| k.id)
            .collect();

        assert_eq!(first_page.len(), 10);
        assert_eq!(second_page.len(), 10);
        let mut all: Vec<i32> = first_page
            .iter()
            .chain(second_page.iter())
            .copied()
            .collect();
        let total = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), total, "两页之间不能有重复行");
        assert!(
            *second_page.iter().max().unwrap() < *first_page.iter().min().unwrap(),
            "第二页必须整体排在第一页之后：{second_page:?} vs {first_page:?}"
        );
    }

    /// 回归：`list_by_user_id` 与 `list_paged` 同口径（新的在前）。
    #[tokio::test]
    async fn list_by_user_id_is_newest_first() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed(&pool, 2).await;
        add(&pool, 2, "other-user", "key-other", None, true, None, NOW)
            .await
            .unwrap();
        add(
            &pool,
            1,
            "my-newest",
            "key-my-newest",
            None,
            true,
            None,
            NOW,
        )
        .await
        .unwrap();

        let rows = list_by_user_id(&pool, 1).await.unwrap();
        assert_eq!(rows.len(), 3, "只应返回 user 1 的 3 条");
        assert_eq!(rows[0].app.as_deref(), Some("my-newest"));
        let ids: Vec<i32> = rows.iter().map(|k| k.id).collect();
        let mut desc = ids.clone();
        desc.sort_by(|a, b| b.cmp(a));
        assert_eq!(ids, desc, "必须按 id 倒序，实际 {ids:?}");
    }

    /// 回归：超大 `page` 不能让列表接口 panic。
    ///
    /// 旧写法 `(page.saturating_sub(1)) * count` 在 `u32` 上溢出：
    /// `4294967294 * 100` 超出 `u32` 上限 —— debug 构建直接 panic（handler
    /// 链路上没有 `CatchPanic` 层，panic 会打断整条 HTTP 连接），release 构建
    /// 则回绕成一个随意的页码。
    #[tokio::test]
    async fn list_paged_survives_huge_page_number() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed(&pool, 3).await;
        let rows = list_paged(&pool, u32::MAX, 100).await.unwrap();
        assert!(
            rows.is_empty(),
            "超出末页就该是空列表，实际 {} 条",
            rows.len()
        );
    }
}
