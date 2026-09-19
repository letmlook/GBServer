//! 国标设备与通道 API，与前端 device.js 对应

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{count_devices, list_channels_filtered, list_devices_paged, Device};
use crate::error::AppError;
use crate::response::ApiResult;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct DevicesQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    /// "ON" / "OFF" / "" (空 = 全部)
    #[serde(default)]
    pub status: Option<String>,
}

/// GET /api/device/query/devices
pub async fn query_devices(
    State(state): State<AppState>,
    Query(q): Query<DevicesQuery>,
) -> Result<Json<ApiResult<DevicePage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let online = match q.status.as_deref() {
        Some("ON") | Some("on") | Some("1") | Some("true") => Some(true),
        Some("OFF") | Some("off") | Some("0") | Some("false") => Some(false),
        _ => None,
    };
    let total = count_devices(&state.pool, q.query.as_deref(), online).await?;
    let list = list_devices_paged(&state.pool, page, count, q.query.as_deref(), online).await?;
    let out = DevicePage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    };
    Ok(Json(ApiResult::success(out)))
}

#[derive(Debug, serde::Serialize)]
pub struct DevicePage {
    pub total: u64,
    pub list: Vec<Device>,
    pub page: u64,
    pub size: u64,
}

/// `GET /api/device/query/latency` 的查询参数。
#[derive(Debug, Deserialize)]
pub struct LatencyQuery {
    /// 逗号分隔的设备 ID。给了就只返回这些设备 —— 列表页一页只有 20 行，
    /// 没必要每 5s 把**全部**设备的样本推下来（设备上千时那是几百 KB 的轮询）。
    #[serde(rename = "deviceIds", default)]
    pub device_ids: Option<String>,
}

/// GET /api/device/query/latency
///
/// 「国标设备」列表「延迟」列的数据源：平台 → 设备 → 平台的 **SIP 往返时间**。
///
/// 数据来自进程内延迟注册表（由 `sip/server.rs` 的探针循环按
/// `sip.heartbeat.latency_probe_interval_secs` 写入），不落库 ——
/// 延迟是秒级变化的实时量，落库只会在后端重启后留下过期值；
/// 重启后一轮探针就会重新填满。
///
/// 前端每次带上当前页的 deviceId，返回里没有的设备就是"还没测出来"。
pub async fn query_device_latency(
    State(_state): State<AppState>,
    Query(q): Query<LatencyQuery>,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    let reg = crate::sip::gb28181::latency_registry();
    let list = match q.device_ids.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(ids) => ids
            .split(',')
            .filter_map(|id| reg.get(id.trim()))
            .collect::<Vec<_>>(),
        None => reg.snapshot(),
    };
    Ok(Json(ApiResult::success(serde_json::json!({
        "list": list,
        // 0 = 探针未启用（配置里关掉了），前端据此显示"未启用"而不是"测量中"。
        "probeIntervalSecs": reg.interval_secs(),
    }))))
}

#[derive(Debug, Deserialize)]
pub struct ChannelsQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    /// 关键字（名称/通道编号）
    pub query: Option<String>,
    pub online: Option<bool>,
    #[serde(alias = "channelType")]
    pub channel_type: Option<i32>,
}

/// GET /api/device/query/devices/:deviceId/channels
pub async fn query_channels(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Query(q): Query<ChannelsQuery>,
) -> Result<Json<ApiResult<ChannelPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    // 三个过滤参数此前被完全忽略（返回该设备全部通道）——通道多的设备上
    // 搜索/在线筛选看起来完全无效。
    let (list, total) = list_channels_filtered(
        &state.pool,
        &device_id,
        q.query.as_deref(),
        q.online,
        q.channel_type,
        page,
        count,
    )
    .await?;
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
    Ok(Json(ApiResult::success(out)))
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
) -> Json<ApiResult<serde_json::Value>> {
    let online_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gb_device WHERE on_line = true"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let offline_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gb_device WHERE on_line = false OR on_line IS NULL"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let total = online_count + offline_count;

    Json(ApiResult::success(serde_json::json!({
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
) -> Json<ApiResult<serde_json::Value>> {
    let today_register: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gb_device WHERE DATE(create_time) = CURRENT_DATE"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let total_devices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gb_device"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let active_devices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gb_device WHERE on_line = true"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    Json(ApiResult::success(serde_json::json!({
        "todayRegister": today_register,
        "totalDevices": total_devices,
        "activeDevices": active_devices,
        "inactiveDevices": total_devices - active_devices
    })))
}
