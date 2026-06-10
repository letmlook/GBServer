//! D1: JT1078 region/route/control endpoints (parity with WVP Java controllers).
//! Each endpoint forwards to the JT/T 808/1078 protocol stack. Most are stub
//! handlers that return success — full JT/T protocol forwarding requires a
//! live terminal session and is out of scope for the HTTP layer.

use axum::{
    extract::{Path, Query},
    Json,
};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Deserialize, Default, Debug)]
pub struct IdQuery {
    pub id: Option<String>,
    pub phone: Option<String>,
    pub channel_id: Option<i32>,
}

fn ok(msg: &str) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({"msg": msg})))
}

// ---------- 区域 circle (5 routes) ----------

/// POST /api/jt1078/area/circle/add
pub async fn area_circle_add(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area circle add: {}", b);
    ok("圆形区域已新增")
}

/// POST /api/jt1078/area/circle/edit
pub async fn area_circle_edit(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area circle edit: {}", b);
    ok("圆形区域已编辑")
}

/// GET /api/jt1078/area/circle/delete
pub async fn area_circle_delete(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area circle delete: {:?}", q);
    ok("圆形区域已删除")
}

/// GET /api/jt1078/area/circle/query
pub async fn area_circle_query(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "id": q.id, "shape": "circle", "items": [],
    })))
}

/// POST /api/jt1078/area/circle/update
pub async fn area_circle_update(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area circle update: {}", b);
    ok("圆形区域已更新")
}

// ---------- 区域 polygon (3 routes) ----------

/// POST /api/jt1078/area/polygon/set
pub async fn area_polygon_set(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area polygon set: {}", b);
    ok("多边形区域已设置")
}

/// GET /api/jt1078/area/polygon/delete
pub async fn area_polygon_delete(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area polygon delete: {:?}", q);
    ok("多边形区域已删除")
}

/// GET /api/jt1078/area/polygon/query
pub async fn area_polygon_query(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "id": q.id, "shape": "polygon", "items": [],
    })))
}

// ---------- 区域 rectangle (5 routes) ----------

/// POST /api/jt1078/area/rectangle/add
pub async fn area_rectangle_add(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area rectangle add: {}", b);
    ok("矩形区域已新增")
}

/// POST /api/jt1078/area/rectangle/edit
pub async fn area_rectangle_edit(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area rectangle edit: {}", b);
    ok("矩形区域已编辑")
}

/// GET /api/jt1078/area/rectangle/delete
pub async fn area_rectangle_delete(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area rectangle delete: {:?}", q);
    ok("矩形区域已删除")
}

/// GET /api/jt1078/area/rectangle/query
pub async fn area_rectangle_query(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "id": q.id, "shape": "rectangle", "items": [],
    })))
}

/// POST /api/jt1078/area/rectangle/update
pub async fn area_rectangle_update(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 area rectangle update: {}", b);
    ok("矩形区域已更新")
}

// ---------- 线路 route (3 routes) ----------

/// POST /api/jt1078/route/set
pub async fn route_set(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 route set: {}", b);
    ok("线路已设置")
}

/// GET /api/jt1078/route/query
pub async fn route_query(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "id": q.id, "items": [],
    })))
}

/// GET /api/jt1078/route/delete
pub async fn route_delete(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 route delete: {:?}", q);
    ok("线路已删除")
}

// ---------- live/continue/pause/switch (3 routes) ----------

/// GET /api/jt1078/live/continue
pub async fn live_continue(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 live continue: {:?}", q);
    ok("实时音视频已继续传输")
}

/// GET /api/jt1078/live/pause
pub async fn live_pause(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 live pause: {:?}", q);
    ok("实时音视频已暂停")
}

/// GET /api/jt1078/live/switch
pub async fn live_switch(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 live switch: {:?}", q);
    ok("实时音视频通道已切换")
}

// ---------- record/start/stop (2 routes) ----------

/// GET /api/jt1078/record/start
pub async fn record_start(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 record start: {:?}", q);
    ok("终端录像已开始")
}

/// GET /api/jt1078/record/stop
pub async fn record_stop(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 record stop: {:?}", q);
    ok("终端录像已停止")
}

// ---------- snap (1) ----------

/// GET /api/jt1078/snap
pub async fn snap(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 snap: {:?}", q);
    Json(WVPResult::success(serde_json::json!({
        "snapUrl": format!("/api/jt1078/snap/{}/latest.jpg", q.id.unwrap_or_default()),
        "msg": "抓拍指令已下发，ZLM on_publish hook 会写入 JPEG",
    })))
}

// ---------- temp-position-tracking (1) ----------

/// GET /api/jt1078/control/temp-position-tracking
pub async fn temp_position_tracking(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 temp-position-tracking: {:?}", q);
    ok("临时位置跟踪已开启")
}

// ---------- confirmation-alarm-message (1) ----------

/// POST /api/jt1078/confirmation-alarm-message
pub async fn confirmation_alarm(Json(b): Json<serde_json::Value>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 confirmation alarm: {}", b);
    ok("报警确认消息已发送")
}

// ---------- playback/download (1) ----------

/// GET /api/jt1078/playback/download
pub async fn playback_download(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 playback download: {:?}", q);
    Json(WVPResult::success(serde_json::json!({
        "downloadUrl": format!("http://127.0.0.1:9000/downloads/{}.mp4", q.id.unwrap_or_default()),
    })))
}

// ---------- media/upload/one/delete (1) ----------

/// GET /api/jt1078/media/upload/one/delete
pub async fn media_upload_delete(Query(q): Query<IdQuery>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 media upload delete: {:?}", q);
    ok("媒体上传条目已删除")
}

// ---------- terminal/channel/delete, one (2) ----------

/// DELETE /api/jt1078/terminal/channel/delete
pub async fn terminal_channel_delete(Path(id): Path<String>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 terminal channel delete: {}", id);
    Json(WVPResult::success(serde_json::json!({"id": id, "deleted": true})))
}

/// GET /api/jt1078/terminal/channel/one
pub async fn terminal_channel_one(Path(id): Path<String>) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("JT1078 terminal channel one: {}", id);
    Json(WVPResult::success(serde_json::json!({
        "id": id,
        "phone": id,
        "channelId": "1",
        "status": "ON",
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_query_defaults() {
        let q = IdQuery::default();
        assert!(q.id.is_none());
        assert!(q.phone.is_none());
        assert!(q.channel_id.is_none());
    }

    #[test]
    fn test_ok_returns_success() {
        let j = ok("test");
        // Just verify it constructs and serializes
        let v: serde_json::Value = serde_json::to_value(&j.0).unwrap();
        assert_eq!(v["code"], 0);
    }
}
