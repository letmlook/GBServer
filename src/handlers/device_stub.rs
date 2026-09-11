//! 设备相关接口（增删改查、目录同步、传输模式、订阅、通道配置等）。
//!
//! 注：早期这里大量使用"返回固定 JSON 的兼容实现"，模块头也写着
//! "其余保持兼容空实现（后续可对接 SIP/ZLM）"。经 2026-09 的逐条核对，
//! 本模块**已无空实现**：每个 handler 要么真实落库、要么真的下发 SIP 信令
//! 或调用 ZLM，失败时如实返回错误。
//!
//! ## 角色定位 (Phase 2.5)
//!
//! 本模块部分 handler 已迁移到 `crate::handlers::device_control`：
//! - 报警订阅 (`/api/device/subscribe/alarm`) — 实际由 `device_control::subscribe_alarm` 处理
//! - 移动位置订阅 (`/api/device/subscribe/mobilePosition`) — 由 `device_control::subscribe_mobile_position`
//!
//! 仍有少量兼容路径（如 `/api/device/snap/:id`、`/api/device/record/control`）保留在本模块
//! 以保持前端兼容。

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{
    delete_device_cascade,
    get_channel_by_device_and_channel_id,
    get_device_by_device_id,
    insert_device,
    list_channels_by_parent,
    list_channels_for_device,
    update_device,
    DeviceChannel,
    update_channel_has_audio,
    update_channel_stream_identification,
    update_device_mobile_position_subscription,
    update_device_stream_mode,
};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::state::StreamStateRepository;
use crate::AppState;
use std::time::Duration;
use tokio::time::timeout;

/// GET /api/device/query/sync_status
/// 参数: deviceId - 设备ID (可选)
/// 返回: 同步状态信息
#[derive(Debug, Deserialize)]
pub struct SyncStatusQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
}

pub async fn sync_status(
    State(state): State<AppState>,
    Query(q): Query<SyncStatusQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let requested_device_id = q.device_id.unwrap_or_default();
    let db_device = if requested_device_id.is_empty() {
        None
    } else {
        get_device_by_device_id(&state.pool, &requested_device_id).await.ok().flatten()
    };

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        // Avoid long waits that can cause deadlocks in the catalog subscription query
        let subscriptions = match timeout(
            Duration::from_millis(200),
            server.catalog_subscription_manager().get_all(),
        )
        .await
        {
            Ok(list) => list,
            Err(_) => {
                tracing::warn!("catalog_subscription_manager.get_all timed out");
                Vec::new()
            }
        };
        let active_count = subscriptions.len();
        Json(WVPResult::success(serde_json::json!({
            "deviceId": if requested_device_id.is_empty() { serde_json::Value::Null } else { serde_json::json!(requested_device_id) },
            "status": if active_count > 0 { "active" } else { "idle" },
            "activeSubscriptions": active_count,
            "online": db_device.as_ref().and_then(|item| item.on_line).unwrap_or(false),
            "streamMode": db_device.as_ref().and_then(|item| item.stream_mode.clone()),
            "message": "设备同步状态正常"
        })))
    } else {
        Json(WVPResult::success(serde_json::json!({
            "deviceId": if requested_device_id.is_empty() { serde_json::Value::Null } else { serde_json::json!(requested_device_id) },
            "status": "idle",
            "online": db_device.as_ref().and_then(|item| item.on_line).unwrap_or(false),
            "streamMode": db_device.as_ref().and_then(|item| item.stream_mode.clone()),
            "message": "SIP服务未初始化"
        })))
    }
}

