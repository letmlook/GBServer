use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::error::{AppError, ErrorCode};
use crate::dyn_where::{BindValue, DynWhere};
use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AlarmQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    #[serde(alias = "alarmMethod")]
    pub alarm_method: Option<String>,
    #[serde(alias = "alarmType")]
    pub alarm_type: Option<String>,
    #[serde(alias = "startTime", alias = "beginTime")]
    pub begin_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
    pub handled: Option<bool>,
    /// 关键字：设备号/通道号/描述（页面的搜索框）
    pub query: Option<String>,
}

/// 告警行（FromRow 是方言无关的，避免为三种数据库各写一份 `Row::get`）。
#[derive(Debug, sqlx::FromRow)]
struct AlarmRow {
    id: i64,
    device_id: Option<String>,
    channel_id: Option<String>,
    alarm_priority: Option<String>,
    alarm_method: Option<String>,
    alarm_type: Option<String>,
    alarm_time: Option<String>,
    alarm_description: Option<String>,
    longitude: Option<f64>,
    latitude: Option<f64>,
    create_time: Option<String>,
    handled: Option<i64>,
    handle_user: Option<String>,
    handle_time: Option<String>,
    handle_result: Option<String>,
}

impl AlarmRow {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "deviceId": self.device_id,
            "channelId": self.channel_id,
            // 前端读的是 alarmPriority（WVP `Alarm` 同名字段）；
            // 早期前端读 `alarmLevel`，后端从不返回该键 → 级别列恒为空。
            "alarmPriority": self.alarm_priority,
            "alarmMethod": self.alarm_method,
            "alarmType": self.alarm_type,
            "alarmTime": self.alarm_time,
            "alarmDescription": self.alarm_description,
            "longitude": self.longitude,
            "latitude": self.latitude,
            "createTime": self.create_time,
            "handled": self.handled.unwrap_or(0) != 0,
            "handleUser": self.handle_user,
            "handleTime": self.handle_time,
            "handleResult": self.handle_result,
        })
    }
}

