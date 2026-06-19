//! SIP 协议集成测试
//!
//! 测试 PendingRequestManager 在模拟 SIP MESSAGE 响应下的行为。
//! 这些测试不依赖真实 SIP 网络，而是直接调用 ResponseRouter 的路由逻辑。

use gbserver::sip::gb28181::pending_request::{
    PendingRequestManager, PendingCmdType, QueryResult,
};
use gbserver::sip::gb28181::ResponseRouter;

#[test]
fn test_pending_request_register_and_complete() {
    let mgr = PendingRequestManager::new();
    let call_id = "test-call-001";

    // 注册一个 DeviceInfo 查询请求
    mgr.register(
        "34020000001320000001",
        42,
        PendingCmdType::DeviceInfo,
        call_id,
        None,
    );

    assert_eq!(mgr.pending_count(), 1, "应有 1 个等待中的请求");
    assert!(mgr.has_pending_for_device("34020000001320000001"));

    // 模拟设备返回 DeviceInfo 响应 XML
    let xml_response = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<SN>42</SN>
<DeviceID>34020000001320000001</DeviceID>
<DeviceName>TestCamera</DeviceName>
<Manufacturer>Hikvision</Manufacturer>
<Model>DS-2CD2043</Model>
<FirmwareVersion>V5.5.81</FirmwareVersion>
<Channel>4</Channel>
</Response>"#;

    // 完成等待中的请求
    let result_xml = mgr.complete(call_id, xml_response);
    assert!(result_xml.is_some(), "complete() 应返回 XML");
    assert_eq!(mgr.pending_count(), 0, "完成后计数应为 0");

    // 解析结构化结果
    let result = mgr.parse_response(PendingCmdType::DeviceInfo, xml_response);
    match result {
        QueryResult::DeviceInfo(info) => {
            assert_eq!(info.device_name.as_deref(), Some("TestCamera"));
            assert_eq!(info.manufacturer.as_deref(), Some("Hikvision"));
            assert_eq!(info.channel_count, Some(4));
        }
        _ => panic!("期望 DeviceInfo 结果"),
    }
}

#[test]
fn test_pending_request_multiple_devices() {
    let mgr = PendingRequestManager::new();

    // 注册多个设备的不同请求
    mgr.register("34020000001110000001", 1, PendingCmdType::DeviceInfo, "call-d1-1", None);
    mgr.register("34020000001110000001", 2, PendingCmdType::DeviceStatus, "call-d1-2", None);
    mgr.register("34020000002220000002", 1, PendingCmdType::DeviceInfo, "call-d2-1", None);

    assert_eq!(mgr.pending_count(), 3);

    // 取消设备1的所有请求
    let cancelled = mgr.cancel_all_for_device("34020000001110000001");
    assert_eq!(cancelled, 2);
    assert_eq!(mgr.pending_count(), 1);

    // 设备2的请求仍存在
    assert!(mgr.has_pending_for_device("34020000002220000002"));
}

#[test]
fn test_pending_request_cancel_single() {
    let mgr = PendingRequestManager::new();
    mgr.register("34020000001110000001", 5, PendingCmdType::DeviceInfo, "call-5", None);
    mgr.register("34020000001110000001", 6, PendingCmdType::DeviceStatus, "call-6", None);

    assert_eq!(mgr.pending_count(), 2);

    // 只取消 SN=5 的请求
    let result = mgr.cancel_for_device("34020000001110000001", 5);
    assert!(result);
    assert_eq!(mgr.pending_count(), 1);

    // SN=6 的仍存在
    assert!(mgr.has_pending_for_device("34020000001110000001"));
}

#[test]
fn test_pending_request_timeout_cleanup() {
    // 使用 0 秒超时的管理器（立即过期）
    let mgr = PendingRequestManager::with_timeout(0);
    mgr.register("34020000001110000001", 1, PendingCmdType::DeviceInfo, "call-x", None);
    assert_eq!(mgr.pending_count(), 1);

    // 等待一瞬间（超时管理器会认为已过期）
    std::thread::sleep(std::time::Duration::from_millis(10));

    let removed = mgr.cleanup_expired();
    assert_eq!(removed.len(), 1);
    assert_eq!(mgr.pending_count(), 0);
}

