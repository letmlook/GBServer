use axum::{extract::State, Json};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::response::ApiResult;
use crate::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchControlRequest {
    /// 目标设备国标 ID 列表
    #[serde(alias = "deviceIds")]
    pub device_ids: Vec<String>,
    /// 要下发的批量命令
    pub command: BatchCommand,
    /// 通道 ID（PTZ 类命令必填；Reboot / SyncCatalog / Query* 类命令可省略）
    #[serde(alias = "channelId")]
    pub channel_id: Option<String>,
    /// 云台转动速度（1-255），仅 PTZ 类命令使用
    pub speed: Option<u8>,
    /// 预置位编号（0-255），仅 Preset 类命令使用
    #[serde(alias = "presetIndex")]
    pub preset_index: Option<u32>,
    /// 仅 ptzStop/reboot：等待设备执行回执，最长 5 秒
    #[serde(default, rename = "waitResult", alias = "wait_result")]
    pub wait_result: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum BatchCommand {
    PtzStop,
    Reboot,
    SyncCatalog,
    QueryDeviceInfo,
    QueryDeviceStatus,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct BatchControlResult {
    /// 请求的设备总数
    pub total: usize,
    /// SIP 下发成功的设备数，不代表设备执行成功
    pub success: usize,
    /// SIP 下发失败的设备数
    pub failed: usize,
    /// 每台设备的发送状态及可选执行回执
    pub results: Vec<DeviceControlResult>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct DeviceControlResult {
    pub device_id: String,
    /// 兼容旧客户端：仅表示 SIP 下发成功
    pub success: bool,
    #[serde(rename = "sipSent")]
    pub sip_sent: bool,
    #[serde(rename = "deviceResult")]
    pub device_result: serde_json::Value,
    pub message: Option<String>,
}

impl DeviceControlResult {
    fn sent(device_id: &str) -> Self {
        Self {
            device_id: device_id.to_owned(),
            success: true,
            sip_sent: true,
            device_result: serde_json::json!({"status": "sent"}),
            message: None,
        }
    }

    fn failed(device_id: &str, message: impl Into<String>) -> Self {
        Self {
            device_id: device_id.to_owned(),
            success: false,
            sip_sent: false,
            device_result: serde_json::json!({"status": "not_sent"}),
            message: Some(message.into()),
        }
    }

    fn with_execution(device_id: &str, status: &str, sn: u32, result_code: Option<String>) -> Self {
        Self {
            device_id: device_id.to_owned(),
            success: true,
            sip_sent: true,
            device_result: serde_json::json!({"status": status, "sn": sn, "resultCode": result_code}),
            message: None,
        }
    }
}

async fn send_control(
    sip: &crate::sip::SipServer,
    device_id: &str,
    channel_id: &str,
    body: &str,
    wait_result: bool,
) -> DeviceControlResult {
    if wait_result {
        match sip
            .send_device_control_confirmable(device_id, channel_id, "DeviceControl", body)
            .await
        {
            Ok((sn, receiver)) => {
                let waited =
                    tokio::time::timeout(std::time::Duration::from_secs(5), receiver).await;
                let (status, result_code) = match waited {
                    Ok(Ok(xml)) => crate::handlers::device_stub::parse_device_control_result(&xml),
                    _ => {
                        sip.cancel_device_control_pending(device_id, sn);
                        ("unconfirmed", None)
                    }
                };
                if status == "rejected" {
                    tracing::warn!(
                        device_id,
                        channel_id,
                        sn,
                        result_code = ?result_code,
                        "Batch DeviceControl rejected by device"
                    );
                }
                DeviceControlResult::with_execution(device_id, status, sn, result_code)
            }
            Err(error) => DeviceControlResult::failed(device_id, error.to_string()),
        }
    } else {
        match sip
            .send_device_control(device_id, channel_id, "DeviceControl", body)
            .await
        {
            Ok(()) => DeviceControlResult::sent(device_id),
            Err(error) => DeviceControlResult::failed(device_id, error.to_string()),
        }
    }
}

/// POST /api/device/batch/control
///
/// 对一组设备批量下发控制命令（云台停止 / 远程重启 / 目录同步 / 信息查询 / 状态查询）。
#[utoipa::path(
    post,
    path = "/api/device/batch/control",
    tag = "device",
    operation_id = "device_batch_control",
    request_body = BatchControlRequest,
    responses(
        (status = 200, description = "批量命令发送统计（success/failed 仅表示 SIP 下发；逐设备 deviceResult 表示设备执行结果）",
         body = ApiResult<BatchControlResult>,
         example = json!({"code":0,"msg":"成功","data":{"total":2,"success":1,"failed":1,"results":[
             {"device_id":"34020000001320000001","success":true,"sipSent":true,"deviceResult":{"status":"sent"},"message":null},
             {"device_id":"34020000001320000002","success":false,"sipSent":false,"deviceResult":{"status":"not_sent"},"message":"设备不在线"}
         ]}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn batch_control(
    State(state): State<AppState>,
    Json(req): Json<BatchControlRequest>,
) -> Json<ApiResult<BatchControlResult>> {
    if req.wait_result && !matches!(req.command, BatchCommand::PtzStop | BatchCommand::Reboot) {
        return Json(ApiResult::error(
            "waitResult 仅支持 ptzStop 和 reboot；查询类命令只确认已发送",
        ));
    }
    let total = req.device_ids.len();
    let mut results = Vec::new();

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => return Json(ApiResult::error("SIP server not available")),
    };

    let sip = &*sip_server;

    for device_id in &req.device_ids {
        let result = match req.command {
            BatchCommand::PtzStop => {
                if let Some(ref channel_id) = req.channel_id {
                    // 停止云台：用国标 8 字节 PTZCmd（0xA5 起始 + 累加校验），
                    // 此前硬编码的 "A500000000AF" 既不是 8 字节、校验也不对。
                    //
                    // 报文结构也必须对：`CmdType` 是 `DeviceControl`，8 字节指令要包在
                    // `<PTZCmd>` 元素里。此前传的是 `cmd_type="PTZCmd"` + **裸十六进制串**
                    // 当 body —— 生成出来的 XML 既没有 `<PTZCmd>` 元素、CmdType 也不是
                    // 合法设备控制类型，真实设备只会丢弃。
                    let stop_cmd = crate::sip::gb28181::front_end_control::build_ptz_cmd(
                        crate::sip::gb28181::front_end_control::PtzAction::Stop,
                        0,
                    );
                    let body = format!("<PTZCmd>{}</PTZCmd>", stop_cmd);
                    send_control(sip, device_id, channel_id, &body, req.wait_result).await
                } else {
                    DeviceControlResult::failed(device_id, "Missing channel_id")
                }
            }
            BatchCommand::SyncCatalog => match sip.send_catalog_query(device_id).await {
                Ok(_) => DeviceControlResult::sent(device_id),
                Err(e) => DeviceControlResult::failed(device_id, e.to_string()),
            },
            BatchCommand::Reboot => {
                // 远程启动：`<CmdType>DeviceControl</CmdType>` + `<TeleBoot>Boot</TeleBoot>`。
                // 此前是 `cmd_type="Reboot"` + 空 body —— 设备端既认不出 CmdType，
                // 也没有 TeleBoot 元素，"批量重启"实际什么都不会发生。
                send_control(
                    sip,
                    device_id,
                    device_id,
                    "<TeleBoot>Boot</TeleBoot>",
                    req.wait_result,
                )
                .await
            }
            BatchCommand::QueryDeviceInfo => {
                // 此前与 DeviceStatus / SyncCatalog 共用 `send_catalog_query`：
                // 三个不同的批量按钮下发的是同一条目录查询。现在各发各的。
                let sn = (chrono::Utc::now().timestamp_millis() % 900_000 + 100_000) as u32;
                match sip.send_device_info_query(device_id, sn).await {
                    Ok(_) => DeviceControlResult::sent(device_id),
                    Err(e) => DeviceControlResult::failed(device_id, e.to_string()),
                }
            }
            BatchCommand::QueryDeviceStatus => {
                let sn = (chrono::Utc::now().timestamp_millis() % 900_000 + 100_000) as u32;
                match sip.send_device_status_query(device_id, sn).await {
                    Ok(_) => DeviceControlResult::sent(device_id),
                    Err(e) => DeviceControlResult::failed(device_id, e.to_string()),
                }
            }
        };
        results.push(result);
    }

    let success = results.iter().filter(|r| r.success).count();
    let failed = total - success;

    Json(ApiResult::success(BatchControlResult {
        total,
        success,
        failed,
        results,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_result_separates_sip_send_from_device_execution() {
        let sent = serde_json::to_value(DeviceControlResult::sent("device-1")).unwrap();
        assert_eq!(sent["success"], true);
        assert_eq!(sent["sipSent"], true);
        assert_eq!(sent["deviceResult"]["status"], "sent");

        let failed =
            serde_json::to_value(DeviceControlResult::failed("device-2", "offline")).unwrap();
        assert_eq!(failed["success"], false);
        assert_eq!(failed["sipSent"], false);
        assert_eq!(failed["deviceResult"]["status"], "not_sent");
    }

    #[test]
    fn batch_wait_result_accepts_camel_and_snake_case() {
        for key in ["waitResult", "wait_result"] {
            let mut value = serde_json::json!({"deviceIds": ["device-1"], "command": "reboot"});
            value[key] = serde_json::json!(true);
            let request: BatchControlRequest = serde_json::from_value(value).unwrap();
            assert!(request.wait_result);
            assert!(matches!(request.command, BatchCommand::Reboot));
        }
    }
}
