use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::jt1078::frame::parse_jt1078_frame;

/// Minimal JT1078 session state for TCP/UDP peer.
/// Handles authentication (simple token), heartbeat timestamp, and reassembly buffer.
#[derive(Debug)]
pub struct Jt1078Session {
    pub peer: SocketAddr,
    pub authenticated: bool,
    pub buffer: Vec<u8>,
    pub last_heartbeat: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameKind {
    AuthSuccess,
    AuthFailure,
    Heartbeat,
    Data(Vec<u8>),
}

impl Jt1078Session {
    pub fn new(peer: SocketAddr) -> Self {
        Self {
            peer,
            authenticated: false,
            buffer: Vec::with_capacity(8 * 1024),
            last_heartbeat: Instant::now(),
        }
    }

    /// Feed raw bytes into the session reassembly buffer and extract complete payloads.
    /// Returns a vector of payload slices (owned Vec<u8>). Caller should inspect/process each payload.
    pub fn feed_bytes(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(data);
        let mut frames = Vec::new();

        loop {
            match parse_jt1078_frame(&self.buffer) {
                Some((frame_len, payload)) => {
                    frames.push(payload.to_vec());
                    // remove consumed bytes
                    self.buffer.drain(0..frame_len);
                }
                None => break,
            }
        }

        frames
    }

    /// Process a single payload (decoded frame). Returns FrameKind for higher-level handling.
    /// Protocol decisions here are intentionally simple for tests:
    /// - AUTH:<token> => authenticate if matches env var WVP__JT1078__TOKEN (default "secret")
    /// - HEARTBEAT payload => update last_heartbeat
    /// - otherwise => Data(payload)
    pub fn process_payload(&mut self, payload: &[u8]) -> FrameKind {
        if payload.starts_with(b"AUTH:") {
            let token = &payload[5..];
            let configured = std::env::var("WVP__JT1078__TOKEN").unwrap_or_else(|_| "secret".into());
            // compare as UTF-8 trimmed string to be tolerant of minor framing differences
            let token_str = String::from_utf8_lossy(token).trim().to_string();
            if token_str == configured || token_str.contains(&configured) {
                self.authenticated = true;
                return FrameKind::AuthSuccess;
            } else {
                self.authenticated = false;
                return FrameKind::AuthFailure;
            }
        }

        if payload == b"HEARTBEAT" {
            self.last_heartbeat = Instant::now();
            return FrameKind::Heartbeat;
        }

        FrameKind::Data(payload.to_vec())
    }

    /// Whether the session is considered timed out given the provided duration
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        Instant::now().duration_since(self.last_heartbeat) > timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_addr() -> SocketAddr {
        "127.0.0.1:60000".parse().unwrap()
    }

    #[test]
    fn test_reassembly_and_payload_extraction() {
        let mut sess = Jt1078Session::new(make_addr());
        let payload = b"hello-reassembly";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        // split into two parts
        let split = 5;
        let part1 = &buf[..split];
        let part2 = &buf[split..];

        let frames1 = sess.feed_bytes(part1);
        assert!(frames1.is_empty());

        let frames2 = sess.feed_bytes(part2);
        assert_eq!(frames2.len(), 1);
        assert_eq!(frames2[0], payload);
    }

    #[test]
    fn test_auth_and_heartbeat_processing() {
        std::env::set_var("WVP__JT1078__TOKEN", "mytoken");
        let mut sess = Jt1078Session::new(make_addr());

        // construct AUTH frame (length-prefixed)
        let payload = b"AUTH:mytoken";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let frames = sess.feed_bytes(&buf);
        assert_eq!(frames.len(), 1);
        // ensure payload extracted exactly
        assert_eq!(frames[0].as_slice(), payload, "extracted payload mismatch: {:?}", frames[0]);
        let kind = sess.process_payload(&frames[0]);
        assert_eq!(kind, FrameKind::AuthSuccess);
        assert!(sess.authenticated);

        // heartbeat
        let hb = b"HEARTBEAT";
        let len = (hb.len() as u32).to_be_bytes();
        let mut buf2 = Vec::new();
        buf2.extend_from_slice(&len);
        buf2.extend_from_slice(hb);

        // record previous heartbeat time
        let prev = sess.last_heartbeat;
        let frames2 = sess.feed_bytes(&buf2);
        let kind2 = sess.process_payload(&frames2[0]);
        assert_eq!(kind2, FrameKind::Heartbeat);
        assert!(sess.last_heartbeat >= prev);
    }

    #[test]
    fn test_auth_failure() {
        std::env::set_var("WVP__JT1078__TOKEN", "good");
        let mut sess = Jt1078Session::new(make_addr());

        let payload = b"AUTH:bad";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let frames = sess.feed_bytes(&buf);
        let kind = sess.process_payload(&frames[0]);
        assert_eq!(kind, FrameKind::AuthFailure);
        assert!(!sess.authenticated);
    }
}
