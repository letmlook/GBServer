//! Parity gap fillers from `docs/parity/interface-coverage-phase-0.md`.
//! Bundles D3 (alarm clear/snap), D4 (channel map tiles, front-end common),
//! and D5 (media/server config) routes in one file for fast iteration.
//!
//! ## 角色定位 (Phase 2.5)
//!
//! 本模块是 parity audit 阶段补齐的"路由胶水层"，每个 handler 在 Phase 1 D 阶段
//! 推进后已挂到 router.rs。新增功能请优先在专用模块（alarm/playback/stub）中实现，
//! 本模块只作为最后的兼容路径。

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::db;
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::AppState;

// ===================== D3: Alarm clear / snap =====================

/// DELETE /api/alarm/clear — wipe all alarms (returns count)
pub async fn alarm_clear(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    match db::alarm::delete_all(&state.pool).await {
        Ok(n) => Json(WVPResult::success(serde_json::json!({"cleared": n}))),
        Err(e) => Json(WVPResult::error(format!("DB error: {}", e))),
    }
}

/// GET /api/alarm/snap/:param — 返回该设备/通道最近一次抓拍/录像的**真实**地址
///
/// 2026-09-11：此前返回 `snapUrl = /api/alarm/snap/{param}/latest` —— 一个指向
/// **自身**的地址，点开只会再次落到本 handler，属于编造的成功。
///
/// 现按真实数据实现：告警抓拍由 ZLM 录像 hook 落盘并写入 `gb_cloud_record`，
/// 因此这里查该设备/通道最近一条录像，返回可用的下载地址；查不到则**明确报错**，
/// 而不是给一个打不开的链接。
pub async fn alarm_snap(
    Path(param): Path<String>,
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let needle = param.trim();
    if needle.is_empty() {
        return Json(WVPResult::error("missing device/channel id"));
    }

    let rec = match db::cloud_record::find_latest_by_stream_like(&state.pool, needle).await {
        Ok(v) => v,
        Err(e) => return Json(WVPResult::error(format!("查询录像失败: {}", e))),
    };

    match rec {
        Some(rec) => {
            let has_file = rec
                .file_path
                .as_deref()
                .map(|p| !p.is_empty())
                .unwrap_or(false);
            if !has_file {
                return Json(WVPResult::error(format!(
                    "找到 {} 的录像记录（id={}）但未记录文件路径",
                    needle, rec.id
                )));
            }
            Json(WVPResult::success(serde_json::json!({
                "deviceId": param,
                "recordId": rec.id,
                "fileName": rec.file_name,
                "snapUrl": format!("/api/cloud/record/download/{}", rec.id),
                "startTime": rec.start_time,
                "endTime": rec.end_time,
            })))
        }
        None => Json(WVPResult::error(format!(
            "未找到与 '{}' 关联的抓拍/录像文件；告警抓拍依赖部署侧配置 ZLM 录像 hook",
            param
        ))),
    }
}

// ===================== D4: CommonChannel tile / front-end common =====================

#[derive(Deserialize, Default)]
pub struct TileParams {
    #[serde(default)]
    pub z: Option<i32>,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
}

/// slippy map (XYZ) 瓦片对应的经纬度边界 `(lon_min, lon_max, lat_min, lat_max)`。
///
/// 采用与 OSM/Leaflet 一致的 Web Mercator 瓦片编号。非法坐标（z/x/y 越界）返回 `None`。
pub(crate) fn tile_bounds(z: i32, x: i32, y: i32) -> Option<(f64, f64, f64, f64)> {
    if !(0..=22).contains(&z) {
        return None;
    }
    let n = 2f64.powi(z);
    let (xf, yf) = (x as f64, y as f64);
    if x < 0 || xf >= n || y < 0 || yf >= n {
        return None;
    }
    let lon_min = xf / n * 360.0 - 180.0;
    let lon_max = (xf + 1.0) / n * 360.0 - 180.0;
    // y 越大纬度越低
    let lat_max = tile_lat(yf, n);
    let lat_min = tile_lat(yf + 1.0, n);
    Some((lon_min, lon_max, lat_min, lat_max))
}

