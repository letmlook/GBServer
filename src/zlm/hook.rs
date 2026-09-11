//! ZLM Webhook 处理

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

/// Phase 4.3: Protocol enable flags synced to ZLM on `on_server_started`.
pub const PROTOCOL_ENABLE_FLAGS: &[(&str, &str)] = &[
    ("protocol.enable_rtsp", "1"),
    ("protocol.enable_rtmp", "1"),
    ("protocol.enable_hls", "1"),
    ("protocol.enable_http", "1"),
    ("protocol.enable_ws", "1"),
    ("protocol.enable_rtp", "1"),
];

use crate::db::cloud_record::{self, CloudRecordInsert};
use crate::db::{stream_proxy, stream_push};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebHookRequest {
    pub hook_name: String,
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hook_name")]
pub enum WebHookEvent {
    #[serde(rename = "on_stream_changed")]
    StreamChanged(StreamChangedData),
    #[serde(rename = "on_stream_not_found")]
    StreamNotFound(StreamNotFoundData),
    #[serde(rename = "on_record_mp4")]
    RecordMp4(RecordMp4Data),
    #[serde(rename = "on_record_hls")]
    RecordHls(RecordHlsData),
    #[serde(rename = "on_play")]
    Play(PlayData),
    #[serde(rename = "on_publish")]
    Publish(PublishData),
    #[serde(rename = "on_server_started")]
    ServerStarted(ServerStartedData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChangedData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    /// 真实 ZLM 的字段名是 **`regist`**（其 wiki 有专门的 commit
    /// "on_stream_changed 注册时添加 regist 字段"），不是 `register`。
    /// 此前只认 `register` → 该事件反序列化失败、被静默丢弃，
    /// 流上下线状态因此永远不同步。
    #[serde(alias = "regist")]
    pub register: bool,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

/// `on_stream_none_reader` 的载荷。
///
/// 官方文档里该事件的 body 只有
/// `{mediaServerId, app, stream, schema, vhost}` —— **没有** `regist`/`register`
/// （那是 `on_stream_changed` 的字段）。
///
/// 此前这里复用了 `StreamChangedData`，而它的 `register: bool` 是必填的，
/// 于是该事件的载荷**永远反序列化失败**（"missing field register"），
/// 整个分支被静默跳过 —— 无人观看自动关流从未生效，日志上还看不出原因。
/// 类型必须按事件分开定义，字段缺失一律容忍。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamNoneReaderData {
    #[serde(default)]
    pub schema: String,
    pub app: String,
    pub stream: String,
    #[serde(default)]
    pub vhost: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamNotFoundData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    pub ssrc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordMp4Data {
    /// 真实 ZLM 的 `on_record_mp4` 载荷里**没有** `schema` 字段
    /// （见 https://docs.zlmediakit.com/guide/media_server/web_hook_api.html
    /// 的 on_record_mp4 示例 body）。此前声明为必填 `String` → 反序列化必然
    /// 失败 → 整个事件被静默丢弃（连 "MP4 recorded" 都不会打），
    /// 云录像永远不入库。
    #[serde(default)]
    pub schema: Option<String>,
    pub app: String,
    pub stream: String,
    #[serde(default)]
    pub vhost: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    #[serde(default)]
    pub folder: Option<String>,
    pub file_size: u64,
    /// 录制时长（秒）。真实 ZLM 的字段名是 **`time_len`**（float）。
    #[serde(default, alias = "time_len")]
    pub file_duration: f64,
    /// 录制开始时间（UNIX 秒）。真实 ZLM 的字段名是 **`start_time`**（整数）。
    #[serde(default, alias = "start_time")]
    pub file_start_time: i64,
    /// 相对播放地址（真实 ZLM 载荷里有，此前未声明）
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordHlsData {
    /// 与 `RecordMp4Data` 同样的问题：ZLM 的录制类 hook 载荷里没有必填的
    /// `schema`，时长字段是 `time_len`、开始时间是 `start_time`（UNIX 秒）。
    /// 声明成必填 `schema`/`file_duration`/`file_create_time` 会让反序列化
    /// 直接失败、整条事件被静默丢弃。
    #[serde(default)]
    pub schema: Option<String>,
    pub app: String,
    pub stream: String,
    #[serde(default)]
    pub vhost: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    #[serde(default)]
    pub folder: Option<String>,
    pub file_size: u64,
    #[serde(default, alias = "time_len")]
    pub file_duration: f64,
    #[serde(default, alias = "start_time")]
    pub file_start_time: i64,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayData {
    pub ip: String,
    pub port: u16,
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishData {
    pub ip: String,
    pub port: u16,
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStartedData {
    /// 全部字段都容错。
    ///
    /// 此前六个端口都是**必填**：ZLM 少发任何一个（不同版本/发行版字段集并不
    /// 一致）都会导致 `from_value` 失败 → 整个 `on_server_started` 被静默丢弃
    /// → 节点状态不更新、端口不同步，**最关键的是 hook 重新配置永远不会执行**
    /// （hook 配置只在这一条事件里做），于是整套 webhook 静默失效。
    /// 现在缺字段就用默认值，重要的动作照常执行。
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub hook_port: u16,
    #[serde(default = "default_rtsp_port")]
    pub rtsp_port: u16,
    #[serde(default = "default_rtmp_port")]
    pub rtmp_port: u16,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default)]
    pub https_port: u16,
}

fn default_rtsp_port() -> u16 {
    554
}
fn default_rtmp_port() -> u16 {
    1935
}
fn default_http_port() -> u16 {
    80
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeepaliveData {
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

/// `on_send_rtp_stopped`（ZLM 停止向目标推流）的载荷。
///
/// 此前这条事件复用了 `StreamChangedData`，而后者**要求 `schema`** ——
/// 该事件的载荷里没有它，于是反序列化失败、整个分支被静默跳过：
/// 级联推流已停但平台仍以为在推，`SendRtpManager` 里的会话永不清理。
/// 这里所有字段都容错，并保留 `ssrc` 以便按推流标识兜底关闭。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendRtpStoppedData {
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub stream: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub vhost: Option<String>,
    #[serde(default)]
    pub ssrc: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpServerTimeoutData {
    pub app: Option<String>,
    pub port: Option<u16>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

// =====================================================================
// ABL 钩子支持（设计文档 §6.3 阶段 0 缺口 1）
//
// ABL（Another Live media Broadcaster）是 ZLMediaKit 兼容的开源分支，
// 暴露额外的 hook 事件用于细粒度控制。参考 Java 实现在生产部署中
// 会使用这些事件，本实现补齐 on_rtp_playlist / on_record_progress。
// =====================================================================

/// ABL `on_rtp_playlist` 事件：RTP 推流端开始 / 停止推 playlist 时触发
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpPlaylistData {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub ssrc: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    /// "start" / "stop"
    pub action: Option<String>,
}

/// ABL `on_record_progress` 事件：MP4 / HLS 录制进度回调（每 N 秒一次）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordProgressData {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub vhost: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    /// 当前累计录制时长（秒）
    pub current_duration: Option<f64>,
    /// 当前累计文件大小（字节）
    pub current_size: Option<u64>,
    /// 进度时间戳（毫秒）
    pub progress_ts: Option<i64>,
}

/// ABL `on_send_rtp_progress` 事件：SendRtp 推流进度（每 N 包一次）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRtpProgressData {
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    pub total_sent: Option<u64>,
    pub bytes_sent: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpServerStartedData {
    pub stream_id: String,
    pub port: Option<u16>,
    pub app: Option<String>,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChangedByAppData {
    pub schema: String,
    pub app: String,
    pub stream: String,
    pub vhost: String,
    #[serde(default, alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

// =====================================================================
// ZlmHookEvent 枚举（Phase 4.1，WVP-Pro 兼容）
//
// 所有 ZLM hook 事件以枚举形式表达，便于 dispatcher 严格匹配及前端按需订阅。
// `from_hook_name` 将 ZLM 字符串 hook 名（如 "on_stream_changed"）解析为枚举；
// `default_response` 返回 WVP-Pro 兼容的成功响应结构。
// =====================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlmHookEvent {
    ServerStarted,
    ServerKeepalive,
    StreamChanged,
    StreamNotFound,
    StreamNoneReader,
    StreamStarted,
    Publish,
    Play,
    RtpServerStarted,
    RtpServerTimeout,
    SendRtpStopped,
    RecordMp4,
    RecordProgress,
    FlowReport,
    Unknown,
}

impl ZlmHookEvent {
    pub fn from_hook_name(name: &str) -> Self {
        match name {
            "on_server_started" => Self::ServerStarted,
            "on_server_keepalive" => Self::ServerKeepalive,
            "on_stream_changed" => Self::StreamChanged,
            "on_stream_not_found" => Self::StreamNotFound,
            "on_stream_none_reader" => Self::StreamNoneReader,
            "on_stream_started" => Self::StreamStarted,
            "on_publish" => Self::Publish,
            "on_play" => Self::Play,
            "on_rtp_server_started" => Self::RtpServerStarted,
            "on_rtp_server_timeout" => Self::RtpServerTimeout,
            "on_send_rtp_stopped" => Self::SendRtpStopped,
            "on_record_mp4" | "on_record_file" => Self::RecordMp4,
            "on_record_progress" => Self::RecordProgress,
            "on_flow_report" => Self::FlowReport,
            _ => Self::Unknown,
        }
    }

    /// WVP-Pro 兼容的默认成功响应：前端只需 `code === 0` 即视为成功。
    pub fn default_response(&self) -> serde_json::Value {
        serde_json::json!({"code": 0, "msg": "success"})
    }
}

/// 把配置里的"单个 hook 地址"归一化成服务器根地址。
///
/// 配置项 `hook_url` 历史上是**一个**地址（默认 `…/api/zlm/hook`），但真实
/// ZLMediaKit 的 hook 请求体里**没有 `hook_name`**，事件类型完全由 URL 决定
/// （见 `hook_routes::handle_hook_event` 的说明与官方文档）。因此必须为每个
/// 事件生成各自的 `/api/hook/<event>` 地址。
pub fn hook_base_url(configured: &str) -> String {
    let trimmed = configured.trim().trim_end_matches('/');
    for suffix in ["/api/zlm/hook", "/api/hook"] {
        if let Some(pos) = trimmed.rfind(suffix) {
            return trimmed[..pos].trim_end_matches('/').to_string();
        }
    }
    trimmed.to_string()
}

/// 某个 hook 事件的回调地址。
pub fn hook_event_url(base: &str, event: &str) -> String {
    format!("{}/api/hook/{}", base.trim_end_matches('/'), event)
}

/// 需要下发配置给 ZLM 的全部 hook 事件。
///
/// 与 `hook_routes::hook_routes()` 暴露的路由**一一对应**：每个事件都有自己的
/// URL，ZLM 才知道自己在触发什么；配了却没人处理的事件（或反之）都会造成
/// "接口回 200 但什么也没发生"。
pub const CONFIGURED_HOOK_EVENTS: &[&str] = &[
    "on_server_started",
    "on_server_keepalive",
    "on_stream_changed",
    "on_stream_not_found",
    "on_stream_none_reader",
    "on_stream_started",
    "on_publish",
    "on_play",
    "on_rtp_server_started",
    "on_rtp_server_timeout",
    "on_send_rtp_stopped",
    "on_record_mp4",
    "on_record_hls",
    "on_flow_report",
    "on_rtp_playlist",
    "on_record_progress",
    "on_send_rtp_progress",
];

/// 构造 `<hook.xxx, url>` 配置项列表（含 `hook.enable` 与 `hook.admin_params`）。
///
/// `secret` 会被写进 `hook.admin_params` —— ZLM 会把该参数**附加到每个 hook
/// 请求的 URL 上**（官方文档：`admin_params=secret=xxx`），而不是放进 body。
/// 不配置它时，ZLM 会用它自己 config.ini 里的默认值，与我们节点的 secret
/// 对不上 → 带鉴权的 `on_publish` / `on_play` 全部被我们拒绝 → ZLM 拒绝一切
/// 推流与播放。
pub fn hook_config_items(configured_hook_url: &str, secret: &str) -> Vec<(String, String)> {
    let base = hook_base_url(configured_hook_url);
    let mut items = vec![
        ("hook.enable".to_string(), "1".to_string()),
        (
            "hook.admin_params".to_string(),
            format!("secret={}", secret),
        ),
        // ZLM 的 hook 超时（秒）：默认 10s，显式收紧到 5s，
        // 避免 ZLM 因为慢回调而长时间阻塞推流/播放鉴权。
        ("hook.timeoutSec".to_string(), "5".to_string()),
    ];
    for event in CONFIGURED_HOOK_EVENTS {
        items.push((
            format!("hook.{}", event),
            hook_event_url(&base, event),
        ));
    }
    items
}

/// 从 stream_id 解析 `(device_id, channel_id)`。
///
/// 转发到 `StreamReconnectManager::parse_stream_id`（唯一实现）——
/// 此前这里另有一份只认 `$` / `/` 的版本，而本平台的流名是
/// `{device}_{channel}`（下划线），导致所有基于它的分支对自家流全部失效。
fn parse_stream_id(stream: &str) -> Option<(String, String)> {
    crate::sip::gb28181::stream_reconnect::StreamReconnectManager::parse_stream_id(stream)
}

fn parse_record_time_ms(value: &str) -> i64 {
    if let Ok(ts) = value.parse::<i64>() {
        if ts > 1_000_000_000_000 {
            return ts;
        }
        if ts > 1_000_000_000 {
            return ts * 1000;
        }
    }

    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .map(|dt| dt.and_utc().timestamp_millis())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis())
}

async fn save_record(
    state: &AppState,
    media_server_id: Option<String>,
    app: String,
    stream: String,
    file_name: String,
    folder: Option<String>,
    file_path: String,
    file_size: u64,
    file_duration: f64,
    file_create_time: String,
) {
    let start_time = parse_record_time_ms(&file_create_time);
    let duration_ms = (file_duration.max(0.0) * 1000.0).round() as i64;
    let end_time = start_time + duration_ms;
    let server_id = state
        .config
        .user_settings
        .as_ref()
        .and_then(|settings| settings.server_id.clone());

    let record = CloudRecordInsert {
        app,
        stream,
        call_id: Some(file_name.clone()),
        start_time: Some(start_time),
        end_time: Some(end_time),
        media_server_id,
        server_id,
        file_name: Some(file_name),
        folder,
        file_path: Some(file_path),
        file_size: Some(file_size.min(i64::MAX as u64) as i64),
        time_len: Some(file_duration),
    };

    if let Err(e) = cloud_record::insert(&state.pool, &record).await {
        tracing::warn!("Failed to save cloud record: {}", e);
    }
}

async fn sync_stream_changed(state: &AppState, data: &StreamChangedData) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let media_server_id = data.media_server_id.as_deref();

    match stream_push::update_pushing_status_by_app_stream(
        &state.pool,
        &data.app,
        &data.stream,
        media_server_id,
        data.register,
        &now,
    )
    .await
    {
        Ok(affected) if affected > 0 => {
            tracing::debug!(
                "Stream push status synced: {}/{} pushing={}",
                data.app,
                data.stream,
                data.register
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to sync stream push status: {}", e),
    }

    match stream_proxy::update_pulling_status_by_app_stream(
        &state.pool,
        &data.app,
        &data.stream,
        media_server_id,
        data.register,
        &now,
    )
    .await
    {
        Ok(affected) if affected > 0 => {
            tracing::debug!(
                "Stream proxy status synced: {}/{} pulling={}",
                data.app,
                data.stream,
                data.register
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to sync stream proxy status: {}", e),
    }

    // Phase 7.1: media_server stream_count now goes through StateStore so it's
    // available to select_least_loaded_server_filtered even on single-node deploys.
    if let Some(media_server_id) = media_server_id {
        if data.register {
            state.state_store.set_media_server(
                media_server_id,
                crate::state_store::MediaServerLoad {
                    server_id: media_server_id.to_string(),
                    stream_count: state.state_store.get_media_server(media_server_id)
                        .map(|s| s.stream_count + 1).unwrap_or(1),
                    rtp_server_count: state.state_store.get_media_server(media_server_id)
                        .map(|s| s.rtp_server_count).unwrap_or(0),
                    online: true,
                    last_keepalive: chrono::Utc::now(),
                },
            );
        } else if let Some(mut s) = state.state_store.get_media_server(media_server_id) {
            s.stream_count = (s.stream_count - 1).max(0);
            s.last_keepalive = chrono::Utc::now();
            state.state_store.set_media_server(media_server_id, s);
        }
    }
    // Update global active stream metrics
    if let Some(ref zlm) = state.zlm_client {
        if let Ok(streams) = zlm.get_media_list(None, None, None).await {
            crate::metrics::set_active_streams(streams.len());
        }
    }

    // Phase 3.1: 当 RTP 流注册时，通知 media_waiter_manager
    // 让 play_start 等媒体到达的 handler 收到 MediaReady 后返回。
    if data.register && data.app == "rtp" {
        if let Some(ref sip_server) = state.sip_server {
            let sip = sip_server.as_ref();
            let resolved = sip
                .media_waiter_manager()
                .resolve_by_stream(&data.stream, &data.app);
            tracing::debug!(
                "on_stream_changed rtp/{} register=true media_waiter resolved={}",
                data.stream, resolved
            );
        }

        // Phase 6.3: 如果 stream 命名以 "jt1078_" 开头，路由到 JtMediaSessionManager
        if data.stream.starts_with("jt1078_") {
            // Format: jt1078_{phone}_{channel_id}
            let rest = data.stream.trim_start_matches("jt1078_");
            let parts: Vec<&str> = rest.splitn(2, '_').collect();
            if parts.len() == 2 {
                let phone = parts[0].to_string();
                let channel_id: u8 = parts[1].parse().unwrap_or(0);
                let mgr_guard = state.jt1078_manager.read().await;
                if let Some(m) = mgr_guard.as_ref() {
                    let resolved = m
                        .media_session_manager()
                        .resolve_waiter(&phone, channel_id, &data.stream);
                    tracing::info!(
                        "6.3 on_stream_changed routed to JtMediaSessionManager: phone={} ch={} resolved={}",
                        phone, channel_id, resolved
                    );
                }
            }
        }
    }

    // 下载会话的状态机：inviting → downloading →（流结束）completed。
    //
    // 通过 stream_id 的 `download_` 前缀识别（与 `gb_record_download_start`
    // 的命名一致）。此前只处理了**上线**：流起来后置为 downloading，
    // 而设备推完流下线（`regist=false`）时什么都不做 —— 下载会话永远停在
    // "downloading"，前端进度条不会结束，用户也不知道文件已经就绪。
    if data.stream.starts_with("download_") {
        if let Some(ref dm) = state.download_manager {
            if let Some(session) = dm.get_by_zlm_stream(&data.stream).await {
                if data.register {
                    tracing::info!(
                        "Download stream ready: session={} stream={}",
                        session.stream_id, data.stream
                    );
                    dm.update_progress_percent(&session.stream_id, 0.0, "downloading")
                        .await;
                } else {
                    // 流注销 == 设备推完。ZLM 侧的 MP4 收尾由
                    // on_record_mp4 回调补齐（此处先把会话标记为完成，
                    // 让 /progress 能给出终态）。
                    tracing::info!(
                        "Download stream finished: session={} stream={}",
                        session.stream_id, data.stream
                    );
                    dm.update_progress_percent(&session.stream_id, 100.0, "completed")
                        .await;
                }
            }
        }
    }
}

fn should_skip_discovered_push(app: &str) -> bool {
    matches!(
        app.to_ascii_lowercase().as_str(),
        "rtp" | "gb_playback" | "playback" | "download"
    )
}

async fn register_published_stream(state: &AppState, data: &PublishData) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let media_server_id = data.media_server_id.as_deref();

    match stream_proxy::get_by_app_stream(&state.pool, &data.app, &data.stream).await {
        Ok(Some(proxy)) => {
            if let Err(e) = stream_proxy::update_pulling_status_by_app_stream(
                &state.pool,
                &data.app,
                &data.stream,
                media_server_id,
                true,
                &now,
            )
            .await
            {
                tracing::warn!("Failed to mark proxy as pulling: {}", e);
            } else {
                tracing::debug!(
                    "Published stream matched proxy id={} {}/{}",
                    proxy.id,
                    data.app,
                    data.stream
                );
            }
            return;
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("Failed to query stream proxy for publish event: {}", e),
    }

    if should_skip_discovered_push(&data.app) {
        return;
    }

    let server_id = state
        .config
        .user_settings
        .as_ref()
        .and_then(|settings| settings.server_id.as_deref());

    if let Err(e) = stream_push::upsert_discovered(
        &state.pool,
        &data.app,
        &data.stream,
        media_server_id,
        server_id,
        &now,
    )
    .await
    {
        tracing::warn!("Failed to register discovered push stream: {}", e);
    }
}

/// 将 ZLM `on_flow_report` 上报的**权威绝对流数**同步到 `StateStore`。
///
/// `streams` 是绝对计数，因此直接覆盖 `stream_count` —— 除同步外，还能纠正
/// `on_stream_changed` 增减路径可能累积的漂移（例如漏收 unregister 事件）。
///
/// 无人观看关流决策的结果。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IdleStreamDecision {
    /// 是否让 ZLM 关闭该流（即响应里的顶层 `close`）。
    pub close: bool,
    /// 决策原因，写进响应 `msg` 与日志，便于现场排查"为什么没关/为什么关了"。
    pub reason: String,
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
}

impl IdleStreamDecision {
    fn keep(reason: impl Into<String>) -> Self {
        Self {
            close: false,
            reason: reason.into(),
            device_id: None,
            channel_id: None,
        }
    }
}

/// ZLM hook 的成功响应。
///
/// **必须是顶层 `code`**：ZLMediaKit 直接读响应对象的 `code` 字段来决定
/// 是否放行（`on_publish` / `on_play`），包进 `WVPResult` 的 `data` 里它读不到。
pub(crate) fn hook_ok_response() -> serde_json::Value {
    serde_json::json!({ "code": 0, "msg": "success" })
}

/// ZLM hook 的失败响应（鉴权不通过）。非 0 的顶层 `code` 会让 ZLM
/// 拒绝推流 / 拒绝播放。
pub(crate) fn hook_error_response(msg: &str) -> serde_json::Value {
    serde_json::json!({ "code": -1, "msg": msg })
}

/// `on_stream_none_reader` 的响应：`close` 同样必须在**顶层**。
///
/// 官方文档：该事件"可以选择是否关闭无人观看的流"，响应为
/// `{"code":0,"close":true|false}`。此前本项目把它包在
/// `WVPResult.data` 里返回，ZLM 读不到 `close`，按默认 `false` 处理 ——
/// 无人观看自动关流**从未生效**。
pub(crate) fn none_reader_response(decision: &IdleStreamDecision) -> serde_json::Value {
    serde_json::json!({
        "code": 0,
        "close": decision.close,
        "msg": decision.reason,
    })
}

/// 判断一个「无人观看」的流是否应该由 ZLM 关闭。
///
/// 判定顺序（先否决、后放行），全部基于**可观测事实**，不猜：
///
/// 1. 不是国标流（推流/拉流代理）→ 不关。它没有"设备端要停"的语义，
///    由用户在推流/代理页面显式停止。
/// 2. ZLM 报告该流仍有 reader（含 hls/mp4 之类内部消费者）→ 不关。
///    这解决了 hook 与播放器连接之间的竞态：`/api/play/start` 先开流、
///    前端随后才挂播放器，这段时间里 `close=true` 会把刚建好的流掐掉。
/// 3. ZLM 正在对该流录像 → 不关（否则录像被截断）。
/// 4. 正在向级联上级平台推流（SendRtp 会话）→ 不关。
/// 5. 平台下发的 `Record` 云录像指令仍在生效 → 不关。
/// 6. 其余国标流（实时点播 / 回放 / 下载）→ 关闭，并给设备发 BYE。
async fn decide_idle_stream(state: &AppState, data: &StreamNoneReaderData) -> IdleStreamDecision {
    let Some((device_id, channel_id)) = parse_stream_id(&data.stream) else {
        return IdleStreamDecision::keep("非国标流（推流/拉流代理），交由用户停止");
    };

    let zlm = state.get_zlm_client(data.media_server_id.as_deref());

    // 2. ZLM 侧真实 reader 数
    if let Some(ref client) = zlm {
        match client
            .get_media_info(&data.schema, "__defaultVhost__", &data.app, &data.stream)
            .await
        {
            Ok(Some(info)) if info.total_reader_count > 0 || info.reader_count > 0 => {
                return IdleStreamDecision::keep(format!(
                    "仍有观看者（reader={}/{}）",
                    info.reader_count, info.total_reader_count
                ));
            }
            Ok(_) => {}
            Err(e) => {
                // 查不到观看者数量时**不能**当成"没人看"：关流会发 BYE 停掉
                // 设备推流，是破坏性动作。无法确认就保持 —— 宁可让 ZLM
                // 自己按 `general.streamNoneReaderDelayMS` 兜底，也不能误杀
                // 正在播放的流。
                return IdleStreamDecision::keep(format!(
                    "无法确认观看者数量（getMediaInfo 失败: {}）",
                    e
                ));
            }
        }

        // 3. 正在录像
        match client
            .is_recording("__defaultVhost__", &data.app, &data.stream)
            .await
        {
            Ok(true) => return IdleStreamDecision::keep("ZLM 正在录像"),
            Ok(false) => {}
            Err(e) => {
                // 同上：确认不了录像状态就不做破坏性操作
                return IdleStreamDecision::keep(format!(
                    "无法确认录像状态（isRecording 失败: {}）",
                    e
                ));
            }
        }
    }

    // 4. 正在向级联平台推流
    //
    // SendRtp 会话按通道索引（`SendRtpSession` 里没有 ZLM stream_id 字段，
    // 级联推的是"这个通道的流"），所以按 channel_id 判定即可覆盖实时与回放。
    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.as_ref();
        if !sip.send_rtp_manager().get_by_channel(&channel_id).is_empty() {
            return IdleStreamDecision::keep("正在向级联上级平台推流");
        }
    }

    // 5. 平台下发的云录像指令（Record/StopRecord）
    if state.state_store.get_recording(&device_id, &channel_id).is_some() {
        return IdleStreamDecision::keep("云录像指令生效中");
    }

    // 6. 可以关
    let mut decision = IdleStreamDecision::keep("无人观看，回收资源");
    decision.close = true;
    decision.device_id = Some(device_id);
    decision.channel_id = Some(channel_id);
    decision
}

/// 真正回收一路无人观看的国标流。
///
/// 必须同时做两件事，缺一不可：
///
/// * **给设备发 BYE**：否则设备会继续往已关闭的端口推 RTP（国标设备不会
///   因为平台关闭了流就自己停）；
/// * **关闭 ZLM 收流端口**（`closeRtpServer`）：`close=true` 只让 ZLM 释放
///   media source，`openRtpServer` 开的那个 UDP/TCP 收流端口仍在监听，
///   不关就会一直占着端口和线程。
async fn close_idle_stream(
    state: &AppState,
    data: &StreamNoneReaderData,
    decision: &IdleStreamDecision,
) {
    let (Some(device_id), Some(channel_id)) = (decision.device_id.as_deref(), decision.channel_id.as_deref())
    else {
        return;
    };

    if let Some(ref sip_server) = state.sip_server {
        let sip = sip_server.as_ref();
        match sip.send_session_bye(device_id, channel_id).await {
            Ok(call_id) => tracing::info!(
                "none_reader: 已向设备发 BYE device={} channel={} call_id={}",
                device_id,
                channel_id,
                call_id
            ),
            Err(e) => tracing::warn!(
                "none_reader: 设备 BYE 发送失败 device={} channel={}: {}",
                device_id,
                channel_id,
                e
            ),
        }
    } else {
        tracing::warn!(
            "none_reader: SIP server 未就绪，无法给 {}/{} 发 BYE",
            device_id,
            channel_id
        );
    }

    if let Some(client) = state.get_zlm_client(data.media_server_id.as_deref()) {
        // 先按 hook 载荷里的 stream 关（这是权威流名），
        // 再按会话记录的 stream_id 兜底（二者在规范流名上一致）。
        if let Err(e) = client.close_rtp_server(&data.stream).await {
            tracing::warn!(
                "none_reader: closeRtpServer({}) 失败: {}",
                data.stream,
                e
            );
        }
    }
}

/// 语义细节：
/// - **已存在**的条目只更新 `stream_count` / `last_keepalive`，不触碰 `online` 与
///   `rtp_server_count`。这一点很重要：内存后端的 filtered 选择会按 `online` 过滤
///   （见 `state_store.rs::media_server_select_least_loaded_filtered`），
///   若在此处强行置 `online = true`，会把已判离线的节点重新变成候选。
/// - **不存在**的条目按「刚上报过即在线」新建，`online: true`。
///
/// 2026-09-11：原先此逻辑写的是 Redis `gb:ms:streams:*` 计数（`cache` 模块）。
/// 该模块已与 StateStore 完全重复并删除，此处为迁移后的唯一状态源。
fn sync_media_server_stream_count(
    store: &crate::state_store::StateStore,
    media_server_id: &str,
    streams: i64,
) {
    let mut load = store
        .get_media_server(media_server_id)
        .unwrap_or(crate::state_store::MediaServerLoad {
            server_id: media_server_id.to_string(),
            stream_count: 0,
            rtp_server_count: 0,
            online: true,
            last_keepalive: chrono::Utc::now(),
        });
    load.stream_count = streams;
    load.last_keepalive = chrono::Utc::now();
    store.set_media_server(media_server_id, load);
}

/// ZLM Webhook 的统一入口（单路径 `/api/zlm/hook`）。
///
/// # 响应契约（重要）
///
/// 返回的是**顶层扁平 JSON**，不是 `WVPResult` 信封。
/// 真实 ZLMediaKit 直接读**顶层**字段：
///
/// * `code`：`0` 表示放行（`on_publish` / `on_play` 的鉴权结论）；非 0 拒绝。
/// * `close`：`on_stream_none_reader` 专用，`true` 表示让 ZLM 关闭该无人流。
/// * `auto_close`：`on_publish` 可选，控制该流后续无人观看时是否直接关闭。
///
/// 此前所有 hook 都返回 `{"code":0,"msg":"成功","data":{...}}` —— `code` 恰好在
/// 顶层所以鉴权看着是对的，但 `close` 被埋进 `data` 里，ZLM 读不到，
/// 于是「无人观看自动关流」永远不生效（ZLM 侧取默认 `false`）。
pub async fn handle_webhook(
    State(state): State<AppState>,
    raw_query: Option<axum::extract::RawQuery>,
    Json(event): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let query = raw_query.and_then(|q| q.0);
    handle_webhook_inner(&state, event, query.as_deref()).await
}

/// 从查询串里取 `secret`。
///
/// 真实 ZLMediaKit 通过 `[hook] admin_params` 把 secret 作为**URL 查询参数**
/// 附带在每个 hook 请求上（默认 `admin_params=secret=xxx`，见官方文档），
/// 而 **body 里没有 `secret` 字段**。此前只从 body 读 → 所有带鉴权的
/// `on_publish` / `on_play` 一律返回 "secret mismatch"，
/// ZLM 因此**拒绝一切推流与播放**。
fn secret_from_query(query: Option<&str>) -> Option<String> {
    let q = query?;
    for kv in q.split('&') {
        let (k, v) = match kv.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if k == "secret" {
            return Some(percent_decode(v));
        }
    }
    None
}

/// 最小化百分号解码（ZLM 的 secret 可能是 base64，含 `+/=`）。
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub(crate) async fn handle_webhook_inner(
    state: &AppState,
    event: serde_json::Value,
    query: Option<&str>,
) -> Json<serde_json::Value> {
    let hook_name = event
        .get("hook_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match hook_name {
        "on_stream_changed" => {
            let parsed = event
                .get("schema")
                .and_then(|_| match serde_json::from_value::<StreamChangedData>(event.clone()) {
                    Ok(d) => Some(d),
                    Err(e) => {
                        tracing::warn!("on_stream_changed 载荷解析失败: {} 原文={}", e, event);
                        None
                    }
                });
            if let Some(data) = parsed {
                tracing::info!(
                    "Stream changed: {}/{}/{} register={}",
                    data.schema,
                    data.app,
                    data.stream,
                    data.register
                );
                sync_stream_changed(&state, &data).await;

                let ws_msg = serde_json::json!({
                    "type": "streamChanged",
                    "app": data.app,
                    "stream": data.stream,
                    "schema": data.schema,
                    "register": data.register,
                });
                state.ws_state.broadcast("streamChanged", ws_msg).await;
            }
        }
        "on_stream_not_found" => {
            // ZLM 在「有播放器请求了一个不存在的流」时触发本事件，
            // 官方文档说明它**不影响 ZLM 行为**（是个通知），配套
            // `on_stream_none_reader` 可以完成按需拉流。
            //
            // 本平台的按需拉流路径只有一条：**给国标设备发 INVITE**，
            // 让设备把 RTP 推到我们事先 `openRtpServer` 分配好的端口。
            //
            // 修正：此前这里在 INVITE 之后还有一段"自动拉流"兜底，
            // 用 `rtsp://{device_id}:8554/{channel_id}` 调
            // `addStreamProxy`。这是**编造出来的地址**：国标设备不会在
            // 8554 端口提供 RTSP 服务（8554 只是 ZLM 自己的 RTSP 端口），
            // 设备编号也不是主机名。该分支只会稳定地拉流失败，并
            // 掩盖真正的原因；现已删除。
            if let Some(data) = serde_json::from_value::<StreamNotFoundData>(event.clone()).ok() {
                tracing::warn!(
                    "Stream not found: {}/{}/{}",
                    data.schema,
                    data.app,
                    data.stream
                );

                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.as_ref();
                    let reconnect_mgr = sip.stream_reconnect_manager();
                    // 登记到重连管理器，按退避策略持续重试
                    reconnect_mgr.on_stream_not_found(&data.app, &data.stream);

                    // 立刻做一次按需拉起
                    if let Some((device_id, channel_id)) =
                        crate::sip::gb28181::stream_reconnect::StreamReconnectManager::parse_stream_id(&data.stream)
                    {
                        if sip.is_device_online(&device_id).await {
                            match sip.start_live_stream(&device_id, &channel_id, 15).await {
                                Ok(stream_id) => {
                                    tracing::info!(
                                        "On-demand pull started: stream={} device={} channel={}",
                                        stream_id, device_id, channel_id
                                    );
                                    reconnect_mgr.mark_success(&data.stream);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "On-demand pull failed for {}/{}: {}",
                                        device_id, channel_id, e
                                    );
                                }
                            }
                        } else {
                            tracing::debug!("Device {} offline, skip on-demand pull", device_id);
                        }
                    }
                }
            }
        }
        "on_record_mp4" => {
            if let Some(data) = serde_json::from_value::<RecordMp4Data>(event.clone()).ok() {
                tracing::info!(
                    "MP4 recorded: {} ({} bytes)",
                    data.file_name,
                    data.file_size
                );
                // 真实 ZLM 给的是 start_time（UNIX 秒），本地格式化成可读时间
                let created = chrono::DateTime::from_timestamp(data.file_start_time, 0)
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| {
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                    });
                save_record(
                    &state,
                    data.media_server_id,
                    data.app,
                    data.stream,
                    data.file_name,
                    data.folder,
                    data.file_path,
                    data.file_size,
                    data.file_duration,
                    created,
                )
                .await;
            }
        }
        "on_record_hls" => {
            if let Some(data) = serde_json::from_value::<RecordHlsData>(event.clone()).ok() {
                tracing::info!(
                    "HLS recorded: {} ({} bytes)",
                    data.file_name,
                    data.file_size
                );
                // 真实 ZLM 给的是 start_time（UNIX 秒），本地格式化成可读时间
                let created = chrono::DateTime::from_timestamp(data.file_start_time, 0)
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| {
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                    });
                save_record(
                    &state,
                    data.media_server_id,
                    data.app,
                    data.stream,
                    data.file_name,
                    data.folder,
                    data.file_path,
                    data.file_size,
                    data.file_duration,
                    created,
                )
                .await;
            }
        }
        "on_play" => {
            // 鉴权必须在**解析 body 之前**无条件执行。
            //
            // 修正：此前 `check_hook_auth` 写在 `if let Some(data) = from_value(...)`
            // 成功分支里 —— 只要 body 少了某个字段导致解析失败，就**完全跳过鉴权**
            // 并返回 {"code":0}（放行）。对一个鉴权型 hook 来说这是 fail-open：
            // 任何我们没预料到的载荷都会在无 secret 的情况下被允许播放。
            // 现在先按 body 里的 ip 字段（尽力而为）做鉴权，失败即拒绝；
            // 解析失败只影响后续的业务处理，不影响鉴权结论。
            let client_ip = event.get("ip").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(resp) = check_hook_auth(state, &event, client_ip, query).await {
                return resp;
            }
            if let Some(data) = serde_json::from_value::<PlayData>(event.clone()).ok() {
                tracing::info!("on_play: {}/{}/{} from {}",
                    data.schema, data.app, data.stream, data.ip);
                // 从 stream_id 解析设备/通道（格式：device_id_channel_id 或 device_id$channel_id）
                if let Some((device_id, channel_id)) = parse_stream_id(&data.stream) {
                    // 播放鉴权（预留）
                    tracing::debug!("on_play: device={} channel={}", device_id, channel_id);
                }
            }
        }
        "on_publish" => {
            // 鉴权必须先做、且与 body 解析无关（理由同 on_play：此前是 fail-open）
            let client_ip = event.get("ip").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(resp) = check_hook_auth(state, &event, client_ip, query).await {
                return resp;
            }
            if let Some(data) = serde_json::from_value::<PublishData>(event.clone()).ok() {
                tracing::info!("on_publish: {}/{}/{} from {}",
                    data.schema, data.app, data.stream, data.ip);
                // 验证推流来源 IP 是否与注册设备匹配
                if let Some((device_id, _channel_id)) = parse_stream_id(&data.stream) {
                    if let Some(ref sip_server) = state.sip_server {
                        let sip = sip_server.as_ref();
                        if let Some(addr) = sip.device_manager().get_address(&device_id).await {
                            // 如果设备注册了 IP 且不匹配，记录但不拒绝
                            if let Ok(parsed_ip) = data.ip.parse::<std::net::IpAddr>() {
                                if addr.ip() != parsed_ip {
                                    tracing::warn!(
                                        "on_publish IP mismatch: device={} registered={} actual={}",
                                        device_id,
                                        addr.ip(),
                                        data.ip
                                    );
                                }
                            }
                        }
                    }
                }
                register_published_stream(&state, &data).await;
            }
        }
        "on_server_started" => {
            if let Some(data) = serde_json::from_value::<ServerStartedData>(event.clone()).ok() {
                tracing::info!(
                    "ZLM server started: rtsp={} rtmp={} http={} https={}",
                    data.rtsp_port,
                    data.rtmp_port,
                    data.http_port,
                    data.https_port
                );
                // Reset media server status and reconfigure hooks
                let media_server_id = event
                    .get("mediaServerId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("zlmediakit-1");

                // Phase 4.2: 重置节点状态（on_server_started 时）
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let _ = crate::db::media_server::update_ports(
                    &state.pool,
                    media_server_id,
                    data.http_port as i32,
                    Some(data.http_port as i32),
                    Some(data.rtsp_port as i32),
                    Some(data.rtmp_port as i32),
                    &now,
                )
                .await;
                // 重置流计数（在 on_server_started 时清零，避免旧数据残留）
                // Phase 7.1: route through StateStore (single source of truth).
                state.state_store.set_media_server(
                    media_server_id,
                    crate::state_store::MediaServerLoad {
                        server_id: media_server_id.to_string(),
                        stream_count: 0,
                        rtp_server_count: 0,
                        online: true,
                        last_keepalive: chrono::Utc::now(),
                    },
                );
                tracing::info!(
                    "ZLM node {} online: http={} rtsp={} rtmp={}",
                    media_server_id,
                    data.http_port,
                    data.rtsp_port,
                    data.rtmp_port
                );

                // Reconfigure ZLM hook URL if zlm_client is available
                if let Some(ref zlm_client) = state.zlm_client {
                    let hook_url = state
                        .config
                        .zlm
                        .as_ref()
                        .and_then(|cfg| cfg.servers.iter().find(|s| s.id == media_server_id))
                        .and_then(|sv| sv.hook_url.clone())
                        .unwrap_or_else(|| {
                            let server_port = state.config.server.port;
                            format!("http://127.0.0.1:{}/api/zlm/hook", server_port)
                        });

                    let secret = zlm_client.secret.clone();
                    // 每个事件用**各自的 URL**（真实 ZLM 靠 URL 区分事件，
                    // body 里没有 hook_name）。此前这里把所有 hook 都指向
                    // 同一个 hook_url，且 on_server_keepalive 缺失 ——
                    // ZLM 收到后无法区分事件，等于整套 hook 都不生效。
                    let config_items = crate::zlm::hook::hook_config_items(&hook_url, &secret);
                    for (key, value) in &config_items {
                        if let Err(e) =
                            zlm_client.set_server_config(&secret, key, value).await
                        {
                            tracing::warn!("Failed to set ZLM config {}={}: {}", key, value, e);
                        }
                    }
                    tracing::info!("ZLM hook URLs reconfigured for server {}", media_server_id);

                    // Phase 4.3: 自动同步 RTP 端口范围 + 协议开关到 ZLM
                    // 与 gb_media_server.rtp_port_range / send_rtp_port_range 对齐
                    match crate::db::media_server::get_media_server_by_id(
                        &state.pool, media_server_id,
                    ).await {
                        Ok(Some(server_config)) => {
                            // rtp.port_range（设备推送端口）
                            if let Some(ref rtp_range) = server_config.rtp_port_range {
                                match crate::zlm::client::set_rtp_port_range(
                                    zlm_client, &secret, "rtp.port_range", rtp_range,
                                ).await {
                                    Ok(()) => tracing::info!(
                                        "ZLM rtp.port_range set to {} for server {}",
                                        rtp_range, media_server_id,
                                    ),
                                    Err(e) => tracing::warn!(
                                        "Failed to set ZLM rtp.port_range={}: {}",
                                        rtp_range, e,
                                    ),
                                }
                            }
                            // send_rtp.port_range（推送上级平台端口）
                            if let Some(ref srtp_range) = server_config.send_rtp_port_range {
                                match crate::zlm::client::set_rtp_port_range(
                                    zlm_client, &secret, "send_rtp.port_range", srtp_range,
                                ).await {
                                    Ok(()) => tracing::info!(
                                        "ZLM send_rtp.port_range set to {} for server {}",
                                        srtp_range, media_server_id,
                                    ),
                                    Err(e) => tracing::warn!(
                                        "Failed to set ZLM send_rtp.port_range={}: {}",
                                        srtp_range, e,
                                    ),
                                }
                            }
                            // 协议开关（与 ZLM 默认对齐：全部启用）
                            for (key, value) in PROTOCOL_ENABLE_FLAGS {
                                if let Err(e) = zlm_client.set_server_config(
                                    &secret, key, value,
                                ).await {
                                    tracing::warn!(
                                        "Failed to set ZLM {}={}: {}",
                                        key, value, e,
                                    );
                                }
                            }
                            tracing::info!(
                                "ZLM node {} fully auto-configured (hooks + rtp ranges + protocols)",
                                media_server_id,
                            );
                        }
                        Ok(None) => {
                            tracing::warn!(
                                "Media server {} not found in DB; skipping RTP port range sync",
                                media_server_id,
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to query media_server {} for RTP config: {}",
                                media_server_id, e,
                            );
                        }
                    }
                }
            }
        }
        "on_server_keepalive" => {
            if let Some(data) = serde_json::from_value::<ServerKeepaliveData>(event.clone()).ok() {
                let server_id = data.media_server_id.as_deref().unwrap_or("zlmediakit-1");
                tracing::debug!("ZLM server keepalive: {}", server_id);
                // Update last keepalive time in DB
                let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                if let Err(e) =
                    crate::db::media_server::update_last_keepalive(&state.pool, server_id, &now)
                        .await
                {
                    tracing::error!("更新 ZLM 节点 {} 心跳时间失败: {}", server_id, e);
                }
            }
        }
        "on_rtp_server_started" => {
            // Phase 3.1: ZLM 成功开启 RTP Server，等待设备推流到达
            if let Some(data) = serde_json::from_value::<RtpServerStartedData>(event.clone()).ok() {
                // 流 ID 就是 RTP server 对应的 stream_id
                let stream_id = &data.stream_id;
                tracing::info!("ZLM RTP server started: stream_id={}", stream_id);
                // 通过 stream_id 反查 call_id，触发 MediaWaiter
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.as_ref();
                    let resolved = sip.notify_media_ready_by_stream(stream_id, "rtp").await;
                    if resolved {
                        tracing::info!("MediaWaiter resolved for stream_id={}", stream_id);
                    }
                }
            }
        }
        "on_stream_started" => {
            // Phase 3.1: 设备推流到达（流正式开始）
            if let Some(data) = serde_json::from_value::<StreamChangedData>(event.clone()).ok() {
                tracing::info!("Stream started: app={} stream={}", data.app, data.stream);
                // 通知 media waiter（通过 call_id 或 stream_id）
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.as_ref();
                    let resolved = sip
                        .notify_media_ready_by_stream(&data.stream, &data.app)
                        .await;
                    if resolved {
                        tracing::info!(
                            "MediaWaiter resolved for stream={}/{}",
                            data.app,
                            data.stream
                        );
                    }
                }
                // 广播 WebSocket 事件
                state
                    .ws_state
                    .broadcast(
                        "streamStarted",
                        serde_json::json!({
                            "app": data.app,
                            "stream": data.stream,
                            "schema": data.schema,
                        }),
                    )
                    .await;
            }
        }

        "on_rtp_server_timeout" => {
            if let Some(data) = serde_json::from_value::<RtpServerStartedData>(event.clone()).ok() {
                tracing::info!(
                    "RTP server timeout: stream_id={} server={}",
                    data.stream_id,
                    data.media_server_id.as_deref().unwrap_or("unknown")
                );
                // Clean up InviteSession associated with this stream
                if let Some(ref sip_server) = state.sip_server {
                    let sip = sip_server.as_ref();
                    // Parse device_id/channel_id from stream_id
                    if let Some((device_id, channel_id)) = parse_stream_id(&data.stream_id) {
                        if let Ok(_) = sip.send_session_bye(&device_id, &channel_id).await {
                            tracing::info!(
                                "Sent BYE for timed-out RTP session {}/{}",
                                device_id,
                                channel_id
                            );
                        }
                    }
                }
            }
        }
        "on_flow_report" => {
            // 流量统计（每个连接断开时触发一次，totalBytes 是该连接的流量）
            let total_traffic = event
                .get("totalBytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let media_server_id = event
                .get("mediaServerId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            // **不要**从载荷里读流数量：真实 ZLM 的 `on_flow_report` body 里
            // 根本没有流数量字段（官方示例只有 mediaServerId/app/duration/
            // params/player/schema/stream/totalBytes/vhost/ip/port/id）。
            // 此前读 `event["streams"]` 恒为 0，于是**每次有播放器或推流端断开**
            // 都会把该节点的流数量写成 0 —— 负载均衡（按 stream_count 选最少负载
            // 节点）因此长期把每个节点都看成空节点。
            //
            // 想拿绝对数量就直接问 ZLM 的 getMediaList；问不到就**保持原值**
            // （不动 StateStore，也不覆盖 DB 列）。
            let streams: Option<i64> = match state.get_zlm_client(Some(media_server_id)) {
                Some(client) => match client.get_media_list(None, None, None).await {
                    Ok(list) => Some(list.len() as i64),
                    Err(e) => {
                        tracing::warn!(
                            "flow report: getMediaList 失败，保持既有流数量 server={}: {}",
                            media_server_id,
                            e
                        );
                        None
                    }
                },
                None => None,
            };

            tracing::debug!(
                "Flow report: server={} totalBytes={} streams={:?}",
                media_server_id,
                total_traffic,
                streams
            );

            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let _ = crate::db::media_server::update_flow_stats(
                &state.pool,
                media_server_id,
                total_traffic as i64,
                streams.map(|n| n as i32),
                &now,
            )
            .await;
            if let Some(n) = streams {
                sync_media_server_stream_count(&state.state_store, media_server_id, n);
            }
        }
        "on_stream_none_reader" => {
            // 这是**唯一**能影响 ZLM 行为的"资源回收"事件：
            // 官方文档 `[hook] on_stream_none_reader` 一节写明
            // 「无人观看流事件，通过该事件，可以选择是否关闭无人观看的流」，
            // 响应体为顶层 `{code:0, close:true|false}`。
            //
            // 此前这里只打日志 + 广播，既不改 ZLM 决策（`close` 压根没返回），
            // 也不向设备发 BYE，注释却写着 "Auto-stop idle streams" ——
            // 等于一个假的资源回收：没人看的国标流会一直占着 ZLM 收流端口，
            // 设备也一直往平台上推。
            match serde_json::from_value::<StreamNoneReaderData>(event.clone()) {
                Ok(data) => {
                    tracing::info!(
                        "Stream no readers: {}/{}/{}",
                        data.schema,
                        data.app,
                        data.stream
                    );

                    let decision = decide_idle_stream(state, &data).await;
                    tracing::info!(
                        "on_stream_none_reader {} → close={} ({})",
                        data.stream,
                        decision.close,
                        decision.reason
                    );

                    state
                        .ws_state
                        .broadcast(
                            "streamNoneReader",
                            serde_json::json!({
                                "app": data.app,
                                "stream": data.stream,
                                "schema": data.schema,
                                "close": decision.close,
                                "reason": decision.reason,
                            }),
                        )
                        .await;

                    if decision.close {
                        close_idle_stream(state, &data, &decision).await;
                    }

                    return Json(none_reader_response(&decision));
                }
                Err(e) => {
                    // 解析失败必须**可见**：此前静默 `.ok()` + `if let` 的组合
                    // 让"载荷字段名对不上"变成完全无声的功能缺失。
                    tracing::warn!("on_stream_none_reader 载荷解析失败: {} 原文={}", e, event);
                }
            }
        }
        "on_send_rtp_stopped" => {
            // Phase 4.1: SendRtp 停止通知（级联平台关闭推流）
            // Phase 5.4: 按 stream 路由到 SendRtpManager 关闭对应 session
            let data: SendRtpStoppedData =
                serde_json::from_value(event.clone()).unwrap_or_default();
            let stream = data.stream.clone().unwrap_or_default();
            let ssrc = data.ssrc.clone().unwrap_or_default();
            tracing::info!(
                "SendRTP stopped: {}/{} ssrc={}",
                data.app.as_deref().unwrap_or(""),
                stream,
                ssrc
            );
            // 5.4: 关闭 SendRtpManager 中匹配的 cascade session。
            // 先按 stream，再按 ssrc 兜底（载荷字段集随 ZLM 版本而异）。
            if let Some(ref sip_server) = state.sip_server {
                let sip = sip_server.as_ref();
                let closed = if stream.is_empty() {
                    None
                } else {
                    sip.send_rtp_manager().close_by_stream(&stream)
                };
                let closed = match closed {
                    Some(session) => Some(session),
                    None => sip.send_rtp_manager().close_by_ssrc(&ssrc),
                };
                match closed {
                    Some(session) => tracing::info!(
                        "5.4 on_send_rtp_stopped → closed cascade session platform={} channel={} stream={}",
                        session.platform_id,
                        session.channel_id,
                        session.upstream_ssrc
                    ),
                    None => tracing::warn!(
                        "on_send_rtp_stopped: 未找到匹配的级联会话 stream={} ssrc={}（可能已清理）",
                        stream,
                        ssrc
                    ),
                }
            }
            // 广播级联停止事件
            state
                .ws_state
                .broadcast(
                    "sendRtpStopped",
                    serde_json::json!({
                        "app": data.app,
                        "stream": stream,
                        "ssrc": ssrc,
                        "schema": data.schema,
                    }),
                )
                .await;
        }
        "on_record_file" => {
            // Phase 4.1: MP4 录像文件落盘通知
            if let Some(data) = serde_json::from_value::<RecordMp4Data>(event.clone()).ok() {
                tracing::info!(
                    "MP4 recording complete: file={} duration={}s",
                    data.file_path.as_str(),
                    data.file_duration
                );
                // 同步录像文件信息到 DB（cloud_record）
                let duration = data.file_duration as i64;
                let _ = crate::db::cloud_record::insert_from_hook(
                    &state.pool,
                    &data.stream,
                    &data.file_path,
                    duration,
                )
                .await;
            }
        }
        // ============ ABL 钩子（设计文档 §6.3 阶段 0 缺口 1）============
        "on_rtp_playlist" => {
            if let Some(data) = serde_json::from_value::<RtpPlaylistData>(event.clone()).ok() {
                tracing::info!(
                    "ABL rtp_playlist: app={:?} stream={:?} ssrc={:?} action={:?}",
                    data.app, data.stream, data.ssrc, data.action
                );
                // 通知 WS 订阅者（用于前端展示 playlist 状态）
                state.ws_state.broadcast("abl_rtp_playlist", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "ssrc": data.ssrc,
                    "action": data.action,
                })).await;
            }
        }
        "on_record_progress" => {
            if let Some(data) = serde_json::from_value::<RecordProgressData>(event.clone()).ok() {
                tracing::debug!(
                    "ABL record_progress: {}/{} duration={:?}s size={:?}B ts={:?}",
                    data.app.as_deref().unwrap_or(""),
                    data.stream.as_deref().unwrap_or(""),
                    data.current_duration, data.current_size, data.progress_ts
                );
                // 更新 DB 中的录像进度（cloud_record），供前端轮询
                if let (Some(app), Some(stream)) = (data.app.as_deref(), data.stream.as_deref()) {
                    let _ = crate::db::cloud_record::update_recording_progress(
                        &state.pool,
                        stream,
                        app,
                        data.current_duration.unwrap_or(0.0),
                        data.current_size.unwrap_or(0),
                    ).await;
                }
            }
        }
        "on_send_rtp_progress" => {
            if let Some(data) = serde_json::from_value::<SendRtpProgressData>(event.clone()).ok() {
                tracing::debug!(
                    "ABL send_rtp_progress: {}/{} total={:?} bytes={:?}",
                    data.app.as_deref().unwrap_or(""),
                    data.stream.as_deref().unwrap_or(""),
                    data.total_sent, data.bytes_sent
                );
                // 级联推流进度（如不需要可关闭日志），用于监控推流质量
                state.ws_state.broadcast("abl_send_rtp_progress", serde_json::json!({
                    "app": data.app,
                    "stream": data.stream,
                    "total_sent": data.total_sent,
                    "bytes_sent": data.bytes_sent,
                })).await;
            }
        }
        _ => {
            tracing::debug!("Unhandled webhook: {}", hook_name);
        }
    }

    Json(hook_ok_response())
}

/// Phase 4.2: hook 鉴权（secret + IP 白名单）
///
/// 返回 `Some(Json)` 表示鉴权失败，调用方应直接 return。
/// 返回 `None` 表示通过（放行或白名单为空）。
async fn check_hook_auth(
    state: &AppState,
    event: &serde_json::Value,
    client_ip_str: &str,
    query: Option<&str>,
) -> Option<Json<serde_json::Value>> {
    use crate::zlm::auth::HookAuthChecker;

    // 优先用 URL 查询参数里的 secret（真实 ZLM 经 `admin_params` 这样传），
    // 其次才看 body 里的 `secret`（兼容手工测试与部分魔改实现）。
    let provided_secret = match secret_from_query(query) {
        Some(s) if !s.is_empty() => s,
        _ => event
            .get("secret")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    };
    let provided_secret = provided_secret.as_str();

    let media_server_id = event
        .get("mediaServerId")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 节点 expected_secret：优先用 in-memory ZLM client，回退到 DB
    let expected_secret: String = if !media_server_id.is_empty() {
        if let Some(client) = state.get_zlm_client(Some(media_server_id)) {
            client.secret.clone()
        } else {
            // 兜底：从 DB 查 secret
            crate::db::media_server::get_media_server_by_id(&state.pool, media_server_id)
                .await
                .ok()
                .flatten()
                .and_then(|s| s.secret)
                .unwrap_or_default()
        }
    } else {
        // 没有 mediaServerId，尝试用默认节点
        state
            .zlm_client
            .as_ref()
            .map(|c| c.secret.clone())
            .unwrap_or_default()
    };

    // 加载白名单 CIDR（按 media_server_id）
    let cidrs_str: Vec<String> = if !media_server_id.is_empty() {
        crate::db::media_server::get_white_list_cidrs(&state.pool, media_server_id)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // 解析 CIDR 字符串为 IpNetwork，解析失败记 warn 并忽略
    let cidrs: Vec<ipnetwork::IpNetwork> = cidrs_str
        .iter()
        .filter_map(|s| match s.parse::<ipnetwork::IpNetwork>() {
            Ok(net) => Some(net),
            Err(e) => {
                tracing::warn!("Invalid CIDR in whitelist '{}': {}", s, e);
                None
            }
        })
        .collect();

    let checker = HookAuthChecker::new(&expected_secret).with_whitelist(cidrs);

    // 顺序很重要：**先校验 secret，再校验 IP 白名单**。
    //
    // 此前是"解析不出客户端 IP 就直接拒绝"，于是当 body 里没有 `ip` 字段时，
    // 返回的是 "invalid client IP" —— 既掩盖了真正的结论（secret 对不对），
    // 也让"secret 正确但载荷缺 ip"的合法请求被拒。secret 是主控制项，
    // 白名单是附加项：主控制项通过后再看白名单，且只在**拿得到 IP** 时才校验
    // （白名单为空时 `check_ip` 本就返回 true）。
    if !checker.check_secret(provided_secret) {
        tracing::warn!(
            "hook auth: secret mismatch from '{}' (server={})",
            client_ip_str,
            media_server_id
        );
        return Some(Json(hook_error_response("Unauthorized: secret mismatch")));
    }

    if !client_ip_str.is_empty() {
        match client_ip_str.parse::<std::net::IpAddr>() {
            Ok(client_ip) => {
                if !checker.check_ip(&client_ip) {
                    tracing::warn!(
                        "hook auth: IP {} not in whitelist (server={})",
                        client_ip,
                        media_server_id
                    );
                    return Some(Json(hook_error_response(
                        "Unauthorized: IP not allowed",
                    )));
                }
            }
            Err(_) => {
                tracing::warn!("hook auth: unparseable client IP '{}'", client_ip_str);
                return Some(Json(hook_error_response(
                    "Unauthorized: invalid client IP",
                )));
            }
        }
    }

    // 走到这里：secret 通过，且（若有 IP）白名单也通过
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============== ZLM 响应契约 ==============
    //
    // ZLMediaKit 直接读**响应顶层**的字段：
    //   * `code`  —— 0 放行 / 非 0 拒绝（on_publish / on_play 鉴权结论）
    //   * `close` —— on_stream_none_reader 是否关闭该无人流
    //
    // 本项目此前把所有 hook 都包成 WVP 信封
    // `{"code":0,"msg":"成功","data":{...}}`：`code` 恰好在顶层所以鉴权
    // 看起来正常，但 `close` 被埋进 `data`，ZLM 永远读不到。

    #[test]
    fn hook_responses_are_flat_for_zlm() {
        let ok = hook_ok_response();
        assert_eq!(ok["code"], 0);
        assert!(
            ok.get("data").is_none(),
            "ZLM 不识 WVP 信封，响应不能带 data 包装"
        );

        let err = hook_error_response("Unauthorized: secret mismatch");
        assert_ne!(err["code"], 0, "鉴权失败必须返回非 0 顶层 code");
        assert!(err.get("data").is_none());
    }

    #[test]
    fn none_reader_response_exposes_close_at_top_level() {
        let mut decision = IdleStreamDecision::keep("无人观看，回收资源");
        decision.close = true;
        let v = none_reader_response(&decision);
        assert_eq!(v["code"], 0);
        assert_eq!(v["close"], true, "close 必须在顶层，ZLM 才读得到");
        assert!(v.get("data").is_none());

        let keep = none_reader_response(&IdleStreamDecision::keep("正在录像"));
        assert_eq!(keep["close"], false);
        assert_eq!(keep["msg"], "正在录像");
    }

    #[test]
    fn idle_decision_keep_never_closes() {
        for reason in ["非国标流（推流/拉流代理），交由用户停止", "ZLM 正在录像"] {
            let d = IdleStreamDecision::keep(reason);
            assert!(!d.close);
            assert_eq!(d.reason, reason);
            assert!(d.device_id.is_none() && d.channel_id.is_none());
        }
    }

    /// 本平台自己的流名是 `{device}_{channel}`（下划线），
    /// 解析必须认这个格式 —— 否则按需拉流/无人观看关流对自家流全部失效。
    #[test]
    fn parse_stream_id_handles_platform_stream_names() {
        let (d, c) = parse_stream_id("34020000001320000001_34020000001320000002").unwrap();
        assert_eq!(d, "34020000001320000001");
        assert_eq!(c, "34020000001320000002");
        // 推流/代理流不属于国标流
        assert!(parse_stream_id("push_stream1").is_none());
        assert!(parse_stream_id("proxy_abc").is_none());
        assert!(parse_stream_id("plainstream").is_none());
    }

    #[test]
    fn test_parse_stream_id_dollar() {
        let s = "34020000002000000001$101";
        let res = parse_stream_id(s);
        assert!(res.is_some());
        let (d, c) = res.unwrap();
        assert_eq!(d, "34020000002000000001");
        assert_eq!(c, "101");
    }

    #[test]
    fn test_parse_stream_id_slash() {
        let s = "device/channel";
        let res = parse_stream_id(s);
        assert!(res.is_some());
        let (d, c) = res.unwrap();
        assert_eq!(d, "device");
        assert_eq!(c, "channel");
    }

    #[test]
    fn test_parse_record_time_ms_unix() {
        let v = "1620000000"; // seconds
        let ms = parse_record_time_ms(v);
        assert!(ms >= 1620000000 * 1000);
    }

    // ============== ZlmHookEvent 解析测试（Phase 4.1） ==============

    #[test]
    fn test_zlm_hook_event_parse_stream_changed() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_changed"),
            ZlmHookEvent::StreamChanged
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_publish_play() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_publish"),
            ZlmHookEvent::Publish
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_play"),
            ZlmHookEvent::Play
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_server_events() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_server_started"),
            ZlmHookEvent::ServerStarted
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_server_keepalive"),
            ZlmHookEvent::ServerKeepalive
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_stream_lifecycle() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_not_found"),
            ZlmHookEvent::StreamNotFound
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_none_reader"),
            ZlmHookEvent::StreamNoneReader
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_stream_started"),
            ZlmHookEvent::StreamStarted
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_rtp_events() {
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_rtp_server_started"),
            ZlmHookEvent::RtpServerStarted
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_rtp_server_timeout"),
            ZlmHookEvent::RtpServerTimeout
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_send_rtp_stopped"),
            ZlmHookEvent::SendRtpStopped
        );
    }

    #[test]
    fn test_zlm_hook_event_parse_record_aliases() {
        // 兼容两个常见 hook 名
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_record_mp4"),
            ZlmHookEvent::RecordMp4
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_record_file"),
            ZlmHookEvent::RecordMp4
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_record_progress"),
            ZlmHookEvent::RecordProgress
        );
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_flow_report"),
            ZlmHookEvent::FlowReport
        );
    }

    #[test]
    fn test_zlm_hook_event_unknown_and_default_response() {
        // 未知 hook 名称 → Unknown
        assert_eq!(
            ZlmHookEvent::from_hook_name("on_something_made_up"),
            ZlmHookEvent::Unknown
        );
        // default_response 始终是 WVP-Pro 兼容的成功结构
        let resp = ZlmHookEvent::StreamChanged.default_response();
        assert_eq!(resp["code"], 0);
        assert_eq!(resp["msg"], "success");
        // 同样适用于 Unknown（保持前端可正常处理）
        let resp_unknown = ZlmHookEvent::Unknown.default_response();
        assert_eq!(resp_unknown["code"], 0);
    }

    // ============== Phase 4.3: set_server_config wiremock 集成测试 ==============
    //
    // 验证 `on_server_started` 自动配置循环中，所有 `set_server_config` 调用
    // （hook.enable / hook.on_* / rtp.port_range / send_rtp.port_range /
    //  protocol.enable_*）实际以正确的 payload 命中 ZLM HTTP API。
    //
    // 由于完整 on_server_started handler 需要 AppState（DB pool / Redis 等），
    // 这里只测底层 `ZlmClient::set_server_config` 的端到端 HTTP 行为，hook.rs
    // 调用端与 client.rs 通过同一方法对接。

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_on_server_started_auto_configures_rtp_port_range() {
        // 1. 启动 wiremock 模拟 ZLM HTTP 服务
        let mock_server = MockServer::start().await;

        // 2. 注册 setServerConfig 端点：返回 code=0
        Mock::given(method("POST"))
            .and(path("/index/api/setServerConfig"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": 0
            })))
            .expect(1..) // 至少 1 次（实际会更多：hooks + rtp + protocols）
            .mount(&mock_server)
            .await;

        // 3. 构造 ZlmClient，指向 mock server
        let uri = mock_server.uri();
        // uri 形如 "http://127.0.0.1:PORT"
        let stripped = uri.trim_start_matches("http://");
        let mut parts = stripped.splitn(2, ':');
        let ip = parts.next().unwrap_or("127.0.0.1").to_string();
        let port: u16 = parts
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(80);

        let zlm_client = crate::zlm::ZlmClient::new(&ip, port, "test-secret");

        // 4. 模拟 on_server_started 中的 3 类 set_server_config 调用
        let secret = "test-secret";

        // (a) hook URL（已有的 Phase 4.1 行为）
        zlm_client
            .set_server_config(secret, "hook.enable", "1")
            .await
            .expect("set_server_config hook.enable");

        // (b) rtp.port_range —— set_rtp_port_range helper handles comma→dash conversion
        crate::zlm::client::set_rtp_port_range(
            &zlm_client, secret, "rtp.port_range", "30000,30200",
        )
        .await
        .expect("set_rtp_port_range rtp.port_range");

        // (c) send_rtp.port_range
        let srtp_value = format!("{}-{}", 40000, 40200);
        zlm_client
            .set_server_config(secret, "send_rtp.port_range", &srtp_value)
            .await
            .expect("set_server_config send_rtp.port_range");

        // (d) 协议开关
        for (key, value) in PROTOCOL_ENABLE_FLAGS {
            zlm_client
                .set_server_config(secret, key, value)
                .await
                .expect("set_server_config protocol flag");
        }

        // 5. 验证 mock server 收到了所有调用（>= 9 次：1 hook + 2 rtp + 6 protocols）
        let received = mock_server.received_requests().await.unwrap_or_default();
        assert!(
            received.len() >= 9,
            "expected at least 9 setServerConfig calls, got {}",
            received.len()
        );

        // 6. 验证每个关键 key 都被正确设置（key 字段在 wiremock 这里我们从 body 解析）
        let mut found_rtp_port_range = false;
        let mut found_send_rtp_port_range = false;
        let mut protocol_flags = std::collections::HashSet::new();
        for req in &received {
            let body = String::from_utf8_lossy(&req.body).to_string();
            if body.contains("\"key\":\"rtp.port_range\"") && body.contains("30000-30200") {
                found_rtp_port_range = true;
            }
            if body.contains("\"key\":\"send_rtp.port_range\"") && body.contains("40000-40200") {
                found_send_rtp_port_range = true;
            }
            for flag in [
                "protocol.enable_rtsp",
                "protocol.enable_rtmp",
                "protocol.enable_hls",
                "protocol.enable_http",
                "protocol.enable_ws",
                "protocol.enable_rtp",
            ] {
                if body.contains(&format!("\"key\":\"{}\"", flag)) {
                    protocol_flags.insert(flag.to_string());
                }
            }
        }
        assert!(found_rtp_port_range, "rtp.port_range=30000-30200 not seen in requests");
        assert!(
            found_send_rtp_port_range,
            "send_rtp.port_range=40000-40200 not seen in requests"
        );
        assert_eq!(
            protocol_flags.len(),
            6,
            "expected 6 protocol.enable_* flags, saw {:?}",
            protocol_flags
        );
    }

    // ============== on_flow_report → StateStore 同步（2026-09-11 cache 迁移） ==============

    /// 构造一个带 `MediaServerLoad` 的内存 StateStore
    fn store_with(id: &str, stream_count: i64, rtp_server_count: i32, online: bool) -> crate::state_store::StateStore {
        let store = crate::state_store::StateStore::in_memory();
        store.set_media_server(
            id,
            crate::state_store::MediaServerLoad {
                server_id: id.to_string(),
                stream_count,
                rtp_server_count,
                online,
                last_keepalive: chrono::Utc::now(),
            },
        );
        store
    }

    #[test]
    fn test_flow_report_creates_entry_for_unknown_server() {
        let store = crate::state_store::StateStore::in_memory();
        sync_media_server_stream_count(&store, "zlm-new", 7);
        let load = store.get_media_server("zlm-new").expect("entry should be created");
        assert_eq!(load.stream_count, 7);
        assert_eq!(load.server_id, "zlm-new");
        assert!(load.online, "刚上报过 flow report 的节点应视为在线");
    }

    #[test]
    fn test_flow_report_overwrites_stream_count_with_absolute_value() {
        // `sync_media_server_stream_count` 的语义是"写入绝对值"（调用方
        // 现在从 ZLM getMediaList 取真实流数，而不是从 flow report 载荷里读）
        let store = store_with("zlm-a", 10, 3, true);
        sync_media_server_stream_count(&store, "zlm-a", 4);
        let load = store.get_media_server("zlm-a").unwrap();
        assert_eq!(load.stream_count, 4, "应被绝对计数覆盖");
        assert_eq!(load.rtp_server_count, 3, "不应触碰 rtp_server_count");
    }

    #[test]
    fn test_flow_report_corrects_drift_downward() {
        // 绝对写入可以纠正 on_stream_changed 增减路径累积的漂移
        // （例如漏收 unregister）
        let store = store_with("zlm-a", 99, 0, true);
        sync_media_server_stream_count(&store, "zlm-a", 0);
        assert_eq!(store.get_media_server("zlm-a").unwrap().stream_count, 0);
    }

    #[test]
    fn test_flow_report_preserves_existing_offline_flag() {
        // 关键回归保护：不能因为收到 flow report 就把已判离线的节点重新置为 online，
        // 否则内存后端的 `media_server_select_least_loaded_filtered`（按 online 过滤）
        // 会重新把该节点纳入候选。
        let store = store_with("zlm-a", 2, 0, false);
        sync_media_server_stream_count(&store, "zlm-a", 5);
        let load = store.get_media_server("zlm-a").unwrap();
        assert_eq!(load.stream_count, 5, "计数仍应更新");
        assert!(!load.online, "已有条目的 online 不应被 flow report 覆盖");
    }
}