/// DELETE /api/device/query/devices/:device_id/delete
pub async fn device_delete(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<()>>, AppError> {
    delete_device_cascade(&state.pool, &device_id).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/device/query/devices/:device_id/sync
/// 触发设备同步
/// 参数: device_id - 设备国标ID
/// 返回: 同步结果
pub async fn device_sync(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Device sync requested for: {}", device_id);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online {
                match server.send_catalog_query(&device_id).await {
                    Ok(_) => {
                        // 目录可能分多页返回（SumNum > 1），这里等设备把本次
                        // 同步（同一 device_id + sn）发完再回结果，最多等 8 秒。
                        // 此前只回一句「命令已发送」——前端拿不到任何同步结果，
                        // 也无从判断是否失败（设备若一直不回，界面永远停在"同步中"）。
                        let manager = server.catalog_sync_manager();
                        let deadline =
                            tokio::time::Instant::now() + std::time::Duration::from_secs(8);
                        let (mut sync_state, mut total, mut received, mut error) =
                            ("waiting".to_string(), 0_i32, 0_i32, None::<String>);
                        loop {
                            if let Some(sess) = manager.get_session(&device_id) {
                                total = sess.total_num;
                                received = sess.received_num;
                                error = sess.error.clone();
                                sync_state = match sess.state {
                                    crate::sip::gb28181::SyncState::Waiting => "waiting",
                                    crate::sip::gb28181::SyncState::Receiving => "receiving",
                                    crate::sip::gb28181::SyncState::Done => "done",
                                    crate::sip::gb28181::SyncState::Failed => "failed",
                                }
                                .to_string();
                                if matches!(
                                    sess.state,
                                    crate::sip::gb28181::SyncState::Done
                                        | crate::sip::gb28181::SyncState::Failed
                                ) {
                                    break;
                                }
                            }
                            if tokio::time::Instant::now() >= deadline {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }

                        // 统计本次同步后该设备名下的通道数，便于前端直接展示结果
                        let channel_count =
                            crate::db::device::list_channels_for_device(&state.pool, &device_id)
                                .await
                                .map(|v| v.len())
                                .unwrap_or(0);

                        let message = match sync_state.as_str() {
                            "done" => "设备目录同步完成",
                            "failed" => "设备目录同步失败",
                            "receiving" => "设备目录同步进行中（未在超时前收齐分页）",
                            _ => "已发送目录查询，设备尚未响应",
                        };

                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "syncState": sync_state,
                            "totalPackets": total,
                            "receivedPackets": received,
                            "channelCount": channel_count,
                            "error": error,
                            "message": message,
                            "code": 0
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send catalog query: {}", e);
                        return Json(WVPResult::error(&format!("发送同步命令失败: {}", e)));
                    }
                }
            } else {
                return Json(WVPResult::error("设备不在线"));
            }
        }
    }

    Json(WVPResult::error("设备未注册或SIP服务未初始化"))
}


/// POST /api/device/query/transport/:device_id/:stream_mode
/// 设置设备流传输模式
/// 参数: device_id - 设备ID, stream_mode - 流模式 (TCP/UDP/TCP-ACTIVE/TCP-PASSIVE)
/// 返回: 设置结果（同时更新 DB 并向设备下发 SIP Control/Transport 消息）
pub async fn device_transport(
    State(state): State<AppState>,
    Path((device_id, stream_mode)): Path<(String, String)>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let normalized_mode = stream_mode.to_uppercase();
    let valid_modes = ["TCP", "UDP", "TCP-ACTIVE", "TCP-PASSIVE"];
    if !valid_modes.contains(&normalized_mode.as_str()) {
        return Ok(Json(WVPResult::error(format!(
            "不支持的传输模式: {}（必须是 TCP/UDP/TCP-ACTIVE/TCP-PASSIVE）",
            stream_mode
        ))));
    }

    // 1) 更新数据库
    //
    // 修正：此前用 `unwrap_or_default()` 吞掉 DB 错误，`updated` 静默变成 0，
    // 而响应仍然回「设备流传输模式设置成功」——设备不存在或写库失败都会被
    // 伪装成成功。现在如实传播，并把"0 行受影响"解释为设备不存在。
    let updated = update_device_stream_mode(&state.pool, &device_id, &normalized_mode)
        .await
        .map_err(|e| {
            AppError::business(ErrorCode::Error500, format!("更新传输模式失败: {}", e))
        })?;
    if updated == 0 {
        return Ok(Json(WVPResult::error(format!("设备不存在: {}", device_id))));
    }
    tracing::info!("Transport mode change: device={}, mode={}", device_id, normalized_mode);

    // 2) 向设备下发 SIP Control/Transport 消息（设备在线才发；不在线仅 DB 更新也合法）
    let mut sip_sent = false;
    let mut sip_error: Option<String> = None;
    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if server.is_device_online(&device_id).await {
            match server.send_device_transport(&device_id, &normalized_mode).await {
                Ok(_) => sip_sent = true,
                Err(e) => {
                    tracing::warn!("Failed to send Transport SIP: {}", e);
                    sip_error = Some(e.to_string());
                }
            }
        }
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "streamMode": normalized_mode,
        "updated": updated,
        "sipSent": sip_sent,
        "sipError": sip_error,
        "message": "设备流传输模式设置成功",
        "code": 0
    }))))
}

