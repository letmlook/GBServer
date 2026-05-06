//! JT1078 protocol stack — JT808/JT1078 vehicle terminal communication
//!
//! Modules:
//! - `frame`: Binary frame parsing (JT1078 structured + legacy length-prefixed)
//! - `session`: Per-connection session state (auth, heartbeat, reassembly, seq tracking)
//! - `manager`: Session lifecycle + terminal registry + command dispatch
//! - `command`: JT808/JT1078 command encoding
//! - `server`: TCP/UDP listener lifecycle

use std::sync::Arc;
use tokio::sync::RwLock;

pub mod server;
pub mod frame;
pub mod session;
pub mod manager;
pub mod command;

use crate::jt1078::manager::Jt1078Manager;

#[derive(Clone)]
pub struct Jt1078Server {
    pub manager: Arc<RwLock<Option<Arc<Jt1078Manager>>>>,
}

impl Jt1078Server {
    pub fn new() -> Self {
        Self { manager: Arc::new(RwLock::new(None)) }
    }

    pub async fn set_manager(&self, mgr: Arc<Jt1078Manager>) {
        *self.manager.write().await = Some(mgr);
    }

    pub async fn get_manager(&self) -> Option<Arc<Jt1078Manager>> {
        self.manager.read().await.clone()
    }

    /// Initialize resources; full implementation to be added later.
    pub async fn init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Start the server loop — delegates to server module which spawns listeners.
    pub async fn start(&self, cfg: Option<crate::config::Jt1078Config>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::jt1078::server::start(self, cfg).await
    }
}
