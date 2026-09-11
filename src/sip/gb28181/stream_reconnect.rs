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
    sip_server: RwLock<Option<Arc<crate::sip::SipServer>>>,
    ws_state: RwLock<Option<Arc<crate::handlers::websocket::WsState>>>,
}

impl StreamReconnectManager {
    pub fn new(enabled: bool, max_retries: u32, retry_interval_secs: u64) -> Self {
        Self {
            entries: DashMap::new(),
            max_retries,
            retry_interval_secs,
            enabled,
            sip_server: RwLock::new(None),
            ws_state: RwLock::new(None),
        }
    }

    pub async fn set_sip_server(&self, server: Arc<crate::sip::SipServer>) {
        *self.sip_server.write().await = Some(server);
    }

    pub async fn set_ws_state(&self, ws: Arc<crate::handlers::websocket::WsState>) {
        *self.ws_state.write().await = Some(ws);
    }

    pub async fn get_sip_server(&self) -> Option<Arc<crate::sip::SipServer>> {
        self.sip_server.read().await.clone()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_gb28181_stream(stream_id: &str) -> bool {
        stream_id.contains('_') && !stream_id.starts_with("proxy_") && !stream_id.starts_with("push_")
    }

    /// 从 ZLM 的 stream_id 解析出 `(device_id, channel_id)`。
    ///
    /// 这是**全局唯一**的解析实现（`zlm::hook` 里同名的私有函数只是转发到这里）。
    /// 历史上存在三种写法，都要能认：
    ///
    /// * `{device}_{channel}` —— 本平台 `openRtpServer` 使用的规范写法；
    /// * `{device}${channel}` —— 部分 WVP 版本 / 上级平台下发的写法；
    /// * `{device}/{channel}` —— 拉流代理模板里的写法。
    ///
    /// 此前这里只认 `_`，而 `zlm::hook` 里另有一份只认 `$`/`/` 的实现 ——
    /// 于是 `on_stream_not_found` 对**本平台自己**的流永远解析失败，
    /// 按需拉流从未真正触发过。
    pub fn parse_stream_id(stream_id: &str) -> Option<(String, String)> {
        // 明确的非国标流（推流/拉流代理）直接排除，避免把 `push_xxx` 拆成
        // device="push" channel="xxx" 这种假阳性。
        if stream_id.starts_with("proxy_") || stream_id.starts_with("push_") {
            return None;
        }
        for sep in ['_', '$', '/'] {
            if let Some((device, channel)) = stream_id.split_once(sep) {
                if !device.is_empty() && !channel.is_empty() {
                    return Some((device.to_string(), channel.to_string()));
                }
            }
        }
        None
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
            if let Ok(ws_guard) = self.ws_state.try_read() {
                if let Some(ref ws) = *ws_guard {
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
            if let Ok(ws_guard) = self.ws_state.try_read() {
                if let Some(ref ws) = *ws_guard {
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
            let sip_server_opt = self.get_sip_server().await;
            for entry in entries {
                if let Some(ref sip_server) = sip_server_opt {
                    tracing::info!("Attempting stream reconnect: {} (retry {}/{})", 
                        entry.stream_id, entry.retry_count + 1, entry.max_retries);

                    let sip = &*sip_server;
                    // 必须走 `start_live_stream`：它会先 `openRtpServer` 拿到
                    // 真实收流端口再发 INVITE。此前这里传 `media_port = 0`，
                    // SDP 里就是 `m=video 0`（表示禁用媒体），设备无处可推，
                    // "自动重连"实际上永远不会成功。
                    let result = sip
                        .start_live_stream(&entry.device_id, &entry.channel_id, 15)
                        .await;


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

    /// 三种历史写法都要认，且必须排除非国标流。
    ///
    /// 这是全局唯一的解析实现：`zlm::hook` 里同名的私有函数转发到这里。
    /// 只认 `_` 会让按需拉流对自家流失效；只认 `$`/`/` 同理。
    #[test]
    fn test_parse_stream_id_accepts_all_known_forms() {
        for (input, dev, ch) in [
            ("34020000001320000001_34020000001320000002", "34020000001320000001", "34020000001320000002"),
            ("34020000001320000001$101", "34020000001320000001", "101"),
            ("34020000001320000001/101", "34020000001320000001", "101"),
        ] {
            let (d, c) = StreamReconnectManager::parse_stream_id(input)
                .unwrap_or_else(|| panic!("{} 应能解析", input));
            assert_eq!(d, dev);
            assert_eq!(c, ch);
        }

        // 非国标流：推流 / 拉流代理 / 无分隔符
        assert!(StreamReconnectManager::parse_stream_id("push_stream1").is_none());
        assert!(StreamReconnectManager::parse_stream_id("proxy_live").is_none());
        assert!(StreamReconnectManager::parse_stream_id("justastream").is_none());
        assert!(StreamReconnectManager::parse_stream_id("_ch").is_none());
        assert!(StreamReconnectManager::parse_stream_id("dev_").is_none());
        assert!(StreamReconnectManager::parse_stream_id("").is_none());
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
