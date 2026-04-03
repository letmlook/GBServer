//! SIP 方法定义
//!
//! 参考 RFC 3261 §7 和各扩展RFC

use std::fmt;

/// SIP 方法枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SipMethod {
    // RFC 3261 基础方法
    Invite = 1,
    Ack,
    Bye,
    Cancel,
    Register,
    Options,

    // RFC 3265 订阅相关
    Subscribe,
    Notify,

    // RFC 3515 转移呼叫
    Refer,

    // RFC 3311 更新会话
    Update,

    // RFC 3262 可靠临时响应确认
    Prack,

    // RFC 3428 即时消息
    Message,

    // RFC 6026 INFO 方法
    Info,

    // 扩展/未知方法
    Unknown,
}

impl SipMethod {
    /// 从字符串解析方法名
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "INVITE" => Self::Invite,
            "ACK" => Self::Ack,
            "BYE" => Self::Bye,
            "CANCEL" => Self::Cancel,
            "REGISTER" => Self::Register,
            "OPTIONS" => Self::Options,
            "SUBSCRIBE" => Self::Subscribe,
            "NOTIFY" => Self::Notify,
            "REFER" => Self::Refer,
            "UPDATE" => Self::Update,
            "PRACK" => Self::Prack,
            "MESSAGE" => Self::Message,
            "INFO" => Self::Info,
            _ => Self::Unknown,
        }
    }

    /// 获取方法名称
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Invite => "INVITE",
            Self::Ack => "ACK",
            Self::Bye => "BYE",
            Self::Cancel => "CANCEL",
            Self::Register => "REGISTER",
            Self::Options => "OPTIONS",
            Self::Subscribe => "SUBSCRIBE",
            Self::Notify => "NOTIFY",
            Self::Refer => "REFER",
            Self::Update => "UPDATE",
            Self::Prack => "PRACK",
            Self::Message => "MESSAGE",
            Self::Info => "INFO",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// 判断是否为稳定方法 (stable method)
    /// ACK 和 CANCEL 不是稳定方法
    pub fn is_stable(&self) -> bool {
        matches!(
            self,
            Self::Invite
                | Self::Bye
                | Self::Register
                | Self::Options
                | Self::Subscribe
                | Self::Notify
                | Self::Refer
                | Self::Update
                | Self::Prack
                | Self::Message
                | Self::Info
        )
    }

    /// 判断是否为Invite方法
    pub fn is_invite(&self) -> bool {
        matches!(self, Self::Invite)
    }

    /// 判断是否需要Ack
    pub fn needs_ack(&self) -> bool {
        matches!(self, Self::Invite)
    }

    /// 判断是否支持Compact Form
    pub fn compact_form(&self) -> Option<&'static str> {
        match self {
            Self::Invite => Some("I"),
            Self::Ack => Some("A"),
            Self::Bye => Some("B"),
            Self::Cancel => Some("C"),
            Self::Options => Some("O"),
            _ => None,
        }
    }
}

impl fmt::Display for SipMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for SipMethod {
    fn default() -> Self {
        Self::Unknown
    }
}

/// SIP 方法集合 (用于 Allow, Accept 等头域)
#[derive(Debug, Clone, Default)]
pub struct SipMethodSet(pub Vec<SipMethod>);

impl SipMethodSet {
    pub fn new() -> Self {
        Self(vec![
            SipMethod::Invite,
            SipMethod::Ack,
            SipMethod::Bye,
            SipMethod::Cancel,
            SipMethod::Register,
            SipMethod::Options,
            SipMethod::Subscribe,
            SipMethod::Notify,
            SipMethod::Refer,
            SipMethod::Prack,
            SipMethod::Update,
            SipMethod::Message,
            SipMethod::Info,
        ])
    }

    pub fn from_str(s: &str) -> Self {
        let methods: Vec<SipMethod> = s
            .split(',')
            .map(|m| SipMethod::from_str(m.trim()))
            .filter(|&m| m != SipMethod::Unknown)
            .collect();
        Self(methods)
    }

    pub fn contains(&self, method: SipMethod) -> bool {
        self.0.contains(&method)
    }

    pub fn to_string(&self) -> String {
        self.0
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// 判断方法是否为 RFC 3261 必须支持的
pub fn is_required_method(method: SipMethod) -> bool {
    matches!(
        method,
        SipMethod::Invite
            | SipMethod::Ack
            | SipMethod::Bye
            | SipMethod::Cancel
            | SipMethod::Register
            | SipMethod::Options
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_from_str() {
        assert_eq!(SipMethod::from_str("INVITE"), SipMethod::Invite);
        assert_eq!(SipMethod::from_str("invite"), SipMethod::Invite);
        assert_eq!(SipMethod::from_str("UNKNOWN"), SipMethod::Unknown);
    }

    #[test]
    fn test_method_as_str() {
        assert_eq!(SipMethod::Invite.as_str(), "INVITE");
        assert_eq!(SipMethod::Ack.as_str(), "ACK");
    }

    #[test]
    fn test_compact_form() {
        assert_eq!(SipMethod::Invite.compact_form(), Some("I"));
        assert_eq!(SipMethod::Message.compact_form(), None);
    }
}
