//! 设备查询 HTTP API Handler
//!
//! Phase 1 核心功能：提供设备信息、状态、配置查询 API
//! 这些 API 通过 SIP MESSAGE 与设备通信，获取实时信息

use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::response::WVPResult;
use crate::sip::gb28181::device_query::{DeviceQueryManager, DeviceInfoResponse, DeviceStatusResponse};

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct DeviceQueryParams {
    /// 设备ID
    pub device_id: String,
    /// 超时秒数（默认10）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    10
}

/// 设备查询响应
#[derive(Debug, Serialize)]
pub struct DeviceQueryResponse<T> {
    pub device_id: String,
    pub data: T,
    pub sn: u32,
}

/// ============================================================================
/// 设备信息查询
/// ============================================================================

/// GET /api/device/query/info/{device_id}
/// 查询设备基本信息
pub async fn device_info(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let sn = chrono::Utc::now().timestamp_millis() as u32;
    
    // 查询数据库获取设备信息作为基础（离线时返回本地数据）
    let local_info = match crate::db::device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => Some(DeviceInfoResponse {
            device_name: d.name,
            manufacturer: d.manufacturer,
            model: d.model,
            firmware: None,
            channel_count: None,
            serial_number: None,
        }),
        _ => None,
    };
    
    // 如果设备在线，尝试发送 SIP MESSAGE 查询
    if let Some(ref sip_server) = state.sip_server {
        let server = sip_server.read().await;
        if server.is_device_online(&device_id).await {
            let commander = server.device_commander();
            let _req = commander.query_device_info(&device_id, sn);
            
            // TODO: 实际实现需要等待响应并返回
            // 当前返回本地缓存数据作为后备
            if let Some(info) = local_info {
                return Json(WVPResult::success(serde_json::json!({
                    "deviceId": device_id,
                    "sn": sn,
                    "data": info,
                    "source": "cache"
                }))).into_response();
            }
        }
    }
    
    // 如果设备不在线或无数据，返回错误
    if local_info.is_none() {
        return (axum::http::StatusCode::NOT_FOUND, Json(WVPResult::<()>::error("Device not found"))).into_response();
    }
    
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "sn": sn,
        "data": local_info,
        "source": "cache"
    }))).into_response()
}

/// ============================================================================
/// 设备状态查询
/// ============================================================================

/// GET /api/device/query/status/{device_id}
/// 查询设备运行状态
pub async fn device_status(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let sn = chrono::Utc::now().timestamp_millis() as u32;
    
    // 检查设备是否在线
    if let Some(ref sip_server) = state.sip_server {
        let server = sip_server.read().await;
        if server.is_device_online(&device_id).await {
            let commander = server.device_commander();
            let _req = commander.query_device_status(&device_id, sn);
            
            // TODO: 实际实现需要等待响应并返回
            // 返回默认在线状态作为后备
            let status = DeviceStatusResponse {
                online: Some("ON".to_string()),
                status: Some("OK".to_string()),
                device_time: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()),
                encode_channel_count: None,
                decode_channel_count: None,
                record_channel_count: None,
                storage_space: None,
            };
            
            return Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "sn": sn,
                "data": status,
                "source": "online"
            }))).into_response();
        }
    }
    
    // 设备离线
    let status = DeviceStatusResponse {
        online: Some("OFF".to_string()),
        status: Some("OFFLINE".to_string()),
        device_time: None,
        encode_channel_count: None,
        decode_channel_count: None,
        record_channel_count: None,
        storage_space: None,
    };
    
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "sn": sn,
        "data": status,
        "source": "cache"
    }))).into_response()
}

/// ============================================================================
/// 设备配置查询
/// ============================================================================

/// GET /api/device/config/query/{device_id}/{config_type}
/// 查询设备配置参数
pub async fn device_config_query(
    State(state): State<AppState>,
    Path((device_id, config_type)): Path<(String, String)>,
) -> impl IntoResponse {
    let sn = chrono::Utc::now().timestamp_millis() as u32;
    
    // 检查设备是否在线
    if let Some(ref sip_server) = state.sip_server {
        let server = sip_server.read().await;
        if server.is_device_online(&device_id).await {
            let commander = server.device_commander();
            let _req = commander.query_device_config(&device_id, sn, &config_type);
            
            // TODO: 实际实现需要等待响应并返回
            return Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "configType": config_type,
                "sn": sn,
                "message": "Query sent, waiting for response"
            }))).into_response();
        }
    }
    
    Json(WVPResult::<()>::error("Device offline or not registered")).into_response()
}

