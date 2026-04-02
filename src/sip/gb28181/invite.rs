//! Invite 会话管理

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct InviteSession {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub ssrc: Option<String>,
    pub stream_type: String,
    pub stream_addr: Option<String>,
    pub stream_port: Option<u16>,
    pub created_at: DateTime<Utc>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Pending,
    Inviting,
    Ringing,
    Active,
    Terminated,
}

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, InviteSession>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self, call_id: &str, device_id: &str, channel_id: &str, stream_type: &str) {
        let session = InviteSession {
            call_id: call_id.to_string(),
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            ssrc: None,
            stream_type: stream_type.to_string(),
            stream_addr: None,
            stream_port: None,
            created_at: Utc::now(),
            status: SessionStatus::Inviting,
        };
        self.sessions.write().await.insert(call_id.to_string(), session);
    }

    pub async fn get(&self, call_id: &str) -> Option<InviteSession> {
        self.sessions.read().await.get(call_id).cloned()
    }

    pub async fn update_status(&self, call_id: &str, status: SessionStatus) {
        let mut guard = self.sessions.write().await;
        if let Some(s) = guard.get_mut(call_id) {
            s.status = status;
        }
    }

    pub async fn set_stream_info(&self, call_id: &str, addr: &str, port: u16) {
        let mut guard = self.sessions.write().await;
        if let Some(s) = guard.get_mut(call_id) {
            s.stream_addr = Some(addr.to_string());
            s.stream_port = Some(port);
            s.status = SessionStatus::Active;
        }
    }

    pub async fn remove(&self, call_id: &str) {
        self.sessions.write().await.remove(call_id);
    }

    pub async fn get_by_device_channel(&self, device_id: &str, channel_id: &str) -> Option<InviteSession> {
        let guard = self.sessions.read().await;
        guard.values()
            .find(|s| s.device_id == device_id && s.channel_id == channel_id && s.status == SessionStatus::Active)
            .cloned()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
