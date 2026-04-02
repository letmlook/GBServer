use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CascadeQuery {
    pub platform_id: Option<String>,
    pub page: Option<u32>,
    pub count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CascadeChannelQuery {
    pub platform_id: String,
    pub channel_id: Option<String>,
}

pub async fn cascade_list(
    State(state): State<AppState>,
    Query(q): Query<CascadeQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Cascade platform list");

    let platforms: Vec<serde_json::Value> = vec![];
    Json(WVPResult::success(serde_json::json!({
        "list": platforms,
        "total": 0
    })))
}

pub async fn cascade_add(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let platform_id = body.get("platformId").and_then(|v| v.as_str()).unwrap_or_default();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or_default();
    let host = body.get("host").and_then(|v| v.as_str()).unwrap_or_default();
    let port = body.get("port").and_then(|v| v.as_u64()).unwrap_or(5060) as u16;
    let device_id = body.get("deviceId").and_then(|v| v.as_str()).unwrap_or_default();
    let password = body.get("password").and_then(|v| v.as_str()).unwrap_or_default();

    tracing::info!("Cascade platform add: {} ({})", name, platform_id);

    Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "name": name,
        "host": host,
        "port": port,
        "deviceId": device_id,
        "status": "pending"
    })))
}

pub async fn cascade_update(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<WVPResult<()>> {
    tracing::info!("Cascade platform update");
    Json(WVPResult::<()>::success_empty())
}

pub async fn cascade_delete(
    State(state): State<AppState>,
    Path(platform_id): Path<String>,
) -> Json<WVPResult<()>> {
    tracing::info!("Cascade platform delete: {}", platform_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn cascade_channel_list(
    State(state): State<AppState>,
    Query(q): Query<CascadeChannelQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Cascade channel list for platform: {}", q.platform_id);

    let channels: Vec<serde_json::Value> = vec![];
    Json(WVPResult::success(serde_json::json!({
        "list": channels,
        "total": 0
    })))
}

pub async fn cascade_channel_push(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    let platform_id = body.get("platformId").and_then(|v| v.as_str()).unwrap_or_default();
    let channel_id = body.get("channelId").and_then(|v| v.as_str()).unwrap_or_default();

    tracing::info!("Cascade channel push: platform={}, channel={}", platform_id, channel_id);

    Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "channelId": channel_id,
        "status": "pushing"
    })))
}

pub async fn cascade_channel_stop(
    State(state): State<AppState>,
    Path((platform_id, channel_id)): Path<(String, String)>,
) -> Json<WVPResult<()>> {
    tracing::info!("Cascade channel stop: platform={}, channel={}", platform_id, channel_id);
    Json(WVPResult::<()>::success_empty())
}

pub async fn cascade_register(
    State(state): State<AppState>,
    Path(platform_id): Path<String>,
) -> Json<WVPResult<serde_json::Value>> {
    tracing::info!("Cascade register to platform: {}", platform_id);

    Json(WVPResult::success(serde_json::json!({
        "platformId": platform_id,
        "status": "registering"
    })))
}

pub async fn cascade_unregister(
    State(state): State<AppState>,
    Path(platform_id): Path<String>,
) -> Json<WVPResult<()>> {
    tracing::info!("Cascade unregister from platform: {}", platform_id);
    Json(WVPResult::<()>::success_empty())
}
