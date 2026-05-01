use axum::{extract::State, Json};
use serde::Deserialize;

use crate::response::WVPResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct BatchControlRequest {
    pub device_ids: Vec<String>,
    pub command: BatchCommand,
    pub channel_id: Option<String>,
    pub speed: Option<u8>,
    pub preset_index: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchCommand {
    PtzStop,
    Reboot,
    SyncCatalog,
    QueryDeviceInfo,
    QueryDeviceStatus,
}

#[derive(Debug, serde::Serialize)]
pub struct BatchControlResult {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub results: Vec<DeviceControlResult>,
}

#[derive(Debug, serde::Serialize)]
pub struct DeviceControlResult {
    pub device_id: String,
    pub success: bool,
    pub message: Option<String>,
}

pub async fn batch_control(
    State(state): State<AppState>,
    Json(req): Json<BatchControlRequest>,
) -> Json<WVPResult<BatchControlResult>> {
    let total = req.device_ids.len();
    let mut results = Vec::new();

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => return Json(WVPResult::error("SIP server not available")),
    };

    let sip = sip_server.read().await;

    for device_id in &req.device_ids {
        let result = match req.command {
            BatchCommand::PtzStop => {
                if let Some(ref channel_id) = req.channel_id {
                    match sip.send_device_control(device_id, channel_id, "PTZCmd", "A500000000AF").await {
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
                match sip.send_device_control(device_id, device_id, "Reboot", "").await {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                }
            }
            BatchCommand::QueryDeviceInfo => {
                match sip.send_catalog_query(device_id).await {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                }
            }
            BatchCommand::QueryDeviceStatus => {
                match sip.send_catalog_query(device_id).await {
                    Ok(_) => DeviceControlResult { device_id: device_id.clone(), success: true, message: None },
                    Err(e) => DeviceControlResult { device_id: device_id.clone(), success: false, message: Some(format!("{}", e)) },
                }
            }
        };
        results.push(result);
    }

    let success = results.iter().filter(|r| r.success).count();
    let failed = total - success;

    Json(WVPResult::success(BatchControlResult {
        total,
        success,
        failed,
        results,
    }))
}
