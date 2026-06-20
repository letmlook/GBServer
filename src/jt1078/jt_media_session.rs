// ! JtMediaSession — JT1078 实时视频/回放/下载会话管理
//!
//! 三层结构：
//!   Handler → JtCommandWaiter (命令关联) → JtMediaSession (媒体状态)
//!
//! 会话类型：
//! - LiveSession: 实时视频（start → stop → ZLM Hook → 清理）
//! - PlaybackSession: 历史回放（start → control(pause/resume/seek/speed) → stop）
//! - DownloadSession: 文件下载（start → stop + 文件上传通知）

use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};

use dashmap::DashMap;
use tokio::sync::oneshot;

/// Phase 6.3: media arrival waiter — sender-side handle.
/// Inserted into JtMediaSessionManager::waiters; resolved by activate_and_resolve
/// when ZLM on_stream_changed / on_publish hook fires.
pub struct MediaWaiter {
    pub phone: String,
    pub channel_id: u8,
    pub sender: Option<oneshot::Sender<JtMediaSession>>,
}

/// 会话状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSessionState {
    Starting,   // 命令已发，等待终端响应
    Active,     // 媒体流传输中
    Paused,     // 暂停中
    Stopping,    // 停止中
    Stopped,     // 已停止
    Failed,      // 失败
}

/// 会话类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSessionType {
    Live,
    Playback,
    Download,
}

/// JT1078 媒体会话
#[derive(Debug, Clone)]
pub struct JtMediaSession {
    /// 终端号码
    pub phone: String,
    /// 通道号
    pub channel_id: u8,
    /// 会话类型
    pub session_type: MediaSessionType,
    /// 当前状态
    pub state: MediaSessionState,
    /// ZLM stream_id（实时视频场景）
    pub zlm_stream_id: Option<String>,
    /// 媒体流 URL（回放/下载场景）
    pub stream_url: Option<String>,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 当前倍速（回放）
    pub speed: f64,
    /// 当前位置（秒，回放）
    pub current_pos_secs: i64,
    /// 最后活动时间
    pub last_activity: DateTime<Utc>,
}

/// 媒体会话管理器
pub struct JtMediaSessionManager {
    /// phone_channel → session
    sessions: Arc<DashMap<String, JtMediaSession>>,
    /// phone_channel → media arrival waiter (Phase 6.3)
    waiters: Arc<DashMap<String, MediaWaiter>>,
}

impl JtMediaSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            waiters: Arc::new(DashMap::new()),
        }
    }

    /// Test-only helper to clone Arc-backed shared state.
    #[doc(hidden)]
    pub fn clone_for_test(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            waiters: self.waiters.clone(),
        }
    }

    /// Phase 6.3: wait for ZLM media arrival (on_stream_changed/on_publish hook).
    /// Returns the activated session, or error on timeout / cancel.
    pub async fn wait_for_media(
        &self,
        phone: &str,
        channel_id: u8,
        timeout: Duration,
    ) -> Result<JtMediaSession, String> {
        let key = format!("{}_{}", phone, channel_id);
        let (tx, rx) = oneshot::channel();
        self.waiters.insert(
            key.clone(),
            MediaWaiter {
                phone: phone.to_string(),
                channel_id,
                sender: Some(tx),
            },
        );
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(sess)) => Ok(sess),
            Ok(Err(_canceled)) => Err("media waiter cancelled".to_string()),
            Err(_elapsed) => {
                self.waiters.remove(&key);
                Err(format!("media wait timeout for {}_{}", phone, channel_id))
            }
        }
    }

    /// Phase 6.3: resolve the matching waiter when ZLM hook fires.
    /// Returns true if a waiter was found and resolved.
    pub fn resolve_waiter(&self, phone: &str, channel_id: u8, zlm_stream_id: &str) -> bool {
        let key = format!("{}_{}", phone, channel_id);
        if let Some((_, mut w)) = self.waiters.remove(&key) {
            // Update the session state to Active
            self.activate(phone, channel_id, zlm_stream_id);
            if let Some(tx) = w.sender.take() {
                if let Some(sess) = self.get(phone, channel_id) {
                    let _ = tx.send(sess);
                    return true;
                }
            }
        }
        false
    }

    fn session_key(phone: &str, channel_id: u8) -> String {
        format!("{}_{}", phone, channel_id)
    }

    /// 创建实时视频会话
    pub fn create_live(&self, phone: &str, channel_id: u8) {
        let key = Self::session_key(phone, channel_id);
        let now = Utc::now();
        self.sessions.insert(key, JtMediaSession {
            phone: phone.to_string(),
            channel_id,
            session_type: MediaSessionType::Live,
            state: MediaSessionState::Starting,
            zlm_stream_id: None,
            stream_url: None,
            start_time: now,
            end_time: None,
            speed: 1.0,
            current_pos_secs: 0,
            last_activity: now,
        });
    }

    /// 创建回放会话
    pub fn create_playback(&self, phone: &str, channel_id: u8) {
        let key = Self::session_key(phone, channel_id);
        let now = Utc::now();
        self.sessions.insert(key, JtMediaSession {
            phone: phone.to_string(),
            channel_id,
            session_type: MediaSessionType::Playback,
            state: MediaSessionState::Starting,
            zlm_stream_id: None,
            stream_url: None,
            start_time: now,
            end_time: None,
            speed: 1.0,
            current_pos_secs: 0,
            last_activity: now,
        });
    }

    /// 媒体到达，激活会话
    pub fn activate(&self, phone: &str, channel_id: u8, zlm_stream_id: &str) {
        let key = Self::session_key(phone, channel_id);
        if let Some(mut s) = self.sessions.get_mut(&key) {
            s.state = MediaSessionState::Active;
            s.zlm_stream_id = Some(zlm_stream_id.to_string());
            s.last_activity = Utc::now();
        }
    }

    /// 暂停会话
    pub fn pause(&self, phone: &str, channel_id: u8) {
        let key = Self::session_key(phone, channel_id);
        if let Some(mut s) = self.sessions.get_mut(&key) {
            s.state = MediaSessionState::Paused;
            s.last_activity = Utc::now();
        }
    }

    /// 恢复会话
    pub fn resume(&self, phone: &str, channel_id: u8) {
        let key = Self::session_key(phone, channel_id);
        if let Some(mut s) = self.sessions.get_mut(&key) {
            s.state = MediaSessionState::Active;
            s.last_activity = Utc::now();
        }
    }

    /// 停止会话
    pub fn stop(&self, phone: &str, channel_id: u8) {
        let key = Self::session_key(phone, channel_id);
        if let Some(mut s) = self.sessions.get_mut(&key) {
            s.state = MediaSessionState::Stopped;
            s.end_time = Some(Utc::now());
            s.last_activity = Utc::now();
        }
    }

    /// 更新回放位置
    pub fn update_position(&self, phone: &str, channel_id: u8, pos_secs: i64) {
        let key = Self::session_key(phone, channel_id);
        if let Some(mut s) = self.sessions.get_mut(&key) {
            s.current_pos_secs = pos_secs;
            s.last_activity = Utc::now();
        }
    }

    /// 更新回放倍速
    pub fn update_speed(&self, phone: &str, channel_id: u8, speed: f64) {
        let key = Self::session_key(phone, channel_id);
        if let Some(mut s) = self.sessions.get_mut(&key) {
            s.speed = speed;
            s.last_activity = Utc::now();
        }
    }

    /// 获取会话
    pub fn get(&self, phone: &str, channel_id: u8) -> Option<JtMediaSession> {
        let key = Self::session_key(phone, channel_id);
        self.sessions.get(&key).map(|r| r.clone())
    }

    /// 删除会话
    pub fn remove(&self, phone: &str, channel_id: u8) -> Option<JtMediaSession> {
        let key = Self::session_key(phone, channel_id);
        self.sessions.remove(&key).map(|(_, v)| v)
    }

    /// 获取终端活跃会话数
    pub fn count_for_phone(&self, phone: &str) -> usize {
        self.sessions.iter().filter(|r| r.phone == phone).count()
    }

    /// 获取活跃会话数
    pub fn active_count(&self) -> usize {
        self.sessions.iter()
            .filter(|r| r.state == MediaSessionState::Active)
            .count()
    }

    /// 获取指定类型的活跃会话
    pub fn get_by_type(&self, t: MediaSessionType) -> Vec<JtMediaSession> {
        self.sessions.iter()
            .filter(|r| r.session_type == t && r.state == MediaSessionState::Active)
            .map(|r| r.clone())
            .collect()
    }
}

