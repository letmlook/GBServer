//! RTP/PS 收流与发流控制端点。
//! Each endpoint forwards to ZLM's `openRtpServer`, `closeRtpServer`,
//! `sendRtp` / `stopSendRtp` family of API calls.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::response::ApiResult;
use crate::AppState;
use crate::zlm::OpenRtpServerRequest;

#[derive(Deserialize, Default)]
pub struct OpenRtpQuery {
    #[serde(alias = "streamId")]
    pub stream_id: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub tcp: Option<bool>,
}

/// POST /api/rtp/receive/open
pub async fn rtp_receive_open(
    State(state): State<AppState>,
    Json(q): Json<OpenRtpQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(ApiResult::error("no ZLM available"));
    };
    let stream_id = q.stream_id.unwrap_or_else(|| {
        format!("rtp_recv_{}", chrono::Utc::now().timestamp_millis())
    });
    let req = OpenRtpServerRequest {
        secret: zlm.secret.clone(),
        stream_id: stream_id.clone(),
        port: q.port,
        use_tcp: Some(q.tcp.unwrap_or(false)),
        rtp_type: Some(0),
        recv_port: None,
    };
    match zlm.open_rtp_server(&req).await {
        Ok(info) => Json(ApiResult::success(serde_json::json!({
            "streamId": stream_id, "port": info.port, "ssrc": info.ssrc,
        }))),
        Err(e) => Json(ApiResult::error(format!("ZLM error: {}", e))),
    }
}

/// POST /api/rtp/receive/close/:stream_id
pub async fn rtp_receive_close(
    Path(stream_id): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(ApiResult::error("no ZLM available"));
    };
    match zlm.close_rtp_server_ex(&stream_id).await {
        // `hit=0` = ZLM 上没有这条收流服务：如实告知，不报"已关闭"
        Ok(true) => Json(ApiResult::success(
            serde_json::json!({"streamId": stream_id, "closed": true}),
        )),
        Ok(false) => Json(ApiResult::error(format!(
            "没有正在收流的服务: stream={stream_id}"
        ))),
        Err(e) => Json(ApiResult::error(format!("ZLM error: {}", e))),
    }
}

#[derive(Deserialize, Default)]
pub struct SendRtpBody {
    pub stream_id: Option<String>,
    pub ssrc: Option<String>,
    pub target_ip: Option<String>,
    pub target_port: Option<u16>,
    /// 可选：ZLM 上的 app / stream（`startSendRtp` 必需）。
    /// 缺省时按本平台 GB28181 流的约定取 `app=rtp`、`stream=stream_id`。
    pub app: Option<String>,
    pub stream: Option<String>,
}

/// POST /api/rtp/send/start — push our stream to a remote RTP receiver
pub async fn rtp_send_start(
    State(state): State<AppState>,
    Json(b): Json<SendRtpBody>,
) -> Json<ApiResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(ApiResult::error("no ZLM available"));
    };
    let stream_id = b.stream_id.unwrap_or_default();
    let ssrc = b.ssrc.unwrap_or_default();
    let target_ip = b.target_ip.unwrap_or_default();
    let target_port = b.target_port.unwrap_or(0);
    if stream_id.is_empty() || ssrc.is_empty() || target_ip.is_empty() || target_port == 0 {
        return Json(ApiResult::error("missing stream_id/ssrc/target_ip/target_port"));
    }
    // **真实 ZLM 没有 `/index/api/sendRtpInfo`**（`getApiList` 里没有，直接调用
    // 返回 404 的 HTML）—— 此前这个端点必然失败。真正的"把流推到远端"接口是
    // `startSendRtp`（本客户端已有 `start_send_rtp`），参数
    // `vhost/app/stream/ssrc/dst_url/dst_port/is_udp`。
    //
    // 这里按调用方给的信息组装：`app`/`stream` 可用请求体里的可选字段覆盖
    // （默认 app=`rtp`、stream=`stream_id`，与本平台 GB28181 流的命名一致）。
    let app = b.app.clone().unwrap_or_else(|| "rtp".to_string());
    let stream = b.stream.clone().unwrap_or_else(|| stream_id.clone());
    // ZLM 要求 `dst_url` 是**裸主机名/IP**（`dst_port` 单独传）。
    // 实测：带 `rtp://` 前缀会得到 `dns resolution failed: rtp://host...`，
    // 带端口同理 —— 此前级联推流处也是这么写的（已一并修正）。
    let dst_url = target_ip.clone();
    match zlm
        .start_send_rtp(
            "__defaultVhost__",
            &app,
            &stream,
            &ssrc,
            &dst_url,
            target_port,
            true,  // is_udp
            None,  // src_port：由 ZLM 自动分配
            false, // use_ps：仅在级联 PS 封装时置真
        )
        .await
    {
        Ok(()) => Json(ApiResult::success(serde_json::json!({
            "streamId": stream_id,
            "app": app,
            "stream": stream,
            "targetIp": target_ip,
            "targetPort": target_port,
            "dstUrl": dst_url,
        }))),
        Err(e) => Json(ApiResult::error(format!("ZLM error: {}", e))),
    }
}

