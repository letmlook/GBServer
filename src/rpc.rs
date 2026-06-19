// ! Cross-node RPC — GBServer 节点间通信抽象
//!
//! 两种实现：
//! - LocalRpc  — 单机直接调用（无 Redis 时）
//! - RedisRpc  — Redis Pub/Sub 跨节点广播（多节点部署时）
//!
//! 支持的 RPC 目标：
//! - device_control(device_id, cmd) — 设备控制
//! - play_stop(device_id, channel_id) — 停止播放
//! - stream_state_changed(app, stream_id, online)
//! - cascade_sendrtp_start/stop
//! - cloud_record_sync(record_id)
//! - ws_broadcast(event, payload)

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum RpcTarget {
    /// 广播到所有节点
    Broadcast,
    /// 发送给特定节点
    Node(String),
    /// 发给持有该设备/流/通道的节点（基于一致性哈希）
    Key(String),
    /// 只本地处理
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub target: String,
    pub payload: serde_json::Value,
    pub reply_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub trait RpcTransport: Send + Sync {
    fn broadcast(&self, request: &RpcRequest) -> Result<(), String>;
    fn send_to(&self, node_id: &str, request: &RpcRequest) -> Result<(), String>;
    fn receive(&self) -> broadcast::Receiver<RpcRequest>;
    fn node_id(&self) -> &str;
}

/// 单机 RPC（无网络，所有调用直接执行）
pub struct LocalRpc {
    node_id: String,
    tx: broadcast::Sender<RpcRequest>,
}

impl LocalRpc {
    pub fn new(node_id: &str) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { node_id: node_id.to_string(), tx }
    }
}

impl RpcTransport for LocalRpc {
    fn broadcast(&self, request: &RpcRequest) -> Result<(), String> {
        self.tx.send(request.clone()).map(|_| ()).map_err(|e| e.to_string())
    }
    fn send_to(&self, _node_id: &str, request: &RpcRequest) -> Result<(), String> {
        self.tx.send(request.clone()).map(|_| ()).map_err(|e| e.to_string())
    }
    fn receive(&self) -> broadcast::Receiver<RpcRequest> {
        self.tx.subscribe()
    }
    fn node_id(&self) -> &str { &self.node_id }
}

/// RPC 路由器 — 将请求分发到本地处理器
pub struct RpcRouter {
    handlers: RwLock<HashMap<String, Box<dyn RpcHandler>>>,
}

pub trait RpcHandler: Send + Sync {
    fn name(&self) -> &str;
    fn handle(&self, payload: serde_json::Value) -> RpcResponse;
}

impl RpcRouter {
    pub fn new() -> Self {
        Self { handlers: RwLock::new(HashMap::new()) }
    }

