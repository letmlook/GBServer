use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;

const HEADER_ACCESS_TOKEN: &str = "access-token";
const AUDIENCE: &str = "Audience";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub aud: String,
    pub userName: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Clone)]
pub struct JwtKeys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl JwtKeys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }

    pub fn create_token(&self, username: &str, exp_minutes: i64) -> Option<String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
        let exp = now + exp_minutes * 60;
        let claims = Claims {
            sub: "login".to_string(),
            aud: AUDIENCE.to_string(),
            userName: username.to_string(),
            exp,
            iat: now,
        };
        jsonwebtoken::encode(&Header::default(), &claims, &self.encoding).ok()
    }

    pub fn verify_token(&self, token: &str) -> Option<Claims> {
        let mut validation = Validation::default();
        validation.set_audience(&[AUDIENCE]);
        validation.set_required_spec_claims(&["sub", "aud", "exp", "userName"]);
        let data = decode::<Claims>(token, &self.decoding, &validation).ok()?;
        Some(data.claims)
    }
}

pub fn extract_token(req: &Request) -> Option<String> {
    extract_token_from_headers(req.headers())
}

pub fn extract_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(v) = headers.get(HEADER_ACCESS_TOKEN) {
        if let Ok(s) = v.to_str() {
            return Some(s.to_string());
        }
    }
    if let Some(v) = headers.get(AUTHORIZATION) {
        if let Ok(s) = v.to_str() {
            if s.starts_with("Bearer ") {
                return Some(s[7..].to_string());
            }
        }
    }
    None
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    // 1. Try JWT token first (access-token header or Authorization: Bearer)
    let token = extract_token(&request);
    if let Some(ref t) = token {
        let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
        if let Some(claims) = keys.verify_token(t) {
            // Audit logging (fire-and-forget, non-blocking)
            let method = request.method().to_string();
            let path = request.uri().path().to_string();
            let ip = extract_client_ip(request.headers());
            let pool = state.pool.clone();
            let username = claims.userName.clone();
            let action = determine_action(&method, &path);
            let resource = path.clone();

            tokio::spawn(async move {
                let _ = crate::db::audit_log::insert(
                    &pool, &username, &action, &resource, &method, &path, &ip, 200,
                ).await;
            });

            return next.run(request).await;
        }
    }

    // 2. Try API Key authentication (X-API-Key header or query param)
    let api_key = extract_api_key(&request);
    if let Some(ref key) = api_key {
        let pool = state.pool.clone();
        match crate::db::user_api_key::get_by_api_key(&pool, key).await {
            Ok(Some(api_key_record)) => {
                // Check if enabled
                if api_key_record.enable.unwrap_or(false) == false {
                    return (
                        axum::http::StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({"code": 401, "msg": "API Key已禁用", "data": null})),
                    ).into_response();
                }
                // Check if expired
                if let Some(expired_at) = api_key_record.expired_at {
                    let now_ts = SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64).unwrap_or(0);
                    if expired_at > 0 && now_ts > expired_at {
                        return (
                            axum::http::StatusCode::UNAUTHORIZED,
                            axum::Json(serde_json::json!({"code": 401, "msg": "API Key已过期", "data": null})),
                        ).into_response();
                    }
                }

                // Audit logging for API Key
                let method = request.method().to_string();
                let path = request.uri().path().to_string();
                let ip = extract_client_ip(request.headers());
                let action = determine_action(&method, &path);
                let resource = path.clone();
                let username = format!("apikey:{}", api_key_record.app.as_deref().unwrap_or("unknown"));

                tokio::spawn(async move {
                    let _ = crate::db::audit_log::insert(
                        &pool, &username, &action, &resource, &method, &path, &ip, 200,
                    ).await;
                });

                return next.run(request).await;
            }
            Ok(None) => {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({"code": 401, "msg": "API Key无效", "data": null})),
                ).into_response();
            }
            Err(e) => {
                tracing::warn!("Failed to verify API Key: {}", e);
            }
        }
    }

    // No valid auth found
    (
        axum::http::StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({"code": 401, "msg": "请登录后重新请求", "data": null})),
    ).into_response()
}

fn extract_api_key(req: &Request) -> Option<String> {
    // Check X-API-Key header
    if let Some(v) = req.headers().get("X-API-Key") {
        if let Ok(s) = v.to_str() {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    // Check query parameter
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some(value) = pair.strip_prefix("apiKey=") {
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string()
}

fn determine_action(method: &str, path: &str) -> String {
    if path.starts_with("/api/user/login") {
        return "登录".to_string();
    }
    if path.starts_with("/api/user/logout") {
        return "登出".to_string();
    }
    match method {
        "POST" => "新增".to_string(),
        "PUT" => "修改".to_string(),
        "DELETE" => "删除".to_string(),
        _ => "查询".to_string(),
    }
}
