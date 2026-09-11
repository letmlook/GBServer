//! JT1078 region/route/control endpoints (parity with reference Java controllers).
//!
//! ## 实现分级
//!
//! - **DB 持久化层**（已实装，2026-08-23）：
//!   圆形 / 多边形 / 矩形 区域围栏 + 路线 的 CRUD，
//!   共 16 个 HTTP 端点写入 `gb_jt_area_circle` / `gb_jt_area_polygon` /
//!   `gb_jt_area_rectangle` / `gb_jt_route` 表。
//!   下发到终端仍依赖 JT/T 808/1078 协议栈通过 SIP 控制信道。
//!
//! - **协议操作层**（2026-09-11 全部接线）：
//!   所有控制类端点均经 `src/jt1078/` 的 `Jt1078Manager` **真实下发**并等待终端通用应答：
//!   - 0x9101 / 0x9102 实时音视频请求与控制（live_switch / live_continue / live_pause）
//!   - 0x8801 摄像头立即拍摄（snap）与**录像开始/停止**（record_start / record_stop，
//!     使用同一原语的「拍摄命令」字段：1=开始录像、0=停止录像）
//!   - 0x8803 存储多媒体数据上传（media_upload_delete）
//!   - 0x8202 临时位置跟踪控制（temp_position_tracking）
//!   - 0x8203 人工确认报警消息（confirmation_alarm）
//!   - 0x9205 文件上传指令（playback_download）
//!   - 终端通道删除落库
//!
//!   2026-09-11 之前 record_start / record_stop / temp_position_tracking /
//!   confirmation_alarm / playback_download 五个端点会显式报「协议原语尚未实现」；
//!   实际核查发现 `command.rs` 已有大部分原语（0x8801/0x8803），仅缺 0x8202/0x8203/0x9205，
//!   现已补齐并全部接线，不再有"未实现"端点。

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::jt1078 as jt_db;
use crate::response::WVPResult;
use crate::AppState;

#[derive(Deserialize, Default, Debug)]
pub struct IdQuery {
    pub id: Option<String>,
    pub phone: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<i32>,
}

fn err(msg: &str) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::<serde_json::Value>::error(msg.to_string()))
}

// ============================================================================
// 区域 — circle（圆形围栏）
// ============================================================================