/// GET /api/alarm/list - 查询告警列表
///
/// 筛选（WVP `AlarmController.list` 是 alarmType/beginTime/endTime，
/// 这里额外支持 deviceId/channelId/alarmMethod/handled/query）：
///
/// * `beginTime`/`endTime`（也接受 `startTime`）—— **此前这两个参数被 DTO
///   收下，却在三个方言的 SQL 里从未使用**：选了时间范围结果完全不变；
/// * `query` —— 关键字，匹配设备号/通道号/描述（页面上的「关键字」输入框）。
pub async fn alarm_list(
    State(state): State<AppState>,
    Query(q): Query<AlarmQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(10).clamp(1, 500);
    let offset = ((page - 1) * count) as i64;

    let w = alarm_filter(&q);

    const COLS: &str = "SELECT id, device_id, channel_id, alarm_priority, alarm_method, alarm_type, \
         alarm_time, alarm_description, longitude, latitude, create_time, \
         handled, handle_user, handle_time, handle_result \
         FROM gb_device_alarm";
    const COUNT_BASE: &str = "SELECT COUNT(*) FROM gb_device_alarm";

    let limit_ph = if cfg!(feature = "postgres") {
        format!(" LIMIT ${} OFFSET ${}", w.binds.len() + 1, w.binds.len() + 2)
    } else {
        " LIMIT ? OFFSET ?".to_string()
    };
    let sql_rows = format!("{}{} ORDER BY create_time DESC{limit_ph}", w.sql(COLS), "");
    let mut query = sqlx::query_as::<_, AlarmRow>(&sql_rows);
    for b in &w.binds {
        query = match b {
            BindValue::Text(v) => query.bind(v.as_str()),
            BindValue::Int(v) => query.bind(*v),
            BindValue::Big(v) => query.bind(*v),
            BindValue::Bool(v) => query.bind(*v),
        };
    }
    let rows: Vec<AlarmRow> = query
        .bind(count as i64)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {e}")))?;

    let count_sql = w.sql(COUNT_BASE);
    let mut query = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &w.binds {
        query = match b {
            BindValue::Text(v) => query.bind(v.as_str()),
            BindValue::Int(v) => query.bind(*v),
            BindValue::Big(v) => query.bind(*v),
            BindValue::Bool(v) => query.bind(*v),
        };
    }
    let total: i64 = query
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {e}")))?;

    let list: Vec<serde_json::Value> = rows.iter().map(|r| r.to_json()).collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

/// 把查询参数拼成 WHERE（list 与 clear 共用，保证"看到什么就清什么"）。
fn alarm_filter(q: &AlarmQuery) -> DynWhere {
    let mut w = DynWhere::new();
    if let Some(v) = q.device_id.as_deref().filter(|s| !s.is_empty()) {
        w.add("device_id = ?", vec![BindValue::Text(v.to_string())]);
    }
    if let Some(v) = q.channel_id.as_deref().filter(|s| !s.is_empty()) {
        w.add("channel_id = ?", vec![BindValue::Text(v.to_string())]);
    }
    if let Some(v) = q.alarm_type.as_deref().filter(|s| !s.is_empty()) {
        // 兼容逗号分隔的多个类型（WVP 的 `List<AlarmType>` 会重复同名参数）
        let types: Vec<&str> = v.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        match types.len() {
            0 => {}
            1 => w.add("alarm_type = ?", vec![BindValue::Text(types[0].to_string())]),
            _ => {
                let ph = vec!["?"; types.len()].join(",");
                w.add(
                    format!("alarm_type IN ({ph})"),
                    types.iter().map(|t| BindValue::Text(t.to_string())).collect(),
                );
            }
        }
    }
    if let Some(v) = q.alarm_method.as_deref().filter(|s| !s.is_empty()) {
        w.add("alarm_method = ?", vec![BindValue::Text(v.to_string())]);
    }
    if let Some(v) = q.begin_time.as_deref().filter(|s| !s.is_empty()) {
        // 国标上报时间是 alarm_time 字符串；没有它的旧数据退化成入库时间，
        // 否则这些行会被时间筛选静默丢掉。
        w.add(
            "COALESCE(alarm_time, create_time) >= ?",
            vec![BindValue::Text(v.to_string())],
        );
    }
    if let Some(v) = q.end_time.as_deref().filter(|s| !s.is_empty()) {
        w.add(
            "COALESCE(alarm_time, create_time) <= ?",
            vec![BindValue::Text(v.to_string())],
        );
    }
    if let Some(v) = q.query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{v}%");
        w.add(
            "(device_id LIKE ? OR channel_id LIKE ? OR alarm_description LIKE ?)",
            vec![
                BindValue::Text(like.clone()),
                BindValue::Text(like.clone()),
                BindValue::Text(like),
            ],
        );
    }
    if let Some(handled) = q.handled {
        w.add("handled = ?", vec![BindValue::Int(if handled { 1 } else { 0 })]);
    }
    w
}

/// GET /api/alarm/detail/:id - 查询告警详情
///
/// 用 `FromRow` 一次性取全列（此前手写 `SELECT *` + 15 个 `r.get(...)`，
/// 三种方言各抄一遍，`handle_result` 这类新列很容易漏掉一个分支）。
pub async fn alarm_detail(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    let row: Option<AlarmRow> = sqlx::query_as("SELECT id, device_id, channel_id, alarm_priority, \
        alarm_method, alarm_type, alarm_time, alarm_description, longitude, latitude, \
        create_time, handled, handle_user, handle_time, handle_result \
        FROM gb_device_alarm WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {e}")))?;

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let row: Option<AlarmRow> = sqlx::query_as("SELECT id, device_id, channel_id, alarm_priority, \
        alarm_method, alarm_type, alarm_time, alarm_description, longitude, latitude, \
        create_time, handled, handle_user, handle_time, handle_result \
        FROM gb_device_alarm WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库查询失败: {e}")))?;

    match row {
        Some(r) => Ok(Json(WVPResult::success(r.to_json()))),
        None => Ok(Json(WVPResult::error("告警不存在"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct AlarmHandleBody {
    pub id: Option<i64>,
    #[serde(alias = "handleUser")]
    pub handle_user: Option<String>,
    pub handled: Option<bool>,
    /// 处理结论（前端「处理结果」输入框）。**此前该字段后端 DTO 里不存在**，
    /// 用户填写的结论被 serde 静默丢弃，任何接口也读不回来。
    #[serde(alias = "handleResult", alias = "result")]
    pub handle_result: Option<String>,
}

/// POST /api/alarm/handle - 处理告警
pub async fn alarm_handle(
    State(state): State<AppState>,
    Json(body): Json<AlarmHandleBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::error("缺少告警ID".to_string())));
    }

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let handle_user = body.handle_user.as_deref();

    // Persist the handle state. The migration in db::alarm::ensure_columns
    // adds the handled/handle_user/handle_time columns on first run.
    let rows = crate::db::alarm::set_handled(
        &state.pool,
        id,
        handle_user,
        &now,
        body.handle_result.as_deref(),
    )
    .await?;

    if rows == 0 {
        return Ok(Json(WVPResult::error("告警不存在".to_string())));
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "handled": true,
        "handleUser": handle_user,
        "handleTime": now,
        "handleResult": body.handle_result,
        "message": "告警已处理"
    }))))
}

