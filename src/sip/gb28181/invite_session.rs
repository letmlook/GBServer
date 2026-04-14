use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamType {
    Play,
    Playback,
    Download,
    Talk,
    Broadcast,
}

#[derive(Debug, Clone)]
pub struct SdpInfo {
    pub session_name: String,
    pub connection_info: String,
    pub media_lines: Vec<MediaLine>,
    pub origin: String,
    pub bandwidth: Option<String>,
    pub timing: String,
}

#[derive(Debug, Clone)]
pub struct MediaLine {
    pub media: String,
    pub port: u16,
    pub proto: String,
    pub format: String,
    pub rtpmap: Option<String>,
    pub fmtp: Option<String>,
    pub sendrecv: Option<String>,
    pub track_id: Option<String>,
}

impl SdpInfo {
    pub fn parse(sdp: &str) -> Option<Self> {
        let mut session_name = String::new();
        let mut connection_info = String::new();
        let mut origin = String::new();
        let mut timing = String::new();
        let mut bandwidth = None;
        let mut media_lines = Vec::new();
        let mut current_media: Option<MediaLine> = None;

        for line in sdp.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let (key, value) = if let Some(pos) = line.find('=') {
                (&line[..pos], &line[pos+1..])
            } else {
                continue;
            };

            match key {
                "v" => {},
                "o" => origin = value.to_string(),
                "s" => session_name = value.to_string(),
                "c" => connection_info = value.to_string(),
                "t" => timing = value.to_string(),
                "b" => bandwidth = Some(value.to_string()),
                "m" => {
                    if let Some(media) = current_media.take() {
                        media_lines.push(media);
                    }
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() >= 3 {
                        current_media = Some(MediaLine {
                            media: parts[0].to_string(),
                            port: parts[1].parse().unwrap_or(0),
                            proto: parts.get(2).unwrap_or(&"").to_string(),
                            format: parts.get(3).unwrap_or(&"").to_string(),
                            rtpmap: None,
                            fmtp: None,
                            sendrecv: None,
                            track_id: None,
                        });
                    }
                }
                "a" => {
                    if let Some(ref mut media) = current_media {
                        if value.starts_with("rtpmap:") {
                            media.rtpmap = Some(value.to_string());
                        } else if value.starts_with("fmtp:") {
                            media.fmtp = Some(value.to_string());
                        } else if value.starts_with("sendonly") || value.starts_with("recvonly") || 
                                  value.starts_with("sendrecv") || value.starts_with("inactive") {
                            media.sendrecv = Some(value.to_string());
                        } else if value.starts_with("track:") {
                            media.track_id = Some(value.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(media) = current_media {
            media_lines.push(media);
        }

        Some(SdpInfo {
            session_name,
            connection_info,
            media_lines,
            origin,
            bandwidth,
            timing,
        })
    }

    pub fn get_video_port(&self) -> Option<u16> {
        self.media_lines.iter()
            .find(|m| m.media == "video")
            .map(|m| m.port)
    }

    pub fn get_audio_port(&self) -> Option<u16> {
        self.media_lines.iter()
            .find(|m| m.media == "audio")
            .map(|m| m.port)
    }

    pub fn get_ssrc(&self) -> Option<String> {
        for media in &self.media_lines {
            if let Some(ref rtpmap) = media.rtpmap {
                if rtpmap.contains("PS/90000") {
                    return media.track_id.clone();
                }
            }
        }
        None
    }

    pub fn has_video(&self) -> bool {
        self.media_lines.iter().any(|m| m.media == "video")
    }

    pub fn has_audio(&self) -> bool {
        self.media_lines.iter().any(|m| m.media == "audio")
    }
}

#[derive(Debug, Clone)]
pub struct InviteSession {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub stream_type: StreamType,
    pub ssrc: Option<String>,
    pub device_ip: String,
    pub device_port: u16,
    pub zlm_stream_id: Option<String>,
    pub zlm_app: String,
    pub media_port: u16,
    pub audio_port: Option<u16>,
    pub status: InviteSessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub peer_addr: SocketAddr,
    pub sdp_request: Option<String>,
    pub sdp_response: Option<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteSessionStatus {
    Pending,
    Inviting,
    Ringing,
    Active,
    Terminating,
    Terminated,
}

impl InviteSession {
    pub fn new(
        call_id: &str,
        device_id: &str,
        channel_id: &str,
        stream_type: StreamType,
        peer_addr: SocketAddr,
    ) -> Self {
        let app = Self::default_app_for_stream_type(&stream_type);
        Self {
            call_id: call_id.to_string(),
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            stream_type,
            ssrc: None,
            device_ip: String::new(),
            device_port: 0,
            zlm_stream_id: None,
            zlm_app: app,
            media_port: 0,
            audio_port: None,
            status: InviteSessionStatus::Pending,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            peer_addr,
            sdp_request: None,
            sdp_response: None,
            timeout_seconds: 60,
        }
    }

    fn default_app_for_stream_type(stream_type: &StreamType) -> String {
        match stream_type {
            StreamType::Play => "rtp".to_string(),
            StreamType::Playback => "playback".to_string(),
            StreamType::Download => "download".to_string(),
            StreamType::Talk => "talk".to_string(),
            StreamType::Broadcast => "broadcast".to_string(),
        }
    }

    pub fn set_device_info(&mut self, ip: &str, port: u16) {
        self.device_ip = ip.to_string();
        self.device_port = port;
    }

    pub fn set_sdp(&mut self, sdp: &str) {
        if let Some(sdp_info) = SdpInfo::parse(sdp) {
            self.media_port = sdp_info.get_video_port().unwrap_or(0);
            self.audio_port = sdp_info.get_audio_port();
            self.ssrc = sdp_info.get_ssrc();
        }
        self.sdp_request = Some(sdp.to_string());
    }

    pub fn set_zlm_stream(&mut self, stream_id: &str, app: &str) {
        self.zlm_stream_id = Some(stream_id.to_string());
        self.zlm_app = app.to_string();
    }

    pub fn is_active(&self) -> bool {
        self.status == InviteSessionStatus::Active
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

pub struct InviteSessionManager {
    sessions: Arc<RwLock<HashMap<String, InviteSession>>>,
}

impl InviteSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self, session: InviteSession) -> String {
        let call_id = session.call_id.clone();
        self.sessions.write().await.insert(call_id.clone(), session);
        call_id
    }

    pub async fn get(&self, call_id: &str) -> Option<InviteSession> {
        self.sessions.read().await.get(call_id).cloned()
    }

    pub async fn get_mut(&self, call_id: &str) -> Option<tokio::sync::RwLockWriteGuard<'_, HashMap<String, InviteSession>>> {
        None
    }

    pub async fn update(&self, session: &InviteSession) {
        let mut guard = self.sessions.write().await;
        guard.insert(session.call_id.clone(), session.clone());
    }

    pub async fn remove(&self, call_id: &str) -> Option<InviteSession> {
        self.sessions.write().await.remove(call_id)
    }

    pub async fn get_by_device_channel(&self, device_id: &str, channel_id: &str) -> Option<InviteSession> {
        let guard = self.sessions.read().await;
        guard.values()
            .find(|s| s.device_id == device_id && s.channel_id == channel_id && s.is_active())
            .cloned()
    }

    pub async fn get_active_sessions(&self) -> Vec<InviteSession> {
        self.sessions.read().await
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    pub async fn get_pending_sessions(&self) -> Vec<InviteSession> {
        self.sessions.read().await
            .values()
            .filter(|s| s.status == InviteSessionStatus::Pending || 
                      s.status == InviteSessionStatus::Inviting ||
                      s.status == InviteSessionStatus::Ringing)
            .cloned()
            .collect()
    }

    pub async fn cleanup_expired(&self, max_age_secs: i64) -> Vec<String> {
        let now = Utc::now();
        let mut guard = self.sessions.write().await;
        let mut removed = Vec::new();
        
        guard.retain(|call_id, session| {
            let age = (now - session.last_activity).num_seconds();
            if age > max_age_secs && session.status == InviteSessionStatus::Terminated {
                removed.push(call_id.clone());
                return false;
            }
            true
        });
        
        removed
    }

    pub async fn update_status(&self, call_id: &str, status: InviteSessionStatus) {
        let mut guard = self.sessions.write().await;
        if let Some(session) = guard.get_mut(call_id) {
            session.status = status;
            session.last_activity = Utc::now();
        }
    }

    pub async fn find_by_call_id(&self, call_id: &str) -> Option<InviteSession> {
        self.sessions.read().await.get(call_id).cloned()
    }

    pub async fn is_stream_active(&self, device_id: &str, channel_id: &str) -> bool {
        self.get_by_device_channel(device_id, channel_id).await
            .map(|s| s.is_active())
            .unwrap_or(false)
    }
}

impl Default for InviteSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_invite_sdp(
    local_ip: &str,
    media_port: u16,
    stream_type: &str,
    ssrc: Option<&str>,
) -> String {
    let ssrc_str = ssrc.unwrap_or("0100000001");
    format!("v=0\r\no=- 0 0 IN IP4 {}\r\ns={}\r\nc=IN IP4 {}\r\nt=0 0\r\nm=video {} RTP/AVP 96\r\na=rtpmap:96 PS/90000\r\na=sendonly\r\ny={}\r\nf=v/1/96/1/2/1/1/0\r\n",
        local_ip, stream_type, local_ip, media_port, ssrc_str)
}

pub fn build_talk_sdp(local_ip: &str, audio_port: u16) -> String {
    format!("v=0\r\no=- 0 0 IN IP4 {}\r\ns=TALK\r\nc=IN IP4 {}\r\nt=0 0\r\nm=audio {} RTP/AVP 8 0 101\r\na=rtpmap:8 PCMA/8000\r\na=rtpmap:0 PCMU/8000\r\na=rtpmap:101 telephone-event/8000\r\na=sendrecv\r\ny=020000\r\n",
        local_ip, local_ip, audio_port)
}

pub fn build_playback_sdp(local_ip: &str, media_port: u16, start_time: &str, end_time: &str) -> String {
    let t_field = if !start_time.is_empty() && start_time != "0" {
        format!("{} {}", start_time, end_time)
    } else {
        "0 0".to_string()
    };
    format!("v=0\r\no=- 0 0 IN IP4 {}\r\ns=Playback\r\nc=IN IP4 {}\r\nt={}\r\nm=video {} RTP/AVP 96\r\na=rtpmap:96 PS/90000\r\na=sendonly\r\ny=0100000001\r\nf=v/1/96/1/2/1/1/0\r\n",
        local_ip, local_ip, t_field, media_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdp_parse() {
        let sdp = r#"v=0
o=- 0 0 IN IP4 192.168.1.100
s=Play
c=IN IP4 192.168.1.100
t=0 0
m=video 50000 RTP/AVP 96
a=rtpmap:96 PS/90000
a=sendonly
y=0100000001
f=v/1/96/1/2/1/1/0
"#;
        
        let info = SdpInfo::parse(sdp).unwrap();
        assert_eq!(info.session_name, "Play");
        assert_eq!(info.get_video_port(), Some(50000));
        assert!(info.has_video());
    }
}
