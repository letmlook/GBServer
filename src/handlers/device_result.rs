//! 设备命令的**统一结果语义**（B14）。
//!
//! 平台必须能区分四种状态，不能混为一谈：
//!
//! 1. **平台配置已保存** —— 平台自己的库写成功；
//! 2. **SIP 消息已发送** —— 命令已经发给设备，设备还没回话；
//! 3. **设备返回执行结果** —— 设备回了 `Result=OK`（确认）或非 OK（拒绝）；
//! 4. **结果未确认** —— 超时或回执缺失。
//!
//! 「发送 → 等待回执 → 解释 `Result`」这段逻辑此前在多个 handler 里各写一份，
//! 语义容易悄悄漂移（有的等回执、有的不等；失败时的字段名也不一样）。
//! 本模块把它收敛到**一处**，所有设备命令入口共用。
//!
//! 状态取值（`deviceResult.status`）：
//!
//! | 取值 | 含义 |
//! |---|---|
//! | `not_sent` | 设备不在线等原因，命令**没有**发出去 |
//! | `sent` | 已发出，调用方没有要求等待回执 |
//! | `confirmed` | 设备回了 `Result=OK` |
//! | `rejected` | 设备回了非 OK 的 `Result` |
//! | `unconfirmed` | 等待超时或回执缺失 |

use std::time::Duration;

use serde_json::{json, Value};

/// 等待设备回执的上限。
///
/// 与传输模式、批量控制等已有入口保持一致：命令**不会**因为等不到回执而重发，
/// 超时只影响"结论"，不影响"已经发出去"这件事。
pub(crate) const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// 命令没有发出去（设备离线、目标不存在等）。
pub(crate) const STATUS_NOT_SENT: &str = "not_sent";
/// 已发出，但调用方没有要求等待回执。
pub(crate) const STATUS_SENT: &str = "sent";
/// 设备明确回了 `Result=OK`。
pub(crate) const STATUS_CONFIRMED: &str = "confirmed";
/// 设备回了非 OK 的 `Result`。
pub(crate) const STATUS_REJECTED: &str = "rejected";
/// 等待超时或回执缺失。
pub(crate) const STATUS_UNCONFIRMED: &str = "unconfirmed";

/// 一条设备命令的下发结论。
#[derive(Debug, Clone)]
pub(crate) struct CommandOutcome {
    /// SIP 消息是否已经发出。
    pub sip_sent: bool,
    /// 设备执行结果；`status` 字段取值见本模块常量。
    pub device_result: Value,
}

impl CommandOutcome {
    /// 命令没有发出去。
    pub fn not_sent() -> Self {
        Self {
            sip_sent: false,
            device_result: json!({"status": STATUS_NOT_SENT}),
        }
    }

    /// 已发出，未等待回执。
    pub fn sent() -> Self {
        Self {
            sip_sent: true,
            device_result: json!({"status": STATUS_SENT}),
        }
    }
}

/// 发送一条设备命令；`wait_result` 为真时最多等待 [`WAIT_TIMEOUT`] 拿设备回执。
///
/// 返回 `Err` 表示**发送本身失败**（命令没发出去）；调用方据此决定是整体报错，
/// 还是保留"平台已保存、但没发到设备"的结论（见 `device_transport` 的做法）。
///
/// `label` 只用于日志，让多入口的日志能区分是哪条命令。
pub(crate) async fn send_command(
    server: &crate::sip::SipServer,
    device_id: &str,
    channel_id: &str,
    cmd_type: &str,
    body: &str,
    wait_result: bool,
    label: &str,
) -> Result<CommandOutcome, anyhow::Error> {
    if !wait_result {
        return match server
            .send_device_control(device_id, channel_id, cmd_type, body)
            .await
        {
            Ok(()) => Ok(CommandOutcome::sent()),
            Err(e) => {
                tracing::error!("{label} 下发失败 device={device_id}: {e}");
                Err(e)
            }
        };
    }
    send_command_and_wait(server, device_id, channel_id, cmd_type, body, label).await
}

