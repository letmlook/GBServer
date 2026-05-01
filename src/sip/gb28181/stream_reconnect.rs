use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectState {
    Idle,
    Reconnecting,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ReconnectEntry {
    pub device_id: String,
    pub channel_id: String,
    pub stream_id: String,
    pub app: String,
    pub retry_count: u32,
    pub max_retries: u32,
    pub state: ReconnectState,
    pub last_attempt: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct StreamReconnectManager {
    entries: DashMap<String, ReconnectEntry>,
    max_retries: u32,
    retry_interval_secs: u64,
    enabled: bool,
    sip_server: Option<Arc<RwLock<crate::sip::SipServer>>>,
    ws_state: Option<Arc<crate::handlers::websocket::WsState>>,
}

impl StreamReconnectManager {
    pub fn new(enabled: bool, max_retries: u32, retry_interval_secs: u64) -> Self {
        Self {
            entries: DashMap::new(),
            max_retries,
            retry_interval_secs,
            enabled,
            sip_server: None,
            ws_state: None,
        }
    }

    pub fn set_sip_server(&mut self, server: Arc<RwLock<crate::sip::SipServer>>) {
        self.sip_server = Some(server);
    }

    pub fn set_ws_state(&mut self, ws: Arc<crate::handlers::websocket::WsState>) {
        self.ws_state = Some(ws);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_gb28181_stream(stream_id: &str) -> bool {
        stream_id.contains('_') && !stream_id.starts_with("proxy_") && !stream_id.starts_with("push_")
    }

    pub fn parse_stream_id(stream_id: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = stream_id.splitn(2, '_').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    pub fn on_stream_not_found(&self, app: &str, stream_id: &str) -> Option<ReconnectEntry> {
        if !self.enabled {
            return None;
        }

        if !Self::is_gb28181_stream(stream_id) {
            return None;
        }

        if self.entries.contains_key(stream_id) {
            return None;
        }

        let (device_id, channel_id) = Self::parse_stream_id(stream_id)?;

        let entry = ReconnectEntry {
            device_id,
            channel_id,
            stream_id: stream_id.to_string(),
            app: app.to_string(),
            retry_count: 0,
            max_retries: self.max_retries,
            state: ReconnectState::Reconnecting,
            last_attempt: None,
            created_at: Utc::now(),
        };

        self.entries.insert(stream_id.to_string(), entry.clone());
        Some(entry)
    }

    pub fn increment_retry(&self, stream_id: &str) -> Option<ReconnectState> {
        let mut entry = self.entries.get_mut(stream_id)?;
        entry.retry_count += 1;
        entry.last_attempt = Some(Utc::now());

        if entry.retry_count >= entry.max_retries {
            entry.state = ReconnectState::Failed;
            if let Some(ref ws) = self.ws_state {
                let msg = serde_json::json!({
                    "type": "streamReconnect",
                    "streamId": stream_id,
                    "state": "failed",
                    "retryCount": entry.retry_count
                });
                tokio::spawn({
                    let ws = ws.clone();
                    let msg = msg.clone();
                    async move { ws.broadcast("streamReconnect", msg).await; }
                });
            }
            Some(ReconnectState::Failed)
        } else {
            Some(ReconnectState::Reconnecting)
        }
    }

    pub fn mark_success(&self, stream_id: &str) {
        if let Some((_, entry)) = self.entries.remove(stream_id) {
            tracing::info!("Stream reconnected successfully: {} (after {} retries)", 
                stream_id, entry.retry_count);
            if let Some(ref ws) = self.ws_state {
                let msg = serde_json::json!({
                    "type": "streamReconnect",
                    "streamId": stream_id,
                    "state": "success",
                    "retryCount": entry.retry_count
                });
                tokio::spawn({
                    let ws = ws.clone();
                    let msg = msg.clone();
                    async move { ws.broadcast("streamReconnect", msg).await; }
                });
            }
        }
    }

    pub fn remove(&self, stream_id: &str) {
        self.entries.remove(stream_id);
    }

    pub fn get_reconnectable(&self) -> Vec<ReconnectEntry> {
        self.entries
            .iter()
            .filter(|e| e.state == ReconnectState::Reconnecting)
            .map(|e| e.clone())
            .collect()
    }

    pub fn retry_interval_secs(&self) -> u64 {
        self.retry_interval_secs
    }

    pub async fn run_reconnect_loop(&self) {
        if !self.enabled {
            return;
        }

        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.retry_interval_secs)
        );

        loop {
            interval.tick().await;

            let entries = self.get_reconnectable();
            for entry in entries {
                if let Some(ref sip_server) = self.sip_server {
                    tracing::info!("Attempting stream reconnect: {} (retry {}/{})", 
                        entry.stream_id, entry.retry_count + 1, entry.max_retries);

                    let sip = sip_server.read().await;
                    let result = sip.send_play_invite_and_wait(
                        &entry.device_id,
                        &entry.channel_id,
                        0,
                        None,
                    ).await;

                    drop(sip);

                    match result {
                        Ok(_) => {
                            tracing::info!("Stream reconnect succeeded: {}", entry.stream_id);
                            self.mark_success(&entry.stream_id);
                        }
                        Err(e) => {
                            tracing::warn!("Stream reconnect failed: {} - {}", entry.stream_id, e);
                            if let Some(state) = self.increment_retry(&entry.stream_id) {
                                if state == ReconnectState::Failed {
                                    tracing::error!("Stream reconnect giving up: {}", entry.stream_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_gb28181_stream() {
        assert!(StreamReconnectManager::is_gb28181_stream("34020000001320000001_34020000001320000001"));
        assert!(!StreamReconnectManager::is_gb28181_stream("proxy_stream"));
        assert!(!StreamReconnectManager::is_gb28181_stream("push_stream"));
    }

    #[test]
    fn test_parse_stream_id() {
        let (device, channel) = StreamReconnectManager::parse_stream_id("dev123_ch456").unwrap();
        assert_eq!(device, "dev123");
        assert_eq!(channel, "ch456");
    }

    #[test]
    fn test_on_stream_not_found() {
        let manager = StreamReconnectManager::new(true, 3, 5);
        let entry = manager.on_stream_not_found("rtp", "device1_channel1");
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.device_id, "device1");
        assert_eq!(e.channel_id, "channel1");
        assert_eq!(e.state, ReconnectState::Reconnecting);
    }

    #[test]
    fn test_increment_retry_exceed_max() {
        let manager = StreamReconnectManager::new(true, 2, 5);
        manager.on_stream_not_found("rtp", "dev_ch");
        manager.increment_retry("dev_ch");
        let state = manager.increment_retry("dev_ch");
        assert_eq!(state, Some(ReconnectState::Failed));
    }

    #[test]
    fn test_disabled() {
        let manager = StreamReconnectManager::new(false, 3, 5);
        let entry = manager.on_stream_not_found("rtp", "dev_ch");
        assert!(entry.is_none());
    }
}
