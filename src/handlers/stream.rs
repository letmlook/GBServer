//! 推流 /api/push 与拉流代理 /api/proxy，对应前端 streamPush.js / streamProxy.js

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::db::{stream_push, stream_proxy, StreamPush, StreamProxy};
use crate::error::AppError;
use crate::response::WVPResult;
use crate::zlm::OpenRtpServerRequest;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PushListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub pushing: Option<bool>,
    pub mediaServerId: Option<String>,
}

/// GET /api/push/list
pub async fn push_list(
    State(state): State<AppState>,
    Query(q): Query<PushListQuery>,
) -> Result<Json<WVPResult<PushListPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = stream_push::count_all(
        &state.pool,
        q.mediaServerId.as_deref(),
        q.pushing,
    )
    .await?;
    let list = stream_push::list_paged(
        &state.pool,
        page,
        count,
        q.mediaServerId.as_deref(),
        q.pushing,
    )
    .await?;
    Ok(Json(WVPResult::success(PushListPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    })))
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

/// POST /api/push/remove 请求体
#[derive(Debug, Deserialize)]
pub struct PushRemoveBody {
    pub id: Option<i64>,
}

/// POST /api/push/remove
pub async fn push_remove(
    State(state): State<AppState>,
    Json(body): Json<PushRemoveBody>,
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
    pub media_server_id: Option<String>,
    pub use_tcp: Option<bool>,
}

