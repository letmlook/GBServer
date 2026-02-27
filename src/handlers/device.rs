//! 国标设备与通道 API，与前端 device.js 对应

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{count_channels, count_devices, list_channels_paged, list_devices_paged, Device, DeviceChannel};
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
    let out = ChannelPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    };
    Ok(Json(WVPResult::success(out)))
}

#[derive(Debug, serde::Serialize)]
pub struct ChannelPage {
    pub total: u64,
    pub list: Vec<DeviceChannel>,
    pub page: u64,
    pub size: u64,
}
