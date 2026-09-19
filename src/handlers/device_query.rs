//! 设备查询 HTTP API Handler
//!
//! Phase 1 核心功能：提供设备信息、状态、配置查询 API
//! 这些 API 通过 SIP MESSAGE 与设备通信，获取实时信息

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::response::WVPResult;
use crate::sip::gb28181::device_query::{DeviceInfoResponse, DeviceStatusResponse};

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct DeviceQueryParams {
    /// 设备ID
    #[serde(alias = "deviceId")]
    pub device_id: String,
    /// 超时秒数（默认10）
    #[serde(default = "default_timeout")]
    #[serde(alias = "timeoutSecs")]
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
                            .send_device_info_query(&device_id, sn)
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
                            .send_device_status_query(&device_id, sn)
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
    // 与查询参数版（`device_control::device_config_query`）共用同一份实现，
    // 避免"同一功能两处实现、且一处只发不等"的重复。
    Json(WVPResult::success(
        crate::handlers::device_control::query_config_and_wait(&state, &device_id, &config_type)
            .await,
    ))
    .into_response()
}

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
/// 获取通道快照 —— 返回一个**同源**的图片 URL（`/api/play/snap.jpg/...`）。
///
/// 为什么不直接把 ZLM 的图片地址给前端：
/// * ZLM 这个版本的 `getSnap` 返回裸 JPEG 字节，**不给文件名/path**，
///   后端拼不出 `http://zlm/snap/xxx.jpeg` 这种地址；
/// * 即使拼得出来，也会把媒体节点暴露成浏览器的直连目标，遇到反代/防火墙
///   就取不到图。
///
/// 所以改为后端代理：本接口先确认流是否存在（GB28181 设备按需推流，没在
/// 拉流时抓图必然失败），再把 URL 交回前端；真正的字节由
/// [`get_snap_image`] 从 ZLM 取回转给浏览器。
pub async fn get_snap(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(ref zlm_client) = state.zlm_client else {
        return Json(WVPResult::<()>::error("ZLM not configured")).into_response();
    };

    let stream_id = format!("{}_{}", device_id, channel_id);
    // 与 /api/play/start 一致：ZLM 的 RTP server 把国标流建在 app = "rtp" 下。
    let app = "rtp";

    // 流不存在时不要硬等 getSnap 超时（默认 10s），直接给前端一个明确原因，
    // 让缩略图列立即回落到占位图标。
    match zlm_client
        .is_media_exist("rtsp", "__defaultVhost__", app, &stream_id)
        .await
    {
        Ok(false) => {
            return Json(WVPResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "streamId": stream_id,
                "snapUrl": null,
                "error": "该通道当前没有活跃的流（国标设备按需推流，需先播放一次）",
            })))
            .into_response();
        }
        Err(e) => {
            tracing::warn!("查询 ZLM 流是否存在失败（按已存在处理）: {}", e);
        }
        Ok(true) => {}
    }

    // 把调用方的 JWT 透传进图片 URL —— 浏览器 `<img>` 无法设置请求头，
    // 图片端点靠 `?token=` 鉴权（与 /api/talk/audio 同一套模式）。
    let token = crate::auth::extract_token_from_headers(&headers).unwrap_or_default();
    let snap_url = if token.is_empty() {
        format!("/api/play/snap.jpg/{}/{}", device_id, channel_id)
    } else {
        format!(
            "/api/play/snap.jpg/{}/{}?token={}",
            device_id, channel_id, token
        )
    };

    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "streamId": stream_id,
        "app": app,
        "snapUrl": snap_url,
    })))
    .into_response()
}

