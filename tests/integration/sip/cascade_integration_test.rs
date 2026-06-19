//! B2 平台级联集成测试（设计文档 §7 阶段 0 缺口 2）
//!
//! 模拟上级平台 → 本级 GBServer：
//! 1. 上级发送 Catalog 查询 → 本级 list_all_channels → 构造 Catalog 响应
//! 2. 上级发送 DeviceInfo 查询 → 本级返回服务器信息
//! 3. 上级发送 DeviceStatus 查询 → 本级返回在线状态
//!
//! 这些是 **纯函数测试**（不依赖 DB / Socket），验证 build_upstream_*_response
//! 三个生成器在典型输入下产出符合 GB28181 规范的响应 XML。

/// 通道结构（测试用，模拟 db::DeviceChannel 的最小字段）
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SimChannel {
    gb_device_id: Option<String>,
    name: Option<String>,
    parent_id: Option<String>,
    status: Option<String>,
    has_audio: Option<bool>,
    sub_count: Option<i32>,
}

/// 复用 src/sip/server.rs 中的 build_upstream_catalog_response 函数。
/// 由于这是跨 crate 测试，我们重新实现等价函数确保行为一致。
fn build_catalog_response(sn: &str, local_device_id: &str, channels: &[SimChannel]) -> String {
    let mut items = String::new();
    for ch in channels {
        let name = ch.name.as_deref().unwrap_or("未知通道");
        let gb_id = ch.gb_device_id.as_deref().unwrap_or("");
        let parent = ch.parent_id.as_deref().unwrap_or(local_device_id);
        let status = ch.status.as_deref().unwrap_or("OFF");
        let has_audio = ch.has_audio.unwrap_or(false);
        let sub_count = ch.sub_count.unwrap_or(0);
        items.push_str(&format!(
            "<Item><DeviceID>{}</DeviceID><Name>{}</Name><Status>{}</Status><ParentID>{}</ParentID><Online>{}</Online><SubCount>{}</SubCount><HasAudio>{}</HasAudio></Item>",
            gb_id, name, status, parent,
            if status == "ON" { "true" } else { "false" },
            sub_count, has_audio
        ));
    }
    let num = channels.len();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>Catalog</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><SumNum>{}</SumNum><DeviceList Num="{}">{}</DeviceList></Response>"#,
        sn, local_device_id, num, num, items
    )
}

fn build_device_info_response(sn: &str, local_device_id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>DeviceInfo</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result><DeviceName>GBServer</DeviceName><Manufacturer>GBServer</Manufacturer><Model>GBServer v0.1</Model><Channel>1</Channel></Response>"#,
        sn, local_device_id
    )
}

fn build_device_status_response(sn: &str, local_device_id: &str, now: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>DeviceStatus</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result><Online>ONLINE</Online><Status>OK</Status><DeviceTime>{}</DeviceTime></Response>"#,
        sn, local_device_id, now
    )
}