/// 发送一条设备命令并**等待**设备回执。
///
/// 与 [`send_command`] 的区别只在于"一定等回执"：有些入口（如传输模式）的
/// 非等待路径走的是**另一种 SIP 消息**（`Control/Transport`），不能复用
/// `send_device_control`，但等待路径与其它入口完全一致，所以单独暴露本函数。
pub(crate) async fn send_command_and_wait(
    server: &crate::sip::SipServer,
    device_id: &str,
    channel_id: &str,
    cmd_type: &str,
    body: &str,
    label: &str,
) -> Result<CommandOutcome, anyhow::Error> {
    let (sn, receiver) = match server
        .send_device_control_confirmable(device_id, channel_id, cmd_type, body)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("{label} 下发失败 device={device_id}: {e}");
            return Err(e);
        }
    };

    let waited = tokio::time::timeout(WAIT_TIMEOUT, receiver).await;
    let (status, result_code) = match waited {
        Ok(Ok(xml)) => crate::handlers::device_stub::parse_device_control_result(&xml),
        _ => {
            // 超时或通道被丢弃：撤掉 pending，避免迟到的回执去完成下一代请求。
            server.cancel_device_control_pending(device_id, sn);
            (STATUS_UNCONFIRMED, None)
        }
    };
    if status == STATUS_REJECTED {
        tracing::warn!(
            device_id,
            channel_id,
            sn,
            result_code = ?result_code,
            "{label} 被设备拒绝"
        );
    }
    Ok(CommandOutcome {
        sip_sent: true,
        device_result: json!({"status": status, "sn": sn, "resultCode": result_code}),
    })
}

/// 把命令结论并进已有的响应 JSON。
///
/// 保持既有字段不变，只**追加** `sipSent` 与 `deviceResult`，
/// 这样老前端不会因为多了字段而出问题。
pub(crate) fn merge_into(data: &mut Value, outcome: &CommandOutcome) {
    if let Some(obj) = data.as_object_mut() {
        obj.insert("sipSent".to_string(), json!(outcome.sip_sent));
        obj.insert("deviceResult".to_string(), outcome.device_result.clone());
    }
}

/// 统一的命令入口：设备必须在线且可寻址，否则返回错误。
///
/// 返回 `Err`（业务错误字符串）表示**没能发出去**；此时**不**返回"已发送"。
pub(crate) async fn send_command_to_online_device(
    state: &crate::AppState,
    device_id: &str,
    channel_id: &str,
    cmd_type: &str,
    body: &str,
    wait_result: bool,
    label: &str,
) -> Result<CommandOutcome, String> {
    if device_id.is_empty() {
        return Err("device_id is required".to_string());
    }
    let Some(ref sip_server) = state.sip_server else {
        return Err("SIP server not available".to_string());
    };
    let server = &**sip_server;
    let Some(device) = server.device_manager().get(device_id).await else {
        return Err(format!("设备不存在或未注册: {device_id}"));
    };
    if !device.online || device.addr.is_none() {
        return Err(format!("设备不在线: {device_id}"));
    }
    send_command(
        server,
        device_id,
        channel_id,
        cmd_type,
        body,
        wait_result,
        label,
    )
    .await
    .map_err(|e| format!("命令发送失败: {e}"))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    /// 结果合并只**追加**字段，不动既有字段。
    #[test]
    fn merge_into_appends_without_touching_existing_fields() {
        let mut data = json!({"deviceId": "d1", "result": "command sent"});
        let outcome = CommandOutcome::sent();
        merge_into(&mut data, &outcome);
        assert_eq!(data["deviceId"], json!("d1"));
        assert_eq!(data["result"], json!("command sent"));
        assert_eq!(data["sipSent"], json!(true));
        assert_eq!(data["deviceResult"]["status"], json!(STATUS_SENT));
    }

    /// `not_sent` 必须同时表达"没发出去"和"没有设备结论"。
    #[test]
    fn not_sent_outcome_has_both_signals() {
        let outcome = CommandOutcome::not_sent();
        assert!(!outcome.sip_sent);
        assert_eq!(outcome.device_result["status"], json!(STATUS_NOT_SENT));
    }
}
