use axum::{extract::State, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct WebRtcOfferRequest {
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(rename = "type")]
    pub offer_type: Option<String>,
    pub sdp: Option<String>,
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
}

pub async fn webrtc_play(
    State(state): State<AppState>,
    Json(req): Json<WebRtcOfferRequest>,
) -> Json<WVPResult<serde_json::Value>> {
    let app = req.app.as_deref().unwrap_or("rtp");
    let stream = req.stream.clone().unwrap_or_else(|| {
        match (&req.device_id, &req.channel_id) {
            (Some(d), Some(c)) => format!("{}_{}", d, c),
            _ => String::new(),
        }
    });

    if let Some(ref zlm_client) = state.zlm_client {
        let mut params = std::collections::HashMap::new();
        params.insert("secret", zlm_client.secret.clone());
        params.insert("app", app.to_string());
        params.insert("stream", stream.clone());
        params.insert("type", "play".to_string());

        if let Some(ref sdp) = req.sdp {
            params.insert("sdp", sdp.clone());
        }

        let url = format!("{}/index/api/webrtc", zlm_client.base_url());

        match reqwest::Client::new()
            .post(&url)
            .json(&params)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => {
                match resp.json::<serde_json::Value>().await {
                    Ok(body) => {
                        let code = body.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
                        if code == 0 {
                            let answer_sdp = body.get("sdp").and_then(|v| v.as_str()).unwrap_or("");
                            return Json(WVPResult::success(serde_json::json!({
                                "sdp": answer_sdp,
                                "type": "answer",
                                "app": app,
                                "stream": stream
                            })));
                        } else {
                            let msg = body.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                            return Json(WVPResult::error(format!("ZLM WebRTC error: {}", msg)));
                        }
                    }
                    Err(e) => {
                        return Json(WVPResult::error(format!("Failed to parse WebRTC response: {}", e)));
                    }
                }
            }
            Err(e) => {
                return Json(WVPResult::error(format!("WebRTC request failed: {}", e)));
            }
        }
    }

    Json(WVPResult::error("ZLM not configured"))
}
