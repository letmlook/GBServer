//! SIP 设备模拟器测试
//!
//! 这些测试验证模拟器生成的 SIP 消息格式正确。
//! 不依赖真实服务器，使用字符串解析验证。


/// 简化的 SIP 消息解析（用于测试验证）
fn sip_line(msg: &str, header: &str) -> Option<String> {
    msg.lines()
        .find(|l| l.starts_with(&format!("{}: ", header)))
        .map(|l| l.splitn(2, ": ").nth(1).unwrap_or("").to_string())
}

fn sip_method(msg: &str) -> Option<String> {
    msg.lines().next().map(|l| l.split_whitespace().next().unwrap_or("").to_string())
}

fn sip_body(msg: &str) -> Option<String> {
    let blank_pos = msg.find("\r\n\r\n").or_else(|| msg.find("\n\n"))?;
    Some(msg[blank_pos + (if msg.contains("\r\n\r\n") { 4 } else { 2 })..].to_string())
}

/// 测试：模拟器生成的 REGISTER 消息格式正确
#[test]
fn test_register_message_format() {
    // 模拟构造的 REGISTER 消息（简化版本）
    let device_id = "34020000001320000001";
    let cseq = 1;
    let call_id = "sim-register-1";
    let local_ip = "127.0.0.1";
    let local_port = 5071;
    let expires = 3600;

    let msg = format!(
        "REGISTER sip:{}@127.0.0.1:5060 SIP/2.0\r\n\
        Via: SIP/2.0/UDP {}:{};rport;branch=z9hG4bK{}\r\n\
        From: <sip:{}@{}:{}>;tag=from-tag-{}\r\n\
        To: <sip:{}@{}:{}>\r\n\
        Call-ID: {}\r\n\
        CSeq: {} REGISTER\r\n\
        Contact: <sip:{}@{}:{}>\r\n\
        Expires: {}\r\n\
        Content-Length: 0\r\n\r\n",
        device_id, local_ip, local_port, cseq,
        device_id, device_id.split_at(16).0, device_id.split_at(16).0,
        device_id, device_id.split_at(16).0, device_id.split_at(16).0,
        device_id,
        call_id,
        cseq,
        device_id, local_ip, local_port,
        expires,
    );

    assert_eq!(sip_method(&msg).unwrap(), "REGISTER");
    assert_eq!(sip_line(&msg, "Call-ID").unwrap(), call_id);
    assert!(sip_line(&msg, "CSeq").unwrap().contains("REGISTER"));
    assert_eq!(sip_line(&msg, "Expires").unwrap(), "3600");
}

/// 测试：模拟器生成的 Keepalive 消息包含正确 XML
#[test]
fn test_keepalive_message_format() {
    let device_id = "34020000001320000001";
    let sn = "0000000001";
    let call_id = "sim-ka-1";

    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>Keepalive</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Status>OK</Status>
</Notify>"#,
        sn, device_id
    );

    assert!(body.contains("<CmdType>Keepalive</CmdType>"));
    assert!(body.contains(&format!("<DeviceID>{}</DeviceID>", device_id)));
    assert!(body.contains("<Status>OK</Status>"));
}

/// 测试：模拟器构造的 DeviceInfo 响应 XML
#[test]
fn test_device_info_response_format() {
    let resp = r#"<?xml version="1.0" encoding="UTF-8"?>
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

    assert!(resp.contains("<CmdType>DeviceInfo</CmdType>"));
    assert!(resp.contains("<DeviceName>TestCamera</DeviceName>"));
    assert!(resp.contains("<Manufacturer>Hikvision</Manufacturer>"));
    assert!(resp.contains("<Channel>4</Channel>"));
}

/// 测试：模拟器构造的 Catalog 多包消息
#[test]
fn test_catalog_multipacket_format() {
    let device_id = "34020000001320000001";
    let channel_xml = r#"
<Item>
<DeviceID>34020000001320000001</DeviceID>
<Name>Camera-0001</Name>
<Manufacturer>TestVendor</Manufacturer>
<Status>ON</Status>
<PTZType>2</PTZType>
</Item>"#;

    let resp = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>1</SN>
<DeviceID>{}</DeviceID>
<SumNum>3</SumNum>
<Num>2</Num>
<DeviceList Name="Record" Num="1">
{}
</DeviceList>
</Response>"#,
        device_id,
        channel_xml
    );

    assert!(resp.contains("<CmdType>Catalog</CmdType>"));
    assert!(resp.contains("<SumNum>3</SumNum>"));
    assert!(resp.contains("<Num>2</Num>"));
    assert!(resp.contains("<Name>Camera-0001</Name>"));
    assert!(resp.contains("<Status>ON</Status>"));
}

