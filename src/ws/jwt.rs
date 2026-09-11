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
    /// 真实用户名。`/api/user/login` 签发的 token 里 `sub` 恒为字符串
    /// `"login"`，用户名放在 `userName`。
    ///
    /// 此前 `WsClaims` 没有这个字段，`ws_handler` 直接拿 `claims.sub` 当用户
    /// 身份去注册 WsHub —— 于是**所有登录用户在 WS 侧都是同一个身份 "login"**，
    /// 按用户定向推送/隔离自然不成立。
    #[serde(default, rename = "userName")]
    pub user_name: Option<String>,
}

impl WsClaims {
    /// 用于鉴权与身份标识的用户名：优先 `userName`，退回 `sub`。
    pub fn username(&self) -> String {
        self.user_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.sub.clone())
    }
}

/// 验证 JWT 并取出 WS 需要的 claims。
///
/// # 此前的严重缺陷
///
/// 这里用的是裸 `Validation::new(HS256)`，**没有设置期望 audience**。
/// 而 `jsonwebtoken` 在「token 带 `aud` 声明、但校验未配置期望 audience」时
/// 会直接返回 `InvalidAudience`。`/api/user/login` 签发的 token 一定带
/// `aud`（见 `auth::JwtKeys::new`），于是：
///
/// * **登录拿到的 token 永远无法建立 `/api/ws`**（设备状态/告警实时推送整条
///   链路不可用），报错就是 `JWT invalid: InvalidAudience`；
/// * 而单元测试自造的 token 是 `WsClaims { sub, exp, role }`，**没有 `aud`**，
///   于是测试全绿 —— 典型的"测试把 bug 固化了"。
///
/// 现与 HTTP 侧保持同一套校验参数（同样的 audience 与必需声明）。
pub fn verify_ws_jwt(token: &str, secret: &str) -> Result<WsClaims, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 30;
    validation.set_audience(&[crate::auth::AUDIENCE]);
    validation.set_required_spec_claims(&["sub", "aud", "exp"]);
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

    /// 造一个**与 `/api/user/login` 同形状**的 token（含 aud / userName）。
    ///
    /// 之前这里用的是 `WsClaims { sub, exp, role }` —— 没有 `aud`，
    /// 正好绕过了 `verify_ws_jwt` 缺失 audience 校验的 bug，测试因此全绿
    /// 而真实登录 token 一律被拒。现在必须带上 `aud`。
    fn make_token(secret: &str, exp_offset_secs: i64) -> String {
        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct LoginShapedClaims {
            sub: String,
            aud: String,
            userName: String,
            exp: i64,
            iat: i64,
            role: Option<String>,
        }
        let now = chrono::Utc::now().timestamp();
        let claims = LoginShapedClaims {
            sub: "login".into(),
            aud: crate::auth::AUDIENCE.into(),
            userName: "alice".into(),
            exp: now + exp_offset_secs,
            iat: now,
            role: Some("admin".into()),
        };
        encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
    }

    /// 回归守卫：**真实登录签发的 token** 必须能通过 WS 校验。
    /// （修复前这里会报 `InvalidAudience`，导致 /api/ws 对登录用户完全不可用。）
    #[test]
    fn test_verify_ws_jwt_accepts_real_login_token() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let login_token = crate::auth::JwtKeys::new(secret.as_bytes())
            .create_token("alice", 30)
            .expect("签发登录 token");
        let claims = verify_ws_jwt(&login_token, secret)
            .expect("登录 token 必须能用于 WebSocket 鉴权");
        assert_eq!(claims.sub, "login", "登录 token 的 sub 固定为 login");
        assert_eq!(
            claims.username(),
            "alice",
            "WS 侧身份必须取 userName，否则所有用户都是同一个身份"
        );
    }

    /// 缺少 aud 的 token 必须被拒绝（aud 是必需声明）。
    #[test]
    fn test_verify_ws_jwt_rejects_token_without_audience() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let exp = chrono::Utc::now().timestamp() + 60;
        let claims = WsClaims { sub: "alice".into(), exp, role: None, user_name: None };
        let token =
            encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap();
        assert!(verify_ws_jwt(&token, secret).is_err());
    }

    #[test]
    fn test_verify_ws_jwt_valid() {
        let secret = "test-secret-test-secret-test-secret-1234";
        let token = make_token(secret, 60);
        let claims = verify_ws_jwt(&token, secret).unwrap();
        assert_eq!(claims.username(), "alice");
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