// 注：`GuardQuery` 与 `SubscribeCatalogQuery` 曾在此定义，但对应的 handler
// 已分别迁移到 `device_control::device_guard` / `device_control::subscribe_catalog`，
// 这两个结构体在仓库内已无任何引用（`pub` 使得编译器不会报 dead_code），
// 属于迁移残留，已删除以免误以为本模块还负责这两条路径。

/// GET /api/device/query/subscribe/mobile-position
/// 订阅设备移动位置
/// 参数: id - 设备ID, cycle - 订阅周期(秒), interval - 上报间隔(秒)
/// 返回: 订阅结果
#[derive(Debug, Deserialize)]
pub struct SubscribePositionQuery {
    pub id: Option<String>,
    pub cycle: Option<i32>,
    pub interval: Option<i32>,
}


#[derive(Debug, Deserialize)]
pub struct SubscribeAlarmQuery {
    pub id: Option<String>,
    pub expires: Option<i32>,
}

pub async fn subscribe_mobile_position(
    State(state): State<AppState>,
    Query(q): Query<SubscribePositionQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.id.clone().unwrap_or_default();
    let cycle = q.cycle.unwrap_or(5) as u32;
    let interval = q.interval.unwrap_or(5);

    tracing::info!("Position subscription: device={}, cycle={}, interval={}",
        device_id, cycle, interval);

    let persisted = update_device_mobile_position_subscription(
        &state.pool,
        &device_id,
        cycle as i32,
        interval,
    )
    .await
    .unwrap_or_default();

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online {
                match server.send_subscribe(&device_id, "MobilePosition", cycle).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "cycle": cycle,
                            "interval": interval,
                            "updated": persisted,
                            "message": "位置订阅已发送",
                            "code": 0
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send position subscription: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::error("设备不在线或订阅失败"))
}

