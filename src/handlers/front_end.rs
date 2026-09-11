//! Front-end control API /api/front-end, matching frontEnd.js
//! PTZ, preset, cruise, scan, auxiliary, wiper, iris, focus controls via SIP

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::sip::gb28181::front_end_control::{build_ptz_cmd, FiAction, PresetAction, PtzAction};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PtzQuery {
    /// WVP 的查询参数名是 `command`；老版前端（含本仓库 Vue3 直播页）发的是 `cmd`。
    #[serde(alias = "cmd")]
    pub command: Option<String>,
    #[serde(alias = "horizonSpeed")]
    pub horizon_speed: Option<i32>,
    #[serde(alias = "verticalSpeed")]
    pub vertical_speed: Option<i32>,
    #[serde(alias = "zoomSpeed")]
    pub zoom_speed: Option<i32>,
    pub speed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    #[serde(alias = "scanId")]
    pub scan_id: Option<String>,
    pub speed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CruiseQuery {
    #[serde(alias = "cruiseId")]
    pub cruise_id: Option<String>,
    #[serde(alias = "presetId")]
    pub preset_id: Option<i32>,
    #[serde(alias = "cruiseSpeed")]
    pub cruise_speed: Option<i32>,
    #[serde(alias = "cruiseTime")]
    pub cruise_time: Option<i32>,
    pub speed: Option<i32>,
    pub time: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct PresetQuery {
    /// WVP 的查询参数名是 `presetId`（camelCase）
    #[serde(alias = "presetId")]
    pub preset_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct AuxiliaryQuery {
    #[serde(alias = "cmd")]
    pub command: Option<String>,
    #[serde(alias = "switchId")]
    pub switch_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct WiperQuery {
    #[serde(alias = "cmd")]
    pub command: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LegacyFrontEndCommandQuery {
    #[serde(alias = "cmdCode")]
    pub cmd_code: Option<i32>,
    pub parameter1: Option<i32>,
    pub parameter2: Option<i32>,
    #[serde(alias = "combindCode2")]
    pub combind_code2: Option<i32>,
}

/// 云台/变倍：**唯一**实现见 `sip::gb28181::front_end_control`（8 字节 + 校验和）。
///
/// 此前这里自己拼了一个 7 字节的 `0501000000ssFF`：长度不对、没有累加校验、
/// 指令码位置也不是国标定义的位置，设备收到等于乱码 —— 而 handler 还返回
/// `code:0`「已下发」，前端于是提示成功、云台不动。
///
/// 速度取值：WVP 的查询参数是 `horizonSpeed`/`verticalSpeed`/`zoomSpeed`，
/// 国标里水平/垂直速度各一个字节、变倍速度放在组合码2 高 4 位。前端只给一个
/// `speed` 时三个方向共用它（老前端就是只发 `speed`）。
fn ptz_speed(q: &PtzQuery, action: PtzAction) -> u8 {
    let fallback = q.speed.unwrap_or(50);
    let raw = match action {
        PtzAction::ZoomIn | PtzAction::ZoomOut => q.zoom_speed.or(q.horizon_speed).unwrap_or(fallback),
        _ => q.horizon_speed.or(q.vertical_speed).unwrap_or(fallback),
    };
    raw.clamp(0, 255) as u8
}

/// 聚焦/光圈：国标里是**独立的 `<FICmd>` 元素**，不能塞进 `PTZCmd`。
///
/// 返回 `(元素名, 元素内容)`，交给 `send_via_sip` 包进 `<Control>`。
fn fi_element(cmd: &str) -> Option<(&'static str, String)> {
    FiAction::parse(cmd).map(|fi| ("FICmd", fi.as_cmd_value().to_string()))
}

/// 预置位：`<PresetCmd>` + `<PresetIndex>`。
fn preset_elements(cmd: &str, preset_index: u32) -> Option<String> {
    let action = PresetAction::parse(cmd)?;
    Some(format!(
        "<PresetCmd>{}</PresetCmd><PresetIndex>{}</PresetIndex>",
        action.as_cmd_value(),
        preset_index
    ))
}

/// 兼容端点用：调用方已经算好了 4 个字节，这里补 `A5 0F 01` 前缀与**累加校验**。
fn build_raw_front_end_xml(cmd_code: i32, parameter1: i32, parameter2: i32, combind_code2: i32) -> String {
    let body = [
        0xA5u8,
        0x0F,
        0x01,
        (cmd_code & 0xff) as u8,
        (parameter1 & 0xff) as u8,
        (parameter2 & 0xff) as u8,
        (combind_code2 & 0xff) as u8,
    ];
    let checksum = body.iter().fold(0u8, |acc, x| acc.wrapping_add(*x));
    let mut bytes = body.to_vec();
    bytes.push(checksum);
    let hex: String = bytes.iter().map(|x| format!("{:02X}", x)).collect();
    format!(r#"<PTZCmd>{}</PTZCmd>"#, hex)
}

fn build_auxiliary_xml(command: &str, switch_id: u32) -> String {
    let aux_cmd = if command.to_lowercase() == "on" { "Set" } else { "Reset" };
    format!(r#"<AuxiliaryCmd><cmd>{}</cmd><index>{}</index></AuxiliaryCmd>"#, aux_cmd, switch_id)
}

fn build_wiper_xml(command: &str) -> String {
    let wiper_cmd = if command.to_lowercase() == "on" { "Open" } else { "Close" };
    format!(r#"<WiperCmd>{}</WiperCmd>"#, wiper_cmd)
}

fn build_scan_xml(cmd: &str, scan_id: u32, speed: u8) -> String {
    match cmd {
        "setSpeed" => format!(r#"<ScanSpeed id="{}" speed="{}" />"#, scan_id, speed),
        "setLeft" => format!(r#"<ScanSet id="{}" type="left" />"#, scan_id),
        "setRight" => format!(r#"<ScanSet id="{}" type="right" />"#, scan_id),
        "start" => format!(r#"<ScanCmd id="{}" action="start" />"#, scan_id),
        "stop" => format!(r#"<ScanCmd id="{}" action="stop" />"#, scan_id),
        _ => format!(r#"<ScanCmd id="{}" />"#, scan_id),
    }
}

fn build_cruise_xml(cmd: &str, cruise_id: u32, preset_id: u32, speed: u8, time: u32) -> String {
    match cmd {
        "addPoint" => format!(r#"<CruiseCmd id="{}" preset="{}" action="add" />"#, cruise_id, preset_id),
        "deletePoint" => format!(r#"<CruiseCmd id="{}" preset="{}" action="delete" />"#, cruise_id, preset_id),
        "speed" => format!(r#"<CruiseSpeed id="{}" speed="{}" />"#, cruise_id, speed),
        "time" => format!(r#"<CruiseTime id="{}" time="{}" />"#, cruise_id, time),
        "start" => format!(r#"<CruiseCmd id="{}" action="start" />"#, cruise_id),
        "stop" => format!(r#"<CruiseCmd id="{}" action="stop" />"#, cruise_id),
        _ => format!(r#"<CruiseCmd id="{}" />"#, cruise_id),
    }
}

async fn send_via_sip(
    state: &AppState,
    device_id: &str,
    channel_id: &str,
    cmd_type: &str,
    body: &str,
) -> Result<(), String> {
    if let Some(ref sip_server) = state.sip_server {
        let server = &*sip_server;
        if let Some(device) = server.device_manager().get(device_id).await {
            if device.online && device.addr.is_some() {
                return match server.send_device_control(device_id, channel_id, cmd_type, body).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("SIP send failed: {}", e)),
                };
            }
        }
    }
    Err("Device not online or SIP not initialized".to_string())
}

fn success_json(msg: &str) -> serde_json::Value {
    serde_json::json!({ "code": 0, "msg": msg })
}

// ========== PTZ ==========
pub async fn ptz(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<PtzQuery>,
) -> Json<serde_json::Value> {
    let command = q.command.clone().unwrap_or_default();

    // 认不出的命令**必须报错**：此前落到兜底分支，下发一个方向位全 0 的
    // "无动作"命令却返回成功，用户看到"已下发"但云台不动。
    let Some(action) = PtzAction::parse(&command) else {
        return Json(serde_json::json!({
            "code": 1,
            "msg": format!("不支持的云台命令: {command:?}（可用: up/down/left/right/zoom_in/zoom_out/stop）")
        }));
    };
    let speed = ptz_speed(&q, action);

    tracing::info!(
        "PTZ control: device={}, channel={}, cmd={}, action={:?}, speed={}",
        device_id, channel_id, command, action, speed
    );

    let body = format!("<PTZCmd>{}</PTZCmd>", build_ptz_cmd(action, speed));
    match send_via_sip(&state, &device_id, &channel_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("PTZ 控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

/// POST /api/ptz/front_end_command/:device_id/:channel_id
///
/// Compatibility endpoint used by older player components. They already
/// calculate GB28181 front-end command bytes and pass them as decimal query
/// parameters, so this handler only wraps those bytes in a DeviceControl XML.
pub async fn legacy_front_end_command(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<LegacyFrontEndCommandQuery>,
) -> Json<serde_json::Value> {
    let cmd_code = q.cmd_code.unwrap_or(0);
    let parameter1 = q.parameter1.unwrap_or(0);
    let parameter2 = q.parameter2.unwrap_or(0);
    let combind_code2 = q.combind_code2.unwrap_or(0);

    tracing::info!(
        "Legacy front-end command: device={}, channel={}, cmd={}, p1={}, p2={}, c2={}",
        device_id, channel_id, cmd_code, parameter1, parameter2, combind_code2
    );

    let body = build_raw_front_end_xml(cmd_code, parameter1, parameter2, combind_code2);
    match send_via_sip(&state, &device_id, &channel_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("前端控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Auxiliary ==========
pub async fn auxiliary(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<AuxiliaryQuery>,
) -> Json<serde_json::Value> {
    let command = q.command.clone().unwrap_or_default();
    let switch_id = q.switch_id.unwrap_or(0) as u32;

    tracing::info!(
        "Auxiliary control: device={}, channel={}, cmd={}, switch={}",
        device_id, channel_id, command, switch_id
    );

    let body = build_auxiliary_xml(&command, switch_id);
    match send_via_sip(&state, &device_id, &channel_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("辅助开关控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Wiper ==========
pub async fn wiper(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<WiperQuery>,
) -> Json<serde_json::Value> {
    let command = q.command.clone().unwrap_or_default();

    tracing::info!("Wiper control: device={}, channel={}, cmd={}", device_id, channel_id, command);

    let body = build_wiper_xml(&command);
    match send_via_sip(&state, &device_id, &channel_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("雨刷控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Iris ==========
pub async fn iris(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
    Query(q): Query<PtzQuery>,
) -> Json<serde_json::Value> {
    let command = q.command.clone().unwrap_or_default();

    // 国标 2016：聚焦/光圈是 <FICmd> 独立元素，内容为 IrisOpen/IrisClose。
    // 老前端发的 "on"/"off"/"open"/"close" 都归一到同一个元素值。
    let lower = command.to_ascii_lowercase();
    let normalized: &str = match lower.as_str() {
        "on" | "open" | "iris_in" => "IRIS_OPEN",
        "off" | "close" | "iris_out" => "IRIS_CLOSE",
        other => other,
    };
    let Some((elem, value)) = fi_element(normalized) else {
        return Json(serde_json::json!({
            "code": 1,
            "msg": format!("不支持的光圈命令: {command:?}（可用: on/off/open/close）")
        }));
    };

    tracing::info!(
        "Iris control: device={}, channel={}, cmd={}, element={}",
        device_id, channel_id, command, elem
    );

    let body = format!("<{elem}>{value}</{elem}>");
    match send_via_sip(&state, &device_id, &channel_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("光圈控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Focus ==========
pub async fn focus(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<PtzQuery>,
) -> Json<serde_json::Value> {
    let command = q.command.clone().unwrap_or_default();

    let lower = command.to_ascii_lowercase();
    let normalized: &str = match lower.as_str() {
        "on" | "open" | "focus_in" => "FOCUS_IN",
        "off" | "close" | "focus_out" => "FOCUS_OUT",
        other => other,
    };
    let Some((elem, value)) = fi_element(normalized) else {
        return Json(serde_json::json!({
            "code": 1,
            "msg": format!("不支持的聚焦命令: {command:?}（可用: on/off/open/close）")
        }));
    };

    tracing::info!(
        "Focus control: device={}, channel={}, cmd={}, element={}",
        device_id, channel_device_id, command, elem
    );

    let body = format!("<{elem}>{value}</{elem}>");
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("焦距控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Preset ==========
pub async fn preset_query(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
) -> Json<serde_json::Value> {
    tracing::info!("Preset query: device={}, channel={}", device_id, channel_device_id);

    let body = r#"<PTZCmd Query="PresetList"/>"#.to_string();
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(serde_json::json!({
            "code": 0,
            "data": [],
            "msg": "预置位查询命令已发送"
        })),
        Err(_) => Json(serde_json::json!({
            "code": 0,
            "data": [],
            "msg": "设备不在线，返回空列表"
        })),
    }
}

pub async fn preset_add(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<PresetQuery>,
) -> Json<serde_json::Value> {
    let preset_id = q.preset_id.unwrap_or(0);

    tracing::info!(
        "Preset add: device={}, channel={}, preset={}",
        device_id, channel_device_id, preset_id
    );

    let Some(body) = preset_elements("SET_PRESET", preset_id as u32) else {
        return Json(serde_json::json!({ "code": 1, "msg": "预置位命令构造失败" }));
    };
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("预置位添加成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn preset_call(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<PresetQuery>,
) -> Json<serde_json::Value> {
    let preset_id = q.preset_id.unwrap_or(0);

    tracing::info!(
        "Preset call: device={}, channel={}, preset={}",
        device_id, channel_device_id, preset_id
    );

    let Some(body) = preset_elements("GOTO_PRESET", preset_id as u32) else {
        return Json(serde_json::json!({ "code": 1, "msg": "预置位命令构造失败" }));
    };
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("预置位调用成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn preset_delete(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<PresetQuery>,
) -> Json<serde_json::Value> {
    let preset_id = q.preset_id.unwrap_or(0);

    tracing::info!(
        "Preset delete: device={}, channel={}, preset={}",
        device_id, channel_device_id, preset_id
    );

    let Some(body) = preset_elements("CLEAR_PRESET", preset_id as u32) else {
        return Json(serde_json::json!({ "code": 1, "msg": "预置位命令构造失败" }));
    };
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("预置位删除成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Cruise ==========
pub async fn cruise_point_add(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<CruiseQuery>,
) -> Json<serde_json::Value> {
    let cruise_id = q.cruise_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0) as u32;

    tracing::info!(
        "Cruise point add: device={}, channel={}, cruise={}, preset={}",
        device_id, channel_device_id, cruise_id, preset_id
    );

    let body = build_cruise_xml("addPoint", cruise_id, preset_id, 0, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("巡航点添加成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn cruise_point_delete(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<CruiseQuery>,
) -> Json<serde_json::Value> {
    let cruise_id = q.cruise_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);
    let preset_id = q.preset_id.unwrap_or(0) as u32;

    tracing::info!(
        "Cruise point delete: device={}, channel={}, cruise={}, preset={}",
        device_id, channel_device_id, cruise_id, preset_id
    );

    let body = build_cruise_xml("deletePoint", cruise_id, preset_id, 0, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("巡航点删除成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn cruise_speed(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<CruiseQuery>,
) -> Json<serde_json::Value> {
    let cruise_id = q.cruise_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);
    let speed = q.speed.unwrap_or(1) as u8;

    tracing::info!(
        "Cruise speed: device={}, channel={}, cruise={}, speed={}",
        device_id, channel_device_id, cruise_id, speed
    );

    let body = build_cruise_xml("speed", cruise_id, 0, speed, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("巡航速度设置成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn cruise_time(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<CruiseQuery>,
) -> Json<serde_json::Value> {
    let cruise_id = q.cruise_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);
    let time = q.time.unwrap_or(10) as u32;

    tracing::info!(
        "Cruise time: device={}, channel={}, cruise={}, time={}",
        device_id, channel_device_id, cruise_id, time
    );

    let body = build_cruise_xml("time", cruise_id, 0, 0, time);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("巡航时间设置成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn cruise_start(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<CruiseQuery>,
) -> Json<serde_json::Value> {
    let cruise_id = q.cruise_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);

    tracing::info!(
        "Cruise start: device={}, channel={}, cruise={}",
        device_id, channel_device_id, cruise_id
    );

    let body = build_cruise_xml("start", cruise_id, 0, 0, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("巡航启动成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn cruise_stop(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<CruiseQuery>,
) -> Json<serde_json::Value> {
    let cruise_id = q.cruise_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);

    tracing::info!(
        "Cruise stop: device={}, channel={}, cruise={}",
        device_id, channel_device_id, cruise_id
    );

    let body = build_cruise_xml("stop", cruise_id, 0, 0, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("巡航停止成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

// ========== Scan ==========
pub async fn scan_set_speed(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<ScanQuery>,
) -> Json<serde_json::Value> {
    let scan_id = q.scan_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);
    let speed = q.speed.unwrap_or(1) as u8;

    tracing::info!(
        "Scan set speed: device={}, channel={}, scan={}, speed={}",
        device_id, channel_device_id, scan_id, speed
    );

    let body = build_scan_xml("setSpeed", scan_id, speed);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("扫描速度设置成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn scan_set_left(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<ScanQuery>,
) -> Json<serde_json::Value> {
    let scan_id = q.scan_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);

    tracing::info!(
        "Scan set left: device={}, channel={}, scan={}",
        device_id, channel_device_id, scan_id
    );

    let body = build_scan_xml("setLeft", scan_id, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("左边界设置成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn scan_set_right(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<ScanQuery>,
) -> Json<serde_json::Value> {
    let scan_id = q.scan_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);

    tracing::info!(
        "Scan set right: device={}, channel={}, scan={}",
        device_id, channel_device_id, scan_id
    );

    let body = build_scan_xml("setRight", scan_id, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("右边界设置成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn scan_start(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<ScanQuery>,
) -> Json<serde_json::Value> {
    let scan_id = q.scan_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);

    tracing::info!(
        "Scan start: device={}, channel={}, scan={}",
        device_id, channel_device_id, scan_id
    );

    let body = build_scan_xml("start", scan_id, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("扫描启动成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

pub async fn scan_stop(
    State(state): State<AppState>,
    Path((device_id, channel_device_id)): Path<(String, String)>,
    Query(q): Query<ScanQuery>,
) -> Json<serde_json::Value> {
    let scan_id = q.scan_id.clone().unwrap_or_default().parse::<u32>().unwrap_or(0);

    tracing::info!(
        "Scan stop: device={}, channel={}, scan={}",
        device_id, channel_device_id, scan_id
    );

    let body = build_scan_xml("stop", scan_id, 0);
    match send_via_sip(&state, &device_id, &channel_device_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("扫描停止成功")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

#[cfg(test)]
mod front_end_wire_tests {
    use super::*;

    fn q(pairs: &[(&str, i32)]) -> PtzQuery {
        // 直接构造，避免依赖 serde_urlencoded（query 参数名由 axum 的 Query 解析，
        // serde alias 的正确性另有 `ptz_query_accepts_cmd_alias` 覆盖）
        let mut out = PtzQuery {
            command: None,
            horizon_speed: None,
            vertical_speed: None,
            zoom_speed: None,
            speed: None,
        };
        for (k, v) in pairs {
            match *k {
                "horizonSpeed" => out.horizon_speed = Some(*v),
                "verticalSpeed" => out.vertical_speed = Some(*v),
                "zoomSpeed" => out.zoom_speed = Some(*v),
                "speed" => out.speed = Some(*v),
                _ => {}
            }
        }
        out
    }

    /// 老前端只发 `speed`，三个方向共用；WVP 的三个独立参数优先。
    #[test]
    fn test_ptz_speed_selection() {
        assert_eq!(ptz_speed(&q(&[("speed", 50)]), PtzAction::Up), 50);
        assert_eq!(
            ptz_speed(&q(&[("horizonSpeed", 200), ("speed", 50)]), PtzAction::Left),
            200
        );
        assert_eq!(
            ptz_speed(&q(&[("zoomSpeed", 7), ("speed", 50)]), PtzAction::ZoomIn),
            7
        );
        // 垂直动作也吃 horizonSpeed（前端只有一个滑块）
        assert_eq!(ptz_speed(&q(&[("horizonSpeed", 33)]), PtzAction::Down), 33);
        // 越界钳制
        assert_eq!(ptz_speed(&q(&[("speed", 9999)]), PtzAction::Up), 255);
        assert_eq!(ptz_speed(&q(&[("speed", -5)]), PtzAction::Up), 0);
    }

    /// 云台命令必须是**国标 8 字节 + 校验和**，不能是此前的 7 字节拼串。
    #[test]
    fn test_ptz_body_is_standard_8_bytes() {
        let body = format!("<PTZCmd>{}</PTZCmd>", build_ptz_cmd(PtzAction::Up, 50));
        // A5 0F 01 08 00 32 00 + sum(A5,0F,01,08,00,32,00)=EF
        assert_eq!(body, "<PTZCmd>A50F0108003200EF</PTZCmd>");
        let hex = &body[8..24];
        assert_eq!(hex.len(), 16, "必须 8 字节");
        assert!(hex.starts_with("A5"));
        let bytes: Vec<u8> = (0..8)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        let sum = bytes[..7].iter().fold(0u8, |a, b| a.wrapping_add(*b));
        assert_eq!(bytes[7], sum, "末字节必须是前 7 字节累加和");
    }

    /// 聚焦/光圈必须用 `<FICmd>`（国标独立元素），不能塞进 `PTZCmd`。
    #[test]
    fn test_fi_uses_separate_element() {
        assert_eq!(fi_element("IRIS_OPEN"), Some(("FICmd", "IrisOpen".to_string())));
        assert_eq!(fi_element("FOCUS_OUT"), Some(("FICmd", "FocusFar".to_string())));
        assert_eq!(fi_element("bogus"), None);
    }

    /// 预置位用 `<PresetCmd>` + `<PresetIndex>`，且动作名是国标枚举值。
    #[test]
    fn test_preset_elements() {
        assert_eq!(
            preset_elements("SET_PRESET", 5).unwrap(),
            "<PresetCmd>SetPreset</PresetCmd><PresetIndex>5</PresetIndex>"
        );
        assert_eq!(
            preset_elements("GOTO_PRESET", 7).unwrap(),
            "<PresetCmd>CallPreset</PresetCmd><PresetIndex>7</PresetIndex>"
        );
        assert_eq!(
            preset_elements("CLEAR_PRESET", 3).unwrap(),
            "<PresetCmd>DelPreset</PresetCmd><PresetIndex>3</PresetIndex>"
        );
    }

    /// 兼容端点：调用方给的 4 个字节要补 A5 0F 01 前缀并**计算校验和**。
    #[test]
    fn test_raw_front_end_command_has_checksum() {
        let xml = build_raw_front_end_xml(0x08, 0x00, 0x1F, 0x00);
        let hex = xml.trim_start_matches("<PTZCmd>").trim_end_matches("</PTZCmd>");
        let bytes: Vec<u8> = (0..8)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        assert_eq!(&bytes[..4], &[0xA5, 0x0F, 0x01, 0x08]);
        assert_eq!(&bytes[4..7], &[0x00, 0x1F, 0x00]);
        assert_eq!(bytes[7], bytes[..7].iter().fold(0u8, |a, b| a.wrapping_add(*b)));
    }
}

#[cfg(test)]
mod front_end_query_alias_tests {
    use super::*;

    /// 前端（含 WVP 的 frontEnd.js）一律发 camelCase。缺 alias 时 serde 会**静默**
    /// 丢弃参数——例如预置位编号绑不上，设备收到的是 `PresetIndex=0`。
    /// 这里逐个端点的 DTO 用 camelCase 键反序列化一遍。
    ///
    /// 说明：用 serde_json 驱动同一个 `Deserialize` 实现，对 alias 的验证与
    /// axum 的 `Query`（serde_urlencoded）等价。
    #[test]
    fn test_ptz_query_aliases() {
        let q: PtzQuery =
            serde_json::from_value(serde_json::json!({"cmd": "up", "horizonSpeed": 31, "speed": 9}))
                .unwrap();
        assert_eq!(q.command.as_deref(), Some("up"), "`cmd` 必须能绑到 command");
        assert_eq!(q.horizon_speed, Some(31));
        assert_eq!(q.speed, Some(9));

        let q: PtzQuery =
            serde_json::from_value(serde_json::json!({"command": "left", "zoomSpeed": 7})).unwrap();
        assert_eq!(q.command.as_deref(), Some("left"));
        assert_eq!(q.zoom_speed, Some(7));
    }

    #[test]
    fn test_preset_query_aliases() {
        let q: PresetQuery = serde_json::from_value(serde_json::json!({"presetId": 5})).unwrap();
        assert_eq!(q.preset_id, Some(5), "`presetId` 必须能绑到 preset_id");
        let q: PresetQuery = serde_json::from_value(serde_json::json!({"preset_id": 6})).unwrap();
        assert_eq!(q.preset_id, Some(6));
    }

    #[test]
    fn test_cruise_and_scan_query_aliases() {
        let q: CruiseQuery = serde_json::from_value(serde_json::json!({
            "cruiseId": "3", "presetId": 8, "cruiseSpeed": 2, "cruiseTime": 15, "speed": 4, "time": 20
        }))
        .unwrap();
        assert_eq!(q.cruise_id.as_deref(), Some("3"));
        assert_eq!(q.preset_id, Some(8));
        assert_eq!(q.cruise_speed, Some(2));
        assert_eq!(q.cruise_time, Some(15));

        let q: ScanQuery =
            serde_json::from_value(serde_json::json!({"scanId": "2", "speed": 5})).unwrap();
        assert_eq!(q.scan_id.as_deref(), Some("2"));
        assert_eq!(q.speed, Some(5));
    }

    #[test]
    fn test_auxiliary_and_wiper_query_aliases() {
        let q: AuxiliaryQuery =
            serde_json::from_value(serde_json::json!({"cmd": "on", "switchId": 1})).unwrap();
        assert_eq!(q.command.as_deref(), Some("on"));
        assert_eq!(q.switch_id, Some(1));

        let q: WiperQuery = serde_json::from_value(serde_json::json!({"cmd": "off"})).unwrap();
        assert_eq!(q.command.as_deref(), Some("off"));
    }
}
