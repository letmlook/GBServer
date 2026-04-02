use axum::{extract::{Path, State}, Json};
use crate::response::WVPResult;
use crate::AppState;

pub async fn talk_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Talk start: device={}, channel={}", device_id, channel_id);

    let call_id = format!("talk_{}_{}_{}", device_id, channel_id,
        chrono::Utc::now().timestamp_millis());

    let media_port = 9000u16;
    let sdp = crate::sip::gb28181::talk::build_invite_sdp(&channel_id, media_port);

    Json(WVPResult::success(serde_json::json!({
        "callId": call_id,
        "deviceId": device_id,
        "channelId": channel_id,
        "mediaPort": media_port,
        "sdp": sdp,
        "status": "ready"
    })))
}

pub async fn talk_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Talk stop: device={}, channel={}", device_id, channel_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn talk_invite(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Talk invite: device={}, channel={}", device_id, channel_id);

    let call_id = format!("invite_{}_{}", device_id, channel_id);

    Json(WVPResult::success(serde_json::json!({
        "callId": call_id,
        "deviceId": device_id,
        "channelId": channel_id,
        "sdp": crate::sip::gb28181::talk::build_invite_sdp(&channel_id, 9000),
        "mediaPort": 9000
    })))
}

pub async fn talk_ack(
    State(state): State<AppState>,
    Path(call_id): Path<String>,
) -> Json<WVPResult<()>> {
    tracing::info!("Talk ACK: call_id={}", call_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn talk_bye(
    State(state): State<AppState>,
    Path(call_id): Path<String>,
) -> Json<WVPResult<()>> {
    tracing::info!("Talk BYE: call_id={}", call_id);
    Json(WVPResult::<()>::success_empty())
}
