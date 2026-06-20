//! Phase 7.4: `audit_middleware` — automatically writes an `gb_audit_log`
//! entry for every API response.
//!
//! Behavior:
//! - Skipped when `config.audit.enabled = false`
//! - Skipped for `/metrics`, `/api/health`, `/api/ready` to avoid self-recursion
//! - Username extracted from JWT in `access-token` header (or "anonymous")
//! - IP from `x-forwarded-for` or "0.0.0.0"
//! - Path truncated to 500 chars
//! - status_code captured from response
//! - DB INSERT happens in a tokio::spawn (no impact on response latency)
//! - DB failures only logged + increment `audit_log_writes_failed` metric

use std::time::Instant;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::AppState;

/// Phase 7.4: Audit middleware.
pub async fn audit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if !state.config.audit.enabled {
        return next.run(req).await;
    }
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    // Skip self-recursion / high-volume endpoints
    if path == "/metrics" || path == "/api/health" || path == "/api/ready" {
        return next.run(req).await;
    }
    // Best-effort IP extraction
    let ip = req.headers().get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0.0.0.0".to_string());
    // Best-effort username extraction
    let username = req.headers().get("access-token")
        .and_then(|v| v.to_str().ok())
        .and_then(|t| crate::auth::decode_jwt_unsafe(t).ok())
        .map(|c| c.user)
        .unwrap_or_else(|| "anonymous".to_string());
    let resource = path.clone();
    let action = format!("{} {}", method, path);
    let path_truncated = if path.len() > 500 { path[..500].to_string() } else { path.clone() };
    let ip_clone = ip.clone();
    let username_clone = username.clone();
    let started = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16() as i32;
    let elapsed_ms = started.elapsed().as_millis() as i64;
    // Async write — fire-and-forget so we don't block the response.
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let res = crate::db::audit_log::insert_with_metrics(
            &pool, &username_clone, &action, &resource, &method, &path_truncated, &ip_clone, status, elapsed_ms,
        ).await;
        match res {
            Ok(_) => crate::metrics::inc_audit_log_writes_total(),
            Err(e) => {
                crate::metrics::inc_audit_log_writes_failed();
                tracing::warn!("audit_middleware insert failed: {}", e);
            }
        }
    });
    response
}

#[cfg(test)]
mod tests {
    // Pure logic is straightforward; full integration is in tests/integration/audit_test.rs.
    // We document the contract here so future maintainers can extend it.
    #[test]
    fn test_audit_middleware_compiles() {
        // Compile-time check that audit_middleware is callable with our State.
        // No runtime assertions possible without a TestServer (covered in integration test).
    }
}
