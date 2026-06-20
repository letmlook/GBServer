//! Phase 4.5: 流状态统一接口
//!
//! `gb_stream_push` / `gb_stream_proxy` / (未来 `gb_send_rtp`) 共用
//! `StreamStatus` 枚举 + `StreamState` trait，上层 handler 无需 switch table。
//!
//! - 新增 `status` 文本列（`'ready' | 'pushing' | 'active' | 'stopped' | 'failed'`）
//!   与历史 `pushing` / `pulling` bool 字段**并存**，旧 API 完全兼容。
//! - `is_active()`: 推送/拉取中 (`Pushing` / `Active`)，便于上层做"在线流"过滤。

use serde::{Deserialize, Serialize};

/// 流生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    /// 流已注册但未推送
    Ready,
    /// 正在推送/拉取（外部推流 + GB28181 拉流）
    Pushing,
    /// 拉流/推流中（代理 / SendRtp）
    Active,
    /// 流结束/超时
    Stopped,
    /// 推流失败
    Failed,
}

impl StreamStatus {
    /// 是否处于活跃状态（正在推送/拉取/代理）
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pushing | Self::Active)
    }

    /// 序列化到数据库列的字符串（lowercase，与 `serde(rename_all)` 一致）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Pushing => "pushing",
            Self::Active => "active",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

impl Default for StreamStatus {
    fn default() -> Self {
        Self::Ready
    }
}

impl std::str::FromStr for StreamStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "ready" => Ok(Self::Ready),
            "pushing" => Ok(Self::Pushing),
            "active" => Ok(Self::Active),
            "stopped" => Ok(Self::Stopped),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown StreamStatus: {}", other)),
        }
    }
}

/// 上层查询统一抽象：3 个流表共用此 trait。
/// `set_status` 需要 `&mut self`；trait 对象调用者需内部可变性。
pub trait StreamState: Send + Sync {
    /// 流唯一标识（业务上 = app/stream）
    fn stream_id(&self) -> &str;

    /// 应用名（live / playback / ...）
    fn app(&self) -> &str;

    /// 当前流状态
    fn status(&self) -> StreamStatus;

    /// 设置流状态（handler / 后台 loop 用）
    fn set_status(&mut self, status: StreamStatus);

    /// 所属媒体服务器 ID（auto / 节点 ID）
    fn media_server_id(&self) -> Option<&str>;

    /// 关联的 GB28181 设备 ID（SendRtp / 部分 stream_push 关联）
    fn device_id(&self) -> Option<&str>;

    /// 关联的通道 ID
    fn channel_id(&self) -> Option<&str>;
}

// ====================================================================
// 单元测试（6 个）
// ====================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_status_is_active() {
        // Pushing & Active -> true
        assert!(StreamStatus::Pushing.is_active());
        assert!(StreamStatus::Active.is_active());
        // Ready / Stopped / Failed -> false
        assert!(!StreamStatus::Ready.is_active());
        assert!(!StreamStatus::Stopped.is_active());
        assert!(!StreamStatus::Failed.is_active());
    }

    #[test]
    fn test_stream_status_as_str() {
        assert_eq!(StreamStatus::Ready.as_str(), "ready");
        assert_eq!(StreamStatus::Pushing.as_str(), "pushing");
        assert_eq!(StreamStatus::Active.as_str(), "active");
        assert_eq!(StreamStatus::Stopped.as_str(), "stopped");
        assert_eq!(StreamStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn test_stream_status_serde_lowercase() {
        // serde rename_all = "lowercase" → JSON 序列化用小写字符串
        let json = serde_json::to_string(&StreamStatus::Ready).unwrap();
        assert_eq!(json, "\"ready\"");
        let json = serde_json::to_string(&StreamStatus::Pushing).unwrap();
        assert_eq!(json, "\"pushing\"");
        let json = serde_json::to_string(&StreamStatus::Failed).unwrap();
        assert_eq!(json, "\"failed\"");
        // 反序列化同样支持
        let back: StreamStatus = serde_json::from_str("\"active\"").unwrap();
        assert_eq!(back, StreamStatus::Active);
    }

    #[test]
    fn test_stream_status_default() {
        assert_eq!(StreamStatus::default(), StreamStatus::Ready);
    }

    #[test]
    fn test_stream_status_from_str() {
        assert_eq!("ready".parse::<StreamStatus>().unwrap(), StreamStatus::Ready);
        assert_eq!("PUSHING".parse::<StreamStatus>().unwrap(), StreamStatus::Pushing);
        assert_eq!("Active".parse::<StreamStatus>().unwrap(), StreamStatus::Active);
        assert!("bogus".parse::<StreamStatus>().is_err());
    }

    #[test]
    fn test_stream_state_trait_object() {
        // 验证 trait 可作对象使用 + set_status 写回字段
        struct MockState {
            id: String,
            app: String,
            st: StreamStatus,
            ms: Option<String>,
            dev: Option<String>,
            ch: Option<String>,
        }
        impl StreamState for MockState {
            fn stream_id(&self) -> &str { &self.id }
            fn app(&self) -> &str { &self.app }
            fn status(&self) -> StreamStatus { self.st }
            fn set_status(&mut self, status: StreamStatus) { self.st = status; }
            fn media_server_id(&self) -> Option<&str> { self.ms.as_deref() }
            fn device_id(&self) -> Option<&str> { self.dev.as_deref() }
            fn channel_id(&self) -> Option<&str> { self.ch.as_deref() }
        }

        let mut m = MockState {
            id: "live/test".into(),
            app: "live".into(),
            st: StreamStatus::Ready,
            ms: Some("zlm-1".into()),
            dev: Some("34020000001320000001".into()),
            ch: Some("34020000001320000002".into()),
        };
        // 通过 Box<dyn StreamState> 调用
        let mut boxed: Box<dyn StreamState> = Box::new(m);
        assert_eq!(boxed.stream_id(), "live/test");
        assert_eq!(boxed.app(), "live");
        assert_eq!(boxed.status(), StreamStatus::Ready);
        assert!(!boxed.status().is_active());
        boxed.set_status(StreamStatus::Pushing);
        assert!(boxed.status().is_active());
        assert_eq!(boxed.media_server_id(), Some("zlm-1"));
        assert_eq!(boxed.device_id(), Some("34020000001320000001"));
        assert_eq!(boxed.channel_id(), Some("34020000001320000002"));
    }
}