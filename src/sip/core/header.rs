//! SIP Header 字段定义和解析
//!
//! 参考 RFC 3261 §20

use std::collections::HashMap;
use std::str::FromStr;

/// SIP 通用 Header 名称
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderName {
    Via,
    From,
    To,
    CallId,
    CSeq,
    MaxForwards,
    Contact,
    ContentType,
    ContentLength,
    UserAgent,
    Server,
    Allow,
    Supported,
    Require,
    Unsupported,
    ProxyAuthenticate,
    ProxyAuthorization,
    WWWAuthenticate,
    Authorization,
    Expires,
    Date,
    RecordRoute,
    Route,
    ProxyRequire,
    SessionExpires,
    MinSE,
    Event,
    SubscriptionState,
    AllowEvents,
    Accept,
    AcceptEncoding,
    AcceptLanguage,
    AlertInfo,
    CallInfo,
    ContactHeader, // Contact (避免与Contact方法混淆)
    Diversion,
    PAssertedIdentity,
    PPreferredIdentity,
    Replaces,
    RemotePartyId,
    HistoryInfo,
    Rack,
    RAck,
    ContentDisposition,
    MIMEVersion,
    SecurityClient,
    SecurityServer,
    SecurityVerify,
}

impl HeaderName {
    /// 从字符串解析 Header 名称
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "via" => Some(Self::Via),
            "from" => Some(Self::From),
            "to" => Some(Self::To),
            "call-id" | "callid" => Some(Self::CallId),
            "cseq" => Some(Self::CSeq),
            "max-forwards" => Some(Self::MaxForwards),
            "contact" => Some(Self::Contact),
            "content-type" | "ctype" => Some(Self::ContentType),
            "content-length" | "clen" => Some(Self::ContentLength),
            "user-agent" => Some(Self::UserAgent),
            "server" => Some(Self::Server),
            "allow" => Some(Self::Allow),
            "supported" | "k" => Some(Self::Supported),
            "require" => Some(Self::Require),
            "unsupported" => Some(Self::Unsupported),
            "proxy-authenticate" => Some(Self::ProxyAuthenticate),
            "proxy-authorization" => Some(Self::ProxyAuthorization),
            "www-authenticate" => Some(Self::WWWAuthenticate),
            "authorization" => Some(Self::Authorization),
            "expires" => Some(Self::Expires),
            "date" => Some(Self::Date),
            "record-route" => Some(Self::RecordRoute),
            "route" => Some(Self::Route),
            "proxy-require" => Some(Self::ProxyRequire),
            "session-expires" | "x" => Some(Self::SessionExpires),
            "min-se" => Some(Self::MinSE),
            "event" | "o" => Some(Self::Event),
            "subscription-state" => Some(Self::SubscriptionState),
            "allow-events" | "u" => Some(Self::AllowEvents),
            "accept" => Some(Self::Accept),
            "accept-encoding" => Some(Self::AcceptEncoding),
            "accept-language" => Some(Self::AcceptLanguage),
            "alert-info" => Some(Self::AlertInfo),
            "call-info" => Some(Self::CallInfo),
            "diversion" => Some(Self::Diversion),
            "p-asserted-identity" => Some(Self::PAssertedIdentity),
            "p-preferred-identity" => Some(Self::PPreferredIdentity),
            "replaces" => Some(Self::Replaces),
            "remote-party-id" => Some(Self::RemotePartyId),
            "history-info" => Some(Self::HistoryInfo),
            "rack" => Some(Self::Rack),
            "rack" => Some(Self::RAck),
            "content-disposition" => Some(Self::ContentDisposition),
            "mime-version" => Some(Self::MIMEVersion),
            "security-client" => Some(Self::SecurityClient),
            "security-server" => Some(Self::SecurityServer),
            "security-verify" => Some(Self::SecurityVerify),
            _ => None,
        }
    }

    /// 获取 Header 名称字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Via => "Via",
            Self::From => "From",
            Self::To => "To",
            Self::CallId => "Call-ID",
            Self::CSeq => "CSeq",
            Self::MaxForwards => "Max-Forwards",
            Self::Contact => "Contact",
            Self::ContentType => "Content-Type",
            Self::ContentLength => "Content-Length",
            Self::UserAgent => "User-Agent",
            Self::Server => "Server",
            Self::Allow => "Allow",
            Self::Supported => "Supported",
            Self::Require => "Require",
            Self::Unsupported => "Unsupported",
            Self::ProxyAuthenticate => "Proxy-Authenticate",
            Self::ProxyAuthorization => "Proxy-Authorization",
            Self::WWWAuthenticate => "WWW-Authenticate",
            Self::Authorization => "Authorization",
            Self::Expires => "Expires",
            Self::Date => "Date",
            Self::RecordRoute => "Record-Route",
            Self::Route => "Route",
            Self::ProxyRequire => "Proxy-Require",
            Self::SessionExpires => "Session-Expires",
            Self::MinSE => "Min-SE",
            Self::Event => "Event",
            Self::SubscriptionState => "Subscription-State",
            Self::AllowEvents => "Allow-Events",
            Self::Accept => "Accept",
            Self::AcceptEncoding => "Accept-Encoding",
            Self::AcceptLanguage => "Accept-Language",
            Self::AlertInfo => "Alert-Info",
            Self::CallInfo => "Call-Info",
            Self::Diversion => "Diversion",
            Self::PAssertedIdentity => "P-Asserted-Identity",
            Self::PPreferredIdentity => "P-Preferred-Identity",
            Self::Replaces => "Replaces",
            Self::RemotePartyId => "Remote-Party-ID",
            Self::HistoryInfo => "History-Info",
            Self::Rack => "RAck",
            Self::RAck => "RAck",
            Self::ContentDisposition => "Content-Disposition",
            Self::MIMEVersion => "MIME-Version",
            Self::SecurityClient => "Security-Client",
            Self::SecurityServer => "Security-Server",
            Self::SecurityVerify => "Security-Verify",
            Self::ContactHeader => "Contact",
        }
    }

    /// 紧凑形式 (如果有)
    pub fn compact_form(&self) -> Option<&'static str> {
        match self {
            Self::CallId => Some("i"),
            Self::Contact => Some("m"),
            Self::ContentType => Some("c"),
            Self::ContentLength => Some("l"),
            Self::Supported => Some("k"),
            Self::SessionExpires => Some("x"),
            Self::Event => Some("o"),
            Self::AllowEvents => Some("u"),
            _ => None,
        }
    }
}