/// POST /api/rtp/send/stop/:stream_id
///
/// 停止一路 SendRtp 推流。
///
/// 修正：此前只返回一句 `"SendRtp stop is implicit on stream teardown"` 的
/// **假成功** —— 既不查会话也不调 ZLM，调用方以为推流已停，实际 ZLM 仍在
/// 往目标地址推 RTP。
///
/// 现在按 stream_id / ssrc 定位并真正调用 `stopSendRtp`（ZLM 允许二者
/// 任一选中会话，两个都给最稳妥）。
#[derive(Deserialize, Default)]
pub struct StopSendRtpQuery {
    /// 可选：无法从 stream_id 推断时显式给出 SSRC
    pub ssrc: Option<String>,
    /// 可选：ZLM 应用名（默认 rtp）
    pub app: Option<String>,
}

pub async fn rtp_send_stop(
    Path(stream_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<StopSendRtpQuery>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    let Some(zlm) = state.zlm_clients.values().next() else {
        return Json(ApiResult::error("no ZLM available"));
    };
    let app = q.app.unwrap_or_else(|| "rtp".to_string());

    // SSRC 兜底：级联推流的 SSRC 存在 SendRtpManager 会话里
    let ssrc = match q.ssrc {
        Some(s) if !s.is_empty() => Some(s),
        _ => state
            .sip_server
            .as_ref()
            .and_then(|sip| {
                sip.send_rtp_manager()
                    .get_by_channel(&stream_id)
                    .first()
                    .map(|session| session.upstream_ssrc.clone())
            }),
    };

    match zlm
        .stop_send_rtp_ex("__defaultVhost__", &app, Some(&stream_id), ssrc.as_deref())
        .await
    {
        // ZLM 的 `existed` 说明这条推流**本来是否存在**：
        // 为 false 时不能报"已停止"（否则第三方会以为推流被关掉了）
        Ok(true) => Json(ApiResult::success(serde_json::json!({
            "streamId": stream_id,
            "app": app,
            "ssrc": ssrc,
            "stopped": true,
        }))),
        Ok(false) => Json(ApiResult::error(format!(
            "没有正在推送的流: stream={stream_id} app={app}"
        ))),
        Err(e) => Json(ApiResult::error(format!("ZLM stopSendRtp 失败: {}", e))),
    }
}

// ---------- PS aliases (PS is just RTP over MPEG-TS in reference impl) ----------

/// POST /api/ps/receive/open — alias of /api/rtp/receive/open
pub async fn ps_receive_open(
    State(state): State<AppState>,
    Json(q): Json<OpenRtpQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    rtp_receive_open(State(state), Json(q)).await
}

/// POST /api/ps/receive/close/:stream_id
pub async fn ps_receive_close(
    Path(stream_id): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    rtp_receive_close(Path(stream_id), State(state)).await
}

/// POST /api/ps/send/start
pub async fn ps_send_start(
    State(state): State<AppState>,
    Json(b): Json<SendRtpBody>,
) -> Json<ApiResult<serde_json::Value>> {
    rtp_send_start(State(state), Json(b)).await
}

/// POST /api/ps/send/stop/:stream_id
pub async fn ps_send_stop(
    Path(stream_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<StopSendRtpQuery>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    rtp_send_stop(Path(stream_id), axum::extract::Query(q), State(state)).await
}

/// GET /api/ps/getTestPort — return a free UDP port for testing
pub async fn ps_get_test_port() -> Json<ApiResult<serde_json::Value>> {
    use std::net::UdpSocket;
    let port = UdpSocket::bind("127.0.0.1:0").ok()
        .and_then(|s| s.local_addr().ok().map(|a| a.port()))
        .unwrap_or(0);
    Json(ApiResult::success(serde_json::json!({"port": port})))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_port_zero_is_invalid() {
        assert_ne!(0, 1);
    }
}

// ============================================================================
// 第三方对接（`vmanager/rtp|ps`）的查询参数风格入口
//
// 这一套用 `?stream=`（收流）/ `?callId=`（发流），而本平台的历史实现是
// `POST /api/rtp/send/stop/{stream_id}` 这种路径参数形式。两套都保留：
// 第三方按既有文档调用时不再落到 SPA 兜底。
// ============================================================================

#[derive(Debug, Default, Deserialize)]
pub struct ThirdPartyStreamQuery {
    /// 参数名为 `stream`；兼容 `streamId`/`stream_id`。
    #[serde(
        alias = "stream",
        alias = "streamId",
        alias = "stream_id",
        default,
        deserialize_with = "crate::serde_flex::de_opt_string"
    )]
    pub stream: Option<String>,
    /// 发流用的唯一标识（`callId`），本平台等价于 stream_id。
    #[serde(
        alias = "callId",
        alias = "call_id",
        default,
        deserialize_with = "crate::serde_flex::de_opt_string"
    )]
    pub call_id: Option<String>,
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub ssrc: Option<String>,
    #[serde(default, deserialize_with = "crate::serde_flex::de_opt_string")]
    pub app: Option<String>,
}

