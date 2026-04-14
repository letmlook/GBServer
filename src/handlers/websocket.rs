use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::AppState;

type TxMap = Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>;

pub struct WsState {
    pub tx_map: TxMap,
}

impl WsState {
    pub fn new() -> Self {
        Self {
            tx_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn broadcast(&self, event: &str, data: serde_json::Value) {
        let msg = json!({ "event": event, "data": data });
        let msg = Message::Text(msg.to_string());
        let map = self.tx_map.read().await;
        let mut failed = Vec::new();
        for (id, tx) in map.iter() {
            if tx.send(msg.clone()).is_err() {
                failed.push(id.clone());
            }
        }
        drop(map);
        if !failed.is_empty() {
            let mut map = self.tx_map.write().await;
            for id in failed {
                map.remove(&id);
            }
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let ws_state = state.ws_state.clone();
    ws.on_upgrade(move |socket| async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let client_id = format!("client_{}", rand::random::<u64>());

        {
            let mut map = ws_state.tx_map.write().await;
            map.insert(client_id.clone(), tx);
        }

        tracing::info!("WebSocket connected: {}", client_id);

        let (mut ws_tx, mut ws_rx) = socket.split();
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_tx.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let ws_state_clone = ws_state.clone();
        let client_id_clone = client_id.clone();
        let recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_rx.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if val.get("type").and_then(|v| v.as_str()) == Some("ping") {
                            let _ = ws_state_clone.tx_map.read().await.get(&client_id_clone)
                                .map(|tx| tx.send(Message::Text(r#"{"event":"pong"}"#.to_string())));
                        }
                    }
                } else if let Message::Close(_) = msg {
                    break;
                }
            }
        });

        tokio::select! {
            _ = send_task => {}
            _ = recv_task => {}
        }

        {
            let mut map = ws_state.tx_map.write().await;
            map.remove(&client_id);
        }
        tracing::info!("WebSocket disconnected: {}", client_id);
    })
}