/// Via 头域解析结果
#[derive(Debug, Clone)]
pub struct ViaHeader {
    pub protocol: String,  // SIP/2.0
    pub transport: String, // UDP, TCP, TLS
    pub host: String,
    pub port: u16,
    pub params: HashMap<String, String>,
}

impl ViaHeader {
    /// 解析 Via 头域值
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(' ').collect();
        if parts.len() < 2 {
            return None;
        }

        let protocol = parts[0].to_string();
        let mut rest = parts[1].split(';');
        let host_port: Vec<&str> = rest.next()?.split(':').collect();

        // 从protocol中提取transport (最后一个部分)
        let transport = protocol.split('/').last()?.to_string();
        let (host, port) = if host_port.len() > 1 {
            (
                host_port[0].to_string(),
                host_port[1].parse().unwrap_or(5060),
            )
        } else {
            (host_port[0].to_string(), 5060)
        };

        let mut params = HashMap::new();
        for param in rest {
            if let Some((key, value)) = param.split_once('=') {
                params.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        Some(Self {
            protocol,
            transport,
            host,
            port,
            params,
        })
    }

    /// 生成 Via 头域字符串
    pub fn to_string(&self) -> String {
        let mut s = format!("{} {}:{}", self.protocol, self.transport, self.host);
        if self.port != 5060 {
            s.push_str(&format!(":{}", self.port));
        }
        for (key, value) in &self.params {
            s.push_str(&format!(";{}={}", key, value));
        }
        s
    }

    /// 获取 branch 参数
    pub fn branch(&self) -> Option<&str> {
        self.params.get("branch").map(|s| s.as_str())
    }

    /// 获取 rport 参数
    pub fn rport(&self) -> Option<u16> {
        self.params.get("rport").and_then(|s| s.parse().ok())
    }

    /// 获取 received 参数
    pub fn received(&self) -> Option<&str> {
        self.params.get("received").map(|s| s.as_str())
    }
}

/// From/To 头域解析结果
#[derive(Debug, Clone)]
pub struct NameAddr {
    pub display_name: Option<String>,
    pub uri: String,
    pub tag: Option<String>,
}

impl NameAddr {
    /// 解析 Name-Addr 格式
    /// 格式: ["Display Name"] <sip:user@host:port>;tag=xxx
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        let mut display_name = None;
        let mut uri = String::new();
        let mut tag = None;

