// ! PlaybackInviteSession — GB28181 回放 INVITE 会话管理
//!
//! 与 InviteSessionManager 分离：InviteSessionManager 管理实时流，
//! PlaybackInviteSessionManager 管理回放/下载 INVITE 会话。
//!
//! 每个回放会话独立于直播会话，支持暂停/恢复/seek/倍速控制。

use std::sync::Arc;

use dashmap::DashMap;
use chrono::{DateTime, Utc};

/// 回放会话状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// 等待设备响应中
    Pending,
    /// INVITE 已发送，等待 200 OK
    Inviting,
    /// 设备已接受，回放中
    Playing,
    /// 暂停中
    Paused,
    /// 等待拖动/seek 响应
    Seeking,
    /// 等待倍速切换响应
    SpeedChanging,
    /// 正在停止
    Stopping,
    /// 已停止
    Stopped,
}

/// GB28181 回放会话
#[derive(Debug, Clone)]
pub struct PlaybackInviteSession {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub stream_id: String,
    pub app: String,
    pub start_time: String,
    pub end_time: String,
    pub current_time: String,
    pub speed: f64,
    pub state: PlaybackState,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub timeout_secs: u64,
}

impl PlaybackInviteSession {
    pub fn new(
        call_id: String,
        device_id: String,
        channel_id: String,
        stream_id: String,
        start_time: String,
        end_time: String,
    ) -> Self {
        Self {
            call_id,
            device_id,
            channel_id,
            stream_id,
            app: "playback".to_string(),
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            current_time: start_time,
            speed: 1.0,
            state: PlaybackState::Pending,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            timeout_secs: 60,
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }

    pub fn set_state(&mut self, state: PlaybackState) {
        self.state = state;
        self.update_activity();
    }

    pub fn set_current_time(&mut self, time: &str) {
        self.current_time = time.to_string();
        self.update_activity();
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed;
        self.update_activity();
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self.state, PlaybackState::Stopped | PlaybackState::Stopping)
    }
}

/// 回放会话管理器
pub struct PlaybackInviteSessionManager {
    sessions: Arc<DashMap<String, PlaybackInviteSession>>,
    /// E1: 可选 StateStore
    state_store: Option<Arc<crate::state_store::StateStore>>,
}

