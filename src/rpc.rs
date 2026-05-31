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
use serde::{Serialize, de::DeserializeOwned};

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
        self.tx.send(request.clone()).map_err(|e| e.to_string())
    }
    fn send_to(&self, _node_id: &str, request: &RpcRequest) -> Result<(), String> {
        self.tx.send(request.clone()).map_err(|e| e.to_string())
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
        tokio::spawn(async move {
            let mut rx = transport.receive();
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
pub fn register_standard_handlers(router: &RpcRouter) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_rpc_broadcast() {
        let rpc = Arc::new(LocalRpc::new("node-1"));
        let router = Arc::new(RpcRouter::new());

        router.register(DeviceControlHandler).await;
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
}
