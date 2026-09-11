//! 区域（行政区划）handler
//!
//! 2026-09-11：`region_one` / `region_page_list` / `region_sync` 此前是**假实现**：
//! 分别返回编造的 `name: "region-{id}"`、永远为空的列表、以及不做任何事的
//! 「区域同步 ok」。现全部改为真实查询 / 真实同步。
//!
//! 逻辑刻意抽成只依赖 `&db::Pool` 的函数，handler 仅做薄包装 ——
//! 这样可以用真实内存 SQLite 直接测试（无需构造整个 `AppState`）。

use std::collections::HashSet;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::db;
use crate::db::region::Region;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

/// 把 `Region` 行转成前端 `Region` 接口期望的字段。
fn region_json(r: &Region, parent_name: Option<String>) -> serde_json::Value {
    serde_json::json!({
        "id": r.id,
        "deviceId": r.device_id,
        "name": r.name,
        "parentId": r.parent_id,
        "parentName": parent_name,
        "createTime": r.create_time,
        "updateTime": r.update_time,
    })
}

/// 查询单个区域（含父区域名称）。
pub(crate) async fn load_region_detail(
    pool: &db::Pool,
    id: i32,
) -> sqlx::Result<Option<serde_json::Value>> {
    let Some(region) = db::region::get_by_id(pool, id).await? else {
        return Ok(None);
    };
    let parent_name = match region.parent_id {
        Some(pid) => db::region::get_by_id(pool, pid).await?.map(|p| p.name),
        None => None,
    };
    Ok(Some(region_json(&region, parent_name)))
}

/// 分页查询区域，返回 `(当页数据, 总数)`。
pub(crate) async fn load_region_page(
    pool: &db::Pool,
    page: u32,
    count: u32,
) -> sqlx::Result<(Vec<serde_json::Value>, i64)> {
    let page = page.max(1);
    let count = count.max(1);

    let all = db::region::list_all(pool).await?;
    let total = db::region::count_all(pool).await?;

    // 预取 id→name，避免逐行查父名称造成 N+1
    let name_by_id: std::collections::HashMap<i32, String> =
        all.iter().map(|r| (r.id, r.name.clone())).collect();

    let start = ((page - 1) as usize).saturating_mul(count as usize);
    let list: Vec<serde_json::Value> = all
        .iter()
        .skip(start)
        .take(count as usize)
        .map(|r| {
            let parent_name = r.parent_id.and_then(|pid| name_by_id.get(&pid).cloned());
            region_json(r, parent_name)
        })
        .collect();

    Ok((list, total))
}

/// 按通道的行政区划编码补齐区域表，返回本次**新建**的区域数量。
///
/// 语义与既有 `region_add_by_civil_code` 保持一致（区域以 civilCode 作为 `device_id`）。
/// **幂等**：已存在的区域不会被改写。
pub(crate) async fn sync_regions_from_civil_codes(pool: &db::Pool) -> sqlx::Result<u32> {
    let codes = db::common_channel::list_distinct_civil_codes(pool).await?;
    if codes.is_empty() {
        return Ok(0);
    }
    let existing: HashSet<String> = db::region::list_all(pool)
        .await?
        .into_iter()
        .map(|r| r.device_id)
        .collect();

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut created = 0u32;
    for code in codes {
        if existing.contains(&code) {
            continue;
        }
        let name = format!("区域 {}", code);
        db::region::add(pool, &code, &name, None, None, &now).await?;
        created += 1;
    }
    Ok(created)
}

/// GET /api/region/one?id=...
pub async fn region_one(
    State(state): State<AppState>,
    Query(q): Query<RegionOne>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let detail = load_region_detail(&state.pool, q.id as i32)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::Error404, "区域不存在"))?;
    Ok(Json(WVPResult::success(detail)))
}

/// GET /api/region/page/list?page=&count=
pub async fn region_page_list(
    State(state): State<AppState>,
    Query(q): Query<PageList>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15).max(1);
    let (list, total) = load_region_page(&state.pool, page, count).await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "list": list,
        "total": total,
        "page": page,
        "count": count,
    }))))
}

/// GET /api/region/sync
pub async fn region_sync(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let created = sync_regions_from_civil_codes(&state.pool).await?;
    Ok(Json(WVPResult::success(serde_json::json!({
        "count": created,
        "synced": created,
        "msg": format!("区域同步完成，新增 {} 个区域", created),
    }))))
}

#[derive(Deserialize)]
pub struct RegionOne {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct PageList {
    pub page: Option<u32>,
    pub count: Option<u32>,
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::test_support::sqlite_pool_with_schema;

