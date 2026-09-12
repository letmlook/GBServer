//! 推流 /api/push 与拉流代理 /api/proxy，对应前端 streamPush.js / streamProxy.js

use axum::{
    extract::{Query, State, Multipart},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::db::{stream_push, stream_proxy, StreamPush, StreamProxy};
use crate::error::{AppError, ErrorCode};
use crate::response::WVPResult;
use crate::zlm::OpenRtpServerRequest;

use crate::AppState;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct PushListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub pushing: Option<String>,
    pub mediaServerId: Option<String>,
}

/// GET /api/push/list
pub async fn push_list(
    State(state): State<AppState>,
    Query(q): Query<PushListQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let pushing = q.pushing.as_deref().and_then(|s| s.parse().ok());
    // 关键字过滤此前被 DTO 收下却从未使用（页面按签名传 query 会静默忽略）
    let query = q.query.as_deref().filter(|s| !s.trim().is_empty());
    let total = stream_push::count_all(&state.pool, q.mediaServerId.as_deref(), pushing, query).await?;
    let list = stream_push::list_paged(
        &state.pool,
        page,
        count,
        q.mediaServerId.as_deref(),
        pushing,
        query,
    )
    .await?;
    // 每行补一个**推流地址**：WVP 的表里没有 url 列，前端"源 URL"那一列
    // 在我们这里应该展示"往哪里推"（rtmp://<媒体节点>:1935/<app>/<stream>）。
    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|p| {
            let mut v = serde_json::to_value(p).unwrap_or(serde_json::Value::Null);
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "pushUrl".to_string(),
                    serde_json::json!(build_push_url(&state, p)),
                );
            }
            v
        })
        .collect();
    Ok(Json(WVPResult::success(serde_json::json!({
        "total": total,
        "list": rows,
        "page": page,
        "size": count,
    }))))
}

/// 推流地址：`rtmp://<媒体节点 ip>:1935/<app>/<stream>`。
///
/// 媒体节点 ip 取配置里该节点的 ip，取不到就用默认 ZLM 客户端的 ip。
fn build_push_url(state: &AppState, p: &StreamPush) -> String {
    let ip = p
        .media_server_id
        .as_deref()
        .and_then(|id| {
            state
                .config
                .zlm
                .as_ref()
                .and_then(|cfg| cfg.servers.iter().find(|s| s.id == id))
                .map(|s| s.ip.clone())
        })
        .or_else(|| state.zlm_client.as_ref().map(|c| c.ip.clone()))
        .unwrap_or_else(|| "127.0.0.1".to_string());
    format!(
        "rtmp://{}:1935/{}/{}",
        ip,
        p.app.as_deref().unwrap_or("push"),
        p.stream.as_deref().unwrap_or("")
    )
}

#[derive(Debug, serde::Serialize)]
pub struct PushListPage {
    pub total: u64,
    pub list: Vec<StreamPush>,
    pub page: u64,
    pub size: u64,
}

/// POST /api/push/add 请求体
#[derive(Debug, Deserialize)]
pub struct PushAddBody {
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

/// POST /api/push/add
pub async fn push_add(
    State(state): State<AppState>,
    Json(body): Json<PushAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let app = body.app.unwrap_or_else(|| "push".to_string());
    let stream = body.stream.unwrap_or_default();
    let media_server_id = body.media_server_id.unwrap_or_default();
    
    if stream.is_empty() {
        return Ok(Json(WVPResult::error("Stream ID is required")));
    }
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    match stream_push::add(&state.pool, &app, &stream, &media_server_id, &now).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "app": app,
                "stream": stream,
                "mediaServerId": media_server_id,
                "message": "Push stream added successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to add push stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}

/// POST /api/push/update 请求体
#[derive(Debug, Deserialize)]
pub struct PushUpdateBody {
    pub id: Option<i64>,
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

/// POST /api/push/update
pub async fn push_update(
    State(state): State<AppState>,
    Json(body): Json<PushUpdateBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::error("Push stream ID is required")));
    }
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    match stream_push::update(
        &state.pool,
        id,
        body.app.as_deref(),
        body.stream.as_deref(),
        body.media_server_id.as_deref(),
        &now,
    ).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "id": id,
                "message": "Push stream updated successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to update push stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}

/// POST /api/push/remove 请求体 / 查询参数（两种都接受）
#[derive(Debug, Deserialize)]
pub struct PushRemoveBody {
    pub id: Option<i64>,
}

/// POST /api/push/remove
pub async fn push_remove(
    State(state): State<AppState>,
    Query(body): Query<PushRemoveBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::error("Push stream ID is required")));
    }
    
    // Get the push stream info first to close ZLM connection
    if let Ok(Some(push)) = stream_push::get_by_id(&state.pool, id as i64).await {
        if push.pushing.unwrap_or(false) {
            let media_server_id = push.media_server_id.as_ref();
            if let Some(zlm_client) = state.get_zlm_client(media_server_id.map(|s| s.as_str())) {
                if let Some(stream) = &push.stream {
                    if let Err(e) = zlm_client.close_rtp_server(stream).await {
                        tracing::warn!("Failed to close RTP server: {}", e);
                    }
                }
            }
        }
    }
    
    match stream_push::delete_by_id(&state.pool, id as i64).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "id": id,
                "message": "Push stream removed successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to remove push stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}

/// POST /api/push/start 请求体
#[derive(Debug, Deserialize)]
pub struct PushStartBody {
    pub id: Option<i64>,
    pub stream: Option<String>,
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
    #[serde(alias = "useTcp")]
    pub use_tcp: Option<bool>,
}

