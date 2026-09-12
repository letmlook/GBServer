use axum::{
    extract::ws::{Message, WebSocketUpgrade},
    extract::{Path, Query, RawQuery, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

// ========== Talk 对讲功能 ==========

/// GET /api/talk/start/:device_id/:channel_id — 开始语音对讲
/// 发送 SIP INVITE 信令给设备，建立语音通话
pub async fn talk_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    tracing::info!("[Talk] 开始对讲: device={}, channel={}", device_id, channel_id);

    // 获取 SIP 服务器
    let sip_server = state.sip_server.as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "SIP服务未启动"))?;

    // 发送语音对讲 INVITE
    let result = {
        let sip = &*sip_server;
        sip.send_talk_invite(&device_id, &channel_id).await
    };

    match result {
        Ok(call_id) => {
            // 修正：此前这里又自己拼了一个 `talk_{device}_{channel}_{ts}` 作为 callId
            // 返回给前端，与 TalkManager 里登记的 call_id 不是同一个字符串
            // （登记时用的是 send_talk_invite 内部生成的那个），
            // 前端拿这个 callId 去 /api/talk/status 或停止对讲都会查不到。
            // 现在统一使用 send_talk_invite 返回的真实 call_id。
            tracing::info!("[Talk] INVITE 发送成功: call_id={}", call_id);

            // 必须**等设备 200 OK**（会话变 Active）再返回：
            // 前端拿到 200 后会立刻连 `/api/talk/audio/...`，而那个 WS 只认 Active
            // 会话并校验设备音频地址 —— 立即返回的话握手必然抢在 200 OK 之前，
            // 用户看到的是「开启对讲失败」，随后连 BYE 都发不出去（会话还是 Inviting）。
            let manager = sip_server.talk_manager();
            let wait_ms = 8_000;
            match manager.wait_active(&device_id, &channel_id, wait_ms).await {
                Some(session) => {
                    tracing::info!(
                        "[Talk] 会话已激活: call_id={} device_audio={}:{}",
                        session.call_id,
                        session.device_ip,
                        session.device_port
                    );
                    Ok(Json(WVPResult::success(serde_json::json!({
                        "callId": session.call_id,
                        "deviceId": device_id,
                        "channelId": channel_id,
                        "status": "active",
                        "localPort": session.local_port,
                        "deviceIp": session.device_ip,
                        "devicePort": session.device_port,
                        "msg": "对讲已建立，可以发送音频"
                    }))))
                }
                None => {
                    // 超时：清理掉这个半成品会话（先按不限状态找到它再移除），
                    // 否则它会以 Inviting 状态残留在 TalkManager 里。
                    if let Some(stale) =
                        manager.get_any_by_device_channel(&device_id, &channel_id).await
                    {
                        let _ = manager.remove(&stale.call_id).await;
                    }
                    let _ = {
                        let sip = &*sip_server;
                        sip.send_talk_bye(&device_id, &channel_id).await
                    };
                    tracing::warn!(
                        "[Talk] 等待设备 200 OK 超时（{}ms）: device={} channel={}",
                        wait_ms,
                        device_id,
                        channel_id
                    );
                    Err(AppError::business(
                        ErrorCode::Error500,
                        "对讲失败：设备未在 8 秒内应答（200 OK）",
                    ))
                }
            }
        }
        Err(e) => {
            tracing::error!("[Talk] INVITE 发送失败: {}", e);
            Err(AppError::business(ErrorCode::Error100, format!("对讲请求失败: {}", e)))
        }
    }
}