    async fn seed_channel_with_civil_code(pool: &db::Pool, device_id: &str, gb_id: &str, civil: &str) {
        sqlx::query(
            "INSERT INTO gb_device_channel \
             (device_id, name, gb_device_id, civil_code, status, data_type, data_device_id, create_time, update_time) \
             VALUES (?, ?, ?, ?, 1, 0, 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')",
        )
        .bind(device_id)
        .bind(format!("ch-{}", gb_id))
        .bind(gb_id)
        .bind(civil)
        .execute(pool)
        .await
        .expect("insert channel");
    }

    #[tokio::test]
    async fn test_region_one_returns_real_row_not_fabricated_name() {
        let pool = sqlite_pool_with_schema().await;
        let now = "2026-01-01 00:00:00";
        let parent = db::region::add(&pool, "ROOT", "根区域", None, None, now)
            .await
            .unwrap();
        assert!(parent > 0);
        let parent_id: i32 = sqlx::query_scalar("SELECT id FROM gb_common_region WHERE device_id = 'ROOT'")
            .fetch_one(&pool)
            .await
            .unwrap();
        db::region::add(&pool, "340200", "南京市", Some(parent_id), Some("ROOT"), now)
            .await
            .unwrap();
        let child_id: i32 = sqlx::query_scalar("SELECT id FROM gb_common_region WHERE device_id = '340200'")
            .fetch_one(&pool)
            .await
            .unwrap();

        let detail = load_region_detail(&pool, child_id).await.unwrap().unwrap();
        // 关键：name 必须来自数据库，而不是 `region-{id}` 这类编造值
        assert_eq!(detail["name"], "南京市");
        assert_eq!(detail["deviceId"], "340200");
        assert_eq!(detail["parentName"], "根区域");
        assert_eq!(detail["parentId"], parent_id);
    }

    #[tokio::test]
    async fn test_region_one_missing_returns_none() {
        let pool = sqlite_pool_with_schema().await;
        assert!(load_region_detail(&pool, 12345).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_region_page_list_paginates_real_rows() {
        let pool = sqlite_pool_with_schema().await;
        let now = "2026-01-01 00:00:00";
        for i in 0..5 {
            db::region::add(&pool, &format!("code{}", i), &format!("区域{}", i), None, None, now)
                .await
                .unwrap();
        }

        let (page1, total) = load_region_page(&pool, 1, 2).await.unwrap();
        assert_eq!(total, 5, "总数应为真实行数");
        assert_eq!(page1.len(), 2);
        let (page3, _) = load_region_page(&pool, 3, 2).await.unwrap();
        assert_eq!(page3.len(), 1, "第 3 页只剩 1 条");
        let (page9, _) = load_region_page(&pool, 9, 2).await.unwrap();
        assert!(page9.is_empty(), "越界页应为空");
    }

    #[tokio::test]
    async fn test_region_sync_creates_regions_from_channel_civil_codes() {
        let pool = sqlite_pool_with_schema().await;
        seed_channel_with_civil_code(&pool, "dev1", "ch1", "340200").await;
        seed_channel_with_civil_code(&pool, "dev1", "ch2", "340200").await; // 重复 civil
        seed_channel_with_civil_code(&pool, "dev1", "ch3", "340100").await;
        // 空 civil_code 不应产生区域
        seed_channel_with_civil_code(&pool, "dev1", "ch4", "").await;

        let created = sync_regions_from_civil_codes(&pool).await.unwrap();
        assert_eq!(created, 2, "两个不同 civil_code 应各建一个区域");

        let rows = db::region::list_all(&pool).await.unwrap();
        let mut codes: Vec<String> = rows.iter().map(|r| r.device_id.clone()).collect();
        codes.sort();
        assert_eq!(codes, vec!["340100".to_string(), "340200".to_string()]);
    }

    #[tokio::test]
    async fn test_region_sync_is_idempotent() {
        let pool = sqlite_pool_with_schema().await;
        seed_channel_with_civil_code(&pool, "dev1", "ch1", "340200").await;

        assert_eq!(sync_regions_from_civil_codes(&pool).await.unwrap(), 1);
        // 第二次不应重复创建（区域以 device_id 唯一）
        assert_eq!(sync_regions_from_civil_codes(&pool).await.unwrap(), 0);
        assert_eq!(db::region::count_all(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_region_sync_without_channels_creates_nothing() {
        let pool = sqlite_pool_with_schema().await;
        assert_eq!(sync_regions_from_civil_codes(&pool).await.unwrap(), 0);
        assert_eq!(db::region::count_all(&pool).await.unwrap(), 0);
    }
}
