//! Invite 会话管理

use std::collections::HashMap;
use std::net::SocketAddr;
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
    /// INVITE 时的 From 头（带 server tag），用来发 ACK 时复用，
    /// 保证 ACK 和 INVITE 同一对话（tag 一致）。
    pub from_header: Option<String>,
    /// INVITE 时的 CSeq 数字（"INVITE N" 中的 N），ACK 用 "ACK N"。
    pub cseq_num: Option<u32>,
    /// 设备地址（IP:port），发 ACK 时直接用。
    pub device_addr: Option<SocketAddr>,
    /// ACK 是否已发送。handle_response 收到 200 OK 时检查这个标志,
    /// 已发过则跳过——某些 GB28181 设备(gbcpp/1.0 mock 实测)会把每次
    /// ACK 当成新请求重新回 200 OK,造成 ACK 风暴(单 call_id 几十万次
    /// ACK 反复发送,日志爆炸 + CPU 占用 + SIP socket 拥塞)。
    /// RFC 3261 13.3.1.4: 2xx 重传由 UAC 吸收,不应再发 ACK。
    pub ack_sent: bool,
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
            from_header: None,
            cseq_num: None,
            device_addr: None,
            ack_sent: false,
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

    /// 标记该 call_id 的 INVITE 200 OK 已被 ACK 回应过。handle_response
    /// 在重新收到 200 OK（设备重传/gbcpp mock 误把 ACK 当新请求）时
    /// 检查此标志,跳过再次发送 ACK——RFC 3261 13.3.1.4 要求 2xx 重传
    /// 由 UAC 吸收不再 ACK。返回 true 表示本次是首次（实际应发 ACK）,
    /// false 表示已经发过（应跳过）。
    pub async fn mark_ack_sent_if_first(&self, call_id: &str) -> bool {
        let mut guard = self.sessions.write().await;
        if let Some(s) = guard.get_mut(call_id) {
            if s.ack_sent {
                false
            } else {
                s.ack_sent = true;
                true
            }
        } else {
            // 没有 session 记录（理论上不应发生,但保险起见允许 ACK）
            true
        }
    }

    /// 在发 INVITE 后回填 session 的 From / CSeq / device_addr，
    /// 供后续 `handle_response` 收到 200 OK 时构造 ACK。
    /// 如果 session 不存在则先用 device_id="" 占位 create（之后 handle_response
    /// 实际只需要 from/cseq/addr 三个字段）。
    pub async fn set_invite_context(
        &self,
        call_id: &str,
        from_header: String,
        cseq_num: u32,
        device_addr: SocketAddr,
    ) {
        // 先确保 session 存在(占位 create,字段后续被回填)
        {
            let guard = self.sessions.read().await;
            if !guard.contains_key(call_id) {
                drop(guard);
                self.create(call_id, "", "", "Unknown").await;
            }
        }
        let mut guard = self.sessions.write().await;
        if let Some(s) = guard.get_mut(call_id) {
            s.from_header = Some(from_header);
            s.cseq_num = Some(cseq_num);
            s.device_addr = Some(device_addr);
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