/// GET /api/talk/stop/:device_id/:channel_id — 停止语音对讲
/// 发送 SIP BYE 信令给设备，结束语音通话
pub async fn talk_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Result<Json<WVPResult<()>>, AppError> {
    tracing::info!("[Talk] 停止对讲: device={}, channel={}", device_id, channel_id);

    // 获取 SIP 服务器
    let sip_server = state.sip_server.as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "SIP服务未启动"))?;

    // 发送语音对讲 BYE
    let result = {
        let sip = &*sip_server;
        sip.send_talk_bye(&device_id, &channel_id).await
    };

    match result {
        Ok(_) => {
            tracing::info!("[Talk] BYE 发送成功");
            Ok(Json(WVPResult::<()>::success_empty()))
        }
        Err(e) => {
            // 对讲可能已经结束（BYE 本身不报错），但**残留会话必须清掉**：
            // 此前这里只记一条日志就返回成功，于是 `Inviting`/未激活的会话
            // 永远留在 TalkManager 里（`cleanup_expired` 只清理 Terminated），
            // 后续同一通道再次对讲会拿到脏会话。
            tracing::warn!("[Talk] BYE 发送失败（仍清理会话）: {}", e);
            if let Some(stale) = sip_server
                .talk_manager()
                .get_any_by_device_channel(&device_id, &channel_id)
                .await
            {
                let _ = sip_server.talk_manager().remove(&stale.call_id).await;
                tracing::info!("[Talk] 已移除残留会话 call_id={}", stale.call_id);
            }
            // 对讲可能已经结束，返回成功以避免前端报错
            Ok(Json(WVPResult::<()>::success_empty()))
        }
    }
}

/// GET /api/talk/invite/:device_id/:channel_id — 发起语音对讲邀请
/// 与 start 类似，但用于明确的邀请流程
pub async fn talk_invite(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    tracing::info!("[Talk] 邀请对讲: device={}, channel={}", device_id, channel_id);

    // 获取 SIP 服务器
    let sip_server = state.sip_server.as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "SIP服务未启动"))?;

    // 发送语音对讲 INVITE
    let result = {
        let sip = &*sip_server;
        sip.send_talk_invite(&device_id, &channel_id).await
    };

    match result {
        Ok(call_id) => {
            // 获取本地IP用于SDP
            let local_ip = state.config.sip.as_ref()
                .map(|c| c.ip.clone())
                .unwrap_or_else(|| "0.0.0.0".to_string());

            // 展示用 SDP 必须用真实分配到的收流端口：此前固定传 0，
            // 回给前端的是一份 m=audio 0（端口 0 = 媒体流被禁用）的无效 SDP。
            // 会话按 call_id 精确查询（此前按 device+channel 模糊查，
            // 同一设备重复发起时会拿到旧会话）。
            let local_port = {
                let sip = &*sip_server;
                sip.talk_manager()
                    .get(&call_id)
                    .await
                    .map(|s| s.local_port)
                    .unwrap_or(0)
            };

            tracing::info!("[Talk] 邀请发送成功: call_id={}", call_id);

            Ok(Json(WVPResult::success(serde_json::json!({
                "callId": call_id,
                "deviceId": device_id,
                "channelId": channel_id,
                "localIp": local_ip,
                "localPort": local_port,
                "status": "inviting",
                "sdp": crate::sip::gb28181::build_talk_sdp(&local_ip, local_port)
            }))))
        }
        Err(e) => {
            tracing::error!("[Talk] 邀请发送失败: {}", e);
            Err(AppError::business(ErrorCode::Error100, format!("对讲邀请失败: {}", e)))
        }
    }
}

/// POST /api/talk/ack — 处理语音对讲 ACK
/// 设备响应 200 OK 后，前端发送 ACK 确认
#[derive(Debug, Deserialize)]
pub struct TalkAckQuery {
    #[serde(alias = "callId")]
    pub call_id: Option<String>,
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
}

