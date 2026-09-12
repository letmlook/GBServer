use axum::{extract::{Query, State}, Json};
use serde::Deserialize;

use crate::db::update_device_catalog_subscription;
use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct PtzQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    pub command: Option<String>,
    pub speed: Option<u8>,
    #[serde(alias = "presetIndex")]
    pub preset_index: Option<u32>,
    #[serde(alias = "guardCmd")]
    pub guard_cmd: Option<String>,
}

pub async fn device_ptz(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    let speed = q.speed.unwrap_or(1);

    if device_id.is_empty() || channel_id.is_empty() {
        return Json(WVPResult::error("device_id and channel_id are required"));
    }

    tracing::info!("PTZ control: device={}, channel={}, cmd={}, speed={}",
        device_id, channel_id, command, speed);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                let ptz_cmd = build_ptz_xml(&command, speed, 0, 0);
                match server.send_device_control(&device_id, &channel_id, "DeviceControl", &ptz_cmd).await {
                    Ok(_) => {
                        tracing::info!("PTZ command sent via SIP: {}", device_id);
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "channelId": channel_id,
                            "command": command,
                            "speed": speed,
                            "result": "PTZ command sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send PTZ command: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::error("Device not online"))
}

pub async fn device_preset(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    let preset_index = q.preset_index.unwrap_or(0);

    if device_id.is_empty() {
        return Json(WVPResult::error("device_id is required"));
    }

    tracing::info!("Preset control: device={}, channel={}, cmd={}, preset={}",
        device_id, channel_id, command, preset_index);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                let preset_cmd = build_preset_xml(&command, preset_index);
                match server.send_device_control(&device_id, &channel_id, "DeviceControl", &preset_cmd).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "channelId": channel_id,
                            "command": command,
                            "presetIndex": preset_index,
                            "result": "Preset command sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send preset command: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::error("Device not online"))
}

pub async fn device_guard(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let guard_cmd = q.guard_cmd.clone().unwrap_or_default();

    if device_id.is_empty() {
        return Json(WVPResult::error("device_id is required"));
    }

    let cmd_type = if guard_cmd == "SetGuard" { "设防" } else { "撤防" };
    tracing::info!("Guard control: device={}, cmd={}", device_id, guard_cmd);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                let guard_xml = format!(r#"<GuardCmd>{}</GuardCmd>"#, guard_cmd);
                match server.send_device_control(&device_id, &device_id, "DeviceControl", &guard_xml).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "guardCmd": guard_cmd,
                            "result": format!("{} command sent", cmd_type)
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send guard command: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::error("Device not online"))
}

#[derive(Debug, Deserialize)]
pub struct SubscribeQuery {
    pub id: Option<String>,
    pub cycle: Option<i32>,
    pub interval: Option<i32>,
}

pub async fn subscribe_catalog(
    State(state): State<AppState>,
    Query(q): Query<SubscribeQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.id.clone().unwrap_or_default();
    let cycle = q.cycle.unwrap_or(3600) as u32;
    let updated = update_device_catalog_subscription(&state.pool, &device_id, cycle as i32)
        .await
        .unwrap_or_default();

    tracing::info!("Catalog subscription: device={}, cycle={}", device_id, cycle);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online {
                match server.send_subscribe(&device_id, "Catalog", cycle).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "cycle": cycle,
                            "updated": updated,
                            "result": "Catalog subscription sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send catalog subscription: {}", e);
                    }
                }
            }
        }
    }

    if updated > 0 {
        return Json(WVPResult::success(serde_json::json!({
            "deviceId": device_id,
            "cycle": cycle,
            "updated": updated,
            "result": "Catalog subscription saved"
        })));
    }

    Json(WVPResult::error("Device not online or subscription failed"))
}

pub async fn subscribe_mobile_position(
    State(state): State<AppState>,
    Query(q): Query<SubscribeQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.id.clone().unwrap_or_default();
    let cycle = q.cycle.unwrap_or(5) as u32;
    let interval = q.interval.unwrap_or(5);

    tracing::info!("Position subscription: device={}, cycle={}, interval={}",
        device_id, cycle, interval);

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
                            "result": "Position subscription sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send position subscription: {}", e);
                    }
                }
            }
        }
    }

    Json(WVPResult::error("Device not online or subscription failed"))
}

