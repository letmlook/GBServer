// ! PendingRequest — 命令→响应完整生命周期管理
//!
//! 每个 SIP MESSAGE 命令（如 DeviceInfo/DeviceStatus/Config/Catalog/RecordInfo）
//! 发出去后在这里注册，等待设备响应时解析 XML，返回结构化结果。
//!
//! 与 InviteSessionManager 的区别：
//! - PendingRequest 管理命令-应答（DeviceInfo 等查询）
//! - InviteSessionManager 管理 INVITE 会话（直播/回放/下载/对讲）

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::sip::gb28181::DeviceQueryManager;

/// 等待中的 SIP 命令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingCmdType {
    /// 查询设备基本信息
    DeviceInfo,
    /// 查询设备运行状态
    DeviceStatus,
    /// 查询设备配置参数
    DeviceConfig,
    /// 查询目录（Catalog）
    Catalog,
    /// 查询录像信息（RecordInfo）
    RecordInfo,
    /// 查询移动位置
    MobilePosition,
    /// 通用 SIP MESSAGE
    GenericMessage,
}

impl PendingCmdType {
    pub fn cmd_type_str(&self) -> &'static str {
        match self {
            PendingCmdType::DeviceInfo => "DeviceInfo",
            PendingCmdType::DeviceStatus => "DeviceStatus",
            PendingCmdType::DeviceConfig => "ConfigDownload",
            PendingCmdType::Catalog => "Catalog",
            PendingCmdType::RecordInfo => "RecordInfo",
            PendingCmdType::MobilePosition => "MobilePosition",
            PendingCmdType::GenericMessage => "Message",
        }
    }
}

/// 等待中的请求元数据
#[derive(Debug, Clone)]
pub struct PendingRequest {
    /// 设备 ID
    pub device_id: String,
    /// 流水号（SN）
    pub sn: u32,
    /// 命令类型
    pub cmd_type: PendingCmdType,
    /// Call-ID（用于匹配响应）
    pub call_id: String,
    /// 创建时间
    pub created_at: Instant,
    /// 超时时长
    pub timeout_secs: u64,
    /// 响应回调（不再使用）
    #[allow(dead_code)]
    response_sender: Option<()>,
}

impl PendingRequest {
    pub fn new(
        device_id: String,
        sn: u32,
        cmd_type: PendingCmdType,
        call_id: String,
        timeout_secs: Option<u64>,
    ) -> PendingRequest {
        let timeout = timeout_secs.unwrap_or(30); // default 30s timeout
        let req = PendingRequest {
            device_id,
            sn,
            cmd_type,
            call_id,
            created_at: Instant::now(),
            timeout_secs: timeout,
            response_sender: None,
        };
        req
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(self.timeout_secs)
    }
}

/// PendingRequest 管理器
///
/// 支持三种 key 查找：
/// - `(device_id, sn)` — 通用查询匹配
/// - `call_id` — INVITE/对话匹配
pub struct PendingRequestManager {
    /// 按 call_id 索引（主要索引）
    by_call_id: Arc<DashMap<String, PendingRequest>>,
    /// 按 device_id + sn 索引
    by_device_sn: Arc<DashMap<String, PendingRequest>>,
    /// 超时阈值（秒）
    default_timeout_secs: u64,
}

impl PendingRequestManager {
    pub fn new() -> Self {
        Self {
            by_call_id: Arc::new(DashMap::new()),
            by_device_sn: Arc::new(DashMap::new()),
            default_timeout_secs: 10,
        }
    }

    pub fn with_timeout(mut self, default_timeout_secs: u64) -> Self {
        self.default_timeout_secs = default_timeout_secs;
        self
    }

