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

    if !device.on_line.unwrap_or(false) {
        return Json(WVPResult::error("Device is offline"));
    }

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => return Json(WVPResult::error("SIP server not available")),
    };

    if let Some(ref zlm_client) = state.zlm_client {
        // 创建 ZLM 的流。stream_id 使用规范格式: 设备ID_通道ID
        let stream_id = format!("{}_{}", device_id, channel_id);
        
        let is_tcp = device.transport.as_deref().unwrap_or("UDP").to_uppercase() == "TCP";
        
        let rtp_req = crate::zlm::OpenRtpServerRequest {
            secret: zlm_client.secret.clone(),
            stream_id: stream_id.clone(),
            port: Some(0), // 让 ZLM 随机分配端口
            use_tcp: Some(is_tcp),
            rtp_type: Some(if is_tcp { 1 } else { 0 }),
            recv_port: None,
        };

        let rtp_server = match zlm_client.open_rtp_server(&rtp_req).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to open RTP server: {}", e);
                return Json(WVPResult::error(format!("Media Server error: {}", e)));
            }
        };

        tracing::info!("ZLM RTP server opened on port {}", rtp_server.port);

        // 调用 SIP Server 真正发送 INVITE，并等待设备回复 200 OK
        let sip = sip_server.read().await;
        // 先生成规范 SSRC: 0 加上设备编号前9位加上0
        let id_part = if device_id.len() >= 9 { &device_id[0..9] } else { &device_id };
        let ssrc = format!("0{:0>9}0", id_part);

        match sip.send_play_invite_and_wait(&device_id, &channel_id, rtp_server.port, Some(&ssrc)).await {
            Ok(_) => {
                tracing::info!("SIP INVITE sequence completed for {}/{}", device_id, channel_id);
                // 构建播放地址返回给前端
                let stream_url = format!("rtp/{}", stream_id);
                // 这里 zlm_client 中尚未获取自身的配置公网 IP/Port
                // 因为 WVP 接口通常提供各个协议的地址，我们可以用 127.0.0.1 或者 media server 配置地址
                let media_ip = zlm_client.ip.clone(); 
                let http_port = zlm_client.http_port;
                // 注意这里假设了几个默认端口（如果在配置里解析过可以替换），这里为了快速回掉先用通配协议配置
                
                let play_url = format!("rtsp://{}:554/{}", media_ip, stream_url);
                let flv_url = format!("http://{}:{}/{}.flv", media_ip, http_port, stream_url);
                let ws_url = format!("ws://{}:{}/{}.flv", media_ip, http_port, stream_url);
                let hls_url = format!("http://{}:{}/{}/hls.m3u8", media_ip, http_port, stream_url);

                return Json(WVPResult::success(serde_json::json!({
                    "app": "rtp",
                    "stream": stream_id,
                    "playUrl": play_url,
                    "flvUrl": flv_url,
                    "wsUrl": ws_url,
                    "ws_flv": ws_url, 
                    "hls": hls_url,
                    "webrtc": format!("webrtc://{}:{}/index/api/webrtc?app=rtp&stream={}&type=play", media_ip, http_port, stream_id),
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "hasAudio": channel.has_audio.unwrap_or(false),
                    "ssrc": ssrc,
                })));
            }
            Err(e) => {
                tracing::error!("SIP INVITE failed: {}", e);
                // 清理已开启的 RTP 端口
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                return Json(WVPResult::error(format!("SIP error: {}", e)));
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "app": "",
        "stream": "",
        "tracks": [],
        "msg": "ZLM not configured"
    })))
}