pub async fn talk_ack(
    State(_state): State<AppState>,
    Query(q): Query<TalkAckQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let call_id = q.call_id.as_deref()
        .or(q.device_id.as_deref())
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 callId 或 deviceId"))?;
    
    tracing::info!("[Talk] ACK 确认: call_id={}", call_id);
    
    // ACK 主要由 SIP 层处理，这里只做日志记录
    // SIP 服务器收到设备的 200 OK 后会自动发送 ACK
    
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/talk/bye — 结束语音对讲
/// 与 stop 类似，用于明确的结束流程
pub async fn talk_bye(
    State(state): State<AppState>,
    Json(body): Json<TalkAckQuery>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body.device_id.as_deref()
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 deviceId"))?;
    let channel_id = body.channel_id.as_deref()
        .ok_or_else(|| AppError::business(ErrorCode::Error400, "缺少 channelId"))?;
    
    tracing::info!("[Talk] BYE 结束: device={}, channel={}", device_id, channel_id);

    // 获取 SIP 服务器
    let sip_server = state.sip_server.as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "SIP服务未启动"))?;

    // 发送语音对讲 BYE
    let result = {
        let sip = &*sip_server;
        sip.send_talk_bye(device_id, channel_id).await
    };

    match result {
        Ok(_) => {
            tracing::info!("[Talk] BYE 发送成功");
            Ok(Json(WVPResult::<()>::success_empty()))
        }
        Err(e) => {
            tracing::error!("[Talk] BYE 发送失败: {}", e);
            // 仍然返回成功，避免前端报错
            Ok(Json(WVPResult::<()>::success_empty()))
        }
    }
}

/// GET /api/talk/status/:device_id/:channel_id — 查询对讲状态
pub async fn talk_status(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    tracing::debug!("[Talk] 查询状态: device={}, channel={}", device_id, channel_id);

    let sip_server = state.sip_server.as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "SIP服务未启动"))?;

    let talk_session = {
        let sip = &*sip_server;
        sip.talk_manager().get_by_device_channel(&device_id, &channel_id).await
    };

    match talk_session {
        Some(session) => {
            let status_str = match session.status {
                crate::sip::gb28181::talk::TalkStatus::Pending => "pending",
                crate::sip::gb28181::talk::TalkStatus::Inviting => "inviting",
                crate::sip::gb28181::talk::TalkStatus::Ringing => "ringing",
                crate::sip::gb28181::talk::TalkStatus::Active => "active",
                crate::sip::gb28181::talk::TalkStatus::Terminating => "terminating",
                crate::sip::gb28181::talk::TalkStatus::Terminated => "terminated",
            };
            
            Ok(Json(WVPResult::success(serde_json::json!({
                "callId": session.call_id,
                "deviceId": session.device_id,
                "channelId": session.channel_id,
                "status": status_str,
                "localPort": session.local_port,
                "deviceIp": session.device_ip,
                "devicePort": session.device_port,
                "zlmStreamId": session.zlm_stream_id,
                "startTime": session.start_time.to_rfc3339(),
                "lastActivity": session.last_activity.to_rfc3339()
            }))))
        }
        None => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "status": "idle",
                "msg": "当前无活跃的对讲会话"
            }))))
        }
    }
}

/// GET /api/talk/list — 获取所有活跃对讲会话列表
pub async fn talk_list(
    State(state): State<AppState>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    tracing::debug!("[Talk] 获取对讲列表");

    // 获取 SIP 服务器和 TalkManager
    let sip_server = state.sip_server.as_ref()
        .ok_or_else(|| AppError::business(ErrorCode::Error100, "SIP服务未启动"))?;

    let talk_manager = {
        let sip = &*sip_server;
        sip.talk_manager()
    };
    
    let sessions = talk_manager.get_active_sessions().await;

    let list: Vec<serde_json::Value> = sessions.iter().map(|s| {
        let status_str = match s.status {
            crate::sip::gb28181::talk::TalkStatus::Pending => "pending",
            crate::sip::gb28181::talk::TalkStatus::Inviting => "inviting",
            crate::sip::gb28181::talk::TalkStatus::Ringing => "ringing",
            crate::sip::gb28181::talk::TalkStatus::Active => "active",
            crate::sip::gb28181::talk::TalkStatus::Terminating => "terminating",
            crate::sip::gb28181::talk::TalkStatus::Terminated => "terminated",
        };
        
        serde_json::json!({
            "callId": s.call_id,
            "deviceId": s.device_id,
            "channelId": s.channel_id,
            "status": status_str,
            "localPort": s.local_port,
            "deviceIp": s.device_ip,
            "devicePort": s.device_port,
            "startTime": s.start_time.to_rfc3339()
        })
    }).collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "total": list.len(),
        "list": list
    }))))
}