#[allow(non_snake_case)]
/// 构造 `<Control>` 的子元素体（云台/镜头/预置位）。
///
/// 统一走 `sip::gb28181::front_end_control`：**全部**是 `PTZCmd` 8 字节格式
/// （0xA5 起始 + 指令码 + 累加校验）—— 聚焦/光圈、预置位也走指令码，
/// 与 WVP 的 `SourcePTZServiceForGbImpl` 和 GB/T 28181-2022 §A.3 一致。
///
/// 修正：此前这里生成 `05 01 00 00 00 ss FF`（6 字节、非 A5 起始、无校验），
/// 且把聚焦/光圈/预置位一律塞进 `<PTZCmd>` —— 真实设备按国标解析时
/// 得到的都是无效指令。
fn build_ptz_xml(command: &str, speed: u8, preset: u32, _dwStop: u32) -> String {
    // 云台/聚焦光圈/预置位统一是 `PTZCmd` 8 字节指令（与 WVP 一致）
    match crate::sip::gb28181::front_end_control::control_element(command, speed, preset) {
        Some((_, v)) => format!(r#"<PTZCmd>{}</PTZCmd>"#, v),
        _ => format!(r#"<PTZCmd>{}</PTZCmd>"#, crate::sip::gb28181::front_end_control::build_ptz_cmd(
            crate::sip::gb28181::front_end_control::PtzAction::Stop, 0)),
    }
}

fn build_preset_xml(command: &str, preset_index: u32) -> String {
    build_ptz_xml(command, 1, preset_index, 0)
}

/// 设备配置查询
#[derive(Debug, Deserialize)]
pub struct ConfigQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "configType")]
    pub config_type: Option<String>,
}

pub async fn device_config_query(
    State(state): State<AppState>,
    Query(q): Query<ConfigQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let config_type = q.config_type.clone().unwrap_or_else(|| "BasicParam".to_string());

    if device_id.is_empty() {
        return Json(WVPResult::error("device_id is required"));
    }

    tracing::info!("Config query: device={}, type={}", device_id, config_type);
    Json(WVPResult::success(
        query_config_and_wait(&state, &device_id, &config_type).await,
    ))
}

/// 下发 ConfigDownload 查询并**等待**设备应答，返回统一的 JSON 载荷。
///
/// 修正：`device_config_query`（查询参数版）此前自己拼 XML 直接
/// `send_message_to_device` 就返回 `"Config query sent"` —— 既不登记 pending
/// 请求（设备回来的应答会被当作 unsolicited 丢弃），也从不等待，等于把
/// "查询设备配置"实现成了一个纯发送动作。同一份功能在
/// `device_query::device_config_query`（路径参数版）里本来是**真的**在等，
/// 两处实现并存且行为不同。现抽成本函数，两处共用，消除重复。
pub(crate) async fn query_config_and_wait(
    state: &AppState,
    device_id: &str,
    config_type: &str,
) -> serde_json::Value {
    let Some(ref sip_server) = state.sip_server else {
        return serde_json::json!({
            "deviceId": device_id,
            "configType": config_type,
            "status": "sip_unavailable",
            "message": "SIP 服务未启动",
        });
    };
    let server = &**sip_server;
    let Some(device) = server.device_manager().get(device_id).await else {
        return serde_json::json!({
            "deviceId": device_id,
            "configType": config_type,
            "status": "not_registered",
            "message": "设备未注册",
        });
    };
    if !device.online {
        return serde_json::json!({
            "deviceId": device_id,
            "configType": config_type,
            "status": "offline",
            "message": "设备不在线",
        });
    }

    // SN 必须"登记用哪个、发出就用哪个、设备回显也就是哪个" —— 这是
    // Call-ID 不一致时唯一的关联依据（见 pending_request::extract_sn）。
    let sn = chrono::Utc::now().timestamp_millis() as u32;
    let commander = server.device_commander();
    let (req, rx) = commander.register_device_config_with_receiver(device_id, sn);
    if let Err(e) = server
        .send_device_config_query(device_id, config_type, sn)
        .await
    {
        tracing::error!("下发 ConfigDownload 失败 device={}: {}", device_id, e);
        return serde_json::json!({
            "deviceId": device_id,
            "configType": config_type,
            "sn": sn,
            "status": "send_failed",
            "message": format!("下发查询失败: {}", e),
        });
    }

    match commander.await_response(req, rx, 15).await {
        // ConfigDownload 各 ConfigType 的结构差异大，不做强解析，原样透传 XML
        Ok(xml) => serde_json::json!({
            "deviceId": device_id,
            "configType": config_type,
            "sn": sn,
            "xml": xml,
            "source": "live",
        }),
        Err(_) => serde_json::json!({
            "deviceId": device_id,
            "configType": config_type,
            "sn": sn,
            "status": "timeout",
            "message": "Device did not respond within 15s",
            "source": "live",
        }),
    }
}

