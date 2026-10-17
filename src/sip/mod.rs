//! SIP 模块 - GB28181 SIP 信令实现
//!
//! 参考 gbt_sip_server 架构设计，基于 tokio 异步运行时实现

pub mod core;
pub mod gb28181;
pub mod server;
pub mod transport;

pub use core::method::SipMethod;
pub use server::PlaybackControlCmd;
pub use server::SipServer;

/// SIP 身份随 Cargo 包版本更新，避免请求头与 HTTP/OpenAPI 的版本漂移。
pub const USER_AGENT: &str = concat!("GBServer/", env!("CARGO_PKG_VERSION"));
