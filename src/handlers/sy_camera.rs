//! Hikvision/Uniview-style camera API (`/api/sy/camera/*`).
//! These endpoints expose the device+channel tables in the contract format
//! expected by Hikvision iSecure Center / Uniview clients.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::response::WVPResult;
use crate::AppState;

// ---------- shared DTOs ----------

/// Hikvision-style camera row. Combines a `gb_device` row with its first
/// channel (or itself when the device has no children).
#[derive(Serialize, Clone)]
pub struct CameraRow {
    pub id: i32,
    pub device_id: String,
    pub channel_id: String,
    pub name: String,
    pub status: String,
    pub online: bool,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub civil_code: Option<String>,
    pub address: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub has_audio: bool,
    pub sub_count: i32,
    pub parent_device_id: Option<String>,
    /// 该行代表"设备本身"（设备没有任何通道时，它自己就是一路摄像头），
    /// 而不是某个通道。前端据此把这类行与真正的通道区分开 ——
    /// 否则会拿 `channel_id == device_id` 去点播，必然失败。
    pub is_device: bool,
}

/// Mobile-friendly subset (fewer fields, smaller payload).
#[derive(Serialize)]
pub struct CameraMobile {
    pub device_id: String,
    pub channel_id: String,
    pub name: String,
    pub online: bool,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub address: Option<String>,
}

/// Filter by administrative code prefix.
#[derive(Deserialize, Default)]
pub struct AddressQuery {
    #[serde(alias = "civilCode")]
    pub civil_code: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
}

/// Filter by bounding box (south-west + north-east corners).
#[derive(Deserialize, Default)]
pub struct BoxQuery {
    pub min_lng: Option<f64>,
    pub min_lat: Option<f64>,
    pub max_lng: Option<f64>,
    pub max_lat: Option<f64>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
}

/// Filter by circle (center + radius in meters).
#[derive(Deserialize, Default)]
pub struct CircleQuery {
    pub lng: Option<f64>,
    pub lat: Option<f64>,
    pub radius: Option<f64>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
}

/// Filter by polygon (lng/lat pairs alternating).
#[derive(Deserialize, Default)]
pub struct PolygonQuery {
    pub points: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
}

