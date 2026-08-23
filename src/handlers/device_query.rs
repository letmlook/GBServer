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

use crate::AppState;
use crate::response::WVPResult;
use crate::sip::gb28181::device_query::{DeviceInfoResponse, DeviceStatusResponse};

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

    // 设备在线：注册 + 发送 SIP MESSAGE + 等待响应（带 15s 超时）
    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if server.is_device_online(&device_id).await {
            let commander = server.device_commander();
            let server = &*server;
            return match commander
                .query_device_info_and_parse(
                    &device_id,
                    sn,
                    async {
                        server
                            .send_device_info_query(&device_id)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    15,
                )
                .await
            {
                crate::sip::gb28181::device_commander::DeviceInfoResult::Ok(info) => {
                    Json(WVPResult::success(serde_json::json!({
                        "deviceId": device_id,
                        "sn": sn,
                        "data": info,
                        "source": "live",
                    })))
                    .into_response()
                }
                crate::sip::gb28181::device_commander::DeviceInfoResult::ParseError(msg) => {
                    Json(WVPResult::success(serde_json::json!({
                        "deviceId": device_id,
                        "sn": sn,
                        "status": "timeout_or_error",
                        "message": msg,
                        "source": "live",
                    })))
                    .into_response()
                }
            };
        }
    }

    // 设备离线或未注册，返回数据库缓存数据
    match crate::db::device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => {
            let info = DeviceInfoResponse {
                device_name: d.name,
                manufacturer: d.manufacturer,
                model: d.model,
                firmware: None,
                channel_count: None,
                serial_number: None,
            };
            Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "sn": sn,
                "data": info,
                "source": "cache",
            })))
            .into_response()
        }
        _ => (
            axum::http::StatusCode::NOT_FOUND,
            Json(WVPResult::<()>::error("Device not found")),
        )
            .into_response(),
    }
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

    // 设备在线：注册 + 发送 SIP MESSAGE + 等待响应（带 15s 超时）
    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if server.is_device_online(&device_id).await {
            let commander = server.device_commander();
            let server = &*server;
            return match commander
                .query_device_status_and_parse(
                    &device_id,
                    sn,
                    async {
                        server
                            .send_device_status_query(&device_id)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    15,
                )
                .await
            {
                crate::sip::gb28181::device_commander::DeviceStatusResult::Ok(status) => {
                    Json(WVPResult::success(serde_json::json!({
                        "deviceId": device_id,
                        "sn": sn,
                        "data": status,
                        "source": "live",
                    })))
                    .into_response()
                }
                crate::sip::gb28181::device_commander::DeviceStatusResult::ParseError(msg) => {
                    Json(WVPResult::success(serde_json::json!({
                        "deviceId": device_id,
                        "sn": sn,
                        "status": "timeout_or_error",
                        "message": msg,
                        "source": "live",
                    })))
                    .into_response()
                }
            };
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
        "source": "cache",
    })))
    .into_response()
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
        let server = &*sip_server;
        if server.is_device_online(&device_id).await {
            let commander = server.device_commander();
            let (_req, rx) =
                commander.register_device_config_with_receiver(&device_id, sn);
            // 启动发送（不阻塞等待）
            let server_send = server.clone();
            let device_id_send = device_id.clone();
            let config_type_send = config_type.clone();
            let send_task = tokio::spawn(async move {
                server_send
                    .send_device_config_query(&device_id_send, &config_type_send)
                    .await
            });
            // 等待响应（带 15s 超时）
            return match commander.await_response(_req, rx, 15).await {
                Ok(xml) => {
                    // 不强解析 ConfigDownload 结构（多种配置类型结构差异大），把原始 XML 透传
                    Json(WVPResult::success(serde_json::json!({
                        "deviceId": device_id,
                        "configType": config_type,
                        "sn": sn,
                        "xml": xml,
                        "source": "live",
                    })))
                    .into_response()
                }
                Err(_) => {
                    let _ = send_task.await;
                    Json(WVPResult::success(serde_json::json!({
                        "deviceId": device_id,
                        "configType": config_type,
                        "sn": sn,
                        "status": "timeout",
                        "message": "Device did not respond within 15s",
                        "source": "live",
                    })))
                    .into_response()
                }
            };
        }
    }

    Json(WVPResult::<()>::error("Device offline or not registered")).into_response()
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
        let server = &*sip_server;
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
    // 获取 ZLM 客户端
    if let Some(ref zlm_client) = state.zlm_client {
        // 构建 RTSP URL
        let host = zlm_client.ip.as_str();
        let port = zlm_client.http_port;
        let stream_id = format!("{}_{}", device_id, channel_id);
        let rtsp_url = format!("rtsp://{}:{}/live/{}", host, port, stream_id);
        
        // 调用 ZLM 抓图
        match zlm_client.get_snap(&rtsp_url, Some(10.0), None).await {
            Ok(snap_path) => {
                // 返回相对路径，前端可以拼接完整 URL
                let snap_url = format!("/static/snap/{}", snap_path.split('/').last().unwrap_or(&snap_path));
                Json(WVPResult::success(serde_json::json!({
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "streamId": stream_id,
                    "snapUrl": snap_url,
                    "path": snap_path,
                }))).into_response()
            }
            Err(e) => {
                tracing::warn!("Snap failed for {}/{}: {}", device_id, channel_id, e);
                Json(WVPResult::success(serde_json::json!({
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "streamId": stream_id,
                    "error": format!("{}", e),
                    "snapUrl": null,
                }))).into_response()
            }
        }
    } else {
        Json(WVPResult::<()>::error("ZLM not configured")).into_response()
    }
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