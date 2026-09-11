use super::header::{CSeq, Contact, NameAddr, ViaHeader};
use super::method::SipMethod;
use super::status::StatusCode;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone)]
pub enum SipMessage {
    Request(SipRequest),
    Response(SipResponse),
}

impl fmt::Display for SipMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipMessage::Request(req) => write!(f, "{}", req),
            SipMessage::Response(resp) => write!(f, "{}", resp),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SipRequest {
    pub method: SipMethod,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl SipRequest {
    pub fn new(method: SipMethod, uri: String) -> Self {
        Self {
            method,
            uri,
            version: "SIP/2.0".to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn method(&self) -> SipMethod {
        self.method
    }

    pub fn method_str(&self) -> &str {
        self.method.as_str()
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }

    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_lowercase(), value.to_string());
    }
}

/// 序列化为**线上格式**的 SIP 消息。
///
/// # 必须是 CRLF
///
/// RFC 3261 §7 规定 SIP 起始行与头字段之间用 `CRLF` 分隔。此前这里用
/// `writeln!`（只写 `\n`），而 `Parser::parse_request` 是按 `"\r\n"` 切分的：
/// 于是 `format!("{}", msg)` 出来的文本**一行都切不开** ——
/// 起始行被当成整条消息，`From`/`To`/`CSeq` 等头字段**全部丢失**。
///
/// TCP 路径正是靠这条往返（读到消息 → `format!` → 重新解析）在跑，
/// 所以 TCP 设备发出的每一个请求在平台侧都表现为"没有任何头字段"，
/// 连设备 ID 都取不到（`REGISTER: Cannot extract device ID - from="" to=""`）。
impl fmt::Display for SipRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}\r\n", self.method.as_str(), self.uri, self.version)?;
        for (name, value) in &self.headers {
            // Content-Length 由下面统一写：解析出来的头里本来就有它，
            // 再来一次就是**重复的 Content-Length**，接收方对以哪个为准
            // 可以有不同解释（分帧歧义）。
            if name.eq_ignore_ascii_case("content-length") {
                continue;
            }
            write!(f, "{}: {}\r\n", name, value)?;
        }
        if let Some(body) = &self.body {
            write!(f, "Content-Length: {}\r\n\r\n", body.len())?;
            write!(f, "{}", body)?;
        } else {
            write!(f, "\r\n")?;
        }
        Ok(())
    }
}

impl SipRequest {
    pub fn remove_header(&mut self, name: &str) -> Option<String> {
        self.headers.remove(&name.to_lowercase())
    }

    pub fn from(&self) -> Option<NameAddr> {
        self.header("from").and_then(|s| NameAddr::parse(s))
    }

    pub fn to(&self) -> Option<NameAddr> {
        self.header("to").and_then(|s| NameAddr::parse(s))
    }

    pub fn call_id(&self) -> Option<&String> {
        self.header("call-id")
    }

    pub fn cseq(&self) -> Option<CSeq> {
        self.header("cseq").and_then(|s| CSeq::parse(s))
    }

    pub fn via(&self) -> Option<ViaHeader> {
        self.header("via").and_then(|s| ViaHeader::parse(s))
    }

    pub fn contact(&self) -> Option<Contact> {
        self.header("contact").and_then(|s| Contact::parse(s))
    }

    pub fn content_type(&self) -> Option<&String> {
        self.header("content-type")
    }

    pub fn content_length(&self) -> Option<usize> {
        self.header("content-length").and_then(|s| s.parse().ok())
    }

    pub fn expires(&self) -> Option<u32> {
        self.header("expires").and_then(|s| s.parse().ok())
    }

    pub fn max_forwards(&self) -> Option<u32> {
        self.header("max-forwards").and_then(|s| s.parse().ok())
    }

    pub fn allow(&self) -> Option<&String> {
        self.header("allow")
    }

    pub fn supported(&self) -> Option<&String> {
        self.header("supported")
    }

    pub fn require(&self) -> Option<&String> {
        self.header("require")
    }

    pub fn event(&self) -> Option<&String> {
        self.header("event")
    }

    pub fn subscription_state(&self) -> Option<&String> {
        self.header("subscription-state")
    }