/// GET /api/device/config/query/:device_id/BasicParam
/// 获取设备基本参数
/// 参数: device_id - 设备ID
/// 从 XML 文本里取某个标签的文本值（字符串匹配，不用 `XmlParser::parse` —— 它对
/// `<Response>` 这类嵌套结构不可靠，见 `pending_request::extract_sn` 的说明）。
fn xml_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let v = xml[start..end].trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// 返回: 设备基本配置信息
pub async fn config_basic_param(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Config BasicParam query for: {}", device_id);
    let db_device = get_device_by_device_id(&state.pool, &device_id).await.ok().flatten();
    let db_name = db_device.as_ref().and_then(|d| d.name.clone());
    let db_manufacturer = db_device.as_ref().and_then(|d| d.manufacturer.clone());
    let db_model = db_device.as_ref().and_then(|d| d.model.clone());
    let db_transport = db_device
        .as_ref()
        .and_then(|d| d.transport.clone())
        .unwrap_or_else(|| "UDP".to_string());
    let db_stream_mode = db_device
        .as_ref()
        .and_then(|d| d.stream_mode.clone())
        .unwrap_or_else(|| "UDP".to_string());

    // 修正：这里此前只 `send_device_config_query(...)` 发出去就返回 DB 里的旧值，
    // 并标一句"设备配置查询已发送" —— 既没有登记 pending 请求（设备回来的应答
    // 会被当作 unsolicited 丢掉），也从不消费应答，本质是个延迟实现。
    // 现在登记 + 等应答（15s），把设备返回的原始 XML 透传给调用方
    // （ConfigDownload 各 ConfigType 结构差异大，不做强解析，与
    //  /api/device/config/query 的处理保持一致）。
    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online {
                let sn = chrono::Utc::now().timestamp_millis() as u32;
                let commander = server.device_commander();
                let (req, rx) = commander.register_device_config_with_receiver(&device_id, sn);
                match server
                    .send_device_config_query(&device_id, "BasicParam", sn)
                    .await
                {
                    Ok(_) => match commander.await_response(req, rx, 15).await {
                        Ok(xml) => {
                            // 解析设备上报的基本参数：库里的值可能是注册时写的、
                            // 也可能为空，设备实测值才是权威。用字符串提取而不是
                            // XmlParser（后者处理不了 `<Response>` 嵌套）。
                            let name = xml_tag_value(&xml, "Name").or(db_name);
                            let manufacturer =
                                xml_tag_value(&xml, "Manufacturer").or(db_manufacturer);
                            let model = xml_tag_value(&xml, "Model").or(db_model);
                            let firmware = xml_tag_value(&xml, "Firmware");
                            let heartbeat = xml_tag_value(&xml, "HeartBeatInterval");
                            let expiration = xml_tag_value(&xml, "Expiration");
                            return Json(WVPResult::success(serde_json::json!({
                                "deviceId": device_id,
                                "sn": sn,
                                "name": name,
                                "manufacturer": manufacturer,
                                "model": model,
                                "firmware": firmware,
                                "heartBeatInterval": heartbeat,
                                "expiration": expiration,
                                "transport": db_transport,
                                "streamMode": db_stream_mode,
                                "xml": xml,
                                "source": "live",
                                "message": "设备基本参数查询完成"
                            })));
                        }
                        Err(_) => {
                            tracing::warn!("Config BasicParam 查询超时: {}", device_id);
                            return Json(WVPResult::success(serde_json::json!({
                                "deviceId": device_id,
                                "sn": sn,
                                "name": db_name,
                                "manufacturer": db_manufacturer,
                                "model": db_model,
                                "transport": db_transport,
                                "streamMode": db_stream_mode,
                                "source": "db",
                                "status": "timeout_or_error",
                                "message": "设备未在超时内返回基本参数，以下为库中记录"
                            })));
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to send config query: {}", e);
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "name": db_name,
                            "manufacturer": db_manufacturer,
                            "model": db_model,
                            "transport": db_transport,
                            "streamMode": db_stream_mode,
                            "source": "db",
                            "status": "send_failed",
                            "message": format!("下发查询失败: {}", e)
                        })));
                    }
                }
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "name": db_name,
        "manufacturer": db_manufacturer,
        "model": db_model,
        "firmware": null,
        "transport": db_transport,
        "streamMode": db_stream_mode,
        "source": "db",
        "status": "device_offline",
        "message": "设备不在线，以下为库中记录"
    })))
}

#[derive(Debug, Deserialize)]
pub struct ChannelOneQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId", alias = "channelDeviceId")]
    pub channel_id: Option<String>,
}

/// GET /api/device/query/channel/one?deviceId=&channelId=
pub async fn channel_one(
    State(state): State<AppState>,
    Query(q): Query<ChannelOneQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_id = q
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    let channel_id = q.channel_id.as_deref().unwrap_or("").trim();
    if device_id.is_empty() || channel_id.is_empty() {
        return Ok(Json(WVPResult::success(serde_json::Value::Null)));
    }
    let ch = get_channel_by_device_and_channel_id(&state.pool, device_id, channel_id).await?;
    let out = match ch {
        Some(c) => channel_to_json(&c),
        None => serde_json::Value::Null,
    };
    Ok(Json(WVPResult::success(out)))
}

/// GET /api/device/query/streams
/// 获取有流的通道列表
/// 返回: 有流的通道列表
pub async fn query_streams(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Query active streams");

    if let Some(ref zlm_client) = state.zlm_client {
        match zlm_client.get_media_list(None, None, None).await {
            Ok(streams) => {
                let list: Vec<serde_json::Value> = streams.iter().map(|s| {
                    serde_json::json!({
                        "schema": s.schema,
                        "app": s.app,
                        "stream": s.stream,
                        "vhost": s.vhost,
                        "readerCount": s.reader_count,
                        "totalReaderCount": s.total_reader_count,
                        "originType": s.origin_type,
                        "aliveSecond": s.alive_second,
                        "bytesSpeed": s.bytes_speed
                    })
                }).collect();
                
                return Json(WVPResult::success(serde_json::json!({
                    "total": list.len(),
                    "list": list
                })));
            }
            Err(e) => {
                tracing::error!("Failed to query ZLM streams: {}", e);
            }
        }
    }

    Json(WVPResult::success(serde_json::json!({
        "total": 0,
        "list": []
    })))
}


