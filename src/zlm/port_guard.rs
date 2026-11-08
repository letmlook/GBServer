use crate::AppState;

use super::ZlmClient;

/// Owns an RTP allocation until it is handed to a live session.
/// The stream ID must be unique to this allocation, including on failed opens.
pub struct RtpServerLease {
    client: ZlmClient,
    stream_id: String,
    armed: bool,
}

impl RtpServerLease {
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RtpServerLease {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let client = self.client.clone();
        let stream_id = self.stream_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Err(error) = client.close_rtp_server(&stream_id).await {
                    tracing::warn!(%stream_id, "清理 RTP 端口失败: {}", error);
                }
            });
        }
    }
}

/// Open a uniquely named RTP server with cancellation-safe ownership transfer.
pub async fn open_owned_rtp_server(
    client: &ZlmClient,
    request: super::OpenRtpServerRequest,
) -> anyhow::Result<(super::RtpServerInfo, RtpServerLease)> {
    let client = client.clone();
    tokio::spawn(async move {
        let lease = RtpServerLease {
            client: client.clone(),
            stream_id: request.stream_id.clone(),
            armed: true,
        };
        let info = client.open_rtp_server(&request).await?;
        Ok((info, lease))
    })
    .await?
}

fn rtp_port_range_problem(port: u16, range: &str) -> Option<String> {
    let (start, end) = match super::client::parse_port_range(range) {
        Ok(bounds) => bounds,
        Err(_) => return Some(format!("RTP 端口范围配置无效：{range}")),
    };
    if start > end {
        return Some(format!("RTP 端口范围配置无效：{range}"));
    }
    if port == 0 || !(start..=end).contains(&port) {
        return Some(format!("分配端口 {port} 不在配置范围 {range} 内"));
    }
    None
}

/// 在把 ZLM 分配的 RTP 端口交给设备前核对节点声明的范围。
/// 无效端口会立即关闭；返回值已包含节点、流和请求上下文，可直接用于业务错误。
pub async fn validate_allocated_rtp_port(
    state: &AppState,
    client: &ZlmClient,
    stream_id: &str,
    port: u16,
    request: &str,
) -> Result<(), String> {
    let configured_range = match crate::db::media_server::list_media_servers(&state.pool).await {
        Ok(servers) => servers
            .into_iter()
            .find(|server| {
                server.ip.as_deref() == Some(client.ip.as_str())
                    && server.http_port == Some(client.http_port as i32)
            })
            .and_then(|server| server.rtp_port_range),
        Err(error) => {
            tracing::warn!(%stream_id, "查询媒体节点 RTP 端口范围失败: {error}");
            None
        }
    }
    .or_else(|| {
        state.config.zlm.as_ref().and_then(|zlm| {
            zlm.servers
                .iter()
                .find(|server| server.ip == client.ip && server.http_port == client.http_port)
                .and_then(|server| server.rtp_port_range.clone())
        })
    });
    let problem = if port == 0 {
        Some("ZLM 未分配有效的 RTP 端口".to_string())
    } else {
        configured_range
            .as_deref()
            .and_then(|range| rtp_port_range_problem(port, range))
    };
    let Some(problem) = problem else {
        return Ok(());
    };
    tracing::error!(%stream_id, node = %client.ip, http_port = client.http_port, %problem,
        "ZLM RTP 端口与节点配置不一致，取消请求");
    if let Err(error) = client.close_rtp_server(stream_id).await {
        tracing::warn!(%stream_id, "关闭无效 RTP 端口失败: {error}");
    }
    Err(format!(
        "RTP 收流端口不可用（节点 {}:{}, 流 {}, 请求 {}）：{}",
        client.ip, client.http_port, stream_id, request, problem
    ))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn cancelled_open_releases_eventual_rtp_allocation() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let entered = Arc::new(tokio::sync::Notify::new());
        let proceed = Arc::new(tokio::sync::Notify::new());
        let closed = Arc::new(AtomicUsize::new(0));
        let (a, b, c) = (entered.clone(), proceed.clone(), closed.clone());
        let router = axum::Router::new()
            .route(
                "/index/api/openRtpServer",
                axum::routing::get(move || {
                    let (a, b) = (a.clone(), b.clone());
                    async move {
                        a.notify_one();
                        b.notified().await;
                        axum::Json(serde_json::json!({"code":0,"port":30001}))
                    }
                }),
            )
            .route(
                "/index/api/closeRtpServer",
                axum::routing::get(move || {
                    let c = c.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        axum::Json(serde_json::json!({"code":0,"hit":1}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = super::ZlmClient::new("127.0.0.1", port, "test");
        let request = super::super::OpenRtpServerRequest {
            stream_id: "unique-test".into(),
            port: Some(0),
            ..Default::default()
        };
        let task =
            tokio::spawn(async move { super::open_owned_rtp_server(&client, request).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        task.abort();
        let _ = task.await;
        proceed.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while closed.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("RTP allocation leaked");
        server.abort();
    }
    #[test]
    fn allocated_rtp_port_must_fit_configured_range() {
        assert!(super::rtp_port_range_problem(30050, "30000-30100").is_none());
        assert!(super::rtp_port_range_problem(30050, "30000,30100").is_none());
        assert!(super::rtp_port_range_problem(30050, "40000-40100")
            .unwrap()
            .contains("30050"));
        assert!(super::rtp_port_range_problem(0, "30000-30100").is_some());
        assert!(super::rtp_port_range_problem(30050, "invalid").is_some());
        assert!(super::rtp_port_range_problem(30050, "30100-30000").is_some());
    }
}
