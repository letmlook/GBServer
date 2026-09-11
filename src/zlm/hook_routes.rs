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

use crate::AppState;
use crate::zlm::hook::{handle_webhook_inner, ZlmHookEvent};

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
// 以下 5 个事件在 `handle_webhook` 里**本来就有分派分支**，但此前没有对应的
// 多路径路由 —— 也就没有任何合法 URL 能让 ZLM 把她们送进来（配置里虽然写了
// `hook.on_record_hls` 等，但指向的是单路径 `/api/zlm/hook`，而真实 ZLM 的
// body 不含 hook_name，见 handle_hook_event 的说明）。补齐路由后，
// per-event URL 配置才能真正生效。
const ON_RECORD_HLS: &str = "on_record_hls";
const ON_RECORD_FILE: &str = "on_record_file";
const ON_RTP_PLAYLIST: &str = "on_rtp_playlist";
const ON_RECORD_PROGRESS: &str = "on_record_progress";
const ON_SEND_RTP_PROGRESS: &str = "on_send_rtp_progress";

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
#[derive(Debug, Clone, Copy)]
pub struct RecordHls;
#[derive(Debug, Clone, Copy)]
pub struct RecordFile;
#[derive(Debug, Clone, Copy)]
pub struct RtpPlaylist;
#[derive(Debug, Clone, Copy)]
pub struct RecordProgress;
#[derive(Debug, Clone, Copy)]
pub struct SendRtpProgress;

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
impl HookEventTag for RecordHls {
    const HOOK_NAME: &'static str = ON_RECORD_HLS;
    const ENUM: ZlmHookEvent = ZlmHookEvent::Unknown;
}
impl HookEventTag for RecordFile {
    // `on_record_file` 在 `handle_webhook` 里是独立分支，但语义与
    // `on_record_mp4` 同族（`from_hook_name` 也把两者都映射到 RecordMp4）
    const HOOK_NAME: &'static str = ON_RECORD_FILE;
    const ENUM: ZlmHookEvent = ZlmHookEvent::RecordMp4;
}
impl HookEventTag for RtpPlaylist {
    const HOOK_NAME: &'static str = ON_RTP_PLAYLIST;
    const ENUM: ZlmHookEvent = ZlmHookEvent::Unknown;
}
impl HookEventTag for RecordProgress {
    const HOOK_NAME: &'static str = ON_RECORD_PROGRESS;
    const ENUM: ZlmHookEvent = ZlmHookEvent::RecordProgress;
}
impl HookEventTag for SendRtpProgress {
    const HOOK_NAME: &'static str = ON_SEND_RTP_PROGRESS;
    const ENUM: ZlmHookEvent = ZlmHookEvent::Unknown;
}

/// 本模块实际暴露的多路径 hook 事件清单。
///
/// 与 `hook::CONFIGURED_HOOK_EVENTS`（下发给 ZLM 的配置项）必须**完全一致**：
/// 少配 → 该事件收不到；多配 → ZLM 会 404 且我们白等。两者由单元测试交叉校验。
/// 只有路由、但**不下发给 ZLM** 的事件名（别名/兼容名）。
///
/// `on_record_file` 只存在于 `handle_webhook` 的分派器与部分 WVP 分支/魔改版
/// ZLM 中；ZLM 官方配置项里只有 `on_record_mp4` / `on_record_hls`
/// （见 https://docs.zlmediakit.com/guide/media_server/web_hook_api.html 的
/// `[hook]` 默认配置）。因此我们保留路由以兼容那些会发该名字的实现，
/// 但不把它写进 `setServerConfig`（ZLM 不认识该键）。
pub const HOOK_EVENT_ALIASES: &[&str] = &[ON_RECORD_FILE];

pub const ROUTED_HOOK_EVENTS: &[&str] = &[
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
    ON_RECORD_HLS,
    ON_RECORD_FILE,
    ON_FLOW_REPORT,
    ON_RTP_PLAYLIST,
    ON_RECORD_PROGRESS,
    ON_SEND_RTP_PROGRESS,
];

