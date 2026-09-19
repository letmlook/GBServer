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
}

#[derive(Debug, Deserialize, ToSchema)]
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
    /// 下发成功的设备数
    pub success: usize,
    /// 下发失败的设备数
    pub failed: usize,
    /// 每台设备的执行结果
    pub results: Vec<DeviceControlResult>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct DeviceControlResult {
    pub device_id: String,
    pub success: bool,
    pub message: Option<String>,
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
        (status = 200, description = "批量命令下发结果（逐设备汇总 success/failed）",
         body = ApiResult<BatchControlResult>,
         example = json!({"code":0,"msg":"成功","data":{"total":2,"success":1,"failed":1,"results":[
             {"deviceId":"34020000001320000001","success":true,"message":null},
             {"deviceId":"34020000001320000002","success":false,"message":"设备不在线"}
         ]}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn batch_control(
    State(state): State<AppState>,
    Json(req): Json<BatchControlRequest>,
) -> Json<ApiResult<BatchControlResult>> {
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
                    match sip
                        .send_device_control(device_id, channel_id, "DeviceControl", &body)
                        .await
                    {
                        Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                        Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                    }
                } else {
                    DeviceControlResult { device_id: device_id.clone(), success: false, message: Some("Missing channel_id".to_string()) }
                }
            }
            BatchCommand::SyncCatalog => {
                match sip.send_catalog_query(device_id).await {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                }
            }
            BatchCommand::Reboot => {
                // 远程启动：`<CmdType>DeviceControl</CmdType>` + `<TeleBoot>Boot</TeleBoot>`。
                // 此前是 `cmd_type="Reboot"` + 空 body —— 设备端既认不出 CmdType，
                // 也没有 TeleBoot 元素，"批量重启"实际什么都不会发生。
                match sip
                    .send_device_control(device_id, device_id, "DeviceControl", "<TeleBoot>Boot</TeleBoot>")
                    .await
                {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                }
            }
            BatchCommand::QueryDeviceInfo => {
                // 此前与 DeviceStatus / SyncCatalog 共用 `send_catalog_query`：
                // 三个不同的批量按钮下发的是同一条目录查询。现在各发各的。
                let sn = (chrono::Utc::now().timestamp_millis() % 900_000 + 100_000) as u32;
                match sip.send_device_info_query(device_id, sn).await {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                }
            }
            BatchCommand::QueryDeviceStatus => {
                let sn = (chrono::Utc::now().timestamp_millis() % 900_000 + 100_000) as u32;
                match sip.send_device_status_query(device_id, sn).await {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
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