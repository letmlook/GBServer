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
    /// 本次对讲的 SSRC（SDP `y=` 与发出的 RTP 包必须一致）
    pub ssrc: Option<String>,
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
            ssrc: None,
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

    pub fn set_ssrc(&mut self, ssrc: &str) {
        self.ssrc = Some(ssrc.to_string());
    }

    /// SSRC 的数值形式（RTP 头要 u32）。SSRC 按国标是 10 位十进制。
    pub fn ssrc_u32(&self) -> Option<u32> {
        self.ssrc.as_deref().and_then(|s| s.trim().parse::<u32>().ok())
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

    pub async fn get_mut(&self, _call_id: &str) -> Option<tokio::sync::RwLockWriteGuard<'_, HashMap<String, TalkSession>>> {
        Some(self.sessions.write().await)
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

    /// 等会话进入 `Active`（设备 200 OK 时由 SIP 响应处理写入）。
    ///
    /// 存在的意义：`/api/talk/start` 发完 INVITE 就返回的话，前端会在同一微任务里
    /// 立刻去连语音 WebSocket，而 WS 只认 `Active` 会话 —— 设备 200 OK 至少要再走
    /// 一个 SIP 往返，于是**握手必然抢在 200 OK 之前**，吃 404、弹「开启对讲失败」，
    /// 且此时 `send_talk_bye` 也取不到会话（会话还是 Inviting），清理静默失败。
    /// 现在 start 会等到 Active（或超时）再返回。
    pub async fn wait_active(
        &self,
        device_id: &str,
        channel_id: &str,
        timeout_ms: u64,
    ) -> Option<TalkSession> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            if let Some(s) = self.get_by_device_channel(device_id, channel_id).await {
                return Some(s);
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    /// 按 device/channel 取会话（**不限状态**）。用于停止对讲时清理 `Inviting`
    /// 这类还没激活的会话 —— `get_by_device_channel` 只认 Active。
    pub async fn get_any_by_device_channel(
        &self,
        device_id: &str,
        channel_id: &str,
    ) -> Option<TalkSession> {
        self.sessions
            .read()
            .await
            .values()
            .find(|s| s.device_id == device_id && s.channel_id == channel_id)
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

/// 对讲 SDP。
///
/// 这里原本是第三份手写副本（与 `invite_session::build_talk_sdp`、
/// `sdp_builder::talk_sdp` 并存，且 `y=` 取值各不相同）。
/// 现在统一委托给 `sdp_builder`，只保留这个入口以兼容既有调用点。
pub fn build_talk_sdp(local_ip: &str, media_port: u16) -> String {
    build_talk_sdp_with_ssrc(local_ip, media_port, super::sdp_builder::DEFAULT_SSRC)
}

/// 带显式 SSRC 的对讲 SDP。
///
/// 对讲必须让 SDP 的 `y=` 与实际发出的 RTP 包的 SSRC 一致，否则设备侧
/// 无法把音频关联到这次会话；此前两者毫无关联（SDP 用常量 0100000001，
/// 而 RTP 发送端根本不存在）。
pub fn build_talk_sdp_with_ssrc(local_ip: &str, media_port: u16, ssrc: &str) -> String {
    super::sdp_builder::talk_sdp(local_ip, media_port, ssrc)
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
