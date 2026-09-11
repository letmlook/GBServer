//! JT1078 protocol stack — JT808/JT1078 vehicle terminal communication
//!
//! Modules:
//! - `frame`: Binary frame parsing (JT1078 structured + legacy length-prefixed)
//! - `session`: Per-connection session state (auth, heartbeat, reassembly, seq tracking)
//! - `manager`: Session lifecycle + terminal registry + command dispatch
//! - `command`: JT808/JT1078 command encoding
//! - `server`: TCP/UDP listener lifecycle
//!
//! ## ⚠️ 与部标 JT/T 808 / JT/T 1078 规范的已知偏差（简化实现）
//!
//! 当前实现是"能跑通自有模拟器与联动链路"的简化版，接入真实部标终端前需对齐：
//!
//! 1. **媒体帧格式**（`frame.rs`）：使用 `0x7E 0x01` + u16 长度 / 4 字节 u32 长度
//!    前缀的自定义分帧 + XOR 校验，**不是** JT/T 1078 规定的 RTP 头结构
//!    （SIM 卡号 BCD + 通道号 + 数据类型 + 时间戳 + Seq 的 30 字节头）。
//! 2. **终端接入认证**（`session.rs`）：明文 `AUTH:<token>` 与环境变量
//!    `GBSERVER__JT1078__TOKEN` 比较，**不是** JT/T 808 的注册应答分配鉴权码
//!    流程（0x0100 注册 → 0x8100 应答携带 AuthCode → 后续消息携带鉴权码校验）。
//! 3. **命令下发传输**（`manager.rs`）：`send_command` 每次新建临时 UDP socket
//!    发送，不复用终端已建立的 TCP 连接；NAT 场景下可能不可达。
//! 4. 协议原语（2026-09-11 已补齐）：0x8202 临时位置跟踪、0x8203 报警确认、
//!    0x9205 录像回放上传（文件上传指令）等，均已在 `command.rs` 实现并接线；
//!    不再存在"未实现"的 JT1078 端点。
//!    （对应 HTTP 端点显式报错，见 `handlers/jt1078_extra.rs`）。

use std::sync::Arc;
use tokio::sync::RwLock;

pub mod server;
pub mod frame;
pub mod session;
pub mod manager;
pub mod command;
pub mod command_waiter;
pub mod response_parser;

use crate::jt1078::manager::Jt1078Manager;

#[derive(Clone)]
pub struct Jt1078Server {
    pub manager: Arc<RwLock<Option<Arc<Jt1078Manager>>>>,
    /// 数据库连接池（终端注册需要落库到 `gb_jt_terminal`，
    /// 否则 `/api/jt1078/terminal/list` 永远是空列表）。
    pool: Option<crate::db::Pool>,
}

impl Jt1078Server {
    pub fn new() -> Self {
        Self { manager: Arc::new(RwLock::new(None)), pool: None }
    }

    pub fn with_pool(pool: crate::db::Pool) -> Self {
        Self { manager: Arc::new(RwLock::new(None)), pool: Some(pool) }
    }

    pub fn pool(&self) -> Option<crate::db::Pool> {
        self.pool.clone()
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

// Phase 6.3: JT1078 media session management
pub mod jt_media_session;
