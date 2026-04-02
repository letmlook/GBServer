//! SIP 消息解析器

use super::message::{SipMessage, SipRequest, SipResponse};
use anyhow::Result;

pub struct Parser;

impl Parser {
    pub fn parse(raw: &[u8]) -> Result<SipMessage> {
        let text = String::from_utf8_lossy(raw);
        let text = text.trim();

        if text.starts_with("SIP/") {
            Ok(SipMessage::Response(Self::parse_response(text)?))
        } else {
            Ok(SipMessage::Request(Self::parse_request(text)?))
        }
    }

    fn parse_request(text: &str) -> Result<SipRequest> {
        let mut lines = text.split("\r\n");
        let request_line = lines.next().unwrap_or("");
        let parts: Vec<&str> = request_line.split_whitespace().collect();

        if parts.len() < 3 {
            anyhow::bail!("Invalid request line: {}", request_line);
        }

        let method = parts[0].to_string();
        let uri = parts[1].to_string();
        let version = parts[2].to_string();

        let mut headers = std::collections::HashMap::new();
        let mut body = None;
        let mut in_body = false;
        let mut body_content = String::new();

        for line in lines {
            if line.is_empty() {
                in_body = true;
                continue;
            }
            if in_body {
                body_content.push_str(line);
                continue;
            }
            if let Some(pos) = line.find(':') {
                let name = line[..pos].trim().to_lowercase();
                let value = line[pos + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }

        if !body_content.is_empty() {
            body = Some(body_content);
        }

        Ok(SipRequest {
            method,
            uri,
            version,
            headers,
            body,
        })
    }

    fn parse_response(text: &str) -> Result<SipResponse> {
        let mut lines = text.split("\r\n");
        let status_line = lines.next().unwrap_or("");
        let parts: Vec<&str> = status_line.split_whitespace().collect();

        if parts.len() < 3 {
            anyhow::bail!("Invalid status line: {}", status_line);
        }

        let version = parts[0].to_string();
        let status_code = parts[1].parse()?;
        let reason = parts[2..].join(" ");

        let mut headers = std::collections::HashMap::new();
        let mut body = None;
        let mut in_body = false;
        let mut body_content = String::new();

        for line in lines {
            if line.is_empty() {
                in_body = true;
                continue;
            }
            if in_body {
                body_content.push_str(line);
                continue;
            }
            if let Some(pos) = line.find(':') {
                let name = line[..pos].trim().to_lowercase();
                let value = line[pos + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }

        if !body_content.is_empty() {
            body = Some(body_content);
        }

        Ok(SipResponse {
            version,
            status_code,
            reason,
            headers,
            body,
        })
    }

    pub fn generate_response(
        status_code: u16,
        reason: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> String {
        let mut response = format!("SIP/2.0 {} {}\r\n", status_code, reason);
        for (name, value) in headers {
            response.push_str(&format!("{}: {}\r\n", name, value));
        }
        if let Some(b) = body {
            response.push_str(&format!("Content-Length: {}\r\n\r\n{}", b.len(), b));
        } else {
            response.push_str("Content-Length: 0\r\n\r\n");
        }
        response
    }

    pub fn generate_request(
        method: &str,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> String {
        let mut request = format!("{} {} SIP/2.0\r\n", method, uri);
        for (name, value) in headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }
        if let Some(b) = body {
            request.push_str(&format!("Content-Length: {}\r\n\r\n{}", b.len(), b));
        } else {
            request.push_str("Content-Length: 0\r\n\r\n");
        }
        request
    }
}
