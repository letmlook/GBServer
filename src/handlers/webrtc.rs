use axum::{extract::State, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct WebRtcOfferRequest {
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(rename = "type")]
    #[serde(alias = "offerType")]
    pub offer_type: Option<String>,
    pub sdp: Option<String>,
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
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
    if stream.is_empty() {
        return Json(WVPResult::error("缺少 stream（或 deviceId+channelId）"));
    }
    let Some(ref sdp_offer) = req.sdp.clone().filter(|s| !s.trim().is_empty()) else {
        return Json(WVPResult::error("缺少 sdp（WebRTC offer）"));
    };
    // WVP 的语义：`type=play` 拉流、`type=push` 推流；缺省拉流。
    let offer_type = req
        .offer_type
        .as_deref()
        .map(str::trim)
        .filter(|s| matches!(*s, "play" | "push"))
        .unwrap_or("play");

    if let Some(ref zlm_client) = state.zlm_client {
        // **ZLM 只从 URL 查询参数（或表单）里取 app/stream/type，SDP 必须是请求体**。
        //
        // 此前这里把 `{secret, app, stream, type, sdp}` 整体当 **JSON body** POST 过去，
        // ZLM 一律回 `Required parameter missed: "type"` —— 即 `/api/play/webrtc`
        // **从来没有成功过一次**（实测 2026-09-13）。正确调用形态（实测可用，
        // ZLM 会继续做 SDP 校验）：
        //   POST /index/api/webrtc?secret=..&app=..&stream=..&type=play
        //   Content-Type: application/sdp   body: <offer sdp>
        let mut url = match reqwest::Url::parse(&format!("{}/index/api/webrtc", zlm_client.base_url()))
        {
            Ok(u) => u,
            Err(e) => return Json(WVPResult::error(format!("ZLM 地址非法: {e}"))),
        };
        url.query_pairs_mut()
            .append_pair("secret", &zlm_client.secret)
            .append_pair("app", app)
            .append_pair("stream", &stream)
            .append_pair("type", offer_type);

        match reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/sdp")
            .body(sdp_offer.clone())
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => {
                // ZLM 成功时回 JSON：`{"code":0,"sdp":"<answer>","id":"..."}`；
                // 失败时回 `{"code":-xxx,"msg":"..."}`（也可能是纯文本错误）。
                let status = resp.status();
                let raw = resp.text().await.unwrap_or_default();
                match serde_json::from_str::<serde_json::Value>(&raw) {
                    Ok(body) => {
                        let code = body.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
                        if code == 0 {
                            let answer_sdp =
                                body.get("sdp").and_then(|v| v.as_str()).unwrap_or("");
                            if answer_sdp.is_empty() {
                                return Json(WVPResult::error(
                                    "ZLM 返回成功但没有 answer SDP",
                                ));
                            }
                            return Json(WVPResult::success(serde_json::json!({
                                "sdp": answer_sdp,
                                "type": "answer",
                                "app": app,
                                "stream": stream,
                                // ZLM 的 webrtc 会话 id，前端断开时可用（delete_webrtc）
                                "id": body.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                            })));
                        }
                        let msg = body
                            .get("msg")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");
                        return Json(WVPResult::error(format!("ZLM WebRTC error: {}", msg)));
                    }
                    // 有些 ZLM 版本/接口（whep）直接回 SDP 文本
                    Err(_) if raw.trim_start().starts_with("v=") => {
                        return Json(WVPResult::success(serde_json::json!({
                            "sdp": raw,
                            "type": "answer",
                            "app": app,
                            "stream": stream
                        })));
                    }
                    Err(e) => {
                        return Json(WVPResult::error(format!(
                            "解析 ZLM WebRTC 响应失败（HTTP {status}）: {e}; body={}",
                            raw.chars().take(200).collect::<String>()
                        )));
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
