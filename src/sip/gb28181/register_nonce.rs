//! Short-lived, source-bound challenges for inbound device REGISTER requests.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(120);
const MAX_CHALLENGES: usize = 4096;
const MAX_FINGERPRINT_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationIngress {
    Udp,
    Tcp(uuid::Uuid),
}

struct Challenge {
    device_id: String,
    source: SocketAddr,
    ingress: RegistrationIngress,
    issued: Instant,
    accepted_request: Option<String>,
}

#[derive(Default)]
pub struct RegisterNonces {
    entries: Mutex<HashMap<String, Challenge>>,
}

impl RegisterNonces {
    pub fn issue_bound(
        &self,
        nonce: &str,
        device_id: &str,
        source: SocketAddr,
        ingress: RegistrationIngress,
    ) {
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
                ingress,
                issued: now,
                accepted_request: None,
            },
        );
    }

    pub fn matches_bound(
        &self,
        nonce: &str,
        device_id: &str,
        source: SocketAddr,
        ingress: RegistrationIngress,
    ) -> bool {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.get(nonce).is_some_and(|entry| {
            entry.device_id == device_id
                && entry.source == source
                && entry.ingress == ingress
                && entry.issued.elapsed() < TTL
        })
    }

    /// Permit one authenticated request and exact retransmissions from the same socket.
    pub fn accept_bound(
        &self,
        nonce: &str,
        device_id: &str,
        source: SocketAddr,
        ingress: RegistrationIngress,
        fingerprint: &str,
    ) -> bool {
        if fingerprint.len() > MAX_FINGERPRINT_BYTES {
            return false;
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = entries.get_mut(nonce) else {
            return false;
        };
        if entry.device_id != device_id
            || entry.source != source
            || entry.ingress != ingress
            || entry.issued.elapsed() >= TTL
        {
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

    pub fn invalidate_ingress(&self, source: SocketAddr, ingress: RegistrationIngress) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|_, entry| entry.source != source || entry.ingress != ingress);
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
        nonces.issue_bound("n", "device", source, RegistrationIngress::Udp);
        assert!(!nonces.matches_bound("n", "device", other, RegistrationIngress::Udp));
        assert!(!nonces.accept_bound("n", "device", other, RegistrationIngress::Udp, "one"));
        assert!(nonces.accept_bound("n", "device", source, RegistrationIngress::Udp, "one"));
        assert!(nonces.accept_bound("n", "device", source, RegistrationIngress::Udp, "one"));
        assert!(!nonces.accept_bound("n", "device", source, RegistrationIngress::Udp, "different"));
        nonces.entries.lock().unwrap().get_mut("n").unwrap().issued = Instant::now() - TTL;
        assert!(!nonces.matches_bound("n", "device", source, RegistrationIngress::Udp));
    }

    #[test]
    fn challenge_cannot_cross_transport_or_tcp_connection_generation() {
        let nonces = RegisterNonces::default();
        let source = "127.0.0.1:10000".parse().unwrap();
        let tcp_first = RegistrationIngress::Tcp(uuid::Uuid::new_v4());
        let tcp_reconnected = RegistrationIngress::Tcp(uuid::Uuid::new_v4());
        nonces.issue_bound("tcp-nonce", "device", source, tcp_first);
        assert!(!nonces.matches_bound("tcp-nonce", "device", source, RegistrationIngress::Udp));
        assert!(!nonces.matches_bound("tcp-nonce", "device", source, tcp_reconnected));
        assert!(nonces.matches_bound("tcp-nonce", "device", source, tcp_first));
        assert!(!nonces.accept_bound(
            "tcp-nonce",
            "device",
            source,
            RegistrationIngress::Udp,
            "auth"
        ));
        assert!(nonces.accept_bound("tcp-nonce", "device", source, tcp_first, "auth"));
    }
}
