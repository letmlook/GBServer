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

    /// 按 call_id 读取命令类型（不删除），用于 SIP 响应路由判断
    pub fn peek_cmd_type(&self, call_id: &str) -> Option<PendingCmdType> {
        self.by_call_id
            .get(call_id)
            .map(|entry| entry.value().cmd_type)
    }

    /// 按 call_id 完成并返回被完成请求的 cmd_type，便于调用方分支处理
    pub fn complete_with_meta(
        &self,
        call_id: &str,
        xml_response: &str,
    ) -> Option<(PendingCmdType, String)> {
        if let Some((_, req)) = self.by_call_id.remove(call_id) {
            let ds_key = format!("{}:{}", req.device_id, req.sn);
            self.by_device_sn.remove(&ds_key);
            return Some((req.cmd_type, xml_response.to_string()));
        }
        if let Some((_, req)) = self.by_device_sn.remove(call_id) {
            return Some((req.cmd_type, xml_response.to_string()));
        }
        None
    }

    /// 解析响应 XML 并返回结构化数据
    pub fn parse_response(&self, cmd_type: PendingCmdType, xml: &str) -> QueryResult {
        match cmd_type {
            PendingCmdType::DeviceInfo => {
                QueryResult::DeviceInfo(DeviceQueryManager::parse_device_info(xml))
            }
            PendingCmdType::DeviceStatus => {
                QueryResult::DeviceStatus(DeviceQueryManager::parse_device_status(xml))
            }
            _ => QueryResult::Raw(xml.to_string()),
        }
    }

    /// 清理已超时的请求，返回被清理的 call_id 列表
    pub fn cleanup_expired(&self) -> Vec<String> {
        let mut removed = Vec::new();

        let snap: Vec<_> = self
            .by_call_id
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
        self.by_device_sn.iter().any(|r| r.device_id == device_id)
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
        let snap: Vec<_> = self
            .by_device_sn
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
        let req = mgr.register(
            "34020000001110000001",
            1,
            PendingCmdType::DeviceInfo,
            "call-abc",
            None,
        );
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
        mgr.register(
            "34020000001110000001",
            1,
            PendingCmdType::DeviceInfo,
            "call-1",
            None,
        );
        mgr.register(
            "34020000001110000001",
            2,
            PendingCmdType::DeviceStatus,
            "call-2",
            None,
        );
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
        let mgr = PendingRequestManager::new().with_timeout(1); // 1 second timeout
        mgr.register(
            "34020000001110000001",
            1,
            PendingCmdType::DeviceInfo,
            "call-x",
            None,
        );
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
    pub fn route_message_response(
        &self,
        body: &str,
        call_id: &str,
    ) -> Option<(PendingCmdType, String)> {
        // 不依赖有 bug 的 XmlParser::parse（无法处理 Response 嵌套），
        // 直接用字符串匹配取 <CmdType>X</CmdType>，更稳。
        let cmd_type_str = extract_cmd_type(body);

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
        tracing::debug!(
            "Unsolicited MESSAGE response for CallID {}: {}",
            call_id,
            cmd_type_str
        );
        None
    }

    /// 路由通用 SIP 响应（Response 方法）
    pub fn route_response(&self, status_code: u16, call_id: &str) -> Option<PendingCmdType> {
        if call_id.is_empty() {
            return None;
        }

        if let Some(cmd_type) = self.pending.peek_cmd_type(call_id) {
            // 用状态码构造 XML 占位响应，保证等待方能区分 200 与 4xx/5xx
            let reason = sip_reason_phrase(status_code);
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Response><StatusCode>{}</StatusCode><Reason>{}</Reason></Response>"#,
                status_code, reason
            );
            let _ = self.pending.complete(call_id, &xml);
            tracing::debug!(
                "SIP Response {} for CallID {} resolved as {:?}",
                status_code,
                call_id,
                cmd_type
            );
            return Some(cmd_type);
        }

        tracing::debug!(
            "SIP Response {} for CallID {} had no matching pending request",
            status_code,
            call_id
        );
        None
    }

    /// 路由后取 cmd_type，附带把响应 XML 也拿出来（当调用方有更具体的 XML 想要用时）
    pub fn route_response_with_xml(
        &self,
        status_code: u16,
        call_id: &str,
    ) -> Option<(PendingCmdType, String)> {
        if call_id.is_empty() {
            return None;
        }
        let reason = sip_reason_phrase(status_code);
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Response><StatusCode>{}</StatusCode><Reason>{}</Reason></Response>"#,
            status_code, reason
        );
        self.pending
            .complete_with_meta(call_id, &xml)
            .map(|(cmd_type, body)| (cmd_type, body))
    }
}