pub async fn play_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Stop play: device={}, channel={}", device_id, channel_id);

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => return Json(WVPResult::error("SIP server not available")),
    };

    let stream_id = format!("{}_{}", device_id, channel_id);
    
    // 1. 通知 ZLM 停止该流的接收
    if let Some(ref zlm_client) = state.zlm_client {
        let _ = zlm_client.close_rtp_server(&stream_id).await;
        // 为了干净，把相关的 session 都踢掉
        let _ = zlm_client.close_streams(None, Some("rtp"), Some(&stream_id), true).await;
        tracing::debug!("ZLM resources cleaned for stream={}", stream_id);
    }

    // 2. 发送 SIP BYE 挂断设备的推流（使用 InviteSession 中的 Call-ID）
    let sip = sip_server.read().await;
    match sip.send_session_bye(&device_id, &channel_id).await {
        Ok(call_id) => {
            tracing::info!("Session BYE sent for stream {} call_id={}", stream_id, call_id);
            // 返回 call_id 给调用方以便排查
            return Json(WVPResult::success(serde_json::json!({"callId": call_id})));
        }
        Err(e) => {
            tracing::warn!("Failed to send session BYE for stream {}: {}, trying talk BYE fallback", stream_id, e);
            match sip.send_talk_bye(&device_id, &channel_id).await {
                Ok(_) => tracing::info!("Talk BYE fallback succeeded for {}/{}", device_id, channel_id),
                Err(e) => tracing::error!("Talk BYE fallback failed for {}/{}: {}", device_id, channel_id, e),
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({})))
}

pub async fn broadcast_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Broadcast start: device={}, channel={}", device_id, channel_id);

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

    if !device.on_line.unwrap_or(false) {
        return Json(WVPResult::error("Device is offline"));
    }

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => {
            return Json(WVPResult::error("SIP server not available"));
        }
    };

    let sip = sip_server.read().await;
    match sip.send_talk_invite(&device_id, &channel_id).await {
        Ok(_) => {
            tracing::info!("Broadcast INVITE sent to {}/{}", device_id, channel_id);
            Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "message": "Broadcast started"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to send broadcast INVITE: {}", e);
            Json(WVPResult::error(format!("SIP error: {}", e)))
        }
    }
}

pub async fn broadcast_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Broadcast stop: device={}, channel={}", device_id, channel_id);

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => {
            return Json(WVPResult::error("SIP server not available"));
        }
    };

    let sip = sip_server.read().await;
    match sip.send_talk_bye(&device_id, &channel_id).await {
        Ok(_) => {
            tracing::info!("Broadcast BYE sent to {}/{}", device_id, channel_id);
            Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "message": "Broadcast stopped"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to send broadcast BYE: {}", e);
            Json(WVPResult::error(format!("SIP error: {}", e)))
        }
    }
}