        if s.contains('<') {
            let parts: Vec<&str> = s.split('<').collect();
            if !parts.is_empty() {
                let name_part = parts[0].trim().trim_matches('"');
                if !name_part.is_empty() {
                    display_name = Some(name_part.to_string());
                }
            }
            if let Some(uri_and_params) = parts.get(1) {
                // 分离URI和外部参数
                let after_bracket: Vec<&str> = uri_and_params.split('>').collect();
                let uri_part = after_bracket.get(0).unwrap_or(&"");
                uri = uri_part.to_string();

                // 处理尖括号外的参数
                if let Some(params_str) = after_bracket.get(1) {
                    for param in params_str.split(';') {
                        if let Some((key, value)) = param.trim().split_once('=') {
                            if key.trim() == "tag" {
                                tag = Some(value.trim().to_string());
                            }
                        }
                    }
                }

                // 处理URI内部的参数
                let (_, internal_params) = uri_part.split_once(';').unwrap_or((uri_part, ""));
                for param in internal_params.split(';') {
                    if let Some((key, value)) = param.split_once('=') {
                        if key.trim() == "tag" {
                            tag = Some(value.trim().to_string());
                        }
                    }
                }
            }
        } else {
            let (u, params) = s.split_once(';').unwrap_or((s, ""));
            uri = u.to_string();

            for param in params.split(';') {
                if let Some((key, value)) = param.split_once('=') {
                    if key.trim() == "tag" {
                        tag = Some(value.trim().to_string());
                    }
                }
            }
        }

        Some(Self {
            display_name,
            uri: uri.trim_matches('<').trim_matches('>').to_string(),
            tag,
        })
    }

    /// 生成 Name-Addr 字符串
    pub fn to_string(&self) -> String {
        let mut s = String::new();
        if let Some(ref name) = self.display_name {
            s.push_str(&format!("\"{}\" ", name));
        }
        s.push_str(&format!("<{}>", self.uri));
        if let Some(ref tag) = self.tag {
            s.push_str(&format!(";tag={}", tag));
        }
        s
    }
}

/// CSeq 头域解析结果
#[derive(Debug, Clone)]
pub struct CSeq {
    pub seq: u32,
    pub method: String,
}

impl CSeq {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }
        Some(Self {
            seq: parts[0].parse().ok()?,
            method: parts[1].to_string(),
        })
    }

    pub fn to_string(&self) -> String {
        format!("{} {}", self.seq, self.method)
    }

    /// 判断是否为特定方法的 CSeq
    pub fn is_method(&self, method: &str) -> bool {
        self.method.to_uppercase() == method.to_uppercase()
    }
}

/// Contact 头域解析结果
#[derive(Debug, Clone)]
pub struct Contact {
    pub display_name: Option<String>,
    pub uri: String,
    pub q: Option<f32>,
    pub expires: Option<u32>,
    pub params: HashMap<String, String>,
}

impl Contact {
    pub fn parse(s: &str) -> Option<Self> {
        let mut params = HashMap::new();

        // 移除 < >
        let s = s.trim().trim_start_matches('<').trim_end_matches('>');

        let parts: Vec<&str> = s.split(';').collect();
        let uri = parts[0].to_string();

        for param in &parts[1..] {
            if let Some((key, value)) = param.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                params.insert(key.to_string(), value.to_string());
            }
        }

        Some(Self {
            display_name: None,
            uri,
            q: params.get("q").and_then(|s| s.parse().ok()),
            expires: params.get("expires").and_then(|s| s.parse().ok()),
            params,
        })
    }

    pub fn to_string(&self) -> String {
        let mut s = if let Some(ref name) = self.display_name {
            format!("\"{}\" <{}>", name, self.uri)
        } else {
            format!("<{}>", self.uri)
        };

        if let Some(q) = self.q {
            s.push_str(&format!(";q={}", q));
        }
        if let Some(expires) = self.expires {
            s.push_str(&format!(";expires={}", expires));
        }

        for (key, value) in &self.params {
            if key != "q" && key != "expires" {
                s.push_str(&format!(";{}={}", key, value));
            }
        }

        s
    }
}

/// Subscription-State 头域解析
#[derive(Debug, Clone)]
pub struct SubscriptionState {
    pub state: String, // active, pending, terminated
    pub expires: Option<u32>,
    pub reason: Option<String>,
}

impl SubscriptionState {
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split(';').collect();
        let state = parts[0].trim().to_string();

        let mut expires = None;
        let mut reason = None;

        for part in &parts[1..] {
            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "expires" => expires = value.trim().parse().ok(),
                    "reason" => reason = Some(value.trim().to_string()),
                    _ => {}
                }
            }
        }

        Self {
            state,
            expires,
            reason,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == "active"
    }

    pub fn is_pending(&self) -> bool {
        self.state == "pending"
    }

    pub fn is_terminated(&self) -> bool {
        self.state == "terminated"
    }
}

