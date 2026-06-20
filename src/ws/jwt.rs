//! Phase 7.3: WebSocket JWT validation.

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// Phase 7.3: JWT claims understood by the WS handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsClaims {
    pub sub: String,
    pub exp: i64,
    #[serde(default)]
    pub role: Option<String>,
}

/// Verify a JWT and extract claims. Mirrors the regular HTTP-side JWT validation
/// but kept separate so we can extend it (e.g. allow short-lived "WS-only" tokens).
pub fn verify_ws_jwt(token: &str, secret: &str) -> Result<WsClaims, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 30;
    let data = decode::<WsClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| format!("JWT invalid: {}", e))?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn make_token(secret: &str, exp_offset_secs: i64) -> String {
        let exp = chrono::Utc::now().timestamp() + exp_offset_secs;
        let claims = WsClaims { sub: "alice".into(), exp, role: Some("admin".into()) };
        encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
    }

    #[test]
    fn test_verify_ws_jwt_valid() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let token = make_token(secret, 60);
        let claims = verify_ws_jwt(&token, secret).unwrap();
        assert_eq!(claims.sub, "alice");
        assert_eq!(claims.role.as_deref(), Some("admin"));
    }

    #[test]
    fn test_verify_ws_jwt_expired() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let token = make_token(secret, -120);  // expired 2 min ago
        let result = verify_ws_jwt(&token, secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_ws_jwt_bad_secret() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let token = make_token(secret, 60);
        let result = verify_ws_jwt(&token, "wrong-secret-wrong-secret-wrong-secret-x");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_ws_jwt_garbage_token() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let result = verify_ws_jwt("not-a-jwt", secret);
        assert!(result.is_err());
    }
}
