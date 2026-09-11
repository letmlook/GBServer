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
    /// 本端 From 头的 tag。
    ///
    /// 发 BYE 时**必须**与 INVITE 的 From tag 一致（RFC 3261 §12.2.2：
    /// 对话由 Call-ID + 本地 tag + 远端 tag 三元组标识）。此前
    /// `send_session_bye` 每次重新 `generate_tag()`，设备侧匹配不到对话，
    /// 典型响应是 `481 Call/Transaction Does Not Exist` —— 设备不会停止推流。
    pub local_tag: Option<String>,
    /// 对端 To 头的 tag（来自 200 OK 的 `To` 头）。
    ///
    /// 出站 INVITE 的对端 tag 只在 200 OK 里出现，必须回填进会话，
    /// 否则 BYE 的 To 头 missing tag，同样会被设备判为 481。
    pub remote_tag: Option<String>,
    /// 本对话最近一次 INVITE 的 CSeq 序号。
    ///
    /// RFC 3261 §12.2.1.1：对话内请求的 CSeq 必须严格递增，
    /// BYE 用 `invite_cseq + 1`（此前一律硬编码 `BYE 1`，与
    /// `INVITE 1` 相等 → 设备按乱序请求拒绝）。
    pub invite_cseq: u32,
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
            local_tag: None,
            remote_tag: None,
            invite_cseq: 1,
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

    /// 记录设备在 200 OK 里返回的应答 SDP。
    ///
    /// 与 `set_sdp`（记录**请求** SDP）分开：应答里的 `m=` 端口代表设备
    /// 实际推流的目标端口，TCP 被动模式下与请求端口不同，是不能互相覆盖的
    /// 两条信息。
    pub fn set_sdp_response(&mut self, sdp: &str) {
        // 应答 SDP 里的 c=/m= 是设备侧信息；只记录文本，不覆盖请求侧的
        // media_port/ssrc（请求侧是 ZLM 收流端口与 SSRC，二者语义不同）。
        self.sdp_response = Some(sdp.to_string());
    }

    pub fn set_zlm_stream(&mut self, stream_id: &str, app: &str) {
        self.zlm_stream_id = Some(stream_id.to_string());
        self.zlm_app = app.to_string();
    }

    /// 记录对话标识（本地 From tag / 对端 To tag / INVITE 的 CSeq 序号）。
    ///
    /// 出站 INVITE 发出后立刻记录本地 tag 与 CSeq；对端 tag 要等 200 OK
    /// 到达时再回填（见 `SipServer::handle_response`）。
    pub fn set_local_dialog(&mut self, local_tag: &str, invite_cseq: u32) {
        self.local_tag = Some(local_tag.to_string());
        self.invite_cseq = invite_cseq;
    }

    pub fn set_remote_tag(&mut self, remote_tag: &str) {
        self.remote_tag = Some(remote_tag.to_string());
    }

    /// 构造 BYE 需要的 CSeq 序号：对话内必须严格递增。
    pub fn bye_cseq(&self) -> u32 {
        self.invite_cseq.saturating_add(1)
    }

    pub fn is_active(&self) -> bool {
        self.status == InviteSessionStatus::Active
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

impl crate::state::StreamState for InviteSession {
    fn stream_id(&self) -> &str {
        self.zlm_stream_id.as_deref().unwrap_or(&self.call_id)
    }
    fn app(&self) -> &str {
        &self.zlm_app
    }
    fn status(&self) -> crate::state::StreamStatus {
        use InviteSessionStatus::*;
        match self.status {
            Active => crate::state::StreamStatus::Active,
            Pending | Inviting | Ringing => crate::state::StreamStatus::Pushing,
            Terminating | Terminated => crate::state::StreamStatus::Stopped,
        }
    }
    fn set_status(&mut self, status: crate::state::StreamStatus) {
        use crate::state::StreamStatus;
        self.status = match status {
            StreamStatus::Ready | StreamStatus::Pushing => InviteSessionStatus::Inviting,
            StreamStatus::Active => InviteSessionStatus::Active,
            StreamStatus::Stopped | StreamStatus::Failed => InviteSessionStatus::Terminated,
        };
    }
    fn media_server_id(&self) -> Option<&str> {
        None
    }
    fn device_id(&self) -> Option<&str> {
        Some(&self.device_id)
    }
    fn channel_id(&self) -> Option<&str> {
        Some(&self.channel_id)
    }
}

pub struct InviteSessionManager {
    sessions: Arc<RwLock<HashMap<String, InviteSession>>>,
    /// E1: 可选 StateStore，让活跃会话在多节点之间共享
    state_store: Option<Arc<crate::state_store::StateStore>>,
}

impl InviteSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            state_store: None,
        }
    }

    /// E1: 注入 StateStore
    pub fn set_state_store(&mut self, store: Arc<crate::state_store::StateStore>) {
        self.state_store = Some(store);
    }

    /// E1: 把单个会话状态同步到 StateStore
    fn sync_to_store(&self, session: &InviteSession) {
        if let Some(ref store) = self.state_store {
            store.set_invite_session(&session.call_id, crate::state_store::InviteSessionState {
                call_id: session.call_id.clone(),
                device_id: session.device_id.clone(),
                channel_id: session.channel_id.clone(),
                session_type: format!("{:?}", session.stream_type),
                zlm_stream_id: session.zlm_stream_id.clone(),
                status: format!("{:?}", session.status),
                created_at: session.created_at,
                last_activity: session.last_activity,
            });
        }
    }

    pub async fn create(&self, session: InviteSession) -> String {
        let call_id = session.call_id.clone();
        // E1: 同步 StateStore
        self.sync_to_store(&session);
        self.sessions.write().await.insert(call_id.clone(), session);
        call_id
    }

    pub async fn get(&self, call_id: &str) -> Option<InviteSession> {
        self.sessions.read().await.get(call_id).cloned()
    }

    pub async fn get_mut(&self, _call_id: &str) -> Option<tokio::sync::RwLockWriteGuard<'_, HashMap<String, InviteSession>>> {
        Some(self.sessions.write().await)
    }

    pub async fn update(&self, session: &InviteSession) {
        let mut guard = self.sessions.write().await;
        guard.insert(session.call_id.clone(), session.clone());
        // E1: 同步 StateStore
        self.sync_to_store(session);
    }

    pub async fn remove(&self, call_id: &str) -> Option<InviteSession> {
        // E1: 同步 StateStore 删除
        if let Some(ref store) = self.state_store {
            store.remove_invite_session(call_id);
        }
        self.sessions.write().await.remove(call_id)
    }

    /// 收到 200 OK 后回填对端 tag / 应答 SDP，并把会话置为 `Active`。
    ///
    /// 对端 tag 只出现在 200 OK 的 `To` 头里，必须在这里落库，
    /// 否则后续 BYE 的 To 头缺 tag，设备按"对话不存在"处理。
    pub async fn mark_answered(
        &self,
        call_id: &str,
        remote_tag: Option<&str>,
        sdp_response: Option<&str>,
    ) -> bool {
        let mut guard = self.sessions.write().await;
        let Some(session) = guard.get_mut(call_id) else {
            return false;
        };
        if let Some(tag) = remote_tag.filter(|t| !t.is_empty()) {
            session.set_remote_tag(tag);
        }
        if let Some(sdp) = sdp_response {
            session.set_sdp_response(sdp);
        }
        session.status = InviteSessionStatus::Active;
        session.update_activity();
        let snapshot = session.clone();
        drop(guard);
        self.sync_to_store(&snapshot);
        true
    }

    /// 按设备/通道回填 ZLM 流标识。
    ///
    /// 出站 INVITE 发出时还不知道 ZLM 会给哪个 stream_id 收流
    /// （要等 `openRtpServer` 之后的媒体到达），因此这条信息在
    /// 媒体就绪后回填，供 BYE 时释放 RTP 端口使用。
    pub async fn set_zlm_stream_by_device_channel(
        &self,
        device_id: &str,
        channel_id: &str,
        stream_id: &str,
        app: &str,
    ) -> bool {
        let mut guard = self.sessions.write().await;
        let Some(session) = guard.values_mut().find(|s| {
            s.device_id == device_id
                && s.channel_id == channel_id
                && s.status != InviteSessionStatus::Terminated
        }) else {
            return false;
        };
        session.set_zlm_stream(stream_id, app);
        let snapshot = session.clone();
        drop(guard);
        self.sync_to_store(&snapshot);
        true
    }

    pub async fn get_by_device_channel(&self, device_id: &str, channel_id: &str) -> Option<InviteSession> {
        let guard = self.sessions.read().await;
        // Consider sessions that are not terminated (Inviting, Ringing, Active)
        guard.values()
            .find(|s| s.device_id == device_id && s.channel_id == channel_id && s.status != InviteSessionStatus::Terminated)
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

        // E1: 同步删除 StateStore 中的过期会话
        if let Some(ref store) = self.state_store {
            for call_id in &removed {
                store.remove_invite_session(call_id);
            }
        }

        removed
    }

    pub async fn update_status(&self, call_id: &str, status: InviteSessionStatus) {
        let mut guard = self.sessions.write().await;
        if let Some(session) = guard.get_mut(call_id) {
            session.status = status;
            session.last_activity = Utc::now();
            // E1: 同步 StateStore
            self.sync_to_store(session);
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

// ---------------------------------------------------------------------------
// INVITE SDP 构造统一委托给 `sdp_builder`。
//
// 这里曾经有一份手写实现，与 `sdp_builder.rs`（同样存在但当时无人调用）
// 以及 `talk.rs::build_talk_sdp` 三者并存，且互相矛盾：
//   * 方向：本文件写死 `a=sendonly`，`sdp_builder` 对点播用 `a=recvonly`；
//     按 GB/T 28181-2016 附录示例，平台发出的取流 INVITE 应为 recvonly
//     （平台收、设备发），设备 200 OK 才是 sendonly。
//   * 回放 `y=`：写死 `0100000001`，所有回放会话共用一个 SSRC。
//   * 回放 `t=`：直接把 ISO 时间串写进去，而国标要求 UNIX 秒。
//   * 对讲会话名大小写：本文件 `s=TALK`，`talk.rs` `s=Talk`。
//
// 现在只有 `sdp_builder` 一处实现，下面三个函数只做参数适配。
// ---------------------------------------------------------------------------

pub fn build_invite_sdp(
    local_ip: &str,
    media_port: u16,
    stream_type: &str,
    ssrc: Option<&str>,
) -> String {
    super::sdp_builder::SdpBuilder::new(
        local_ip,
        media_port,
        super::sdp_builder::stream_type_from_str(stream_type),
        ssrc.unwrap_or(super::sdp_builder::DEFAULT_SSRC),
    )
    .build()
}

pub fn build_talk_sdp(local_ip: &str, audio_port: u16) -> String {
    super::sdp_builder::talk_sdp(
        local_ip,
        audio_port,
        super::sdp_builder::DEFAULT_SSRC,
    )
}

/// 回放 SDP。`ssrc` 由调用方提供（回放流的 SSRC 需与 Subject/`y=` 一致）。
pub fn build_playback_sdp(
    local_ip: &str,
    media_port: u16,
    start_time: &str,
    end_time: &str,
    ssrc: Option<&str>,
) -> String {
    super::sdp_builder::SdpBuilder::new(
        local_ip,
        media_port,
        StreamType::Playback,
        ssrc.unwrap_or(super::sdp_builder::DEFAULT_SSRC),
    )
    .time_range(start_time, end_time)
    .build()
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

// ============================================================================
// 增强：生产级 InviteSessionStore
// 新增方法（兼容原有接口，仅扩展）
// ============================================================================

impl InviteSession {
    /// 按会话类型判断是否应关闭 ZLM 流
    pub fn should_close_zlm_on_stop(&self) -> bool {
        matches!(
            self.stream_type,
            StreamType::Play | StreamType::Playback | StreamType::Download
        )
    }

    /// 判断是否可以接受媒体到达
    pub fn can_accept_media(&self) -> bool {
        matches!(
            self.status,
            InviteSessionStatus::Inviting | InviteSessionStatus::Ringing
        )
    }

    /// 会话是否已完成使命（可清理）
    pub fn is_resolved(&self) -> bool {
        matches!(
            self.status,
            InviteSessionStatus::Terminated | InviteSessionStatus::Terminating
        )
    }

    /// 获取会话存活时长（秒）
    pub fn age_seconds(&self) -> i64 {
        (Utc::now() - self.last_activity).num_seconds()
    }
}

impl InviteSessionManager {
    /// 按会话类型查找设备的活跃会话
    pub async fn get_active_session_by_type(
        &self,
        device_id: &str,
        channel_id: &str,
        stream_type: StreamType,
    ) -> Option<InviteSession> {
        let guard = self.sessions.read().await;
        guard.values().find(|s| {
            s.device_id == device_id
            && s.channel_id == channel_id
            && s.stream_type == stream_type
            && s.is_active()
        }).cloned()
    }

    /// 获取设备的全部会话
    pub async fn get_sessions_by_device(&self, device_id: &str) -> Vec<InviteSession> {
        self.sessions.read().await
            .values()
            .filter(|s| s.device_id == device_id)
            .cloned()
            .collect()
    }

    /// 获取设备的活跃通道数（用于资源限制）
    pub async fn active_channel_count(&self, device_id: &str) -> usize {
        self.sessions.read().await
            .values()
            .filter(|s| s.device_id == device_id && s.is_active())
            .count()
    }

    /// 按媒体服务器 ID 查找活跃会话（用于 ZLM 节点下线时清理）
    pub async fn get_sessions_by_zlm(&self, _media_server_id: &str) -> Vec<InviteSession> {
        self.sessions.read().await
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    /// 将会话转入 Terminating 状态（优雅关闭中）
    pub async fn mark_terminating(&self, call_id: &str) {
        let mut guard = self.sessions.write().await;
        if let Some(session) = guard.get_mut(call_id) {
            session.status = InviteSessionStatus::Terminating;
            session.last_activity = Utc::now();
        }
    }

    /// 将会话标记为终止，并返回被终止的会话信息（用于通知清理 ZLM）
    pub async fn terminate(&self, call_id: &str) -> Option<InviteSession> {
        let mut guard = self.sessions.write().await;
        if let Some(session) = guard.get_mut(call_id) {
            let old = session.clone();
            session.status = InviteSessionStatus::Terminated;
            session.last_activity = Utc::now();
            return Some(old);
        }
        None
    }

    /// 将会话激活（收到 200 OK 后调用）
    pub async fn activate(&self, call_id: &str) {
        self.update_status(call_id, InviteSessionStatus::Active).await;
    }

    /// 批量清理所有已终止且超过 max_age 秒的会话
    /// 返回被清理的 call_id 列表
    pub async fn purge_expired(&self, max_age_secs: i64) -> Vec<String> {
        let now = Utc::now();
        let mut guard = self.sessions.write().await;
        let mut removed = Vec::new();

        guard.retain(|call_id, session| {
            let age = (now - session.last_activity).num_seconds();
            if age > max_age_secs && session.is_resolved() {
                removed.push(call_id.clone());
                return false;
            }
            true
        });

        removed
    }

    /// 获取总会话数（用于监控）
    pub async fn total_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// 获取按状态分组的会话统计
    pub async fn stats(&self) -> SessionStats {
        let guard = self.sessions.read().await;
        let mut stats = SessionStats::default();
        for s in guard.values() {
            match s.status {
                InviteSessionStatus::Pending  => stats.pending += 1,
                InviteSessionStatus::Inviting => stats.inviting += 1,
                InviteSessionStatus::Ringing   => stats.ringing += 1,
                InviteSessionStatus::Active    => stats.active += 1,
                InviteSessionStatus::Terminating => stats.terminating += 1,
                InviteSessionStatus::Terminated => stats.terminated += 1,
            }
        }
        stats
    }
}

/// 会话统计（用于监控和告警）
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub pending: usize,
    pub inviting: usize,
    pub ringing: usize,
    pub active: usize,
    pub terminating: usize,
    pub terminated: usize,
}

