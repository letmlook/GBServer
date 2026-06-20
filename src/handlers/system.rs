//! Phase 7.6: System info / stats / online users endpoints.
//!
//! Designed to mirror WVP-Pro's `SystemController` endpoints for frontend
//! compatibility. Returns basic runtime + cluster + DB aggregate counts.

use std::sync::OnceLock;
use std::time::Instant;

use axum::extract::State;
use axum::Json;

use crate::response::WVPResult;
use crate::state::StreamStateRepository;
use crate::AppState;

static STARTED_AT: OnceLock<chrono::DateTime<chrono::Utc>> = OnceLock::new();

fn started_at() -> chrono::DateTime<chrono::Utc> {
    *STARTED_AT.get_or_init(chrono::Utc::now)
}

/// GET /api/system/info — version + uptime + features + cluster node id
pub async fn system_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let now = chrono::Utc::now();
    let uptime = now.signed_duration_since(started_at()).num_seconds();
    Json(WVPResult::success(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "nodeId": state.cluster_registry.config().node_id,
        "startedAt": started_at().to_rfc3339(),
        "uptimeSeconds": uptime,
        "features": {
            "redis": state.redis.is_some(),
            "cluster": !state.config.cluster.single_node_mode,
            "audit": state.config.audit.enabled,
            "wsHub": true,
        },
        "redis": state.redis.is_some(),
    })))
}

/// GET /api/system/stats — aggregate counts (devices / streams / invites / JT / WS)
pub async fn system_stats(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // DB-side count (best-effort, never error)
    let devices_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
    let channels_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device_channel")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    // StateStore counts
    let online = state.state_repo.count_online_devices();
    let streams = state.state_repo.count_active_streams();
    let invites = state.state_repo.count_active_sessions();

    // JT counts (best-effort)
    let jt_terminals = state.state_repo.store.all_jt_terminals().len();
    let jt_sessions = state.state_repo.store.all_jt_media_sessions().len();

    // Cluster
    let cluster_nodes = state.cluster_registry.list_active().await;

    Json(WVPResult::success(serde_json::json!({
        "devices": { "total": devices_total, "online": online },
        "channels": { "total": channels_total },
        "streams": { "active": streams },
        "invites": { "active": invites },
        "jt1078": {
            "terminals": jt_terminals,
            "mediaSessions": jt_sessions,
        },
        "cluster": {
            "nodes": cluster_nodes.len(),
            "localNodeId": state.cluster_registry.config().node_id,
        },
    })))
}

/// GET /api/system/version — minimal version endpoint
pub async fn system_version() -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "GBServer",
    })))
}

/// GET /api/system/online-users — Phase 7.6: list online users (basic impl)
pub async fn online_users(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    // Without a gb_online_user table yet, derive from active WebSocket clients.
    // This is a best-effort approximation; the full implementation requires a
    // gb_online_user table (Phase 8 follow-up).
    let ws_count = state.ws_state.broadcast_count().await;
    let users: Vec<serde_json::Value> = (0..ws_count)
        .map(|i| serde_json::json!({
            "id": format!("ws-{}", i),
            "username": format!("ws-user-{}", i),
            "source": "ws",
        }))
        .collect();
    Json(WVPResult::success(serde_json::json!({
        "list": users,
        "total": users.len(),
        "note": "approximate count via WS clients; full gb_online_user table pending",
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_started_at_is_set() {
        let t = started_at();
        // Should be a recent timestamp (within last 60s)
        let elapsed = chrono::Utc::now().signed_duration_since(t).num_seconds();
        assert!(elapsed >= 0 && elapsed < 60);
    }

    #[tokio::test]
    async fn test_system_version_shape() {
        let body = system_version().await;
        // Body should contain version
        let json: serde_json::Value = serde_json::to_value(&body.0).unwrap();
        assert!(json["code"].as_i64().is_some());
        assert!(json["data"]["version"].is_string());
    }
}