/// DELETE /api/alarm/delete/:id - 删除告警
pub async fn alarm_delete(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_device_alarm WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("DELETE FROM gb_device_alarm WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    // 返回被删除的条数：前端/自动化可以据此确认"只删了这一条"，
    // 而不是靠刷新后的行数（列表可能同时在增长）。
    let deleted = r.rows_affected();
    if deleted == 0 {
        return Ok(Json(WVPResult::error("告警不存在")));
    }
    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": deleted }))))
}

/// DELETE /api/alarm/batch - 批量删除告警（body `{"ids":[...]}`）
#[derive(Debug, Deserialize)]
pub struct AlarmBatchDelete {
    pub ids: Vec<i64>,
}

pub async fn alarm_batch_delete(
    State(state): State<AppState>,
    Json(body): Json<AlarmBatchDelete>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    delete_alarm_ids(&state.pool, &body.ids).await
}

/// DELETE /api/alarm/delete - 批量删除告警（WVP 契约：body 是**裸数组** `[1,2,3]`）
///
/// WVP 的 `AlarmController.delete(@RequestBody List<Long> ids)` 收的就是裸数组，
/// 其前端 `deleteAlarms(ids)` 也是 `data: ids`。只提供 `/batch` 会让按 WVP
/// 契约写的调用方拿不到端点。
pub async fn alarm_delete_batch_wvp(
    State(state): State<AppState>,
    Json(ids): Json<Vec<i64>>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    delete_alarm_ids(&state.pool, &ids).await
}

async fn delete_alarm_ids(
    pool: &crate::db::Pool,
    ids: &[i64],
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    if ids.is_empty() {
        return Ok(Json(WVPResult::success(serde_json::json!({ "deleted": 0 }))));
    }

    let mut deleted = 0u64;
    for id in ids {
        let id = *id;
        #[cfg(feature = "postgres")]
        let r = sqlx::query("DELETE FROM gb_device_alarm WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;
        #[cfg(any(feature = "mysql", feature = "sqlite"))]
        let r = sqlx::query("DELETE FROM gb_device_alarm WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;
        deleted += r.rows_affected();
    }

    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": deleted }))))
}

/// DELETE /api/alarm/clear - 按筛选条件清空告警（WVP `clearAlarmsByCondition`）
///
/// 关键差异：WVP 的这个接口**不接受单个 id**，只按 alarmType/beginTime/endTime
/// 清空；而此前的实现是 `delete_all()` —— 忽略全部参数、**清空整张表**。
/// 前端在单行点「清除」调的就是它，等于一次误删全库告警。
///
/// 现在的语义：与 `GET /api/alarm/list` 用**同一套筛选**（`alarm_filter`），
/// 清空的就是用户在页面上看到的那些；不带任何条件时才是清空全部，
/// 并且会明确告知被清空的条数。
pub async fn alarm_clear(
    State(state): State<AppState>,
    Query(q): Query<AlarmQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let w = alarm_filter(&q);
    // delete_where 只接受文本绑定；本函数的筛选条件全部是文本（无 Int 条件）
    let binds: Vec<String> = w
        .binds
        .iter()
        .map(|b| match b {
            BindValue::Text(v) => v.clone(),
            BindValue::Int(v) => v.to_string(),
            BindValue::Big(v) => v.to_string(),
            BindValue::Bool(v) => v.to_string(),
        })
        .collect();
    let where_sql = if w.conds.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", w.conds.join(" AND "))
    };
    let cleared = crate::db::alarm::delete_where(&state.pool, &where_sql, &binds)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("清空失败: {e}")))?;
    tracing::info!("alarm_clear: 清空 {cleared} 条告警（条件: {where_sql}）");
    Ok(Json(WVPResult::success(serde_json::json!({ "cleared": cleared }))))
}

