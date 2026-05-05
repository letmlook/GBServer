use axum::{extract::State, Json};
use crate::metrics;
use crate::AppState;

pub async fn metrics_handler(State(_state): State<AppState>) -> (axum::http::StatusCode, String) {
    let body = metrics::gather();
    (axum::http::StatusCode::OK, body)
}