/// Bulk lookup by GB-IDs.
#[derive(Deserialize, Default)]
pub struct IdsQuery {
    pub ids: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct PageQuery {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
    /// 关键字。WVP 的参数名是 `query`；`keyword` 是本平台历史拼写，两者都收
    /// —— 此前只认 `keyword`，前端按签名传 `query` 时被静默丢弃，搜索框毫无反应。
    #[serde(default, alias = "query")]
    pub keyword: Option<String>,
    /// 在线过滤。WVP 叫 `status`；本平台前端叫 `online`。
    /// 此前 DTO 里根本没有这个字段，handler 还把这个参数**硬编码为 None**，
    /// 于是"只看在线"完全无效。
    #[serde(default, alias = "status")]
    pub online: Option<bool>,
    /// 行政区划前缀过滤（作用于通道的 `civil_code`）
    #[serde(default, alias = "civilCode")]
    pub civil_code: Option<String>,
}

// ---------- helpers ----------

fn opt_to_string(s: &Option<String>) -> String {
    s.as_deref().unwrap_or("").to_string()
}

fn device_to_row(d: &db::Device, ch: Option<&db::DeviceChannel>) -> CameraRow {
    // 修正：此前无论有没有通道，`id` 与 `name` 都取**设备**的 —— 于是同一设备下
    // 的每个通道行 id 都等于设备 id（列表里出现重复 id），通道名也退化成设备名
    // （设备名为空时通道名就变成空串）。现在有通道时用通道自己的 id/name。
    let (id, channel_id, name, sub_count, has_audio, longitude, latitude, civil_code, address) =
        if let Some(c) = ch {
            let ch_name = opt_to_string(&c.name);
            (
                c.id,
                opt_to_string(&c.gb_device_id),
                if ch_name.is_empty() { opt_to_string(&d.name) } else { ch_name },
                c.sub_count.unwrap_or(0),
                c.has_audio.unwrap_or(false),
                c.longitude,
                c.latitude,
                c.civil_code.clone(),
                c.address.clone(),
            )
        } else {
            (
                d.id,
                d.device_id.clone(),
                opt_to_string(&d.name),
                0,
                false,
                None,
                None,
                None,
                None,
            )
        };
    CameraRow {
        id,
        is_device: ch.is_none(),
        device_id: d.device_id.clone(),
        channel_id,
        name,
        status: if d.on_line.unwrap_or(false) { "ON".into() } else { "OFF".into() },
        online: d.on_line.unwrap_or(false),
        longitude,
        latitude,
        civil_code,
        address,
        manufacturer: d.manufacturer.clone(),
        model: d.model.clone(),
        has_audio,
        sub_count,
        parent_device_id: None,
    }
}

fn channel_to_mobile(ch: &db::DeviceChannel) -> CameraMobile {
    CameraMobile {
        device_id: opt_to_string(&ch.device_id),
        channel_id: opt_to_string(&ch.gb_device_id),
        name: opt_to_string(&ch.name),
        online: ch.status.as_deref() == Some("ON"),
        longitude: ch.longitude,
        latitude: ch.latitude,
        address: ch.address.clone(),
    }
}

// ---------- handlers ----------

/// 摄像机行集合（`/camera/list` 与 `/camera/list-with-child` 共用）。
///
/// **行级过滤 + 行级分页**：WVP 的同名接口是在**通道**维度过滤和分页的
/// （`ChannelProvider.queryListWithChildForSy`：`query` 匹配通道的
/// `gb_device_id`/`gb_name`，`status` 过滤通道在线状态，PageHelper 作用于通道查询）。
/// 此前这里按**设备**维度分页 + 过滤，于是：
///   * 按通道名搜索永远 0 结果（设备的 `name` 里没有通道名）；
///   * `count=1000` 被 db 层截到 100，第 101 台设备及其通道静默消失；
///   * `total` 返回的是"本页展开出的行数"，分页器永远只有一页。
async fn camera_rows(state: &AppState, q: &PageQuery) -> (Vec<CameraRow>, u64, u32, u32) {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(100).clamp(1, 1000);

    // 设备一次取到上限（SQLite 部署的设备上限本就是 500），通道一次取全量后按设备分组，
    // 避免 N+1 查询。
    let devices = db::device::list_devices_paged(&state.pool, 1, 1000, None, None)
        .await
        .unwrap_or_default();
    let channels = db::device::list_all_channels(&state.pool)
        .await
        .unwrap_or_default();
    let mut by_device: std::collections::HashMap<String, Vec<db::DeviceChannel>> =
        std::collections::HashMap::new();
    for ch in channels {
        by_device
            .entry(ch.device_id.clone().unwrap_or_default())
            .or_default()
            .push(ch);
    }

    let mut rows: Vec<CameraRow> = Vec::with_capacity(devices.len());
    for d in &devices {
        match by_device.get(&d.device_id) {
            Some(chs) if !chs.is_empty() => {
                for ch in chs {
                    let mut row = device_to_row(d, Some(ch));
                    row.parent_device_id = Some(d.device_id.clone());
                    rows.push(row);
                }
            }
            // 没有通道的设备：用它自己那一行（`is_device = true`），
            // 否则这类设备在预览页会完全消失。
            _ => rows.push(device_to_row(d, None)),
        }
    }

    // ---- 行级过滤 ----
    let kw = q
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let civil = q
        .civil_code
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    rows.retain(|r| {
        if let Some(ref kw) = kw {
            let hit = r.name.contains(kw.as_str())
                || r.device_id.contains(kw.as_str())
                || r.channel_id.contains(kw.as_str());
            if !hit {
                return false;
            }
        }
        if let Some(on) = q.online {
            if r.online != on {
                return false;
            }
        }
        if let Some(ref prefix) = civil {
            // 行政区划是**通道**属性；没有通道的设备行不参与该过滤
            match r.civil_code.as_deref() {
                Some(c) if c.starts_with(prefix.as_str()) => {}
                _ => return false,
            }
        }
        true
    });

    let total = rows.len() as u64;
    let start = (((page - 1) as usize) * count as usize).min(rows.len());
    let end = (start + count as usize).min(rows.len());
    (rows[start..end].to_vec(), total, page, count)
}

/// GET /api/sy/camera/list
///
/// WVP 的同名端点返回的是**通道**列表（`CameraChannel`）。此前这里返回的是
/// 设备行（`is_device = true`、`channel_id == device_id`），照 live 页既有的
/// 过滤口径（`!c.is_device`）会被整批滤掉 → 通道树为空；默认 `count=15`
/// 还会进一步截断。现在与 `/list-with-child` 共用同一套通道行。
pub async fn camera_list(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let (rows, total, page, count) = camera_rows(&state, &q).await;
    let list_total = rows.len();
    Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
        "listTotal": list_total,
        "page": page,
        "count": count,
    })))
}

