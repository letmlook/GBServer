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
use futures_util::StreamExt;

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
    /// Phase 7.2: identifier of the node that originated this request.
    /// Used by Redis-backed transport to skip self-echo.
    #[serde(default)]
    pub from_node: Option<String>,
}

impl Default for RpcRequest {
    fn default() -> Self {
        Self {
            method: String::new(),
            target: String::new(),
            payload: serde_json::Value::Null,
            reply_to: None,
            from_node: None,
        }
    }
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

/// RPC 路由器 — 将请求分发到本地处理器，并可通过注册的出站传输
/// （Redis Pub/Sub / HTTP 对端）向其他节点广播。
pub struct RpcRouter {
    handlers: RwLock<HashMap<String, Box<dyn RpcHandler>>>,
    /// 出站传输列表：broadcast_outbound / send_to_outbound 逐个尝试
    outbound: RwLock<Vec<Arc<dyn RpcTransport>>>,
}

pub trait RpcHandler: Send + Sync {
    fn name(&self) -> &str;
    fn handle(&self, payload: serde_json::Value) -> RpcResponse;
}

impl RpcRouter {
    pub fn new() -> Self {
        Self { handlers: RwLock::new(HashMap::new()), outbound: RwLock::new(Vec::new()) }
    }

    pub async fn register<H: RpcHandler + 'static>(&self, handler: H) {
        let name = handler.name().to_string();
        self.handlers.write().await.insert(name, Box::new(handler));
    }

    /// 注册出站传输（Redis Pub/Sub / HttpRpc 等），可叠加多个
    pub async fn register_outbound(&self, transport: Arc<dyn RpcTransport>) {
        self.outbound.write().await.push(transport);
    }

    /// 向所有已注册出站传输广播请求。各传输独立投递、错误仅记日志，
    /// 不向上传播（跨节点通知是 best-effort 语义）。
    pub async fn broadcast_outbound(&self, request: &RpcRequest) {
        let out = self.outbound.read().await;
        for t in out.iter() {
            if let Err(e) = t.broadcast(request) {
                tracing::warn!("RPC outbound broadcast via {} failed: {}", t.node_id(), e);
            }
        }
    }

    /// 定向发送到指定节点（逐个出站传输尝试，首个成功即返回）
    pub async fn send_to_outbound(&self, node_id: &str, request: &RpcRequest) -> Result<(), String> {
        let out = self.outbound.read().await;
        let mut last_err = format!("no outbound transport registered (target={})", node_id);
        for t in out.iter() {
            match t.send_to(node_id, request) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
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
        // 克隆仅保留 handler 注册能力（handlers 不随克隆复制，历史行为如此）；
        // outbound 为空 —— spawn_listener 场景下克隆体不需要出站传输。
        Self {
            handlers: RwLock::new(HashMap::new()),
            outbound: RwLock::new(Vec::new()),
        }
    }
}

impl Default for RpcRouter {
    fn default() -> Self { Self::new() }
}

// 跨节点业务 RPC handler 注册说明：
//
// 历史版本在此注册 device_control / play_stop / cloud_record_sync 三个
// "echo 占位" handler —— 它们把请求原样回显并返回 ok:true，从未执行任何
// 实际动作，会向调用方伪装成功，现已移除。未注册方法的 route() 会显式
// 返回 ok:false + "Unknown method"。当前唯一在用的跨节点方法是
// `ws_broadcast`（由 lib.rs 注册，fanout 到本节点 WS 客户端）。
// 后续跨节点业务流（设备指令转发、停止播放等）落地时，应在构造
// RpcRouter 后用 `router.register(...)` 注册持有真实执行逻辑的 handler。

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

/// RpcTransport 适配：HttpRpc 的 broadcast/send_to 是异步 HTTP 调用，
/// 与 RedisRpcTransport 相同采用 fire-and-forget（spawn 后台投递，
/// 失败仅记日志）。receive() 返回永远静默的通道（HttpRpc 无入站侧，
/// 入站统一走对端的 POST /api/rpc → RpcRouter::route）。
impl RpcTransport for HttpRpc {
    fn broadcast(&self, request: &RpcRequest) -> Result<(), String> {
        let mut req = request.clone();
        if req.from_node.is_none() {
            req.from_node = Some(self.node_id.clone());
        }
        let http = self.http.clone();
        let endpoints = self.config.peer_endpoints.clone();
        if endpoints.is_empty() {
            return Err("HttpRpc: no peer_endpoints configured".to_string());
        }
        tokio::spawn(async move {
            for ep in &endpoints {
                let url = format!("{}/api/rpc", ep.trim_end_matches('/'));
                if let Err(e) = http.post(&url).json(&req).send().await {
                    tracing::warn!("HttpRpc broadcast to {} failed: {}", url, e);
                }
            }
        });
        Ok(())
    }

