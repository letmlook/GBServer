//! JT1078 protocol stack skeleton
//!
//! This module is a placeholder for the JT808/JT1078 implementation (parsing, sessions,
//! transport, and recording). It is intentionally minimal so the repository compiles
//! and serves as a starting point for implementing the full stack.

use std::sync::Arc;

pub mod server;
pub mod frame;
pub mod session;

#[derive(Debug, Clone)]
pub struct Jt1078Server {
    // Placeholder fields
    _marker: Arc<()>,
}

impl Jt1078Server {
    pub fn new() -> Self {
        Self { _marker: Arc::new(()) }
    }

    /// Initialize resources; full implementation to be added later.
    pub async fn init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Start the server loop (placeholder) — delegates to server module which spawns listeners.
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::jt1078::server::start(self).await
    }
}

