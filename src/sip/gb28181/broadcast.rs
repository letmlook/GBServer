//! BroadcastManager — 语音广播会话管理（设备 → 多个客户端）
//!
//! 与 TalkManager 的区别：
//! - Talk：客户端 → 设备（一对一，客户端对讲）
//! - Broadcast：设备 → 客户端（一对多，设备广播声音给多个客户端）
//! - Subject 第 4 段 SSRC 前缀：Talk=3，Broadcast=4（与 WVP Java 兼容）
//!
//! Phase 3.5: 把 broadcast 与 talk 拆分为独立 manager，避免 BYE 走错方向

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastStatus {
    Pending,
    Inviting,
    Ringing,
    Active,
    Terminating,
    Terminated,
}

#[derive(Debug, Clone)]
pub struct BroadcastSession {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub device_ip: String,
    pub device_port: u16,
    pub zlm_stream_id: Option<String>,
    pub local_port: u16,
    pub status: BroadcastStatus,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub timeout_seconds: u64,
    /// 本端 From 头的 tag（INVITE 时生成，BYE 必须逐字复用）。
    pub local_tag: Option<String>,
    /// 对端 To 头的 tag（来自 200 OK 的 `To` 头，BYE 必须带上）。
    pub remote_tag: Option<String>,
    /// 本次 INVITE 的 CSeq（BYE 必须严格大于它）。
    pub invite_cseq: u32,
}

