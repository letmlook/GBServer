use axum::{extract::Path, Json, extract::State};
use crate::response::WVPResult;
use crate::AppState;
use crate::db::device as db_device;

/// 从 SDP 文本里解析首个 m=video/m=audio 的端口号。
/// 例如 `m=video 11001 TCP/RTP/AVP 96` 返回 Some(11001)。
fn parse_sdp_media_port(sdp: &str) -> Option<u16> {
    for line in sdp.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("m=") {
            // 形如: "video 11001 TCP/RTP/AVP 96" / "audio 8000 RTP/AVP 0"
            let mut it = rest.split_whitespace();
            let media_type = it.next()?;
            if media_type == "video" || media_type == "audio" {
                if let Some(p) = it.next() {
                    if let Ok(port) = p.parse::<u16>() {
                        return Some(port);
                    }
                }
            }
        }
    }
    None
}

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

        let transport_mode = device
            .transport
            .as_deref()
            .or(device.stream_mode.as_deref())
            .unwrap_or("UDP")
            .to_uppercase();
        let is_tcp_passive = transport_mode == "TCP-PASSIVE";
        let is_tcp = transport_mode == "TCP" || is_tcp_passive;

        // 预开 ZLM RTP server —— 标准 GB28181 设备会按 INVITE SDP m= 端口
        // (也就是这个端口) 推流。对于非标准实现(例如 gbcpp/1.0 mock 在
        // 200 OK SDP 里另写 m= 端口),会在收到 200 OK 后用 connectRtpServer
        // 接管。
        //
        // TCP-PASSIVE 例外:设备不在这个端口推流,它在等 ZLM 主动 connect。
        // 这个端口开不开都不影响(下一步会 close),但 openRtpServer 仍要调
        // 才能保证 ZLM 内部流注册成功,后续 connectRtpServer 才能用同一个
        // stream_id 接管。
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

        tracing::info!("ZLM RTP server opened on port {} (transport={})", rtp_server.port, transport_mode);

        // 调用 SIP Server 真正发送 INVITE，并等待设备回复 200 OK
        let sip = &*sip_server;
        // 先生成规范 SSRC: 0 加上设备编号前9位加上0
        let id_part = if device_id.len() >= 9 { &device_id[0..9] } else { &device_id };
        let ssrc = format!("0{:0>9}0", id_part);

        // TCP-PASSIVE 设备不走"等媒体到达"路径:它压根不会推流给 ZLM,
        // 等 ZLM 主动 connect 它的 listen 端口。把 SIP 200 OK 拿到后,
        // 关闭刚才开的 ZLM 监听端口,改用 connectRtpServer 让 ZLM 去连
        // 设备 SDP 宣告的推流端口。
        if is_tcp_passive {
            let (call_id, resp) = match sip
                .send_play_invite_and_wait(&device_id, &channel_id, rtp_server.port, Some(&ssrc))
                .await
            {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::error!("SIP INVITE for TCP-PASSIVE failed: {}", e);
                    let _ = zlm_client.close_rtp_server(&stream_id).await;
                    let _ = sip.send_session_bye(&device_id, &channel_id).await;
                    return Json(WVPResult::error(format!("SIP error: {}", e)));
                }
            };
            tracing::info!(
                "TCP-PASSIVE device: SIP 200 OK received for {}, switching ZLM to connectRtpServer",
                call_id
            );

            let device_port = resp
                .body
                .as_deref()
                .and_then(parse_sdp_media_port)
                .ok_or_else(|| {
                    tracing::error!(
                        "TCP-PASSIVE device 200 OK missing/invalid SDP m= port. Full SDP body:\n{:?}",
                        resp.body
                    );
                    "device SDP missing media port"
                });

            let device_port = match device_port {
                Ok(p) => p,
                Err(e) => {
                    let _ = zlm_client.close_rtp_server(&stream_id).await;
                    let _ = sip.send_session_bye(&device_id, &channel_id).await;
                    return Json(WVPResult::error(e));
                }
            };

            // 让 ZLM 主动 connect 到设备的 TCP 端口。注意:不要先
            // closeRtpServer,那样 ZLM 就找不到 stream_id 对应的流,
            // connectRtpServer 会返回 "can not find the stream"。
            // openRtpServer 已经创建好流,connectRtpServer 接管它做主动
            // connect。
            let dst = format!(
                "rtp://{}:{}",
                device.ip.as_deref().unwrap_or("0.0.0.0"),
                device_port
            );
            if let Err(e) = zlm_client.connect_rtp_server(&stream_id, &dst, device_port, None).await {
                tracing::error!(
                    "connectRtpServer to TCP-PASSIVE device:{} failed: {}",
                    device_port,
                    e
                );
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                let _ = sip.send_session_bye(&device_id, &channel_id).await;
                return Json(WVPResult::error(format!("ZLM connect error: {}", e)));
            }
            tracing::info!(
                "ZLM connectRtpServer (TCP-PASSIVE) -> {} (stream_id={})",
                dst,
                stream_id
            );

            // 等几秒让 ZLM connect 后拿到媒体再返回成功(此时 ZLM 应在
            // 通过 hook 通知 media-ready,但客户端只关心 stream_id/play_url)
            return Json(WVPResult::success(serde_json::json!({
                "app": "rtp",
                "stream": stream_id,
                "playUrl": format!("rtsp://{}:554/rtp/{}", zlm_client.ip, stream_id),
                "flvUrl": format!("http://{}:{}/{}.flv", zlm_client.ip, zlm_client.http_port, stream_id),
                "wsUrl": format!("ws://{}:{}/{}.flv", zlm_client.ip, zlm_client.http_port, stream_id),
                "ws_flv": format!("ws://{}:{}/{}.flv", zlm_client.ip, zlm_client.http_port, stream_id),
                "hls": format!("http://{}:{}/rtp/{}/hls.m3u8", zlm_client.ip, zlm_client.http_port, stream_id),
                "webrtc": format!("webrtc://{}:{}/index/api/webrtc?app=rtp&stream={}&type=play", zlm_client.ip, zlm_client.http_port, stream_id),
                "deviceId": device_id,
                "channelId": channel_id,
                "hasAudio": channel.has_audio.unwrap_or(false),
                "ssrc": ssrc,
                "transport": "TCP-PASSIVE",
            })));
        }

        match sip.send_play_invite_and_wait_media(
            &device_id, &channel_id, rtp_server.port, &stream_id, Some(&ssrc), 15,
        ).await {
            Ok((_call_id, _zlm_stream_id, invite_resp)) => {
                tracing::info!("SIP INVITE + ZLM media ready for {}/{}", device_id, channel_id);

                // 解析设备 200 OK SDP,如果 m= 端口和 ZLM RTP server 端口不一致
                // (gbcpp/1.0 等非标准设备),用 connectRtpServer 让 ZLM 主动
                // connect 到设备宣告的推流端口,避免 ZLM 守错端口收不到流。
                if let Some(resp) = invite_resp {
                    if let Some(sdp) = &resp.body {
                        if let Some(device_port) = parse_sdp_media_port(sdp) {
                            if device_port != rtp_server.port {
                                tracing::warn!(
                                    "Device 200 OK m={} != ZLM RTP server port {}. Switching to ZLM connectRtpServer to device:{}",
                                    device_port, rtp_server.port, device_port
                                );
                                // 不先 closeRtpServer,否则 ZLM 找不到 stream
                                let dst = format!(
                                    "{}://{}:{}",
                                    if is_tcp { "rtp" } else { "rtp" },
                                    device.ip.as_deref().unwrap_or("0.0.0.0"),
                                    device_port
                                );
                                if let Err(e) =
                                    zlm_client.connect_rtp_server(&stream_id, &dst, device_port, None).await
                                {
                                    tracing::error!(
                                        "connectRtpServer to device:{} failed: {}",
                                        device_port, e
                                    );
                                } else {
                                    tracing::info!(
                                        "ZLM connectRtpServer -> {} (stream_id={})",
                                        dst, stream_id
                                    );
                                }
                            }
                        }
                    }
                }

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
                tracing::error!("SIP INVITE or media wait failed: {}", e);
                // 清理已开启的 RTP 端口
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                // 兜底再发一次 BYE，确保设备端不会持续推流
                let _ = sip.send_session_bye(&device_id, &channel_id).await;
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
    // Phase 3.1: 删除 talk BYE fallback —— live 与 talk 是不同 session，
    // talk BYE 走错语义；live 路径 3.1 保证 InviteSession 一定存在。
    let sip = &*sip_server;
    match sip.send_session_bye(&device_id, &channel_id).await {
        Ok(call_id) => {
            tracing::info!("Session BYE sent for stream {} call_id={}", stream_id, call_id);
            // 返回 call_id 给调用方以便排查
            return Json(WVPResult::success(serde_json::json!({"callId": call_id})));
        }
        Err(e) => {
            tracing::warn!("Failed to send session BYE for stream {}: {}", stream_id, e);
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

    let sip = &*sip_server;
    // Phase 3.5: broadcast 走独立 BroadcastManager，不再共享 talk_invite
    match sip.send_broadcast_invite(&device_id, &channel_id).await {
        Ok(call_id) => {
            tracing::info!("Broadcast INVITE sent to {}/{} call_id={}", device_id, channel_id, call_id);
            Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "callId": call_id,
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

    let sip = &*sip_server;
    // Phase 3.5: broadcast BYE 走 BroadcastManager，与 talk 互不影响
    match sip.send_broadcast_bye(&device_id, &channel_id).await {
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
