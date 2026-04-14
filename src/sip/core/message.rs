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

impl fmt::Display for SipRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {} {}", self.method.as_str(), self.uri, self.version)?;
        for (name, value) in &self.headers {
            writeln!(f, "{}: {}", name, value)?;
        }
        if let Some(body) = &self.body {
            writeln!(f, "Content-Length: {}", body.len())?;
            writeln!(f)?;
            write!(f, "{}", body)?;
        } else {
            writeln!(f)?;
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

impl fmt::Display for SipResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {} {}", self.version, self.status_code.code(), self.reason)?;
        for (name, value) in &self.headers {
            writeln!(f, "{}: {}", name, value)?;
        }
        if let Some(body) = &self.body {
            writeln!(f, "Content-Length: {}", body.len())?;
            writeln!(f)?;
            write!(f, "{}", body)?;
        } else {
            writeln!(f)?;
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
