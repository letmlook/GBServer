//! 国标设备与通道 API，与前端 device.js 对应

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{count_channels, count_devices, list_channels_paged, list_devices_paged, Device};
use crate::error::AppError;
use crate::response::WVPResult;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct DevicesQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub status: Option<bool>,
}

/// GET /api/device/query/devices
pub async fn query_devices(
    State(state): State<AppState>,
    Query(q): Query<DevicesQuery>,
) -> Result<Json<WVPResult<DevicePage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = count_devices(&state.pool, q.query.as_deref(), q.status).await?;
    let list = list_devices_paged(&state.pool, page, count, q.query.as_deref(), q.status).await?;
    let out = DevicePage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    };
    Ok(Json(WVPResult::success(out)))
}

#[derive(Debug, serde::Serialize)]
pub struct DevicePage {
    pub total: u64,
    pub list: Vec<Device>,
    pub page: u64,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub struct ChannelsQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
}

/// GET /api/device/query/devices/:deviceId/channels
pub async fn query_channels(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Query(q): Query<ChannelsQuery>,
) -> Result<Json<WVPResult<ChannelPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = count_channels(&state.pool, &device_id).await?;
    let list = list_channels_paged(&state.pool, &device_id, page, count).await?;
    // Phase 5: 同时输出 camelCase + gb_* + ptzTypeText,前端 /device/channel、
    // 地图信息窗等都用同一份数据。
    let rows: Vec<serde_json::Value> = list
        .into_iter()
        .map(|c| crate::handlers::device_stub::channel_to_json(&c))
        .collect();
    let out = ChannelPage {
        total: total as u64,
        list: rows,
        page: page as u64,
        size: count as u64,
    };
    Ok(Json(WVPResult::success(out)))
}

#[derive(Debug, serde::Serialize)]
pub struct ChannelPage {
    pub total: u64,
    pub list: Vec<serde_json::Value>,
    pub page: u64,
    pub size: u64,
}

/// GET /api/device/query/statistics/keepalive
/// 设备保活统计
pub async fn device_keepalive_statistics(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let online_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wvp_device WHERE on_line = true"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let offline_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wvp_device WHERE on_line = false OR on_line IS NULL"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let total = online_count + offline_count;

    Json(WVPResult::success(serde_json::json!({
        "online": online_count,
        "offline": offline_count,
        "total": total,
        "onlineRate": if total > 0 { online_count as f64 / total as f64 * 100.0 } else { 0.0 }
    })))
}

/// GET /api/device/query/statistics/register
/// 设备注册统计
pub async fn device_register_statistics(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let today_register: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wvp_device WHERE DATE(create_time) = CURRENT_DATE"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let total_devices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wvp_device"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let active_devices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wvp_device WHERE on_line = true"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    Json(WVPResult::success(serde_json::json!({
        "todayRegister": today_register,
        "totalDevices": total_devices,
        "activeDevices": active_devices,
        "inactiveDevices": total_devices - active_devices
    })))
}