/// 从 XML body 中提取 <CmdType>X</CmdType> 的值，兼容任意层级嵌套。
fn extract_cmd_type(xml: &str) -> &str {
    let open = match xml.find("<CmdType>") {
        Some(idx) => idx,
        None => return "",
    };
    let start = open + "<CmdType>".len();
    let end_close = match xml[start..].find("</CmdType>") {
        Some(idx) => start + idx,
        None => return "",
    };
    xml[start..end_close].trim()
}

fn sip_reason_phrase(code: u16) -> &'static str {
    match code {
        100 => "Trying",
        180 => "Ringing",
        183 => "Session Progress",
        200 => "OK",
        202 => "Accepted",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Moved Temporarily",
        305 => "Use Proxy",
        380 => "Alternative Service",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        410 => "Gone",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Unsupported URI Scheme",
        420 => "Bad Extension",
        421 => "Extension Required",
        423 => "Interval Too Brief",
        480 => "Temporarily Unavailable",
        481 => "Call/Transaction Does Not Exist",
        482 => "Loop Detected",
        483 => "Too Many Hops",
        484 => "Address Incomplete",
        485 => "Ambiguous",
        486 => "Busy Here",
        487 => "Request Terminated",
        488 => "Not Acceptable Here",
        491 => "Request Pending",
        493 => "Undecipherable",
        500 => "Server Internal Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Server Time-out",
        505 => "Version Not Supported",
        513 => "Message Too Large",
        600 => "Busy Everywhere",
        603 => "Decline",
        604 => "Does Not Exist Anywhere",
        606 => "Not Acceptable",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod response_router_tests {
    use super::*;

    fn fixture() -> (Arc<PendingRequestManager>, ResponseRouter) {
        let mgr = Arc::new(PendingRequestManager::new());
        let router = ResponseRouter::new(mgr.clone());
        (mgr, router)
    }

    #[test]
    fn route_response_hits_pending_by_call_id() {
        let (mgr, router) = fixture();
        mgr.register(
            "34020000001110000001",
            1,
            PendingCmdType::DeviceInfo,
            "call-info-1",
            None,
        );
        let cmd = router.route_response(200, "call-info-1");
        assert_eq!(cmd, Some(PendingCmdType::DeviceInfo));
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn route_response_with_xml_returns_cmd_and_body() {
        let (mgr, router) = fixture();
        mgr.register(
            "34020000001110000001",
            7,
            PendingCmdType::DeviceStatus,
            "call-status-7",
            None,
        );
        let (cmd, body) = router
            .route_response_with_xml(200, "call-status-7")
            .expect("expected resolved response");
        assert_eq!(cmd, PendingCmdType::DeviceStatus);
        assert!(body.contains("<StatusCode>200</StatusCode>"));
        assert!(body.contains("<Reason>OK</Reason>"));
    }

    #[test]
    fn route_response_returns_none_for_unknown_call_id() {
        let (_mgr, router) = fixture();
        assert_eq!(router.route_response(200, "ghost-call"), None);
    }

    #[test]
    fn route_response_returns_none_for_empty_call_id() {
        let (_mgr, router) = fixture();
        assert_eq!(router.route_response(200, ""), None);
    }

    #[test]
    fn route_response_4xx_uses_correct_reason() {
        let (mgr, router) = fixture();
        mgr.register(
            "34020000001110000001",
            9,
            PendingCmdType::DeviceConfig,
            "call-cfg-9",
            None,
        );
        let (_cmd, body) = router
            .route_response_with_xml(404, "call-cfg-9")
            .expect("expected resolved response");
        assert!(body.contains("<StatusCode>404</StatusCode>"));
        assert!(body.contains("<Reason>Not Found</Reason>"));
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn route_message_response_resolves_all_known_cmd_types() {
        let cases = [
            (
                "DeviceInfo",
                PendingCmdType::DeviceInfo,
                "<DeviceName>IPC-1</DeviceName>",
            ),
            (
                "DeviceStatus",
                PendingCmdType::DeviceStatus,
                "<Online>ONLINE</Online>",
            ),
            (
                "Catalog",
                PendingCmdType::Catalog,
                "<DeviceList><Item/></DeviceList>",
            ),
            (
                "RecordInfo",
                PendingCmdType::RecordInfo,
                "<RecordList><Item/></RecordList>",
            ),
            (
                "MobilePosition",
                PendingCmdType::MobilePosition,
                "<Longitude>120</Longitude>",
            ),
            (
                "ConfigDownload",
                PendingCmdType::DeviceConfig,
                "<BasicParam><Name>cam</Name></BasicParam>",
            ),
        ];
        for (i, (cmd_str, expected_type, payload)) in cases.iter().enumerate() {
            let (mgr, router) = fixture();
            let call_id = format!("call-{}", i);
            mgr.register(
                "34020000001110000001",
                i as u32 + 1,
                *expected_type,
                &call_id,
                None,
            );
            let body = format!(
                r#"<?xml version="1.0"?><Response><CmdType>{}</CmdType>{}</Response>"#,
                cmd_str, payload
            );
            let resolved = router.route_message_response(&body, &call_id);
            assert_eq!(
                resolved,
                Some((*expected_type, body.clone())),
                "case {}",
                cmd_str
            );
            assert_eq!(
                mgr.pending_count(),
                0,
                "case {} should clear pending",
                cmd_str
            );
        }
    }

    #[test]
    fn route_message_response_returns_none_for_unknown_cmd_type() {
        let (mgr, router) = fixture();
        mgr.register(
            "34020000001110000001",
            11,
            PendingCmdType::DeviceInfo,
            "call-unk",
            None,
        );
        let body = r#"<?xml version="1.0"?><Response><CmdType>Future</CmdType></Response>"#;
        let resolved = router.route_message_response(body, "call-unk");
        assert!(resolved.is_none());
        // 不应被错删
        assert_eq!(mgr.pending_count(), 1);
    }

    #[test]
    fn route_message_response_returns_none_for_unregistered_call_id() {
        let (_mgr, router) = fixture();
        let body = r#"<?xml version="1.0"?><Response><CmdType>DeviceInfo</CmdType></Response>"#;
        let resolved = router.route_message_response(body, "call-not-registered");
        assert!(resolved.is_none());
    }

    #[test]
    fn accumulate_record_info_collects_all_packets() {
        let (_mgr, router) = fixture();
        let mut buffer = String::new();
        let mut packet_count = 0;
        let packet1 = r#"<?xml version="1.0"?><Response><CmdType>RecordInfo</CmdType><SumNum>2</SumNum><RecordList><Item><Name>seg1</Name></Item></RecordList></Response>"#;
        assert!(!router.accumulate_record_info("c1", packet1, &mut buffer, &mut packet_count, 2));
        assert!(buffer.contains("<Item>"));
        let packet2 = r#"<?xml version="1.0"?><Response><CmdType>RecordInfo</CmdType><SumNum>2</SumNum><RecordList><Item><Name>seg2</Name></Item></RecordList></Response>"#;
        let done = router.accumulate_record_info("c1", packet2, &mut buffer, &mut packet_count, 2);
        assert!(done);
        assert_eq!(packet_count, 2);
        assert!(buffer.contains("seg1"));
        assert!(buffer.contains("seg2"));
    }

    #[test]
    fn accumulate_record_info_returns_true_when_count_matches() {
        let (_mgr, router) = fixture();
        let mut buffer = String::new();
        let mut packet_count = 0;
        let packet = r#"<?xml version="1.0"?><Response><CmdType>RecordInfo</CmdType><SumNum>1</SumNum><RecordList><Item><Name>only</Name></Item></RecordList></Response>"#;
        let done = router.accumulate_record_info("c1", packet, &mut buffer, &mut packet_count, 1);
        assert!(done);
        assert_eq!(packet_count, 1);
    }

    #[test]
    fn route_response_invite_bye_cancel_call_id_completes() {
        let (mgr, router) = fixture();
        for (i, call_id) in ["call-invite", "call-bye", "call-cancel"]
            .iter()
            .enumerate()
        {
            mgr.register(
                "34020000001110000001",
                i as u32 + 1,
                PendingCmdType::DeviceInfo,
                call_id,
                None,
            );
        }
        for (status, call_id) in [
            (200u16, "call-invite"),
            (200u16, "call-bye"),
            (487u16, "call-cancel"),
        ] {
            let cmd = router.route_response(status, call_id);
            assert_eq!(cmd, Some(PendingCmdType::DeviceInfo));
        }
        assert_eq!(mgr.pending_count(), 0);
    }
}

impl ResponseRouter {
    /// 批量解析 RecordInfo 多包响应
    ///
    /// RecordInfo 响应可能分多包发送（SumNum > 1）。
    /// 将 body 内容追加到已积累的缓冲中，返回是否收齐。
    pub fn accumulate_record_info(
        &self,
        _call_id: &str,
        body: &str,
        buffer: &mut String,
        packet_count: &mut i32,
        total_num: i32,
    ) -> bool {
        use crate::sip::gb28181::XmlParser;
        let _ = _call_id;
        if buffer.is_empty() {
            *buffer = body.to_string();
        } else {
            // 后续包只把 RecordItem 节点 append，避免覆盖前面的 <Response> 头
            let items = XmlParser::extract_record_items(body);
            for item in items {
                *buffer += &item;
            }
        }

        // GB28181 SumNum/Num 是包序号不是 item 数。调用方维护 packet_count，
        // 每收一包 +1，达到 total_num 即收齐。
        *packet_count += 1;
        *packet_count >= total_num
    }
}

/// 从 XML 中提取 RecordItem 节点（辅助方法）
impl crate::sip::gb28181::XmlParser {
    /// 提取 RecordInfo XML 中的 RecordItem 节点列表
    #[allow(dead_code)]
    pub fn extract_record_items(xml: &str) -> Vec<String> {
        let mut items = Vec::new();
        for (open_tag, close_tag) in [
            ("<RecordItem>", "</RecordItem>"),
            ("<RecordItem ", "</RecordItem>"),
            ("<Item>", "</Item>"),
            ("<Item ", "</Item>"),
        ] {
            let mut cursor = 0;
            while let Some(begin) = xml[cursor..].find(open_tag) {
                let abs_begin = cursor + begin;
                if let Some(end) = xml[abs_begin..].find(close_tag) {
                    let item_end = abs_begin + end + close_tag.len();
                    items.push(xml[abs_begin..item_end].to_string());
                    cursor = item_end;
                } else {
                    break;
                }
            }
        }
        items
    }

    /// 统计 RecordInfo XML 中 RecordItem 的数量
    #[allow(dead_code)]
    pub fn count_record_items(xml: &str) -> i32 {
        let record_count =
            xml.matches("<RecordItem>").count() as i32 + xml.matches("<RecordItem ").count() as i32;
        if record_count > 0 {
            return record_count;
        }
        if xml.contains("<RecordList>") || xml.contains("<RecordList ") {
            return xml.matches("<Item>").count() as i32 + xml.matches("<Item ").count() as i32;
        }
        xml.matches("<Item>").count() as i32 + xml.matches("<Item ").count() as i32
    }
}
