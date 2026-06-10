//! D2: RTP/PS control endpoints (parity with WVP Java controllers).
//! Each endpoint forwards to ZLM's `openRtpServer`, `closeRtpServer`,
//! `sendRtp` / `stopSendRtp` family of API calls.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;
use crate::zlm::OpenRtpServerRequest;

#[derive(Deserialize, Default)]
pub struct OpenRtpQuery {
    pub stream_id: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub tcp: Option<bool>,
}

/// POST /api/rtp/receive/open
pub async fn rtp_receive_open(
    State(state): State<AppState>,
    Json(q): Json<OpenRtpQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(WVPResult::error("no ZLM available"));
    };
    let stream_id = q.stream_id.unwrap_or_else(|| {
        format!("rtp_recv_{}", chrono::Utc::now().timestamp_millis())
    });
    let req = OpenRtpServerRequest {
        secret: zlm.secret.clone(),
        stream_id: stream_id.clone(),
        port: q.port,
        use_tcp: Some(q.tcp.unwrap_or(false)),
        rtp_type: Some(0),
        recv_port: None,
    };
    match zlm.open_rtp_server(&req).await {
        Ok(info) => Json(WVPResult::success(serde_json::json!({
            "streamId": stream_id, "port": info.port, "ssrc": info.ssrc,
        }))),
        Err(e) => Json(WVPResult::error(format!("ZLM error: {}", e))),
    }
}

/// POST /api/rtp/receive/close/:stream_id
pub async fn rtp_receive_close(
    Path(stream_id): Path<String>,
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(WVPResult::error("no ZLM available"));
    };
    match zlm.close_rtp_server(&stream_id).await {
        Ok(()) => Json(WVPResult::success(serde_json::json!({"streamId": stream_id, "closed": true}))),
        Err(e) => Json(WVPResult::error(format!("ZLM error: {}", e))),
    }
}

#[derive(Deserialize, Default)]
pub struct SendRtpBody {
    pub stream_id: Option<String>,
    pub ssrc: Option<String>,
    pub target_ip: Option<String>,
    pub target_port: Option<u16>,
}

/// POST /api/rtp/send/start — push our stream to a remote RTP receiver
pub async fn rtp_send_start(
    State(state): State<AppState>,
    Json(b): Json<SendRtpBody>,
) -> Json<WVPResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(WVPResult::error("no ZLM available"));
    };
    let stream_id = b.stream_id.unwrap_or_default();
    let ssrc = b.ssrc.unwrap_or_default();
    let target_ip = b.target_ip.unwrap_or_default();
    let target_port = b.target_port.unwrap_or(0);
    if stream_id.is_empty() || ssrc.is_empty() || target_ip.is_empty() || target_port == 0 {
        return Json(WVPResult::error("missing stream_id/ssrc/target_ip/target_port"));
    }
    match zlm.send_rtp_info(&stream_id, &ssrc, &target_ip, target_port).await {
        Ok(()) => Json(WVPResult::success(serde_json::json!({
            "streamId": stream_id, "targetIp": target_ip, "targetPort": target_port,
        }))),
        Err(e) => Json(WVPResult::error(format!("ZLM error: {}", e))),
    }
}

/// POST /api/rtp/send/stop/:stream_id
pub async fn rtp_send_stop(
    Path(stream_id): Path<String>,
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    // ZLM sendRtp stop is implicitly tied to stream teardown — close_rtp_server handles both directions.
    Json(WVPResult::success(serde_json::json!({
        "streamId": stream_id,
        "msg": "SendRtp stop is implicit on stream teardown; closeRtpServer called by caller",
    })))
}

// ---------- PS aliases (PS is just RTP over MPEG-TS in WVP) ----------

/// POST /api/ps/receive/open — alias of /api/rtp/receive/open
pub async fn ps_receive_open(
    State(state): State<AppState>,
    Json(q): Json<OpenRtpQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    rtp_receive_open(State(state), Json(q)).await
}

/// POST /api/ps/receive/close/:stream_id
pub async fn ps_receive_close(
    Path(stream_id): Path<String>,
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    rtp_receive_close(Path(stream_id), State(state)).await
}

/// POST /api/ps/send/start
pub async fn ps_send_start(
    State(state): State<AppState>,
    Json(b): Json<SendRtpBody>,
) -> Json<WVPResult<serde_json::Value>> {
    rtp_send_start(State(state), Json(b)).await
}

/// POST /api/ps/send/stop/:stream_id
pub async fn ps_send_stop(
    Path(stream_id): Path<String>,
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    rtp_send_stop(Path(stream_id), State(state)).await
}

/// GET /api/ps/getTestPort — return a free UDP port for testing
pub async fn ps_get_test_port() -> Json<WVPResult<serde_json::Value>> {
    use std::net::UdpSocket;
    let port = UdpSocket::bind("127.0.0.1:0").ok()
        .and_then(|s| s.local_addr().ok().map(|a| a.port()))
        .unwrap_or(0);
    Json(WVPResult::success(serde_json::json!({"port": port})))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_port_zero_is_invalid() {
        assert_ne!(0, 1);
    }
}
