//! Phase 7.5: `/api/health` (liveness) and `/api/ready` (readiness) handlers.
//!
//! - `/api/health` returns 200 unconditionally if the process is alive.
//!   Intentionally does NOT touch the DB / Redis — Kubernetes liveness probes
//!   must not be coupled to transient backend failures (otherwise the pod
//!   gets killed during a brief DB hiccup).
//!
//! - `/api/ready` returns 200 only when DB is reachable AND (in cluster mode)
//!   at least one cluster node is registered. Returns 503 otherwise so the
//!   load balancer can drain traffic from this pod.
//!
//! In `single_node_mode`, `/api/ready` only checks the DB.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::AppState;

/// Phase 7.5: Liveness — always 200.
pub async fn liveness() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "alive",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

/// Phase 7.5: Readiness — 200 if DB + (cluster OR single_node_mode) are OK.
pub async fn readiness(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db_ok = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let cluster_ok = if state.config.cluster.single_node_mode {
        true
    } else {
        state.cluster_registry.list_active().await.len() >= 1
    };

    let redis_ok = if let Some(redis) = state.redis.as_ref() {
        let mut conn = redis.clone();
        use redis::AsyncCommands;
        let res = tokio::time::timeout(
            Duration::from_secs(2),
            conn.get::<_, Option<String>>("gb:health:ping"),
        )
        .await;
        res.map(|r| r.is_ok()).unwrap_or(false)
    } else {
        true // Redis is optional
    };

    let all_ok = db_ok && cluster_ok && redis_ok;
    let body = serde_json::json!({
        "status": if all_ok { "ready" } else { "not_ready" },
        "checks": {
            "database": db_ok,
            "cluster": cluster_ok,
            "redis": redis_ok,
        },
        "clusterNodes": state.cluster_registry.list_active().await.len(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    let code = if all_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (code, Json(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_liveness_returns_200() {
        let (_status, body) = liveness().await;
        assert_eq!(body["status"], "alive");
    }
}
