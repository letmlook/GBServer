use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::ws::{verify_ws_jwt, WsQuery};
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

    /// Phase 7.6: count currently-connected WS clients (approximate).
    pub async fn broadcast_count(&self) -> usize {
        self.tx_map.read().await.len()
    }
}

/// Phase 7.3: WebSocket upgrade handler with JWT validation.
///
/// Token can be provided via `?token=` query parameter OR
/// `Authorization: Bearer <token>` header (Axum requires the latter
/// because we don't run the regular auth_middleware on `/api/ws`).
///
/// Phase 7.6: Uses `RawQuery` (string slice) instead of `Query<WsQuery>`
/// so plain HTTP probes without any query string return 401 (not 400
/// from Axum's Query extractor).
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    raw_query: Option<axum::extract::RawQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 1) Parse token from raw query string (token=<jwt>).
    let qstring: String = raw_query.and_then(|q| q.0).unwrap_or_default();
    let token = qstring
        .split('&')
        .find_map(|kv| kv.strip_prefix("token="))
        .map(|s| url_decode(s))
        .or_else(|| {
            headers.get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(String::from)
        });
    let Some(token) = token else {
        return (axum::http::StatusCode::UNAUTHORIZED, "Missing JWT (query ?token= or Authorization: Bearer)").into_response();
    };
    // 2) Verify
    let claims = match verify_ws_jwt(&token, &state.config.jwt.secret) {
        Ok(c) => c,
        Err(e) => return (axum::http::StatusCode::UNAUTHORIZED, e).into_response(),
    };
    let events = qstring
        .split('&')
        .find_map(|kv| kv.strip_prefix("events="))
        .map(|s| url_decode(s));
    let params = WsQuery {
        token: Some(token.clone()),
        events,
    };

    let ws_state = state.ws_state.clone();
    let ws_hub = state.ws_hub.clone();
    let user = claims.sub.clone();
    let events_csv = params.events.clone();
    ws.on_upgrade(move |socket| async move {
        // Register in WsHub (cluster-aware)
        let client_id = ws_hub.register(user.clone(), events_csv).await;
        // Also register in legacy WsState (back-compat — used by older handlers)
        let (legacy_tx, mut rx) = mpsc::unbounded_channel::<Message>();
        {
            let mut map = ws_state.tx_map.write().await;
            map.insert(client_id.clone(), legacy_tx);
        }

        tracing::info!("WebSocket connected: {} (user={})", client_id, user);

        let (mut ws_tx, mut ws_rx) = socket.split();
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_tx.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let ws_state_clone = ws_state.clone();
        let ws_hub_clone = ws_hub.clone();
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
        ws_hub_clone.unregister(&client_id).await;
        tracing::info!("WebSocket disconnected: {}", client_id);
    })
}

/// Phase 7.6: detect a real WebSocket upgrade request via headers.
fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    let has_upgrade = headers.get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    let has_connection = headers.get("connection")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);
    let has_key = headers.get("sec-websocket-key").is_some();
    has_upgrade && has_connection && has_key
}

/// Phase 7.6: minimal percent-decode for query parameters.
fn url_decode(s: &str) -> String {
    // Minimal decode: %XX → byte, '+' → ' '. JWT tokens are URL-safe base64 so
    // '+' rarely appears; this is sufficient for token= and events=.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i+1]), hex(bytes[i+2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
