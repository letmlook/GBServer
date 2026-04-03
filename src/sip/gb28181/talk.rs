use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalkStatus {
    Pending,
    Inviting,
    Ringing,
    Active,
    Terminating,
    Terminated,
}

#[derive(Debug, Clone)]
pub struct TalkSession {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub device_ip: String,
    pub device_port: u16,
    pub zlm_stream_id: Option<String>,
    pub local_port: u16,
    pub status: TalkStatus,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub timeout_seconds: u64,
}

impl TalkSession {
    pub fn new(call_id: &str, device_id: &str, channel_id: &str) -> Self {
        Self {
            call_id: call_id.to_string(),
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            device_ip: String::new(),
            device_port: 0,
            zlm_stream_id: None,
            local_port: 0,
            status: TalkStatus::Pending,
            start_time: Utc::now(),
            last_activity: Utc::now(),
            timeout_seconds: 60,
        }
    }

    pub fn set_device_info(&mut self, ip: &str, port: u16) {
        self.device_ip = ip.to_string();
        self.device_port = port;
    }

    pub fn set_zlm_stream(&mut self, stream_id: &str) {
        self.zlm_stream_id = Some(stream_id.to_string());
    }

    pub fn set_local_port(&mut self, port: u16) {
        self.local_port = port;
    }

    pub fn is_active(&self) -> bool {
        self.status == TalkStatus::Active
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

pub struct TalkManager {
    sessions: Arc<RwLock<HashMap<String, TalkSession>>>,
}

impl TalkManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self, call_id: &str, device_id: &str, channel_id: &str) -> TalkSession {
        let session = TalkSession::new(call_id, device_id, channel_id);
        self.sessions.write().await.insert(call_id.to_string(), session.clone());
        session
    }

    pub async fn get(&self, call_id: &str) -> Option<TalkSession> {
        self.sessions.read().await.get(call_id).cloned()
    }

    pub async fn get_mut(&self, call_id: &str) -> Option<tokio::sync::RwLockWriteGuard<'_, HashMap<String, TalkSession>>> {
        None
    }

    pub async fn update(&self, session: &TalkSession) {
        let mut guard = self.sessions.write().await;
        guard.insert(session.call_id.clone(), session.clone());
    }

    pub async fn remove(&self, call_id: &str) -> Option<TalkSession> {
        self.sessions.write().await.remove(call_id)
    }

    pub async fn get_by_device_channel(&self, device_id: &str, channel_id: &str) -> Option<TalkSession> {
        self.sessions.read().await
            .values()
            .find(|s| s.device_id == device_id && s.channel_id == channel_id && s.is_active())
            .cloned()
    }

    pub async fn get_active_sessions(&self) -> Vec<TalkSession> {
        self.sessions.read().await
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    pub async fn cleanup_expired(&self, max_age_secs: i64) -> Vec<String> {
        let now = Utc::now();
        let mut guard = self.sessions.write().await;
        let mut removed = Vec::new();
        
        guard.retain(|call_id, session| {
            let age = (now - session.last_activity).num_seconds();
            if age > max_age_secs && session.status == TalkStatus::Terminated {
                removed.push(call_id.clone());
                return false;
            }
            true
        });
        
        removed
    }

    pub async fn update_status(&self, call_id: &str, status: TalkStatus) {
        let mut guard = self.sessions.write().await;
        if let Some(session) = guard.get_mut(call_id) {
            session.status = status;
            session.last_activity = Utc::now();
        }
    }
}

impl Default for TalkManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_talk_sdp(local_ip: &str, media_port: u16) -> String {
    format!(
        "v=0\r\n\
        o=- 0 0 IN IP4 {}\r\n\
        s=Talk\r\n\
        c=IN IP4 {}\r\n\
        t=0 0\r\n\
        m=audio {} RTP/AVP 8 0 101\r\n\
        a=rtpmap:8 PCMA/8000\r\n\
        a=rtpmap:0 PCMU/8000\r\n\
        a=rtpmap:101 telephone-event/8000\r\n\
        a=sendrecv\r\n\
        y=020000\r\n",
        local_ip, local_ip, media_port
    )
}

pub fn parse_talk_sdp(sdp: &str) -> Option<(String, u16)> {
    let mut media_ip = None;
    let mut media_port = None;

    for line in sdp.lines() {
        let line = line.trim();
        if line.starts_with("c=IN IP4 ") {
            media_ip = Some(line[9..].to_string());
        } else if line.starts_with("m=audio ") {
            if let Some(port_str) = line.split_whitespace().nth(1) {
                media_port = port_str.parse().ok();
            }
        }
    }

    match (media_ip, media_port) {
        (Some(ip), Some(port)) => Some((ip, port)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_talk_sdp_parse() {
        let sdp = build_talk_sdp("192.168.1.1", 8000);
        let (ip, port) = parse_talk_sdp(&sdp).unwrap();
        assert_eq!(ip, "192.168.1.1");
        assert_eq!(port, 8000);
    }
}