#[cfg(test)]
mod hook_config_tests {
    use super::*;

    #[test]
    fn hook_base_url_normalizes_known_shapes() {
        assert_eq!(
            hook_base_url("http://127.0.0.1:18080/api/zlm/hook"),
            "http://127.0.0.1:18080"
        );
        assert_eq!(
            hook_base_url("http://127.0.0.1:18080/api/hook/on_play"),
            "http://127.0.0.1:18080"
        );
        assert_eq!(
            hook_base_url("http://example.com:9000/"),
            "http://example.com:9000"
        );
        assert_eq!(
            hook_base_url("http://example.com:9000"),
            "http://example.com:9000"
        );
    }

    /// 真实 ZLM 经 `[hook] admin_params` 把 secret 作为 URL 查询参数附加，
    /// body 里没有它。此前只从 body 读 → 所有带鉴权的 on_publish / on_play
    /// 一律 "secret mismatch"，ZLM 因此拒绝一切推流与播放。
    #[test]
    fn secret_is_read_from_query_string() {
        assert_eq!(
            secret_from_query(Some("secret=abc123")).as_deref(),
            Some("abc123")
        );
        // 与其它参数混排
        assert_eq!(
            secret_from_query(Some("foo=1&secret=S%2B%2F%3D&bar=2")).as_deref(),
            Some("S+/=")
        );
        // `+` 视作空格（表单编码习惯）
        assert_eq!(
            secret_from_query(Some("secret=a+b")).as_deref(),
            Some("a b")
        );
        assert_eq!(secret_from_query(Some("other=1")), None);
        assert_eq!(secret_from_query(Some("")), None);
        assert_eq!(secret_from_query(None), None);
        // 键名必须完全匹配，不能把 `mysecret=` 当成 secret
        assert_eq!(secret_from_query(Some("mysecret=x")), None);
    }

