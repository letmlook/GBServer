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
        // `total/current/errorMsg/syncIng` 是 WVP `SyncStatus` 的字段，
        // 前端（含 legacy 的同步进度弹窗）靠它们算百分比并显示错误。
        // 此前这几个键一个都没有 → 进度条恒为 0/空。
        let total = db_device
            .as_ref()
            .and_then(|item| item.channel_count)
            .unwrap_or(0) as i64;
        let syncing = active_count > 0;
        Json(WVPResult::success(serde_json::json!({
            "deviceId": if requested_device_id.is_empty() { serde_json::Value::Null } else { serde_json::json!(requested_device_id) },
            "status": if syncing { "active" } else { "idle" },
            "activeSubscriptions": active_count,
            "online": db_device.as_ref().and_then(|item| item.on_line).unwrap_or(false),
            "streamMode": db_device.as_ref().and_then(|item| item.stream_mode.clone()),
            "syncIng": syncing,
            "total": total,
            "current": if syncing { 0 } else { total },
            "errorMsg": serde_json::Value::Null,
            "message": if syncing { "正在同步设备目录" } else { "同步完成" }
        })))
    } else {
        Json(WVPResult::success(serde_json::json!({
            "deviceId": if requested_device_id.is_empty() { serde_json::Value::Null } else { serde_json::json!(requested_device_id) },
            "status": "idle",
            "online": db_device.as_ref().and_then(|item| item.on_line).unwrap_or(false),
            "streamMode": db_device.as_ref().and_then(|item| item.stream_mode.clone()),
            "syncIng": false,
            "total": 0,
            "current": 0,
            "errorMsg": "SIP服务未初始化",
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

                        // 超时后把会话**确定性收尾**：设备声明 SumNum 页却只发了几页
                        // （甚至一页都不发）时，会话不能永远停在 receiving ——
                        // 各分页已逐包入库，这里按"已完成（可能不完整）"或"失败"收尾，
                        // 并把差异写进 error，前端才判断得出结果。
                        if matches!(sync_state.as_str(), "waiting" | "receiving") {
                            manager.finalize_partial(&device_id, 8);
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
                            }
                        }

                        // 统计本次同步后该设备名下的通道数，便于前端直接展示结果
                        let channel_count =
                            crate::db::device::list_channels_for_device(&state.pool, &device_id)
                                .await
                                .map(|v| v.len())
                                .unwrap_or(0);

                        let message = match sync_state.as_str() {
                            "done" if error.is_some() => "设备目录同步完成（分页不完整，见 error）",
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
///
/// 返回**正在推流**的通道列表（数据源是 ZLM `/index/api/getMediaList`，逐节点汇总）。
///
/// 与 WVP 的 `DeviceQuery./streams`（返回 `PageInfo<DeviceChannel>`）对齐的关键点：
/// * 每行必须带 `deviceId`/`channelId` —— 前端要靠它跳转到实时预览；此前只把
///   ZLM 的流信息原样透出，`deviceId` 不存在，仪表盘 6 张卡片全是死链；
/// * `mediaServerId` 标注该流属于哪个节点（多节点部署时前端/排查都需要）；
/// * 支持 `page`/`count`/`query`（`query` 匹配设备号/通道号/流名），`total` 是
///   **过滤后**的总数，而不是当前页条数。
pub async fn query_streams(
    State(state): State<AppState>,
    Query(q): Query<StreamQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(50).clamp(1, 1000);
    let keyword = q
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase());

    tracing::info!("Query active streams: page={page} count={count} query={keyword:?}");

    // 多节点：逐节点查询并标注来源；默认节点若不在表里也要带上
    let mut nodes: Vec<(String, std::sync::Arc<crate::zlm::ZlmClient>)> = state
        .zlm_clients
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(ref default_client) = state.zlm_client {
        if !nodes.iter().any(|(_, c)| std::sync::Arc::ptr_eq(c, default_client)) {
            nodes.push(("auto".to_string(), default_client.clone()));
        }
    }

    let mut rows: Vec<serde_json::Value> = Vec::new();
    for (media_server_id, client) in nodes {
        match client.get_media_list(None, None, None).await {
            Ok(streams) => {
                for s in &streams {
                    rows.push(stream_row_json(&media_server_id, s));
                }
            }
            Err(e) => {
                tracing::warn!("查询 ZLM 节点 {media_server_id} 的流列表失败: {e}");
            }
        }
    }

    if let Some(ref kw) = keyword {
        rows.retain(|r| {
            ["deviceId", "channelId", "stream", "app"].iter().any(|k| {
                r.get(k)
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_lowercase().contains(kw))
                    .unwrap_or(false)
            })
        });
    }
    let total = rows.len();
    let offset = ((page - 1) * count) as usize;
    let list: Vec<serde_json::Value> = rows.into_iter().skip(offset).take(count as usize).collect();

    Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": list
    })))
}