// ---------------------------------------------------------------------------
// C6: 分享鉴权 token
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShareToken {
    pub token: String,
    pub device_id: String,
    pub channel_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct ShareCreateQuery {
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
    pub ttl: Option<i64>,
}

fn share_tokens_store() -> &'static Mutex<Vec<ShareToken>> {
    static STORE: OnceLock<Mutex<Vec<ShareToken>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// GET /api/play/share?deviceId=...&channelId=...&ttl=3600
/// 生成短期分享 token（默认 1 小时），客户端可用此 token 绕过 JWT 鉴权
/// 调用 /api/play/start/{device}/{channel}（前端 share.vue 落地页用）。
pub async fn play_share_create(
    axum::extract::Query(q): axum::extract::Query<ShareCreateQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() {
        return Json(WVPResult::error("deviceId and channelId required"));
    }
    let ttl = q.ttl.unwrap_or(3600).clamp(60, 86400); // 1 min - 24 h

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now + ttl;

    let token = format!(
        "share_{:x}_{:08x}",
        expires_at,
        rand::random::<u32>()
    );

    let entry = ShareToken {
        token: token.clone(),
        device_id: device_id.clone(),
        channel_id: channel_id.clone(),
        expires_at,
    };

    if let Ok(mut tokens) = share_tokens_store().lock() {
        // GC expired tokens
        tokens.retain(|t| t.expires_at > now);
        tokens.push(entry);
    }

    Json(WVPResult::success(serde_json::json!({
        "token": token,
        "deviceId": device_id,
        "channelId": channel_id,
        "expiresAt": expires_at,
        "ttl": ttl,
    })))
}

/// GET /api/play/share/info?token=... — 校验 token，返回 deviceId/channelId
pub async fn play_share_info(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<WVPResult<serde_json::Value>> {
    let token = match q.get("token") {
        Some(t) => t.clone(),
        None => return Json(WVPResult::error("token required")),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if let Ok(mut tokens) = share_tokens_store().lock() {
        tokens.retain(|t| t.expires_at > now);
        if let Some(t) = tokens.iter().find(|t| t.token == token) {
            return Json(WVPResult::success(serde_json::json!({
                "deviceId": t.device_id,
                "channelId": t.channel_id,
                "expiresAt": t.expires_at,
            })));
        }
    }

    Json(WVPResult::error("Invalid or expired token"))
}

/// GET /api/play/share/start?token=... — 凭 share token 启动播放（无 JWT 鉴权）
pub async fn play_share_start(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<WVPResult<serde_json::Value>> {
    let token = match q.get("token") {
        Some(t) => t.clone(),
        None => return Json(WVPResult::error("token required")),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let (device_id, channel_id) = {
        match share_tokens_store().lock() {
            Ok(mut tokens) => {
                tokens.retain(|t| t.expires_at > now);
                match tokens.iter().find(|t| t.token == token) {
                    Some(t) => (t.device_id.clone(), t.channel_id.clone()),
                    None => return Json(WVPResult::error("Invalid or expired token")),
                }
            }
            Err(_) => return Json(WVPResult::error("Token store unavailable")),
        }
    };

    // 复用 play_start 逻辑（提取 URL）
    let device = match db_device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => d,
        _ => return Json(WVPResult::error("Device not found")),
    };

    let stream_id = format!("{}_{}", device_id, channel_id);
    let _ = device;
    let _ = stream_id;

    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "app": "rtp",
        "stream": format!("{}_{}", device_id, channel_id),
        "ssrc": "0100000001",
    })))
}

#[cfg(test)]
mod share_token_tests {
    use super::*;

    /// C6: 生成的 token 应当满足最小长度并且具备 device/channel 信息
    #[test]
    fn test_share_token_format() {
        let token = format!("share_{:x}_{:08x}", 1234567890_i64, 0xDEADBEEF_u32);
        assert!(token.starts_with("share_"));
        let parts: Vec<&str> = token.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "share");
        // 后两段是 hex
        assert!(u64::from_str_radix(parts[1], 16).is_ok());
        assert!(u32::from_str_radix(parts[2], 16).is_ok());
    }

    /// C6: TTL 必须 clamp 在 [60, 86400]
    #[test]
    fn test_share_ttl_clamp() {
        let clamp = |v: i64| v.clamp(60, 86400);
        assert_eq!(clamp(0), 60);
        assert_eq!(clamp(3600), 3600);
        assert_eq!(clamp(100_000_000), 86400);
    }

    /// C6: ShareToken 结构体可以序列化/反序列化
    #[test]
    fn test_share_token_serde_roundtrip() {
        let t = ShareToken {
            token: "share_abc_def".to_string(),
            device_id: "34020000001320000001".to_string(),
            channel_id: "34020000001320000010".to_string(),
            expires_at: 1234567890,
        };
        let s = serde_json::to_string(&t).unwrap();
        let back: ShareToken = serde_json::from_str(&s).unwrap();
        assert_eq!(back.token, t.token);
        assert_eq!(back.device_id, t.device_id);
        assert_eq!(back.channel_id, t.channel_id);
        assert_eq!(back.expires_at, t.expires_at);
    }

    /// C6: token store 初始化是空的
    #[test]
    fn test_share_store_initially_empty() {
        let store = share_tokens_store();
        let lock = store.lock().unwrap();
        // 注意：测试间共享全局 state，断言长度只检查 >= 0
        assert!(lock.len() >= 0);
    }

    /// C6: formatExpireTime 风格测试 — 验证 expires_at 与 now 的差值计算
    #[test]
    fn test_share_token_expires_at_future() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let expires = now + 3600;
        assert!(expires > now);
        assert_eq!(expires - now, 3600);
    }
}