    /// 注册一个新的待等请求，返回请求元数据
    pub fn register(
        &self,
        device_id: &str,
        sn: u32,
        cmd_type: PendingCmdType,
        call_id: &str,
        timeout_secs: Option<u64>,
    ) -> PendingRequest {
        let req = PendingRequest::new(
            device_id.to_string(),
            sn,
            cmd_type,
            call_id.to_string(),
            timeout_secs,
        );

        self.by_call_id.insert(call_id.to_string(), req.clone());

        let ds_key = format!("{}:{}", device_id, sn);
        self.by_device_sn.insert(ds_key, req.clone());

        req
    }

    /// 按 call_id 完成一个等待中的请求，返回原始 XML
    pub fn complete(&self, call_id: &str, xml_response: &str) -> Option<String> {
        // Try call_id first
        if let Some((_, req)) = self.by_call_id.remove(call_id) {
            let ds_key = format!("{}:{}", req.device_id, req.sn);
            self.by_device_sn.remove(&ds_key);
            return Some(xml_response.to_string());
        }

        // Try device_sn as fallback
        if let Some((_, _req)) = self.by_device_sn.remove(call_id) {
            return Some(xml_response.to_string());
        }

        None
    }

    /// 解析响应 XML 并返回结构化数据
    pub fn parse_response(&self, cmd_type: PendingCmdType, xml: &str) -> QueryResult {
        match cmd_type {
            PendingCmdType::DeviceInfo => QueryResult::DeviceInfo(
                DeviceQueryManager::parse_device_info(xml),
            ),
            PendingCmdType::DeviceStatus => QueryResult::DeviceStatus(
                DeviceQueryManager::parse_device_status(xml),
            ),
            _ => QueryResult::Raw(xml.to_string()),
        }
    }

    /// 清理已超时的请求，返回被清理的 call_id 列表
    pub fn cleanup_expired(&self) -> Vec<String> {
        let mut removed = Vec::new();

        let snap: Vec<_> = self.by_call_id
            .iter()
            .map(|r| {
                let key = r.key().clone();
                let req = PendingRequest {
                    device_id: r.device_id.clone(),
                    sn: r.sn,
                    cmd_type: r.cmd_type,
                    call_id: r.call_id.clone(),
                    created_at: r.created_at,
                    timeout_secs: r.timeout_secs,
                    response_sender: None,
                };
                (key, req)
            })
            .collect();

        for (key, req) in snap {
            if req.is_expired() {
                let ds_key = format!("{}:{}", req.device_id, req.sn);
                self.by_call_id.remove(&key);
                self.by_device_sn.remove(&ds_key);
                removed.push(key);
            }
        }

        removed
    }

    /// 获取当前等待中的请求数量（用于监控）
    pub fn pending_count(&self) -> usize {
        self.by_call_id.len()
    }

    /// 检查某个设备是否有等待中的请求
    pub fn has_pending_for_device(&self, device_id: &str) -> bool {
        self.by_device_sn
            .iter()
            .any(|r| r.device_id == device_id)
    }

    /// 获取设备所有等待中的请求
    pub fn get_pending_for_device(&self, device_id: &str) -> Vec<PendingRequest> {
        self.by_device_sn
            .iter()
            .filter(|r| r.device_id == device_id)
            .map(|r| PendingRequest {
                device_id: r.device_id.clone(),
                sn: r.sn,
                cmd_type: r.cmd_type,
                call_id: r.call_id.clone(),
                created_at: r.created_at,
                timeout_secs: r.timeout_secs,
                response_sender: None,
            })
            .collect()
    }

    /// 取消设备某个等待请求
    pub fn cancel_for_device(&self, device_id: &str, sn: u32) -> bool {
        let key = format!("{}:{}", device_id, sn);
        if let Some((_, req)) = self.by_device_sn.remove(&key) {
            self.by_call_id.remove(&req.call_id);
            return true;
        }
        false
    }

