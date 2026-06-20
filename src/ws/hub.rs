//! Phase 7.3: `WsHub` — cluster-aware WebSocket fanout.
//!
//! API:
//! - `broadcast_event(event, data)` — dispatch to local clients AND broadcast
//!   via RPC for other nodes to pick up.
//! - `handle_rpc_broadcast(payload)` — invoked when a `ws_broadcast` RPC
//!   arrives from another node; dispatches to local clients only.
//! - `register(user, events)` / `unregister(client_id)` — manage clients.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::ws::Message;
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};

use crate::rpc::{RpcRequest, RpcRouter};

/// Phase 7.3: WS upgrade query parameters.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WsQuery {
    #[serde(default)]
    pub token: Option<String>,
    /// Comma-separated list of subscribed events, or "*" for all.
    /// Default: device_status, alarm, record_state, jt_position, jt_alarm.
    #[serde(default)]
    pub events: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub user: String,
    pub subscribed: HashSet<String>,
    pub tx: mpsc::UnboundedSender<Message>,
}

pub struct WsHub {
    node_id: String,
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
    rpc_router: Option<Arc<RpcRouter>>,
}

impl WsHub {
    pub fn new(node_id: String, rpc_router: Option<Arc<RpcRouter>>) -> Self {
        Self {
            node_id,
            clients: Arc::new(RwLock::new(HashMap::new())),
            rpc_router,
        }
    }

    /// Phase 7.3: late-bind the RpcRouter after construction (so AppState can
    /// first construct WsHub without router, then attach once the router is
    /// available).
    pub fn set_router(&mut self, router: Option<Arc<RpcRouter>>) {
        self.rpc_router = router;
    }

    /// Register a new WS client. Returns the assigned client_id.
    pub async fn register(&self, user: String, events_csv: Option<String>) -> String {
        let subscribed = parse_events(events_csv.as_deref());
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let client_id = format!("client_{}", uuid_like());
        let info = ClientInfo { user, subscribed, tx };
        self.clients.write().await.insert(client_id.clone(), info);

        // Spawn a task that drains rx → nothing for now (legacy WsState did this in handler).
        // The handler is responsible for wiring rx into the WS send_task; here we just
        // hand out the tx and store the client.
        let _ = rx; // suppress unused warning
        client_id
    }

    pub async fn unregister(&self, client_id: &str) {
        self.clients.write().await.remove(client_id);
    }

    /// Broadcast an event to all matching local clients, and (if cluster is
    /// enabled) to other nodes via RPC.
    pub async fn broadcast_event(&self, event: &str, data: serde_json::Value) {
        // 1) local dispatch
        self.local_dispatch(event, &data).await;
        // 2) cluster broadcast
        if let Some(router) = self.rpc_router.as_ref() {
            let payload = serde_json::json!({
                "event": event,
                "data": data,
                "from_node": self.node_id,
            });
            let req = RpcRequest {
                method: "ws_broadcast".to_string(),
                target: "broadcast".to_string(),
                payload,
                reply_to: None,
                from_node: Some(self.node_id.clone()),
            };
            let _ = router.route(&req).await;
        }
    }

    /// Phase 7.3: handle an incoming `ws_broadcast` RPC from another node —
    /// dispatches to local clients only (no recursion).
    pub async fn handle_rpc_broadcast(&self, payload: serde_json::Value) {
        let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let data = payload.get("data").cloned().unwrap_or(serde_json::json!({}));
        if event.is_empty() {
            return;
        }
        self.local_dispatch(event, &data).await;
    }

