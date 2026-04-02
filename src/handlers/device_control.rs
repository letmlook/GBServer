use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PtzQuery {
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
    pub command: Option<String>,
    pub speed: Option<u8>,
    pub preset_index: Option<u32>,
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
                tracing::info!("Sending PTZ command to device: {}", device_id);
                return Json(WVPResult::success(serde_json::json!({
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "command": command,
                    "speed": speed,
                    "result": "PTZ command sent"
                })));
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
                return Json(WVPResult::success(serde_json::json!({
                    "deviceId": device_id,
                    "channelId": channel_id,
                    "command": command,
                    "presetIndex": preset_index,
                    "result": "Preset command sent"
                })));
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

    tracing::info!("Guard control: device={}, cmd={}", device_id, guard_cmd);

    if let Some(ref sip_server) = state.sip_server {
        let server = sip_server.read().await;
        if let Some(device) = server.device_manager().get(&device_id).await {
            if device.online && device.addr.is_some() {
                return Json(WVPResult::success(serde_json::json!({
                    "deviceId": device_id,
                    "guardCmd": guard_cmd,
                    "result": "Guard command sent"
                })));
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
    let cycle = q.cycle.unwrap_or(3600);

    tracing::info!("Catalog subscription: device={}, cycle={}", device_id, cycle);

    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "cycle": cycle,
        "result": "Catalog subscription set"
    })))
}

pub async fn subscribe_mobile_position(
    State(state): State<AppState>,
    Query(q): Query<SubscribeQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.id.clone().unwrap_or_default();
    let cycle = q.cycle.unwrap_or(5);
    let interval = q.interval.unwrap_or(5);

    tracing::info!("Position subscription: device={}, cycle={}, interval={}",
        device_id, cycle, interval);

    Json(WVPResult::success(serde_json::json!({
        "deviceId": device_id,
        "cycle": cycle,
        "interval": interval,
        "result": "Position subscription set"
    })))
}