/// POST /api/push/start
pub async fn push_start(
    State(state): State<AppState>,
    Json(body): Json<PushStartBody>,
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
                let _ = stream_push::update(&state.pool, id, None, None, Some(&ms_id), &now).await;
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
                        let _ = zlm_client.close_rtp_server(stream).await;
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
pub async fn push_save_to_gb(
    State(state): State<AppState>,
    Json(body): Json<PushBatchRemoveBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let ids = body.ids.unwrap_or_default();
    
    // For GB28181, we would register these push streams as virtual devices
    // This is a placeholder - actual implementation would need device registration logic
    Ok(Json(WVPResult::success(serde_json::json!({
        "saved": ids.len(),
        "message": "Push streams saved to GB (placeholder)"
    }))))
}

/// POST /api/push/remove_form_gb - 从国标移除推流信息
pub async fn push_remove_form_gb(
    State(state): State<AppState>,
    Json(body): Json<PushBatchRemoveBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let ids = body.ids.unwrap_or_default();
    
    // For GB28181, we would unregister these push streams from virtual devices
    Ok(Json(WVPResult::success(serde_json::json!({
        "removed": ids.len(),
        "message": "Push streams removed from GB (placeholder)"
    }))))
}

#[derive(Debug, Deserialize)]
pub struct ProxyListQuery {
    pub page: Option<u32>,
    pub count: Option<u32>,
    pub query: Option<String>,
    pub pulling: Option<bool>,
    pub mediaServerId: Option<String>,
}

/// GET /api/proxy/list
pub async fn proxy_list(
    State(state): State<AppState>,
    Query(q): Query<ProxyListQuery>,
) -> Result<Json<WVPResult<ProxyListPage>>, AppError> {
    let page = q.page.unwrap_or(1);
    let count = q.count.unwrap_or(10).min(100);
    let total = stream_proxy::count_all(
        &state.pool,
        q.mediaServerId.as_deref(),
        q.pulling,
    )
    .await?;
    let list = stream_proxy::list_paged(
        &state.pool,
        page,
        count,
        q.mediaServerId.as_deref(),
        q.pulling,
    )
    .await?;
    Ok(Json(WVPResult::success(ProxyListPage {
        total: total as u64,
        list,
        page: page as u64,
        size: count as u64,
    })))
}

#[derive(Debug, serde::Serialize)]
pub struct ProxyListPage {
    pub total: u64,
    pub list: Vec<StreamProxy>,
    pub page: u64,
    pub size: u64,
}

/// GET /api/proxy/ffmpeg_cmd/list
pub async fn proxy_ffmpeg_cmd_list() -> Json<WVPResult<Vec<serde_json::Value>>> {
    Json(WVPResult::success(vec![]))
}

#[derive(Debug, Deserialize)]
pub struct ProxyAddBody {
    pub app: Option<String>,
    pub stream: Option<String>,
    pub src_url: Option<String>,
    pub media_server_id: Option<String>,
    pub name: Option<String>,
    pub enable_audio: Option<bool>,
    pub enable_mp4: Option<bool>,
}

pub async fn proxy_add(
    State(state): State<AppState>,
    Json(body): Json<ProxyAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let app = body.app.unwrap_or_else(|| "proxy".to_string());
    let stream = body.stream.unwrap_or_default();
    let src_url = body.src_url.unwrap_or_default();
    let media_server_id = body.media_server_id.unwrap_or_default();
    let name = body.name.unwrap_or_else(|| stream.clone());
    
    if stream.is_empty() || src_url.is_empty() {
        return Ok(Json(WVPResult::error("Stream and src_url are required")));
    }
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    match stream_proxy::add(&state.pool, &app, &stream, &src_url, &media_server_id, &name, &now).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "app": app,
                "stream": stream,
                "srcUrl": src_url,
                "mediaServerId": media_server_id,
                "name": name,
                "message": "Proxy stream added successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to add proxy stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProxyUpdateBody {
    pub id: Option<i64>,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub src_url: Option<String>,
    pub media_server_id: Option<String>,
    pub name: Option<String>,
}

pub async fn proxy_update(
    State(state): State<AppState>,
    Json(body): Json<ProxyUpdateBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    if id <= 0 {
        return Ok(Json(WVPResult::error("Proxy ID is required")));
    }
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    match stream_proxy::update(
        &state.pool,
        id,
        body.app.as_deref(),
        body.stream.as_deref(),
        body.src_url.as_deref(),
        body.media_server_id.as_deref(),
        body.name.as_deref(),
        &now,
    ).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "id": id,
                "message": "Proxy stream updated successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to update proxy stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}

pub async fn proxy_save(
    State(state): State<AppState>,
    Json(body): Json<ProxyAddBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let app = body.app.unwrap_or_else(|| "proxy".to_string());
    let stream = body.stream.unwrap_or_default();
    let src_url = body.src_url.unwrap_or_default();
    let media_server_id = body.media_server_id.unwrap_or_default();
    let name = body.name.unwrap_or_else(|| stream.clone());
    
    if stream.is_empty() || src_url.is_empty() {
        return Ok(Json(WVPResult::error("Stream and src_url are required")));
    }
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    match stream_proxy::add(&state.pool, &app, &stream, &src_url, &media_server_id, &name, &now).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "app": app,
                "stream": stream,
                "srcUrl": src_url,
                "mediaServerId": media_server_id,
                "name": name,
                "message": "Proxy stream saved successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to save proxy stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProxyStartBody {
    pub id: Option<i64>,
    pub stream: Option<String>,
    pub media_server_id: Option<String>,
}

pub async fn proxy_start(
    State(state): State<AppState>,
    Json(body): Json<ProxyStartBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    let stream_id = body.stream.clone().unwrap_or_default();
    
    let (proxy_stream, proxy_url, media_server_id, proxy_app) = if id > 0 {
        match stream_proxy::get_by_id(&state.pool, id as i64).await {
            Ok(Some(proxy)) => {
                let ms_id = proxy.media_server_id.clone().unwrap_or_default();
                let s = proxy.stream.clone().unwrap_or_default();
                let u = proxy.src_url.clone().unwrap_or_default();
                let a = proxy.app.clone().unwrap_or_else(|| "proxy".to_string());
                (Some(s), Some(u), Some(ms_id), Some(a))
            }
            Ok(None) => (None, None, None, None),
            Err(e) => {
                tracing::error!("Failed to get proxy info: {}", e);
                return Ok(Json(WVPResult::error("Database error")));
            }
        }
    } else {
        (Some(stream_id.clone()), None, body.media_server_id.clone(), None)
    };
    
    let stream = proxy_stream.unwrap_or_default();
    let src_url = proxy_url.unwrap_or_default();
    let ms_id = media_server_id.unwrap_or_default();
    let app = proxy_app.unwrap_or_else(|| "proxy".to_string());
    
    if stream.is_empty() || src_url.is_empty() {
        return Ok(Json(WVPResult::error("Stream and src_url are required")));
    }
    
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
    
    let request = crate::zlm::AddStreamProxyRequest {
        secret: zlm.secret.clone(),
        vhost: "__defaultVhost__".to_string(),
        app: app.clone(),
        stream: stream.clone(),
        url: src_url.clone(),
        rtp_type: Some(0),
        timeout_sec: Some(30.0),
        enable_hls: Some(false),
        enable_mp4: Some(false),
        enable_rtsp: Some(true),
        enable_rtmp: Some(true),
        enable_fmp4: Some(true),
        enable_ts: Some(false),
        enableAAC: Some(false),
    };
    
    match zlm.add_stream_proxy(&request).await {
        Ok(key) => {
            tracing::info!("Proxy stream started: {} -> {}", key, src_url);
            
            let stream_url = format!("{}/{}", app, stream);
            let play_url = format!("rtsp://127.0.0.1/live/{}", stream_url);
            let flv_url = format!("http://127.0.0.1/flv/live.app?stream={}", stream_url);
            
            Ok(Json(WVPResult::success(serde_json::json!({
                "app": app,
                "stream": stream,
                "streamKey": key,
                "playUrl": play_url,
                "flvUrl": flv_url,
                "wsUrl": format!("ws://127.0.0.1/live/{}", stream_url),
                "mediaServerId": ms_id,
                "message": "Proxy stream started successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to start proxy stream: {}", e);
            Ok(Json(WVPResult::error(format!("ZLM error: {}", e))))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProxyStopBody {
    pub id: Option<i64>,
    pub stream: Option<String>,
    pub media_server_id: Option<String>,
}

pub async fn proxy_stop(
    State(state): State<AppState>,
    Json(body): Json<ProxyStopBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    let stream_id = body.stream.clone().unwrap_or_default();
    
    let (stream, media_server_id, app) = if id > 0 {
        match stream_proxy::get_by_id(&state.pool, id as i64).await {
            Ok(Some(proxy)) => {
                let ms_id = proxy.media_server_id.clone().unwrap_or_default();
                let s = proxy.stream.clone().unwrap_or_default();
                let a = proxy.app.clone().unwrap_or_else(|| "proxy".to_string());
                (s, ms_id, a)
            }
            Ok(None) => (stream_id, String::new(), "proxy".to_string()),
            Err(e) => {
                tracing::error!("Failed to get proxy info: {}", e);
                return Ok(Json(WVPResult::error("Database error")));
            }
        }
    } else {
        (stream_id, body.media_server_id.unwrap_or_default(), "proxy".to_string())
    };
    
    if stream.is_empty() {
        return Ok(Json(WVPResult::error("Stream ID is required")));
    }
    
    let zlm_client = state.get_zlm_client(Some(&media_server_id));
    
    if let Some(zlm) = zlm_client {
        match zlm.close_streams(Some("rtsp"), Some(&app), Some(&stream), true).await {
            Ok(_) => {
                tracing::info!("Proxy stream stopped: {}/{}", app, stream);
                Ok(Json(WVPResult::success(serde_json::json!({
                    "stream": stream,
                    "message": "Proxy stream stopped successfully"
                }))))
            }
            Err(e) => {
                tracing::error!("Failed to stop proxy stream: {}", e);
                Ok(Json(WVPResult::error(format!("ZLM error: {}", e))))
            }
        }
    } else {
        Ok(Json(WVPResult::error("ZLM client not available")))
    }
}

#[derive(Debug, Deserialize)]
pub struct ProxyDeleteBody {
    pub id: Option<i64>,
    pub stream: Option<String>,
    pub media_server_id: Option<String>,
}

pub async fn proxy_delete(
    State(state): State<AppState>,
    Json(body): Json<ProxyDeleteBody>,
) -> Result<Json<WVPResult<serde_json::Value>>, AppError> {
    let id = body.id.unwrap_or(0);
    
    if id <= 0 {
        return Ok(Json(WVPResult::error("Proxy ID is required")));
    }
    
    if let Ok(Some(proxy)) = stream_proxy::get_by_id(&state.pool, id as i64).await {
        if proxy.pulling.unwrap_or(false) {
            if let Some(ref zlm_client) = state.zlm_client {
                let app = proxy.app.clone().unwrap_or_else(|| "proxy".to_string());
                let stream = proxy.stream.clone().unwrap_or_default();
                if !stream.is_empty() {
                    let _ = zlm_client.close_streams(Some("rtsp"), Some(&app), Some(&stream), true).await;
                }
            }
        }
    }
    
    match stream_proxy::delete_by_id(&state.pool, id as i64).await {
        Ok(_) => {
            Ok(Json(WVPResult::success(serde_json::json!({
                "id": id,
                "message": "Proxy stream deleted successfully"
            }))))
        }
        Err(e) => {
            tracing::error!("Failed to delete proxy stream: {}", e);
            Ok(Json(WVPResult::error(format!("Database error: {}", e))))
        }
    }
}