impl BroadcastSession {
    pub fn new(call_id: &str, device_id: &str, channel_id: &str) -> Self {
        Self {
            call_id: call_id.to_string(),
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            device_ip: String::new(),
            device_port: 0,
            zlm_stream_id: None,
            local_port: 0,
            status: BroadcastStatus::Pending,
            start_time: Utc::now(),
            last_activity: Utc::now(),
            timeout_seconds: 60,
            local_tag: None,
            remote_tag: None,
            invite_cseq: 1,
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

    /// 记录对话标识（本地 From tag / INVITE CSeq）。
    pub fn set_local_dialog(&mut self, local_tag: &str, invite_cseq: u32) {
        self.local_tag = Some(local_tag.to_string());
        self.invite_cseq = invite_cseq;
    }

    /// 回填 200 OK 里对端 To 头的 tag。
    pub fn set_remote_tag(&mut self, remote_tag: &str) {
        if !remote_tag.is_empty() {
            self.remote_tag = Some(remote_tag.to_string());
        }
    }

    /// BYE 的 CSeq：对话内必须严格递增。
    pub fn bye_cseq(&self) -> u32 {
        self.invite_cseq.saturating_add(1)
    }

    pub fn is_active(&self) -> bool {
        self.status == BroadcastStatus::Active
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

pub struct BroadcastManager {
    sessions: Arc<RwLock<HashMap<String, BroadcastSession>>>,
}

impl BroadcastManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建一个广播会话
    pub async fn create(&self, session: BroadcastSession) {
        self.sessions
            .write()
            .await
            .insert(session.call_id.clone(), session);
    }

    pub async fn get(&self, call_id: &str) -> Option<BroadcastSession> {
        self.sessions.read().await.get(call_id).cloned()
    }

    /// 按 device_id + channel_id 查找（一个设备同通道只允许一个广播）
    pub async fn get_by_device_channel(
        &self,
        device_id: &str,
        channel_id: &str,
    ) -> Option<BroadcastSession> {
        self.sessions
            .read()
            .await
            .values()
            .find(|s| s.device_id == device_id && s.channel_id == channel_id)
            .cloned()
    }

    pub async fn activate(&self, call_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(call_id) {
            s.status = BroadcastStatus::Active;
            s.update_activity();
        }
    }

    /// 收到设备 200 OK：回填对端 To tag，并把会话置为 `Active`。
    ///
    /// 对端 tag 只在 200 OK 的 `To` 头里出现，BYE 的 `To` 缺它会被设备
    /// 判为"对话不存在"（481），设备继续推流。
    pub async fn mark_answered(&self, call_id: &str, remote_tag: Option<&str>) -> bool {
        let mut guard = self.sessions.write().await;
        let Some(s) = guard.get_mut(call_id) else {
            return false;
        };
        if let Some(tag) = remote_tag.filter(|t| !t.is_empty()) {
            s.set_remote_tag(tag);
        }
        s.status = BroadcastStatus::Active;
        s.update_activity();
        true
    }

    pub async fn start_terminating(&self, call_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(call_id) {
            s.status = BroadcastStatus::Terminating;
        }
    }

    pub async fn terminate(&self, call_id: &str) {
        if let Some(s) = self.sessions.write().await.get_mut(call_id) {
            s.status = BroadcastStatus::Terminated;
        }
    }

    pub async fn remove(&self, call_id: &str) {
        self.sessions.write().await.remove(call_id);
    }

    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// 列出所有 active 会话
    pub async fn list_active(&self) -> Vec<BroadcastSession> {
        self.sessions
            .read()
            .await
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }
}

impl Default for BroadcastManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(call_id: &str, device_id: &str, channel_id: &str) -> BroadcastSession {
        BroadcastSession::new(call_id, device_id, channel_id)
    }

    /// Phase 3.5: 基本的 create / get / remove
    #[tokio::test]
    async fn test_broadcast_manager_lifecycle() {
        let mgr = BroadcastManager::new();
        let s = make_session("call-1", "dev1", "ch1");
        mgr.create(s).await;
        assert_eq!(mgr.count().await, 1);
        let got = mgr.get("call-1").await.unwrap();
        assert_eq!(got.device_id, "dev1");
        assert_eq!(got.status, BroadcastStatus::Pending);
        mgr.remove("call-1").await;
        assert_eq!(mgr.count().await, 0);
    }

    /// BYE 必须是对话内请求：CSeq 严格大于 INVITE 的。
    #[test]
    fn broadcast_bye_cseq_increments_after_invite() {
        let mut s = make_session("bc_x", "dev", "ch");
        assert_eq!(s.bye_cseq(), 2);
        s.set_local_dialog("local-tag", 5);
        assert_eq!(s.bye_cseq(), 6);
        assert_eq!(s.local_tag.as_deref(), Some("local-tag"));
    }

    /// 200 OK 回填对端 To tag（BYE 必须带）；空 tag 不覆盖旧值。
    #[tokio::test]
    async fn broadcast_mark_answered_backfills_remote_tag() {
        let mgr = BroadcastManager::new();
        mgr.create(make_session("bc_1", "dev", "ch")).await;

        assert!(mgr.mark_answered("bc_1", Some("remote-tag")).await);
        let s = mgr.get("bc_1").await.unwrap();
        assert_eq!(s.status, BroadcastStatus::Active);
        assert_eq!(s.remote_tag.as_deref(), Some("remote-tag"));

        assert!(mgr.mark_answered("bc_1", None).await);
        assert_eq!(
            mgr.get("bc_1").await.unwrap().remote_tag.as_deref(),
            Some("remote-tag")
        );
        assert!(!mgr.mark_answered("nope", Some("t")).await);
    }

    /// Phase 3.5: 状态机：Pending → Active → Terminating → Terminated
    #[tokio::test]
    async fn test_broadcast_status_transitions() {
        let mgr = BroadcastManager::new();
        mgr.create(make_session("c1", "d", "ch")).await;

        mgr.activate("c1").await;
        assert_eq!(
            mgr.get("c1").await.unwrap().status,
            BroadcastStatus::Active
        );

        mgr.start_terminating("c1").await;
        assert_eq!(
            mgr.get("c1").await.unwrap().status,
            BroadcastStatus::Terminating
        );

        mgr.terminate("c1").await;
        assert_eq!(
            mgr.get("c1").await.unwrap().status,
            BroadcastStatus::Terminated
        );
    }

    /// Phase 3.5: talk 与 broadcast session 不互相影响（同 device/channel 可同时存在）
    #[tokio::test]
    async fn test_broadcast_independent_of_talk() {
        let mgr = BroadcastManager::new();
        mgr.create(make_session("bc1", "dev1", "ch1")).await;
        mgr.create(make_session("bc2", "dev1", "ch1")).await;
        assert_eq!(mgr.count().await, 2);
        // 移除一个不影响另一个
        mgr.remove("bc1").await;
        assert_eq!(mgr.count().await, 1);
        assert!(mgr.get("bc2").await.is_some());
    }

    /// Phase 3.5: 按 device_id + channel_id 查找
    #[tokio::test]
    async fn test_broadcast_get_by_device_channel() {
        let mgr = BroadcastManager::new();
        mgr.create(make_session("c1", "dev1", "ch1")).await;
        mgr.create(make_session("c2", "dev2", "ch1")).await;

        let s = mgr.get_by_device_channel("dev1", "ch1").await;
        assert!(s.is_some());
        assert_eq!(s.unwrap().call_id, "c1");

        let s = mgr.get_by_device_channel("dev1", "ch_not_exist").await;
        assert!(s.is_none());
    }
}
