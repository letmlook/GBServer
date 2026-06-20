//! Phase 4: 状态层抽象（流状态统一接口 / 未来媒体会话状态）
//! Phase 7.1: 新增 `StreamStateRepository` trait — 在 StateStore 之上提供业务抽象层，
//!   用于统一"recording / invite session / pending counter"的高频访问语义。

pub mod repository;
pub mod stream_status;

pub use repository::{StateStoreRepository, StreamStateRepository};
pub use stream_status::{StreamState, StreamStatus};
