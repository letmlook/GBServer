// ! MediaWaiter — ZLM 媒体到达等待器
//!
//! play.rs 的 send_play_invite_and_wait() 在 SIP 200 OK 后调用 await_media()，
//! 等待 ZLM Hook 确认媒体流已到达（on_stream_started / on_rtp_server_started）。
//!
//! 流程：
//!   HTTP handler (play_start)
//!     → open ZLM RTP server
//!     → send_play_invite_and_wait()
//!         → 发送 SIP INVITE，等待 SIP 200 OK
//!         → await_media() — 等待 ZLM Hook 触发 resolve
//!     → 收到媒体流 ID，返回播放地址
//!
//!   ZLM Hook 收到流到达事件
//!     → notify_media_ready(call_id, stream_id) — 唤醒等待中的任务

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::oneshot;

/// 媒体流等待结果
#[derive(Debug)]
pub enum MediaWaitResult {
    /// 媒体流到达，返回 ZLM stream_id
    MediaReady { zlm_stream_id: String, app: String },
    /// 超时
    Timeout,
    /// 会话不存在或已被清理
    SessionNotFound,
}

/// 单个等待器元数据
#[derive(Debug, Clone)]
pub struct MediaWaiter {
    pub call_id: String,
    pub zlm_stream_id: String,
    /// 完整 waiter key，格式 `{call_id}:{app}:{stream_id}`；
    /// 由 register() 通过 with_app() 写入，cleanup_expired() 复用，
    /// 保证 active_keys 与 receivers 用同一 key 清理。
    pub waiter_key: String,
    pub created_at: Instant,
    pub timeout_secs: u64,
}

impl MediaWaiter {
    pub fn new(call_id: String, zlm_stream_id: String, timeout_secs: u64) -> Self {
        Self {
            call_id,
            zlm_stream_id,
            waiter_key: String::new(),
            created_at: Instant::now(),
            timeout_secs,
        }
    }

    /// builder：补全 `waiter_key`，让 cleanup_expired 能与 register 写入的 key 对齐
    pub fn with_app(mut self, app: &str) -> Self {
        self.waiter_key = format!("{}:{}:{}", self.call_id, app, self.zlm_stream_id);
        self
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(self.timeout_secs)
    }
}

/// 媒体到达等待管理器
///
/// 用法：
/// ```ignore
/// // 在 send_play_invite_and_wait() 中：
/// let waiter_key = format!("{}_{}", call_id, stream_id);
/// let waiter = mgr.register_waiter(call_id, stream_id, 15).await;
///
/// // 发送 SIP INVITE，等待 200 OK...
///
/// // 等待 ZLM Hook 触发
/// match waiter.await_media(15).await {
///     Ok(MediaWaitResult::MediaReady { zlm_stream_id, app }) => { ... }
///     Ok(MediaWaitResult::Timeout) => { return Err("ZLM media timeout".into()); }
///     _ => { ... }
/// }
/// ```
pub struct MediaWaiterManager {
    /// 按 call_id 索引的等待器（用于 ZLM Hook 通过 call_id 找到等待者）
    by_call_id: Arc<DashMap<String, MediaWaiter>>,
    /// 按 stream_id 索引的等待器（用于 ZLM Hook 通过 stream_id 找到等待者）
    by_stream_id: Arc<DashMap<String, MediaWaiter>>,
    /// 等待者注册后创建 oneshot receiver
    receivers: Arc<DashMap<String, oneshot::Sender<MediaWaitResult>>>,
    /// 活跃的等待者（用于并发安全地删除）
    active_keys: Arc<DashMap<String, ()>>,
}

impl MediaWaiterManager {
    pub fn new() -> Self {
        Self {
            by_call_id: Arc::new(DashMap::new()),
            by_stream_id: Arc::new(DashMap::new()),
            receivers: Arc::new(DashMap::new()),
            active_keys: Arc::new(DashMap::new()),
        }
    }

    /// 注册一个媒体等待器，同时创建 oneshot channel
    /// 返回 (waiter_key, oneshot::Receiver)
    pub fn register(
        &self,
        call_id: &str,
        stream_id: &str,
        app: &str,
        timeout_secs: u64,
    ) -> (String, tokio::sync::oneshot::Receiver<MediaWaitResult>) {
        let waiter = MediaWaiter::new(call_id.to_string(), stream_id.to_string(), timeout_secs);
        // builder 模式：补上 waiter_key，让 cleanup_expired 与 register 用同一 key
        let waiter = waiter.with_app(app);
        let waiter_key = format!("{}:{}:{}", call_id, app, stream_id);

        let (tx, rx) = oneshot::channel();
        self.receivers.insert(waiter_key.clone(), tx);
        self.by_call_id.insert(call_id.to_string(), waiter.clone());
        self.by_stream_id.insert(stream_id.to_string(), waiter);
        self.active_keys.insert(waiter_key.clone(), ());

        (waiter_key, rx)
    }

