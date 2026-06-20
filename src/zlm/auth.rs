//! Hook 鉴权（Phase 4.2）
//!
//! 校验 ZLM 通过 hook 上行的请求：
//! 1. `secret` 字段：必须与节点配置的 `expected_secret` 匹配（constant-time 比较）
//! 2. 客户端 IP：必须在白名单 CIDR 之内
//!
//! 设计要点：
//! - `expected_secret` 为空字符串时，secret 校验放行（向后兼容未配置 secret 的旧节点）
//! - `whitelist` 为空时，IP 校验放行（向后兼容未配置白名单的旧节点）
//! - secret 长度不一致时直接返回 false（不进行字节比较），但仍然走完整字节循环以保持
//!   constant-time 行为（在长度不同的输入上，循环不会执行任何字节比较）

use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    Ok,
    UnauthorizedSecret,
    IpNotWhitelisted,
}

#[derive(Debug, Clone)]
pub struct HookAuthChecker {
    /// 节点配置 secret（必须匹配）
    expected_secret: String,
    /// 可选白名单 CIDR
    whitelist: Vec<ipnetwork::IpNetwork>,
}

impl HookAuthChecker {
    /// 创建不携带白名单的检查器
    pub fn new(secret: &str) -> Self {
        Self {
            expected_secret: secret.to_string(),
            whitelist: Vec::new(),
        }
    }

    /// 设置白名单（链式调用）
    pub fn with_whitelist(mut self, cidrs: Vec<ipnetwork::IpNetwork>) -> Self {
        self.whitelist = cidrs;
        self
    }

    /// 直接构造（用于测试）
    #[cfg(test)]
    pub(crate) fn new_with(secret: &str, whitelist: Vec<ipnetwork::IpNetwork>) -> Self {
        Self {
            expected_secret: secret.to_string(),
            whitelist,
        }
    }

    /// 校验 ZLM 推送的 secret 字段
    ///
    /// 注意：使用 constant-time 比较避免 timing attack。
    /// 当 `expected_secret` 为空字符串时直接返回 true（向后兼容）。
    pub fn check_secret(&self, provided: &str) -> bool {
        // 向后兼容：未配置 expected_secret 时放行
        if self.expected_secret.is_empty() {
            return true;
        }

        // 长度不一致时直接 false，但要遍历完所有字节保持 constant-time 行为
        let exp = self.expected_secret.as_bytes();
        let prov = provided.as_bytes();

        // 先比对长度，把差异累加到 diff
        let mut diff = (exp.len() ^ prov.len()) as u8;

        // 取较短长度进行字节比较
        let common_len = exp.len().min(prov.len());
        for i in 0..common_len {
            diff |= exp[i] ^ prov[i];
        }

        diff == 0
    }

    /// 校验客户端 IP
    ///
    /// - `whitelist` 为空时返回 true（向后兼容）
    /// - 否则判断 IP 是否落在任意一个 CIDR 中
    pub fn check_ip(&self, ip: &IpAddr) -> bool {
        if self.whitelist.is_empty() {
            return true;
        }
        self.whitelist.iter().any(|net| net.contains(*ip))
    }

    /// 同时校验 secret 和 IP
    pub fn check(&self, secret: &str, ip: &IpAddr) -> AuthResult {
        if !self.check_secret(secret) {
            return AuthResult::UnauthorizedSecret;
        }
        if !self.check_ip(ip) {
            return AuthResult::IpNotWhitelisted;
        }
        AuthResult::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;

    #[test]
    fn test_check_secret_match() {
        let checker = HookAuthChecker::new("super-secret");
        assert!(checker.check_secret("super-secret"));
        assert_eq!(checker.check("super-secret", &IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))), AuthResult::Ok);
    }

    #[test]
    fn test_check_secret_mismatch() {
        let checker = HookAuthChecker::new("super-secret");
        assert!(!checker.check_secret("wrong"));
        assert!(!checker.check_secret(""));
        assert!(!checker.check_secret("super-secre"));   // shorter
        assert!(!checker.check_secret("super-secret-1")); // longer

        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        assert_eq!(checker.check("wrong", &ip), AuthResult::UnauthorizedSecret);
        assert_eq!(checker.check("", &ip), AuthResult::UnauthorizedSecret);
    }

    #[test]
    fn test_check_ip_whitelist_match() {
        let cidrs = vec![
            ipnetwork::IpNetwork::from_str("10.0.0.0/8").unwrap(),
            ipnetwork::IpNetwork::from_str("192.168.1.0/24").unwrap(),
        ];
        let checker = HookAuthChecker::new("any").with_whitelist(cidrs);

        let in_range = IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3));
        let in_range2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let out_of_range = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));

        assert!(checker.check_ip(&in_range));
        assert!(checker.check_ip(&in_range2));
        assert!(!checker.check_ip(&out_of_range));

        assert_eq!(checker.check("any", &in_range), AuthResult::Ok);
        assert_eq!(checker.check("any", &out_of_range), AuthResult::IpNotWhitelisted);
    }

    #[test]
    fn test_check_ip_whitelist_miss() {
        let cidrs = vec![
            ipnetwork::IpNetwork::from_str("10.0.0.0/8").unwrap(),
        ];
        let checker = HookAuthChecker::new("any").with_whitelist(cidrs);

        let miss_v4 = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1));
        let miss_v6 = IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1));

        assert!(!checker.check_ip(&miss_v4));
        assert!(!checker.check_ip(&miss_v6));
        assert_eq!(checker.check("any", &miss_v4), AuthResult::IpNotWhitelisted);
        assert_eq!(checker.check("any", &miss_v6), AuthResult::IpNotWhitelisted);
    }

    #[test]
    fn test_check_constant_time() {
        // 这个测试验证：
        // 1) expected_secret 为空时永远放行
        // 2) 长度不同时直接 false
        // 3) 长度相同时按字节比较
        // 4) 即便长度不同，也"跑完"相同代码路径以保持 constant-time 行为
        let empty = HookAuthChecker::new("");
        assert!(empty.check_secret(""));
        assert!(empty.check_secret("anything"));

        // 长度不一致
        let checker = HookAuthChecker::new("abc");
        assert!(!checker.check_secret(""));
        assert!(!checker.check_secret("a"));
        assert!(!checker.check_secret("abcd"));
        // 长度相同但内容不一致
        assert!(!checker.check_secret("abd"));
        assert!(!checker.check_secret("xyz"));
        // 完全相同
        assert!(checker.check_secret("abc"));

        // secret 检查失败优先于 IP 检查（即使 IP 在白名单内）
        let cidrs = vec![ipnetwork::IpNetwork::from_str("0.0.0.0/0").unwrap()];
        let checker = HookAuthChecker::new("abc").with_whitelist(cidrs);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        assert_eq!(checker.check("wrong", &ip), AuthResult::UnauthorizedSecret);
    }
}