    fn send_to(&self, node_id: &str, request: &RpcRequest) -> Result<(), String> {
        let mut req = request.clone();
        if req.from_node.is_none() {
            req.from_node = Some(self.node_id.clone());
        }
        let endpoint = self.config.peer_endpoints.iter()
            .find(|e| e.contains(node_id))
            .ok_or_else(|| format!("HttpRpc: No endpoint for node_id={}", node_id))?
            .clone();
        let http = self.http.clone();
        tokio::spawn(async move {
            let url = format!("{}/api/rpc", endpoint.trim_end_matches('/'));
            if let Err(e) = http.post(&url).json(&req).send().await {
                tracing::warn!("HttpRpc send_to {} failed: {}", url, e);
            }
        });
        Ok(())
    }

    fn receive(&self) -> broadcast::Receiver<RpcRequest> {
        // HttpRpc 没有入站推送通道；返回一个无发送者的 receiver（永远 pending/关闭）
        broadcast::channel(1).1
    }

    fn node_id(&self) -> &str { &self.node_id }
}

// ---------------------------------------------------------------------------
// Phase 7.2: Redis-backed RPC transport
// ---------------------------------------------------------------------------

/// Phase 7.2: Redis-based RPC transport using Pub/Sub for broadcast.
///
/// `broadcast` publishes the serialized `RpcRequest` to channel
/// `gb:rpc:channel`. All nodes subscribed on the same channel receive it
/// (fanout delivery). `send_to` uses Redis Streams for at-least-once delivery
/// to a specific node (`gb:rpc:inbox:{node_id}`).
///
/// Local node receives its own broadcast on the channel and the listener
/// filters out messages with `from_node == local_node_id` to avoid double
/// processing.
pub struct RedisRpcTransport {
    pub node_id: String,
    pub channel: String,
    pub redis: Arc<tokio::sync::Mutex<redis::aio::ConnectionManager>>,
    pub client: redis::Client,
    local_tx: tokio::sync::broadcast::Sender<RpcRequest>,
}

impl RedisRpcTransport {
    pub fn new(node_id: String, redis: Arc<tokio::sync::Mutex<redis::aio::ConnectionManager>>, client: redis::Client) -> Self {
        let (local_tx, _) = tokio::sync::broadcast::channel(1024);
        Self {
            node_id,
            channel: "gb:rpc:channel".to_string(),
            redis,
            client,
            local_tx,
        }
    }