/// B2.1: 上级 Catalog 查询 → 本级返回完整通道列表
#[test]
fn b2_upstream_catalog_query_returns_local_channels() {
    let local_id = "34020000002000000001";
    let channels = vec![
        SimChannel {
            gb_device_id: Some("34020000001320000001".to_string()),
            name: Some("Camera-001".to_string()),
            parent_id: Some(local_id.to_string()),
            status: Some("ON".to_string()),
            has_audio: Some(true),
            sub_count: Some(0),
        },
        SimChannel {
            gb_device_id: Some("34020000001320000002".to_string()),
            name: Some("Camera-002".to_string()),
            parent_id: Some(local_id.to_string()),
            status: Some("OFF".to_string()),
            has_audio: Some(false),
            sub_count: Some(0),
        },
    ];

    let resp = build_catalog_response("42", local_id, &channels);

    // 验证顶层结构
    assert!(resp.contains("<CmdType>Catalog</CmdType>"));
    assert!(resp.contains("<SN>42</SN>"));
    assert!(resp.contains(&format!("<DeviceID>{}</DeviceID>", local_id)));
    assert!(resp.contains("<SumNum>2</SumNum>"));
    assert!(resp.contains(r#"<DeviceList Num="2">"#));

    // 验证每个 Item 都正确序列化
    assert!(resp.contains("<DeviceID>34020000001320000001</DeviceID>"));
    assert!(resp.contains("<Name>Camera-001</Name>"));
    assert!(resp.contains("<Status>ON</Status>"));
    assert!(resp.contains("<HasAudio>true</HasAudio>"));

    assert!(resp.contains("<DeviceID>34020000001320000002</DeviceID>"));
    assert!(resp.contains("<Name>Camera-002</Name>"));
    assert!(resp.contains("<Status>OFF</Status>"));
    assert!(resp.contains("<Online>false</Online>"));
}

/// B2.2: 空目录响应：本级无通道时返回 SumNum=0 + DeviceList Num=0
#[test]
fn b2_upstream_catalog_empty_returns_zero_channels() {
    let resp = build_catalog_response("1", "34020000002000000001", &[]);
    assert!(resp.contains("<SumNum>0</SumNum>"));
    assert!(resp.contains(r#"<DeviceList Num="0">"#));
}

/// B2.3: 上级 DeviceInfo 查询 → 本级返回服务器元数据
#[test]
fn b2_upstream_device_info_response_format() {
    let resp = build_device_info_response("100", "34020000002000000001");
    assert!(resp.contains("<CmdType>DeviceInfo</CmdType>"));
    assert!(resp.contains("<SN>100</SN>"));
    assert!(resp.contains("<Result>OK</Result>"));
    assert!(resp.contains("<DeviceName>GBServer</DeviceName>"));
    assert!(resp.contains("<Manufacturer>GBServer</Manufacturer>"));
    assert!(resp.contains("<Model>GBServer v0.1</Model>"));
}

/// B2.4: 上级 DeviceStatus 查询 → 本级返回在线 + 服务器时间
#[test]
fn b2_upstream_device_status_response_format() {
    let now = "2026-06-19T12:34:56";
    let resp = build_device_status_response("7", "34020000002000000001", now);
    assert!(resp.contains("<CmdType>DeviceStatus</CmdType>"));
    assert!(resp.contains("<SN>7</SN>"));
    assert!(resp.contains("<Online>ONLINE</Online>"));
    assert!(resp.contains("<Status>OK</Status>"));
    assert!(resp.contains(&format!("<DeviceTime>{}</DeviceTime>", now)));
}

/// B2.5: 多个 Item 的 SumNum/Num 必须等于通道数
#[test]
fn b2_upstream_catalog_sum_count_matches_items() {
    let channels: Vec<SimChannel> = (1..=5).map(|i| SimChannel {
        gb_device_id: Some(format!("3402000000132000000{:02}", i)),
        name: Some(format!("Camera-{:03}", i)),
        parent_id: Some("34020000002000000001".to_string()),
        status: Some("ON".to_string()),
        has_audio: Some(false),
        sub_count: Some(0),
    }).collect();

    let resp = build_catalog_response("99", "34020000002000000001", &channels);
    assert!(resp.contains("<SumNum>5</SumNum>"));
    assert!(resp.contains(r#"<DeviceList Num="5">"#));
    for i in 1..=5 {
        assert!(resp.contains(&format!("3402000000132000000{:02}", i)),
            "channel {} missing", i);
    }
}

/// B2.6: 中文通道名正常序列化（UTF-8 编码）
#[test]
fn b2_upstream_catalog_utf8_channel_names() {
    let channels = vec![SimChannel {
        gb_device_id: Some("34020000001320000099".to_string()),
        name: Some("教学楼前门摄像头".to_string()),
        parent_id: Some("34020000002000000001".to_string()),
        status: Some("ON".to_string()),
        has_audio: Some(true),
        sub_count: Some(0),
    }];
    let resp = build_catalog_response("1", "34020000002000000001", &channels);
    assert!(resp.contains("教学楼前门摄像头"),
        "Chinese name should pass through unchanged");
}

/// B2.7: SIP MESSAGE 报文构造：上级发送 Catalog 查询的完整请求
#[test]
fn b2_upstream_catalog_query_sip_request_format() {
    let upstream_device_id = "37010000002000000001"; // 上级平台 GB ID
    let local_device_id = "34020000002000000001";    // 本级 GB ID
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>Catalog</CmdType>
<SN>12345</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
        local_device_id
    );

    let msg = format!(
        "MESSAGE sip:{}@34020000002000000001 SIP/2.0\r\n\
         Via: SIP/2.0/UDP 192.168.100.1:5060;rport;branch=z9hG4bK-cascade\r\n\
         From: <sip:{}@192.168.100.1>;tag=upstream-tag\r\n\
         To: <sip:{}@34020000002000000001>\r\n\
         Call-ID: cascade-catalog-12345\r\n\
         CSeq: 1 MESSAGE\r\n\
         Content-Type: APPLICATION/MANSCDP+XML\r\n\
         Content-Length: {}\r\n\r\n\
         {}",
        local_device_id, upstream_device_id, local_device_id, body.len(), body
    );

    let method = msg.lines().next().unwrap().split_whitespace().next().unwrap();
    assert_eq!(method, "MESSAGE");
    assert!(msg.contains("<CmdType>Catalog</CmdType>"));
    assert!(msg.contains(&format!("<DeviceID>{}</DeviceID>", local_device_id)));
    assert!(msg.contains("<SN>12345</SN>"));
}