#[test]
fn test_pending_request_device_status_response() {
    let mgr = PendingRequestManager::new();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceStatus</CmdType>
<SN>7</SN>
<DeviceID>34020000001320000001</DeviceID>
<Online>ONLINE</Online>
<Status>OK</Status>
<DeviceTime>2026-01-01T12:00:00</DeviceTime>
<EncodeChannel>4</EncodeChannel>
<RecordChannel>4</RecordChannel>
</Response>"#;

    let result = mgr.parse_response(PendingCmdType::DeviceStatus, xml);
    match result {
        QueryResult::DeviceStatus(status) => {
            assert_eq!(status.online.as_deref(), Some("ONLINE"));
            assert_eq!(status.status.as_deref(), Some("OK"));
            assert_eq!(status.encode_channel_count, Some(4));
        }
        _ => panic!("期望 DeviceStatus 结果"),
    }
}

#[test]
fn test_pending_request_record_info_raw() {
    let mgr = PendingRequestManager::new();
    let xml = r#"<?xml version="1.0"?>
<Response>
<CmdType>RecordInfo</CmdType>
<SN>10</SN>
<DeviceID>34020000001320000001</DeviceID>
<SumNum>2</SumNum>
<Num>2</Num>
<RecordList>
<Item><Name>Recording-001</Name><StartTime>2026-01-01T08:00:00</StartTime><EndTime>2026-01-01T08:30:00</EndTime></Item>
<Item><Name>Recording-002</Name><StartTime>2026-01-01T09:00:00</StartTime><EndTime>2026-01-01T09:15:00</EndTime></Item>
</RecordList>
</Response>"#;

    // RecordInfo 未专门解析，返回原始 XML
    let result = mgr.parse_response(PendingCmdType::RecordInfo, xml);
    match result {
        QueryResult::Raw(raw) => {
            assert!(raw.contains("Recording-001"));
            assert!(raw.contains("Recording-002"));
        }
        _ => panic!("期望 Raw 结果"),
    }
}

#[test]
fn test_response_router_message_response() {
    let mgr = PendingRequestManager::new();
    let router = ResponseRouter::new(std::sync::Arc::new(mgr));

    // 注册请求
    let pending = mgr.register(
        "34020000001320000001",
        100,
        PendingCmdType::DeviceInfo,
        "router-call-001",
        None,
    );

    let response_xml = r#"<?xml version="1.0"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<SN>100</SN>
<DeviceID>34020000001320000001</DeviceID>
<DeviceName>RouterTestCam</DeviceName>
<Manufacturer>Test</Manufacturer>
</Response>"#;

    // 路由 MESSAGE 响应
    let result = router.route_message_response(response_xml, "router-call-001");
    assert!(result.is_some());

    let (cmd_type, xml) = result.unwrap();
    assert_eq!(cmd_type, PendingCmdType::DeviceInfo);
    assert!(xml.contains("RouterTestCam"));
}

#[test]
fn test_response_router_unsolicited() {
    let mgr = PendingRequestManager::new();
    let router = ResponseRouter::new(std::sync::Arc::new(mgr));

    // 未注册的 Call-ID 的响应，应返回 None（不 panic）
    let xml = r#"<?xml version="1.0"?><Response><CmdType>DeviceInfo</CmdType></Response>"#;
    let result = router.route_message_response(xml, "unknown-call-id");
    assert!(result.is_none());
}

#[test]
fn test_response_router_accumulate_record_info() {
    let mgr = PendingRequestManager::new();
    let router = ResponseRouter::new(std::sync::Arc::new(mgr));

    let call_id = "ri-call-001";
    let mut buffer = String::new();
    let total = 2;

    // 第一包
    let page1 = r#"<?xml version="1.0"?>
<Response>
<CmdType>RecordInfo</CmdType>
<SN>1</SN>
<DeviceID>34020000001320000001</DeviceID>
<SumNum>2</SumNum>
<Num>2</Num>
<Item><Name>Rec-01</Name></Item>
</Response>"#;

    let done = router.accumulate_record_info(call_id, page1, &mut buffer, total);
    assert!(!done, "第一包不应认为收齐");
    assert!(buffer.contains("Rec-01"));

    // 第二包
    let page2 = r#"<?xml version="1.0"?>
<Response>
<Item><Name>Rec-02</Name></Item>
</Response>"#;

    let done = router.accumulate_record_info(call_id, page2, &mut buffer, total);
    assert!(done, "两包后应认为收齐");
    assert!(buffer.contains("Rec-01"));
    assert!(buffer.contains("Rec-02"));
}