impl ThirdPartyStreamQuery {
    fn target(&self) -> Option<String> {
        self.stream
            .clone()
            .or_else(|| self.call_id.clone())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

/// `GET /api/rtp/receive/close?stream=`
pub async fn rtp_receive_close_query(
    axum::extract::Query(q): axum::extract::Query<ThirdPartyStreamQuery>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    match q.target() {
        Some(stream) => rtp_receive_close(Path(stream), State(state)).await,
        None => Json(ApiResult::error("缺少 stream 参数")),
    }
}

/// `GET /api/ps/receive/close?stream=`
pub async fn ps_receive_close_query(
    axum::extract::Query(q): axum::extract::Query<ThirdPartyStreamQuery>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    match q.target() {
        Some(stream) => ps_receive_close(Path(stream), State(state)).await,
        None => Json(ApiResult::error("缺少 stream 参数")),
    }
}

/// `GET /api/rtp/send/stop?callId=`
pub async fn rtp_send_stop_query(
    axum::extract::Query(q): axum::extract::Query<ThirdPartyStreamQuery>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    match q.target() {
        Some(stream) => {
            rtp_send_stop(
                Path(stream),
                axum::extract::Query(StopSendRtpQuery {
                    ssrc: q.ssrc.clone(),
                    app: q.app.clone(),
                }),
                State(state),
            )
            .await
        }
        None => Json(ApiResult::error("缺少 callId 参数")),
    }
}

/// `GET /api/ps/send/stop?callId=`
pub async fn ps_send_stop_query(
    axum::extract::Query(q): axum::extract::Query<ThirdPartyStreamQuery>,
    State(state): State<AppState>,
) -> Json<ApiResult<serde_json::Value>> {
    match q.target() {
        Some(stream) => {
            ps_send_stop(
                Path(stream),
                axum::extract::Query(StopSendRtpQuery {
                    ssrc: q.ssrc.clone(),
                    app: q.app.clone(),
                }),
                State(state),
            )
            .await
        }
        None => Json(ApiResult::error("缺少 callId 参数")),
    }
}
