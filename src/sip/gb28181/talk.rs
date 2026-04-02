use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct TalkSession {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub start_time: DateTime<Utc>,
    pub active: bool,
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

    pub async fn start(&self, call_id: &str, device_id: &str, channel_id: &str) -> TalkSession {
        let session = TalkSession {
            call_id: call_id.to_string(),
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            start_time: Utc::now(),
            active: true,
        };
        self.sessions.write().await.insert(call_id.to_string(), session.clone());
        session
    }

    pub async fn stop(&self, call_id: &str) {
        if let Some(mut s) = self.sessions.write().await.get_mut(call_id) {
            s.active = false;
        }
    }

    pub async fn is_active(&self, call_id: &str) -> bool {
        self.sessions.read().await.get(call_id)
            .map(|s| s.active)
            .unwrap_or(false)
    }

    pub async fn get(&self, call_id: &str) -> Option<TalkSession> {
        self.sessions.read().await.get(call_id).cloned()
    }
}

impl Default for TalkManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_invite_sdp(channel_id: &str, media_port: u16) -> String {
    format!(
        "v=0\r\n\
         o={} 0 0 IN IP4 127.0.0.1\r\n\
         s=Talk\r\n\
         c=IN IP4 127.0.0.1\r\n\
         t=0 0\r\n\
         m=audio {} RTP/AVP 8 0 101\r\n\
         a=rtpmap:8 PCMA/8000\r\n\
         a=rtpmap:0 PCMU/8000\r\n\
         a=rtpmap:101 telephone-event/8000\r\n\
         a=sendonly\r\n\
         y=010000\r\n",
        channel_id,
        media_port
    )
}

pub fn parse_sdp_answer(sdp: &str) -> Option<(String, u16)> {
    let mut media_ip = None;
    let mut media_port = None;

    for line in sdp.lines() {
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

pub fn build_ok_response(sdp: &str, local_port: u16) -> String {
    format!(
        "SIP/2.0 200 OK\r\n\
         Via: SIP/2.0/UDP 127.0.0.1\r\n\
         From: <sip:{}@127.0.0.1>\r\n\
         To: <sip:device@127.0.0.1>;tag=server\r\n\
         Call-ID: talk\r\n\
         CSeq: 1 INVITE\r\n\
         Content-Type: application/sdp\r\n\
         Content-Length: {}\r\n\r\n\
         {}",
        local_port,
        sdp.len(),
        sdp
    )
}
