// ! DeviceCommander — 设备命令→响应 完整生命周期
//!
//! 在 PendingRequestManager 基础上，为每种设备查询类型提供：
//! 1. 构造 SIP MESSAGE 查询 XML
//! 2. 注册 PendingRequest
//! 3. 通过 SipServer 发送
//! 4. 等待响应（超时）
//! 5. 解析结构化结果返回
//!
//! 与 PendingRequestManager 的区别：
//! - PendingRequestManager 是底层双向索引
//! - DeviceCommander 是业务层：知道如何 build xml / 如何解析结果 / 如何构造超时错误

use std::sync::Arc;

use crate::sip::gb28181::PendingRequestManager;
use crate::sip::gb28181::PendingCmdType;
use crate::sip::gb28181::pending_request::PendingRequest;
use crate::sip::gb28181::QueryResult;

/// 设备查询配置
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// 自定义超时秒数（默认 10s）
    pub timeout_secs: u64,
    /// 是否等待设备确认响应（有些命令 fire-and-forget）
    pub wait_response: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            timeout_secs: 10,
            wait_response: true,
        }
    }
}

/// 设备查询结果
#[derive(Debug)]
pub enum DeviceQueryResult {
    /// 查询成功，返回解析结果
    Ok(QueryResult),
    /// 超时无响应
    Timeout,
    /// 设备不在线或无注册地址
    DeviceOffline,
    /// 其他错误
    Error(String),
}

/// 设备命令发送器（业务层，封装 PendingRequest 使用）
pub struct DeviceCommander {
    pending: Arc<PendingRequestManager>,
}

impl DeviceCommander {
    pub fn new(pending: Arc<PendingRequestManager>) -> Self {
        Self { pending }
    }

    /// 查询设备基本信息（DeviceInfo）
    pub fn query_device_info(&self, device_id: &str, sn: u32) -> PendingRequest {
        let call_id = format!("di_{}_{}", device_id, sn);
        self.pending.register(
            device_id,
            sn,
            PendingCmdType::DeviceInfo,
            &call_id,
            None,
        )
    }

    /// 查询设备运行状态（DeviceStatus）
    pub fn query_device_status(&self, device_id: &str, sn: u32) -> PendingRequest {
        let call_id = format!("ds_{}_{}", device_id, sn);
        self.pending.register(
            device_id,
            sn,
            PendingCmdType::DeviceStatus,
            &call_id,
            None,
        )
    }

    /// 查询设备配置参数（ConfigDownload）
    #[allow(unused_variables)]
    pub fn query_device_config(&self, device_id: &str, sn: u32, config_type: &str) -> PendingRequest {
        let call_id = format!("dc_{}_{}", device_id, sn);
        self.pending.register(
            device_id,
            sn,
            PendingCmdType::DeviceConfig,
            &call_id,
            None,
        )
    }

    /// 查询录像信息（RecordInfo）
    pub fn query_record_info(&self, device_id: &str, sn: u32) -> PendingRequest {
        let call_id = format!("ri_{}_{}", device_id, sn);
        self.pending.register(
            device_id,
            sn,
            PendingCmdType::RecordInfo,
            &call_id,
            None,
        )
    }

    /// 解析 DeviceInfo 响应 XML
    pub fn parse_device_info(&self, xml: &str) -> DeviceInfoResult {
        let result = self.pending.parse_response(PendingCmdType::DeviceInfo, xml);
        match result {
            QueryResult::DeviceInfo(info) => DeviceInfoResult::Ok(DeviceInfoData {
                device_name: info.device_name,
                manufacturer: info.manufacturer,
                model: info.model,
                firmware: info.firmware,
                channel_count: info.channel_count,
                serial_number: info.serial_number,
            }),
            QueryResult::Raw(raw) => DeviceInfoResult::ParseError(raw),
            _ => DeviceInfoResult::ParseError(xml.to_string()),
        }
    }

    /// 解析 DeviceStatus 响应 XML
    pub fn parse_device_status(&self, xml: &str) -> DeviceStatusResult {
        let result = self.pending.parse_response(PendingCmdType::DeviceStatus, xml);
        match result {
            QueryResult::DeviceStatus(status) => DeviceStatusResult::Ok(DeviceStatusData {
                online: status.online,
                status: status.status,
                device_time: status.device_time,
                encode_channel_count: status.encode_channel_count,
                decode_channel_count: status.decode_channel_count,
                record_channel_count: status.record_channel_count,
                storage_total: status.storage_space.as_ref().and_then(|s| s.total),
                storage_remain: status.storage_space.as_ref().and_then(|s| s.remain),
            }),
            QueryResult::Raw(raw) => DeviceStatusResult::ParseError(raw),
            _ => DeviceStatusResult::ParseError(xml.to_string()),
        }
    }

