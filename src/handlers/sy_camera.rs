//! Hikvision/Uniview-style camera API (`/api/sy/camera/*`).
//! These endpoints expose the device+channel tables in the contract format
//! expected by Hikvision iSecure Center / Uniview clients.

use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::response::WVPResult;
use crate::AppState;

// ---------- shared DTOs ----------

/// Hikvision-style camera row. Combines a `gb_device` row with its first
/// channel (or itself when the device has no children).
#[derive(Serialize)]
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
    #[serde(default)]
    pub keyword: Option<String>,
}

// ---------- helpers ----------

fn opt_to_string(s: &Option<String>) -> String {
    s.as_deref().unwrap_or("").to_string()
}

fn device_to_row(d: &db::Device, ch: Option<&db::DeviceChannel>) -> CameraRow {
    let (channel_id, sub_count, has_audio, longitude, latitude, civil_code, address) = if let Some(c) = ch {
        (
            opt_to_string(&c.gb_device_id),
            c.sub_count.unwrap_or(0),
            c.has_audio.unwrap_or(false),
            c.longitude,
            c.latitude,
            c.civil_code.clone(),
            c.address.clone(),
        )
    } else {
        (d.device_id.clone(), 0, false, None, None, None, None)
    };
    CameraRow {
        id: d.id,
        device_id: d.device_id.clone(),
        channel_id,
        name: opt_to_string(&d.name),
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

/// GET /api/sy/camera/list
pub async fn camera_list(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15);
    let kw = q.keyword.as_deref();
    let devices = db::device::list_devices_paged(&state.pool, page, count, kw, None)
        .await.unwrap_or_default();
    let rows: Vec<CameraRow> = devices.iter().map(|d| device_to_row(d, None)).collect();
    let total = db::device::count_devices(&state.pool, kw, None).await.unwrap_or(0);
    Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": total,
        "page": page,
        "count": count,
    })))
}

/// GET /api/sy/camera/list-with-child — every device with its child channels flattened
pub async fn camera_list_with_child(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(15);
    let kw = q.keyword.as_deref();
    let devices = db::device::list_devices_paged(&state.pool, page, count, kw, None)
        .await.unwrap_or_default();
    let mut rows: Vec<CameraRow> = Vec::with_capacity(devices.len());
    for d in &devices {
        let parent_id = d.device_id.clone();
        let channels = db::device::list_channels_for_device(&state.pool, &parent_id)
            .await.unwrap_or_default();
        if channels.is_empty() {
            rows.push(device_to_row(d, None));
        } else {
            for ch in &channels {
                let mut row = device_to_row(d, Some(ch));
                row.parent_device_id = Some(parent_id.clone());
                rows.push(row);
            }
        }
    }
    Json(WVPResult::success(serde_json::json!({
        "list": rows,
        "total": rows.len(),
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