    pub async fn register<H: RpcHandler + 'static>(&self, handler: H) {
        let name = handler.name().to_string();
        self.handlers.write().await.insert(name, Box::new(handler));
    }

    pub async fn route(&self, request: &RpcRequest) -> RpcResponse {
        let handlers = self.handlers.read().await;
        if let Some(handler) = handlers.get(&request.method) {
            handler.handle(request.payload.clone())
        } else {
            RpcResponse { ok: false, result: None, error: Some(format!("Unknown method: {}", request.method)) }
        }
    }

    pub async fn spawn_listener<T: RpcTransport + 'static>(&self, transport: Arc<T>) {
        let router = Arc::new(self.clone());
        // 必须在 spawn 前同步 subscribe —— tokio::broadcast::Sender::send 在没有
        // 活跃 receiver 时会返回 SendError(Closed)，导致紧随 spawn_listener
        // 的 broadcast 因 race 而失败。原实现把 subscribe 放在 spawned task 内，
        // 测试中 broadcast 在 task 还没被 poll 之前就触发，因此报 "channel closed"。
        let mut rx = transport.receive();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(request) => {
                        let response = router.route(&request).await;
                        tracing::debug!("RPC {} handled, ok={}", request.method, response.ok);
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("RPC lag: dropped {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

impl Clone for RpcRouter {
    fn clone(&self) -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for RpcRouter {
    fn default() -> Self { Self::new() }
}

/// 注册标准 RPC 处理器
pub async fn register_standard_handlers(router: &RpcRouter) {
    // device_control 处理器
    struct DeviceControlHandler;
    impl RpcHandler for DeviceControlHandler {
        fn name(&self) -> &str { "device_control" }
        fn handle(&self, payload: serde_json::Value) -> RpcResponse {
            RpcResponse { ok: true, result: Some(payload), error: None }
        }
    }
    // play_stop 处理器
    struct PlayStopHandler;
    impl RpcHandler for PlayStopHandler {
        fn name(&self) -> &str { "play_stop" }
        fn handle(&self, payload: serde_json::Value) -> RpcResponse {
            RpcResponse { ok: true, result: Some(payload), error: None }
        }
    }
    // cloud_record_sync 处理器
    struct CloudRecordHandler;
    impl RpcHandler for CloudRecordHandler {
        fn name(&self) -> &str { "cloud_record_sync" }
        fn handle(&self, payload: serde_json::Value) -> RpcResponse {
            RpcResponse { ok: true, result: Some(payload), error: None }
        }
    }

    router.register(DeviceControlHandler).await;
    router.register(PlayStopHandler).await;
    router.register(CloudRecordHandler).await;
}

// ---------------------------------------------------------------------------
// E2: HTTP-over-JSON RPC 客户端
// ---------------------------------------------------------------------------

/// HTTP RPC 客户端配置
#[derive(Debug, Clone)]
pub struct HttpRpcConfig {
    pub peer_endpoints: Vec<String>, // 例如 ["http://node2:18080", "http://node3:18080"]
    pub timeout_secs: u64,
}

impl Default for HttpRpcConfig {
    fn default() -> Self {
        Self {
            peer_endpoints: Vec::new(),
            timeout_secs: 5,
        }
    }
}

/// HTTP RPC 客户端 — 用 reqwest POST /api/rpc 把 RpcRequest 投递到远端节点
pub struct HttpRpc {
    node_id: String,
    config: HttpRpcConfig,
    http: reqwest::Client,
}

impl HttpRpc {
    pub fn new(node_id: &str, config: HttpRpcConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { node_id: node_id.to_string(), config, http }
    }

    pub async fn send_request(&self, endpoint: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
        let url = format!("{}/api/rpc", endpoint.trim_end_matches('/'));
        let resp = self.http
            .post(&url)
            .json(request)
            .send()
            .await
            .map_err(|e| format!("HTTP send failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} from {}", resp.status(), url));
        }
        resp.json::<RpcResponse>().await
            .map_err(|e| format!("decode RpcResponse failed: {}", e))
    }

    pub async fn broadcast(&self, request: &RpcRequest) -> Vec<(String, Result<RpcResponse, String>)> {
        let mut results = Vec::new();
        for ep in &self.config.peer_endpoints {
            let r = self.send_request(ep, request).await;
            results.push((ep.clone(), r));
        }
        results
    }

    pub async fn send_to(&self, node_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
        // 简单按 node_id 匹配 endpoint（生产可用 service discovery）
        let endpoint = self.config.peer_endpoints.iter()
            .find(|e| e.contains(node_id))
            .ok_or_else(|| format!("No endpoint for node_id={}", node_id))?;
        self.send_request(endpoint, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_rpc_broadcast() {
        let rpc = Arc::new(LocalRpc::new("node-1"));
        let router = Arc::new(RpcRouter::new());

        register_standard_handlers(&router).await;
        router.spawn_listener(rpc.clone()).await;

        rpc.broadcast(&RpcRequest {
            method: "device_control".to_string(),
            target: "Broadcast".to_string(),
            payload: serde_json::json!({"device_id": "dev1", "cmd": "stop"}),
            reply_to: None,
        }).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[test]
    fn test_rpc_response() {
        let resp = RpcResponse {
            ok: true,
            result: Some(serde_json::json!({"ok": true})),
            error: None,
        };
        assert!(resp.ok);
    }

    /// E2: HttpRpcConfig 默认值
    #[test]
    fn test_http_rpc_config_default() {
        let c = HttpRpcConfig::default();
        assert!(c.peer_endpoints.is_empty());
        assert_eq!(c.timeout_secs, 5);
    }

    /// E2: HttpRpc 构造不应 panic
    #[test]
    fn test_http_rpc_new() {
        let rpc = HttpRpc::new("node-1", HttpRpcConfig::default());
        assert_eq!(rpc.node_id, "node-1");
    }

    /// E2: HTTP URL 拼接正确
    #[tokio::test]
    async fn test_http_rpc_url_building() {
        let rpc = HttpRpc::new("node-1", HttpRpcConfig {
            peer_endpoints: vec!["http://node2:18080".to_string()],
            ..Default::default()
        });
        let req = RpcRequest {
            method: "device_control".to_string(),
            target: "Node:node2".to_string(),
            payload: serde_json::json!({"device_id": "dev1"}),
            reply_to: None,
        };
        // 实际发送会失败（无 server）—— send_to 应当能找到 endpoint 并尝试
        // send_request；我们要验证的是 endpoint 查找逻辑正确，即错误不应来自
        // endpoint 查找阶段，而是来自 HTTP 发送阶段。
        let result = rpc.send_to("node2", &req).await;
        let err = result.expect_err("无 server 时 send_to 应返回 Err（HTTP 失败）");
        assert!(
            !err.starts_with("No endpoint for"),
            "endpoint 查找应成功（url=http://node2:18080 含 node2），但 send_to 返回 endpoint 错误: {}",
            err
        );
    }

    /// E2: send_to 不存在的 node_id 应返回错误
    #[tokio::test]
    async fn test_http_rpc_send_to_unknown_node() {
        let rpc = HttpRpc::new("node-1", HttpRpcConfig::default());
        let req = RpcRequest {
            method: "device_control".to_string(),
            target: "Node:unknown".to_string(),
            payload: serde_json::json!({}),
            reply_to: None,
        };
        let result = rpc.send_to("unknown", &req).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("No endpoint") || err.contains("HTTP"), "错误信息: {}", err);
    }

    /// E2: 端到端 mock 远端返回 RpcResponse
    #[tokio::test]
    async fn test_http_rpc_roundtrip_via_mock_server() {
        use std::net::SocketAddr;
        use axum::{routing::post, Router as AxRouter};
        use axum::extract::Json;
        use axum::response::IntoResponse;

        async fn echo(Json(req): Json<RpcRequest>) -> impl IntoResponse {
            Json(RpcResponse {
                ok: true,
                result: Some(req.payload),
                error: None,
            })
        }

        let app: AxRouter = AxRouter::new().route("/api/rpc", post(echo));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        // 给 server 一点启动时间
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let rpc = HttpRpc::new("node-1", HttpRpcConfig {
            peer_endpoints: vec![format!("http://{}", addr)],
            timeout_secs: 2,
        });

        let req = RpcRequest {
            method: "device_control".to_string(),
            target: "Broadcast".to_string(),
            payload: serde_json::json!({"device_id": "dev-abc"}),
            reply_to: None,
        };

        let resp = rpc.send_request(&format!("http://{}", addr), &req).await.unwrap();
        assert!(resp.ok);
        assert_eq!(resp.result.unwrap()["device_id"], "dev-abc");
    }

    /// E2: HttpRpc.broadcast 对多 endpoint 扇出
    #[tokio::test]
    async fn test_http_rpc_broadcast_returns_per_endpoint_results() {
        use axum::{routing::post, Router as AxRouter};
        use axum::extract::Json;
        use axum::response::IntoResponse;

        async fn echo(Json(req): Json<RpcRequest>) -> impl IntoResponse {
            Json(RpcResponse {
                ok: true,
                result: Some(req.payload),
                error: None,
            })
        }

        // 起两个独立 mock server
        let app1: AxRouter = AxRouter::new().route("/api/rpc", post(echo));
        let app2: AxRouter = AxRouter::new().route("/api/rpc", post(echo));
        let l1 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a1 = l1.local_addr().unwrap();
        let l2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a2 = l2.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l1, app1).await.unwrap(); });
        tokio::spawn(async move { axum::serve(l2, app2).await.unwrap(); });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let rpc = HttpRpc::new("node-1", HttpRpcConfig {
            peer_endpoints: vec![format!("http://{}", a1), format!("http://{}", a2)],
            timeout_secs: 2,
        });

        let req = RpcRequest {
            method: "play_stop".to_string(),
            target: "Broadcast".to_string(),
            payload: serde_json::json!({"a": 1}),
            reply_to: None,
        };
        let results = rpc.broadcast(&req).await;
        assert_eq!(results.len(), 2);
        for (ep, r) in results {
            assert!(r.is_ok(), "endpoint={} 应成功", ep);
        }
    }
}
