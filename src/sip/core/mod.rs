//! SIP 核心层 - 消息解析/生成、事务管理

pub mod message;
pub mod parser;
pub mod transaction;
pub mod dialog;

pub use message::{SipMessage, SipRequest, SipResponse, SipHeader};
pub use parser::Parser;
pub use transaction::TransactionManager;
pub use dialog::DialogManager;
