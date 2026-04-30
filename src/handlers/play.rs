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
) -> Json<WVPResult<()>> {
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
        }
        Err(e) => {
            tracing::warn!("Failed to send session BYE for stream {}: {}, trying talk BYE fallback", stream_id, e);
            let _ = sip.send_talk_bye(&device_id, &channel_id).await;
        }
    }

    Json(WVPResult::<()>::success_empty())
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
