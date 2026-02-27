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
    pub sub: String,      // "login"
    pub aud: String,      // Audience
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

/// 从请求头获取 access-token（与前端 access-token 一致）
pub fn extract_token(req: &Request) -> Option<String> {
    extract_token_from_headers(req.headers())
}

/// 从 HeaderMap 获取 token（用于 handler 内）
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

/// 认证中间件：未带有效 token 的请求返回 401（需配合 from_fn_with_state 传入 AppState）
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let token = match extract_token(&request) {
        Some(t) => t,
        None => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({"code": 401, "msg": "请登录后重新请求", "data": null})),
            )
                .into_response();
        }
    };
    let keys = JwtKeys::new(state.config.jwt.secret.as_bytes());
    if keys.verify_token(&token).is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"code": 401, "msg": "请登录后重新请求", "data": null})),
        )
            .into_response();
    }
    next.run(request).await
}