    /// 取消设备所有等待请求
    #[allow(dead_code)]
    pub fn cancel_all_for_device(&self, device_id: &str) -> usize {
        let snap: Vec<_> = self.by_device_sn
            .iter()
            .filter(|r| r.device_id == device_id)
            .map(|r| (r.key().clone(), r.call_id.clone()))
            .collect();

        let mut count = 0;
        for (ds_key, call_id) in snap {
            self.by_call_id.remove(&call_id);
            self.by_device_sn.remove(&ds_key);
            count += 1;
        }
        count
    }
}

impl Default for PendingRequestManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 查询命令解析结果
#[derive(Debug, Clone)]
pub enum QueryResult {
    DeviceInfo(DeviceInfoResponse),
    DeviceStatus(DeviceStatusResponse),
    /// 其他未解析的命令类型，返回原始 XML
    Raw(String),
}

pub use crate::sip::gb28181::device_query::{DeviceInfoResponse, DeviceStatusResponse};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_complete() {
        let mgr = PendingRequestManager::new();
        let req = mgr.register("34020000001110000001", 1, PendingCmdType::DeviceInfo, "call-abc", None);
        assert_eq!(mgr.pending_count(), 1);
        assert!(mgr.has_pending_for_device("34020000001110000001"));

        let xml = r#"<?xml version="1.0"?><Response><DeviceName>TestCam</DeviceName></Response>"#;
        let result = mgr.complete("call-abc", xml);
        assert!(result.is_some());
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn test_cancel_all_for_device() {
        let mgr = PendingRequestManager::new();
        mgr.register("34020000001110000001", 1, PendingCmdType::DeviceInfo, "call-1", None);
        mgr.register("34020000001110000001", 2, PendingCmdType::DeviceStatus, "call-2", None);
        assert_eq!(mgr.pending_count(), 2);

        let count = mgr.cancel_all_for_device("34020000001110000001");
        assert_eq!(count, 2);
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn test_parse_device_info_response() {
        let mgr = PendingRequestManager::new();
        let xml = r#"<?xml version="1.0"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<DeviceName>IPC-01</DeviceName>
<Manufacturer>Hikvision</Manufacturer>
<Model>DS-2CD2043</Model>
<FirmwareVersion>V5.5.81</FirmwareVersion>
<Channel>4</Channel>
</Response>"#;
        let result = mgr.parse_response(PendingCmdType::DeviceInfo, xml);
        match result {
            QueryResult::DeviceInfo(info) => {
                assert_eq!(info.device_name.as_deref(), Some("IPC-01"));
                assert_eq!(info.manufacturer.as_deref(), Some("Hikvision"));
                assert_eq!(info.channel_count, Some(4));
            }
            _ => panic!("Expected DeviceInfo result"),
        }
    }

    #[test]
    fn test_cleanup_expired() {
        let mut mgr = PendingRequestManager::new();
        mgr.default_timeout_secs = 1; // 1 second timeout
        mgr.register("34020000001110000001", 1, PendingCmdType::DeviceInfo, "call-x", None);
        assert_eq!(mgr.pending_count(), 1);

        std::thread::sleep(Duration::from_secs(2));
        let removed = mgr.cleanup_expired();
        assert_eq!(removed.len(), 1);
        assert_eq!(mgr.pending_count(), 0);
    }
}

// ============================================================================
// SIP 响应路由分发器 — 集成到 SipServer
// ============================================================================
//
// 使用方式：在 SipServer 中创建 ResponseRouter，将 PendingRequestManager 注入。
// handle_response() 收到 SIP 响应时调用 router.route_response()。
// handle_message() 收到设备 MESSAGE 响应时调用 router.route_message_response()。
// ============================================================================

/// SIP 响应路由分发器
///
/// 统一处理来自设备的 SIP MESSAGE 响应和 SIP Response。
/// 自动识别命令类型，完成对应的 PendingRequest，返回结构化结果。
pub struct ResponseRouter {
    pending: Arc<PendingRequestManager>,
}

impl ResponseRouter {
    pub fn new(pending: Arc<PendingRequestManager>) -> Self {
        Self { pending }
    }

    /// 路由 SIP MESSAGE 响应（MESSAGE 是请求也是响应，body 中带 Response）
    ///
    /// 从 XML 提取 CmdType，返回完成后的 XML（供 parse_response 使用）。
    pub fn route_message_response(&self, body: &str, call_id: &str) -> Option<(PendingCmdType, String)> {
        use crate::sip::gb28181::XmlParser;
        let cmd_type = XmlParser::get_cmd_type(body);
        let cmd_type_str = cmd_type.as_deref().unwrap_or("");

        let pending_type = match cmd_type_str {
            "DeviceInfo" => Some(PendingCmdType::DeviceInfo),
            "DeviceStatus" => Some(PendingCmdType::DeviceStatus),
            "ConfigDownload" => Some(PendingCmdType::DeviceConfig),
            "Catalog" => Some(PendingCmdType::Catalog),
            "RecordInfo" => Some(PendingCmdType::RecordInfo),
            "MobilePosition" => Some(PendingCmdType::MobilePosition),
            _ => None,
        };

        if let Some(pt) = pending_type {
            if let Some(xml) = self.pending.complete(call_id, body) {
                return Some((pt, xml));
            }
        }

        // 未注册的响应，只记录日志
        tracing::debug!("Unsolicited MESSAGE response for CallID {}: {}", call_id, cmd_type_str);
        None
    }

    /// 路由通用 SIP 响应（Response 方法）
    pub fn route_response(&self, status_code: u16, call_id: &str) -> Option<PendingCmdType> {
        if status_code >= 200 {
            // 尝试通过 call_id 找到 PendingRequest 获取类型
            // 目前 call_id 可能不包含设备/SN 信息，这里返回 None
            // 真实实现需在 register 时同时记录状态到 Redis/Map
            tracing::debug!("SIP Response {} for CallID {}", status_code, call_id);
            None
        } else {
            None
        }
    }

    /// 批量解析 RecordInfo 多包响应
    ///
    /// RecordInfo 响应可能分多包发送（SumNum > 1）。
    /// 将 body 内容追加到已积累的缓冲中，返回是否收齐。
    pub fn accumulate_record_info(
        &self,
        _call_id: &str,
        body: &str,
        buffer: &mut String,
        total_num: i32,
    ) -> bool {
        use crate::sip::gb28181::XmlParser;
        if !buffer.is_empty() {
            // 已有过往内容，追加 RecordItem 节点
            let items = XmlParser::extract_record_items(body);
            for item in items {
                *buffer += &item;
            }
        } else {
            *buffer = body.to_string();
        }

        let current_count = XmlParser::count_record_items(buffer);
        current_count >= total_num
    }
}

/// 从 XML 中提取 RecordItem 节点（辅助方法）
impl crate::sip::gb28181::XmlParser {
    /// 提取 RecordInfo XML 中的 RecordItem 节点列表
    #[allow(dead_code)]
    pub fn extract_record_items(xml: &str) -> Vec<String> {
        let mut items = Vec::new();
        let mut start = 0;
        while let Some(begin) = xml[start..].find("<RecordItem ") {
            let abs_begin = start + begin;
            if let Some(end) = xml[abs_begin..].find("</RecordItem>") {
                let item_end = abs_begin + end + strlen("</RecordItem>");
                items.push(xml[abs_begin..item_end].to_string());
                start = item_end;
            } else {
                break;
            }
        }
        items
    }

    /// 统计 RecordInfo XML 中 RecordItem 的数量
    #[allow(dead_code)]
    pub fn count_record_items(xml: &str) -> i32 {
        let count = xml.matches("<RecordItem ").count() as i32;
        if count == 0 && xml.contains("<RecordList ") {
            return xml.matches("<Item ").count() as i32;
        }
        count
    }
}

fn strlen(s: &str) -> usize {
    s.len()
}