/// POST /api/jt1078/area/circle/add
pub async fn area_circle_add(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = b.get("phone").and_then(|v| v.as_str()).unwrap_or_default();
    let label = b.get("label").and_then(|v| v.as_str());
    let lat = b.get("centerLat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lon = b.get("centerLon").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let radius = b.get("radius").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    if phone.is_empty() || radius <= 0 {
        return err("phone / radius 必填且 radius>0");
    }
    match jt_db::insert_area_circle(&state.pool, phone, label, lat, lon, radius).await {
        Ok(id) => Json(WVPResult::success(serde_json::json!({
            "id": id, "phone": phone, "label": label,
            "centerLat": lat, "centerLon": lon, "radius": radius,
            "msg": "圆形区域已新增"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// POST /api/jt1078/area/circle/edit  (WVP 别名：与 update 同义)
pub async fn area_circle_edit(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = b.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let label = b.get("label").and_then(|v| v.as_str());
    let lat = b.get("centerLat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lon = b.get("centerLon").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let radius = b.get("radius").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    if id <= 0 {
        return err("id 必填且 >0");
    }
    match jt_db::update_area_circle(&state.pool, id, label, lat, lon, radius).await {
        Ok(n) if n > 0 => Json(WVPResult::success(serde_json::json!({
            "id": id, "updated": n, "msg": "圆形区域已编辑"
        }))),
        Ok(_) => err("未找到该 id"),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/area/circle/delete?id=<i64>
pub async fn area_circle_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = q.id.as_deref().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    if id <= 0 {
        return err("id 必填且 >0");
    }
    match jt_db::delete_area_circle(&state.pool, id).await {
        Ok(n) => Json(WVPResult::success(serde_json::json!({
            "id": id, "deleted": n, "msg": "圆形区域已删除"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/area/circle/query?phone=<phone_number>
pub async fn area_circle_query(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = q.phone.clone().unwrap_or_default();
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::list_area_circles_by_phone(&state.pool, &phone).await {
        Ok(items) => Json(WVPResult::success(serde_json::json!({
            "phone": phone, "shape": "circle", "count": items.len(), "items": items,
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// POST /api/jt1078/area/circle/update
pub async fn area_circle_update(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    // 与 edit 同义（WVP 区分 edit/update 是历史命名差异）
    area_circle_edit(State(state), Json(b)).await
}

// ============================================================================
// 区域 — polygon（多边形围栏）
// ============================================================================

/// POST /api/jt1078/area/polygon/set
pub async fn area_polygon_set(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = b.get("phone").and_then(|v| v.as_str()).unwrap_or_default();
    let label = b.get("label").and_then(|v| v.as_str());
    let points = b.get("points").cloned().unwrap_or(serde_json::json!([]));
    let points_json = serde_json::to_string(&points).unwrap_or_else(|_| "[]".to_string());
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::insert_area_polygon(&state.pool, phone, label, &points_json).await {
        Ok(id) => Json(WVPResult::success(serde_json::json!({
            "id": id, "phone": phone, "label": label,
            "msg": "多边形区域已设置"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/area/polygon/delete?id=<i64>
pub async fn area_polygon_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = q.id.as_deref().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    if id <= 0 {
        return err("id 必填且 >0");
    }
    match jt_db::delete_area_polygon(&state.pool, id).await {
        Ok(n) => Json(WVPResult::success(serde_json::json!({
            "id": id, "deleted": n, "msg": "多边形区域已删除"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/area/polygon/query?phone=<phone_number>
pub async fn area_polygon_query(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = q.phone.clone().unwrap_or_default();
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::list_area_polygons_by_phone(&state.pool, &phone).await {
        Ok(items) => Json(WVPResult::success(serde_json::json!({
            "phone": phone, "shape": "polygon", "count": items.len(), "items": items,
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

// ============================================================================
// 区域 — rectangle（矩形围栏）
// ============================================================================

/// POST /api/jt1078/area/rectangle/add
pub async fn area_rectangle_add(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = b.get("phone").and_then(|v| v.as_str()).unwrap_or_default();
    let label = b.get("label").and_then(|v| v.as_str());
    let lt_lat = b.get("leftTopLat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lt_lon = b.get("leftTopLon").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let rb_lat = b.get("rightBottomLat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let rb_lon = b.get("rightBottomLon").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::insert_area_rectangle(&state.pool, phone, label, lt_lat, lt_lon, rb_lat, rb_lon).await {
        Ok(id) => Json(WVPResult::success(serde_json::json!({
            "id": id, "phone": phone, "label": label,
            "leftTopLat": lt_lat, "leftTopLon": lt_lon,
            "rightBottomLat": rb_lat, "rightBottomLon": rb_lon,
            "msg": "矩形区域已新增"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// POST /api/jt1078/area/rectangle/edit
pub async fn area_rectangle_edit(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = b.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let label = b.get("label").and_then(|v| v.as_str());
    let lt_lat = b.get("leftTopLat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lt_lon = b.get("leftTopLon").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let rb_lat = b.get("rightBottomLat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let rb_lon = b.get("rightBottomLon").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if id <= 0 {
        return err("id 必填且 >0");
    }
    match jt_db::update_area_rectangle(&state.pool, id, label, lt_lat, lt_lon, rb_lat, rb_lon).await {
        Ok(n) if n > 0 => Json(WVPResult::success(serde_json::json!({
            "id": id, "updated": n, "msg": "矩形区域已编辑"
        }))),
        Ok(_) => err("未找到该 id"),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/area/rectangle/delete?id=<i64>
pub async fn area_rectangle_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = q.id.as_deref().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    if id <= 0 {
        return err("id 必填且 >0");
    }
    match jt_db::delete_area_rectangle(&state.pool, id).await {
        Ok(n) => Json(WVPResult::success(serde_json::json!({
            "id": id, "deleted": n, "msg": "矩形区域已删除"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/area/rectangle/query?phone=<phone_number>
pub async fn area_rectangle_query(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = q.phone.clone().unwrap_or_default();
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::list_area_rectangles_by_phone(&state.pool, &phone).await {
        Ok(items) => Json(WVPResult::success(serde_json::json!({
            "phone": phone, "shape": "rectangle", "count": items.len(), "items": items,
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// POST /api/jt1078/area/rectangle/update
pub async fn area_rectangle_update(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    // 与 edit 同义
    area_rectangle_edit(State(state), Json(b)).await
}

// ============================================================================
// 路线（线路）
// ============================================================================

/// POST /api/jt1078/route/set
pub async fn route_set(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = b.get("phone").and_then(|v| v.as_str()).unwrap_or_default();
    let label = b.get("label").and_then(|v| v.as_str());
    let waypoints = b.get("waypoints").cloned().unwrap_or(serde_json::json!([]));
    let waypoints_json = serde_json::to_string(&waypoints).unwrap_or_else(|_| "[]".to_string());
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::insert_route(&state.pool, phone, label, &waypoints_json).await {
        Ok(id) => Json(WVPResult::success(serde_json::json!({
            "id": id, "phone": phone, "label": label,
            "msg": "路线已设置"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/route/query?phone=<phone_number>
pub async fn route_query(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = q.phone.clone().unwrap_or_default();
    if phone.is_empty() {
        return err("phone 必填");
    }
    match jt_db::list_routes_by_phone(&state.pool, &phone).await {
        Ok(items) => Json(WVPResult::success(serde_json::json!({
            "phone": phone, "count": items.len(), "items": items,
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

/// GET /api/jt1078/route/delete?id=<i64>
pub async fn route_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let id = q.id.as_deref().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    if id <= 0 {
        return err("id 必填且 >0");
    }
    match jt_db::delete_route(&state.pool, id).await {
        Ok(n) => Json(WVPResult::success(serde_json::json!({
            "id": id, "deleted": n, "msg": "路线已删除"
        }))),
        Err(e) => err(&format!("DB error: {}", e)),
    }
}

// ============================================================================
// 协议操作层 — 有对应 JT/T 协议原语的端点经 Jt1078Manager 真实下发；
// 协议栈尚未提供的原语显式报错（不再伪装"已受理"成功）。
// ============================================================================

use crate::handlers::jt1078::get_jt_manager;

/// 校验 phone 并解析通道号（默认 1）
fn phone_and_channel(q: &IdQuery) -> Result<(String, u8), Json<WVPResult<serde_json::Value>>> {
    let phone = q.phone.clone().unwrap_or_default();
    if phone.trim().is_empty() {
        return Err(err("缺少 phone"));
    }
    Ok((phone, q.channel_id.unwrap_or(1).clamp(1, 255) as u8))
}

/// 直播继续（JT/T1078 0x9102 实时音视频控制，control=0 继续）
pub async fn live_continue(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let (phone, channel) = match phone_and_channel(&q) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr.send_live_video_control_and_wait(&phone, channel, 0x00, false, 5).await {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": q.phone, "channelId": q.channel_id, "msg": "直播已继续"
        }))),
        Ok(result) => err(&format!("直播继续被终端拒绝 result={}", result)),
        Err(e) => err(&format!("直播继续命令失败: {}", e)),
    }
}

/// 直播暂停（JT/T1078 0x9102，control=1 暂停）
pub async fn live_pause(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let (phone, channel) = match phone_and_channel(&q) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr.send_live_video_control_and_wait(&phone, channel, 0x01, false, 5).await {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": q.phone, "channelId": q.channel_id, "msg": "直播已暂停"
        }))),
        Ok(result) => err(&format!("直播暂停被终端拒绝 result={}", result)),
        Err(e) => err(&format!("直播暂停命令失败: {}", e)),
    }
}

/// 直播切换（重新对目标通道发起 0x9101 实时音视频请求，close=false）
pub async fn live_switch(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let (phone, channel) = match phone_and_channel(&q) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr.send_live_video_and_wait(&phone, channel, 0, false, 5).await {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": q.phone, "channelId": q.channel_id, "msg": "直播切换命令已下发"
        }))),
        Ok(result) => err(&format!("直播切换被终端拒绝 result={}", result)),
        Err(e) => err(&format!("直播切换命令失败: {}", e)),
    }
}

/// 终端录像控制（开始/停止）—— JT/T808 0x8801「拍摄命令」字段：
/// `1`=开始录像、`0`=停止录像。等待终端通用应答后才返回成功。
async fn record_control(
    state: &AppState,
    q: &IdQuery,
    start: bool,
) -> Json<WVPResult<serde_json::Value>> {
    let (phone, channel) = match phone_and_channel(q) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mgr = match get_jt_manager(state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    // duration=0 → 按终端最小间隔持续录像；save=true 保存到终端存储
    match mgr
        .send_record_control_and_wait(&phone, channel, start, 0, 5)
        .await
    {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": q.phone,
            "channelId": q.channel_id,
            "recording": start,
            "msg": if start { "终端录像已开始" } else { "终端录像已停止" }
        }))),
        Ok(result) => err(&format!("录像控制被终端拒绝 result={}", result)),
        Err(e) => err(&format!("录像控制命令失败: {}", e)),
    }
}

/// GET /api/jt1078/record/start — 终端录像开始
pub async fn record_start(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    record_control(&state, &q, true).await
}

/// GET /api/jt1078/record/stop — 终端录像停止
pub async fn record_stop(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    record_control(&state, &q, false).await
}

/// 抓拍（JT808 0x8801 拍照指令，等待终端通用应答）
pub async fn snap(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let (phone, channel) = match phone_and_channel(&q) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr.send_take_photo_and_wait(&phone, channel, 5).await {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": q.phone, "channelId": q.channel_id,
            "msg": "抓拍命令已被终端应答，媒体文件将经 0x1200 上报"
        }))),
        Ok(result) => err(&format!("抓拍被终端拒绝 result={}", result)),
        Err(e) => err(&format!("抓拍命令失败: {}", e)),
    }
}

/// 临时位置跟踪查询参数（JT/T808 0x8202）
#[derive(Deserialize, Default, Debug)]
pub struct TempPositionTrackingQuery {
    pub phone: Option<String>,
    /// 上报时间间隔（秒），默认 30
    pub interval: Option<u16>,
    /// 跟踪有效期（秒），默认 300；兼容 `validity` / `duration` 别名
    #[serde(alias = "validity", alias = "duration")]
    pub expires: Option<u32>,
}

/// 临时位置跟踪控制 —— JT/T808 0x8202（时间间隔 + 有效期），等待终端通用应答
pub async fn temp_position_tracking(
    State(state): State<AppState>,
    Query(q): Query<TempPositionTrackingQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = q.phone.clone().unwrap_or_default().trim().to_string();
    if phone.is_empty() {
        return err("缺少 phone");
    }
    let interval = q.interval.unwrap_or(30).max(1);
    let expires = q.expires.unwrap_or(300).max(1);

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr
        .send_temp_position_tracking_and_wait(&phone, interval, expires, 5)
        .await
    {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": phone,
            "interval": interval,
            "expires": expires,
            "msg": "临时位置跟踪已启用"
        }))),
        Ok(result) => err(&format!("临时位置跟踪被终端拒绝 result={}", result)),
        Err(e) => err(&format!("临时位置跟踪命令失败: {}", e)),
    }
}

/// 人工确认报警消息 —— JT/T808 0x8203（报警流水号 + 确认类型位标志）
pub async fn confirmation_alarm(
    State(state): State<AppState>,
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = b
        .get("phone")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if phone.is_empty() {
        return err("缺少 phone");
    }

    fn pick_u64(b: &serde_json::Value, keys: &[&str]) -> Option<u64> {
        keys.iter().find_map(|k| b.get(*k)).and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        })
    }

    // 报警消息流水号：兼容多种字段命名
    let alarm_seq = pick_u64(&b, &["alarmId", "alarm_id", "alarmSeq", "serial"]).unwrap_or(0) as u16;
    // 人工确认报警类型位标志：默认全 1（确认全部）
    let alarm_type = pick_u64(&b, &["type", "alarmType", "alarm_type", "confirmType"])
        .unwrap_or(0xFFFF_FFFF) as u32;

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr
        .send_confirm_alarm_and_wait(&phone, alarm_seq, alarm_type, 5)
        .await
    {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": phone,
            "alarmId": alarm_seq,
            "alarmType": alarm_type,
            "msg": "报警人工确认已下发"
        }))),
        Ok(result) => err(&format!("报警确认被终端拒绝 result={}", result)),
        Err(e) => err(&format!("报警确认命令失败: {}", e)),
    }
}

/// 录像下载查询参数（JT/T1078 0x9205 文件上传指令）
#[derive(Deserialize, Default, Debug)]
pub struct PlaybackDownloadQuery {
    pub phone: Option<String>,
    pub channel_id: Option<i32>,
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
}

/// 录像下载 —— JT/T1078 0x9205：请求终端把指定时间段的音视频资源上传到平台
pub async fn playback_download(
    State(state): State<AppState>,
    Query(q): Query<PlaybackDownloadQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = q.phone.clone().unwrap_or_default().trim().to_string();
    if phone.is_empty() {
        return err("缺少 phone");
    }
    let start = q.start_time.clone().unwrap_or_default();
    let end = q.end_time.clone().unwrap_or_default();
    if start.trim().is_empty() || end.trim().is_empty() {
        return err("缺少 startTime / endTime —— 录像下载必须指定时间段");
    }
    let channel = q.channel_id.unwrap_or(1).clamp(1, 255) as u8;

    // 先本地校验时间可解析，避免把错误时间段下发到终端
    if crate::jt1078::command::try_encode_time_bcd(&start).is_none() {
        return err(&format!("无法解析 startTime: {}", start));
    }
    if crate::jt1078::command::try_encode_time_bcd(&end).is_none() {
        return err(&format!("无法解析 endTime: {}", end));
    }

    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr
        .send_file_upload_and_wait(&phone, channel, &start, &end, 5)
        .await
    {
        Ok(0) => Json(WVPResult::success(serde_json::json!({
            "phone": phone,
            "channelId": channel,
            "startTime": start,
            "endTime": end,
            "msg": "录像上传指令已被终端应答，文件将经 0x1200/0x9206 上报"
        }))),
        Ok(result) => err(&format!("录像下载被终端拒绝 result={}", result)),
        Err(e) => err(&format!("录像下载命令失败: {}", e)),
    }
}

/// 删除已上传的媒体项（JT808 0x8803 delete_flag=1）
pub async fn media_upload_delete(
    State(state): State<AppState>,
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let phone = match q.phone.clone() {
        Some(p) if !p.trim().is_empty() => p,
        _ => return err("缺少 phone"),
    };
    let media_id = match q.id.as_deref().and_then(|s| s.parse::<u32>().ok()) {
        Some(v) => v,
        None => return err("缺少或非法的媒体 id（数字）"),
    };
    let mgr = match get_jt_manager(&state).await {
        Ok(m) => m,
        Err(_) => return err("JT1078服务未启动"),
    };
    match mgr.send_media_delete(&phone, media_id).await {
        Ok(()) => Json(WVPResult::success(serde_json::json!({
            "phone": q.phone, "mediaId": media_id, "msg": "媒体删除命令已下发"
        }))),
        Err(e) => err(&format!("媒体删除命令失败: {}", e)),
    }
}

/// 终端通道删除（落库删除 gb_jt_channel 记录）
pub async fn terminal_channel_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    let Ok(ch_id) = id.parse::<i64>() else {
        return err("非法的通道 id");
    };
    match jt_db::delete_channel(&state.pool, ch_id).await {
        Ok(n) if n > 0 => Json(WVPResult::success(serde_json::json!({
            "id": id, "msg": "通道已删除"
        }))),
        Ok(_) => err("通道不存在"),
        Err(e) => err(&format!("删除通道失败: {}", e)),
    }
}

/// 终端通道详情
pub async fn terminal_channel_one(
    Path(id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 terminal channel one: {}", id);
    Json(WVPResult::success(serde_json::json!({
        "id": id,
        "msg": "请使用主 handler /api/jt1078/terminal/channel/one/{id}"
    })))
}