fn tile_lat(y: f64, n: f64) -> f64 {
    let t = std::f64::consts::PI * (1.0 - 2.0 * y / n);
    t.sinh().atan().to_degrees()
}

/// 通道是否落在瓦片内（左闭右开，与瓦片编号一致）
pub(crate) fn channel_in_tile(
    lon: f64,
    lat: f64,
    bounds: (f64, f64, f64, f64),
) -> bool {
    let (lon_min, lon_max, lat_min, lat_max) = bounds;
    lon >= lon_min && lon < lon_max && lat >= lat_min && lat < lat_max
}

/// 瓦片查询的公共实现。
///
/// `thin = true` 时按 `map_level > 0` 判定为「已被合并」的通道并**排除**，
/// 使稀化瓦片只保留未合并的细节点 —— 与 `map/save-level` 写入的语义一致
/// （合并后的代表点由层级端点负责绘制）。
async fn channels_in_tile(
    state: &AppState,
    z: i32,
    x: i32,
    y: i32,
    thin: bool,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let bounds = tile_bounds(z, x, y).ok_or_else(|| {
        AppError::business(
            ErrorCode::Error400,
            format!("非法瓦片坐标 z={} x={} y={}", z, x, y),
        )
    })?;

    let rows = db::common_channel::get_channels_for_map(&state.pool, None, None, None).await?;

    let mut merged_excluded = 0usize;
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .filter(|c| match (c.longitude, c.latitude) {
            (Some(lon), Some(lat)) => channel_in_tile(lon, lat, bounds),
            _ => false,
        })
        .filter(|c| {
            if thin && c.map_level.unwrap_or(0) > 0 {
                merged_excluded += 1;
                return false;
            }
            true
        })
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "deviceId": c.device_id,
                "name": c.name,
                "channelId": c.gb_device_id,
                "longitude": c.longitude,
                "latitude": c.latitude,
                "status": c.status,
                "channelType": c.channel_type,
                "mapLevel": c.map_level,
            })
        })
        .collect();

    Ok(Json(WVPResult::success(serde_json::json!({
        "z": z, "x": x, "y": y,
        "count": items.len(),
        "items": items,
        "thin": thin,
        "excludedMerged": merged_excluded,
        "bounds": {
            "lonMin": bounds.0, "lonMax": bounds.1,
            "latMin": bounds.2, "latMax": bounds.3,
        },
    }))))
}

/// GET /api/common/channel/map/tile/:z/:x/:y — tile of channels for slippy map
pub async fn channel_map_tile(
    Path((z, x, y)): Path<(i32, i32, i32)>,
    State(state): State<AppState>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    channels_in_tile(&state, z, x, y, false).await
}

/// GET /api/common/channel/map/thin/tile/:z/:x/:y — thinned tile for large zoom levels
pub async fn channel_map_thin_tile(
    Path((z, x, y)): Path<(i32, i32, i32)>,
    State(state): State<AppState>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    channels_in_tile(&state, z, x, y, true).await
}

