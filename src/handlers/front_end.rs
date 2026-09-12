//! Front-end control API /api/front-end, matching frontEnd.js
//! PTZ, preset, cruise, scan, auxiliary, wiper, iris, focus controls via SIP

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::sip::gb28181::front_end_control::{
    build_auxiliary_cmd, build_fi_cmd, build_preset_cmd, build_ptz_cmd, build_ptz_cmd_raw,
    build_scan_cmd, build_tour_cmd, build_wiper_cmd, FiAction, PresetAction, PtzAction, ScanAction,
    TourAction,
};
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

/// 聚焦/光圈：**与 WVP 一致地走 8 字节 `PTZCmd`**。
///
/// 此前这里发的是 2016 风格的独立元素 `<FICmd>IrisOpen</FICmd>`。
/// 但 WVP-PRO（平替目标）的聚焦/光圈是 `PTZCmd` 二进制指令码
/// （`SourcePTZServiceForGbImpl::fi`：基址 `1<<6`，聚焦 bit1/bit0、光圈 bit3/bit2），
/// GB/T 28181-**2022** §A.3.3/A.3.4 也是同样的二进制编码 —— 只发独立元素的实现
/// 在只认 PTZCmd 的设备上完全无效。

/// 预置位：同样走 `PTZCmd`（`0x81` 设置 / `0x82` 调用 / `0x83` 删除，
/// 编号在**数据2**），与 WVP 的 `preset` 分支一致。
fn preset_body(cmd: &str, preset_index: u32) -> Option<String> {
    let action = PresetAction::parse(cmd)?;
    Some(format!(
        "<PTZCmd>{}</PTZCmd>",
        build_preset_cmd(action, preset_index)
    ))
}

/// 兼容端点用：调用方给的是「指令码/数据1/数据2/组合码2」四段，
/// 这里补 `A5 0F 01` 前缀、把组合码2 移入高 4 位并计算**累加校验**。
///
/// 组合码2 的口径与 WVP 的 `/api/ptz/front_end/{...}` 一致：取值 0-15，
/// 由本函数左移 4 位写入字节7（此前直接当字节7 写，等于少移了 4 位）。
fn build_raw_front_end_xml(cmd_code: i32, parameter1: i32, parameter2: i32, combind_code2: i32) -> String {
    build_ptz_cmd_raw(
        (cmd_code & 0xff) as u8,
        (parameter1 & 0xff) as u8,
        (parameter2 & 0xff) as u8,
        (combind_code2.clamp(0, 15)) as u8,
    )
    .pipe_ptz_xml()
}

/// 把 8 字节指令串包成 `<PTZCmd>…</PTZCmd>`。
trait PipePtzXml {
    fn pipe_ptz_xml(self) -> String;
}
impl PipePtzXml for String {
    fn pipe_ptz_xml(self) -> String {
        format!("<PTZCmd>{}</PTZCmd>", self)
    }
}

fn aux_is_on(command: &str) -> bool {
    command.eq_ignore_ascii_case("on")
}

fn build_auxiliary_xml(command: &str, switch_id: u32) -> String {
    build_auxiliary_cmd(aux_is_on(command), (switch_id & 0xff) as u8).pipe_ptz_xml()
}

fn build_wiper_xml(command: &str) -> String {
    build_wiper_cmd(aux_is_on(command)).pipe_ptz_xml()
}

fn build_scan_xml(cmd: &str, scan_id: u32, speed: u8) -> String {
    let action = match cmd {
        "setSpeed" => ScanAction::SetSpeed,
        "setLeft" => ScanAction::SetLeft,
        "setRight" => ScanAction::SetRight,
        "stop" => ScanAction::Stop,
        _ => ScanAction::Start,
    };
    build_scan_cmd(action, (scan_id & 0xff) as u8, speed).pipe_ptz_xml()
}