    /// 查询设备后是否还有等待中的请求（用于检测超时）
    pub fn has_pending_for(&self, device_id: &str) -> bool {
        self.pending.has_pending_for_device(device_id)
    }

    /// 取消设备的所有等待请求（设备注销时调用）
    pub fn cancel_all_for_device(&self, device_id: &str) -> usize {
        self.pending.cancel_all_for_device(device_id)
    }

    /// 当前等待中的请求数量
    pub fn pending_count(&self) -> usize {
        self.pending.pending_count()
    }

    // ========================================================================
    // Phase 2.1: await 响应版本（基于 register_with_receiver）
    //
    // 调用方使用：
    //   let cmd = ...;
    //   let (req, rx) = cmd.register_device_info_with_receiver(device_id, sn);
    //   send_query_via_sip_server(...);  // 调用方自己发送 SIP MESSAGE
    //   match cmd.await_response(req, rx, 15).await {
    //       Ok(xml) => cmd.parse_device_info(&xml),
    //       Err(Timeout) => DeviceOffline,
    //   }
    // ========================================================================

    /// 注册 DeviceInfo 查询并保留 receiver
    pub fn register_device_info_with_receiver(
        &self, device_id: &str, sn: u32,
    ) -> (PendingRequest, tokio::sync::oneshot::Receiver<String>) {
        let call_id = format!("di_{}_{}", device_id, sn);
        self.pending.register_with_receiver(
            device_id, sn, PendingCmdType::DeviceInfo, &call_id, None,
        )
    }

    /// 注册 DeviceStatus 查询并保留 receiver
    pub fn register_device_status_with_receiver(
        &self, device_id: &str, sn: u32,
    ) -> (PendingRequest, tokio::sync::oneshot::Receiver<String>) {
        let call_id = format!("ds_{}_{}", device_id, sn);
        self.pending.register_with_receiver(
            device_id, sn, PendingCmdType::DeviceStatus, &call_id, None,
        )
    }

    /// 注册 DeviceConfig 查询并保留 receiver
    pub fn register_device_config_with_receiver(
        &self, device_id: &str, sn: u32,
    ) -> (PendingRequest, tokio::sync::oneshot::Receiver<String>) {
        let call_id = format!("dc_{}_{}", device_id, sn);
        self.pending.register_with_receiver(
            device_id, sn, PendingCmdType::DeviceConfig, &call_id, None,
        )
    }

