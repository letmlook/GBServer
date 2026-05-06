//! Front-end control API /api/front-end, matching frontEnd.js
//! PTZ, preset, cruise, scan, auxiliary, wiper, iris, focus controls via SIP

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PtzQuery {
    pub command: Option<String>,
    pub horizon_speed: Option<i32>,
    pub vertical_speed: Option<i32>,
    pub zoom_speed: Option<i32>,
    pub speed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    pub scan_id: Option<String>,
    pub speed: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CruiseQuery {
    pub cruise_id: Option<String>,
    pub preset_id: Option<i32>,
    pub cruise_speed: Option<i32>,
    pub cruise_time: Option<i32>,
    pub speed: Option<i32>,
    pub time: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct PresetQuery {
    pub preset_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct AuxiliaryQuery {
    pub command: Option<String>,
    pub switch_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct WiperQuery {
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

fn build_ptz_xml(command: &str, h_speed: u8, v_speed: u8, z_speed: u8) -> String {
    let ptz_cmd = match command.to_ascii_uppercase().as_str() {
        "UP" => format!("0501000000{:02X}FF", h_speed),
        "DOWN" => format!("0501000001{:02X}FF", v_speed),
        "LEFT" => format!("0501000002{:02X}FF", h_speed),
        "RIGHT" => format!("0501000003{:02X}FF", h_speed),
        "ZOOM_IN" => format!("0501010000{:02X}FF", z_speed),
        "ZOOM_OUT" => format!("0501010001{:02X}FF", z_speed),
        "FOCUS_IN" => format!("0501020000{:02X}FF", z_speed),
        "FOCUS_OUT" => format!("0501020001{:02X}FF", z_speed),
        "IRIS_IN" => format!("0501030000{:02X}FF", z_speed),
        "IRIS_OUT" => format!("0501030001{:02X}FF", z_speed),
        "STOP" => "05010000000000FF".to_string(),
        _ => format!("050100000000{:02X}FF", h_speed),
    };
    format!(r#"<PTZCmd>{}</PTZCmd>"#, ptz_cmd)
}

fn build_raw_front_end_xml(cmd_code: i32, parameter1: i32, parameter2: i32, combind_code2: i32) -> String {
    let code1 = (cmd_code & 0xff) as u8;
    let param1 = (parameter1 & 0xff) as u8;
    let param2 = (parameter2 & 0xff) as u8;
    let code2 = (combind_code2 & 0xff) as u8;
    format!(
        r#"<PTZCmd>A50F01{:02X}{:02X}{:02X}{:02X}</PTZCmd>"#,
        code1, param1, param2, code2
    )
}

fn build_preset_xml(command: &str, preset_index: u32) -> String {
    let preset_cmd = match command.to_ascii_uppercase().as_str() {
        "GOTO_PRESET" => format!("07000100000000{:02X}FF", preset_index),
        "SET_PRESET" => format!("07000100010000{:02X}FF", preset_index),
        "CLEAR_PRESET" => format!("07000100020000{:02X}FF", preset_index),
        _ => format!("07000100000000{:02X}FF", preset_index),
    };
    format!(r#"<PTZCmd>{}</PTZCmd>"#, preset_cmd)
}

fn build_auxiliary_xml(command: &str, switch_id: u32) -> String {
    let aux_cmd = if command.to_lowercase() == "on" { "Set" } else { "Reset" };
    format!(r#"<AuxiliaryCmd><cmd>{}</cmd><index>{}</index></AuxiliaryCmd>"#, aux_cmd, switch_id)
}

fn build_wiper_xml(command: &str) -> String {
    let wiper_cmd = if command.to_lowercase() == "on" { "Open" } else { "Close" };
    format!(r#"<WiperCmd>{}</WiperCmd>"#, wiper_cmd)
}

fn build_fi_xml(cmd_type: &str, command: &str, speed: u8) -> String {
    let fi_cmd = match (cmd_type, command.to_lowercase().as_str()) {
        ("iris", "on" | "open") => format!("0501030000{:02X}FF", speed),
        ("iris", _) => format!("0501030001{:02X}FF", speed),
        ("focus", "on" | "open") => format!("0501020000{:02X}FF", speed),
        ("focus", _) => format!("0501020001{:02X}FF", speed),
        _ => format!("0501030000{:02X}FF", speed),
    };
    format!(r#"<PTZCmd>{}</PTZCmd>"#, fi_cmd)
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
        let server = sip_server.read().await;
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
    let h_speed = q.horizon_speed.unwrap_or(1) as u8;
    let v_speed = q.vertical_speed.unwrap_or(1) as u8;
    let z_speed = q.zoom_speed.unwrap_or(1) as u8;

    tracing::info!(
        "PTZ control: device={}, channel={}, cmd={}, h={}, v={}, z={}",
        device_id, channel_id, command, h_speed, v_speed, z_speed
    );

    let body = build_ptz_xml(&command, h_speed, v_speed, z_speed);
    match send_via_sip(&state, &device_id, &channel_id, "DeviceControl", &body).await {
        Ok(()) => Json(success_json("PTZ 控制命令已发送")),
        Err(e) => Json(serde_json::json!({ "code": 1, "msg": e })),
    }
}

/// POST /api/ptz/front_end_command/:device_id/:channel_id
///
/// Compatibility endpoint used by older WVP player components. They already
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
    let speed = q.speed.unwrap_or(1) as u8;

    tracing::info!(
        "Iris control: device={}, channel={}, cmd={}, speed={}",
        device_id, channel_id, command, speed
    );

    let body = build_fi_xml("iris", &command, speed);
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
    let speed = q.speed.unwrap_or(1) as u8;

    tracing::info!(
        "Focus control: device={}, channel={}, cmd={}, speed={}",
        device_id, channel_device_id, command, speed
    );

    let body = build_fi_xml("focus", &command, speed);
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

    let body = build_preset_xml("SET_PRESET", preset_id as u32);
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

    let body = build_preset_xml("GOTO_PRESET", preset_id as u32);
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

    let body = build_preset_xml("CLEAR_PRESET", preset_id as u32);
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
