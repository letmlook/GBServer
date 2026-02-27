//! 实时播放 /api/play，对应前端 play.js（实际拉流需 ZLM/SIP，此处先返回占位）

use axum::Json;

use crate::response::WVPResult;

/// GET /api/play/start/:deviceId/:channelId
pub async fn play_start() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "app": "",
        "stream": "",
        "tracks": []
    })))
}
/// GET /api/play/stop/:deviceId/:channelId
pub async fn play_stop() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// GET /api/play/broadcast/:deviceId/:channelId
pub async fn broadcast_start() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
/// GET /api/play/broadcast/stop/:deviceId/:channelId
pub async fn broadcast_stop() -> Json<WVPResult<()>> {
    Json(WVPResult::<()>::success_empty())
}