/// GET /api/play/snap.jpg/{device_id}/{channel_id}?token=<jwt>
///
/// 从 ZLM 捞一帧 JPEG 转发给浏览器。**注册在 `api_protected` 之外** ——
/// 浏览器 `<img src>` 不能带 `access-token` 头，所以鉴权走 `?token=`
/// （与 `/api/talk/audio/:device_id/:channel_id` 完全同一套模式）。
pub async fn get_snap_image(
    State(state): State<AppState>,
    Query(q): Query<SnapImageQuery>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Response {
    // ---- 鉴权：?token= 或 Authorization: Bearer ----
    let Some(ref token) = q.token else {
        return snap_error(StatusCode::UNAUTHORIZED, "缺少 JWT（请用 ?token=）");
    };
    if let Err(e) = crate::ws::verify_ws_jwt(token, &state.config.jwt.secret) {
        return snap_error(StatusCode::UNAUTHORIZED, &format!("鉴权失败: {}", e));
    }

    let Some(ref zlm_client) = state.zlm_client else {
        return snap_error(StatusCode::SERVICE_UNAVAILABLE, "ZLM not configured");
    };

    let host = zlm_client.ip.as_str();
    let stream_id = format!("{}_{}", device_id, channel_id);
    let app = "rtp";
    let (rtsp_port, _) = media_server_ports(&state, host).await;
    // 用 RTSP 回环地址让 ZLM 从自己内部取流（不经过外部网络）。
    // 此前写成 `rtsp://{ip}:{http_port}/live/{stream}` —— 端口和 app 都是错的。
    let rtsp_url = format!("rtsp://{}:{}/{}/{}", host, rtsp_port, app, stream_id);

    match zlm_client.get_snap(&rtsp_url, Some(10.0)).await {
        Ok(bytes) => {
            let mut resp = Response::new(Body::from(bytes));
            resp.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
            // 浏览器/中间层别缓存：流是活的，每次拿到的帧都不同。
            resp.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store, max-age=0"),
            );
            resp
        }
        Err(e) => {
            tracing::warn!("Snap image failed for {}: {}", stream_id, e);
            snap_error(StatusCode::NOT_FOUND, &format!("抓图失败: {}", e))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SnapImageQuery {
    pub token: Option<String>,
}

fn snap_error(status: StatusCode, msg: &str) -> Response {
    let body = serde_json::json!({ "code": -1, "msg": msg, "data": null }).to_string();
    let mut resp = Response::new(Body::from(body));
    *resp.status_mut() = status;
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    resp
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
    let device_id = params
        .get("deviceId")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let channel_id = params
        .get("channelId")
        .and_then(|v| v.as_str())
        .unwrap_or(device_id);
    let protocol = params
        .get("protocol")
        .and_then(|v| v.as_str())
        .unwrap_or("rtsp");

    let Some(ref zlm_client) = state.zlm_client else {
        return Json(WVPResult::<()>::error("ZLM not configured")).into_response();
    };

    let host = zlm_client.ip.as_str();
    let http_port = zlm_client.http_port;
    // 流 ID 与 /api/play/start 保持一致：`{deviceId}_{channelId}`，
    // 且 ZLM 的 RTP server 把流建在 **app = "rtp"** 下。
    let stream_id = format!("{}_{}", device_id, channel_id);
    let app = "rtp";

    // RTSP / RTMP 端口优先取该媒体服务器在库里的配置，取不到再用协议默认值。
    // 此前 RTSP URL 直接用了 **http_port**（8080），生成的是连不上的地址。
    let (rtsp_port, rtmp_port) = media_server_ports(&state, host).await;

    // 诚实性：这个接口只是"给出某个通道的播放地址"，它**不会**去拉起流。
    // 若流尚未建立（没人调用 /api/play/start），返回的地址其实是播不出来的 ——
    // 因此这里先向 ZLM 确认该流是否存在，不存在就明确报错并指路，
    // 而不是给前端一个永远转圈的 URL。
    match zlm_client
        .is_media_exist("rtsp", "__defaultVhost__", app, &stream_id)
        .await
    {
        Ok(false) => {
            return Json(WVPResult::<()>::error(format!(
                "流 {} 尚未建立：请先调用 /api/play/start/{}/{} 拉起实时流，再取播放地址",
                stream_id, device_id, channel_id
            )))
            .into_response();
        }
        Err(e) => {
            tracing::warn!("查询 ZLM 流是否存在失败（按已存在处理）: {}", e);
        }
        Ok(true) => {}
    }

    let url = match protocol {
        "rtsp" => format!("rtsp://{}:{}/{}/{}", host, rtsp_port, app, stream_id),
        "rtmp" => format!("rtmp://{}:{}/{}/{}", host, rtmp_port, app, stream_id),
        "hls" => format!(
            "http://{}:{}/{}/{}/hls.m3u8",
            host, http_port, app, stream_id
        ),
        "flv" => crate::zlm::address_builder::http_flv_url(&host, http_port, &app, &stream_id),
        "ws_flv" => crate::zlm::address_builder::ws_flv_url(&host, http_port, &app, &stream_id),
        "webrtc" => format!(
            "webrtc://{}:{}/index/api/webrtc?app={}&stream={}&type=play",
            host, http_port, app, stream_id
        ),
        other => {
            return Json(WVPResult::<()>::error(format!(
                "不支持的 protocol: {}（可选 rtsp/rtmp/hls/flv/ws_flv/webrtc）",
                other
            )))
            .into_response();
        }
    };

    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "streamId": stream_id,
        "app": app,
        "url": url,
        "protocol": protocol,
        "rtspPort": rtsp_port,
        "rtmpPort": rtmp_port,
        "httpPort": http_port,
    })))
    .into_response()
}