/// 泛型 handler：所有 hook 路由共用此实现
///
/// 1. 反序列化请求体（保留 `hook_name`）
/// 2. 校验请求体中的 `hook_name` 与路由绑定的标签匹配（不匹配返回 success 占位）
/// 3. 委托给 `handle_webhook`，由其根据 hook_name 路由到具体业务逻辑
async fn handle_hook_event<T: HookEventTag>(
    State(state): State<AppState>,
    raw_query: Option<axum::extract::RawQuery>,
    Json(mut event): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // **必须在这里把路由绑定的事件名注入请求体。**
    //
    // 真实 ZLMediaKit 的 hook 请求体是一个**扁平 JSON，且不含 `hook_name`
    // 字段** —— 事件类型完全由各自配置的 URL 决定（官方文档：
    // https://docs.zlmediakit.com/guide/media_server/web_hook_api.html ，
    // 可见 `on_play` / `on_publish` / `on_record_mp4` … 各有独立 URL，
    // body 里只有 app/stream/schema/mediaServerId 这些业务字段）。
    //
    // 而 `handle_webhook` 是按 body 里的 `hook_name` 分派的，缺失时取
    // `"unknown"` → 所有事件都走 "Unhandled webhook" 分支。
    // 此前这里只做了一次"不一致就 warn"的校验就原样转发，于是**整套 ZLM
    // webhook 集成在真实环境下完全不生效**（流状态、录像落盘、级联通告、
    // RTP server 就绪通知全部收不到），而接口总是回 `{"code":0}` 看不出问题。
    //
    // 现在：body 里没有 hook_name 时用路由绑定的事件名补上；有且不一致时
    // 以路由为准（URL 才是权威来源）。
    let route_name = T::HOOK_NAME;
    match event.get("hook_name").and_then(|v| v.as_str()) {
        Some(name) if name == route_name => {}
        Some(name) => {
            tracing::warn!(
                "hook 路由绑定 {} 与请求体 hook_name={} 不一致，以路由为准",
                route_name,
                name
            );
            if let Some(obj) = event.as_object_mut() {
                obj.insert(
                    "hook_name".to_string(),
                    serde_json::Value::String(route_name.to_string()),
                );
            }
        }
        None => {
            if let Some(obj) = event.as_object_mut() {
                obj.insert(
                    "hook_name".to_string(),
                    serde_json::Value::String(route_name.to_string()),
                );
            }
        }
    }

    // 把查询串一并透传：真实 ZLM 经 `[hook] admin_params` 把 secret 作为
    // URL 参数附加（body 里没有 secret），鉴权必须能读到它。
    let query = raw_query.and_then(|q| q.0);
    handle_webhook_inner(&state, event, query.as_deref()).await
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
        .route(
            "/api/hook/on_record_hls",
            post(handle_hook_event::<RecordHls>),
        )
        .route(
            "/api/hook/on_record_file",
            post(handle_hook_event::<RecordFile>),
        )
        .route(
            "/api/hook/on_rtp_playlist",
            post(handle_hook_event::<RtpPlaylist>),
        )
        .route(
            "/api/hook/on_record_progress",
            post(handle_hook_event::<RecordProgress>),
        )
        .route(
            "/api/hook/on_send_rtp_progress",
            post(handle_hook_event::<SendRtpProgress>),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zlm::hook::ZlmHookEvent;

    /// 下发给 ZLM 的 hook 事件清单与实际挂载的路由必须一一对应。
    ///
    /// 这条断言防住的正是本轮修掉的缺陷：配置里写了 `hook.on_record_hls`
    /// 之类的项，却**没有任何路由**能接住它（反之亦然），于是 ZLM 收到 404
    /// 或我们永远等不到该事件，而两边都不报错。
    #[test]
    fn configured_hook_events_all_have_routes() {
        // 方向一（最关键）：**配给 ZLM 的每个事件都必须有路由**，
        // 否则 ZLM 会 POST 到一个不存在的地址（404）而我们永远收不到该事件。
        for event in crate::zlm::hook::CONFIGURED_HOOK_EVENTS {
            assert!(
                ROUTED_HOOK_EVENTS.contains(event),
                "配置了 {} 却没有对应路由",
                event
            );
        }
    }

    #[test]
    fn routed_hook_events_are_configured_or_documented_aliases() {
        // 方向二：有路由但没配的事件，只能是显式登记的别名/兼容名，
        // 避免"写了路由却没人会调用"的死路由悄悄积累。
        for event in ROUTED_HOOK_EVENTS {
            assert!(
                crate::zlm::hook::CONFIGURED_HOOK_EVENTS.contains(event)
                    || HOOK_EVENT_ALIASES.contains(event),
                "路由 {} 既未配置给 ZLM，也不是已登记的别名",
                event
            );
        }
    }

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