/// POST /api/push/start
pub async fn push_start(
    State(state): State<AppState>,
    Query(body): Query<PushStartBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    let stream_id = body.stream.clone().unwrap_or_default();
    
    // Get push info from DB or use provided values
    let (push_stream, media_server_id) = if id > 0 {
        match stream_push::get_by_id(&state.pool, id as i64).await {
            Ok(Some(push)) => {
                let ms_id = push.media_server_id.clone().unwrap_or_default();
                let s = push.stream.clone().unwrap_or_default();
                (Some(s), Some(ms_id))
            }
            Ok(None) => (None, None),
            Err(e) => {
                tracing::error!("Failed to get push info: {}", e);
                return Ok(Json(WVPResult::error("Database error")));
            }
        }
    } else {
        (Some(stream_id.clone()), body.media_server_id.clone())
    };
    
    let stream = push_stream.unwrap_or_default();
    let ms_id = media_server_id.unwrap_or_default();
    
    if stream.is_empty() {
        return Ok(Json(WVPResult::error("Stream ID is required")));
    }
    
    // Get ZLM client
    let zlm_client = if !ms_id.is_empty() {
        state.zlm_client.clone()
    } else {
        state.zlm_client.clone()
    };
    
    let zlm = match zlm_client {
        Some(c) => c,
        None => {
            return Ok(Json(WVPResult::error("ZLM client not available")));
        }
    };
    
    // Open RTP server via ZLM
    let req = OpenRtpServerRequest {
        secret: zlm.secret.clone(),
        stream_id: stream.clone(),
        port: None,
        use_tcp: body.use_tcp,
        rtp_type: None,
        recv_port: None,
    };
    
    match zlm.open_rtp_server(&req).await {
        Ok(rtp_info) => {
            tracing::info!("RTP server opened: {} -> {:?}", stream, rtp_info);
            
            if id > 0 {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                // 修正：此前吞掉错误 —— ZLM 已开好收流端口但库里没记，
                // 「推流中」状态与媒体面不一致。
                stream_push::update(&state.pool, id, None, None, Some(&ms_id), &now)
                    .await
                    .map_err(|e| {
                        AppError::business(ErrorCode::Error500, format!("推流记录更新失败: {}", e))
                    })?;
                stream_push::update_pushing_status(&state.pool, id, true)
                    .await
                    .map_err(|e| {
                        AppError::business(ErrorCode::Error500, format!("推流状态更新失败: {}", e))
                    })?;
            }
            
            Ok(Json(WVPResult::success(serde_json::json!({
                "stream": stream,
                "port": rtp_info.port,
                "ssrc": rtp_info.ssrc,
                "clientIp": rtp_info.client_ip,
                "clientPort": rtp_info.client_port,
                "mediaServerId": ms_id,
                "message": "Push stream started successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to open RTP server: {}", e);
            Ok(Json(WVPResult::error(format!("ZLM error: {}", e))))
        }
    }
}

/// POST /api/push/batch_remove 请求体
#[derive(Debug, Deserialize)]
pub struct PushBatchRemoveBody {
    pub ids: Option<Vec<i64>>,
}

/// POST /api/push/batch_remove
pub async fn push_batch_remove(
    State(state): State<AppState>,
    Json(body): Json<PushBatchRemoveBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let ids = body.ids.unwrap_or_default();
    let mut removed = 0;
    let mut errors = Vec::new();
    
    for id in ids {
        // Get push info first
        if let Ok(Some(push)) = stream_push::get_by_id(&state.pool, id as i64).await {
            // Close ZLM if pushing
            if push.pushing.unwrap_or(false) {
                if let Some(ref zlm_client) = state.zlm_client {
                    if let Some(stream) = &push.stream {
                        if let Err(e) = zlm_client.close_rtp_server(stream).await {
                            tracing::warn!(
                                "批量删除推流 {} 时关闭 ZLM RTP server 失败: {}",
                                stream,
                                e
                            );
                        }
                    }
                }
            }
        }
        
        match stream_push::delete_by_id(&state.pool, id as i64).await {
            Ok(n) => removed += n,
            Err(e) => errors.push(format!("ID {}: {}", id, e)),
        }
    }
    
    Ok(Json(WVPResult::success(serde_json::json!({
        "removed": removed,
        "errors": errors,
        "message": if errors.is_empty() { "Batch remove successful" } else { "Batch remove completed with errors" }
    }))))
}

/// POST /api/push/save_to_gb - 保存推流信息到国标
/// 内部工具 — 按 feature 分发不同 SQL；sqlite 路径下部分参数仅在 cfg(postgres/mysql) 中使用
#[allow(unused_variables)]
pub async fn push_save_to_gb(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let device_id = body.get("deviceId").and_then(|v| v.as_str()).unwrap_or("");
    let channel_id = body.get("channelId").and_then(|v| v.as_str()).unwrap_or("");
    
    if id <= 0 || device_id.is_empty() {
        return Ok(Json(WVPResult::error("缺少必要参数".to_string())));
    }
    
    // 此前更新的是**不存在的列**（`gb_stream_push.device_id/channel_id`）→
    // 接口稳定报 500 "no such column: device_id"。现在写入真实存在的
    // `gb_device_id` / `gb_channel_id`。
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let affected =
        stream_push::set_gb_binding(&state.pool, id, Some(device_id), Some(channel_id), &now).await
            .map_err(|e| AppError::business(ErrorCode::Error500, format!("绑定国标设备失败: {e}")))?;
    if affected == 0 {
        return Err(AppError::business(ErrorCode::Error404, format!("推流不存在: {id}")));
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "saved": 1,
        "message": "推流已保存到国标"
    }))))
}

/// POST /api/push/remove_form_gb - 从国标移除推流信息
/// 内部工具 — 按 feature 分发不同 SQL；sqlite 路径下部分参数仅在 cfg(postgres/mysql) 中使用
#[allow(unused_variables)]
pub async fn push_remove_form_gb(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    
    if id <= 0 {
        return Ok(Json(WVPResult::error("缺少必要参数".to_string())));
    }
    
    // 同 save_to_gb：原来更新的是不存在的列 → 稳定 500。现在清空真实列。
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let affected = stream_push::set_gb_binding(&state.pool, id, None, None, &now)
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("解绑国标设备失败: {e}")))?;
    if affected == 0 {
        return Err(AppError::business(ErrorCode::Error404, format!("推流不存在: {id}")));
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "removed": 1,
        "message": "推流已从国标移除"
    }))))
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct ProxyListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub pulling: Option<String>,
    pub mediaServerId: Option<String>,
}

/// `pulling` 查询参数是字符串（WVP 前端发 "true"/"false"，也有传 "1"/"0" 的）。
/// 空串表示"全部"，不是 `false` —— 用 `parse::<bool>()` 的话 "1" 会被静默当成不过滤。
fn parse_opt_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// GET /api/proxy/list
pub async fn proxy_list(
    State(state): State<AppState>,
    Query(q): Query<ProxyListQuery>,
) -> Result<Json<WVPResult<ProxyListPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let pulling = q.pulling.as_deref().and_then(parse_opt_bool);
    // `query` 此前被 DTO 收下却从未使用：搜索框输入什么，返回的都是同一批数据。
    let query = q.query.as_deref().filter(|s| !s.trim().is_empty());
    let total =
        stream_proxy::count_all(&state.pool, q.mediaServerId.as_deref(), pulling, query).await?;
    let list = stream_proxy::list_paged(
        &state.pool,
        page,
        count,
        q.mediaServerId.as_deref(),
        pulling,
        query,
    )
    .await?;
    let pages = if count == 0 {
        0
    } else {
        (total as u64 + count as u64 - 1) / count as u64
    };
    Ok(Json(WVPResult::success(ProxyListPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
        page_num: page as u64,
        page_size: count as u64,
        pages,
    })))
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyListPage {
    pub total: u64,
    pub list: Vec<StreamProxy>,
    pub page: u64,
    pub size: u64,
    /// PageHelper 的 `PageInfo` 兼容字段（WVP 返回的就是 PageInfo）
    pub page_num: u64,
    pub page_size: u64,
    pub pages: u64,
}