/// GET /api/sy/camera/list-with-child — every device with its child channels flattened
pub async fn camera_list_with_child(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let (rows, total, page, count) = camera_rows(&state, &q).await;
    let list_total = rows.len();
    Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
        "listTotal": list_total,
        "page": page,
        "count": count,
    })))
}

/// GET /api/sy/camera/list-for-mobile — slim rows, channels only
pub async fn camera_list_for_mobile(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let rows: Vec<CameraMobile> = channels.iter().map(channel_to_mobile).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": rows.len(),
    })))
}

/// GET /api/sy/camera/cont-with-child — alias of list-with-child (contract variant)
pub async fn camera_cont_with_child(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    camera_list_with_child(State(state), Query(q)).await
}

/// GET /api/sy/camera/list/box?min_lng=&min_lat=&max_lng=&max_lat=
pub async fn camera_list_box(
    State(state): State<AppState>,
    Query(q): Query<BoxQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let min_lng = q.min_lng.unwrap_or(f64::MIN);
    let min_lat = q.min_lat.unwrap_or(f64::MIN);
    let max_lng = q.max_lng.unwrap_or(f64::MAX);
    let max_lat = q.max_lat.unwrap_or(f64::MAX);
    let out: Vec<CameraMobile> = channels.iter().filter(|c| {
        match (c.longitude, c.latitude) {
            (Some(lng), Some(lat)) => lng >= min_lng && lng <= max_lng && lat >= min_lat && lat <= max_lat,
            _ => false,
        }
    }).map(channel_to_mobile).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": out,
        "total": out.len(),
    })))
}

/// GET /api/sy/camera/list/circle?lng=&lat=&radius=
pub async fn camera_list_circle(
    State(state): State<AppState>,
    Query(q): Query<CircleQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let (cx, cy, r) = (q.lng.unwrap_or(0.0), q.lat.unwrap_or(0.0), q.radius.unwrap_or(0.0));
    let out: Vec<CameraMobile> = channels.iter().filter(|c| {
        if let (Some(lng), Some(lat)) = (c.longitude, c.latitude) {
            let dx = lng - cx;
            let dy = lat - cy;
            (dx * dx + dy * dy).sqrt() <= r
        } else { false }
    }).map(channel_to_mobile).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": out,
        "total": out.len(),
    })))
}

/// GET /api/sy/camera/list/polygon?points=lng1,lat1;lng2,lat2;...
pub async fn camera_list_polygon(
    State(state): State<AppState>,
    Query(q): Query<PolygonQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let points_str = q.points.unwrap_or_default();
    let polygon: Vec<(f64, f64)> = points_str.split(';').filter_map(|p| {
        let mut parts = p.split(',');
        let lng = parts.next()?.trim().parse().ok()?;
        let lat = parts.next()?.trim().parse().ok()?;
        Some((lng, lat))
    }).collect();
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let out: Vec<CameraMobile> = channels.iter().filter(|c| {
        if let (Some(lng), Some(lat)) = (c.longitude, c.latitude) {
            point_in_polygon(lng, lat, &polygon)
        } else { false }
    }).map(channel_to_mobile).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": out,
        "total": out.len(),
    })))
}

