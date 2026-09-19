use axum::{extract::{Query, State}, Json};
use serde::Deserialize;

use crate::db::update_device_catalog_subscription;
use crate::response::ApiResult;
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
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    let speed = q.speed.unwrap_or(1);

    if device_id.is_empty() || channel_id.is_empty() {
        return Json(ApiResult::error("device_id and channel_id are required"));
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
                        return Json(ApiResult::success(serde_json::json!({
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

    Json(ApiResult::error("Device not online"))
}

pub async fn device_preset(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    let preset_index = q.preset_index.unwrap_or(0);

    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
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
                        return Json(ApiResult::success(serde_json::json!({
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

    Json(ApiResult::error("Device not online"))
}

pub async fn device_guard(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let guard_cmd = q.guard_cmd.clone().unwrap_or_default();

    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
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
                        return Json(ApiResult::success(serde_json::json!({
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

    Json(ApiResult::error("Device not online"))
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
) -> Json<ApiResult<serde_json::Value>> {
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
                        return Json(ApiResult::success(serde_json::json!({
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
        return Json(ApiResult::success(serde_json::json!({
            "deviceId": device_id,
            "cycle": cycle,
            "updated": updated,
            "result": "Catalog subscription saved"
        })));
    }

    Json(ApiResult::error("Device not online or subscription failed"))
}

pub async fn subscribe_mobile_position(
    State(state): State<AppState>,
    Query(q): Query<SubscribeQuery>,
) -> Json<ApiResult<serde_json::Value>> {
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
                        return Json(ApiResult::success(serde_json::json!({
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

    Json(ApiResult::error("Device not online or subscription failed"))
}

#[allow(non_snake_case)]
/// 构造 `<Control>` 的子元素体（云台/镜头/预置位）。
///
/// 统一走 `sip::gb28181::front_end_control`：**全部**是 `PTZCmd` 8 字节格式
/// （0xA5 起始 + 指令码 + 累加校验）—— 聚焦/光圈、预置位也走指令码，
/// 与 GB/T 28181-2022 §A.3 一致。
///
/// 修正：此前这里生成 `05 01 00 00 00 ss FF`（6 字节、非 A5 起始、无校验），
/// 且把聚焦/光圈/预置位一律塞进 `<PTZCmd>` —— 真实设备按国标解析时
/// 得到的都是无效指令。
fn build_ptz_xml(command: &str, speed: u8, preset: u32, _dwStop: u32) -> String {
    // 云台/聚焦光圈/预置位统一是 `PTZCmd` 8 字节指令
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
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let config_type = q.config_type.clone().unwrap_or_else(|| "BasicParam".to_string());

    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
    }

    tracing::info!("Config query: device={}, type={}", device_id, config_type);
    Json(ApiResult::success(
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
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = body.device_id.clone().unwrap_or_default();
    let config_type = body.config_type.clone().unwrap_or_else(|| "BasicParam".to_string());
    let config_data = body.config_data.clone().unwrap_or(serde_json::json!({}));

    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
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
                        
                        // 只给**内层元素**：外壳（`<Control>`/CmdType/SN/DeviceID）
                        // 由 `send_device_control` 统一生成。此前这里又拼了一整份
                        // `<Control>` 文档再交给它包装，报文里出现**嵌套的 Control**
                        // 和夹在元素中间的 `<?xml ...?>` 声明 —— 非法 XML，
                        // 真实设备解析失败即丢弃（配置下发/重启因此从未真正生效）。
                        format!(
                            "<BasicParam>\n<SIPServerID>{}</SIPServerID>\n<SIPServerPort>{}</SIPServerPort>\n<SIPServerDomain>{}</SIPServerDomain>\n<Transport>{}</Transport>\n<CharSet>{}</CharSet>\n</BasicParam>",
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
                            "<SnapConfig>\n<SnapInterval>{}</SnapInterval>\n</SnapConfig>",
                            snap_interval
                        )
                    }
                    _ => {
                        // 通用配置，直接使用传入的JSON
                        format!(
                            "<ConfigType>{}</ConfigType>\n<ConfigData>{}</ConfigData>",
                            config_type,
                            config_data
                        )
                    }
                };

                match server.send_device_control(&device_id, &device_id, "DeviceConfig", &config_xml).await {
                    Ok(_) => {
                        return Json(ApiResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "configType": config_type,
                            "result": "Config update sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send config update: {}", e);
                        return Json(ApiResult::error(format!("Failed to send config update: {}", e)));
                    }
                }
            }
        }
    }

    Json(ApiResult::error("Device not online"))
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
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();

    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
    }

    tracing::info!("Device reboot: device={}", device_id);

    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                // 国标远程启动是 `<TeleBoot>Boot</TeleBoot>`。
                // 此前发的是非标的 `<Restart><ChannelID>0</ChannelID></Restart>`，
                // 而且外面又套了一层 `<Control>`（嵌套 + 元素中间夹 XML 声明 = 非法 XML）。
                let reboot_xml = "<TeleBoot>Boot</TeleBoot>".to_string();

                match server.send_device_control(&device_id, &device_id, "DeviceControl", &reboot_xml).await {
                    Ok(_) => {
                        return Json(ApiResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "result": "Reboot command sent"
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to send reboot command: {}", e);
                        return Json(ApiResult::error(format!("Failed to send reboot command: {}", e)));
                    }
                }
            }
        }
    }

    Json(ApiResult::error("Device not online"))
}

// ============================================================================
// 设备控制剩余端点：远程启动 / 报警复位 / 强制关键帧 /
// 看守位 / 拉框放大缩小。
//
// 这些端点都是真实下发的设备控制命令，
// 本平台此前**完全没有挂载**：第三方前端按这些路径调用时
// 会落到 SPA 兜底拿到 index.html，看起来像"接口不存在"。
// ============================================================================

/// 统一的"设备在线 → 下发 DeviceControl → 返回结果"流程。
///
/// `element` 是 `<Control>` 内部的控制元素（外壳由 `send_device_control` 生成）。
async fn send_control_element(
    state: &AppState,
    device_id: &str,
    channel_id: &str,
    element: &str,
    extra: serde_json::Value,
) -> Json<ApiResult<serde_json::Value>> {
    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
    }
    let Some(ref sip_server) = state.sip_server else {
        return Json(ApiResult::error("SIP server not available"));
    };
    let server = &**sip_server;
    let Some(device) = server.device_manager().get(device_id).await else {
        return Json(ApiResult::error(format!("设备不存在或未注册: {device_id}")));
    };
    if !device.online || device.addr.is_none() {
        return Json(ApiResult::error(format!("设备不在线: {device_id}")));
    }
    if let Err(e) = server
        .send_device_control(device_id, channel_id, "DeviceControl", element)
        .await
    {
        tracing::error!("DeviceControl 下发失败 device={}: {}", device_id, e);
        return Json(ApiResult::error(format!("命令发送失败: {e}")));
    }

    let mut data = serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "result": "command sent",
        "xml": element,
    });
    if let (Some(obj), Some(extra_obj)) = (data.as_object_mut(), extra.as_object()) {
        for (k, v) in extra_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    Json(ApiResult::success(data))
}

/// GET /api/device/control/teleboot/:device_id —— 远程启动
pub async fn device_teleboot(
    State(state): State<AppState>,
    axum::extract::Path(device_id): axum::extract::Path<String>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Device teleboot: device={}", device_id);
    send_control_element(
        &state,
        &device_id,
        &device_id,
        "<TeleBoot>Boot</TeleBoot>",
        serde_json::json!({}),
    )
    .await
}

/// GET /api/device/control/reset_alarm —— 报警复位
#[derive(Debug, Default, Deserialize)]
pub struct ResetAlarmQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    #[serde(alias = "alarmMethod")]
    pub alarm_method: Option<String>,
    #[serde(alias = "alarmType")]
    pub alarm_type: Option<String>,
}

/// 构造报警复位的控制元素：
/// `<AlarmCmd>ResetAlarm</AlarmCmd>`，可选 `<Info><AlarmMethod/><AlarmType/></Info>`。
pub(crate) fn build_alarm_reset_element(
    alarm_method: Option<&str>,
    alarm_type: Option<&str>,
) -> String {
    let method = alarm_method.map(str::trim).filter(|s| !s.is_empty());
    let atype = alarm_type.map(str::trim).filter(|s| !s.is_empty());
    let mut xml = String::from("<AlarmCmd>ResetAlarm</AlarmCmd>");
    if method.is_some() || atype.is_some() {
        xml.push_str("\n<Info>");
        if let Some(m) = method {
            xml.push_str(&format!("\n<AlarmMethod>{}</AlarmMethod>", m));
        }
        if let Some(t) = atype {
            xml.push_str(&format!("\n<AlarmType>{}</AlarmType>", t));
        }
        xml.push_str("\n</Info>");
    }
    xml
}

pub async fn device_reset_alarm(
    State(state): State<AppState>,
    Query(q): Query<ResetAlarmQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let element =
        build_alarm_reset_element(q.alarm_method.as_deref(), q.alarm_type.as_deref());
    tracing::info!(
        "Device reset_alarm: device={}, channel={}, method={:?}, type={:?}",
        device_id,
        channel_id,
        q.alarm_method,
        q.alarm_type
    );
    send_control_element(
        &state,
        &device_id,
        &channel_id,
        &element,
        serde_json::json!({
            "alarmMethod": q.alarm_method,
            "alarmType": q.alarm_type,
        }),
    )
    .await
}

/// GET /api/device/control/i_frame —— 强制关键帧
#[derive(Debug, Default, Deserialize)]
pub struct IFrameQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
}

pub async fn device_iframe(
    State(state): State<AppState>,
    Query(q): Query<IFrameQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    tracing::info!("Device i_frame: device={}, channel={}", device_id, channel_id);
    // GB/T 28181-2016 §9.3.1：`<IFrameCmd>IFrame</IFrameCmd>`。
    // （早期实现里是 `<IFameCmd>Send</IFameCmd>` —— 元素名少一个 r 且取值不同，
    //  那是笔误，严格解析的设备认不出来，这里按国标下发。）
    send_control_element(
        &state,
        &device_id,
        &channel_id,
        "<IFrameCmd>IFrame</IFrameCmd>",
        serde_json::json!({}),
    )
    .await
}

/// GET /api/device/control/home_position —— 看守位设置
#[derive(Debug, Default, Deserialize)]
pub struct HomePositionQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    /// 是否开启看守位（前端可能是 true/false，也可能是 1/0）
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_bool")]
    pub enabled: Option<bool>,
    #[serde(alias = "resetTime", default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub reset_time: Option<i64>,
    #[serde(alias = "presetIndex", default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub preset_index: Option<i64>,
}

/// 构造看守位的控制元素：
/// 开启时带 `<ResetTime>`/`<PresetIndex>`，关闭时只带 `<Enabled>0</Enabled>`。
pub(crate) fn build_home_position_element(
    enabled: bool,
    reset_time: Option<i64>,
    preset_index: Option<i64>,
) -> String {
    if enabled {
        format!(
            "<HomePosition>\n<Enabled>1</Enabled>\n<ResetTime>{}</ResetTime>\n<PresetIndex>{}</PresetIndex>\n</HomePosition>",
            reset_time.unwrap_or(0),
            preset_index.unwrap_or(0)
        )
    } else {
        "<HomePosition>\n<Enabled>0</Enabled>\n</HomePosition>".to_string()
    }
}

pub async fn device_home_position(
    State(state): State<AppState>,
    Query(q): Query<HomePositionQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let enabled = q.enabled.unwrap_or(false);
    let element = build_home_position_element(enabled, q.reset_time, q.preset_index);
    tracing::info!(
        "Device home_position: device={}, channel={}, enabled={}, resetTime={:?}, presetIndex={:?}",
        device_id,
        channel_id,
        enabled,
        q.reset_time,
        q.preset_index
    );
    send_control_element(
        &state,
        &device_id,
        &channel_id,
        &element,
        serde_json::json!({
            "enabled": enabled,
            "resetTime": q.reset_time,
            "presetIndex": q.preset_index,
        }),
    )
    .await
}

/// GET /api/device/control/drag_zoom/zoom_in | zoom_out —— 拉框放大/缩小
#[derive(Debug, Default, Deserialize)]
pub struct DragZoomQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub length: Option<i64>,
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub width: Option<i64>,
    #[serde(alias = "midPointX", default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub mid_point_x: Option<i64>,
    #[serde(alias = "midPointY", default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub mid_point_y: Option<i64>,
    #[serde(alias = "lengthX", default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub length_x: Option<i64>,
    #[serde(alias = "lengthY", default, deserialize_with = "crate::serde_flex::de_opt_i64")]
    pub length_y: Option<i64>,
}

/// 构造拉框放大/缩小的控制元素。
pub(crate) fn build_drag_zoom_element(
    zoom_in: bool,
    length: i64,
    width: i64,
    mid_point_x: i64,
    mid_point_y: i64,
    length_x: i64,
    length_y: i64,
) -> String {
    let tag = if zoom_in { "DragZoomIn" } else { "DragZoomOut" };
    format!(
        "<{tag}>\n<Length>{length}</Length>\n<Width>{width}</Width>\n<MidPointX>{mid_point_x}</MidPointX>\n<MidPointY>{mid_point_y}</MidPointY>\n<LengthX>{length_x}</LengthX>\n<LengthY>{length_y}</LengthY>\n</{tag}>"
    )
}

async fn drag_zoom(
    state: &AppState,
    q: &DragZoomQuery,
    zoom_in: bool,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    // 前端把六个参数都声明成 required；缺参时明确报错，而不是下发一个全 0 的框
    let missing: Vec<&str> = [
        ("length", q.length),
        ("width", q.width),
        ("midPointX", q.mid_point_x),
        ("midPointY", q.mid_point_y),
        ("lengthX", q.length_x),
        ("lengthY", q.length_y),
    ]
    .iter()
    .filter(|(_, v)| v.is_none())
    .map(|(k, _)| *k)
    .collect();
    if !missing.is_empty() {
        return Json(ApiResult::error(format!(
            "缺少参数: {}",
            missing.join(", ")
        )));
    }
    let element = build_drag_zoom_element(
        zoom_in,
        q.length.unwrap_or(0),
        q.width.unwrap_or(0),
        q.mid_point_x.unwrap_or(0),
        q.mid_point_y.unwrap_or(0),
        q.length_x.unwrap_or(0),
        q.length_y.unwrap_or(0),
    );
    tracing::info!(
        "Device drag_zoom({}): device={}, channel={}",
        if zoom_in { "in" } else { "out" },
        device_id,
        channel_id
    );
    send_control_element(
        state,
        &device_id,
        &channel_id,
        &element,
        serde_json::json!({ "zoomIn": zoom_in }),
    )
    .await
}

pub async fn device_drag_zoom_in(
    State(state): State<AppState>,
    Query(q): Query<DragZoomQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    drag_zoom(&state, &q, true).await
}

pub async fn device_drag_zoom_out(
    State(state): State<AppState>,
    Query(q): Query<DragZoomQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    drag_zoom(&state, &q, false).await
}

#[cfg(test)]
mod control_element_tests {
    use super::*;

    /// 报警复位：无条件带 `<AlarmCmd>ResetAlarm</AlarmCmd>`；
    /// 有 alarmMethod/alarmType 时才补 `<Info>`。
    #[test]
    fn alarm_reset_element_shape() {
        assert_eq!(
            build_alarm_reset_element(None, None),
            "<AlarmCmd>ResetAlarm</AlarmCmd>"
        );
        // 空串按"未提供"，不能生成空元素
        assert_eq!(
            build_alarm_reset_element(Some(""), Some("  ")),
            "<AlarmCmd>ResetAlarm</AlarmCmd>"
        );
        let xml = build_alarm_reset_element(Some("5"), Some("1"));
        assert!(xml.contains("<AlarmCmd>ResetAlarm</AlarmCmd>"), "{xml}");
        assert!(xml.contains("<Info>") && xml.contains("</Info>"), "{xml}");
        assert!(xml.contains("<AlarmMethod>5</AlarmMethod>"), "{xml}");
        assert!(xml.contains("<AlarmType>1</AlarmType>"), "{xml}");
    }

    /// 看守位：开启时带 resetTime/presetIndex，关闭时只带 Enabled=0
    /// （元素名与字段顺序固定为下述形状）。
    #[test]
    fn home_position_element_shape() {
        let on = build_home_position_element(true, Some(30), Some(2));
        assert_eq!(
            on,
            "<HomePosition>\n<Enabled>1</Enabled>\n<ResetTime>30</ResetTime>\n<PresetIndex>2</PresetIndex>\n</HomePosition>"
        );
        let off = build_home_position_element(false, Some(30), Some(2));
        assert_eq!(
            off,
            "<HomePosition>\n<Enabled>0</Enabled>\n</HomePosition>"
        );
        // 缺省值补 0，不能出现空标签
        let defaulted = build_home_position_element(true, None, None);
        assert!(defaulted.contains("<ResetTime>0</ResetTime>"), "{defaulted}");
        assert!(defaulted.contains("<PresetIndex>0</PresetIndex>"), "{defaulted}");
    }

    /// 拉框放大/缩小的元素名与六个字段必须保持下述形状。
    #[test]
    fn drag_zoom_element_shape() {
        let zin = build_drag_zoom_element(true, 100, 200, 1, 2, 3, 4);
        assert!(zin.starts_with("<DragZoomIn>"), "{zin}");
        assert!(zin.ends_with("</DragZoomIn>"), "{zin}");
        for frag in [
            "<Length>100</Length>",
            "<Width>200</Width>",
            "<MidPointX>1</MidPointX>",
            "<MidPointY>2</MidPointY>",
            "<LengthX>3</LengthX>",
            "<LengthY>4</LengthY>",
        ] {
            assert!(zin.contains(frag), "{zin} 缺 {frag}");
        }
        let zout = build_drag_zoom_element(false, 1, 2, 3, 4, 5, 6);
        assert!(zout.starts_with("<DragZoomOut>") && zout.ends_with("</DragZoomOut>"), "{zout}");
    }

    /// 前端把开关放在查询串里时可能是 `"true"`/`"1"`，都必须能反序列化。
    #[test]
    fn home_position_query_accepts_flexible_bool() {
        let parse = |v: serde_json::Value| -> HomePositionQuery {
            serde_json::from_value(v).unwrap()
        };
        assert_eq!(
            parse(serde_json::json!({"deviceId": "d", "enabled": true})).enabled,
            Some(true)
        );
        assert_eq!(
            parse(serde_json::json!({"deviceId": "d", "enabled": "true"})).enabled,
            Some(true)
        );
        assert_eq!(
            parse(serde_json::json!({"deviceId": "d", "enabled": 1})).enabled,
            Some(true)
        );
        assert_eq!(
            parse(serde_json::json!({"deviceId": "d", "enabled": "0"})).enabled,
            Some(false)
        );
        assert_eq!(parse(serde_json::json!({"deviceId": "d"})).enabled, None);

        let q = parse(serde_json::json!({
            "deviceId": "d", "channelId": "c",
            "enabled": "1", "resetTime": "30", "presetIndex": 2
        }));
        assert_eq!(q.reset_time, Some(30));
        assert_eq!(q.preset_index, Some(2));

        // 拉框缩放的六个数字同样接受字符串
        let dz: DragZoomQuery = serde_json::from_value(serde_json::json!({
            "deviceId": "d", "channelId": "c",
            "length": "100", "width": 200, "midPointX": "1",
            "midPointY": 2, "lengthX": "3", "lengthY": 4
        }))
        .unwrap();
        assert_eq!(dz.length, Some(100));
        assert_eq!(dz.width, Some(200));
        assert_eq!(dz.mid_point_x, Some(1));
        assert_eq!(dz.length_y, Some(4));
    }
}

// ============================================================================
// 设备配置的查询与下发端点。
//
// 这里用一组语义化路径（而非本平台早期的 `?configType=`）：
//   GET /api/device/config/query/{basicParam,videoParamOpt,svacEncodeConfig,svacDecodeConfig}
//   GET /api/device/config/set/{basicParam,videoParamOpt}
// 前端（通道/设备配置弹窗）直接按这些路径调用，此前全部 404。
// 返回值给出**解析后的字段**形状（同时保留原始 XML，便于排查）。
// ============================================================================

/// 从 ConfigDownload 应答 XML 里抽出基本配置字段。
pub(crate) fn parse_basic_param_xml(xml: &str) -> serde_json::Value {
    use crate::sip::gb28181::xml_parser::XmlParser;
    let mut obj = serde_json::Map::new();
    for tag in [
        "Name",
        "Manufacturer",
        "Model",
        "Firmware",
        "Expiration",
        "HeartBeatInterval",
        "HeartBeatCount",
        "ConfigType",
    ] {
        if let Some(v) = XmlParser::find_first_element(xml, tag) {
            obj.insert(tag.to_string(), serde_json::Value::String(v));
        }
    }
    serde_json::Value::Object(obj)
}

/// 从 ConfigDownload 应答 XML 里抽出视频参数（`<VideoParamOpt>`）。
pub(crate) fn parse_video_param_xml(xml: &str) -> serde_json::Value {
    use crate::sip::gb28181::xml_parser::XmlParser;
    let mut obj = serde_json::Map::new();
    for tag in ["Resolution", "DownloadSpeed", "VideoFormat"] {
        if let Some(v) = XmlParser::find_first_element(xml, tag) {
            obj.insert(tag.to_string(), serde_json::Value::String(v));
        }
    }
    serde_json::Value::Object(obj)
}

/// 统一的设备配置查询实现：`config_type` 是国标 ConfigType
/// （BasicParam / VideoParamOpt / SVACEncodeConfig / SVACDecodeConfig）。
pub(crate) async fn config_query(
    state: &AppState,
    device_id: &str,
    channel_id: Option<&str>,
    config_type: &str,
) -> Json<ApiResult<serde_json::Value>> {
    if device_id.trim().is_empty() {
        return Json(ApiResult::error("deviceId 必须存在"));
    }
    let raw = query_config_and_wait(state, device_id, config_type).await;
    // 设备没应答（离线/超时/发送失败）时如实返回错误原因，不要假装查到了配置
    if let Some(status) = raw.get("status").and_then(|v| v.as_str()) {
        let msg = raw
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or(status);
        return Json(ApiResult::error(format!("查询设备配置失败: {msg}")));
    }
    let xml = raw.get("xml").and_then(|v| v.as_str()).unwrap_or("");
    let parsed = match config_type {
        "BasicParam" => parse_basic_param_xml(xml),
        "VideoParamOpt" => parse_video_param_xml(xml),
        _ => serde_json::json!({}),
    };
    let mut data = serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "configType": config_type,
        "xml": xml,
        "source": raw.get("source").cloned().unwrap_or(serde_json::json!("live")),
    });
    if let (Some(dst), Some(src)) = (data.as_object_mut(), parsed.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    Json(ApiResult::success(data))
}

#[derive(Debug, Default, Deserialize)]
pub struct ConfigQueryParams {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
}

pub async fn config_query_basic_param(
    State(state): State<AppState>,
    Query(q): Query<ConfigQueryParams>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    config_query(&state, &device_id, q.channel_id.as_deref(), "BasicParam").await
}

pub async fn config_query_video_param(
    State(state): State<AppState>,
    Query(q): Query<ConfigQueryParams>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    config_query(&state, &device_id, q.channel_id.as_deref(), "VideoParamOpt").await
}

pub async fn config_query_svac_encode(
    State(state): State<AppState>,
    Query(q): Query<ConfigQueryParams>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    config_query(&state, &device_id, q.channel_id.as_deref(), "SVACEncodeConfig").await
}

pub async fn config_query_svac_decode(
    State(state): State<AppState>,
    Query(q): Query<ConfigQueryParams>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    config_query(&state, &device_id, q.channel_id.as_deref(), "SVACDecodeConfig").await
}

/// `GET /api/device/config/set/basicParam` 的查询参数。
#[derive(Debug, Default, Deserialize)]
pub struct BasicParamQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub expiration: Option<String>,
    #[serde(alias = "heartBeatInterval", default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub heart_beat_interval: Option<String>,
    #[serde(alias = "heartBeatCount", default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub heart_beat_count: Option<String>,
}

/// 基本配置下发的控制元素（只发非空项）。
pub(crate) fn build_basic_param_set_element(q: &BasicParamQuery) -> String {
    let mut xml = String::from("<BasicParam>");
    if let Some(v) = q.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        xml.push_str(&format!("\n<Name>{}</Name>", v));
    }
    if let Some(v) = q
        .expiration
        .as_deref()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|n| *n > 0)
    {
        xml.push_str(&format!("\n<Expiration>{}</Expiration>", v));
    }
    if let Some(v) = q
        .heart_beat_interval
        .as_deref()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|n| *n > 0)
    {
        xml.push_str(&format!("\n<HeartBeatInterval>{}</HeartBeatInterval>", v));
    }
    if let Some(v) = q
        .heart_beat_count
        .as_deref()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|n| *n > 0)
    {
        xml.push_str(&format!("\n<HeartBeatCount>{}</HeartBeatCount>", v));
    }
    xml.push_str("\n</BasicParam>");
    xml
}