/// 设备配置下发
#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "configType")]
    pub config_type: Option<String>,
    #[serde(alias = "configData")]
    pub config_data: Option<serde_json::Value>,
}

pub async fn device_config_update(
    State(state): State<AppState>,
    Json(body): Json<ConfigUpdate>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = body.device_id.clone().unwrap_or_default();
    let config_type = body.config_type.clone().unwrap_or_else(|| "BasicParam".to_string());
    let config_data = body.config_data.clone().unwrap_or(serde_json::json!({}));

    if device_id.is_empty() {
        return Json(WVPResult::error("device_id is required"));
    }

    tracing::info!("Config update: device={}, type={}", device_id, config_type);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                // 根据配置类型构建不同的配置XML
                let config_xml = match config_type.as_str() {
                    "BasicParam" => {
                        let sip_server_id = config_data.get("sipServerId")
                            .and_then(|v| v.as_str()).unwrap_or("");
                        let sip_server_port = config_data.get("sipServerPort")
                            .and_then(|v| v.as_u64()).unwrap_or(5060);
                        let sip_server_domain = config_data.get("sipServerDomain")
                            .and_then(|v| v.as_str()).unwrap_or("");
                        let transport = config_data.get("transport")
                            .and_then(|v| v.as_str()).unwrap_or("UDP");
                        let charset = config_data.get("charset")
                            .and_then(|v| v.as_str()).unwrap_or("GB2312");
                        
                        format!(
                            r#"<?xml version="1.0" encoding="UTF-8"?>
<Control>
<CmdType>DeviceConfig</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<BasicParam>
<SIPServerID>{}</SIPServerID>
<SIPServerPort>{}</SIPServerPort>
<SIPServerDomain>{}</SIPServerDomain>
<Transport>{}</Transport>
<CharSet>{}</CharSet>
</BasicParam>
</Control>"#,
                            chrono::Utc::now().timestamp() % 10000,
                            device_id,
                            sip_server_id,
                            sip_server_port,
                            sip_server_domain,
                            transport,
                            charset
                        )
                    }
                    "SnapConfig" => {
                        let snap_interval = config_data.get("snapInterval")
                            .and_then(|v| v.as_u64()).unwrap_or(0);
                        format!(
                            r#"<?xml version="1.0" encoding="UTF-8"?>
<Control>
<CmdType>DeviceConfig</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<SnapConfig>
<SnapInterval>{}</SnapInterval>
</SnapConfig>
</Control>"#,
                            chrono::Utc::now().timestamp() % 10000,
                            device_id,
                            snap_interval
                        )
                    }
                    _ => {
                        // 通用配置，直接使用传入的JSON
                        format!(
                            r#"<?xml version="1.0" encoding="UTF-8"?>
<Control>
<CmdType>DeviceConfig</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<ConfigType>{}</ConfigType>
<ConfigData>{}</ConfigData>
</Control>"#,
                            chrono::Utc::now().timestamp() % 10000,
                            device_id,
                            config_type,
                            config_data.to_string()
                        )
                    }
                };

                match server.send_device_control(&device_id, &device_id, "DeviceConfig", &config_xml).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "configType": config_type,
                            "result": "Config update sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send config update: {}", e);
                        return Json(WVPResult::error(format!("Failed to send config update: {}", e)));
                    }
                }
            }
        }
    }

    Json(WVPResult::error("Device not online"))
}

/// 设备重启
#[derive(Debug, Deserialize)]
pub struct RebootQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
}

pub async fn device_reboot(
    State(state): State<AppState>,
    Query(q): Query<RebootQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();

    if device_id.is_empty() {
        return Json(WVPResult::error("device_id is required"));
    }

    tracing::info!("Device reboot: device={}", device_id);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                let reboot_xml = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<Control>
<CmdType>DeviceControl</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Restart>
<ChannelID>0</ChannelID>
</Restart>
</Control>"#,
                    chrono::Utc::now().timestamp() % 10000,
                    device_id
                );

                match server.send_device_control(&device_id, &device_id, "DeviceControl", &reboot_xml).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "result": "Reboot command sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send reboot command: {}", e);
                        return Json(WVPResult::error(format!("Failed to send reboot command: {}", e)));
                    }
                }
            }
        }
    }

    Json(WVPResult::error("Device not online"))
}