    /// 按 call_id + stream_id 完成等待（ZLM Hook 调用）
    pub fn resolve(&self, call_id: &str, stream_id: &str, app: &str) -> bool {
        let waiter_key = format!("{}:{}:{}", call_id, app, stream_id);

        if let Some((_, tx)) = self.receivers.remove(&waiter_key) {
            let _ = tx.send(MediaWaitResult::MediaReady {
                zlm_stream_id: stream_id.to_string(),
                app: app.to_string(),
            });
            self.by_call_id.remove(call_id);
            self.by_stream_id.remove(stream_id);
            self.active_keys.remove(&waiter_key);
            return true;
        }
        false
    }

    /// 按 stream_id 完成等待（ZLM Hook 的 on_rtp_server_started）
    pub fn resolve_by_stream(&self, stream_id: &str, app: &str) -> bool {
        // 先 clone 必要字段，drop 读锁后再调 resolve，避免在持锁时调 remove 死锁
        let call_id_opt: Option<String> =
            self.by_stream_id.get(stream_id).map(|w| w.call_id.clone());
        if let Some(call_id) = call_id_opt {
            return self.resolve(&call_id, stream_id, app);
        }
        false
    }

    /// 清理已超时的等待器
    pub fn cleanup_expired(&self) -> Vec<String> {
        let mut removed = Vec::new();

        let snap: Vec<_> = self
            .by_call_id
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        for (call_id, waiter) in snap {
            if waiter.is_expired() {
                // 用 waiter 自带的完整 key，与 register 写入 active_keys 的 key 一致
                let waiter_key = waiter.waiter_key.clone();
                self.receivers.remove(&waiter_key);
                self.by_call_id.remove(&call_id);
                self.by_stream_id.remove(&waiter.zlm_stream_id);
                self.active_keys.remove(&waiter_key);
                removed.push(call_id);
            }
        }

        removed
    }

    /// 获取活跃等待者数量
    pub fn active_count(&self) -> usize {
        self.active_keys.len()
    }
}

impl Default for MediaWaiterManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_media_waiter_register_and_resolve() {
        let mgr = MediaWaiterManager::new();
        let (_, rx) = mgr.register("call-001", "stream-abc", "rtp", 10);

        assert_eq!(mgr.active_count(), 1);

        // ZLM Hook 触发媒体到达
        let resolved = mgr.resolve("call-001", "stream-abc", "rtp");
        assert!(resolved);
        assert_eq!(mgr.active_count(), 0);

        // 等待者已关闭，rx 应收到结果
        let result = rx.await;
        match result {
            Ok(MediaWaitResult::MediaReady { zlm_stream_id, app }) => {
                assert_eq!(zlm_stream_id, "stream-abc");
                assert_eq!(app, "rtp");
            }
            _ => panic!("Expected MediaReady"),
        }
    }

    #[tokio::test]
    async fn test_media_waiter_resolve_by_stream() {
        let mgr = MediaWaiterManager::new();
        mgr.register("call-002", "stream-xyz", "rtp", 10);

        // 通过 stream_id 解析
        let resolved = mgr.resolve_by_stream("stream-xyz", "rtp");
        assert!(resolved);
    }

    #[tokio::test]
    async fn test_media_waiter_timeout_cleanup() {
        // 超时设置为 0 秒，立即过期
        let mgr = MediaWaiterManager::new();
        mgr.register("call-003", "stream-exp", "rtp", 0);
        assert_eq!(mgr.active_count(), 1);

        std::thread::sleep(Duration::from_millis(10));
        let removed = mgr.cleanup_expired();
        assert_eq!(removed.len(), 1);
        assert_eq!(mgr.active_count(), 0);
    }

    #[tokio::test]
    async fn test_media_waiter_unresolved() {
        let mgr = MediaWaiterManager::new();
        mgr.register("call-004", "stream-unres", "rtp", 10);

        // 未注册的 stream 不报错
        let resolved = mgr.resolve("unknown-call", "unknown-stream", "rtp");
        assert!(!resolved);
        assert_eq!(mgr.active_count(), 1);
    }
}