    /// 等待 receiver 响应（带超时）
    pub async fn await_response(
        &self,
        _req: PendingRequest,
        rx: tokio::sync::oneshot::Receiver<String>,
        timeout_secs: u64,
    ) -> Result<String, DeviceQueryResult> {
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            rx,
        ).await {
            Ok(Ok(xml)) => Ok(xml),
            Ok(Err(_recv_cancelled)) => Err(DeviceQueryResult::Timeout),
            Err(_elapsed) => Err(DeviceQueryResult::Timeout),
        }
    }

    /// 一站式：注册 + 等待 + 解析 DeviceInfo
    /// 配合 SipServer.send_device_info_query 一起使用
    pub async fn query_device_info_and_parse(
        &self,
        device_id: &str,
        sn: u32,
        send_fn: impl std::future::Future<Output = Result<(), String>>,
        timeout_secs: u64,
    ) -> DeviceInfoResult {
        let (_req, rx) = self.register_device_info_with_receiver(device_id, sn);
        if let Err(e) = send_fn.await {
            return DeviceInfoResult::ParseError(format!("send failed: {}", e));
        }
        match self.await_response(_req, rx, timeout_secs).await {
            Ok(xml) => self.parse_device_info(&xml),
            Err(DeviceQueryResult::Timeout) => DeviceInfoResult::ParseError("timeout".to_string()),
            Err(_) => DeviceInfoResult::ParseError("error".to_string()),
        }
    }

    /// 一站式：注册 + 等待 + 解析 DeviceStatus
    pub async fn query_device_status_and_parse(
        &self,
        device_id: &str,
        sn: u32,
        send_fn: impl std::future::Future<Output = Result<(), String>>,
        timeout_secs: u64,
    ) -> DeviceStatusResult {
        let (_req, rx) = self.register_device_status_with_receiver(device_id, sn);
        if let Err(e) = send_fn.await {
            return DeviceStatusResult::ParseError(format!("send failed: {}", e));
        }
        match self.await_response(_req, rx, timeout_secs).await {
            Ok(xml) => self.parse_device_status(&xml),
            Err(DeviceQueryResult::Timeout) => DeviceStatusResult::ParseError("timeout".to_string()),
            Err(_) => DeviceStatusResult::ParseError("error".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// 解析结果结构（对外暴露的干净类型，不泄露内部实现）
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum DeviceInfoResult {
    Ok(DeviceInfoData),
    ParseError(String),
}

#[derive(Debug)]
pub enum DeviceStatusResult {
    Ok(DeviceStatusData),
    ParseError(String),
}

#[derive(Debug, Clone)]
pub struct DeviceInfoData {
    pub device_name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub channel_count: Option<i32>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceStatusData {
    pub online: Option<String>,
    pub status: Option<String>,
    pub device_time: Option<String>,
    pub encode_channel_count: Option<i32>,
    pub decode_channel_count: Option<i32>,
    pub record_channel_count: Option<i32>,
    pub storage_total: Option<i64>,
    pub storage_remain: Option<i64>,
}

// ---------------------------------------------------------------------------
// Re-export for internal use
// ---------------------------------------------------------------------------
pub use crate::sip::gb28181::device_query::{DeviceInfoResponse, DeviceStatusResponse};

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commander() -> DeviceCommander {
        DeviceCommander::new(Arc::new(PendingRequestManager::new()))
    }

    #[test]
    fn test_register_and_parse_device_info() {
        let cmd = make_commander();
        let req = cmd.query_device_info("34020000001320000001", 100);
        assert_eq!(cmd.pending_count(), 1);
        assert!(cmd.has_pending_for("34020000001320000001"));

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<SN>100</SN>
<DeviceID>34020000001320000001</DeviceID>
<DeviceName>FrontDoorCam</DeviceName>
<Manufacturer>Dahua</Manufacturer>
<Model>IPC-HDW-4431C</Model>
<FirmwareVersion>V5.1.0</FirmwareVersion>
<Channel>4</Channel>
</Response>"#;

        let result = cmd.parse_device_info(xml);
        match result {
            DeviceInfoResult::Ok(data) => {
                assert_eq!(data.device_name.as_deref(), Some("FrontDoorCam"));
                assert_eq!(data.manufacturer.as_deref(), Some("Dahua"));
                assert_eq!(data.channel_count, Some(4));
            }
            _ => panic!("Expected Ok"),
        }
    }

    #[test]
    fn test_register_and_parse_device_status() {
        let cmd = make_commander();
        cmd.query_device_status("34020000001320000001", 101);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceStatus</CmdType>
<SN>101</SN>
<DeviceID>34020000001320000001</DeviceID>
<Online>ONLINE</Online>
<Status>OK</Status>
<DeviceTime>2026-01-01T12:00:00</DeviceTime>
<EncodeChannel>4</EncodeChannel>
<RecordChannel>2</RecordChannel>
</Response>"#;

        let result = cmd.parse_device_status(xml);
        match result {
            DeviceStatusResult::Ok(data) => {
                assert_eq!(data.online.as_deref(), Some("ONLINE"));
                assert_eq!(data.status.as_deref(), Some("OK"));
                assert_eq!(data.encode_channel_count, Some(4));
                assert_eq!(data.record_channel_count, Some(2));
            }
            _ => panic!("Expected Ok"),
        }
    }

    #[test]
    fn test_cancel_all_on_unregister() {
        let cmd = make_commander();
        cmd.query_device_info("34020000001110000001", 1);
        cmd.query_device_status("34020000001110000001", 2);
        cmd.query_device_info("34020000002220000002", 1);
        assert_eq!(cmd.pending_count(), 3);

        let cancelled = cmd.cancel_all_for_device("34020000001110000001");
        assert_eq!(cancelled, 2);
        assert_eq!(cmd.pending_count(), 1);
        assert!(!cmd.has_pending_for("34020000001110000001"));
        assert!(cmd.has_pending_for("34020000002220000002"));
    }
}