fn point_in_polygon(lng: f64, lat: f64, polygon: &[(f64, f64)]) -> bool {
    if polygon.len() < 3 { return false; }
    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        if ((yi > lat) != (yj > lat))
            && (lng < (xj - xi) * (lat - yi) / (yj - yi + f64::EPSILON) + xi)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// GET /api/sy/camera/list/address?civil_code=...
pub async fn camera_list_address(
    State(state): State<AppState>,
    Query(q): Query<AddressQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let code = q.civil_code.unwrap_or_default();
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let out: Vec<CameraMobile> = if code.is_empty() {
        channels.iter().map(channel_to_mobile).collect()
    } else {
        channels.iter()
            .filter(|c| c.civil_code.as_deref().unwrap_or("").starts_with(&code))
            .map(channel_to_mobile)
            .collect()
    };
    Json(WVPResult::success(serde_json::json!({
        "list": out,
        "total": out.len(),
    })))
}

/// GET /api/sy/camera/list/ids?ids=GB1,GB2,...
pub async fn camera_list_ids(
    State(state): State<AppState>,
    Query(q): Query<IdsQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let ids_str = q.ids.unwrap_or_default();
    let wanted: Vec<&str> = ids_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let out: Vec<CameraMobile> = channels.iter()
        .filter(|c| {
            let ch_id = c.gb_device_id.as_deref().unwrap_or("");
            wanted.iter().any(|w| *w == ch_id)
        })
        .map(channel_to_mobile)
        .collect();
    Json(WVPResult::success(serde_json::json!({
        "list": out,
        "total": out.len(),
    })))
}

/// GET /api/sy/camera/meeting/list — channels with sub_count >= 1 (multi-channel devices)
pub async fn camera_meeting_list(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let channels = db::device::list_all_channels(&state.pool).await.unwrap_or_default();
    let devices = db::device::list_devices_paged(&state.pool, 1, 1, None, None).await.unwrap_or_default();
    let _ = devices; // devices not strictly needed; meeting = devices with sub_count > 0
    let out: Vec<serde_json::Value> = channels.iter().filter(|c| {
        c.sub_count.unwrap_or(0) > 0
    }).map(|c| {
        serde_json::json!({
            "deviceId": c.device_id,
            "channelId": c.gb_device_id,
            "name": c.name,
            "subCount": c.sub_count.unwrap_or(0),
            "manufacturer": c.manufacturer,
            "model": c.model,
            "status": c.status,
        })
    }).collect();
    Json(WVPResult::success(serde_json::json!({
        "list": out,
        "total": out.len(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_polygon_simple_square() {
        let poly = vec![(0.0,0.0),(10.0,0.0),(10.0,10.0),(0.0,10.0)];
        assert!(point_in_polygon(5.0, 5.0, &poly));
        assert!(!point_in_polygon(15.0, 5.0, &poly));
        assert!(!point_in_polygon(5.0, -1.0, &poly));
    }

    #[test]
    fn test_point_in_polygon_triangle() {
        let poly = vec![(0.0,0.0),(10.0,0.0),(5.0,10.0)];
        assert!(point_in_polygon(5.0, 3.0, &poly));
        assert!(!point_in_polygon(0.0, 5.0, &poly));
    }

    #[test]
    fn test_point_in_polygon_too_few_points() {
        assert!(!point_in_polygon(0.0, 0.0, &[]));
        assert!(!point_in_polygon(0.0, 0.0, &[(0.0,0.0)]));
        assert!(!point_in_polygon(0.0, 0.0, &[(0.0,0.0),(1.0,1.0)]));
    }

    #[test]
    fn test_device_to_row_with_channel() {
        let d = db::Device {
            id: 1,
            device_id: "34020000002000000001".to_string(),
            name: Some("TestCam".to_string()),
            on_line: Some(true),
            manufacturer: Some("Hikvision".to_string()),
            model: Some("DS-2CD".to_string()),
            ..Default::default()
        };
        let ch = db::DeviceChannel {
            id: 2,
            device_id: Some("34020000002000000001".to_string()),
            name: Some("Sub1".to_string()),
            gb_device_id: Some("34020000002000000002".to_string()),
            longitude: Some(121.0),
            latitude: Some(31.0),
            civil_code: Some("340200".to_string()),
            address: Some("Shanghai".to_string()),
            has_audio: Some(true),
            sub_count: Some(3),
            status: Some("ON".to_string()),
            ..Default::default()
        };
        let row = device_to_row(&d, Some(&ch));
        assert_eq!(row.device_id, "34020000002000000001");
        assert_eq!(row.channel_id, "34020000002000000002");
        assert_eq!(row.sub_count, 3);
        assert_eq!(row.longitude, Some(121.0));
        assert!(row.has_audio);
    }
}

// ---------- C4: 海康/宇视定制 control/play, control/stop, control/ptz 别名 ----------

#[derive(Debug, Deserialize)]
pub struct CameraControlQuery {
    // 前端/海康宇视客户端传的是 camelCase（`deviceId` / `channelId`），
    // 此前只认 snake_case —— 而旧实现无论参数是否解析成功都返回假成功，
    // 参数从未生效也无人发现。别名两种都收。
    #[serde(alias = "deviceId")]
    pub device_id: Option<String>,
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    pub command: Option<String>,
    pub speed: Option<i32>,
    #[serde(alias = "presetIndex", alias = "presetIndexNo")]
    pub preset: Option<i32>,
}

/// GET /api/sy/camera/control/play?deviceId=...&channelId=...
///
/// sy 视图的**别名路由**：真正转调 `play::play_start`。
///
/// 修正：此前这三个别名端点只打一条日志然后返回 `status: "started"` ——
/// 注释写着"转调 play_start"，代码里却**没有任何调用**：调用方以为已经开始
/// 播放，实际设备既没收到 INVITE，ZLM 也没开收流端口。
pub async fn camera_control_play(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<CameraControlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() {
        return Json(WVPResult::error("deviceId and channelId required"));
    }
    tracing::info!(
        "sy/camera/control/play → play_start {}/{}",
        device_id,
        channel_id
    );
    crate::handlers::play::play_start(State(state), Path((device_id, channel_id))).await
}

/// GET /api/sy/camera/control/stop?deviceId=...&channelId=...
///
/// 别名路由：真正转调 `play::play_stop`（含给设备发 BYE）。
pub async fn camera_control_stop(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<CameraControlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() {
        return Json(WVPResult::error("deviceId and channelId required"));
    }
    tracing::info!(
        "sy/camera/control/stop → play_stop {}/{}",
        device_id,
        channel_id
    );
    crate::handlers::play::play_stop(State(state), Path((device_id, channel_id))).await
}

/// GET /api/sy/camera/control/ptz?deviceId=...&channelId=...&command=...&speed=...&preset=...
///
/// 别名路由：真正转调 `device_control::device_ptz`（下发 SIP DeviceControl/PtzCmd）。
pub async fn camera_control_ptz(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<CameraControlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device_id = q.device_id.clone().unwrap_or_default();
    let channel_id = q.channel_id.clone().unwrap_or_default();
    let command = q.command.clone().unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() || command.is_empty() {
        return Json(WVPResult::error("deviceId, channelId and command required"));
    }
    tracing::info!(
        "sy/camera/control/ptz → device_ptz {}/{} cmd={} speed={:?} preset={:?}",
        device_id,
        channel_id,
        command,
        q.speed,
        q.preset,
    );
    let ptz = crate::handlers::device_control::PtzQuery {
        device_id: Some(device_id),
        channel_id: Some(channel_id),
        command: Some(command),
        speed: q.speed.map(|v| v.clamp(0, 255) as u8),
        preset_index: q.preset.map(|p| p as u32),
        guard_cmd: None
    };
    crate::handlers::device_control::device_ptz(State(state), Query(ptz)).await
}

#[cfg(test)]
mod camera_control_tests {
    use super::*;

    /// C4: CameraControlQuery 应当支持 device_id/channel_id/command/speed/preset
    #[test]
    fn test_camera_control_query_deserialize() {
        let q: CameraControlQuery = serde_json::from_value(serde_json::json!({
            "device_id": "34020000001320000001",
            "channel_id": "34020000001320000010",
            "command": "left",
            "speed": 5,
            "preset": 1,
        })).unwrap();
        assert_eq!(q.device_id.as_deref(), Some("34020000001320000001"));
        assert_eq!(q.command.as_deref(), Some("left"));
        assert_eq!(q.speed, Some(5));
        assert_eq!(q.preset, Some(1));
    }

    /// 前端传的是 camelCase（`deviceId`/`channelId`），必须能解析 ——
    /// 旧实现只认 snake_case 且无论成败都回假成功，参数从未生效。
    #[test]
    fn test_camera_control_query_camel_case() {
        let q: CameraControlQuery = serde_json::from_value(serde_json::json!({
            "deviceId": "34020000001320000001",
            "channelId": "34020000001320000010",
            "command": "right",
            "speed": 4,
            "presetIndex": 2,
        })).unwrap();
        assert_eq!(q.device_id.as_deref(), Some("34020000001320000001"));
        assert_eq!(q.channel_id.as_deref(), Some("34020000001320000010"));
        assert_eq!(q.command.as_deref(), Some("right"));
        assert_eq!(q.speed, Some(4));
        assert_eq!(q.preset, Some(2));
    }

    /// C4: 空 query 应全部为 None
    #[test]
    fn test_camera_control_query_empty() {
        let q: CameraControlQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(q.device_id.is_none());
        assert!(q.channel_id.is_none());
        assert!(q.command.is_none());
        assert!(q.speed.is_none());
        assert!(q.preset.is_none());
    }
}

#[cfg(test)]
mod camera_contract_tests {
    use super::*;
    use crate::test_support::app_state;

    async fn seed(state: &AppState) -> (String, String) {
        let now = "2026-01-01 00:00:00";
        // 一台在线设备（2 个通道）+ 一台离线设备（无通道）
        sqlx::query(
            "INSERT INTO gb_device (device_id, name, on_line, create_time, update_time) \
             VALUES ('34020000001320000001', '在线设备', 1, ?1, ?1)",
        )
        .bind(now)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO gb_device (device_id, name, on_line, create_time, update_time) \
             VALUES ('34020000001320000002', '离线设备', 0, ?1, ?1)",
        )
        .bind(now)
        .execute(&state.pool)
        .await
        .unwrap();
        for (ch, name) in [
            ("34020000001310000001", "通道一"),
            ("34020000001310000002", "通道二"),
        ] {
            sqlx::query(
                "INSERT INTO gb_device_channel (device_id, gb_device_id, name, civil_code, status, data_type, data_device_id, create_time, update_time) \
                 VALUES ('34020000001320000001', ?1, ?2, '340200', 'ON', 1, 1, ?3, ?3)",
            )
            .bind(ch)
            .bind(name)
            .bind(now)
            .execute(&state.pool)
            .await
            .unwrap();
        }
        ("34020000001320000001".to_string(), "34020000001320000002".to_string())
    }

    fn q(v: serde_json::Value) -> PageQuery {
        serde_json::from_value(v).expect("PageQuery")
    }

    /// 前端（WVP）用 `query` 做关键字，本平台历史拼写是 `keyword`；`online`/`status`
    /// 与 `civilCode` 也必须能绑上 —— 此前这些参数全部被静默丢弃。
    #[test]
    fn page_query_accepts_frontend_param_names() {
        let p = q(serde_json::json!({
            "page": 1, "count": 1000, "query": "在线", "online": true, "civilCode": "340201"
        }));
        assert_eq!(p.keyword.as_deref(), Some("在线"));
        assert_eq!(p.online, Some(true));
        assert_eq!(p.civil_code.as_deref(), Some("340201"));

        // 旧拼写仍然可用
        let legacy = q(serde_json::json!({"keyword": "k", "status": false, "civil_code": "34"}));
        assert_eq!(legacy.keyword.as_deref(), Some("k"));
        assert_eq!(legacy.online, Some(false));
        assert_eq!(legacy.civil_code.as_deref(), Some("34"));
    }

    /// `list-with-child` 必须返回**通道级**行，并带上设备维度真实 total。
    #[tokio::test]
    async fn list_with_child_expands_channels_and_reports_device_total() {
        let state = app_state().await;
        let (_dev, _off) = seed(&state).await;

        let res = camera_list_with_child(
            State(state.clone()),
            Query(q(serde_json::json!({"page": 1, "count": 1000}))),
        )
        .await;
        let d = res.0.data.unwrap();
        // 2 个通道 + 1 个无通道设备自身那一行
        assert_eq!(d["listTotal"], 3);
        assert_eq!(d["total"], 3, "total 是匹配的行数（WVP 在通道维度分页）");
        let list = d["list"].as_array().unwrap();
        assert!(list.iter().any(|r| r["channel_id"] == "34020000001310000001"));
        assert!(list.iter().any(|r| r["is_device"] == true));

        // 关键字过滤（前端参数名 query）
        let res = camera_list_with_child(
            State(state.clone()),
            Query(q(serde_json::json!({"query": "通道一"}))),
        )
        .await;
        let d = res.0.data.unwrap();
        assert_eq!(d["listTotal"], 1);
        assert_eq!(d["list"][0]["name"], "通道一");

        // online 过滤：离线设备（及其行）不该出现
        let res = camera_list_with_child(
            State(state.clone()),
            Query(q(serde_json::json!({"online": true, "count": 1000}))),
        )
        .await;
        let d = res.0.data.unwrap();
        // 在线设备有 2 个通道行；离线设备（无通道）那一行被滤掉
        assert_eq!(d["total"], 2, "只看在线设备");
        assert!(d["list"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["device_id"] == "34020000001320000001"));

        // 行政区划前缀过滤（作用在通道上）
        let res = camera_list_with_child(
            State(state.clone()),
            Query(q(serde_json::json!({"civilCode": "340201"}))),
        )
        .await;
        assert_eq!(res.0.data.unwrap()["listTotal"], 0);
        let res = camera_list_with_child(
            State(state.clone()),
            Query(q(serde_json::json!({"civilCode": "3402"}))),
        )
        .await;
        assert_eq!(res.0.data.unwrap()["listTotal"], 2);
    }

    /// `count=1000` 不能被静默截成 100（前端建树依赖它）。
    #[tokio::test]
    async fn count_1000_is_not_truncated_to_100() {
        let state = app_state().await;
        let now = "2026-01-01 00:00:00";
        for i in 1..=150 {
            sqlx::query(
                "INSERT INTO gb_device (device_id, name, on_line, create_time, update_time) \
                 VALUES (?1, ?2, 1, ?3, ?3)",
            )
            .bind(format!("3402000000132{:06}", i))
            .bind(format!("设备{i}"))
            .bind(now)
            .execute(&state.pool)
            .await
            .unwrap();
        }
        let res = camera_list_with_child(
            State(state.clone()),
            Query(q(serde_json::json!({"page": 1, "count": 1000}))),
        )
        .await;
        let d = res.0.data.unwrap();
        assert_eq!(d["listTotal"], 150, "1000 条上限内不应被截断");
        assert_eq!(d["total"], 150);
    }

    /// `/camera/list` 也必须是通道级行（此前是纯设备行，live 页的
    /// `!is_device` 过滤会把它们全滤掉 → 通道树为空）。
    #[tokio::test]
    async fn camera_list_returns_channel_level_rows() {
        let state = app_state().await;
        let _ = seed(&state).await;
        let res = camera_list(
            State(state.clone()),
            Query(q(serde_json::json!({"page": 1, "count": 1000}))),
        )
        .await;
        let d = res.0.data.unwrap();
        let list = d["list"].as_array().unwrap();
        assert!(
            list.iter().any(|r| r["is_device"] == false),
            "应包含真正的通道行，而不是只有设备行"
        );
        assert_eq!(d["listTotal"], 3);
    }
}
