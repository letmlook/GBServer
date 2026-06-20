//! Phase 7.3: WebSocket cluster fanout.
//!
//! `WsHub` extends the legacy per-node `WsState` with cluster-mode support:
//! every event triggered on this node is dispatched to local clients AND
//! broadcast via RPC so that connected clients on other nodes also receive it.
//!
//! JWT validation happens at upgrade time (before WebSocketUpgrade).

pub mod hub;
pub mod jwt;

pub use hub::{ClientInfo, WsHub, WsQuery};
pub use jwt::verify_ws_jwt;
