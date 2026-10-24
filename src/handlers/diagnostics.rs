//! 管理员只读诊断接口；所有字段均为白名单摘要。

use axum::{extract::State, http::HeaderMap, Json};

use crate::error::AppError;
use crate::handlers::authz;
use crate::response::ApiResult;
use crate::AppState;

async fn classify_ping<F>(ping: F) -> (&'static str, Option<&'static str>, Option<u128>)
where
    F: std::future::Future<Output = redis::RedisResult<String>>,
{
    let started = std::time::Instant::now();
    match tokio::time::timeout(std::time::Duration::from_secs(1), ping).await {
        Ok(Ok(response)) if response == "PONG" => {
            ("connected", None, Some(started.elapsed().as_millis()))
        }
        Err(_) => ("unavailable", Some("ping_timeout"), None),
        _ => ("unavailable", Some("ping_failed"), None),
    }
}

#[utoipa::path(
    get,
    path = "/api/test/hook/list",
    tag = "system",
    operation_id = "admin_hook_diagnostics",
    responses(
        (status = 200, description = "最近已验证 Hook 的有界摘要", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "非管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn hook_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    Ok(Json(ApiResult::success(serde_json::json!({
        "retention": "in_memory_latest_per_type",
        "capturedTypes": ["on_play", "on_publish"],
        "items": state.hook_diagnostics.list(),
    }))))
}

#[utoipa::path(
    get,
    path = "/api/test/redis",
    tag = "system",
    operation_id = "admin_redis_diagnostics",
    responses(
        (status = 200, description = "Redis 和 StateStore 连通性摘要", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
        (status = 403, description = "非管理员"),
    ),
    security(("access_token" = [])),
)]
pub async fn redis_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResult<serde_json::Value>>, AppError> {
    authz::require_admin(&state, &headers).await?;
    if state.config.redis.is_none() {
        return Ok(Json(ApiResult::success(serde_json::json!({
            "backend": "memory",
            "status": "not_configured",
        }))));
    }
    let Some(mut connection) = state.redis.clone() else {
        return Ok(Json(ApiResult::success(serde_json::json!({
            "backend": "redis",
            "status": "unavailable",
            "reason": "connection_not_initialized",
        }))));
    };
    let (status, reason, latency_ms) =
        classify_ping(redis::cmd("PING").query_async::<_, String>(&mut connection)).await;
    let data = match status {
        "connected" => serde_json::json!({
            "backend": "redis",
            "status": "connected",
            "latencyMs": latency_ms,
        }),
        _ => serde_json::json!({
            "backend": "redis",
            "status": "unavailable",
            "reason": reason,
        }),
    };
    Ok(Json(ApiResult::success(data)))
}

#[cfg(test)]
mod tests {
    #[cfg(test)]
    #[tokio::test]
    async fn configured_redis_without_connection_is_unavailable_without_leaking_url() {
        let mut state = crate::test_support::app_state().await;
        let mut config = (*state.config).clone();
        config.redis = Some(crate::config::RedisConfig {
            url: "redis://secret-password@127.0.0.1:1".to_string(),
        });
        state.config = std::sync::Arc::new(config);
        let token = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes())
            .create_token("admin", 60)
            .unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("access-token", token.parse().unwrap());
        let response = super::redis_status(axum::extract::State(state), headers)
            .await
            .unwrap()
            .0;
        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("\"status\":\"unavailable\""));
        assert!(serialized.contains("connection_not_initialized"));
        assert!(!serialized.contains("secret-password"));
    }

    #[tokio::test]
    async fn redis_ping_timeout_has_fixed_sanitized_reason() {
        let result =
            super::classify_ping(std::future::pending::<redis::RedisResult<String>>()).await;
        assert_eq!(result.0, "unavailable");
        assert_eq!(result.1, Some("ping_timeout"));
    }

    #[tokio::test]
    async fn redis_pong_is_connected_and_unexpected_reply_is_unavailable() {
        let good = super::classify_ping(std::future::ready(Ok("PONG".to_string()))).await;
        assert_eq!(good.0, "connected");
        let bad = super::classify_ping(std::future::ready(Ok("NOT_PONG".to_string()))).await;
        assert_eq!(bad.0, "unavailable");
        assert_eq!(bad.1, Some("ping_failed"));
    }
}