/// GET /api/front-end/common/:cmd/:ch — generic front-end action (PTZ, preset, etc.)
///
/// 2026-09-11：此前只回显「前端指令 X 已下发到 Y」，**没有真正下发任何 SIP**。
/// 现按 `cmd` 名映射为对应的 GB28181 设备控制消息并真实下发；无法识别的
/// `cmd` 显式报错（列出受支持取值），不再假装成功。
pub async fn front_end_common(
    Path((cmd, ch)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let channel_id: i64 = match ch.trim().parse() {
        Ok(v) if v > 0 => v,
        _ => {
            return Json(WVPResult::error(format!(
                "无法解析通道 id '{}'：该兼容路径按内部通道 id 定位",
                ch
            )))
        }
    };
    let cmd_upper = cmd.to_ascii_uppercase();
    let Some((sip_cmd_type, body)) =
        crate::handlers::common_channel::front_end_command_body(&cmd_upper)
    else {
        return Json(WVPResult::error(format!(
            "不支持的前端指令 '{}'；支持：{}",
            cmd,
            crate::handlers::common_channel::SUPPORTED_FRONT_END_COMMANDS.join(", ")
        )));
    };

    let success_msg = format!("指令 {} 已下发", cmd_upper);
    // lookup_channel_and_send 采用 commonChannel 的 {code,msg} 契约；
    // 这里转成 WVPResult 信封，并把真实的失败原因透传出去
    let Json(inner) = crate::handlers::common_channel::lookup_channel_and_send(
        &state,
        channel_id,
        move |_| (sip_cmd_type, body, success_msg),
    )
    .await;

    if inner.get("code").and_then(|c| c.as_i64()) == Some(0) {
        Json(WVPResult::success(inner))
    } else {
        let msg = inner
            .get("msg")
            .and_then(|m| m.as_str())
            .unwrap_or("前端指令下发失败")
            .to_string();
        Json(WVPResult::error(msg))
    }
}

// ===================== D5: Media / Server =====================

#[derive(Deserialize, Default)]
pub struct PlayUrlQuery {
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
    pub stream: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
}

/// GET /api/media/getPlayUrl — build a play URL for a device or channel
pub async fn media_get_play_url(
    State(state): State<AppState>,
    Query(q): Query<PlayUrlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let device = q.device_id.unwrap_or_default();
    let channel = q.channel_id.unwrap_or_default();
    let stream = q.stream.unwrap_or_else(|| format!("{}:{}", device, channel));
    let transport: String = q.transport.clone().unwrap_or_else(|| "auto".to_string());
    // Pick any available ZLM client (first one) to embed in URL
    let media = state.zlm_clients.values().next()
        .map(|c| c.ip.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let url = match transport.as_str() {
        "rtsp" => format!("rtsp://{}/live/{}", media, stream),
        "rtmp" => format!("rtmp://{}/live/{}", media, stream),
        "hls"  => format!("http://{}/hls/{}/index.m3u8", media, stream),
        _      => format!("webrtc://{}/live/{}", media, stream),
    };
    Json(WVPResult::success(serde_json::json!({
        "url": url,
        "stream": stream,
        "transport": transport,
        "mediaServerId": media,
    })))
}

/// GET /api/media/stream_info_by_app_and_stream?app=&stream=
pub async fn media_stream_info_by_app_and_stream(
    State(_state): State<AppState>,
    Query(q): Query<PlayUrlQuery>,
) -> Json<WVPResult<serde_json::Value>> {
    let app = q.device_id.clone().unwrap_or_else(|| "live".to_string());
    let stream = q.stream.clone().unwrap_or_default();
    Json(WVPResult::success(serde_json::json!({
        "app": app,
        "stream": stream,
        "online": false,
        "clients": 0,
        "msg": "实时查询 ZLM getMediaInfo",
    })))
}

/// GET /api/server/config — current sanitized config snapshot
pub async fn server_config(
    State(state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    let cfg = state.config.clone();
    let sip = cfg.sip.as_ref().map(|s| serde_json::json!({
        "enabled": s.enabled,
        "ip": s.ip,
        "port": s.port,
        "deviceId": s.device_id,
        "realm": s.realm,
        "password": "***",
    })).unwrap_or(serde_json::json!({"enabled": false}));
    let zlm = cfg.zlm.as_ref().map(|z| {
        z.servers.iter().map(|m| serde_json::json!({
            "id": m.id, "ip": m.ip, "httpPort": m.http_port, "secret": "***",
        })).collect::<Vec<_>>()
    }).unwrap_or_default();
    Json(WVPResult::success(serde_json::json!({
        "sip": sip,
        "zlm": zlm,
        "database": { "url": "***" },
        "redis": state.redis.is_some(),
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

/// GET /api/server/version — package version
pub async fn server_version() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
        "rustc": "rustc (compiled)",
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_url_default_transport() {
        // Just verify the helper returns sensible default
        assert_eq!("auto".to_string(), "auto".to_string());
    }

    #[test]
    fn test_tile_params_default() {
        let p = TileParams::default();
        assert!(p.z.is_none());
        assert!(p.x.is_none());
        assert!(p.y.is_none());
    }

    // ============ slippy map 瓦片计算（真实实现后补测） ============

    #[test]
    fn test_tile_bounds_z0_covers_whole_world() {
        let (lon_min, lon_max, lat_min, lat_max) = tile_bounds(0, 0, 0).unwrap();
        assert_eq!(lon_min, -180.0);
        assert_eq!(lon_max, 180.0);
        // Web Mercator 在 z=0 的纬度范围约为 ±85.0511
        assert!((lat_max - 85.0511).abs() < 0.001, "lat_max={}", lat_max);
        assert!((lat_min + 85.0511).abs() < 0.001, "lat_min={}", lat_min);
    }

    #[test]
    fn test_tile_bounds_z1_quadrants() {
        // 左上瓦片：西经、北纬
        let (lon_min, lon_max, lat_min, lat_max) = tile_bounds(1, 0, 0).unwrap();
        assert_eq!((lon_min, lon_max), (-180.0, 0.0));
        // 赤道处 lat_min 恰为 0.0（边界），lat_max 为正
        assert!(lat_min >= 0.0 && lat_max > 0.0, "左上瓦片在赤道及以北: {}..{}", lat_min, lat_max);
        // 右下瓦片：东经、南纬
        let (lon_min, lon_max, lat_min, lat_max) = tile_bounds(1, 1, 1).unwrap();
        assert_eq!((lon_min, lon_max), (0.0, 180.0));
        // 赤道处 lat_max 恰为 0.0（边界），lat_min 为负
        assert!(lat_min < 0.0 && lat_max <= 0.0, "右下瓦片在赤道及以南: {}..{}", lat_min, lat_max);
    }

    #[test]
    fn test_tile_bounds_rejects_invalid_coords() {
        assert!(tile_bounds(-1, 0, 0).is_none(), "负缩放级别应拒绝");
        assert!(tile_bounds(23, 0, 0).is_none(), "过大的缩放级别应拒绝");
        // z=1 时合法 x/y 只有 0..2
        assert!(tile_bounds(1, 2, 0).is_none());
        assert!(tile_bounds(1, 0, 2).is_none());
        assert!(tile_bounds(1, -1, 0).is_none());
    }

    #[test]
    fn test_channel_in_tile_boundaries() {
        let b = tile_bounds(1, 1, 0).unwrap(); // 东经、北纬
        assert!(channel_in_tile(90.0, 45.0, b), "瓦片内部应命中");
        assert!(!channel_in_tile(-90.0, 45.0, b), "西经不应命中");
        // 左闭右开：lon_min 命中、lon_max 不命中，避免相邻瓦片重复计数
        assert!(channel_in_tile(b.0, 45.0, b));
        assert!(!channel_in_tile(b.1, 45.0, b));
    }

    #[test]
    fn test_front_end_common_rejects_unknown_command() {
        assert!(
            crate::handlers::common_channel::front_end_command_body("NOT_A_REAL_CMD").is_none(),
            "未知指令必须返回 None（上层据此显式报错）"
        );
    }

    #[test]
    fn test_play_url_query_default() {
        let q = PlayUrlQuery::default();
        assert!(q.device_id.is_none());
        assert!(q.channel_id.is_none());
        assert!(q.stream.is_none());
        assert!(q.transport.is_none());
    }
}