/// Authorization 头域解析 (Digest)
#[derive(Debug, Clone)]
pub struct Authorization {
    pub username: Option<String>,
    pub realm: Option<String>,
    pub nonce: Option<String>,
    pub uri: Option<String>,
    pub response: Option<String>,
    pub qop: Option<String>,
    pub cnonce: Option<String>,
    pub nc: Option<String>,
    pub opaque: Option<String>,
}

impl Authorization {
    pub fn parse(s: &str) -> Self {
        let mut auth = Self {
            username: None,
            realm: None,
            nonce: None,
            uri: None,
            response: None,
            qop: None,
            cnonce: None,
            nc: None,
            opaque: None,
        };

        for part in s.split(',') {
            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                match key {
                    "username" => auth.username = Some(value.to_string()),
                    "realm" => auth.realm = Some(value.to_string()),
                    "nonce" => auth.nonce = Some(value.to_string()),
                    "uri" => auth.uri = Some(value.to_string()),
                    "response" => auth.response = Some(value.to_string()),
                    "qop" => auth.qop = Some(value.to_string()),
                    "cnonce" => auth.cnonce = Some(value.to_string()),
                    "nc" => auth.nc = Some(value.to_string()),
                    "opaque" => auth.opaque = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        auth
    }

    /// 验证 Digest 认证
    pub fn validate(&self, expected_response: &str) -> bool {
        self.response
            .as_ref()
            .map(|r| r == expected_response)
            .unwrap_or(false)
    }
}

/// WWW-Authenticate / Proxy-Authenticate 头域解析
#[derive(Debug, Clone)]
pub struct Challenge {
    pub scheme: String,
    pub realm: Option<String>,
    pub domain: Option<String>,
    pub nonce: Option<String>,
    pub opaque: Option<String>,
    pub stale: Option<bool>,
    pub algorithm: Option<String>,
    pub qop: Option<String>,
}

impl Challenge {
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.splitn(2, ' ').collect();
        let scheme = parts.first().unwrap_or(&"").to_string();

        let mut challenge = Self {
            scheme,
            realm: None,
            domain: None,
            nonce: None,
            opaque: None,
            stale: None,
            algorithm: None,
            qop: None,
        };

        if parts.len() > 1 {
            for part in parts[1].split(',') {
                if let Some((key, value)) = part.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');
                    match key {
                        "realm" => challenge.realm = Some(value.to_string()),
                        "domain" => challenge.domain = Some(value.to_string()),
                        "nonce" => challenge.nonce = Some(value.to_string()),
                        "opaque" => challenge.opaque = Some(value.to_string()),
                        "stale" => challenge.stale = Some(value.to_lowercase() == "true"),
                        "algorithm" => challenge.algorithm = Some(value.to_string()),
                        "qop" => challenge.qop = Some(value.to_string()),
                        _ => {}
                    }
                }
            }
        }

        challenge
    }

    /// 生成 WWW-Authenticate 响应头
    pub fn to_www_authenticate(&self) -> String {
        let mut s = format!("{} ", self.scheme);
        if let Some(ref realm) = self.realm {
            s.push_str(&format!("realm=\"{}\"", realm));
        }
        if let Some(ref nonce) = self.nonce {
            s.push_str(&format!(", nonce=\"{}\"", nonce));
        }
        if let Some(ref opaque) = self.opaque {
            s.push_str(&format!(", opaque=\"{}\"", opaque));
        }
        if let Some(ref algo) = self.algorithm {
            s.push_str(&format!(", algorithm={}", algo));
        }
        if let Some(ref qop) = self.qop {
            s.push_str(&format!(", qop=\"{}\"", qop));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_via_parse() {
        let via =
            ViaHeader::parse("SIP/2.0/UDP 192.168.1.1:5060;branch=z9hG4bK1234;rport=5060").unwrap();
        assert_eq!(via.protocol, "SIP/2.0/UDP");
        assert_eq!(via.host, "192.168.1.1");
        assert_eq!(via.branch(), Some("z9hG4bK1234"));
    }

    #[test]
    fn test_name_addr_parse() {
        let na = NameAddr::parse("\"Test\" <sip:test@192.168.1.1>;tag=abc123").unwrap();
        assert_eq!(na.display_name, Some("Test".to_string()));
        assert_eq!(na.uri, "sip:test@192.168.1.1");
        assert_eq!(na.tag, Some("abc123".to_string()));
    }

    #[test]
    fn test_cseq_parse() {
        let cseq = CSeq::parse("1 INVITE").unwrap();
        assert_eq!(cseq.seq, 1);
        assert_eq!(cseq.method, "INVITE");
    }
}
