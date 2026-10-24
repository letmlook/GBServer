pub mod alarm;
pub mod authz;
pub mod cloud_record_extra;
pub mod common_channel;
pub mod device;
pub mod device_batch;
pub mod device_control;
pub mod device_query;
pub mod device_result;
pub mod device_stub;
pub mod diagnostics;
pub mod front_end;
pub mod health;
pub mod jt1078;
pub mod jt1078_extra;
pub mod metrics;
pub mod parity_extras;
pub mod platform;
pub mod play;
pub mod playback;
pub mod position;
pub mod region;
pub mod role;
pub mod rtp_control;
pub mod server;
pub mod stream;
pub mod stub;
pub mod sy_camera;
pub mod sy_contract;
pub mod system;
pub mod talk;
pub mod user;
pub mod webrtc;
pub mod websocket;

use crate::error::{AppError, ErrorCode};

/// 写操作命中 **0 行** → 回 404「目标不存在」。
///
/// ## 为什么要有这个统一出口
///
/// 按主键定位的**单行 UPDATE**，目标不存在时若照旧回 200 + `code:0`，
/// 就是**假成功** —— 前端弹「保存成功」，实际一行都没改，管理员以为改完了。
/// 仓库既有先例：
/// - `src/handlers/stub.rs::require_api_key_affected`（API Key 的
///   remark / enable / disable / reset）
/// - `src/handlers/platform.rs::catalog_edit`（裸 SQL 取 `rows_affected`）
/// - `src/handlers/device_stub.rs::device_update`、`::channel_audio`
/// - `src/handlers/jt1078.rs` 的一批终端 / 通道写端点
///
/// 2026-09-25 第九轮审计把同一模式又挖出 7 处（通道 update / reset、
/// 区域 update、分组 update、平台 update、平台通道 update、推流 update），
/// 于是抽成本函数 —— 与其 7 处各写一遍，不如**一个出口**、一处注释。
///
/// ## 三种方言的 `rows_affected` 语义一致
///
/// 语义都是**命中行数**，与「字段值有没有变」无关：
/// - PostgreSQL 统计匹配的；
/// - SQLite 的 `sqlite3_changes()` 统计被 UPDATE 处理过的行；
/// - MySQL 侧 sqlx 握手时开了 `CLIENT_FOUND_ROWS`，回的是**匹配**行数而不是
///   **改变**行数（否则「启用一个已经启用的 Key」会被误判成 404）。
///
/// 所以 `affected == 0` ⇔「目标不存在」在三种方言上等价。
///
/// **前提**：调用方的 db 函数**不能**在「没传任何字段」时提前 `return Ok(0)`
/// —— 那会让「目标存在但没东西可改」被误判成 404。这种 db 函数
/// （如 `db::platform_channel::update`）应当先按 id 查存在性，见调用点注释。
pub(crate) fn require_affected(
    affected: u64,
    what: &str,
    id: impl std::fmt::Display,
) -> Result<(), AppError> {
    if affected == 0 {
        return Err(AppError::business(
            ErrorCode::Error404,
            format!("{what}不存在: id={id}"),
        ));
    }
    Ok(())
}