// ============================================================================
// 语音对讲上行音频（浏览器 → 设备）
// ============================================================================

/// 从 query string 里取一个参数（WS 无法自定义 header，因此沿用 `?token=`）。
fn ws_query_param(qstring: &str, key: &str) -> Option<String> {
    qstring
        .split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{}=", key)))
        .map(|s| {
            // 最小化百分号解码：够用即可（token 是 JWT，不含需要转义的字符）
            s.replace("%20", " ").replace("%2F", "/").replace("%2B", "+")
        })
}

/// WebSocket 音频上行：`GET /api/talk/audio/:device_id/:channel_id?token=<jwt>`
///
/// * 鉴权：与 `/api/ws` 一致，走 `?token=`（浏览器无法为 WS 设置自定义头）。
/// * 客户端 → 服务端：**二进制帧** = 8kHz 单声道 i16 **小端** PCM。
///   服务端编码为 G.711A（PCMA/8000，PT=8），按 20ms/160 字节打成 RTP，
///   发到该对讲会话设备 200 OK 里宣告的 `device_ip:device_port`。
/// * 服务端 → 客户端：文本帧，用于回报已发送的包数/字节数（便于前端显示
///   与排障）。设备方向的音频由 ZLM 的 `local_port` 收流后经 ws-flv 播放，
///   不经过本端点。
///
/// 这是此前完全缺失的那条链路：信令与 SDP 协商都通，但没有任何音频通路。
pub async fn talk_audio_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    raw_query: Option<RawQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let qstring = raw_query.and_then(|q| q.0).unwrap_or_default();
    let token = ws_query_param(&qstring, "token").or_else(|| {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(String::from)
    });
    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            "缺少 JWT（请用 ?token= 或 Authorization: Bearer）",
        )
            .into_response();
    };
    if let Err(e) = crate::ws::verify_ws_jwt(&token, &state.config.jwt.secret) {
        return (StatusCode::UNAUTHORIZED, e).into_response();
    }

    // 取会话：必须有已协商好的设备音频地址与 SSRC
    let Some(ref sip) = state.sip_server else {
        return (StatusCode::SERVICE_UNAVAILABLE, "SIP 服务未启动").into_response();
    };
    // 容忍"客户端比设备 200 OK 更早连上来"：在 6 秒内轮询会话状态，
    // 而不是立刻 404。`/api/talk/start` 现在自己会等，但浏览器重连/刷新、
    // 或第三方客户端先连 WS 的场景仍会走到这里。
    let manager = sip.talk_manager();
    let Some(session) = manager.wait_active(&device_id, &channel_id, 6_000).await else {
        let any = manager.get_any_by_device_channel(&device_id, &channel_id).await;
        return (
            StatusCode::NOT_FOUND,
            if any.is_some() {
                "对讲会话尚未激活（设备还没回 200 OK）"
            } else {
                "没有进行中的对讲会话（请先调用 /api/talk/start）"
            },
        )
            .into_response();
    };
    if session.device_port == 0 || session.device_ip.is_empty() {
        return (
            StatusCode::CONFLICT,
            "对讲会话尚未协商出设备音频地址（等设备 200 OK）",
        )
            .into_response();
    }

    let ssrc = session
        .ssrc_u32()
        .unwrap_or_else(|| crate::sip::gb28181::talk_audio::fallback_ssrc(&session.call_id));
    let device_ip = session.device_ip.clone();
    let device_port = session.device_port;
    let call_id = session.call_id.clone();

    let sender = match crate::sip::gb28181::talk_audio::TalkAudioSender::connect(
        &device_ip,
        device_port,
        ssrc,
    )
    .await
    {
        Ok(s) => std::sync::Arc::new(s),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("创建音频发送器失败: {}", e),
            )
                .into_response();
        }
    };
    tracing::info!(
        "对讲音频上行通道建立: call_id={} -> {}:{} ssrc={}",
        call_id,
        device_ip,
        device_port,
        ssrc
    );

    ws.on_upgrade(move |socket| async move {
        let (mut tx, mut rx) = socket.split();
        let mut frames: u64 = 0;
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Binary(data) => {
                    if data.is_empty() {
                        continue;
                    }
                    if data.len() % 2 != 0 {
                        let _ = tx
                            .send(Message::Text(
                                "{\"error\":\"PCM 字节数必须为偶数（i16 小端）\"}".into(),
                            ))
                            .await;
                        continue;
                    }
                    // 小端 i16 PCM
                    let pcm: Vec<i16> = data
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    match sender.send_pcm_8k(&pcm).await {
                        Ok(n) => {
                            frames += 1;
                            if frames % 25 == 0 {
                                // 约每 0.5s 回报一次，避免刷爆 WS
                                let (packets, bytes) = sender.stats();
                                let _ = tx
                                    .send(Message::Text(
                                        format!(
                                            "{{\"callId\":\"{}\",\"packets\":{},\"bytes\":{}}}",
                                            call_id, packets, bytes
                                        ),
                                    ))
                                    .await;
                            }
                            let _ = n;
                        }
                        Err(e) => {
                            tracing::warn!("对讲音频发送失败 call_id={}: {}", call_id, e);
                            let _ = tx
                                .send(Message::Text(
                                    format!("{{\"error\":\"发送失败: {}\"}}", e),
                                ))
                                .await;
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let (packets, bytes) = sender.stats();
        tracing::info!(
            "对讲音频上行通道关闭: call_id={} 共 {} 包 / {} 字节",
            call_id,
            packets,
            bytes
        );
    })
    .into_response()
}

#[cfg(test)]
mod talk_contract_tests {
    use crate::sip::gb28181::talk::{TalkManager, TalkStatus};
    use crate::test_support::app_state;

    /// `wait_active` 应在设备 200 OK（会话转 Active）后立刻返回，
    /// 而不是让调用方（`/api/talk/start`）提前返回 `inviting`。
    #[tokio::test]
    async fn wait_active_returns_session_once_activated() {
        let mgr = TalkManager::new();
        let session = mgr.create("call-1", "dev-1", "ch-1").await;
        assert!(!session.is_active(), "新建会话是 Inviting");

        // 后台 300ms 后模拟设备 200 OK
        let mgr2 = std::sync::Arc::new(mgr);
        let m = mgr2.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            m.update_status("call-1", TalkStatus::Active).await;
        });

        let got = mgr2.wait_active("dev-1", "ch-1", 3_000).await;
        assert!(got.is_some(), "会话激活后应返回");
        assert!(got.unwrap().is_active());

        // 未激活的通道应超时返回 None，且 get_any_by_device_channel 仍能看到它
        let none = mgr2.wait_active("dev-1", "ch-404", 200).await;
        assert!(none.is_none());
        assert!(mgr2.get_any_by_device_channel("dev-1", "ch-404").await.is_none());
    }

    /// 停止对讲时要能按**不限状态**取到会话 —— `send_talk_bye` 用的是
    /// `get_by_device_channel`（只认 Active），`Inviting` 的会话它取不到，
    /// 于是清理会静默失败、会话残留。
    #[tokio::test]
    async fn get_any_by_device_channel_sees_inviting_sessions() {
        let mgr = TalkManager::new();
        mgr.create("call-2", "dev-2", "ch-2").await;
        assert!(
            mgr.get_by_device_channel("dev-2", "ch-2").await.is_none(),
            "Active-only 查询看不到 Inviting 会话"
        );
        let any = mgr.get_any_by_device_channel("dev-2", "ch-2").await;
        assert!(any.is_some(), "不限状态查询必须能看到它");
        assert_eq!(any.unwrap().call_id, "call-2");
    }

    /// 没有 SIP 时 `/api/talk/start` 明确报错（不返回假会话）。
    #[tokio::test]
    async fn talk_start_without_sip_errors() {
        let state = app_state().await;
        let err = super::talk_start(
            axum::extract::State(state.clone()),
            axum::extract::Path(("34020000001320000001".to_string(), "34020000001310000001".to_string())),
        )
        .await
        .expect_err("SIP 未启用应报错");
        assert!(matches!(err, crate::error::AppError::Business(_, _)));
    }
}