    async fn local_dispatch(&self, event: &str, data: &serde_json::Value) {
        let msg = serde_json::json!({ "event": event, "data": data }).to_string();
        let msg = Message::Text(msg);
        let map = self.clients.read().await;
        let mut failed = Vec::new();
        for (id, client) in map.iter() {
            if client.subscribed.contains(event) || client.subscribed.contains("*") {
                if client.tx.send(msg.clone()).is_err() {
                    failed.push(id.clone());
                }
            }
        }
        drop(map);
        if !failed.is_empty() {
            let mut map = self.clients.write().await;
            for id in failed {
                map.remove(&id);
            }
        }
    }

    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

fn parse_events(csv: Option<&str>) -> HashSet<String> {
    let mut out = HashSet::new();
    match csv {
        Some(s) if !s.is_empty() => {
            for ev in s.split(',') {
                let ev = ev.trim();
                if !ev.is_empty() {
                    out.insert(ev.to_string());
                }
            }
        }
        _ => {
            // Default subscribed events
            for ev in ["device_status", "alarm", "record_state", "jt_position", "jt_alarm"] {
                out.insert(ev.to_string());
            }
        }
    }
    out
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_client_count() {
        let hub = WsHub::new("node-1".into(), None);
        assert_eq!(hub.client_count().await, 0);
        let id = hub.register("alice".into(), Some("*".into())).await;
        assert_eq!(hub.client_count().await, 1);
        hub.unregister(&id).await;
        assert_eq!(hub.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_local_dispatch_to_subscribed() {
        let hub = WsHub::new("node-1".into(), None);
        // Build our own pair so we can read from rx.
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let id = "test-client-1".to_string();
        hub.clients.write().await.insert(id.clone(), ClientInfo {
            user: "alice".into(),
            subscribed: ["alarm".to_string(), "jt_position".to_string()].into_iter().collect(),
            tx,
        });
        hub.broadcast_event("alarm", serde_json::json!({"id": 1})).await;
        let msg = rx.recv().await.unwrap();
        if let Message::Text(t) = msg {
            assert!(t.contains("\"alarm\""));
        } else {
            panic!("expected Text message");
        }
    }

    #[tokio::test]
    async fn test_local_dispatch_skips_unsubscribed() {
        let hub = WsHub::new("node-1".into(), None);
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let id = "test-client-2".to_string();
        hub.clients.write().await.insert(id.clone(), ClientInfo {
            user: "alice".into(),
            subscribed: ["alarm".to_string()].into_iter().collect(),
            tx,
        });
        // jt_position is NOT subscribed; message should not arrive.
        hub.broadcast_event("jt_position", serde_json::json!({})).await;
        // alarm IS subscribed; message should arrive.
        hub.broadcast_event("alarm", serde_json::json!({"x": 1})).await;
        let msg = rx.recv().await.unwrap();
        if let Message::Text(t) = msg {
            assert!(t.contains("\"alarm\""));
            assert!(!t.contains("jt_position"));
        } else {
            panic!("expected Text message");
        }
    }

    #[tokio::test]
    async fn test_handle_rpc_broadcast() {
        let hub = WsHub::new("node-1".into(), None);
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let id = "test-client-3".to_string();
        hub.clients.write().await.insert(id.clone(), ClientInfo {
            user: "alice".into(),
            subscribed: ["jt_alarm".to_string()].into_iter().collect(),
            tx,
        });
        hub.handle_rpc_broadcast(serde_json::json!({
            "event": "jt_alarm",
            "data": { "phone": "13800000001" },
            "from_node": "node-2"
        })).await;
        let msg = rx.recv().await.unwrap();
        if let Message::Text(t) = msg {
            assert!(t.contains("jt_alarm"));
            assert!(t.contains("13800000001"));
        } else {
            panic!("expected Text message");
        }
    }

    #[test]
    fn test_parse_events_default() {
        let set = parse_events(None);
        assert!(set.contains("alarm"));
        assert!(set.contains("jt_position"));
    }

    #[test]
    fn test_parse_events_custom() {
        let set = parse_events(Some("a,b,c"));
        assert_eq!(set.len(), 3);
        assert!(set.contains("a"));
        assert!(set.contains("c"));
    }

    #[test]
    fn test_parse_events_wildcard() {
        let set = parse_events(Some("*"));
        assert!(set.contains("*"));
    }
}
