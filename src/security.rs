//! F3: Security helpers — JWT secret validation and sensitive log redaction.
//!
//! `validate_jwt_secret` is called at startup to refuse insecure secrets
//! (empty / too short / well-known defaults). `redact_sensitive` masks
//! password/secret/token fields in any string before logging.

/// Minimum length for a JWT secret (RFC 7518 / OWASP recommends ≥ 256 bits = 32 bytes).
pub const MIN_JWT_SECRET_LEN: usize = 32;

/// Built-in weak / demo secrets that must never be used in production.
const WEAK_SECRETS: &[&str] = &[
    "change-me",
    "change-me-in-production",
    "secret",
    "admin",
    "admin123",
    "password",
    "1234567890",
    "0123456789abcdef",
];

/// Returns Ok(()) if `secret` is acceptable for a production JWT signing key.
/// Returns Err with a human-readable reason otherwise.
pub fn validate_jwt_secret(secret: &str) -> Result<(), String> {
    if secret.is_empty() {
        return Err("JWT secret is empty — set GBSERVER__JWT__SECRET".into());
    }
    if secret.len() < MIN_JWT_SECRET_LEN {
        return Err(format!(
            "JWT secret too short ({} < {} chars); use a random ≥32-char string",
            secret.len(), MIN_JWT_SECRET_LEN
        ));
    }
    let lower = secret.to_ascii_lowercase();
    for weak in WEAK_SECRETS {
        if lower == *weak || lower.starts_with(weak) {
            return Err(format!(
                "JWT secret matches a known weak/default value (starts with '{}'); \
                 generate a fresh secret with `openssl rand -hex 32`",
                weak
            ));
        }
    }
    Ok(())
}

/// Mask any `<key>=<value>` / `"<key>":"<value>"` / `<key>: <value>` occurrence
/// of a sensitive key in `text`. Sensitive keys are case-insensitive.
const SENSITIVE_KEYS: &[&str] = &[
    "password", "passwd", "secret", "token", "jwt", "apikey", "api_key",
    "authorization", "access_token", "refresh_token", "private",
];

/// Returns `text` with sensitive values replaced by `***`.
pub fn redact_sensitive(text: &str) -> String {
    let mut out = text.to_string();
    for key in SENSITIVE_KEYS {
        // =value
        let eq_pat = format!("{}=", key);
        if let Some(idx) = out.to_ascii_lowercase().find(&eq_pat) {
            // Replace from `=` up to the next , } ) ] space or end-of-string
            let start = idx + eq_pat.len();
            let bytes = out.as_bytes();
            let mut end = start;
            while end < bytes.len() {
                let b = bytes[end];
                if b == b',' || b == b'}' || b == b')' || b == b']' || b == b' ' || b == b'\n' {
                    break;
                }
                end += 1;
            }
            out.replace_range(start..end, "***");
        }
        // "key":"value" or "key": "value"
        let colon_pat = format!("\"{}\":", key);
        if let Some(idx) = out.to_ascii_lowercase().find(&colon_pat) {
            let after_colon = idx + colon_pat.len();
            // skip whitespace
            let bytes = out.as_bytes();
            let mut start = after_colon;
            while start < bytes.len() && bytes[start] == b' ' {
                start += 1;
            }
            if start < bytes.len() && bytes[start] == b'"' {
                // find closing quote
                let mut end = start + 1;
                while end < bytes.len() && bytes[end] != b'"' {
                    if bytes[end] == b'\\' { end += 2; continue; }
                    end += 1;
                }
                if end < bytes.len() {
                    out.replace_range(start+1..end, "***");
                }
            }
        }
        // key: value (yaml-style)
        let colon_space_pat = format!("{}: ", key);
        if let Some(idx) = out.to_ascii_lowercase().find(&colon_space_pat) {
            let start = idx + colon_space_pat.len();
            let bytes = out.as_bytes();
            let mut end = start;
            while end < bytes.len() {
                let b = bytes[end];
                if b == b',' || b == b'}' || b == b')' || b == b']' || b == b'\n' {
                    break;
                }
                end += 1;
            }
            out.replace_range(start..end, "***");
        }
    }
    out
}

/// F3: 便捷宏 — 把任意 Display 值先 redact 再写入日志。
/// 用法：`info_redacted!("received {}", payload);` 等价 `info!("received {}", redact_sensitive(&payload.to_string()));`
#[macro_export]
macro_rules! info_redacted {
    ($($arg:tt)+) => {{
        let formatted = format!($($arg)+);
        tracing::info!("{}", $crate::security::redact_sensitive(&formatted));
    }};
}