/// GET /api/proxy/ffmpeg_cmd/list
///
/// 直接读该媒体节点的 `getServerConfig`，取所有 `ffmpeg.cmd*` 键 ——
/// 此前返回的是 4 条硬编码中文说明（"默认转码模板"…），既不是 ZLM 的模板键，
/// 选中后 `addFfmpegSource` 也找不到对应配置。
pub async fn proxy_ffmpeg_cmd_list(
    State(state): State<AppState>,
    Query(q): Query<ProxyListQuery>,
) -> Result<Json<WVPResult<HashMap<String, String>>>, AppError> {
    let media_server_id = q.mediaServerId.unwrap_or_else(|| "auto".to_string());
    let zlm = state.get_zlm_client(Some(&media_server_id)).ok_or_else(|| {
        AppError::business(
            ErrorCode::Error100,
            format!("流媒体节点不可用: {media_server_id}"),
        )
    })?;
    let cfg = zlm.get_server_config().await.map_err(|e| {
        AppError::business(
            ErrorCode::Error500,
            format!("读取节点 {media_server_id} 配置失败: {e}"),
        )
    })?;
    let cmds: HashMap<String, String> = cfg
        .into_iter()
        .filter(|(k, _)| k.starts_with("ffmpeg.cmd"))
        .collect();
    Ok(Json(WVPResult::success(cmds)))
}

/// 拉流代理的新增/更新请求体。
///
/// 字段名与 WVP 的 `StreamProxy` bean 一致（camelCase）；额外兼容本平台旧版
/// 前端用的 `url` / `enabled` 别名。此前 DTO 只认 `src_url`/`srcUrl`，
/// 前端提交的 `url` 被 serde 静默忽略 → 新增必失败、编辑保存静默不生效。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyBody {
    pub id: Option<i64>,
    pub app: Option<String>,
    pub stream: Option<String>,
    #[serde(alias = "url")]
    pub src_url: Option<String>,
    pub media_server_id: Option<String>,
    pub relates_media_server_id: Option<String>,
    pub name: Option<String>,
    /// "default"（ZLM 原生拉流）或 "ffmpeg"（FFmpeg 转发）
    pub r#type: Option<String>,
    pub timeout: Option<i32>,
    pub ffmpeg_cmd_key: Option<String>,
    /// "0" = TCP / "1" = UDP / "2" = 组播
    pub rtsp_type: Option<String>,
    #[serde(alias = "enabled")]
    pub enable: Option<bool>,
    pub enable_audio: Option<bool>,
    pub enable_mp4: Option<bool>,
    pub enable_disable_none_reader: Option<bool>,
    /// WVP 旧前端用 0/1/2 的 `noneReader` 单选表达无人观看策略（1 = 自动停流）
    pub none_reader: Option<i32>,
}

/// `Some("")` 与 `None` 一律按"未提供"处理，省得把空串写进库。
fn opt_trim(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

impl ProxyBody {
    /// 可写字段；`None` 表示"不改动"（COALESCE 保留旧值）。
    fn to_write(&self) -> stream_proxy::StreamProxyWrite<'_> {
        stream_proxy::StreamProxyWrite {
            app: opt_trim(&self.app),
            stream: opt_trim(&self.stream),
            src_url: opt_trim(&self.src_url),
            // WVP 里用户选的是 relatesMediaServerId；mediaServerId 是运行期解析结果
            media_server_id: opt_trim(&self.media_server_id)
                .or_else(|| opt_trim(&self.relates_media_server_id)),
            name: opt_trim(&self.name),
            r#type: opt_trim(&self.r#type),
            timeout: self.timeout,
            ffmpeg_cmd_key: opt_trim(&self.ffmpeg_cmd_key),
            rtsp_type: opt_trim(&self.rtsp_type),
            enable: self.enable,
            enable_audio: self.enable_audio,
            enable_mp4: self.enable_mp4,
            enable_disable_none_reader: self
                .enable_disable_none_reader
                .or_else(|| self.none_reader.map(|n| n == 1)),
            relates_media_server_id: opt_trim(&self.relates_media_server_id),
        }
    }
}

/// 落库用的当前时间（与推流记录一致，本地时区）。
fn local_now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 新增 / 保存时校验必填项并补齐缺省值，返回 (app, stream, name)。
fn normalize_proxy_body(body: &ProxyBody) -> Result<(String, String, String), String> {
    let app = opt_trim(&body.app).unwrap_or("proxy").to_string();
    let stream = opt_trim(&body.stream).unwrap_or("").to_string();
    if stream.is_empty() {
        return Err("流 ID(stream) 不能为空".to_string());
    }
    if opt_trim(&body.src_url).is_none() {
        return Err("源 URL(srcUrl) 不能为空".to_string());
    }
    let name = opt_trim(&body.name)
        .map(str::to_string)
        .unwrap_or_else(|| stream.clone());
    Ok((app, stream, name))
}

/// 回读并返回真实的行（新增/更新后前端要拿到 id 等字段，不能凭空拼）。
async fn proxy_row_json(
    state: &AppState,
    app: &str,
    stream: &str,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let rec = stream_proxy::get_by_app_stream(&state.pool, app, stream)
        .await?
        .ok_or_else(|| {
            AppError::business(
                ErrorCode::Error500,
                format!("代理写入后回读失败: {app}/{stream}"),
            )
        })?;
    let value = serde_json::to_value(&rec)
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("序列化代理失败: {e}")))?;
    Ok(Json(WVPResult::success(value)))
}

/// POST /api/proxy/add —— 新增（APP+STREAM 已存在时报错，与 WVP 一致）
pub async fn proxy_add(
    State(state): State<AppState>,
    Json(body): Json<ProxyBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let (app, stream, name) = match normalize_proxy_body(&body) {
        Ok(v) => v,
        Err(e) => return Ok(Json(WVPResult::error(e))),
    };
    if stream_proxy::get_by_app_stream(&state.pool, &app, &stream)
        .await?
        .is_some()
    {
        return Ok(Json(WVPResult::error(format!(
            "APP+STREAM 已存在: {app}/{stream}"
        ))));
    }
    let mut w = body.to_write();
    let name_fallback = name.clone();
    w.app = Some(app.as_str());
    w.stream = Some(stream.as_str());
    w.name = Some(name_fallback.as_str());
    stream_proxy::add(&state.pool, &w, &local_now_str())
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("新增代理失败: {e}")))?;
    proxy_row_json(&state, &app, &stream).await
}

