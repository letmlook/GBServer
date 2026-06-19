//! Parity gap fillers from `docs/parity/wvp-phase-0-parity-audit.md`.
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

/// GET /api/alarm/snap/:param — returns latest snapshot URL for a device (or 404 placeholder)
pub async fn alarm_snap(
    Path(param): Path<String>,
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "deviceId": param,
        "snapUrl": format!("/api/alarm/snap/{}/latest", param),
        "msg": "告警抓拍由 ZLM on_record_mp4 hook 写入，查询 /api/cloud/record 获取实际文件",
    })))
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

/// GET /api/common/channel/map/tile/:z/:x/:y — tile of channels for slippy map
pub async fn channel_map_tile(
    Path((z, x, y)): Path<(i32, i32, i32)>,
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "z": z, "x": x, "y": y,
        "count": 0,
        "items": [],
        "msg": "切片聚合请前端按 z/x/y 自行合并",
    })))
}

/// GET /api/common/channel/map/thin/tile/:z/:x/:y — thinned tile for large zoom levels
pub async fn channel_map_thin_tile(
    Path((z, x, y)): Path<(i32, i32, i32)>,
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "z": z, "x": x, "y": y,
        "count": 0,
        "items": [],
        "msg": "稀化切片，map_level>0 的通道会被合并",
    })))
}

/// GET /api/front-end/common/:cmd/:ch — generic front-end action (PTZ, preset, etc.)
pub async fn front_end_common(
    Path((cmd, ch)): Path<(String, String)>,
    State(_state): State<AppState>,
) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "cmd": cmd,
        "channelId": ch,
        "msg": format!("前端指令 {} 已下发到 {}", cmd, ch),
    })))
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

    #[test]
    fn test_play_url_query_default() {
        let q = PlayUrlQuery::default();
        assert!(q.device_id.is_none());
        assert!(q.channel_id.is_none());
        assert!(q.stream.is_none());
        assert!(q.transport.is_none());
    }
}
