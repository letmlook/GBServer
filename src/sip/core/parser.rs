use super::header::{Authorization, CSeq, Challenge, Contact, NameAddr, ViaHeader};
use super::message::{SipMessage, SipRequest, SipResponse};
use super::method::SipMethod;
use super::status::StatusCode;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

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

    pub fn parse_request(text: &str) -> Result<SipRequest> {
        let mut lines = text.split("\r\n");
        let request_line = lines.next().unwrap_or("");

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid request line: {}", request_line));
        }

        let method = SipMethod::from_str(parts[0]);
        let uri = parts[1].to_string();
        let version = parts[2].to_string();

        if version != "SIP/2.0" {
            return Err(anyhow!("Unsupported SIP version: {}", version));
        }

        let mut headers = HashMap::new();
        let mut body = None;
        let mut in_body = false;
        let mut body_content = String::new();

        for line in lines {
            if line.is_empty() {
                in_body = true;
                continue;
            }
            if in_body {
                if !body_content.is_empty() {
                    body_content.push('\r');
                }
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

    pub fn parse_response(text: &str) -> Result<SipResponse> {
        let mut lines = text.split("\r\n");
        let status_line = lines.next().unwrap_or("");

        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid status line: {}", status_line));
        }

        let version = parts[0].to_string();
        if version != "SIP/2.0" {
            return Err(anyhow!("Unsupported SIP version: {}", version));
        }

        let status_code: u16 = parts[1]
            .parse()
            .map_err(|_| anyhow!("Invalid status code: {}", parts[1]))?;
        let reason = parts[2..].join(" ");

        let mut headers = HashMap::new();
        let mut body = None;
        let mut in_body = false;
        let mut body_content = String::new();

        for line in lines {
            if line.is_empty() {
                in_body = true;
                continue;
            }
            if in_body {
                if !body_content.is_empty() {
                    body_content.push('\r');
                }
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

        let status = StatusCode::from_code(status_code);
        Ok(SipResponse {
            version,
            status_code: status,
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

    pub fn generate_response_from_status(
        status: StatusCode,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> String {
        Self::generate_response(status.code(), status.reason(), headers, body)
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

    pub fn generate_request_from_method(
        method: SipMethod,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> String {
        Self::generate_request(method.as_str(), uri, headers, body)
    }

    pub fn generate_trying_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            100,
            "Trying",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_ringing_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        contact: &str,
    ) -> String {
        Self::generate_response(
            180,
            "Ringing",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
                ("Contact", contact),
            ],
            None,
        )
    }

    pub fn generate_session_progress_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        contact: &str,
        body: Option<&str>,
    ) -> String {
        Self::generate_response(
            183,
            "Session Progress",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
                ("Contact", contact),
            ],
            body,
        )
    }

    pub fn generate_bad_request_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            400,
            "Bad Request",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_not_found_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            404,
            "Not Found",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_method_not_allowed_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(405, "Method Not Allowed", &[
            ("Via", via),
            ("From", from),
            ("To", to),
            ("Call-ID", call_id),
            ("CSeq", cseq),
            ("Allow", "INVITE, ACK, CANCEL, BYE, REGISTER, OPTIONS, MESSAGE, INFO, SUBSCRIBE, NOTIFY, REFER, PRACK, UPDATE"),
        ], None)
    }

    pub fn generate_request_timeout_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            408,
            "Request Timeout",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_busy_here_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            486,
            "Busy Here",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_request_terminated_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            487,
            "Request Terminated",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_not_acceptable_here_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            488,
            "Not Acceptable Here",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_server_internal_error_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            500,
            "Server Internal Error",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_not_implemented_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            501,
            "Not Implemented",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_service_unavailable_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_response(
            503,
            "Service Unavailable",
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_ok_response(
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        contact: Option<&str>,
        body: Option<&str>,
    ) -> String {
        let mut headers = vec![
            ("Via", via),
            ("From", from),
            ("To", to),
            ("Call-ID", call_id),
            ("CSeq", cseq),
        ];
        if let Some(c) = contact {
            headers.push(("Contact", c));
        }
        let h: Vec<(&str, &str)> = headers;
        Self::generate_response(200, "OK", &h, body)
    }

    pub fn generate_www_authenticate_response(
        realm: &str,
        nonce: &str,
        opaque: Option<&str>,
    ) -> String {
        let mut auth = format!("Digest realm=\"{}\", nonce=\"{}\"", realm, nonce);
        if let Some(o) = opaque {
            auth.push_str(&format!(", opaque=\"{}\"", o));
        }
        auth.push_str(", algorithm=MD5, qop=\"auth\"");
        auth
    }

    pub fn generate_proxy_authenticate_response(
        realm: &str,
        nonce: &str,
        opaque: Option<&str>,
    ) -> String {
        let mut auth = format!("Digest realm=\"{}\", nonce=\"{}\"", realm, nonce);
        if let Some(o) = opaque {
            auth.push_str(&format!(", opaque=\"{}\"", o));
        }
        auth.push_str(", algorithm=MD5, qop=\"auth\"");
        auth
    }

    pub fn generate_ack(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_request(
            "ACK",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_cancel(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_request(
            "CANCEL",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_prack(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        rack: &str,
    ) -> String {
        Self::generate_request(
            "PRACK",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
                ("RAck", rack),
            ],
            None,
        )
    }

    pub fn generate_bye(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
    ) -> String {
        Self::generate_request(
            "BYE",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            None,
        )
    }

    pub fn generate_subscribe(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: u32,
        event: &str,
        expires: u32,
    ) -> String {
        Self::generate_request(
            "SUBSCRIBE",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", &format!("{} SUBSCRIBE", cseq)),
                ("Event", event),
                ("Expires", &expires.to_string()),
            ],
            None,
        )
    }

    pub fn generate_notify(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: u32,
        event: &str,
        subscription_state: &str,
    ) -> String {
        Self::generate_request(
            "NOTIFY",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", &format!("{} NOTIFY", cseq)),
                ("Event", event),
                ("Subscription-State", subscription_state),
            ],
            None,
        )
    }

    pub fn generate_refer(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        refer_to: &str,
    ) -> String {
        Self::generate_request(
            "REFER",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
                ("Refer-To", refer_to),
            ],
            None,
        )
    }

    pub fn generate_update(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        body: Option<&str>,
    ) -> String {
        Self::generate_request(
            "UPDATE",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            body,
        )
    }

    pub fn generate_info(
        uri: &str,
        via: &str,
        from: &str,
        to: &str,
        call_id: &str,
        cseq: &str,
        body: Option<&str>,
    ) -> String {
        Self::generate_request(
            "INFO",
            uri,
            &[
                ("Via", via),
                ("From", from),
                ("To", to),
                ("Call-ID", call_id),
                ("CSeq", cseq),
            ],
            body,
        )
    }
}
