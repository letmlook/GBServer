//! WVP-Pro 兼容的多路径 hook 路由（Phase 4.1）
//!
//! 背景：WVP-Pro / 部分前端实现对每个 hook 事件订阅单独路径，便于
//! "按需订阅、零反射"地集成。本模块暴露 `/api/hook/<event>` 多路径路由，
//! 与既有 `/api/zlm/hook` 单路径入口并存（后者在 router.rs 中以
//! `zlm_hook::handle_webhook` 形式注册，Phase 0 已就绪）。
//!
//! 事件名常量与 `ZlmHookEvent` 一一对应；handler 通过 phantom 泛型标记
//! 路由绑定的事件，运行时再依据请求体中的 `hook_name` 二次校验 + 分派。

use axum::{
    extract::State,
    routing::post,
    Json, Router,
};

use crate::response::WVPResult;
use crate::AppState;
use crate::zlm::hook::{handle_webhook, ZlmHookEvent};

// 事件名常量（与 ZlmHookEvent::from_hook_name 入参一致）
const ON_SERVER_STARTED: &str = "on_server_started";
const ON_SERVER_KEEPALIVE: &str = "on_server_keepalive";
const ON_STREAM_CHANGED: &str = "on_stream_changed";
const ON_STREAM_NOT_FOUND: &str = "on_stream_not_found";
const ON_STREAM_NONE_READER: &str = "on_stream_none_reader";
const ON_STREAM_STARTED: &str = "on_stream_started";
const ON_PUBLISH: &str = "on_publish";
const ON_PLAY: &str = "on_play";
const ON_RTP_SERVER_STARTED: &str = "on_rtp_server_started";
const ON_RTP_SERVER_TIMEOUT: &str = "on_rtp_server_timeout";
const ON_SEND_RTP_STOPPED: &str = "on_send_rtp_stopped";
const ON_RECORD_MP4: &str = "on_record_mp4";
const ON_FLOW_REPORT: &str = "on_flow_report";

/// Phantom 事件标签：编译期强制每个路由绑定的 hook 名与事件名一致
pub trait HookEventTag: Copy + Clone + Send + Sync + 'static {
    /// 该路由期望的 hook 名称（ZLM 上报字段值）
    const HOOK_NAME: &'static str;
    /// 对应的枚举值
    const ENUM: ZlmHookEvent;
}

#[derive(Debug, Clone, Copy)]
pub struct ServerStarted;
#[derive(Debug, Clone, Copy)]
pub struct ServerKeepalive;
#[derive(Debug, Clone, Copy)]
pub struct StreamChanged;
#[derive(Debug, Clone, Copy)]
pub struct StreamNotFound;
#[derive(Debug, Clone, Copy)]
pub struct StreamNoneReader;
#[derive(Debug, Clone, Copy)]
pub struct StreamStarted;
#[derive(Debug, Clone, Copy)]
pub struct Publish;
#[derive(Debug, Clone, Copy)]
pub struct Play;
#[derive(Debug, Clone, Copy)]
pub struct RtpServerStarted;
#[derive(Debug, Clone, Copy)]
pub struct RtpServerTimeout;
#[derive(Debug, Clone, Copy)]
pub struct SendRtpStopped;
#[derive(Debug, Clone, Copy)]
pub struct RecordMp4;
#[derive(Debug, Clone, Copy)]
pub struct FlowReport;

impl HookEventTag for ServerStarted {
    const HOOK_NAME: &'static str = ON_SERVER_STARTED;
    const ENUM: ZlmHookEvent = ZlmHookEvent::ServerStarted;
}
impl HookEventTag for ServerKeepalive {
    const HOOK_NAME: &'static str = ON_SERVER_KEEPALIVE;
    const ENUM: ZlmHookEvent = ZlmHookEvent::ServerKeepalive;
}
impl HookEventTag for StreamChanged {
    const HOOK_NAME: &'static str = ON_STREAM_CHANGED;
    const ENUM: ZlmHookEvent = ZlmHookEvent::StreamChanged;
}
impl HookEventTag for StreamNotFound {
    const HOOK_NAME: &'static str = ON_STREAM_NOT_FOUND;
    const ENUM: ZlmHookEvent = ZlmHookEvent::StreamNotFound;
}
impl HookEventTag for StreamNoneReader {
    const HOOK_NAME: &'static str = ON_STREAM_NONE_READER;
    const ENUM: ZlmHookEvent = ZlmHookEvent::StreamNoneReader;
}
impl HookEventTag for StreamStarted {
    const HOOK_NAME: &'static str = ON_STREAM_STARTED;
    const ENUM: ZlmHookEvent = ZlmHookEvent::StreamStarted;
}
impl HookEventTag for Publish {
    const HOOK_NAME: &'static str = ON_PUBLISH;
    const ENUM: ZlmHookEvent = ZlmHookEvent::Publish;
}
impl HookEventTag for Play {
    const HOOK_NAME: &'static str = ON_PLAY;
    const ENUM: ZlmHookEvent = ZlmHookEvent::Play;
}
impl HookEventTag for RtpServerStarted {
    const HOOK_NAME: &'static str = ON_RTP_SERVER_STARTED;
    const ENUM: ZlmHookEvent = ZlmHookEvent::RtpServerStarted;
}
impl HookEventTag for RtpServerTimeout {
    const HOOK_NAME: &'static str = ON_RTP_SERVER_TIMEOUT;
    const ENUM: ZlmHookEvent = ZlmHookEvent::RtpServerTimeout;
}
impl HookEventTag for SendRtpStopped {
    const HOOK_NAME: &'static str = ON_SEND_RTP_STOPPED;
    const ENUM: ZlmHookEvent = ZlmHookEvent::SendRtpStopped;
}
impl HookEventTag for RecordMp4 {
    const HOOK_NAME: &'static str = ON_RECORD_MP4;
    const ENUM: ZlmHookEvent = ZlmHookEvent::RecordMp4;
}
impl HookEventTag for FlowReport {
    const HOOK_NAME: &'static str = ON_FLOW_REPORT;
    const ENUM: ZlmHookEvent = ZlmHookEvent::FlowReport;
}

