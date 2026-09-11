//! 把 `tracing` 事件采集到内存队列，供后台任务落库（`gb_log`）。
//!
//! ## 为什么这样设计
//!
//! - **不在 `on_event` 里做 IO**：该回调是同步的、处在日志热路径上。
//!   这里只做一次 `try_lock` + 入队。
//! - **有界队列**：队列满时丢弃最旧的一条并计数，避免日志风暴拖垮进程。
//! - **不阻塞、不 panic**：`try_lock` 失败（例如落库任务正好持锁）直接放弃本次采集；
//!   任何异常都不应影响业务。
//! - **不会递归**：落库任务自身出错时用 `eprintln!`（标准错误）而非 `tracing`，
//!   因此不会再次进入本层。
//!
//! 采集到的条目由 `lib.rs` 中的后台任务每秒 `drain()` 一次并批量写入 `gb_log`。

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use crate::db::log::NewLogEntry;

/// 内存队列上限。超出后丢弃最旧条目（并累加 `dropped()`）。
pub const MAX_BUFFER: usize = 8192;

static BUFFER: OnceLock<Mutex<VecDeque<NewLogEntry>>> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);

fn buffer() -> &'static Mutex<VecDeque<NewLogEntry>> {
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_BUFFER)))
}

/// 取走当前缓冲区中的全部日志（后台落库任务调用）。
pub fn drain() -> Vec<NewLogEntry> {
    match buffer().try_lock() {
        Ok(mut buf) => buf.drain(..).collect(),
        // 正好被采集线程持有锁：本轮跳过，下轮再取
        Err(_) => Vec::new(),
    }
}

/// 因队列满而被丢弃的日志条数（可观测性用）。
pub fn dropped_count() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// 把一个事件放入队列（供测试直接调用）。
pub fn enqueue(entry: NewLogEntry) {
    if let Ok(mut buf) = buffer().try_lock() {
        if buf.len() >= MAX_BUFFER {
            buf.pop_front();
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
        buf.push_back(entry);
    }
}

/// 采集 `message` 字段
#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            // tracing 以 Debug 形式传入 format_args，外层会带引号
            let s = format!("{:?}", value);
            self.message = Some(s.trim_matches('"').to_string());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

/// 把 tracing 事件采集进内存队列的 Layer。
///
/// 注册方式（见 `main.rs`）：
/// ```ignore
/// tracing_subscriber::registry()
///     .with(EnvFilter::new(...))
///     .with(tracing_subscriber::fmt::layer())
///     .with(gbserver::logging::CaptureLayer)
///     .init();
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct CaptureLayer;

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let thread = std::thread::current()
            .name()
            .map(str::to_string)
            .unwrap_or_else(|| "unnamed".to_string());

        enqueue(NewLogEntry {
            time: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level: metadata.level().to_string(),
            logger: Some(metadata.target().to_string()),
            thread: Some(thread),
            message: visitor.message,
            source: Some(format!("{}:{}", metadata.file().unwrap_or(""), metadata.line().unwrap_or(0))),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    /// 本模块的测试共享同一个全局缓冲，必须串行执行，否则会互相 `drain()`。
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_enqueue_and_drain() {
        let _guard = lock();
        let _ = drain(); // 清空可能的历史
        enqueue(NewLogEntry {
            time: "2026-01-01 00:00:00".into(),
            level: "INFO".into(),
            logger: Some("t".into()),
            thread: None,
            message: Some("hello".into()),
            source: None,
        });
        let got = drain();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].message.as_deref(), Some("hello"));
        // drain 之后应为空
        assert!(drain().is_empty());
    }

    /// 队列必须有界：超过上限时丢弃最旧条目，而不是无限增长。
    #[test]
    fn test_buffer_is_bounded() {
        let _guard = lock();
        let _ = drain();
        let before = dropped_count();
        for i in 0..(MAX_BUFFER + 50) {
            enqueue(NewLogEntry {
                time: "2026-01-01 00:00:00".into(),
                level: "INFO".into(),
                logger: None,
                thread: None,
                message: Some(format!("m{}", i)),
                source: None,
            });
        }
        let got = drain();
        assert!(got.len() <= MAX_BUFFER, "队列不得超出上限");
        assert!(
            dropped_count() > before,
            "超出上限的条目应被计入丢弃数"
        );
    }

    /// CaptureLayer 真的能捕获 tracing 事件（验证 layer 接线有效）。
    #[test]
    fn test_capture_layer_captures_events() {
        let _guard = lock();
        let _ = drain();
        let subscriber = tracing_subscriber::registry().with(CaptureLayer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "gbserver::logging::test", device = "34020000001320000001", "注册失败");
        });
        let got = drain();
        assert_eq!(got.len(), 1, "应捕获到 1 条事件");
        assert_eq!(got[0].level, "WARN");
        assert_eq!(got[0].logger.as_deref(), Some("gbserver::logging::test"));
        assert!(
            got[0].message.as_deref().unwrap_or("").contains("注册失败"),
            "message 应被提取: {:?}",
            got[0].message
        );
    }
}