fn build_cruise_xml(cmd: &str, cruise_id: u32, preset_id: u32, speed: u8, time: u32) -> String {
    let action = match cmd {
        "deletePoint" => TourAction::DeletePoint,
        "speed" => TourAction::SetSpeed,
        "time" => TourAction::SetTime,
        "start" => TourAction::Start,
        "stop" => TourAction::Stop,
        _ => TourAction::AddPoint,
    };
    let value = match action {
        TourAction::SetSpeed => speed as u16,
        TourAction::SetTime => time as u16,
        _ => 0,
    };
    build_tour_cmd(
        action,
        (cruise_id & 0xff) as u8,
        (preset_id & 0xff) as u8,
        value,
    )
    .pipe_ptz_xml()
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
    // WVP 的 `/fi/iris` 取值是 `in` / `out` / `stop`（本平台老前端发 on/off/open/close）
    let lower = command.trim().to_ascii_lowercase();
    let speed = q.speed.unwrap_or(50).clamp(0, 255) as u8;
    let body = match lower.as_str() {
        "in" | "open" | "on" | "iris_in" => {
            format!("<PTZCmd>{}</PTZCmd>", build_fi_cmd(FiAction::IrisOpen, speed))
        }
        "out" | "close" | "off" | "iris_out" => {
            format!("<PTZCmd>{}</PTZCmd>", build_fi_cmd(FiAction::IrisClose, speed))
        }
        // 停止：WVP 的 stop 分支两个方向都不置位，于是指令码停在基址 0x40、速度为 0
        "stop" => format!("<PTZCmd>{}</PTZCmd>", build_ptz_cmd_raw(0x40, 0, 0, 0)),
        _ => {
            return Json(serde_json::json!({
                "code": 1,
                "msg": format!("不支持的光圈命令: {command:?}（可用: in/out/stop）")
            }))
        }
    };

    tracing::info!(
        "Iris control: device={}, channel={}, cmd={}",
        device_id,
        channel_id,
        command
    );
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

    // WVP 的 `/fi/focus` 取值是 `near` / `far` / `stop`
    // （此前只认 on/off/open/close/focus_in/focus_out，WVP 前端发 near/far 会被拒）
    let lower = command.trim().to_ascii_lowercase();
    let speed = q.speed.unwrap_or(50).clamp(0, 255) as u8;
    let body = match lower.as_str() {
        "near" | "focus_in" | "on" | "open" => {
            format!("<PTZCmd>{}</PTZCmd>", build_fi_cmd(FiAction::FocusNear, speed))
        }
        "far" | "focus_out" | "off" | "close" => {
            format!("<PTZCmd>{}</PTZCmd>", build_fi_cmd(FiAction::FocusFar, speed))
        }
        "stop" => format!("<PTZCmd>{}</PTZCmd>", build_ptz_cmd_raw(0x40, 0, 0, 0)),
        _ => {
            return Json(serde_json::json!({
                "code": 1,
                "msg": format!("不支持的聚焦命令: {command:?}（可用: near/far/stop）")
            }))
        }
    };

    tracing::info!(
        "Focus control: device={}, channel={}, cmd={}",
        device_id,
        channel_device_id,
        command
    );
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

    let Some(body) = preset_body("SET_PRESET", preset_id as u32) else {
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

    let Some(body) = preset_body("GOTO_PRESET", preset_id as u32) else {
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

    let Some(body) = preset_body("CLEAR_PRESET", preset_id as u32) else {
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

    /// 校验一个 `<PTZCmd>…</PTZCmd>` 体：8 字节、A5 开头、末字节为累加和。
    fn assert_valid_ptzcmd(body: &str) -> Vec<u8> {
        let hex = body
            .trim_start_matches("<PTZCmd>")
            .trim_end_matches("</PTZCmd>");
        assert_eq!(hex.len(), 16, "必须是 8 字节: {body}");
        assert!(hex.starts_with("A50F01"), "{body}");
        let bytes: Vec<u8> = (0..8)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        assert_eq!(
            bytes[7],
            bytes[..7].iter().fold(0u8, |a, b| a.wrapping_add(*b)),
            "校验和不对: {body}"
        );
        bytes
    }

    /// 聚焦/光圈与 WVP 一致地走 **8 字节 PTZCmd**（不是 2016 的 `<FICmd>` 元素）：
    /// 聚焦近 0x42 / 远 0x41（速度在数据1），光圈开 0x48 / 关 0x44（速度在数据2）。
    #[test]
    fn test_fi_uses_ptzcmd_binary() {
        let near = assert_valid_ptzcmd(&build_fi_cmd(FiAction::FocusNear, 0x1F).pipe_ptz_xml());
        assert_eq!(&near[..6], &[0xA5, 0x0F, 0x01, 0x42, 0x1F, 0x00]);
        let far = assert_valid_ptzcmd(&build_fi_cmd(FiAction::FocusFar, 0x1F).pipe_ptz_xml());
        assert_eq!(&far[..6], &[0xA5, 0x0F, 0x01, 0x41, 0x1F, 0x00]);
        let open = assert_valid_ptzcmd(&build_fi_cmd(FiAction::IrisOpen, 0x20).pipe_ptz_xml());
        assert_eq!(&open[..6], &[0xA5, 0x0F, 0x01, 0x48, 0x00, 0x20]);
        let close = assert_valid_ptzcmd(&build_fi_cmd(FiAction::IrisClose, 0x20).pipe_ptz_xml());
        assert_eq!(&close[..6], &[0xA5, 0x0F, 0x01, 0x44, 0x00, 0x20]);
    }

    /// 预置位同样走 PTZCmd：0x81 设置 / 0x82 调用 / 0x83 删除，编号在数据2。
    #[test]
    fn test_preset_uses_ptzcmd_binary() {
        let set = assert_valid_ptzcmd(&preset_body("SET_PRESET", 5).unwrap());
        assert_eq!(&set[..6], &[0xA5, 0x0F, 0x01, 0x81, 0x00, 0x05]);
        let call = assert_valid_ptzcmd(&preset_body("GOTO_PRESET", 7).unwrap());
        assert_eq!(&call[..6], &[0xA5, 0x0F, 0x01, 0x82, 0x00, 0x07]);
        let del = assert_valid_ptzcmd(&preset_body("CLEAR_PRESET", 3).unwrap());
        assert_eq!(&del[..6], &[0xA5, 0x0F, 0x01, 0x83, 0x00, 0x03]);
        assert!(preset_body("bogus", 1).is_none());
    }

    /// 聚焦/光圈的**命令取值**必须与 WVP 一致（`near`/`far`/`stop`、`in`/`out`/`stop`）
    /// —— 此前只认 on/off/open/close 之类，WVP 前端发 `near` 会被直接拒掉。
    ///
    /// 这里逐个取值跑一遍归一化逻辑（与 handler 中的 match 分支保持一致）。
    #[test]
    fn test_fi_command_values_accepted() {
        fn focus_body(cmd: &str, speed: u8) -> Option<String> {
            match cmd.trim().to_ascii_lowercase().as_str() {
                "near" | "focus_in" | "on" | "open" => {
                    Some(build_fi_cmd(FiAction::FocusNear, speed).pipe_ptz_xml())
                }
                "far" | "focus_out" | "off" | "close" => {
                    Some(build_fi_cmd(FiAction::FocusFar, speed).pipe_ptz_xml())
                }
                "stop" => Some(build_ptz_cmd_raw(0x40, 0, 0, 0).pipe_ptz_xml()),
                _ => None,
            }
        }
        fn iris_body(cmd: &str, speed: u8) -> Option<String> {
            match cmd.trim().to_ascii_lowercase().as_str() {
                "in" | "open" | "on" | "iris_in" => {
                    Some(build_fi_cmd(FiAction::IrisOpen, speed).pipe_ptz_xml())
                }
                "out" | "close" | "off" | "iris_out" => {
                    Some(build_fi_cmd(FiAction::IrisClose, speed).pipe_ptz_xml())
                }
                "stop" => Some(build_ptz_cmd_raw(0x40, 0, 0, 0).pipe_ptz_xml()),
                _ => None,
            }
        }

        // WVP 的取值
        let near = assert_valid_ptzcmd(&focus_body("near", 30).unwrap());
        assert_eq!(near[3], 0x42);
        let far = assert_valid_ptzcmd(&focus_body("far", 30).unwrap());
        assert_eq!(far[3], 0x41);
        let inb = assert_valid_ptzcmd(&iris_body("in", 30).unwrap());
        assert_eq!(inb[3], 0x48);
        let out = assert_valid_ptzcmd(&iris_body("out", 30).unwrap());
        assert_eq!(out[3], 0x44);
        // stop → 基址 0x40、速度 0
        let fstop = assert_valid_ptzcmd(&focus_body("stop", 30).unwrap());
        assert_eq!(&fstop[..6], &[0xA5, 0x0F, 0x01, 0x40, 0x00, 0x00]);
        let istop = assert_valid_ptzcmd(&iris_body("stop", 30).unwrap());
        assert_eq!(&istop[..6], &[0xA5, 0x0F, 0x01, 0x40, 0x00, 0x00]);
        // 旧前端的取值继续可用
        assert_eq!(assert_valid_ptzcmd(&focus_body("focus_in", 1).unwrap())[3], 0x42);
        assert_eq!(assert_valid_ptzcmd(&iris_body("close", 1).unwrap())[3], 0x44);
        // 拼错要报错，而不是静默下发错误指令
        assert!(focus_body("nope", 1).is_none());
        assert!(iris_body("nope", 1).is_none());
    }

    /// 巡航/扫描/辅助/雨刷：WVP 的指令码表（都是 8 字节 PTZCmd）。
    #[test]
    fn test_cruise_scan_aux_ptzcmd_codes() {
        let add = assert_valid_ptzcmd(&build_cruise_xml("addPoint", 1, 5, 0, 0));
        assert_eq!(&add[..6], &[0xA5, 0x0F, 0x01, 0x84, 0x01, 0x05]);
        let del = assert_valid_ptzcmd(&build_cruise_xml("deletePoint", 1, 5, 0, 0));
        assert_eq!(&del[..6], &[0xA5, 0x0F, 0x01, 0x85, 0x01, 0x05]);
        let spd = assert_valid_ptzcmd(&build_cruise_xml("speed", 1, 5, 3, 0));
        assert_eq!(&spd[..6], &[0xA5, 0x0F, 0x01, 0x86, 0x01, 0x05]);
        assert_eq!(spd[6] >> 4, 3, "巡航速度写组合码2 高 4 位");
        let time = assert_valid_ptzcmd(&build_cruise_xml("time", 1, 5, 0, 7));
        assert_eq!(&time[..6], &[0xA5, 0x0F, 0x01, 0x87, 0x01, 0x05]);
        assert_eq!(time[6] >> 4, 7);
        let start = assert_valid_ptzcmd(&build_cruise_xml("start", 1, 0, 0, 0));
        assert_eq!(&start[..6], &[0xA5, 0x0F, 0x01, 0x88, 0x01, 0x00]);
        // 停止：国标/WVP 都没有单独指令码 → 0x00（停止所有动作）
        let stop = assert_valid_ptzcmd(&build_cruise_xml("stop", 1, 0, 0, 0));
        assert_eq!(&stop[..6], &[0xA5, 0x0F, 0x01, 0x00, 0x00, 0x00]);

        let s_start = assert_valid_ptzcmd(&build_scan_xml("start", 2, 0));
        assert_eq!(&s_start[..6], &[0xA5, 0x0F, 0x01, 0x89, 0x02, 0x00]);
        let s_left = assert_valid_ptzcmd(&build_scan_xml("setLeft", 2, 0));
        assert_eq!(&s_left[..6], &[0xA5, 0x0F, 0x01, 0x89, 0x02, 0x01]);
        let s_right = assert_valid_ptzcmd(&build_scan_xml("setRight", 2, 0));
        assert_eq!(&s_right[..6], &[0xA5, 0x0F, 0x01, 0x89, 0x02, 0x02]);
        let s_speed = assert_valid_ptzcmd(&build_scan_xml("setSpeed", 2, 9));
        assert_eq!(&s_speed[..6], &[0xA5, 0x0F, 0x01, 0x8A, 0x02, 0x09]);

        let a_on = assert_valid_ptzcmd(&build_auxiliary_xml("on", 3));
        assert_eq!(&a_on[..6], &[0xA5, 0x0F, 0x01, 0x8C, 0x03, 0x00]);
        let a_off = assert_valid_ptzcmd(&build_auxiliary_xml("off", 3));
        assert_eq!(&a_off[..6], &[0xA5, 0x0F, 0x01, 0x8D, 0x03, 0x00]);
        // 雨刷 = 辅助开关编号固定 1
        let w_on = assert_valid_ptzcmd(&build_wiper_xml("on"));
        assert_eq!(&w_on[..6], &[0xA5, 0x0F, 0x01, 0x8C, 0x01, 0x00]);
        let w_off = assert_valid_ptzcmd(&build_wiper_xml("off"));
        assert_eq!(&w_off[..6], &[0xA5, 0x0F, 0x01, 0x8D, 0x01, 0x00]);
    }

    /// 兼容端点：调用方给的 4 段要补 `A5 0F 01` 前缀、把**组合码2 左移 4 位**
    /// 写入字节7（WVP 口径：combindCode2 取值 0-15），并计算累加校验和。
    #[test]
    fn test_raw_front_end_command_has_checksum() {
        let xml = build_raw_front_end_xml(0x08, 0x00, 0x1F, 0x00);
        let bytes = assert_valid_ptzcmd(&xml);
        assert_eq!(&bytes[..4], &[0xA5, 0x0F, 0x01, 0x08]);
        assert_eq!(&bytes[4..7], &[0x00, 0x1F, 0x00]);

        // 组合码2 = 1 → 字节7 = 0x10（此前直接写 0x01，少移了 4 位）
        let zoom = build_raw_front_end_xml(0x10, 0x00, 0x00, 0x01);
        let bytes = assert_valid_ptzcmd(&zoom);
        assert_eq!(bytes[6], 0x10);
        // 超范围（>15）被夹取，不会污染地址高 4 位
        let clamped = build_raw_front_end_xml(0x10, 0x00, 0x00, 99);
        let bytes = assert_valid_ptzcmd(&clamped);
        assert_eq!(bytes[6], 0xF0);
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