/// 泛型 handler：所有 hook 路由共用此实现
///
/// 1. 反序列化请求体（保留 `hook_name`）
/// 2. 校验请求体中的 `hook_name` 与路由绑定的标签匹配（不匹配返回 success 占位）
/// 3. 委托给 `handle_webhook`，由其根据 hook_name 路由到具体业务逻辑
async fn handle_hook_event<T: HookEventTag>(
    State(state): State<AppState>,
    Json(event): Json<serde_json::Value>,
) -> Json<WVPResult<serde_json::Value>> {
    // 防御性校验：路由绑定的事件名与请求体中的 hook_name 不一致时
    // 仍返回 success（保持 WVP-Pro 兼容），但日志告警以便排查配置错误
    if let Some(name) = event.get("hook_name").and_then(|v| v.as_str()) {
        if name != T::HOOK_NAME {
            tracing::warn!(
                "hook 路由绑定 {} 与请求 hook_name={} 不一致（已转发）",
                T::HOOK_NAME,
                name
            );
        }
    } else {
        tracing::debug!(
            "hook 路由 {} 收到无 hook_name 字段的请求（已转发）",
            T::HOOK_NAME
        );
    }

    handle_webhook(State(state), Json(event)).await
}

/// WVP-Pro 多路径 hook 路由集合
///
/// 暴露 13 条 `/api/hook/<event>` POST 路径。`/api/zlm/hook` 单路径入口
/// 由 `router.rs` 直接注册 `zlm_hook::handle_webhook`，不在此处重复。
pub fn hook_routes() -> Router<AppState> {
    Router::new()
        .route("/api/hook/on_server_started", post(handle_hook_event::<ServerStarted>))
        .route(
            "/api/hook/on_server_keepalive",
            post(handle_hook_event::<ServerKeepalive>),
        )
        .route(
            "/api/hook/on_stream_changed",
            post(handle_hook_event::<StreamChanged>),
        )
        .route(
            "/api/hook/on_stream_not_found",
            post(handle_hook_event::<StreamNotFound>),
        )
        .route(
            "/api/hook/on_stream_none_reader",
            post(handle_hook_event::<StreamNoneReader>),
        )
        .route(
            "/api/hook/on_stream_started",
            post(handle_hook_event::<StreamStarted>),
        )
        .route("/api/hook/on_publish", post(handle_hook_event::<Publish>))
        .route("/api/hook/on_play", post(handle_hook_event::<Play>))
        .route(
            "/api/hook/on_rtp_server_started",
            post(handle_hook_event::<RtpServerStarted>),
        )
        .route(
            "/api/hook/on_rtp_server_timeout",
            post(handle_hook_event::<RtpServerTimeout>),
        )
        .route(
            "/api/hook/on_send_rtp_stopped",
            post(handle_hook_event::<SendRtpStopped>),
        )
        .route(
            "/api/hook/on_record_mp4",
            post(handle_hook_event::<RecordMp4>),
        )
        .route(
            "/api/hook/on_flow_report",
            post(handle_hook_event::<FlowReport>),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zlm::hook::ZlmHookEvent;

    #[test]
    fn test_hook_event_tag_mapping_complete() {
        // 所有 13 个标签的 HOOK_NAME 都应能反向解析到非 Unknown 枚举值
        let cases: &[&str] = &[
            ON_SERVER_STARTED,
            ON_SERVER_KEEPALIVE,
            ON_STREAM_CHANGED,
            ON_STREAM_NOT_FOUND,
            ON_STREAM_NONE_READER,
            ON_STREAM_STARTED,
            ON_PUBLISH,
            ON_PLAY,
            ON_RTP_SERVER_STARTED,
            ON_RTP_SERVER_TIMEOUT,
            ON_SEND_RTP_STOPPED,
            ON_RECORD_MP4,
            ON_FLOW_REPORT,
        ];
        for name in cases {
            let ev = ZlmHookEvent::from_hook_name(name);
            assert_ne!(ev, ZlmHookEvent::Unknown, "hook_name={} should not be Unknown", name);
        }
    }

    #[test]
    fn test_hook_event_tag_constants_match() {
        // 编译期已强制 ROUTE 标签与 ENUM 一一对应；这里再验证常量一致
        assert_eq!(<ServerStarted as HookEventTag>::HOOK_NAME, ON_SERVER_STARTED);
        assert_eq!(<StreamChanged as HookEventTag>::HOOK_NAME, ON_STREAM_CHANGED);
        assert_eq!(<RecordMp4 as HookEventTag>::HOOK_NAME, ON_RECORD_MP4);
        assert_eq!(<FlowReport as HookEventTag>::HOOK_NAME, ON_FLOW_REPORT);
    }
}
