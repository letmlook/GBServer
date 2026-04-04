use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
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
        let server = sip_server.read().await;
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
        let server = sip_server.read().await;
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
        let server = sip_server.read().await;
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

    tracing::info!("Catalog subscription: device={}, cycle={}", device_id, cycle);

    if let Some(ref sip_server) = state.sip_server {
        let server = sip_server.read().await;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online {
                match server.send_subscribe(&device_id, "Catalog", cycle).await {
                    Ok(_) => {
                        return Json(WVPResult::success(serde_json::json!({
                            "deviceId": device_id,
                            "cycle": cycle,
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
        let server = sip_server.read().await;
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

fn build_ptz_xml(command: &str, speed: u8, preset: u32, _dwStop: u32) -> String {
    let ptz_cmd = match command.to_ascii_uppercase().as_str() {
        "UP" => format!("0501000000{:02X}FF", speed),
        "DOWN" => format!("0501000001{:02X}FF", speed),
        "LEFT" => format!("0501000002{:02X}FF", speed),
        "RIGHT" => format!("0501000003{:02X}FF", speed),
        "ZOOM_IN" => format!("0501010000{:02X}FF", speed),
        "ZOOM_OUT" => format!("0501010001{:02X}FF", speed),
        "FOCUS_IN" => format!("0501020000{:02X}FF", speed),
        "FOCUS_OUT" => format!("0501020001{:02X}FF", speed),
        "IRIS_IN" => format!("0501030000{:02X}FF", speed),
        "IRIS_OUT" => format!("0501030001{:02X}FF", speed),
        "STOP" => "05010000000000FF".to_string(),
        _ => format!("050100000000{:02X}FF", speed),
    };
    
    format!(r#"<PTZCmd>{}</PTZCmd>"#, ptz_cmd)
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