/// POST /api/proxy/save —— 保存（存在则更新，不存在则新增）
pub async fn proxy_save(
    State(state): State<AppState>,
    Json(body): Json<ProxyBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let (app, stream, name) = match normalize_proxy_body(&body) {
        Ok(v) => v,
        Err(e) => return Ok(Json(WVPResult::error(e))),
    };
    let mut w = body.to_write();
    w.app = Some(app.as_str());
    w.stream = Some(stream.as_str());
    w.name = Some(name.as_str());
    let now = local_now_str();
    match stream_proxy::get_by_app_stream(&state.pool, &app, &stream).await? {
        Some(existing) => {
            stream_proxy::update(&state.pool, existing.id as i64, &w, &now)
                .await
                .map_err(|e| AppError::business(ErrorCode::Error500, format!("保存代理失败: {e}")))?;
        }
        None => {
            stream_proxy::add(&state.pool, &w, &now)
                .await
                .map_err(|e| AppError::business(ErrorCode::Error500, format!("新增代理失败: {e}")))?;
        }
    }
    proxy_row_json(&state, &app, &stream).await
}

/// POST /api/proxy/update
pub async fn proxy_update(
    State(state): State<AppState>,
    Json(body): Json<ProxyBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::error("缺少代理 ID".to_string())));
    }
    // 部分更新：只有真正提供的字段才写库，其余 COALESCE 保留旧值。
    if let Some(s) = &body.stream {
        if s.trim().is_empty() {
            return Ok(Json(WVPResult::error("流 ID(stream) 不能为空".to_string())));
        }
    }
    if let Some(u) = &body.src_url {
        if u.trim().is_empty() {
            return Ok(Json(WVPResult::error(
                "源 URL(srcUrl) 不能为空".to_string(),
            )));
        }
    }
    let existing = stream_proxy::get_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::Error404, format!("代理不存在: {id}")))?;
    let w = body.to_write();
    stream_proxy::update(&state.pool, id, &w, &local_now_str())
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("更新代理失败: {e}")))?;
    // 保存后回读（app/stream 可能被改过，用哪个键回读以库里的最新值为准）
    let app = opt_trim(&body.app).unwrap_or_else(|| existing.app.as_deref().unwrap_or("proxy")).to_string();
    let stream = opt_trim(&body.stream)
        .map(str::to_string)
        .or(existing.stream.clone())
        .unwrap_or_default();
    proxy_row_json(&state, &app, &stream).await
}

/// 启动 / 停止 / 删除共用的定位参数（WVP 的 start/stop 只认 id，del 认 app+stream）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyActionQuery {
    pub id: Option<i64>,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub media_server_id: Option<String>,
}

/// 按 id 或 app+stream 定位代理行。
async fn resolve_proxy(
    state: &AppState,
    q: &ProxyActionQuery,
) -> Result<StreamProxy, AppError> {
    if let Some(id) = q.id.filter(|v| *v > 0) {
        return stream_proxy::get_by_id(&state.pool, id)
            .await?
            .ok_or_else(|| AppError::business(ErrorCode::Error404, format!("代理不存在: {id}")));
    }
    if let Some(app) = opt_trim(&q.app) {
        if let Some(stream) = opt_trim(&q.stream) {
            return stream_proxy::get_by_app_stream(&state.pool, app, stream)
                .await?
                .ok_or_else(|| {
                    AppError::business(ErrorCode::Error404, format!("代理不存在: {app}/{stream}"))
                });
        }
    }
    Err(AppError::business(
        ErrorCode::Error400,
        "缺少 id 或 app+stream 参数",
    ))
}

/// GET /api/proxy/start —— 真正拉起拉流代理
///
/// 按记录里的 `type` 分流：`ffmpeg` 走 `addFFmpegSource`（带 `ffmpeg_cmd_key`），
/// 其余走 `addStreamProxy`（带 `rtsp_type` / `timeout` / `enable_audio` / `enable_mp4`）。
/// 成功后把 `pulling` 置真并写 `stream_status = active`，界面上的"运行中"才有依据。
pub async fn proxy_start(
    State(state): State<AppState>,
    Query(q): Query<ProxyActionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let rec = resolve_proxy(&state, &q).await?;
    let app = rec.app.clone().unwrap_or_else(|| "proxy".to_string());
    let stream = rec.stream.clone().unwrap_or_default();
    let src_url = rec.src_url.clone().unwrap_or_default();
    if stream.is_empty() || src_url.is_empty() {
        return Ok(Json(WVPResult::error(
            "代理记录缺少 stream 或 srcUrl，无法启动".to_string(),
        )));
    }
    let ms_hint = rec
        .media_server_id
        .clone()
        .filter(|s| !s.is_empty() && s != "auto")
        .or_else(|| rec.relates_media_server_id.clone())
        .or_else(|| q.media_server_id.clone());
    let zlm = state.get_zlm_client(ms_hint.as_deref()).ok_or_else(|| {
        AppError::business(ErrorCode::Error100, "ZLM 未配置，无法启动拉流代理")
    })?;

    let timeout_sec = rec.timeout.unwrap_or(30).max(1) as f64;
    let is_ffmpeg = rec.r#type.as_deref() == Some("ffmpeg");
    let stream_key = if is_ffmpeg {
        let dst_url = format!("rtmp://127.0.0.1:1935/{app}/{stream}");
        let req = crate::zlm::AddFFmpegSourceRequest {
            secret: zlm.secret.clone(),
            src_url: src_url.clone(),
            dst_url,
            timeout_ms: Some((timeout_sec * 1000.0) as u32),
            ffmpeg_cmd_key: rec.ffmpeg_cmd_key.clone().filter(|s| !s.is_empty()),
            enable_hls: Some(false),
            enable_mp4: Some(rec.enable_mp4.unwrap_or(false)),
            enable_rtsp: Some(true),
            enable_rtmp: Some(true),
            enable_fmp4: Some(true),
        };
        zlm.add_ffmpeg_source(&req).await
    } else {
        let req = crate::zlm::AddStreamProxyRequest {
            secret: zlm.secret.clone(),
            vhost: "__defaultVhost__".to_string(),
            app: app.clone(),
            stream: stream.clone(),
            url: src_url.clone(),
            rtp_type: rec
                .rtsp_type
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<u32>().ok()),
            timeout_sec: Some(timeout_sec),
            enable_hls: Some(false),
            enable_mp4: Some(rec.enable_mp4.unwrap_or(false)),
            enable_rtsp: Some(true),
            enable_rtmp: Some(true),
            enable_fmp4: Some(true),
            enable_ts: Some(false),
            enable_audio: Some(rec.enable_audio.unwrap_or(false)),
        };
        zlm.add_stream_proxy(&req).await
    };
    let key = match stream_key {
        Ok(k) => k,
        Err(e) => {
            // 启动失败要把状态写回去，否则界面上会一直显示"运行中"
            if let Err(ue) =
                stream_proxy::update_play_state(&state.pool, rec.id as i64, false, "failed").await
            {
                tracing::warn!("代理启动失败后回写状态失败 id={}: {}", rec.id, ue);
            }
            return Err(AppError::business(
                ErrorCode::Error500,
                format!("启动拉流代理 {app}/{stream} 失败: {e}"),
            ));
        }
    };

    if let Err(e) = stream_proxy::update_play_state(&state.pool, rec.id as i64, true, "active").await {
        tracing::warn!("代理启动后回写 pulling 失败 id={}: {}", rec.id, e);
    }
    tracing::info!("拉流代理已启动: {app}/{stream} <- {src_url} (key={key})");

    let resolved_media_server_id = state
        .zlm_server_id(&zlm)
        .or(ms_hint)
        .unwrap_or_default();
    let media_ip = zlm.ip.clone();
    let http_port = zlm.http_port;
    let stream_url = format!("{app}/{stream}");
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": rec.id,
        "app": app,
        "stream": stream,
        "srcUrl": src_url,
        "streamKey": key,
        "mediaServerId": resolved_media_server_id,
        "playUrl": format!("rtsp://{media_ip}:554/{stream_url}"),
        "flvUrl": format!("http://{media_ip}:{http_port}/{stream_url}.flv"),
        "wsUrl": format!("ws://{media_ip}:{http_port}/{stream_url}.live.flv"),
        "hlsUrl": format!("http://{media_ip}:{http_port}/{stream_url}/hls.m3u8"),
        "message": "拉流代理已启动"
    }))))
}

