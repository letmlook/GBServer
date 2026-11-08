//! Conservative subset of SIP routing that can be stored without URI passwords.
//! Unsupported or ambiguous headers are rejected rather than rewritten.
use super::invite_session::SipRoutingHeaders;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeRouting {
    pub contact_uri: String,
    /// As received; the UAC route set requires reversing this order later.
    pub record_route_uris: Vec<String>,
}

pub fn safe_routing(headers: &SipRoutingHeaders) -> Option<SafeRouting> {
    if headers.contacts.len() != 1 || headers.record_routes.len() > 32 {
        return None;
    }
    let contact_uri = safe_uri(&headers.contacts[0])?.to_owned();
    let record_route_uris = headers
        .record_routes
        .iter()
        .map(|v| safe_uri(v).map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some(SafeRouting {
        contact_uri,
        record_route_uris,
    })
}

/// Conservative validation before storing original dialog addresses.
pub fn safe_dialog_address(request_uri: &str, from_header: &str, to_header: &str) -> bool {
    let wrapped = format!("<{}>", request_uri);
    safe_uri(&wrapped).is_some() && safe_tagged_header(from_header) && safe_tagged_header(to_header)
}

fn safe_tagged_header(header: &str) -> bool {
    let Some((address, suffix)) = header.split_once('>') else {
        return false;
    };
    let wrapped = format!("{}>", address);
    if safe_uri(&wrapped).is_none() {
        return false;
    }
    let Some((key, value)) = suffix.strip_prefix(';').and_then(|s| s.split_once('=')) else {
        return false;
    };
    key.eq_ignore_ascii_case("tag")
        && !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
}

fn safe_uri(header: &str) -> Option<&str> {
    let h = header.trim();
    if h.len() > 2048 || !h.starts_with('<') || !h.ends_with('>') {
        return None;
    }
    let uri = &h[1..h.len() - 1];
    if uri.is_empty()
        || !uri.is_ascii()
        || uri.bytes().any(|b| {
            b.is_ascii_whitespace()
                || b.is_ascii_control()
                || matches!(b, b'%' | b'?' | b'<' | b'>' | b'"' | b'\\' | b',' | b'#')
        })
    {
        return None;
    }
    let lower = uri.to_ascii_lowercase();
    let rest = if lower.starts_with("sips:") {
        &uri[5..]
    } else if lower.starts_with("sip:") {
        &uri[4..]
    } else {
        return None;
    };
    let mut parts = rest.split(';');
    let authority = parts.next()?;
    for param in parts {
        if !param.eq_ignore_ascii_case("lr")
            && !param.eq_ignore_ascii_case("transport=udp")
            && !param.eq_ignore_ascii_case("transport=tcp")
        {
            return None;
        }
    }
    let host_port = match authority.split_once('@') {
        Some((user, host)) => {
            if user.is_empty()
                || !user
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'~'))
                || host.contains('@')
            {
                return None;
            }
            host
        }
        None => authority,
    };
    let (host, port) = if let Some(tail) = host_port.strip_prefix('[') {
        let (host, after) = tail.split_once(']')?;
        host.parse::<std::net::Ipv6Addr>().ok()?;
        (
            host,
            if after.is_empty() {
                None
            } else {
                Some(after.strip_prefix(':')?)
            },
        )
    } else {
        let (host, port) = match host_port.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (host_port, None),
        };
        if host.is_empty()
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
        {
            return None;
        }
        (host, port)
    };
    if host.is_empty() || port.is_some_and(|p| p.parse::<u16>().ok().filter(|n| *n != 0).is_none())
    {
        return None;
    }
    Some(uri)
}