    pub fn rack(&self) -> Option<&String> {
        self.header("rack")
    }

    pub fn authorization(&self) -> Option<&String> {
        self.header("authorization")
    }

    pub fn proxy_authorization(&self) -> Option<&String> {
        self.header("proxy-authorization")
    }

    pub fn get_body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn set_body(&mut self, body: String) {
        let len = body.len();
        self.body = Some(body);
        self.set_header("content-length", &len.to_string());
    }
}

#[derive(Debug, Clone)]
pub struct SipResponse {
    pub version: String,
    pub status_code: StatusCode,
    pub reason: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl SipResponse {
    pub fn new(status_code: u16) -> Self {
        let status = StatusCode::from_code(status_code);
        Self {
            version: "SIP/2.0".to_string(),
            status_code: status,
            reason: status.reason().to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn status_code(&self) -> u16 {
        self.status_code.code()
    }

    pub fn status(&self) -> StatusCode {
        self.status_code
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }

    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_lowercase(), value.to_string());
    }

    pub fn is_success(&self) -> bool {
        self.status_code.is_success()
    }

    pub fn is_provisional(&self) -> bool {
        self.status_code.is_provisional()
    }

    pub fn is_error(&self) -> bool {
        self.status_code.is_error()
    }

    pub fn is_final(&self) -> bool {
        self.status_code.is_final()
    }

    pub fn requires_reliable(&self) -> bool {
        self.status_code.requires_reliable()
    }
}

/// 同 `SipRequest`：必须是 CRLF（RFC 3261 §7）。
impl fmt::Display for SipResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}\r\n",
            self.version,
            self.status_code.code(),
            self.reason
        )?;
        for (name, value) in &self.headers {
            // Content-Length 由下面统一写：解析出来的头里本来就有它，
            // 再来一次就是**重复的 Content-Length**，接收方对以哪个为准
            // 可以有不同解释（分帧歧义）。
            if name.eq_ignore_ascii_case("content-length") {
                continue;
            }
            write!(f, "{}: {}\r\n", name, value)?;
        }
        if let Some(body) = &self.body {
            write!(f, "Content-Length: {}\r\n\r\n", body.len())?;
            write!(f, "{}", body)?;
        } else {
            write!(f, "\r\n")?;
        }
        Ok(())
    }
}

impl SipResponse {
    pub fn via(&self) -> Option<&String> {
        self.header("via")
    }

    pub fn from(&self) -> Option<&String> {
        self.header("from")
    }

    pub fn to(&self) -> Option<&String> {
        self.header("to")
    }

    pub fn call_id(&self) -> Option<&String> {
        self.header("call-id")
    }

    pub fn cseq(&self) -> Option<&String> {
        self.header("cseq")
    }

    pub fn contact(&self) -> Option<Contact> {
        self.header("contact").and_then(|s| Contact::parse(s))
    }

    pub fn www_authenticate(&self) -> Option<&String> {
        self.header("www-authenticate")
    }

    pub fn proxy_authenticate(&self) -> Option<&String> {
        self.header("proxy-authenticate")
    }

    pub fn allow(&self) -> Option<&String> {
        self.header("allow")
    }

    pub fn supported(&self) -> Option<&String> {
        self.header("supported")
    }

    pub fn require(&self) -> Option<&String> {
        self.header("require")
    }

    pub fn get_body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn set_body(&mut self, body: String) {
        let len = body.len();
        self.body = Some(body);
        self.set_header("content-length", &len.to_string());
    }
}

#[derive(Debug, Clone)]
pub struct SipHeader {
    pub name: String,
    pub value: String,
}

impl SipHeader {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Trying,
    Proceeding,
    Completed,
    Confirmed,
    Terminated,
    Accepted,
}

impl Status {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Status::Pending,
            1 => Status::Trying,
            2 => Status::Proceeding,
            3 => Status::Completed,
            4 => Status::Confirmed,
            5 => Status::Terminated,
            6 => Status::Accepted,
            _ => Status::Pending,
        }
    }
}

#[cfg(test)]
mod wire_format_tests {
    use super::*;
    use crate::sip::core::parser::Parser;