/// GET /api/proxy/stop
///
/// 关闭 ZLM 侧的流并清 `pulling`。关闭失败只记日志：ZLM 上本来就没有这条流
/// （例如节点重启过）时，"已停止"依然是事实。
pub async fn proxy_stop(
    State(state): State<AppState>,
    Query(q): Query<ProxyActionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let rec = resolve_proxy(&state, &q).await?;
    let app = rec.app.clone().unwrap_or_else(|| "proxy".to_string());
    let stream = rec.stream.clone().unwrap_or_default();
    if stream.is_empty() {
        return Ok(Json(WVPResult::error("代理记录缺少 stream，无法停止".to_string())));
    }
    let ms_hint = rec
        .media_server_id
        .clone()
        .filter(|s| !s.is_empty() && s != "auto")
        .or_else(|| rec.relates_media_server_id.clone())
        .or_else(|| q.media_server_id.clone());
    let mut zlm_error: Option<String> = None;
    if let Some(zlm) = state.get_zlm_client(ms_hint.as_deref()) {
        if let Err(e) = zlm
            .close_streams(Some("rtsp"), Some(&app), Some(&stream), true)
            .await
        {
            tracing::warn!("停止代理 {app}/{stream} 时 ZLM 报错（仍清理状态）: {e}");
            zlm_error = Some(e.to_string());
        }
    } else {
        tracing::warn!("ZLM 未配置，仅清理代理 {app}/{stream} 的拉流状态");
    }
    stream_proxy::update_play_state(&state.pool, rec.id as i64, false, "ready")
        .await
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("更新代理状态失败: {e}")))?;
    tracing::info!("拉流代理已停止: {app}/{stream}");
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": rec.id,
        "app": app,
        "stream": stream,
        "pulling": false,
        "zlmWarning": zlm_error,
        "message": "拉流代理已停止"
    }))))
}

/// DELETE /api/proxy/delete?id=N 与 DELETE /api/proxy/del?app=&stream=
pub async fn proxy_delete(
    State(state): State<AppState>,
    Query(q): Query<ProxyActionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let rec = resolve_proxy(&state, &q).await?;
    // 先停流再删行：反过来的话正在拉的流会变成 ZLM 上的野流，永远无人回收。
    if rec.pulling.unwrap_or(false) {
        let app = rec.app.clone().unwrap_or_else(|| "proxy".to_string());
        let stream = rec.stream.clone().unwrap_or_default();
        if !stream.is_empty() {
            if let Some(zlm) = state.get_zlm_client(rec.media_server_id.as_deref()) {
                if let Err(e) = zlm
                    .close_streams(Some("rtsp"), Some(&app), Some(&stream), true)
                    .await
                {
                    tracing::warn!("删除代理前关闭 {app}/{stream} 失败（继续删除）: {e}");
                }
            }
        }
    }
    let affected = stream_proxy::delete_by_id(&state.pool, rec.id as i64).await?;
    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("代理不存在: {}", rec.id),
        ));
    }
    Ok(Json(WVPResult::success(serde_json::json!({
        "id": rec.id,
        "deleted": affected,
        "message": "拉流代理已删除"
    }))))
}

/// POST /api/push/upload - 上传文件推流
pub async fn push_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let upload_dir = PathBuf::from("uploads");
    if !upload_dir.exists() {
        let _ = std::fs::create_dir_all(&upload_dir);
    }
    
    let mut app = "upload".to_string();
    let mut stream = String::new();
    let mut saved_path = String::new();
    
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let field_name = field.name().unwrap_or_default().to_string();
        
        match field_name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("upload.bin").to_string();
                let ext = std::path::Path::new(&filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("bin");
                
                let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
                let random_suffix: String = (0..4)
                    .map(|_| char::from(b'a' + rand::random::<u8>() % 26))
                    .collect();
                stream = format!("{}_{}", timestamp, random_suffix);
                
                let saved_filename = format!("{}.{}", stream, ext);
                let filepath = upload_dir.join(&saved_filename);
                
                let data = field.bytes().await.map_err(|e| {
                    AppError::business(crate::error::ErrorCode::Error500, format!("读取上传文件失败: {}", e))
                })?;
                
                std::fs::write(&filepath, &data).map_err(|e| {
                    AppError::business(crate::error::ErrorCode::Error500, format!("保存文件失败: {}", e))
                })?;
                
                saved_path = filepath.to_string_lossy().to_string();
                tracing::info!("Uploaded file saved: {}", saved_path);
            }
            "app" => {
                if let Ok(bytes) = field.bytes().await {
                    app = String::from_utf8_lossy(&bytes).to_string();
                }
            }
            "stream" => {
                if let Ok(bytes) = field.bytes().await {
                    stream = String::from_utf8_lossy(&bytes).to_string();
                }
            }
            _ => {}
        }
    }
    
    if stream.is_empty() {
        return Ok(Json(WVPResult::error("未提供文件或流名称")));
    }
    
    let media_server_id = "auto".to_string();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    match stream_push::add(&state.pool, &app, &stream, &media_server_id, &now).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "app": app,
                "stream": stream,
                "url": saved_path,
                "mediaServerId": media_server_id,
                "message": "文件上传成功"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to add push stream record: {}", e);
            Ok(Json(WVPResult::error(format!("数据库错误: {}", e))))
        }
    }
}

/// GET /api/proxy/one —— 按 id 或 app+stream 返回真实的代理行
///
/// 此前不查库、凭空拼 `name = "proxy-{id}"`、`url = rtsp://<ip>:554/live/proxy{id}`，
/// 与真实记录毫无关系；WVP 的签名是 `?app=&stream=`，本平台前端用 `?id=`，两者都支持。
pub async fn proxy_one(
    State(state): State<AppState>,
    Query(q): Query<ProxyActionQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let rec = resolve_proxy(&state, &q).await?;
    let value = serde_json::to_value(&rec)
        .map_err(|e| AppError::business(ErrorCode::Error500, format!("序列化代理失败: {e}")))?;
    Ok(Json(WVPResult::success(value)))
}