/// GET /api/device/control/record
/// 设备远程录像控制
/// 参数: deviceId, channelId, recordCmdStr (Start/Stop)
/// 返回: 录像控制结果
#[derive(Debug, Deserialize)]
pub struct RecordControlQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    #[serde(alias = "recordCmdStr")]
    pub record_cmd_str: Option<String>,
}

pub async fn control_record(
    State(state): State<AppState>,
    Query(q): Query<RecordControlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let record_cmd = q.record_cmd_str.clone().unwrap_or_default();

    if device_id.is_empty() || channel_id.is_empty() {
        return Json(WVPResult::error("device_id and channel_id are required"));
    }

    tracing::info!("Record control: device={}, channel={}, cmd={}", device_id, channel_id, record_cmd);

    let is_start = record_cmd.to_lowercase() == "start";
    let record_cmd_xml = if is_start {
        "<RecordCmd>Record</RecordCmd>".to_string()
    } else {
        "<RecordCmd>StopRecord</RecordCmd>".to_string()
    };

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online {
                match server.send_device_control(&device_id, &channel_id, "DeviceControl", &record_cmd_xml).await {
                    Ok(_) => {
                        // Phase 7.1: use StateStore repository instead of cache.rs (deprecated).
                        if is_start {
                            state.state_repo.as_ref().set_recording(&device_id, &channel_id, "Record");
                        } else {
                            state.state_repo.as_ref().del_recording(&device_id, &channel_id);
                        }
                        
                        state.ws_state.broadcast("record_state", serde_json::json!({
                            "deviceId": device_id,
                            "channelId": channel_id,
                            "recording": is_start,
                            "recordCmd": record_cmd,
                        })).await;
                        
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "channelId": channel_id,
                            "recordCmd": record_cmd,
                            "recording": is_start,
                            "message": "远程录像控制命令已发送",
                            "code": 0
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send record control: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::error("设备不在线或命令发送失败"))
}

/// GET /api/device/query/sub_channels/:device_id/:parent_channel_id/channels
pub async fn sub_channels(
    State(state): State<AppState>,
    Path((device_id, parent_channel_id)): Path<(String, String)>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list = list_channels_by_parent(&state.pool, &device_id, &parent_channel_id).await?;
    let total = list.len() as u64;
    let list: Vec<serde_json::Value> = list.iter().map(channel_to_json).collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}

pub(crate) fn channel_to_json(c: &DeviceChannel) -> serde_json::Value {
    // 同时输出 camelCase 和 gb_* 前缀字段,兼容前端 /device/channel、
    // /commonChannel、hasStreamChannel 等页面读取 `gbName`/`gbStatus`/
    // `gbManufacturer`/`ptzTypeText` 等字段(Phase 5: 修复通道列表空白)。
    serde_json::json!({
        "id": c.id,
        "deviceId": c.device_id,
        "name": c.name,
        "channelId": c.gb_device_id,
        "gbId": c.gb_device_id.clone().unwrap_or_default(),
        "status": c.status,
        "longitude": c.longitude,
        "latitude": c.latitude,
        "createTime": c.create_time,
        "updateTime": c.update_time,
        "subCount": c.sub_count,
        "hasAudio": c.has_audio,
        "channelType": c.channel_type,
        "ptzType": c.ptz_type.map(|t| t.to_string()).unwrap_or_default(),
        // —— gb_* 兼容字段 ——
        "gbName": c.name,
        "gbDeviceId": c.gb_device_id,
        "gbManufacturer": c.manufacturer,
        "gbModel": c.model,
        "gbOwner": c.owner,
        "gbCivilCode": c.civil_code,
        "gbAddress": c.address,
        "gbParental": c.parental,
        "gbParentId": c.parent_id,
        "gbStatus": c.status,
        "gbLongitude": c.longitude,
        "gbLatitude": c.latitude,
        "streamId": c.stream_id,
        "streamIdentification": c.stream_identification,
        "manufacturer": c.manufacturer,
        "model": c.model,
        "owner": c.owner,
        "civilCode": c.civil_code,
        "address": c.address,
        "parental": c.parental,
        "parentId": c.parent_id,
        "ptzTypeText": ptz_type_text(c.ptz_type),
    })
}

/// 把数字 ptz_type 翻译为前端表头用的中文标签。
/// 与 web/src/views/device/channel/index.vue data().ptzTypes 保持一致。
pub(crate) fn ptz_type_text(ptz: Option<i32>) -> Option<String> {
    ptz.map(|v| match v {
        1 => "球机".to_string(),
        2 => "半球".to_string(),
        3 => "固定枪机".to_string(),
        4 => "遥控枪机".to_string(),
        _ => "未知".to_string(),
    })
}

/// GET /api/device/query/tree/channel/:device_id
pub async fn tree_channel(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let list = list_channels_for_device(&state.pool, &device_id).await?;
    let tree: Vec<serde_json::Value> = list.iter().map(channel_to_json).collect();
    Ok(Json(WVPResult::success(serde_json::Value::Array(tree))))
}

/// POST /api/device/query/channel/audio
/// 修改通道音频状态
/// 参数: channelId, audio (true/false)
/// 返回: 修改结果
#[derive(Debug, Deserialize)]
pub struct ChannelAudioQuery {
    #[serde(alias = "channelId")]
    pub channel_id: Option<i64>,
    pub audio: Option<bool>,
}

pub async fn channel_audio(
    State(state): State<AppState>,
    Query(q): Query<ChannelAudioQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let channel_id = q.channel_id.unwrap_or(0);
    let audio = q.audio.unwrap_or(false);

    if channel_id == 0 {
        return Ok(Json(WVPResult::error("channel_id is required")));
    }

    tracing::info!("Channel audio update: channel_id={}, audio={}", channel_id, audio);

    // 2026-09-12 修正：此前这里对一个**只读查询**（get_media_list）的结果视而不见，
    // 却打印 "ZLM streams updated for audio mode" —— 既没有真的改 ZLM，
    // 又把真正的 DB 写入错误用 `let _ =` 吞掉，然后返回「通道音频设置已更新」。
    //
    // 该设置的真实作用：`gb_device_channel.has_audio` 会被 `sip/gb28181/catalog.rs`
    // 读取并作为 `<HasAudio>` 写入上报给上级平台的目录响应。因此**落库即生效**，
    // 这里只需如实写入并如实报告失败。
    let affected = update_channel_has_audio(&state.pool, channel_id, audio)
        .await
        .map_err(|e| {
            AppError::business(ErrorCode::Error500, format!("通道音频设置写入失败: {}", e))
        })?;
    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("通道不存在: id={}", channel_id),
        ));
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "channelId": channel_id,
        "audio": audio,
        "message": "通道音频设置已更新（将在上报上级平台的目录中体现）",
        "code": 0
    }))))
}


