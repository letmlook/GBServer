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
//! - **协议操作层**（2026-09-11 部分接线）：
//!   live_continue / live_pause / live_switch / snap / media_upload_delete /
//!   terminal_channel_delete 经 `src/jt1078/` 的 Jt1078Manager 真实下发
//!   （0x9102 / 0x9101 / 0x8801 / 0x8803 / 落库删除），并等待终端通用应答。
//!   record_start / record_stop / temp_position_tracking / confirmation_alarm /
//!   playback_download —— 协议栈尚未提供对应原语（0x8202/0x8203/0x9205 等），
//!   HTTP 层显式返回错误（不再伪装"已受理"成功）。

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

/// 终端录像开始 — 协议栈尚未提供对应原语（JT/T1078 终端录像控制），
/// 显式报错而非伪装受理成功
pub async fn record_start(
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 record start (unwired): {:?}", q);
    err("终端录像开始指令依赖的 JT/T1078 协议原语尚未实现，命令未下发")
}

/// 终端录像停止 — 同上，显式报错
pub async fn record_stop(
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 record stop (unwired): {:?}", q);
    err("终端录像停止指令依赖的 JT/T1078 协议原语尚未实现，命令未下发")
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

/// 临时位置跟踪 — JT808 0x8203 原语尚未实现，显式报错
pub async fn temp_position_tracking(
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 temp position tracking (unwired): {:?}", q);
    err("临时位置跟踪依赖的 JT/T808 0x8203 协议原语尚未实现，命令未下发")
}

/// 报警确认应答 — JT808 0x8202 原语尚未实现，显式报错
pub async fn confirmation_alarm(
    Json(b): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 confirmation alarm (unwired): {}", b);
    err("报警确认应答依赖的 JT/T808 0x8202 协议原语尚未实现，命令未下发")
}

/// 录像下载 — JT/T1078 0x9205 回放上传原语尚未实现，显式报错
pub async fn playback_download(
    Query(q): Query<IdQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 playback download (unwired): {:?}", q);
    err("录像下载依赖的 JT/T1078 0x9205 协议原语尚未实现，命令未下发")
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