/// GET /api/push/forceClose?id=...
pub async fn push_force_close(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<PushForceCloseQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let Some(ref zlm) = state.zlm_client else {
        return Err(AppError::business(
            ErrorCode::Error100,
            "ZLM 未配置，无法强制关闭推流",
        ));
    };
    // 优先用记录里的真实 stream / app 名；拿不到再退回 push_{id} 约定名
    let rec = stream_push::get_by_id(&state.pool, q.id).await?;
    let stream = rec
        .as_ref()
        .and_then(|r| r.stream.clone())
        .unwrap_or_else(|| format!("push_{}", q.id));
    let app = rec.as_ref().and_then(|r| r.app.clone());

    // 修正：此前 `let _ =` 吞掉 ZLM 错误并无条件返回 closed: true
    zlm.close_streams(app.as_deref(), None, Some(&stream), true)
        .await
        .map_err(|e| {
            AppError::business(ErrorCode::Error500, format!("关闭推流 {} 失败: {}", stream, e))
        })?;

    if let Err(e) = stream_push::update_pushing_status(&state.pool, q.id, false).await {
        tracing::warn!("关闭推流后更新 pushing 状态失败 id={}: {}", q.id, e);
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "id": q.id,
        "stream": stream,
        "closed": true,
    }))))
}

/// 停止推流（前端 `/api/push/stop`）。
///
/// 前端 `web/src/api/streamPush.ts::stopStreamPush` 一直在调这个路径，
/// 但后端从未注册过 —— 请求会落到 SPA 兜底并返回 index.html，
/// 「停止推流」按钮实际不工作。
#[derive(Debug, Deserialize)]
pub struct PushStopQuery {
    pub id: Option<i64>,
    pub stream: Option<String>,
}

pub async fn push_stop(
    State(state): State<AppState>,
    Query(q): Query<PushStopQuery>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = q.id.unwrap_or(0);

    // 解析要关闭的 stream：优先用请求里的，其次查库
    let (stream, app) = if let Some(s) = q.stream.as_deref().filter(|s| !s.is_empty()) {
        (s.to_string(), None)
    } else if id > 0 {
        match stream_push::get_by_id(&state.pool, id).await? {
            Some(rec) => (rec.stream.clone().unwrap_or_default(), rec.app.clone()),
            None => {
                return Err(AppError::business(
                    ErrorCode::Error404,
                    format!("推流记录不存在: {}", id),
                ))
            }
        }
    } else {
        return Err(AppError::business(
            ErrorCode::Error400,
            "缺少 id 或 stream 参数",
        ));
    };

    if stream.is_empty() {
        return Err(AppError::business(
            ErrorCode::Error400,
            "该推流记录没有 stream 名称，无法停止",
        ));
    }

    // 1) 关掉 ZLM 侧的收流（RTP server 与普通流是互斥的两种收流方式，
    //    因此只有两条路径都失败才判定为失败）
    if let Some(ref zlm) = state.zlm_client {
        let rtp_err = zlm.close_rtp_server(&stream).await.err();
        let stream_err = zlm
            .close_streams(app.as_deref(), None, Some(&stream), true)
            .await
            .err();
        if let (Some(a), Some(b)) = (rtp_err, stream_err) {
            return Err(AppError::business(
                ErrorCode::Error500,
                format!("停止推流 {} 失败: RTP={}; stream={}", stream, a, b),
            ));
        }
    }

    // 2) 落库 pushing = false
    if id > 0 {
        stream_push::update_pushing_status(&state.pool, id, false)
            .await
            .map_err(|e| {
                AppError::business(ErrorCode::Error500, format!("推流状态更新失败: {}", e))
            })?;
    }

    Ok(Json(WVPResult::success(serde_json::json!({
        "id": id,
        "stream": stream,
        "stopped": true,
        "message": "推流已停止"
    }))))
}

#[derive(serde::Deserialize)]
pub struct PushForceCloseQuery {
    pub id: i64,
}

#[cfg(test)]
mod proxy_tests {
    use super::*;
    use crate::test_support::app_state;

    fn body(v: serde_json::Value) -> ProxyBody {
        serde_json::from_value(v).expect("ProxyBody 反序列化")
    }