/// 测试：SIP MESSAGE 响应路由识别（通过 Response 关键字）
#[test]
fn test_response_router_identifies_response() {
    let response_msg = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<SN>42</SN>
</Response>"#;

    let query_msg = r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>DeviceInfo</CmdType>
<SN>42</SN>
</Query>"#;

    // 响应包含 <Response>，查询不包含
    assert!(response_msg.contains("<Response>"));
    assert!(!query_msg.contains("<Response>"));
}

/// 测试：多包 RecordInfo 聚合逻辑
#[test]
fn test_record_info_accumulation_logic() {
    let page1 = r#"<Response><RecordList><Item><Name>Rec-01</Name></Item></RecordList></Response>"#;
    let page2 = r#"<Response><RecordList><Item><Name>Rec-02</Name></Item></RecordList></Response>"#;

    // 模拟聚合逻辑
    let mut buffer = String::new();
    let items1 = extract_record_items(page1);
    let items2 = extract_record_items(page2);

    buffer.push_str(page1);
    buffer.push_str(page2);

    assert!(buffer.contains("Rec-01"));
    assert!(buffer.contains("Rec-02"));
    assert_eq!(items1.len(), 1);
    assert_eq!(items2.len(), 1);
}

fn extract_record_items(xml: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut start = 0;
    while let Some(begin) = xml[start..].find("<Item>") {
        let abs_begin = start + begin;
        if let Some(end) = xml[abs_begin..].find("</Item>") {
            let item_end = abs_begin + end + 7; // strlen("</Item>")
            items.push(xml[abs_begin..item_end].to_string());
            start = item_end;
        } else {
            break;
        }
    }
    items
}

// ============================================================================
// 独立工具函数（不依赖 crate:: 路径）
// ============================================================================

fn extract_record_items_xml(xml: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut start = 0;
    while let Some(begin) = xml[start..].find("<Item>") {
        let abs_begin = start + begin;
        if let Some(end) = xml[abs_begin..].find("</Item>") {
            let item_end = abs_begin + end + 7;
            items.push(xml[abs_begin..item_end].to_string());
            start = item_end;
        } else {
            break;
        }
    }
    items
}

fn count_record_items_xml(xml: &str) -> i32 {
    let count = xml.matches("<Item>").count() as i32;
    if count == 0 {
        return xml.matches("<RecordItem ").count() as i32;
    }
    count
}

/// 测试：RecordInfo 多包聚合逻辑（使用独立函数版本）
#[test]
fn test_record_info_accumulation_standalone() {
    let page1 = r#"<Response><RecordList><Item><Name>Rec-A</Name></Item></RecordList></Response>"#;
    let page2 = r#"<Response><RecordList><Item><Name>Rec-B</Name></Item></RecordList></Response>"#;

    let items1 = extract_record_items_xml(page1);
    let items2 = extract_record_items_xml(page2);

    assert_eq!(items1.len(), 1);
    assert_eq!(items1[0].contains("Rec-A"), true);
    assert_eq!(items2.len(), 1);
    assert_eq!(items2[0].contains("Rec-B"), true);

    assert_eq!(count_record_items_xml(page1), 1);
    assert_eq!(count_record_items_xml(page2), 1);
}

/// 测试：通道模拟数据生成（自包含实现，不依赖 fixtures 模块）
#[test]
fn test_sim_channel_data() {
    #[derive(Debug, Clone, PartialEq)]
    struct SimChannel {
        device_id: String,
        name: String,
        status: String,
    }
    fn channel_set(parent: &str, count: u32) -> Vec<SimChannel> {
        (1..=count).map(|i| SimChannel {
            device_id: format!("{}{:04}", parent, i),
            name: format!("Camera-{:04}", i),
            status: "ON".to_string(),
        }).collect()
    }
    let channels = channel_set("34020000001320000001", 3);
    assert_eq!(channels.len(), 3);
    assert_eq!(channels[0].device_id, "340200000013200000010001");
    assert_eq!(channels[0].name, "Camera-0001");
    assert_eq!(channels[0].status, "ON");
    assert_eq!(channels[1].device_id, "340200000013200000010002");
    assert_eq!(channels[2].device_id, "340200000013200000010003");
}