    /// SIP 起始行/头字段之间必须是 **CRLF**（RFC 3261 §7）。
    ///
    /// 回归：`Display` 曾用 `writeln!`（只写 `\n`），而 `Parser` 按 `"\r\n"`
    /// 切分 —— `format!("{}", msg)` 出来的文本一行都切不开，头字段全部丢失。
    /// TCP 路径正是靠这条往返在跑，于是 TCP 设备的每个请求在平台侧都
    /// "没有任何头字段"（`REGISTER: Cannot extract device ID - from="" to=""`），
    /// TCP 注册根本不可能成功。
    #[test]
    fn display_uses_crlf_so_roundtrip_keeps_headers() {
        let raw = "REGISTER sip:3402000000@10.0.0.9:5060 SIP/2.0\r\n\
                   Via: SIP/2.0/TCP 10.0.0.5:52020;branch=z9hG4bK1;rport\r\n\
                   From: <sip:34020000001320000001@3402000000>;tag=abc\r\n\
                   To: <sip:34020000001320000001@3402000000>\r\n\
                   Call-ID: 34020000001320000001@10.0.0.5\r\n\
                   CSeq: 1 REGISTER\r\n\
                   Content-Length: 0\r\n\r\n";
        let parsed = Parser::parse(raw.as_bytes()).expect("原始消息应能解析");
        let SipMessage::Request(req) = &parsed else {
            panic!("应为请求");
        };
        assert_eq!(req.header("from").map(String::as_str), Some("<sip:34020000001320000001@3402000000>;tag=abc"));

        let reserialized = format!("{}", parsed);
        assert!(
            reserialized.contains("\r\n"),
            "序列化必须用 CRLF，实际: {:?}",
            reserialized
        );
        assert!(
            !reserialized.replace("\r\n", "").contains('\n'),
            "不应出现裸 \\n：{:?}",
            reserialized
        );

        // 往返后头字段必须仍在
        let again = Parser::parse(reserialized.as_bytes()).expect("重新解析应成功");
        let SipMessage::Request(req2) = &again else {
            panic!("应为请求");
        };
        assert_eq!(req2.header("from"), req.header("from"));
        assert_eq!(req2.header("to"), req.header("to"));
        assert_eq!(req2.header("call-id"), req.header("call-id"));
        assert_eq!(req2.header("cseq"), req.header("cseq"));
        assert_eq!(req2.header("via"), req.header("via"));
    }

    /// 带 body 的往返：`Content-Length` 必须与实际 body 长度一致，
    /// 否则 TCP 分帧会切错下一条消息。
    #[test]
    fn display_roundtrip_preserves_body_and_content_length() {
        let body = "<?xml version=\"1.0\"?>\r\n<Notify><CmdType>Keepalive</CmdType></Notify>\r\n";
        let raw = format!(
            "MESSAGE sip:3402000000@10.0.0.9:5060 SIP/2.0\r\n\
             Via: SIP/2.0/TCP 10.0.0.5:52020;branch=z9hG4bK2\r\n\
             From: <sip:34020000001320000001@3402000000>;tag=t1\r\n\
             To: <sip:3402000000@3402000000>\r\n\
             Call-ID: ka-1\r\n\
             CSeq: 2 MESSAGE\r\n\
             Content-Type: Application/MANSCDP+xml\r\n\
             Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let parsed = Parser::parse(raw.as_bytes()).unwrap();
        let text = format!("{}", parsed);
        // 分帧正确性的关键是「声明的 Content-Length == 实际写出的 body 长度」
        // （解析会 trim 掉 body 尾部的换行，所以不能拿原始 body 长度比）。
        let (head, written_body) = text.split_once("\r\n\r\n").expect("应有头体分隔");
        let declared: usize = head
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.trim().eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse().ok())?
            })
            .expect("应有 Content-Length");
        assert_eq!(declared, written_body.len(), "Content-Length 与实际 body 不符");
        let again = Parser::parse(text.as_bytes()).unwrap();
        let SipMessage::Request(req) = &again else {
            panic!("应为请求");
        };
        assert_eq!(req.body.as_deref(), Some(body.trim_end_matches("\r\n").trim_end()));
    }
}