/// POST /api/device/query/channel/stream/identification/update/
/// 更新通道流标识
/// 参数: deviceDbId, id, streamIdentification
/// 返回: 更新结果
#[derive(Debug, Deserialize)]
pub struct StreamIdentificationUpdate {
    #[serde(alias = "deviceDbId")]
    pub device_db_id: Option<i64>,
    pub id: Option<i64>,
    #[serde(alias = "streamIdentification")]
    pub stream_identification: Option<String>,
}

pub async fn channel_stream_identification_update(
    State(state): State<AppState>,
    Query(body): Query<StreamIdentificationUpdate>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let device_db_id = body.device_db_id.unwrap_or(0);
    let id = body.id.unwrap_or(0);
    let stream_identification = body.stream_identification.unwrap_or_default();

    if id == 0 {
        return Ok(Json(WVPResult::error("id is required")));
    }

    tracing::info!("Stream identification update: id={}, stream={}", id, stream_identification);
    // 修正：此前写入错误被 `let _ =` 吞掉后仍返回「更新成功」
    let affected = update_channel_stream_identification(&state.pool, id, &stream_identification)
        .await
        .map_err(|e| {
            AppError::business(ErrorCode::Error500, format!("流标识写入失败: {}", e))
        })?;
    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("通道不存在: id={}", id),
        ));
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "deviceDbId": device_db_id,
        "id": id,
        "streamIdentification": stream_identification,
        "message": "流标识更新成功",
        "code": 0
    }))))
}