    /// 旧版前端字段名（`url` / `enabled`）与 WVP 字段名（`srcUrl` / `enable`）都必须能解析。
    #[test]
    fn proxy_body_accepts_legacy_and_wvp_field_names() {
        let legacy = body(serde_json::json!({
            "name": "p1", "type": "rtsp", "app": "live", "stream": "s1",
            "url": "rtsp://cam/1", "enabled": true, "destUrl": "rtmp://x"
        }));
        assert_eq!(legacy.src_url.as_deref(), Some("rtsp://cam/1"));
        assert_eq!(legacy.enable, Some(true));

        let wvp = body(serde_json::json!({
            "name": "p1", "type": "ffmpeg", "app": "live", "stream": "s1",
            "srcUrl": "rtsp://cam/1", "enable": true, "enableAudio": true,
            "enableMp4": true, "rtspType": "0", "timeout": 15,
            "ffmpegCmdKey": "ffmpeg.cmd", "relatesMediaServerId": "ms-1",
            "noneReader": 1
        }));
        assert_eq!(wvp.src_url.as_deref(), Some("rtsp://cam/1"));
        assert_eq!(wvp.r#type.as_deref(), Some("ffmpeg"));
        assert_eq!(wvp.enable_audio, Some(true));
        assert_eq!(wvp.enable_mp4, Some(true));
        assert_eq!(wvp.timeout, Some(15));
        assert_eq!(wvp.ffmpeg_cmd_key.as_deref(), Some("ffmpeg.cmd"));
        assert_eq!(wvp.relates_media_server_id.as_deref(), Some("ms-1"));
        // `noneReader`（0/1/2 单选）在 to_write 里折算成 enableDisableNoneReader
        assert_eq!(wvp.none_reader, Some(1));
        assert_eq!(wvp.to_write().enable_disable_none_reader, Some(true));
    }

    #[test]
    fn parse_opt_bool_handles_empty_and_numeric() {
        assert_eq!(parse_opt_bool(""), None);
        assert_eq!(parse_opt_bool("true"), Some(true));
        assert_eq!(parse_opt_bool("false"), Some(false));
        assert_eq!(parse_opt_bool("1"), Some(true));
        assert_eq!(parse_opt_bool("0"), Some(false));
        assert_eq!(parse_opt_bool("maybe"), None);
    }

    /// 新增必须把 type / srcUrl / enable / enableAudio / enableMp4 / rtspType /
    /// timeout / ffmpegCmdKey / relatesMediaServerId 全部落库 —— 这些以前全丢。
    #[tokio::test]
    async fn proxy_add_persists_all_fields_and_returns_row() {
        let state = app_state().await;
        let resp = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "name": "代理一", "type": "ffmpeg", "app": "live", "stream": "cam1",
                "srcUrl": "rtsp://10.0.0.1/live", "enable": true, "enableAudio": true,
                "enableMp4": true, "rtspType": "1", "timeout": 20,
                "ffmpegCmdKey": "ffmpeg.cmd", "relatesMediaServerId": "ms-9",
                "noneReader": 1
            }))),
        )
        .await
        .expect("proxy_add");
        let data = resp.0.data.expect("data");
        assert_eq!(data["srcUrl"], "rtsp://10.0.0.1/live");
        assert_eq!(data["type"], "ffmpeg");
        assert_eq!(data["enable"], true);
        assert_eq!(data["enableAudio"], true);
        assert_eq!(data["enableMp4"], true);
        assert_eq!(data["rtspType"], "1");
        assert_eq!(data["timeout"], 20);
        assert_eq!(data["ffmpegCmdKey"], "ffmpeg.cmd");
        assert_eq!(data["relatesMediaServerId"], "ms-9");
        assert_eq!(data["enableDisableNoneReader"], true);
        assert_eq!(data["pulling"], false);
        assert_eq!(data["name"], "代理一");
        assert!(data["id"].as_i64().unwrap_or(0) > 0);

        let row = stream_proxy::get_by_app_stream(&state.pool, "live", "cam1")
            .await
            .unwrap()
            .expect("库里有行");
        assert_eq!(row.r#type.as_deref(), Some("ffmpeg"));
        assert_eq!(row.timeout, Some(20));
        assert_eq!(row.enable_audio, Some(true));
        assert_eq!(row.enable_mp4, Some(true));
        assert_eq!(row.rtsp_type.as_deref(), Some("1"));
        assert_eq!(row.enable, Some(true));
        assert_eq!(row.enable_disable_none_reader, Some(true));
        assert_eq!(row.relates_media_server_id.as_deref(), Some("ms-9"));
    }

    /// 旧前端只发 `url`/`enabled`；不补齐的话新增会以"源 URL 不能为空"直接失败。
    #[tokio::test]
    async fn proxy_add_accepts_legacy_url_field() {
        let state = app_state().await;
        let resp = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "name": "legacy", "app": "proxy", "stream": "legacy1",
                "url": "rtmp://a/b", "enabled": false
            }))),
        )
        .await
        .expect("proxy_add");
        assert_eq!(resp.0.code, 0, "msg={}", resp.0.msg);
        let row = stream_proxy::get_by_app_stream(&state.pool, "proxy", "legacy1")
            .await
            .unwrap()
            .expect("库里有行");
        assert_eq!(row.src_url.as_deref(), Some("rtmp://a/b"));
    }

    #[tokio::test]
    async fn proxy_add_rejects_duplicate_and_missing_fields() {
        let state = app_state().await;
        let ok = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "app": "live", "stream": "dup", "srcUrl": "rtsp://a"
            }))),
        )
        .await
        .unwrap();
        assert_eq!(ok.0.code, 0);

        let dup = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "app": "live", "stream": "dup", "srcUrl": "rtsp://b"
            }))),
        )
        .await
        .unwrap();
        assert_ne!(dup.0.code, 0);
        assert!(dup.0.msg.contains("已存在"), "msg={}", dup.0.msg);

        let missing = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({ "app": "live", "stream": "x" }))),
        )
        .await
        .unwrap();
        assert_ne!(missing.0.code, 0);

        // 名称缺省 = stream
        let row = stream_proxy::get_by_app_stream(&state.pool, "live", "dup")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.name.as_deref(), Some("dup"));
    }

    /// 编辑保存：`srcUrl` 必须真的改掉（以前 COALESCE + 字段名不匹配 → 静默不动）。
    #[tokio::test]
    async fn proxy_update_changes_src_url_and_enable() {
        let state = app_state().await;
        let added = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "app": "live", "stream": "u1", "srcUrl": "rtsp://old", "enable": false
            }))),
        )
        .await
        .unwrap();
        let id = added.0.data.unwrap()["id"].as_i64().unwrap();

        let resp = proxy_update(
            State(state.clone()),
            Json(body(serde_json::json!({
                "id": id, "srcUrl": "rtsp://new", "enable": true, "name": "改名"
            }))),
        )
        .await
        .expect("proxy_update");
        assert_eq!(resp.0.code, 0, "msg={}", resp.0.msg);
        assert_eq!(resp.0.data.as_ref().unwrap()["srcUrl"], "rtsp://new");
        assert_eq!(resp.0.data.as_ref().unwrap()["enable"], true);

        let row = stream_proxy::get_by_id(&state.pool, id).await.unwrap().unwrap();
        assert_eq!(row.src_url.as_deref(), Some("rtsp://new"));
        assert_eq!(row.enable, Some(true));
        assert_eq!(row.name.as_deref(), Some("改名"));
        // 未提供的字段保持原值
        assert_eq!(row.app.as_deref(), Some("live"));
    }

    #[tokio::test]
    async fn proxy_update_rejects_unknown_id_and_empty_required() {
        let state = app_state().await;
        let err = proxy_update(
            State(state.clone()),
            Json(body(serde_json::json!({ "id": 12345, "name": "x" }))),
        )
        .await
        .expect_err("不存在的 id");
        assert!(matches!(err, AppError::Business(_, _)));

        let empty = proxy_update(
            State(state.clone()),
            Json(body(serde_json::json!({ "id": 1, "stream": "  " }))),
        )
        .await
        .unwrap();
        assert_ne!(empty.0.code, 0);
    }

    /// `save` 是 upsert：同 app+stream 第二次调用走更新，不会撞唯一索引。
    #[tokio::test]
    async fn proxy_save_upserts() {
        let state = app_state().await;
        for (src, name) in [("rtsp://v1", "v1"), ("rtsp://v2", "v2")] {
            let resp = proxy_save(
                State(state.clone()),
                Json(body(serde_json::json!({
                    "app": "live", "stream": "sv", "srcUrl": src, "name": name
                }))),
            )
            .await
            .unwrap();
            assert_eq!(resp.0.code, 0, "msg={}", resp.0.msg);
        }
        let all = stream_proxy::list_paged(&state.pool, 1, 10, None, None, None)
            .await
            .unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].src_url.as_deref(), Some("rtsp://v2"));
        assert_eq!(all[0].name.as_deref(), Some("v2"));
    }

    /// `query` 过滤此前被完全忽略：搜索什么都是全量。
    #[tokio::test]
    async fn proxy_list_applies_query_and_pulling_filters() {
        let state = app_state().await;
        for (stream, src, name) in [
            ("alpha", "rtsp://10.0.0.1/a", "东门"),
            ("beta", "rtsp://10.0.0.2/b", "西门"),
        ] {
            let _ = proxy_add(
                State(state.clone()),
                Json(body(serde_json::json!({
                    "app": "live", "stream": stream, "srcUrl": src, "name": name
                }))),
            )
            .await
            .unwrap();
        }
        let row = stream_proxy::get_by_app_stream(&state.pool, "live", "beta")
            .await
            .unwrap()
            .unwrap();
        stream_proxy::update_play_state(&state.pool, row.id as i64, true, "active")
            .await
            .unwrap();

        let by_kw = proxy_list(
            State(state.clone()),
            Query(ProxyListQuery {
                page: None,
                count: None,
                query: Some("西门".to_string()),
                pulling: None,
                mediaServerId: None,
            }),
        )
        .await
        .unwrap();
        let page = by_kw.0.data.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.list[0].stream.as_deref(), Some("beta"));
        assert_eq!(page.page_num, 1);

        // 搜索命中的源 URL / type 也要能匹配
        let by_src = proxy_list(
            State(state.clone()),
            Query(ProxyListQuery {
                page: None,
                count: None,
                query: Some("10.0.0.1".to_string()),
                pulling: None,
                mediaServerId: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(by_src.0.data.unwrap().total, 1);

        let pulling = proxy_list(
            State(state.clone()),
            Query(ProxyListQuery {
                page: None,
                count: None,
                query: None,
                pulling: Some("true".to_string()),
                mediaServerId: None,
            }),
        )
        .await
        .unwrap();
        let page = pulling.0.data.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.list[0].pulling, Some(true));

        let not_pulling = proxy_list(
            State(state.clone()),
            Query(ProxyListQuery {
                page: None,
                count: None,
                query: None,
                pulling: Some("false".to_string()),
                mediaServerId: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(not_pulling.0.data.unwrap().total, 1);

        // 空串 = 全部
        let all = proxy_list(
            State(state.clone()),
            Query(ProxyListQuery {
                page: None,
                count: None,
                query: None,
                pulling: Some(String::new()),
                mediaServerId: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(all.0.data.unwrap().total, 2);
    }

    /// `one` 以前不查库、凭空拼 `proxy-{id}` 和假 URL。
    #[tokio::test]
    async fn proxy_one_returns_real_row_by_id_and_by_app_stream() {
        let state = app_state().await;
        let added = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "app": "live", "stream": "one1", "srcUrl": "rtsp://real/1", "name": "真名"
            }))),
        )
        .await
        .unwrap();
        let id = added.0.data.unwrap()["id"].as_i64().unwrap();

        let by_id = proxy_one(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: Some(id),
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .unwrap();
        let data = by_id.0.data.unwrap();
        assert_eq!(data["srcUrl"], "rtsp://real/1");
        assert_eq!(data["name"], "真名");
        assert_eq!(data["app"], "live");

        let by_app_stream = proxy_one(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: None,
                app: Some("live".to_string()),
                stream: Some("one1".to_string()),
                media_server_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(by_app_stream.0.data.unwrap()["id"], id);

        let missing = proxy_one(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: Some(999),
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .expect_err("不存在的代理应 404");
        assert!(matches!(missing, AppError::Business(_, _)));
    }

    #[tokio::test]
    async fn proxy_delete_by_id_and_by_app_stream() {
        let state = app_state().await;
        for stream in ["d1", "d2"] {
            let _ = proxy_add(
                State(state.clone()),
                Json(body(serde_json::json!({
                    "app": "live", "stream": stream, "srcUrl": "rtsp://x"
                }))),
            )
            .await
            .unwrap();
        }
        let id = stream_proxy::get_by_app_stream(&state.pool, "live", "d1")
            .await
            .unwrap()
            .unwrap()
            .id as i64;

        let by_id = proxy_delete(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: Some(id),
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(by_id.0.data.unwrap()["deleted"], 1);

        let by_app_stream = proxy_delete(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: None,
                app: Some("live".to_string()),
                stream: Some("d2".to_string()),
                media_server_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(by_app_stream.0.data.unwrap()["deleted"], 1);

        assert_eq!(
            stream_proxy::count_all(&state.pool, None, None, None)
                .await
                .unwrap(),
            0
        );

        // 缺参数 → 400 而不是静默成功
        let err = proxy_delete(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: None,
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .expect_err("缺参数");
        assert!(matches!(err, AppError::Business(_, _)));
    }

    /// 没有 ZLM 节点时，ffmpeg 模板列表必须报错 —— 以前无论配置如何都返回
    /// 4 条硬编码字符串，用户选中后 ZLM 上并不存在这个 key。
    #[tokio::test]
    async fn proxy_ffmpeg_cmd_list_requires_zlm_node() {
        let state = app_state().await;
        let err = proxy_ffmpeg_cmd_list(
            State(state.clone()),
            Query(ProxyListQuery {
                page: None,
                count: None,
                query: None,
                pulling: None,
                mediaServerId: None,
            }),
        )
        .await
        .expect_err("无 ZLM 节点应报错");
        assert!(matches!(err, AppError::Business(_, _)));
    }

    /// 启动/停止在无 ZLM 时不能假装成功：启动应报错，停止仍要清状态。
    #[tokio::test]
    async fn proxy_start_and_stop_require_zlm() {
        let state = app_state().await;
        let added = proxy_add(
            State(state.clone()),
            Json(body(serde_json::json!({
                "app": "live", "stream": "z1", "srcUrl": "rtsp://z"
            }))),
        )
        .await
        .unwrap();
        let id = added.0.data.unwrap()["id"].as_i64().unwrap();

        let err = proxy_start(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: Some(id),
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .expect_err("无 ZLM 时启动应失败");
        assert!(matches!(err, AppError::Business(_, _)));

        // 手动置为拉流中，停止后状态必须回落
        stream_proxy::update_play_state(&state.pool, id, true, "active")
            .await
            .unwrap();
        let stopped = proxy_stop(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: Some(id),
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(stopped.0.data.unwrap()["pulling"], false);
        let row = stream_proxy::get_by_id(&state.pool, id).await.unwrap().unwrap();
        assert_eq!(row.pulling, Some(false));
        assert_eq!(row.stream_status.as_deref(), Some("ready"));
    }

    /// 代理记录缺 stream 时启动要给出明确错误，而不是拼一个空流名。
    #[tokio::test]
    async fn proxy_start_rejects_incomplete_record() {
        let state = app_state().await;
        sqlx::query(
            "INSERT INTO gb_stream_proxy (app, stream, src_url, pulling, enable) VALUES ('live', '', 'rtsp://x', 0, 0)",
        )
        .execute(&state.pool)
        .await
        .unwrap();
        let id: i32 = sqlx::query_scalar("SELECT id FROM gb_stream_proxy LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let resp = proxy_start(
            State(state.clone()),
            Query(ProxyActionQuery {
                id: Some(id as i64),
                app: None,
                stream: None,
                media_server_id: None,
            }),
        )
        .await
        .unwrap();
        assert_ne!(resp.0.code, 0);
    }
}