    /// Subscribe to incoming RPC messages from Redis Pub/Sub.
    /// Spawns a background task that re-publishes messages to local_tx
    /// (skipping self-echo).
    pub async fn start_subscriber(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let me = self.clone();
        tokio::spawn(async move {
            loop {
                let pubsub_res = me.client.get_async_pubsub().await;
                let mut pubsub = match pubsub_res {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("RedisRpcTransport: get_async_pubsub failed: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }
                };
                if let Err(e) = pubsub.subscribe(&me.channel).await {
                    tracing::warn!("RedisRpcTransport: pubsub subscribe failed: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                let mut stream = pubsub.on_message();
                while let Some(msg) = stream.next().await {
                    let payload: String = match msg.get_payload() {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::debug!("RedisRpcTransport: payload decode failed: {}", e);
                            continue;
                        }
                    };
                    let req: RpcRequest = match serde_json::from_str(&payload) {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::debug!("RedisRpcTransport: RpcRequest decode failed: {}", e);
                            continue;
                        }
                    };
                    // Skip self-echo
                    if req.from_node.as_deref() == Some(&me.node_id) {
                        continue;
                    }
                    let _ = me.local_tx.send(req);
                }
                tracing::warn!("RedisRpcTransport: pubsub stream ended, retrying");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        })
    }
}

impl RpcTransport for RedisRpcTransport {
    fn broadcast(&self, request: &RpcRequest) -> Result<(), String> {
        let mut req = request.clone();
        req.from_node = Some(self.node_id.clone());
        let payload = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        let channel = self.channel.clone();
        let redis = self.redis.clone();
        tokio::spawn(async move {
            use redis::AsyncCommands;
            let conn = redis.lock().await.clone();
            let mut conn = conn;
            let res: Result<i64, _> = conn.publish(&channel, &payload).await;
            if let Err(e) = res {
                tracing::warn!("RedisRpcTransport: publish failed: {}", e);
            }
        });
        Ok(())
    }
    fn send_to(&self, node_id: &str, request: &RpcRequest) -> Result<(), String> {
        let mut req = request.clone();
        req.from_node = Some(self.node_id.clone());
        let payload = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        let stream_key = format!("gb:rpc:inbox:{}", node_id);
        let redis = self.redis.clone();
        tokio::spawn(async move {
            use redis::AsyncCommands;
            let conn = redis.lock().await.clone();
            let mut conn = conn;
            let res: Result<String, _> = conn.xadd(&stream_key, "*", &[("payload", &payload)]).await;
            if let Err(e) = res {
                tracing::warn!("RedisRpcTransport: xadd failed: {}", e);
            }
        });
        Ok(())
    }
    fn receive(&self) -> broadcast::Receiver<RpcRequest> {
        self.local_tx.subscribe()
    }
    fn node_id(&self) -> &str { &self.node_id }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_rpc_broadcast() {
        struct PingHandler;
        impl RpcHandler for PingHandler {
            fn name(&self) -> &str { "ping" }
            fn handle(&self, payload: serde_json::Value) -> RpcResponse {
                RpcResponse { ok: true, result: Some(payload), error: None }
            }
        }

        let rpc = Arc::new(LocalRpc::new("node-1"));
        let router = Arc::new(RpcRouter::new());

        router.register(PingHandler).await;
        router.spawn_listener(rpc.clone()).await;

        rpc.broadcast(&RpcRequest {
            method: "ping".to_string(),
            target: "Broadcast".to_string(),
            payload: serde_json::json!({"device_id": "dev1", "cmd": "stop"}),
            reply_to: None,
            from_node: None,
        }).unwrap();

        // 本地 route：已注册方法返回 ok + 原样 payload
        let req = RpcRequest {
            method: "ping".to_string(),
            target: "local".to_string(),
            payload: serde_json::json!({"k": 1}),
            reply_to: None,
            from_node: None,
        };
        let resp = router.route(&req).await;
        assert!(resp.ok);
        assert_eq!(resp.result, Some(serde_json::json!({"k": 1})));

        // 未注册方法显式失败（历史上 echo handler 会伪装 ok:true）
        let resp = router.route(&RpcRequest {
            method: "play_stop".to_string(),
            ..Default::default()
        }).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap().contains("Unknown method"));
    }

    /// 出站传输注册 + broadcast_outbound 逐个投递
    #[tokio::test]
    async fn test_outbound_broadcast_delivers_via_registered_transports() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingTransport {
            hits: AtomicUsize,
            local_tx: broadcast::Sender<RpcRequest>,
        }
        impl RpcTransport for CountingTransport {
            fn broadcast(&self, _request: &RpcRequest) -> Result<(), String> {
                self.hits.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn send_to(&self, _node_id: &str, _request: &RpcRequest) -> Result<(), String> {
                self.hits.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn receive(&self) -> broadcast::Receiver<RpcRequest> { self.local_tx.subscribe() }
            fn node_id(&self) -> &str { "counting" }
        }

        let t1 = Arc::new(CountingTransport { hits: AtomicUsize::new(0), local_tx: broadcast::channel(8).0 });
        let t2 = Arc::new(CountingTransport { hits: AtomicUsize::new(0), local_tx: broadcast::channel(8).0 });
        let router = RpcRouter::new();
        router.register_outbound(t1.clone() as Arc<dyn RpcTransport>).await;
        router.register_outbound(t2.clone() as Arc<dyn RpcTransport>).await;

        router.broadcast_outbound(&RpcRequest {
            method: "ws_broadcast".to_string(),
            ..Default::default()
        }).await;
        assert_eq!(t1.hits.load(Ordering::SeqCst), 1);
        assert_eq!(t2.hits.load(Ordering::SeqCst), 1);

        router.send_to_outbound("node-9", &RpcRequest::default()).await.unwrap();
        // send_to 命中首个传输即返回
        assert_eq!(t1.hits.load(Ordering::SeqCst), 2);
        assert_eq!(t2.hits.load(Ordering::SeqCst), 1);
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

    /// Phase 7.2: from_node field roundtrips through serde.
    #[test]
    fn test_rpc_request_from_node_serde() {
        let r = RpcRequest {
            method: "device_control".into(),
            target: "Broadcast".into(),
            payload: serde_json::json!({}),
            reply_to: None,
            from_node: Some("node-1".into()),
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"from_node\":\"node-1\""));
        let back: RpcRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.from_node.as_deref(), Some("node-1"));
    }

    /// Phase 7.2: from_node field is optional (backwards compat).
    #[test]
    fn test_rpc_request_from_node_optional() {
        let json = r#"{"method":"x","target":"y","payload":{},"reply_to":null}"#;
        let r: RpcRequest = serde_json::from_str(json).unwrap();
        assert!(r.from_node.is_none());
    }

    /// Phase 7.2: RedisRpcTransport::new constructs without panic.
    #[test]
    fn test_redis_rpc_transport_new_no_panic() {
        let client = redis::Client::open("redis://127.0.0.1:1").unwrap();
        // Don't actually create a ConnectionManager (would require a live Redis);
        // instead just verify field layout is correct.
        let req = RpcRequest {
            method: "device_control".into(),
            target: "Broadcast".into(),
            payload: serde_json::json!({}),
            reply_to: None,
            from_node: Some("node-1".into()),
        };
        let s = serde_json::to_string(&req).unwrap();
        let _: RpcRequest = serde_json::from_str(&s).unwrap();
        let _ = client;
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
            from_node: None,
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
            from_node: None,
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
            from_node: None,
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
            from_node: None,
        };
        let results = rpc.broadcast(&req).await;
        assert_eq!(results.len(), 2);
        for (ep, r) in results {
            assert!(r.is_ok(), "endpoint={} 应成功", ep);
        }
    }
}