    #[test]
    fn percent_decode_handles_malformed_input() {
        assert_eq!(percent_decode("abc"), "abc");
        assert_eq!(percent_decode("%41%42"), "AB");
        // 截断的百分号序列原样保留，不 panic
        assert_eq!(percent_decode("a%4"), "a%4");
        assert_eq!(percent_decode("a%zz"), "a%zz");
    }

    #[test]
    fn hook_event_url_appends_api_hook_path() {
        assert_eq!(
            hook_event_url("http://h:1", "on_play"),
            "http://h:1/api/hook/on_play"
        );
        // 结尾多余斜杠不应产生空段
        assert_eq!(
            hook_event_url("http://h:1/", "on_publish"),
            "http://h:1/api/hook/on_publish"
        );
    }

    /// 每个事件都必须拿到**不同**的 URL —— 真实 ZLM 靠 URL 区分事件，
    /// 共用同一个地址会让 body（无 hook_name）无法判别类型。
    #[test]
    fn hook_config_items_use_distinct_per_event_urls() {
        let items = hook_config_items("http://127.0.0.1:18080/api/zlm/hook", "s3cr3t");
        assert_eq!(items[0], ("hook.enable".to_string(), "1".to_string()));
        // admin_params 必须带上 secret —— ZLM 靠它把 secret 附加到 hook URL 上
        assert!(
            items
                .iter()
                .any(|(k, v)| k == "hook.admin_params" && v == "secret=s3cr3t"),
            "必须配置 hook.admin_params=secret=<node secret>"
        );
        // enable + admin_params + timeoutSec + 每个事件 1 项
        assert_eq!(items.len(), 3 + CONFIGURED_HOOK_EVENTS.len());

        let mut urls: Vec<&String> = items
            .iter()
            .filter(|(k, _)| k.starts_with("hook.on_"))
            .map(|(_, v)| v)
            .collect();
        let total = urls.len();
        urls.sort();
        urls.dedup();
        assert_eq!(urls.len(), total, "每个事件的回调 URL 必须互不相同");

        // 只校验事件项（`hook.on_*`）；enable / admin_params / timeoutSec
        // 不是 URL，不参与该断言。
        for (key, value) in &items {
            if let Some(event) = key.strip_prefix("hook.on_") {
                assert!(
                    value.ends_with(&format!("/api/hook/on_{}", event)),
                    "{} 的 URL 应以 /api/hook/on_{} 结尾，实际 {}",
                    key,
                    event,
                    value
                );
            }
        }
    }
}
