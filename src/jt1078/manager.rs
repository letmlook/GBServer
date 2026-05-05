use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use std::time::Instant;

use crate::jt1078::session::Jt1078Session;

#[derive(Clone)]
pub struct Jt1078Manager {
    sessions: Arc<Mutex<HashMap<SocketAddr, Jt1078Session>>>,
    timeout: Duration,
    /// How long to wait before considering a missing sequence as timed-out (retransmit detection)
    retransmit_wait: Duration,
}

impl Jt1078Manager {
    /// Create a new manager with per-session timeout and retransmit wait
    pub fn new(timeout: Duration, retransmit_wait: Duration) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            timeout,
            retransmit_wait,
        }
    }

    /// Feed bytes from a peer address into its session, creating the session if needed.
    /// Returns parsed frames (Vec<Vec<u8>>), same as Jt1078Session::feed_bytes.
    pub async fn feed_bytes(&self, addr: SocketAddr, data: &[u8]) -> Vec<Vec<u8>> {
        let mut map = self.sessions.lock().await;
        let session = map.entry(addr).or_insert_with(|| Jt1078Session::new(addr));
        // update heartbeat touched time when new data arrives
        session.last_heartbeat = Instant::now();
        session.feed_bytes(data)
    }

    /// Explicitly remove a session
    pub async fn remove(&self, addr: &SocketAddr) {
        let mut map = self.sessions.lock().await;
        map.remove(addr);
    }

    /// Number of active sessions (for testing/metrics)
    pub async fn count(&self) -> usize {
        let map = self.sessions.lock().await;
        map.len()
    }

    /// Process a parsed payload for the given peer by delegating to the session's process_payload.
    pub async fn process_payload_for(&self, addr: SocketAddr, payload: &[u8]) -> crate::jt1078::session::FrameKind {
        let mut map = self.sessions.lock().await;
        let session = map.entry(addr).or_insert_with(|| Jt1078Session::new(addr));
        session.process_payload(payload)
    }

    /// Cleanup timed-out sessions once. Returns removed count.
    pub async fn cleanup_once(&self) -> usize {
        let mut map = self.sessions.lock().await;
        let now = Instant::now();
        let timeout = self.timeout;
        let mut removed = Vec::new();
        for (k, v) in map.iter() {
            if now.duration_since(v.last_heartbeat) > timeout {
                removed.push(*k);
            }
        }
        for k in removed.iter() {
            map.remove(k);
        }
        removed.len()
    }

    /// Spawn a background cleanup loop with given tick interval (returns when cancelled)
    pub async fn cleanup_loop(self, tick: Duration) {
        loop {
            tokio::time::sleep(tick).await;
            let removed = self.cleanup_once().await;
            if removed > 0 {
                tracing::info!("JT1078 manager cleanup removed {} timed-out sessions", removed);
            }

            // scan sessions for retransmit-timeout missing sequences
            let mut map = self.sessions.lock().await;
            for (addr, sess) in map.iter_mut() {
                let timed_out = sess.collect_timed_out_missing(self.retransmit_wait);
                if !timed_out.is_empty() {
                    tracing::warn!("JT1078 missing sequences timed out for {}: {:?}", addr, timed_out);
                    // TODO: emit metric or trigger retransmit hook
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    #[tokio::test]
    async fn test_feed_and_count_and_cleanup() {
        let manager = Jt1078Manager::new(Duration::from_millis(100), Duration::from_millis(200));
        let addr1 = make_addr(60001);
        let payload = b"abc";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let frames = manager.feed_bytes(addr1, &buf).await;
        assert_eq!(frames.len(), 1);
        assert_eq!(manager.count().await, 1);

        // wait for timeout and cleanup
        tokio::time::sleep(Duration::from_millis(200)).await;
        let removed = manager.cleanup_once().await;
        assert_eq!(removed, 1);
        assert_eq!(manager.count().await, 0);
    }
}