/// `GET /api/device/config/set/videoParamOpt` 的查询参数。
#[derive(Debug, Default, Deserialize)]
pub struct VideoParamOptQuery {
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    pub resolution: Option<String>,
    #[serde(alias = "downloadSpeed", default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub download_speed: Option<String>,
}

pub(crate) fn build_video_param_set_element(q: &VideoParamOptQuery) -> String {
    let mut xml = String::from("<VideoParamOpt>");
    if let Some(v) = q.resolution.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        xml.push_str(&format!("\n<Resolution>{}</Resolution>", v));
    }
    if let Some(v) = q
        .download_speed
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        xml.push_str(&format!("\n<DownloadSpeed>{}</DownloadSpeed>", v));
    }
    xml.push_str("\n</VideoParamOpt>");
    xml
}

pub async fn config_set_basic_param(
    State(state): State<AppState>,
    Query(q): Query<BasicParamQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    if device_id.trim().is_empty() {
        return Json(ApiResult::error("设备ID必须存在"));
    }
    let element = build_basic_param_set_element(&q);
    // BasicParam 一律用**设备编码**（大华设备必须用设备 ID）
    send_control_element_with_type(
        &state,
        &device_id,
        &device_id,
        "DeviceConfig",
        &element,
        serde_json::json!({ "configType": "BasicParam" }),
    )
    .await
}