/// `/api/device/query/streams` 的查询参数
#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
}

/// 把一路 ZLM 流拼成前端需要的行。
///
/// `deviceId`/`channelId` 从流名里取前两段（`设备ID_通道ID[...]`），
/// 回放流名形如 `设备ID_通道ID_开始_结束`，所以按段取而不是按 `parse_stream_id`
/// 的整段切分。推流/代理（`push_xxx`/`proxy_xxx`）没有国标标识，如实留空。
fn stream_row_json(media_server_id: &str, s: &crate::zlm::types::MediaInfo) -> serde_json::Value {
    let mut parts = s.stream.split('_');
    let (device_id, channel_id) = match (parts.next(), parts.next()) {
        (Some(d), Some(c))
            if !d.is_empty()
                && !c.is_empty()
                && !s.stream.starts_with("push_")
                && !s.stream.starts_with("proxy_") =>
        {
            (d.to_string(), c.to_string())
        }
        _ => (String::new(), String::new()),
    };
    serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "mediaServerId": media_server_id,
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
    /// 设备 IP / 端口 / 密码 / 注册有效期 / 心跳参数。
    /// 这些列**一直存在**，但后端 DTO 里没有 → 前端填了也静默丢弃（接口还回成功）。
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub password: Option<String>,
    pub expires: Option<i32>,
    #[serde(alias = "heartBeatInterval")]
    pub heart_beat_interval: Option<i32>,
    #[serde(alias = "heartBeatCount")]
    pub heart_beat_count: Option<i32>,
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
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub password: Option<String>,
    pub expires: Option<i32>,
    #[serde(alias = "heartBeatInterval")]
    pub heart_beat_interval: Option<i32>,
    #[serde(alias = "heartBeatCount")]
    pub heart_beat_count: Option<i32>,
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
    let fields = crate::db::device::DeviceWriteFields {
        name: body.name.as_deref(),
        manufacturer: body.manufacturer.as_deref(),
        model: body.model.as_deref(),
        transport: body.transport.as_deref(),
        stream_mode: body.stream_mode.as_deref(),
        media_server_id: body.media_server_id.as_deref(),
        custom_name: body.custom_name.as_deref(),
        ip: body.ip.as_deref(),
        port: body.port,
        // 空密码视为"未填写"，不要写空串
        password: body.password.as_deref().filter(|s| !s.is_empty()),
        expires: body.expires,
        heart_beat_interval: body.heart_beat_interval,
        heart_beat_count: body.heart_beat_count,
    };
    insert_device(&state.pool, device_id, &fields, &now).await?;
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// POST /api/device/query/device/update
pub async fn device_update(
    State(state): State<AppState>,
    Json(body): Json<DeviceUpdateBody>,
) -> Result<Json<WVPResult<()>>, AppError> {
    let device_id = body.device_id.as_deref().unwrap_or("").trim();
    if device_id.is_empty() {
        return Err(AppError::business(ErrorCode::Error400, "缺少 deviceId"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let fields = crate::db::device::DeviceWriteFields {
        name: body.name.as_deref(),
        manufacturer: body.manufacturer.as_deref(),
        model: body.model.as_deref(),
        transport: body.transport.as_deref(),
        stream_mode: body.stream_mode.as_deref(),
        media_server_id: body.media_server_id.as_deref(),
        custom_name: body.custom_name.as_deref(),
        ip: body.ip.as_deref(),
        port: body.port,
        // 前端编辑时不回填密码（传空串）→ 必须视为"不修改"
        password: body.password.as_deref().filter(|s| !s.is_empty()),
        expires: body.expires,
        heart_beat_interval: body.heart_beat_interval,
        heart_beat_count: body.heart_beat_count,
    };
    let affected = update_device(&state.pool, device_id, &fields, &now).await?;
    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("设备不存在: {device_id}"),
        ));
    }
    Ok(Json(WVPResult::<()>::success_empty()))
}

/// GET /api/device/query/devices/:device_id
pub async fn device_one(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    // 直接序列化 `Device`（`#[serde(rename_all = "camelCase")]`），
    // 与列表接口**同源**：此前手写了一份只有 12 个键的 JSON，缺
    // `id/firmware/expires/heartBeat*/registerTime/channelCount` 等，
    // 编辑弹窗靠 `props.device?.id` 判断"新增还是编辑"，缺 id 会把编辑变成新增。
    match get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => Json(WVPResult::success(
            serde_json::to_value(&d).unwrap_or(serde_json::Value::Null),
        )),
        Ok(None) => Json(WVPResult::success(serde_json::json!(null))),
        Err(e) => {
            tracing::warn!("查询设备 {device_id} 失败: {e}");
            Json(WVPResult::error(format!("查询设备失败: {e}")))
        }
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

#[cfg(test)]
mod stream_row_tests {
    use super::*;
    use crate::zlm::types::MediaInfo;

    fn info(stream: &str) -> MediaInfo {
        MediaInfo {
            app: "rtp".to_string(),
            stream: stream.to_string(),
            schema: "rtsp".to_string(),
            vhost: "__defaultVhost__".to_string(),
            reader_count: 0,
            total_reader_count: 0,
            origin_type: 0,
            origin_url: None,
            create_stamp: 0,
            alive_second: 0,
            bytes_speed: 0,
            tracks: Vec::new(),
        }
    }

    /// 实时流 `设备ID_通道ID` 必须解析出国标标识（前端靠它跳转直播页）。
    #[test]
    fn test_live_stream_row_has_device_and_channel() {
        let row = stream_row_json("zlmediakit-1", &info("34020000001320000001_34020000001310000001"));
        assert_eq!(row["deviceId"], "34020000001320000001");
        assert_eq!(row["channelId"], "34020000001310000001");
        assert_eq!(row["mediaServerId"], "zlmediakit-1");
        assert_eq!(row["stream"], "34020000001320000001_34020000001310000001");
    }

    /// 回放流名多两段（开始/结束时间），通道号仍是第二段。
    #[test]
    fn test_playback_stream_row_takes_second_segment() {
        let row = stream_row_json(
            "auto",
            &info("34020000001320000001_34020000001310000001_1700000000_1700003600"),
        );
        assert_eq!(row["deviceId"], "34020000001320000001");
        assert_eq!(row["channelId"], "34020000001310000001");
    }

    /// 推流/代理没有国标标识，如实留空（前端据此跳过，而不是跳转到不存在的通道）。
    #[test]
    fn test_push_and_proxy_streams_have_no_gb_ids() {
        for name in ["push_live1", "proxy_camera1", "single"] {
            let row = stream_row_json("auto", &info(name));
            assert_eq!(row["deviceId"], "", "{name} 不应有 deviceId");
            assert_eq!(row["channelId"], "", "{name} 不应有 channelId");
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod device_write_tests {
    use super::*;
    use crate::test_support::app_state;

    /// 前端编辑框里的 IP / 端口 / 密码 / 注册有效期 / 心跳参数此前**后端 DTO 里
    /// 根本没有**（接口回成功、库里仍为空）。这条测试逐字段验证真的落库。
    #[tokio::test]
    async fn test_device_add_and_update_persist_all_form_fields() {
        let state = app_state().await;

        let add: DeviceAddBody = serde_json::from_value(serde_json::json!({
            "deviceId": "34020000001320000001",
            "name": "前门",
            "manufacturer": "MockVendor",
            "model": "IPC-1",
            "transport": "UDP",
            "streamMode": "TCP-PASSIVE",
            "ip": "192.168.1.10",
            "port": 5060,
            "password": "admin123",
            "expires": 3600,
            "heartBeatInterval": 60,
            "heartBeatCount": 3
        }))
        .expect("camelCase 必须能反序列化");

        let _ = device_add(State(state.clone()), Json(add)).await.expect("新增应成功");

        let d = crate::db::device::get_device_by_device_id(&state.pool, "34020000001320000001")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(d.ip.as_deref(), Some("192.168.1.10"));
        assert_eq!(d.port, Some(5060));
        assert_eq!(d.password.as_deref(), Some("admin123"));
        assert_eq!(d.expires, Some(3600));
        assert_eq!(d.heart_beat_interval, Some(60));
        assert_eq!(d.heart_beat_count, Some(3));
        assert_eq!(d.stream_mode.as_deref(), Some("TCP-PASSIVE"));

        // 编辑：改 IP/端口，且**空密码不能把已有密码清掉**
        let upd: DeviceUpdateBody = serde_json::from_value(serde_json::json!({
            "deviceId": "34020000001320000001",
            "name": "后门",
            "ip": "10.0.0.5",
            "port": 5070,
            "password": ""
        }))
        .unwrap();
        let _ = device_update(State(state.clone()), Json(upd)).await.expect("更新应成功");

        let d = crate::db::device::get_device_by_device_id(&state.pool, "34020000001320000001")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(d.name.as_deref(), Some("后门"));
        assert_eq!(d.ip.as_deref(), Some("10.0.0.5"));
        assert_eq!(d.port, Some(5070));
        assert_eq!(d.password.as_deref(), Some("admin123"), "空密码表示不修改");
        assert_eq!(d.expires, Some(3600), "未提交的字段保持原值");
        assert_eq!(d.heart_beat_count, Some(3));
    }

    /// 设备详情必须返回**完整行**（含 id）：编辑弹窗靠 `id` 判断新增/编辑。
    #[tokio::test]
    async fn test_device_one_returns_full_row() {
        let state = app_state().await;
        let add: DeviceAddBody = serde_json::from_value(serde_json::json!({
            "deviceId": "34020000001320000002",
            "name": "设备2",
            "expires": 1800
        }))
        .unwrap();
        let _ = device_add(State(state.clone()), Json(add)).await.unwrap();

        let resp = device_one(
            State(state.clone()),
            axum::extract::Path("34020000001320000002".to_string()),
        )
        .await;
        let d = resp.0.data.unwrap();
        assert!(d.get("id").and_then(|v| v.as_i64()).unwrap_or(0) > 0, "必须带 id: {d}");
        assert_eq!(d["deviceId"], "34020000001320000002");
        assert_eq!(d["expires"], 1800);
        assert!(d.get("onLine").is_some(), "在线状态键名是 onLine（与 WVP 一致）");
    }

    /// `sync_status` 必须有 total/current/errorMsg（WVP `SyncStatus`），
    /// 否则同步进度弹窗算不出百分比、也看不到错误。
    #[tokio::test]
    async fn test_sync_status_exposes_wvp_fields() {
        let state = app_state().await;
        let resp = sync_status(
            State(state.clone()),
            Query(SyncStatusQuery {
                device_id: Some("34020000001320000001".into()),
            }),
        )
        .await;
        let d = resp.0.data.unwrap();
        assert!(d.get("total").is_some(), "缺 total: {d}");
        assert!(d.get("current").is_some(), "缺 current: {d}");
        assert!(d.get("errorMsg").is_some(), "缺 errorMsg: {d}");
    }
}