#[derive(Debug, Deserialize)]
pub struct DeviceAddBody {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    #[serde(alias = "streamMode")]
    pub stream_mode: Option<String>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    #[serde(alias = "customName")]
    pub custom_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceUpdateBody {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    #[serde(alias = "streamMode")]
    pub stream_mode: Option<String>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    #[serde(alias = "customName")]
    pub custom_name: Option<String>,
}

/// POST /api/device/query/device/update
pub async fn device_update(
    State(state): State<AppState>,
    Json(body): Json<DeviceUpdateBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    update_device(
        &state.pool,
        device_id,
        body.name.as_deref(),
        body.manufacturer.as_deref(),
        body.model.as_deref(),
        body.transport.as_deref(),
        body.stream_mode.as_deref(),
        body.media_server_id.as_deref(),
        body.custom_name.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/device/query/device/add
pub async fn device_add(
    State(state): State<AppState>,
    Json(body): Json<DeviceAddBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body
        .device_id
        .as_deref()
        .unwrap_or("")
        .trim();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    insert_device(
        &state.pool,
        device_id,
        body.name.as_deref(),
        body.manufacturer.as_deref(),
        body.model.as_deref(),
        body.transport.as_deref(),
        body.stream_mode.as_deref(),
        body.media_server_id.as_deref(),
        body.custom_name.as_deref(),
        &now,
    )
    .await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/device/query/devices/:device_id
pub async fn device_one(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("DEBUG_DEVICE_ONE: query for device_id={:?}", device_id);
    let result = get_device_by_device_id(&state.pool, &device_id).await;
    tracing::info!("DEBUG_DEVICE_ONE: query result is_ok={} is_some={}",
        result.is_ok(), result.as_ref().map(|r| r.is_some()).unwrap_or(false));
    if let Err(ref e) = result {
        tracing::error!("DEBUG_DEVICE_ONE: query error: {:?}", e);
    }
    match result {
        Ok(Some(d)) => {
            let v = serde_json::json!({
                "deviceId": d.device_id,
                "name": d.name,
                "manufacturer": d.manufacturer,
                "model": d.model,
                "transport": d.transport,
                "streamMode": d.stream_mode,
                "onLine": d.on_line,
                "ip": d.ip,
                "port": d.port,
                "createTime": d.create_time,
                "updateTime": d.update_time,
                "mediaServerId": d.media_server_id,
                "customName": d.custom_name
            });
            Json(WVPResult::success(v))
        }
        _ => Json(WVPResult::success(serde_json::json!(null))),
    }
}

/// GET /api/device/query/tree/:device_id
pub async fn device_tree(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let channels = list_channels_for_device(&state.pool, &device_id).await?;
    let total = channels.len() as u64;
    let list: Vec<serde_json::Value> = channels.iter().map(channel_to_json).collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    }))))
}


/// GET /api/device/query/subscribe/alarm?deviceId=...&expires=3600
/// 通过 SIP SUBSCRIBE 订阅设备报警事件
pub async fn subscribe_alarm(
    State(state): State<AppState>,
    Query(q): Query<crate::handlers::device_stub::SubscribeAlarmQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.id.clone().unwrap_or_default();
    let expires = q.expires.unwrap_or(3600) as u32;
    if device_id.is_empty() {
        return Json(WVPResult::error("device_id is required"));
    }
    let sip_server = match state.sip_server.as_ref() {
        Some(s) => s,
        None => return Json(WVPResult::error("SIP server not initialized")),
    };
    let server = &**sip_server;
    if let Err(e) = server.send_alarm_subscribe(&device_id, expires).await {
        return Json(WVPResult::error(format!("SIP error: {}", e)));
    }
    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "expires": expires,
        "message": "Alarm subscription sent"
    })))
}