impl Default for JtMediaSessionManager {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_session_lifecycle() {
        let mgr = JtMediaSessionManager::new();
        mgr.create_live("13812340001", 0);
        assert_eq!(mgr.count_for_phone("13812340001"), 1);

        mgr.activate("13812340001", 0, "zlm_live_001");
        assert_eq!(mgr.get("13812340001", 0).unwrap().state, MediaSessionState::Active);

        mgr.stop("13812340001", 0);
        let s = mgr.remove("13812340001", 0).unwrap();
        assert_eq!(s.state, MediaSessionState::Stopped);
        assert!(s.zlm_stream_id.is_some());
    }

    #[test]
    fn test_playback_session_speed() {
        let mgr = JtMediaSessionManager::new();
        mgr.create_playback("13812340001", 0);
        mgr.activate("13812340001", 0, "zlm_pb_001");

        mgr.update_speed("13812340001", 0, 2.0);
        mgr.update_position("13812340001", 0, 3600); // 1 hour in

        let s = mgr.get("13812340001", 0).unwrap();
        assert_eq!(s.speed, 2.0);
        assert_eq!(s.current_pos_secs, 3600);
    }

    #[test]
    fn test_session_by_type() {
        let mgr = JtMediaSessionManager::new();
        mgr.create_live("13812340001", 0);
        mgr.create_playback("13812340001", 1);
        mgr.activate("13812340001", 0, "z1");
        mgr.activate("13812340001", 1, "z2");

        let live = mgr.get_by_type(MediaSessionType::Live);
        let pb = mgr.get_by_type(MediaSessionType::Playback);
        assert_eq!(live.len(), 1);
        assert_eq!(pb.len(), 1);
    }

    #[tokio::test]
    async fn test_wait_for_media_resolves_on_activate() {
        let mgr = JtMediaSessionManager::new();
        mgr.create_live("13812340001", 2);
        let mgr_clone = mgr.clone_for_test();
        let wait_handle = tokio::spawn(async move {
            mgr_clone.wait_for_media("13812340001", 2, std::time::Duration::from_secs(2)).await
        });
        // Give time for waiter to register
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let resolved = mgr.resolve_waiter("13812340001", 2, "jt1078_13812340001_2");
        assert!(resolved);
        let sess = wait_handle.await.unwrap().unwrap();
        assert_eq!(sess.state, MediaSessionState::Active);
        assert_eq!(sess.zlm_stream_id.as_deref(), Some("jt1078_13812340001_2"));
    }

    #[tokio::test]
    async fn test_wait_for_media_timeout() {
        let mgr = JtMediaSessionManager::new();
        mgr.create_live("13812340001", 3);
        let result = mgr.wait_for_media("13812340001", 3, std::time::Duration::from_millis(100)).await;
        assert!(result.is_err());
    }
}
