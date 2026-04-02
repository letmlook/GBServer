use axum::{extract::Path, Json, extract::State};
use crate::response::WVPResult;
use crate::AppState;
use crate::db::device as db_device;

pub async fn play_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Play request: device={}, channel={}", device_id, channel_id);

    let device = match db_device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return Json(WVPResult::error("Device not found"));
        }
        Err(e) => {
            tracing::error!("Failed to query device: {}", e);
            return Json(WVPResult::error("Database error"));
        }
    };

    let channel = match db_device::get_channel_by_device_and_channel_id(&state.pool, &device_id, &channel_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Json(WVPResult::error("Channel not found"));
        }
        Err(e) => {
            tracing::error!("Failed to query channel: {}", e);
            return Json(WVPResult::error("Database error"));
        }
    };

    let ip = device.ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());
    let port = device.port.unwrap_or(554) as u16;
    let has_audio = channel.has_audio.unwrap_or(false);

    if let Some(ref zlm_client) = state.zlm_client {
        let rtsp_url = format!("rtsp://{}:{}/{}", ip, port, channel_id);
        
        let request = crate::zlm::AddStreamProxyRequest {
            secret: zlm_client.secret.clone(),
            vhost: "__defaultVhost__".to_string(),
            app: "gb".to_string(),
            stream: format!("{}@{}", device_id, channel_id),
            url: rtsp_url.clone(),
            rtp_type: Some(0),
            timeout_sec: Some(30.0),
            enable_hls: Some(false),
            enable_mp4: Some(false),
            enable_rtsp: Some(true),
            enable_rtmp: Some(true),
            enable_fmp4: Some(true),
        };

        match zlm_client.add_stream_proxy(&request).await {
            Ok(key) => {
                tracing::info!("Stream started: {} -> {}", key, rtsp_url);
                let stream_url = format!("gb/{}@{}", device_id, channel_id);
                let play_url = format!("rtsp://127.0.0.1/live/{}", stream_url);
                let flv_url = format!("http://127.0.0.1/flv/live.app?stream={}", stream_url);
                return Json(WVPResult::success(serde_json::json!({
                    "app": "gb",
                    "stream": key,
                    "playUrl": play_url,
                    "flvUrl": flv_url,
                    "wsUrl": format!("ws://127.0.0.1/live/{}", stream_url),
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "hasAudio": has_audio,
                    "rtspUrl": rtsp_url,
                })));
            }
            Err(e) => {
                tracing::error!("Failed to start stream: {}", e);
                return Json(WVPResult::error(format!("ZLM error: {}", e)));
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "app": "",
        "stream": "",
        "tracks": [],
        "msg": "ZLM not configured or unavailable"
    })))
}

pub async fn play_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Stop play: device={}, channel={}", device_id, channel_id);

    if let Some(ref zlm_client) = state.zlm_client {
        let stream_key = format!("__defaultVhost__/gb/{}@{}", device_id, channel_id);
        match zlm_client.close_streams(Some("rtsp"), Some("gb"), Some(&format!("{}@{}", device_id, channel_id)), true).await {
            Ok(_) => {
                tracing::info!("Stream stopped: {}", stream_key);
                return Json(WVPResult::<()>::success_empty());
            }
            Err(e) => {
                tracing::error!("Failed to stop stream: {}", e);
            }
        }
    }

    Json(WVPResult::<()>::success_empty())
}

pub async fn broadcast_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Broadcast start: device={}, channel={}", device_id, channel_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn broadcast_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Broadcast stop: device={}, channel={}", device_id, channel_id);
    Json(WVPResult::<()>::success_empty())
}
