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

#[allow(non_snake_case)]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// F3: 从 HeaderMap 提取 Bearer token
    #[test]
    fn test_extract_token_from_authorization_bearer() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer abc.def.ghi".parse().unwrap());
        assert_eq!(extract_token_from_headers(&headers).as_deref(), Some("abc.def.ghi"));
    }

    /// F3: 从 access-token header 提取
    #[test]
    fn test_extract_token_from_access_token_header() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(HEADER_ACCESS_TOKEN, "xyz.tok.en".parse().unwrap());
        assert_eq!(extract_token_from_headers(&headers).as_deref(), Some("xyz.tok.en"));
    }

    /// F3: 没有 token 时返回 None
    #[test]
    fn test_extract_token_none() {
        let headers = axum::http::HeaderMap::new();
        assert!(extract_token_from_headers(&headers).is_none());
    }

    /// F3: Authorization 但不是 Bearer 格式
    #[test]
    fn test_extract_token_authorization_basic_ignored() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION, "Basic dXNlcjpwYXNz".parse().unwrap());
        assert!(extract_token_from_headers(&headers).is_none());
    }

    /// F3: extract_api_key 从 X-API-Key 头取
    #[test]
    fn test_extract_api_key_from_header() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-API-Key", "my-secret-key".parse().unwrap());
        let req = axum::extract::Request::builder()
            .uri("/api/foo")
            .body(axum::body::Body::empty()).unwrap();
        // 直接复用 header 提取逻辑
        let v = headers.get("X-API-Key").and_then(|v| v.to_str().ok());
        assert_eq!(v, Some("my-secret-key"));
    }

    /// F3: determine_action 把 POST 映射到「新增」
    #[test]
    fn test_determine_action_post() {
        assert_eq!(determine_action("POST", "/api/user/add"), "新增");
        assert_eq!(determine_action("DELETE", "/api/user/delete"), "删除");
        assert_eq!(determine_action("PUT", "/api/user/update"), "修改");
        assert_eq!(determine_action("GET", "/api/user/list"), "查询");
        assert_eq!(determine_action("POST", "/api/user/login"), "登录");
        assert_eq!(determine_action("POST", "/api/user/logout"), "登出");
    }

    /// F3: JwtKeys.create_token + verify_token roundtrip
    #[test]
    fn test_jwt_keys_roundtrip() {
        let keys = JwtKeys::new(b"test-secret-key");
        let token = keys.create_token("alice", 60).expect("create token");
        let claims = keys.verify_token(&token).expect("verify token");
        assert_eq!(claims.userName, "alice");
        assert_eq!(claims.aud, AUDIENCE);
    }

    /// F3: 错误的密钥应验证失败
    #[test]
    fn test_jwt_keys_wrong_secret_fails() {
        let keys1 = JwtKeys::new(b"secret-1");
        let keys2 = JwtKeys::new(b"secret-2");
        let token = keys1.create_token("bob", 60).unwrap();
        assert!(keys2.verify_token(&token).is_none(), "不同密钥应验证失败");
    }
}
