//! SIP 消息结构定义

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum SipMessage {
    Request(SipRequest),
    Response(SipResponse),
}

#[derive(Debug, Clone)]
pub struct SipRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SipResponse {
    pub version: String,
    pub status_code: u16,
    pub reason: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SipHeader {
    pub name: String,
    pub value: String,
}

impl SipRequest {
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
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

    pub fn via(&self) -> Option<&String> {
        self.header("via")
    }

    pub fn content_type(&self) -> Option<&String> {
        self.header("content-type")
    }
}

impl SipResponse {
    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
}