pub async fn config_set_video_param(
    State(state): State<AppState>,
    Query(q): Query<VideoParamOptQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    if device_id.trim().is_empty() {
        return Json(ApiResult::error("设备ID必须存在"));
    }
    let element = build_video_param_set_element(&q);
    send_control_element_with_type(
        &state,
        &device_id,
        &device_id,
        "DeviceConfig",
        &element,
        serde_json::json!({ "configType": "VideoParamOpt" }),
    )
    .await
}

/// 与 [`send_control_element`] 相同，但允许指定 `CmdType`
/// （设备配置下发用 `DeviceConfig` 而不是 `DeviceControl`）。
pub(crate) async fn send_control_element_with_type(
    state: &AppState,
    device_id: &str,
    channel_id: &str,
    cmd_type: &str,
    element: &str,
    extra: serde_json::Value,
) -> Json<ApiResult<serde_json::Value>> {
    if device_id.is_empty() {
        return Json(ApiResult::error("device_id is required"));
    }
    let Some(ref sip_server) = state.sip_server else {
        return Json(ApiResult::error("SIP server not available"));
    };
    let server = &**sip_server;
    let Some(device) = server.device_manager().get(device_id).await else {
        return Json(ApiResult::error(format!("设备不存在或未注册: {device_id}")));
    };
    if !device.online || device.addr.is_none() {
        return Json(ApiResult::error(format!("设备不在线: {device_id}")));
    }
    if let Err(e) = server
        .send_device_control(device_id, channel_id, cmd_type, element)
        .await
    {
        tracing::error!("{cmd_type} 下发失败 device={}: {}", device_id, e);
        return Json(ApiResult::error(format!("命令发送失败: {e}")));
    }
    let mut data = serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "result": "command sent",
        "xml": element,
    });
    if let (Some(obj), Some(extra_obj)) = (data.as_object_mut(), extra.as_object()) {
        for (k, v) in extra_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    Json(ApiResult::success(data))
}