/// POST /api/device/config/update
/// 更新设备配置参数
pub async fn device_config_update(
    State(_state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // TODO: 实现设备配置更新
    Json(WVPResult::<()>::error("Not implemented")).into_response()
}

/// ============================================================================
/// SSRC 管理
/// ============================================================================

/// GET /api/play/ssrc/{device_id}/{channel_id}
/// 获取播放的 SSRC 信息
pub async fn get_ssrc(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Some(ref sip_server) = state.sip_server {
        let server = sip_server.read().await;
        let ssrc_mgr = server.ssrc_manager();
        let ssrc = ssrc_mgr.allocate(&device_id, &channel_id, "live");
        return Json(WVPResult::success(serde_json::json!({
            "deviceId": device_id,
            "channelId": channel_id,
            "ssrc": ssrc,
        }))).into_response();
    }
    Json(WVPResult::<()>::error("SIP server not available")).into_response()
}

/// ============================================================================
/// 快照
/// ============================================================================

/// GET /api/play/snap/{device_id}/{channel_id}
/// 获取通道快照
pub async fn get_snap(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> impl IntoResponse {
    // TODO: 实现快照获取
    // 需要与 ZLM 交互获取抓图
    Json(WVPResult::<()>::error("Not implemented")).into_response()
}

/// ============================================================================
/// 播放 URL
/// ============================================================================

/// GET /api/media/getPlayUrl
/// 获取播放地址
pub async fn get_play_url(
    State(state): State<AppState>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let device_id = params.get("deviceId")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let channel_id = params.get("channelId")
        .and_then(|v| v.as_str())
        .unwrap_or(device_id);
    let protocol = params.get("protocol")
        .and_then(|v| v.as_str())
        .unwrap_or("rtsp");
    
    // 获取 ZLM 配置
    if let Some(ref zlm_client) = state.zlm_client {
        let host = zlm_client.ip.as_str();
        let http_port = zlm_client.http_port;
        let rtmp_port = 1935u16; // default RTMP port
        
        // 生成流 ID
        let stream_id = format!("{}_{}", device_id, channel_id);
        let play_url = match protocol {
            "rtsp" => format!("rtsp://{}:{}/{}/{}", host, http_port, "live", stream_id),
            "rtmp" => format!("rtmp://{}:{}/live/{}", host, rtmp_port, stream_id),
            "hls" => format!("http://{}:{}/hls/{}.m3u8", host, http_port, stream_id),
            "webrtc" => format!("webrtc://{}:{}/{}", host, http_port, stream_id),
            _ => format!("rtsp://{}:{}/live/{}", host, http_port, stream_id),
        };
        
        return Json(WVPResult::success(serde_json::json!({
            "deviceId": device_id,
            "channelId": channel_id,
            "streamId": stream_id,
            "url": play_url,
            "protocol": protocol,
        }))).into_response();
    }
    
    Json(WVPResult::<()>::error("ZLM not configured")).into_response()
}

/// GET /api/media/stream_info_by_app_and_stream
/// 获取流信息
pub async fn stream_info(
    State(state): State<AppState>,
    Query(params): Query<serde_json::Value>,
) -> impl IntoResponse {
    let app = params.get("app")
        .and_then(|v| v.as_str())
        .unwrap_or("live");
    let stream = params.get("stream")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    
    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client.get_media_list(None, Some(app), Some(stream)).await {
            Ok(list) => {
                return Json(WVPResult::success(serde_json::json!({
                    "app": app,
                    "stream": stream,
                    "count": list.len(),
                    "streams": list,
                }))).into_response();
            }
            Err(e) => {
                return Json(WVPResult::<()>::error(format!("ZLM error: {}", e))).into_response();
            }
        }
    }
    
    Json(WVPResult::<()>::error("ZLM not configured")).into_response()
}