#[macro_export]
macro_rules! warn_redacted {
    ($($arg:tt)+) => {{
        let formatted = format!($($arg)+);
        tracing::warn!("{}", $crate::security::redact_sensitive(&formatted));
    }};
}

#[macro_export]
macro_rules! debug_redacted {
    ($($arg:tt)+) => {{
        let formatted = format!($($arg)+);
        tracing::debug!("{}", $crate::security::redact_sensitive(&formatted));
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_jwt_secret_rejects_empty() {
        assert!(validate_jwt_secret("").is_err());
    }

    #[test]
    fn test_validate_jwt_secret_rejects_too_short() {
        assert!(validate_jwt_secret("short").is_err());
    }

    #[test]
    fn test_validate_jwt_secret_rejects_known_weak() {
        assert!(validate_jwt_secret("password1234567890123456789012345").is_err());
        assert!(validate_jwt_secret("change-me-in-production-32-chars-lo").is_err());
    }

    #[test]
    fn test_validate_jwt_secret_accepts_strong_random() {
        // 32+ char hex string with no weak prefix
        let secret = "a7f3c9e2b1d84f6a0e8c5b2d9f1a4e7c0b3d6f9a2e5c8b1d4f7a0e3c6b9d2f5";
        assert!(validate_jwt_secret(secret).is_ok());
    }

    #[test]
    fn test_redact_sensitive_equals_form() {
        let s = "password=admin123&user=foo";
        let r = redact_sensitive(s);
        assert!(r.contains("password=***"));
        assert!(!r.contains("admin123"));
    }

    #[test]
    fn test_redact_sensitive_json_form() {
        let s = "{\"password\":\"hunter2\",\"name\":\"x\"}";
        let r = redact_sensitive(s);
        assert!(r.contains("\"password\":\"***\""));
        assert!(!r.contains("hunter2"));
    }

    #[test]
    fn test_redact_sensitive_case_insensitive() {
        let s = "PASSWORD=foo JWT=hunter2";
        let r = redact_sensitive(s);
        assert!(r.contains("PASSWORD=***"));
        assert!(r.contains("JWT=***") || r.contains("jwt=***"));
        assert!(!r.contains("hunter2"));
    }

    #[test]
    fn test_redact_sensitive_yaml_form() {
        let s = "password: hunter2\nuser: foo";
        let r = redact_sensitive(s);
        assert!(r.contains("password: ***"));
        assert!(!r.contains("hunter2"));
    }

    #[test]
    fn test_redact_sensitive_preserves_non_sensitive() {
        let s = "user=alice&role=admin";
        let r = redact_sensitive(s);
        assert_eq!(r, "user=alice&role=admin");
    }

    #[test]
    fn test_min_jwt_secret_len_is_32() {
        assert_eq!(MIN_JWT_SECRET_LEN, 32);
    }

    /// F3: redact_sensitive 对 query-string 形式也生效
    #[test]
    fn test_redact_sensitive_query_string() {
        let s = "/api/login?password=hunter2&user=alice";
        let r = redact_sensitive(s);
        assert!(r.contains("password=***"));
        assert!(!r.contains("hunter2"));
        assert!(r.contains("user=alice"));
    }

    /// F3: redact_sensitive 对 apikey 头生效
    #[test]
    fn test_redact_sensitive_apikey_header() {
        let s = "X-API-Key: secret-key-abc-123";
        let r = redact_sensitive(s);
        assert!(r.contains("X-API-Key: ***"));
        assert!(!r.contains("secret-key-abc-123"));
    }

    /// F3: 边界 — 仅敏感键出现，无值，不应 panic
    #[test]
    fn test_redact_sensitive_no_value_after_key() {
        let s = "password=";
        let r = redact_sensitive(s);
        // 不 panic，内容不变
        assert_eq!(r, "password=");
    }

    /// F3: 多敏感字段同时脱敏
    #[test]
    fn test_redact_sensitive_multiple_fields() {
        let s = r#"password=foo&token=bar&user=baz"#;
        let r = redact_sensitive(s);
        assert!(r.contains("password=***"));
        assert!(r.contains("token=***"));
        assert!(r.contains("user=baz"));
    }
}
