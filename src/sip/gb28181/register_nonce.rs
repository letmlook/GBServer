//! Short-lived, source-bound challenges for inbound device REGISTER requests.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(120);
const MAX_CHALLENGES: usize = 4096;
const MAX_FINGERPRINT_BYTES: usize = 8192;

struct Challenge {
    device_id: String,
    source: SocketAddr,
    issued: Instant,
    accepted_request: Option<String>,
}

#[derive(Default)]
pub struct RegisterNonces {
    entries: Mutex<HashMap<String, Challenge>>,
}

impl RegisterNonces {
    pub fn issue(&self, nonce: &str, device_id: &str, source: SocketAddr) {
        if nonce.len() > 128 || device_id.len() > 128 {
            return;
        }
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| now.duration_since(entry.issued) < TTL);
        if entries.len() >= MAX_CHALLENGES {
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.issued)
                .map(|(key, _)| key.clone())
            {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            nonce.to_owned(),
            Challenge {
                device_id: device_id.to_owned(),
                source,
                issued: now,
                accepted_request: None,
            },
        );
    }

    pub fn matches(&self, nonce: &str, device_id: &str, source: SocketAddr) -> bool {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.get(nonce).is_some_and(|entry| {
            entry.device_id == device_id && entry.source == source && entry.issued.elapsed() < TTL
        })
    }

    /// Permit one authenticated request and exact retransmissions from the same socket.
    pub fn accept(
        &self,
        nonce: &str,
        device_id: &str,
        source: SocketAddr,
        fingerprint: &str,
    ) -> bool {
        if fingerprint.len() > MAX_FINGERPRINT_BYTES {
            return false;
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = entries.get_mut(nonce) else {
            return false;
        };
        if entry.device_id != device_id || entry.source != source || entry.issued.elapsed() >= TTL {
            return false;
        }
        match &entry.accepted_request {
            Some(previous) => previous == fingerprint,
            None => {
                entry.accepted_request = Some(fingerprint.to_owned());
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_binding_expiration_and_retransmission_are_enforced() {
        let nonces = RegisterNonces::default();
        let source = "127.0.0.1:10000".parse().unwrap();
        let other = "127.0.0.1:10001".parse().unwrap();
        nonces.issue("n", "device", source);
        assert!(!nonces.matches("n", "device", other));
        assert!(!nonces.accept("n", "device", other, "one"));
        assert!(nonces.accept("n", "device", source, "one"));
        assert!(nonces.accept("n", "device", source, "one"));
        assert!(!nonces.accept("n", "device", source, "different"));
        nonces.entries.lock().unwrap().get_mut("n").unwrap().issued = Instant::now() - TTL;
        assert!(!nonces.matches("n", "device", source));
    }
}