/// DELETE /api/alarm/device/:device_id - 删除设备的所有告警
pub async fn alarm_delete_by_device(
    State(state): State<AppState>,
    axum::extract::Path(device_id): axum::extract::Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_device_alarm WHERE device_id = $1")
        .bind(&device_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("DELETE FROM gb_device_alarm WHERE device_id = ?")
        .bind(&device_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": r.rows_affected() }))))
}

/// DELETE /api/alarm/before/:time - 删除指定时间之前的告警
pub async fn alarm_delete_before_time(
    State(state): State<AppState>,
    axum::extract::Path(before_time): axum::extract::Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    #[cfg(feature = "postgres")]
    let r = sqlx::query("DELETE FROM gb_device_alarm WHERE create_time < $1")
        .bind(&before_time)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    #[cfg(any(feature = "mysql", feature = "sqlite"))]
    let r = sqlx::query("DELETE FROM gb_device_alarm WHERE create_time < ?")
        .bind(&before_time)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("数据库删除失败: {}", e)))?;

    Ok(Json(WVPResult::success(serde_json::json!({ "deleted": r.rows_affected() }))))
}

#[cfg(all(test, feature = "sqlite"))]
mod alarm_contract_tests {
    use super::*;
    use crate::test_support::app_state;

    async fn seed(state: &AppState, device: &str, atype: &str, atime: &str, priority: &str) {
        sqlx::query(
            "INSERT INTO gb_device_alarm \
             (device_id, channel_id, alarm_priority, alarm_method, alarm_type, alarm_time, \
              alarm_description, create_time, handled) \
             VALUES (?, 'ch1', ?, '1', ?, ?, 'desc', '2026-09-12 10:00:00', 0)",
        )
        .bind(device)
        .bind(priority)
        .bind(atype)
        .bind(atime)
        .execute(&state.pool)
        .await
        .expect("seed alarm");
    }

    fn q(v: serde_json::Value) -> AlarmQuery {
        serde_json::from_value(v).expect("AlarmQuery")
    }

    /// `beginTime`/`endTime` 此前被 DTO 收下却在 SQL 里从未使用 —— 选了时间范围
    /// 结果完全不变。这条测试用真实数据钉住它确实生效。
    #[tokio::test]
    async fn test_time_range_actually_filters() {
        let state = app_state().await;
        seed(&state, "d1", "1", "2026-09-01 08:00:00", "1").await;
        seed(&state, "d2", "1", "2026-09-10 08:00:00", "2").await;
        seed(&state, "d3", "1", "2026-09-20 08:00:00", "3").await;

        let all = alarm_list(State(state.clone()), Query(q(serde_json::json!({"page":1,"count":10}))))
            .await
            .unwrap();
        assert_eq!(all.0.data.as_ref().unwrap()["total"], 3);

        let ranged = alarm_list(
            State(state.clone()),
            Query(q(serde_json::json!({
                "page": 1, "count": 10,
                "beginTime": "2026-09-05 00:00:00",
                "endTime": "2026-09-15 00:00:00"
            }))),
        )
        .await
        .unwrap();
        let d = ranged.0.data.unwrap();
        assert_eq!(d["total"], 1, "时间范围必须真的过滤: {d}");
        assert_eq!(d["list"][0]["deviceId"], "d2");

        // startTime 是历史别名，也要能用
        let alias = alarm_list(
            State(state.clone()),
            Query(q(serde_json::json!({"page":1,"count":10,"startTime":"2026-09-05 00:00:00"}))),
        )
        .await
        .unwrap();
        assert_eq!(alias.0.data.unwrap()["total"], 2);
    }

    /// 关键字（页面搜索框）此前后端没有该参数，输入后结果完全不变。
    #[tokio::test]
    async fn test_keyword_filters_device_and_description() {
        let state = app_state().await;
        seed(&state, "34020000001320000001", "1", "2026-09-01 08:00:00", "1").await;
        seed(&state, "34020000001320000002", "1", "2026-09-01 08:00:00", "1").await;

        let hit = alarm_list(
            State(state.clone()),
            Query(q(serde_json::json!({"page":1,"count":10,"query":"000002"}))),
        )
        .await
        .unwrap();
        assert_eq!(hit.0.data.unwrap()["total"], 1, "只有 dev2 命中");

        let desc = alarm_list(
            State(state.clone()),
            Query(q(serde_json::json!({"page":1,"count":10,"query":"desc"}))),
        )
        .await
        .unwrap();
        assert_eq!(desc.0.data.unwrap()["total"], 2, "描述也要匹配");
    }

