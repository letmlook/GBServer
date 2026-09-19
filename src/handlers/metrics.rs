use axum::extract::State;
use crate::metrics;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/metrics",
    tag = "system",
    operation_id = "metrics",
    summary = "Prometheus 指标（文本格式，非 JSON）",
    responses(
        (status = 200, description = "Prometheus exposition 文本",
         content_type = "text/plain"),
    ),
)]
pub async fn metrics_handler(State(_state): State<AppState>) -> (axum::http::StatusCode, String) {
    let body = metrics::gather();
    (axum::http::StatusCode::OK, body)
}
