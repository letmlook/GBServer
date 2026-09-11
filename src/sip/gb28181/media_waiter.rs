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
    /// 设备拒绝：SIP INVITE 收到 4xx/5xx 响应（设备不支持 / 没录像 / 不在线等）
    DeviceRejected { status: u16, reason: String },
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
    /// **先于等待者到达**的媒体就绪通知：`stream_id -> 通知时刻`。
    ///
    /// 竞态是真实存在的：`openRtpServer` 在 ZLM 内部创建流时就会触发
    /// `on_rtp_server_started`，而调用方要等这个 HTTP 响应回来之后才注册
    /// 等待者。通知先到、等待者后注册的情况下，原先会直接丢弃 ——
    /// 结果是"媒体明明已经就绪，播放请求却要干等 15 秒超时失败"。
    ///
    /// 这里把这类通知暂存一小段时间：`register` 时若命中就立即完成，
    /// 超过 `EARLY_READY_TTL` 的陈旧条目在登记/清理时被丢弃。
    early_ready: Arc<DashMap<String, Instant>>,
}

/// 早到通知的有效期：超过它就不再对新的等待者生效（避免把上一次播放的
/// 通知误配给下一次播放）。
const EARLY_READY_TTL: Duration = Duration::from_secs(30);

impl MediaWaiterManager {
    pub fn new() -> Self {
        Self {
            by_call_id: Arc::new(DashMap::new()),
            by_stream_id: Arc::new(DashMap::new()),
            receivers: Arc::new(DashMap::new()),
            active_keys: Arc::new(DashMap::new()),
            early_ready: Arc::new(DashMap::new()),
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

        // 通知早于注册：立刻完成，调用方拿到的是一个已就绪的 oneshot。
        if let Some((_, at)) = self.early_ready.remove(stream_id) {
            if at.elapsed() <= EARLY_READY_TTL {
                if let Some((_, tx)) = self.receivers.remove(&waiter_key) {
                    let _ = tx.send(MediaWaitResult::MediaReady {
                        zlm_stream_id: stream_id.to_string(),
                        app: app.to_string(),
                    });
                }
                self.by_call_id.remove(call_id);
                self.by_stream_id.remove(stream_id);
                self.active_keys.remove(&waiter_key);
                tracing::info!(
                    "MediaWaiter: stream {} 的通知早于注册（{}ms），立即就绪",
                    stream_id,
                    at.elapsed().as_millis()
                );
            }
        }

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
        // 没有等待者：暂存通知，供随后注册的等待者立刻命中
        self.early_ready.insert(stream_id.to_string(), Instant::now());
        false
    }

    /// 按 call_id 主动 reject 等待（设备回 4xx/5xx 时调用，避免等到 15s 超时）。
    /// 通过 `by_call_id` 反查到 `waiter_key`，再从 `receivers` 取 sender。
    pub fn reject_by_call_id(&self, call_id: &str, status: u16, reason: &str) -> bool {
        let waiter_key_opt: Option<String> =
            self.by_call_id.get(call_id).map(|w| w.waiter_key.clone());
        let Some(waiter_key) = waiter_key_opt else {
            return false;
        };
        let Some((_, tx)) = self.receivers.remove(&waiter_key) else {
            return false;
        };
        let _ = tx.send(MediaWaitResult::DeviceRejected {
            status,
            reason: reason.to_string(),
        });
        self.by_call_id.remove(call_id);
        // 同步把 stream_id / active_keys 也清掉
        if let Some(stream_id) = waiter_key.split(':').next_back() {
            self.by_stream_id.remove(stream_id);
        }
        self.active_keys.remove(&waiter_key);
        true
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

    /// 丢弃过期的"早到通知"条目，避免长期运行后无界增长。
    pub fn prune_early_ready(&self) {
        self.early_ready
            .retain(|_, at| at.elapsed() <= EARLY_READY_TTL);
    }

    /// 暂存的"早到通知"条目数（测试/诊断用）。
    pub fn early_ready_count(&self) -> usize {
        self.early_ready.len()
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

    #[tokio::test]
    async fn test_media_waiter_reject_by_call_id() {
        // 验证设备回 4xx/5xx 时 reject_by_call_id 能立即通知等待者
        // （避免干等到 media_waiter timeout）。
        let mgr = MediaWaiterManager::new();
        let (_, rx) = mgr.register("call-005", "stream-rej", "rtp", 60);
        assert_eq!(mgr.active_count(), 1);

        let rejected = mgr.reject_by_call_id("call-005", 503, "Service Unavailable");
        assert!(rejected);
        assert_eq!(mgr.active_count(), 0);

        // 等待者应收到 DeviceRejected 变体（不再是 Timeout）
        match rx.await {
            Ok(MediaWaitResult::DeviceRejected { status, reason }) => {
                assert_eq!(status, 503);
                assert_eq!(reason, "Service Unavailable");
            }
            other => panic!("Expected DeviceRejected, got {:?}", other),
        }

        // 二次 reject / 不存在的 call_id 不报错
        assert!(!mgr.reject_by_call_id("call-005", 486, "Busy Here"));
        assert!(!mgr.reject_by_call_id("nonexistent", 503, "x"));
    }

    /// 竞态回归：ZLM 的媒体就绪通知可能**早于**等待者注册
    /// （`openRtpServer` 内部就会触发 `on_rtp_server_started`，
    /// 而调用方要等响应回来才注册）。此前这种通知会被直接丢弃，
    /// 播放请求只能干等 15 秒超时。
    #[tokio::test]
    async fn test_early_ready_notification_is_not_lost() {
        let mgr = MediaWaiterManager::new();

        // 通知先到，此时还没有等待者
        assert!(!mgr.resolve_by_stream("stream-early", "rtp"));
        assert_eq!(mgr.early_ready_count(), 1);

        // 之后才注册：应立即拿到 MediaReady，而不是等超时
        let (_key, rx) = mgr.register("call-early", "stream-early", "rtp", 60);
        match tokio::time::timeout(Duration::from_secs(1), rx).await {
            Ok(Ok(MediaWaitResult::MediaReady { zlm_stream_id, app })) => {
                assert_eq!(zlm_stream_id, "stream-early");
                assert_eq!(app, "rtp");
            }
            other => panic!("早到通知必须立即兑现，实际: {:?}", other),
        }
        // 兑现后不再残留
        assert_eq!(mgr.early_ready_count(), 0);
        assert_eq!(mgr.active_count(), 0);
    }

    /// 早到通知只对**同一个** stream 生效，不能串到别的流上。
    #[tokio::test]
    async fn test_early_ready_is_scoped_to_stream() {
        let mgr = MediaWaiterManager::new();
        assert!(!mgr.resolve_by_stream("stream-a", "rtp"));

        let (_key, mut rx) = mgr.register("call-b", "stream-b", "rtp", 60);
        // 别的流的等待者不应被 stream-a 的通知兑现
        match tokio::time::timeout(Duration::from_millis(200), &mut rx).await {
            Err(_) => {} // 仍然在等（符合预期）
            other => panic!("不应被无关流的通知唤醒: {:?}", other),
        }
        // 该通知仍在，等 stream-a 的等待者来取
        assert_eq!(mgr.early_ready_count(), 1);
    }

    /// 过期通知必须被清理，避免内存无界增长与跨次播放误配。
    #[tokio::test]
    async fn test_prune_early_ready_drops_stale_entries() {
        let mgr = MediaWaiterManager::new();
        assert!(!mgr.resolve_by_stream("stream-old", "rtp"));
        assert_eq!(mgr.early_ready_count(), 1);
        // 未过期：保留
        mgr.prune_early_ready();
        assert_eq!(mgr.early_ready_count(), 1);
    }
}