    /// **安全回归**：`/api/alarm/clear` 此前忽略全部参数、无条件 `DELETE FROM
    /// gb_device_alarm`。带筛选条件调用时只能清掉匹配的行。
    #[tokio::test]
    async fn test_clear_by_condition_does_not_wipe_everything() {
        let state = app_state().await;
        seed(&state, "d1", "1", "2026-09-01 08:00:00", "1").await;
        seed(&state, "d2", "2", "2026-09-10 08:00:00", "2").await;
        seed(&state, "d3", "1", "2026-09-20 08:00:00", "3").await;

        let res = alarm_clear(
            State(state.clone()),
            Query(q(serde_json::json!({"alarmType": "1"}))),
        )
        .await
        .unwrap();
        assert_eq!(res.0.data.unwrap()["cleared"], 2);

        let left = alarm_list(State(state.clone()), Query(q(serde_json::json!({"page":1,"count":10}))))
            .await
            .unwrap();
        let d = left.0.data.unwrap();
        assert_eq!(d["total"], 1, "只应清掉 alarmType=1 的两条: {d}");
        assert_eq!(d["list"][0]["deviceId"], "d2");
    }

    /// 处理告警：必须能落库「处理结论」（此前 DTO 里没有 result 字段）。
    #[tokio::test]
    async fn test_handle_persists_result_and_reads_back() {
        let state = app_state().await;
        seed(&state, "d1", "1", "2026-09-01 08:00:00", "1").await;

        // 前端 api 层发的键名是 `result`（与后端字段不同名），必须能绑上
        let body: AlarmHandleBody = serde_json::from_value(serde_json::json!({
            "id": 1, "handleUser": "admin", "handled": true, "result": "已电话确认"
        }))
        .expect("前端载荷必须能反序列化");
        assert_eq!(body.handle_result.as_deref(), Some("已电话确认"));
        let res = alarm_handle(State(state.clone()), Json(body)).await.unwrap();
        assert_eq!(res.0.data.unwrap()["handled"], true);

        let detail = alarm_detail(State(state.clone()), axum::extract::Path(1i64)).await.unwrap();
        let d = detail.0.data.unwrap();
        assert_eq!(d["handled"], true);
        assert_eq!(d["handleUser"], "admin");
        assert_eq!(d["handleResult"], "已电话确认", "处理结论必须落库并返回");

        // handled 过滤也应生效
        let handled = alarm_list(
            State(state.clone()),
            Query(q(serde_json::json!({"page":1,"count":10,"handled":true}))),
        )
        .await
        .unwrap();
        assert_eq!(handled.0.data.unwrap()["total"], 1);
        let pending = alarm_list(
            State(state.clone()),
            Query(q(serde_json::json!({"page":1,"count":10,"handled":false}))),
        )
        .await
        .unwrap();
        assert_eq!(pending.0.data.unwrap()["total"], 0);
    }

    /// WVP 的批量删除是 `DELETE /api/alarm/delete` + **裸数组** body。
    #[tokio::test]
    async fn test_wvp_batch_delete_accepts_bare_array() {
        let state = app_state().await;
        seed(&state, "d1", "1", "2026-09-01 08:00:00", "1").await;
        seed(&state, "d2", "1", "2026-09-02 08:00:00", "1").await;
        seed(&state, "d3", "1", "2026-09-03 08:00:00", "1").await;

        let body: Vec<i64> = serde_json::from_value(serde_json::json!([1, 3])).unwrap();
        let res = alarm_delete_batch_wvp(State(state.clone()), Json(body)).await.unwrap();
        assert_eq!(res.0.data.unwrap()["deleted"], 2);

        let left = alarm_list(State(state.clone()), Query(q(serde_json::json!({"page":1,"count":10}))))
            .await
            .unwrap();
        assert_eq!(left.0.data.unwrap()["total"], 1);
    }
}