impl PlaybackInviteSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            state_store: None,
        }
    }

    /// E1: 注入 StateStore
    pub fn set_state_store(&mut self, store: Arc<crate::state_store::StateStore>) {
        self.state_store = Some(store);
    }

    /// E1: 同步当前会话状态到 StateStore
    fn sync_to_store(&self, call_id: &str, s: &PlaybackInviteSession) {
        if let Some(ref store) = self.state_store {
            store.set_invite_session(call_id, crate::state_store::InviteSessionState {
                call_id: s.call_id.clone(),
                device_id: s.device_id.clone(),
                channel_id: s.channel_id.clone(),
                session_type: "playback".to_string(),
                zlm_stream_id: Some(s.stream_id.clone()),
                status: format!("{:?}", s.state),
                created_at: s.created_at,
                last_activity: s.last_activity,
            });
        }
    }

    /// E1: 获取活跃会话数（含 StateStore 中的）
    pub fn active_count(&self) -> usize {
        let local = self.sessions.iter().filter(|r| r.state == PlaybackState::Playing).count();
        local
    }

    /// 创建新会话
    pub fn create(&self, session: PlaybackInviteSession) -> String {
        let call_id = session.call_id.clone();
        // E1: 同步到 StateStore
        if let Some(ref store) = self.state_store {
            store.set_invite_session(&call_id, crate::state_store::InviteSessionState {
                call_id: session.call_id.clone(),
                device_id: session.device_id.clone(),
                channel_id: session.channel_id.clone(),
                session_type: "playback".to_string(),
                zlm_stream_id: Some(session.stream_id.clone()),
                status: format!("{:?}", session.state),
                created_at: session.created_at,
                last_activity: session.last_activity,
            });
        }
        self.sessions.insert(call_id.clone(), session);
        call_id
    }

    /// 获取会话
    pub fn get(&self, call_id: &str) -> Option<PlaybackInviteSession> {
        self.sessions.get(call_id).map(|r| r.clone())
    }

    /// 按 stream_id 获取会话
    pub fn get_by_stream(&self, stream_id: &str) -> Option<PlaybackInviteSession> {
        self.sessions
            .iter()
            .find(|r| r.stream_id == stream_id)
            .map(|r| r.clone())
    }

    /// 更新会话
    pub fn update(&self, session: &PlaybackInviteSession) {
        self.sessions.insert(session.call_id.clone(), session.clone());
    }

    /// 删除会话
    pub fn remove(&self, call_id: &str) -> Option<PlaybackInviteSession> {
        self.sessions.remove(call_id).map(|(_, v)| v)
    }

    /// 激活会话（收到 200 OK）
    pub fn activate(&self, call_id: &str) {
        if let Some(mut s) = self.sessions.get_mut(call_id) {
            s.set_state(PlaybackState::Playing);
            self.sync_to_store(call_id, &s);
        }
    }

    /// 暂停会话
    pub fn pause(&self, call_id: &str) {
        if let Some(mut s) = self.sessions.get_mut(call_id) {
            s.set_state(PlaybackState::Paused);
            self.sync_to_store(call_id, &s);
        }
    }

    /// 恢复会话
    pub fn resume(&self, call_id: &str) {
        if let Some(mut s) = self.sessions.get_mut(call_id) {
            s.set_state(PlaybackState::Playing);
            self.sync_to_store(call_id, &s);
        }
    }

    /// 停止会话
    pub fn stop(&self, call_id: &str) {
        if let Some(mut s) = self.sessions.get_mut(call_id) {
            s.set_state(PlaybackState::Stopped);
            // E1: 同步 StateStore
            if let Some(ref store) = self.state_store {
                store.remove_invite_session(call_id);
            }
        }
    }

    /// 删除会话（purge 路径）
    pub fn remove_with_state_cleanup(&self, call_id: &str) -> Option<PlaybackInviteSession> {
        if let Some(ref store) = self.state_store {
            store.remove_invite_session(call_id);
        }
        self.sessions.remove(call_id).map(|(_, v)| v)
    }

    /// 更新当前播放时间
    pub fn update_time(&self, call_id: &str, time: &str) {
        if let Some(mut s) = self.sessions.get_mut(call_id) {
            s.set_current_time(time);
        }
    }

    /// 更新倍速
    pub fn update_speed(&self, call_id: &str, speed: f64) {
        if let Some(mut s) = self.sessions.get_mut(call_id) {
            s.set_speed(speed);
        }
    }

    /// 清理所有已停止的会话
    pub fn purge(&self) -> Vec<String> {
        let snap: Vec<_> = self.sessions
            .iter()
            .filter(|r| r.is_resolved())
            .map(|r| r.key().clone())
            .collect();

        let mut removed = Vec::new();
        for key in snap {
            self.sessions.remove(&key);
            // E1: 同步 StateStore 清理
            if let Some(ref store) = self.state_store {
                store.remove_invite_session(&key);
            }
            removed.push(key);
        }
        removed
    }

    /// 总会话数
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for PlaybackInviteSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_session_lifecycle() {
        let mgr = PlaybackInviteSessionManager::new();
        let call_id = mgr.create(PlaybackInviteSession::new(
            "call-pb-001".to_string(),
            "34020000001320000001".to_string(),
            "34020000001320000001".to_string(),
            "stream-pb-001".to_string(),
            "2026-01-01T00:00:00".to_string(),
            "2026-01-01T01:00:00".to_string(),
        ));

        assert_eq!(mgr.count(), 1);
        assert!(mgr.get(&call_id).is_some());

        mgr.activate(&call_id);
        assert_eq!(mgr.get(&call_id).unwrap().state, PlaybackState::Playing);

        mgr.pause(&call_id);
        assert_eq!(mgr.get(&call_id).unwrap().state, PlaybackState::Paused);

        mgr.resume(&call_id);
        assert_eq!(mgr.get(&call_id).unwrap().state, PlaybackState::Playing);

        mgr.stop(&call_id);
        assert_eq!(mgr.get(&call_id).unwrap().state, PlaybackState::Stopped);

        let removed = mgr.purge();
        assert_eq!(removed.len(), 1);
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_playback_session_speed() {
        let mgr = PlaybackInviteSessionManager::new();
        let call_id = mgr.create(PlaybackInviteSession::new(
            "call-pb-002".to_string(),
            "34020000001320000001".to_string(),
            "34020000001320000001".to_string(),
            "stream-pb-002".to_string(),
            "2026-01-01T00:00:00".to_string(),
            "2026-01-01T01:00:00".to_string(),
        ));

        mgr.update_speed(&call_id, 2.0);
        assert_eq!(mgr.get(&call_id).unwrap().speed, 2.0);

        mgr.update_speed(&call_id, 0.5);
        assert_eq!(mgr.get(&call_id).unwrap().speed, 0.5);
    }
}
