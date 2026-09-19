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

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DevicesQuery {
    /// 页码，从 1 开始（默认 1）
    pub page: Option<u32>,
    /// 每页条数（默认 10，上限 100）
    pub count: Option<u32>,
    /// 名称/编号模糊关键字
    pub query: Option<String>,
    /// 设备在线状态过滤：`ON` / `OFF`；空值 = 全部
    #[serde(default)]
    pub status: Option<String>,
}

/// GET /api/device/query/devices
///
/// 设备分页列表（支持按名称/编号模糊匹配 + 在线状态过滤）。
#[utoipa::path(
    get,
    path = "/api/device/query/devices",
    tag = "device",
    operation_id = "device_query_devices",
    params(DevicesQuery),
    responses(
        (status = 200, description = "设备分页结果 `{total, list, page, size}` —— `list` 内为完整 `Device` 行（含 id/firmware/heartBeat*/registerTime/channelCount 等）",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"total":42,"page":1,"size":10,"list":[
             {"id":1,"deviceId":"34020000001320000001","name":"前门","onLine":true}
         ]}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
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
#[derive(Debug, Deserialize, utoipa::IntoParams)]
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
#[utoipa::path(
    get,
    path = "/api/device/query/latency",
    tag = "device",
    operation_id = "device_query_latency",
    params(LatencyQuery),
    responses(
        (status = 200, description = "每台设备最近一次 SIP 往返延迟（毫秒） + 探针周期",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"probeIntervalSecs":15,"list":[
             {"deviceId":"34020000001320000001","latencyMs":42,"updatedAt":"2026-09-13T07:00:00Z"}
         ]}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
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

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ChannelsQuery {
    /// 页码，从 1 开始（默认 1）
    pub page: Option<u32>,
    /// 每页条数（默认 10，上限 100）
    pub count: Option<u32>,
    /// 关键字（名称/通道编号）
    pub query: Option<String>,
    /// 仅返回在线通道
    pub online: Option<bool>,
    /// 通道类型（前端字段名 `channelType`）
    #[serde(alias = "channelType")]
    pub channel_type: Option<i32>,
}

/// GET /api/device/query/devices/{device_id}/channels
#[utoipa::path(
    get,
    path = "/api/device/query/devices/{device_id}/channels",
    tag = "device",
    operation_id = "device_query_device_channels",
    params(
        ("device_id" = String, Path, description = "设备国标 ID（20 位）"),
        ChannelsQuery,
    ),
    responses(
        (status = 200, description = "该设备下的通道分页结果（每行带 camelCase + gb_* 兼容字段）",
         body = ApiResult<ChannelPage>,
         example = json!({"code":0,"msg":"成功","data":{"total":4,"page":1,"size":10,"list":[
             {"id":1,"deviceId":"34020000001320000001","channelId":"34020000001310000001","name":"通道1","status":"ON"}
         ]}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
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

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ChannelPage {
    pub total: u64,
    pub list: Vec<serde_json::Value>,
    pub page: u64,
    pub size: u64,
}

/// GET /api/device/query/statistics/keepalive
/// 设备保活统计
#[utoipa::path(
    get,
    path = "/api/device/query/statistics/keepalive",
    tag = "device",
    operation_id = "device_query_keepalive_statistics",
    responses(
        (status = 200, description = "在线/离线设备数 + 上线率",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"online":18,"offline":4,"total":22,"onlineRate":81.82}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
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
#[utoipa::path(
    get,
    path = "/api/device/query/statistics/register",
    tag = "device",
    operation_id = "device_query_register_statistics",
    responses(
        (status = 200, description = "今日新增 / 总数 / 在离线数",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"todayRegister":3,"totalDevices":22,"activeDevices":18,"inactiveDevices":4}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
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