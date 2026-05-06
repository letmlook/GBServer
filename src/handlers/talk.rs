use axum::{
    extract::{Path, Query, State},
    Json,
};
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
        let sip = sip_server.write().await;
        sip.send_talk_invite(&device_id, &channel_id).await
    };

    match result {
        Ok(_) => {
            // 生成呼叫ID用于后续跟踪
            let call_id = format!("talk_{}_{}_{}", device_id, channel_id,
                chrono::Utc::now().timestamp_millis());
            
            tracing::info!("[Talk] INVITE 发送成功: call_id={}", call_id);
            
            Ok(Json(WVPResult::success(serde_json::json!({
                "callId": call_id,
                "deviceId": device_id,
                "channelId": channel_id,
                "status": "inviting",
                "msg": "对讲请求已发送，等待设备响应"
            }))))
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
        let sip = sip_server.write().await;
        sip.send_talk_bye(&device_id, &channel_id).await
    };

    match result {
        Ok(_) => {
            tracing::info!("[Talk] BYE 发送成功");
            Ok(Json(WVPResult::<()>::success_empty()))
        }
        Err(e) => {
            tracing::error!("[Talk] BYE 发送失败: {}", e);
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
        let sip = sip_server.write().await;
        sip.send_talk_invite(&device_id, &channel_id).await
    };

    match result {
        Ok(_) => {
            let call_id = format!("invite_{}_{}", device_id, channel_id);
            
            // 获取本地IP用于SDP
            let local_ip = state.config.sip.as_ref()
                .map(|c| c.ip.clone())
                .unwrap_or_else(|| "0.0.0.0".to_string());
            
            tracing::info!("[Talk] 邀请发送成功: call_id={}", call_id);
            
            Ok(Json(WVPResult::success(serde_json::json!({
                "callId": call_id,
                "deviceId": device_id,
                "channelId": channel_id,
                "localIp": local_ip,
                "status": "inviting",
                "sdp": crate::sip::gb28181::build_talk_sdp(&local_ip, 0)
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
    pub call_id: Option<String>,
    pub device_id: Option<String>,
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
        let sip = sip_server.write().await;
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
        let sip = sip_server.read().await;
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
        let sip = sip_server.read().await;
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