/// 取某台媒体服务器的 RTSP / RTMP 端口：按 IP 在库中匹配，取不到用协议默认值。
async fn media_server_ports(state: &AppState, host: &str) -> (u16, u16) {
    const DEFAULT_RTSP: u16 = 554;
    const DEFAULT_RTMP: u16 = 1935;
    match crate::db::media_server::list_media_servers(&state.pool).await {
        Ok(list) => {
            if let Some(row) = list.into_iter().find(|m| m.ip.as_deref() == Some(host)) {
                let rtsp = row
                    .rtsp_port
                    .and_then(|p| u16::try_from(p).ok())
                    .unwrap_or(DEFAULT_RTSP);
                let rtmp = row
                    .rtmp_port
                    .and_then(|p| u16::try_from(p).ok())
                    .unwrap_or(DEFAULT_RTMP);
                return (rtsp, rtmp);
            }
        }
        Err(e) => tracing::warn!("读取媒体服务器端口配置失败: {}", e),
    }
    (DEFAULT_RTSP, DEFAULT_RTMP)
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
// ============================================================================
// WVP `DeviceQuery.java` 兼容入口
//
// WVP 用的是混合风格（部分路径参数、部分查询参数），与本平台早期的
// `/api/device/query/info/{id}` 形式不同。这里把 WVP 的路径/参数风格接上，
// 复用同一批真实实现（同样是"注册 pending → 发 SIP → 等应答"）。
// ============================================================================

/// `GET /api/device/query/info?deviceId=` → 同 `device_info`（路径参数版）。
pub async fn device_info_query(
    State(state): State<AppState>,
    Query(q): Query<DeviceIdQuery>,
) -> impl IntoResponse {
    device_info(State(state), Path(q.device_id.unwrap_or_default())).await
}

/// `GET /api/device/query/devices/{deviceId}/status` → 同 `device_status`。
pub async fn device_status_path(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    device_status(State(state), Path(device_id)).await
}

/// `GET /api/device/query/{deviceId}/sync_status` → 同 `device_stub::sync_status`。
pub async fn sync_status_path(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    crate::handlers::device_stub::sync_status(
        State(state),
        Query(crate::handlers::device_stub::SyncStatusQuery {
            device_id: Some(device_id),
        }),
    )
    .await
}

/// `GET /api/device/query/snap/{deviceId}/{channelId}` → 同 `/api/play/snap/...`。
pub async fn snap_path(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> impl IntoResponse {
    get_snap(State(state), headers, Path((device_id, channel_id))).await
}

/// `GET /api/play/ssrc?deviceId=&channelId=` → 同 `/api/play/ssrc/{d}/{c}`。
pub async fn ssrc_query(
    State(state): State<AppState>,
    Query(q): Query<DeviceChannelQuery>,
) -> impl IntoResponse {
    get_ssrc(
        State(state),
        Path((
            q.device_id.unwrap_or_default(),
            q.channel_id.unwrap_or_default(),
        )),
    )
    .await
}

/// `GET /api/play/snap?deviceId=&channelId=` → 同 `/api/play/snap/{d}/{c}`。
pub async fn snap_query(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<DeviceChannelQuery>,
) -> impl IntoResponse {
    get_snap(
        State(state),
        headers,
        Path((
            q.device_id.unwrap_or_default(),
            q.channel_id.unwrap_or_default(),
        )),
    )
    .await
}

#[derive(Debug, Default, Deserialize)]
pub struct DeviceIdQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DeviceChannelQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
}

/// `GET /api/device/query/channel/raw?id=` —— 国标通道编辑时的原始行回显。
///
/// WVP 直接返回 `DeviceChannel` 行；这里返回同源的通道行（含 gb_* 兼容字段）。
pub async fn channel_raw(
    State(state): State<AppState>,
    Query(q): Query<ChannelRawQuery>,
) -> impl IntoResponse {
    let Some(id) = q.id else {
        return Json(WVPResult::<serde_json::Value>::error("缺少 id 参数")).into_response();
    };
    match crate::db::device::get_channel_by_id(&state.pool, id).await {
        Ok(Some(ch)) => Json(WVPResult::success(
            crate::handlers::device_stub::channel_to_json(&ch),
        ))
        .into_response(),
        Ok(None) => Json(WVPResult::<serde_json::Value>::error(format!(
            "通道不存在: {id}"
        )))
        .into_response(),
        Err(e) => {
            tracing::error!("channel/raw 查询失败 id={}: {}", id, e);
            Json(WVPResult::<serde_json::Value>::error(format!(
                "查询通道失败: {e}"
            )))
            .into_response()
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ChannelRawQuery {
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub id: Option<i64>,
}

/// `GET /api/device/query/alarm` —— **向设备查询当前报警**（不是 DB 历史列表）。
///
/// 支持 WVP 的全部过滤条件：报警级别区间 / 报警方式 / 报警类型 / 时间区间。
pub async fn device_alarm_query(
    State(state): State<AppState>,
    Query(q): Query<DeviceAlarmQuery>,
) -> impl IntoResponse {
    let device_id = q.device_id.clone().unwrap_or_default();
    if device_id.is_empty() {
        return Json(WVPResult::<serde_json::Value>::error("deviceId 必须存在")).into_response();
    }
    let Some(ref sip_server) = state.sip_server else {
        return Json(WVPResult::<serde_json::Value>::error("SIP server not available"))
            .into_response();
    };
    let server = &**sip_server;
    if !server.is_device_online(&device_id).await {
        return Json(WVPResult::<serde_json::Value>::error(format!(
            "设备不在线: {device_id}"
        )))
        .into_response();
    }

    let sn = chrono::Utc::now().timestamp_millis() as u32;
    let commander = server.device_commander();
    let (req, rx) = commander.register_alarm_query_with_receiver(&device_id, sn);
    if let Err(e) = server
        .send_alarm_query(
            &device_id,
            q.start_priority.as_deref(),
            q.end_priority.as_deref(),
            q.alarm_method.as_deref(),
            q.alarm_type.as_deref(),
            q.start_time.as_deref(),
            q.end_time.as_deref(),
            sn,
        )
        .await
    {
        tracing::error!("报警查询下发失败 device={}: {}", device_id, e);
        return Json(WVPResult::<serde_json::Value>::error(format!(
            "下发报警查询失败: {e}"
        )))
        .into_response();
    }

    match commander.await_response(req, rx, 15).await {
        Ok(xml) => Json(WVPResult::success(serde_json::json!({
            "deviceId": device_id,
            "sn": sn,
            "xml": xml,
            "alarms": parse_alarm_list(&xml),
            "source": "live",
        })))
        .into_response(),
        Err(_) => Json(WVPResult::<serde_json::Value>::error(
            "设备未在 15 秒内应答报警查询",
        ))
        .into_response(),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct DeviceAlarmQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "startPriority")]
    pub start_priority: Option<String>,
    #[serde(alias = "endPriority")]
    pub end_priority: Option<String>,
    #[serde(alias = "alarmMethod")]
    pub alarm_method: Option<String>,
    #[serde(alias = "alarmType")]
    pub alarm_type: Option<String>,
    #[serde(alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(alias = "endTime")]
    pub end_time: Option<String>,
}

/// 解析设备应答里的 `<AlarmList>` 各 `<Item>`（GB/T 28181 A.2.4.4）。
pub fn parse_alarm_list(xml: &str) -> Vec<serde_json::Value> {
    use crate::sip::gb28181::xml_parser::XmlParser;
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Item>") {
        let after = &rest[start + "<Item>".len()..];
        let Some(end) = after.find("</Item>") else {
            break;
        };
        let item = &after[..end];
        let mut obj = serde_json::Map::new();
        for tag in [
            "DeviceID",
            "AlarmPriority",
            "AlarmMethod",
            "AlarmTime",
            "AlarmDescription",
            "Longitude",
            "Latitude",
        ] {
            if let Some(v) = XmlParser::find_first_element(item, tag) {
                obj.insert(tag.to_string(), serde_json::Value::String(v));
            }
        }
        out.push(serde_json::Value::Object(obj));
        rest = &after[end + "</Item>".len()..];
    }
    out
}

#[cfg(test)]
mod wvp_compat_tests {
    use super::*;

    /// 设备应答里的 AlarmList 必须逐条解析出来（WVP `deviceService.alarm` 的形状）。
    #[test]
    fn parse_alarm_list_extracts_items() {
        let xml = r#"<?xml version="1.0"?>
<Response>
<CmdType>Alarm</CmdType>
<SN>123</SN>
<DeviceID>34020000001320000001</DeviceID>
<AlarmList Num="2">
<Item>
<DeviceID>34020000001320000001</DeviceID>
<AlarmPriority>1</AlarmPriority>
<AlarmMethod>5</AlarmMethod>
<AlarmTime>2026-09-13T07:00:00</AlarmTime>
<AlarmDescription>移动侦测</AlarmDescription>
</Item>
<Item>
<AlarmPriority>2</AlarmPriority>
<AlarmMethod>2</AlarmMethod>
<AlarmTime>2026-09-13T07:01:00</AlarmTime>
</Item>
</AlarmList>
</Response>"#;
        let items = parse_alarm_list(xml);
        assert_eq!(items.len(), 2, "{items:?}");
        assert_eq!(items[0]["AlarmPriority"], "1");
        assert_eq!(items[0]["AlarmMethod"], "5");
        assert_eq!(items[0]["AlarmDescription"], "移动侦测");
        assert_eq!(items[1]["AlarmPriority"], "2");
        // 第二条没写 DeviceID，不应凭空造一个
        assert!(items[1].get("DeviceID").is_none(), "{:?}", items[1]);
    }

    #[test]
    fn parse_alarm_list_empty_is_empty() {
        assert!(parse_alarm_list("<Response></Response>").is_empty());
        assert!(parse_alarm_list("").is_empty());
    }

    /// WVP 的查询参数风格 DTO（camelCase + 数字型 id）都必须能反序列化。
    #[test]
    fn wvp_query_dtos_accept_frontend_shapes() {
        let q: DeviceIdQuery = serde_json::from_value(serde_json::json!({"deviceId": "d"})).unwrap();
        assert_eq!(q.device_id.as_deref(), Some("d"));

        let q: DeviceChannelQuery =
            serde_json::from_value(serde_json::json!({"deviceId": "d", "channelId": "c"})).unwrap();
        assert_eq!(q.device_id.as_deref(), Some("d"));
        assert_eq!(q.channel_id.as_deref(), Some("c"));

        let q: ChannelRawQuery = serde_json::from_value(serde_json::json!({"id": 7})).unwrap();
        assert_eq!(q.id, Some(7));
        let q: ChannelRawQuery = serde_json::from_value(serde_json::json!({"id": "7"})).unwrap();
        assert_eq!(q.id, Some(7));

        let q: DeviceAlarmQuery = serde_json::from_value(serde_json::json!({
            "deviceId": "d", "startPriority": "1", "endPriority": "4",
            "alarmMethod": "5", "alarmType": "1",
            "startTime": "2026-09-13T00:00:00", "endTime": "2026-09-13T23:59:59"
        }))
        .unwrap();
        assert_eq!(q.start_priority.as_deref(), Some("1"));
        assert_eq!(q.alarm_method.as_deref(), Some("5"));
        assert_eq!(q.end_time.as_deref(), Some("2026-09-13T23:59:59"));
    }
}
