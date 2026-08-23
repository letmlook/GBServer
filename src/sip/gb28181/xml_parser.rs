use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct XmlParser;

impl XmlParser {
    /// Properly maps each XML tag name to its text content.
    /// For nested elements with the same name, the last value wins.
    pub fn parse_fields(xml: &str) -> HashMap<String, String> {
        let mut reader = Reader::from_str(xml);
        let mut result = HashMap::new();
        let mut buf = Vec::new();
        let mut current_tag = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                }
                Ok(Event::Text(e)) => {
                    let text = String::from_utf8_lossy(&e).trim().to_string();
                    if !text.is_empty() && !current_tag.is_empty() {
                        result.insert(current_tag.clone(), text);
                    }
                }
                Ok(Event::End(_)) => {
                    current_tag.clear();
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }
        result
    }

    pub fn parse(xml: &str) -> HashMap<String, String> {
        let mut reader = Reader::from_str(xml);
        let mut result = HashMap::new();
        let mut buf = Vec::new();
        let mut current_tag = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = String::from_utf8_lossy(&attr.value).to_string();
                        result.insert(key, value);
                    }
                    current_tag = name.clone();
                    result.insert("__tag".to_string(), name);
                }
                Ok(Event::Text(e)) => {
                    let text = String::from_utf8_lossy(&e).trim().to_string();
                    if !text.is_empty() {
                        // 既保留原 __text 全局键,也写入当前 tag 名作为 key,
                        // 这样 XmlParser::get_cmd_type / get_device_id 能在 GB28181
                        // 使用元素内容（如 <CmdType>Keepalive</CmdType>）时正确取值。
                        result.insert("__text".to_string(), text.clone());
                        if !current_tag.is_empty() {
                            result.insert(current_tag.clone(), text);
                        }
                    }
                }
                Ok(Event::End(_)) => {
                    current_tag.clear();
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }
        result
    }

    pub fn get_device_id(xml: &str) -> Option<String> {
        // 不使用 parse() 的 HashMap 提取 — 后者在同名标签多次出现时会保留最后一个值
        // （例如 <Response><DeviceID>parent</DeviceID>...
        //       <DeviceList><Item><DeviceID>child1</DeviceID>...
        // 上层需要的是父 DeviceID），所以这里用直接字符串扫描返回首个 <DeviceID>。
        if let Some(val) = Self::find_first_element(xml, "DeviceID") {
            return Some(val);
        }
        // 属性形式：Query/Notify/Response 根标签上的 DeviceID="..."，GB/T 28181 设备普遍用此形式
        Self::find_first_attr(xml, "DeviceID")
    }

    pub fn get_cmd_type(xml: &str) -> Option<String> {
        let parsed = Self::parse(xml);
        parsed.get("CmdType").cloned()
    }

    pub fn get_sn(xml: &str) -> Option<u32> {
        let parsed = Self::parse(xml);
        parsed.get("SN").and_then(|s| s.parse().ok())
    }

    pub fn build_response(cmd_type: &str, sn: u32, device_id: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="{}"?><Response><CmdType>{}</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result></Response>"#,
            "UTF-8", cmd_type, sn, device_id
        )
    }

    pub fn build_catalog(device_id: &str, sn: u32, channels: &[ChannelInfo]) -> String {
        let mut channel_xml = String::new();
        for ch in channels {
            channel_xml.push_str(&format!(
                r#"<Item><DeviceID>{}</DeviceID><Name>{}</Name><Manufacturer>{}</Manufacturer><Model>{}</Model><Owner>{}</Owner><CivilCode>{}</CivilCode><Address>{}</Address><Status>{}</Status><Longitude>{}</Longitude><Latitude>{}</Latitude></Item>"#,
                ch.device_id,
                ch.name,
                ch.manufacturer.as_deref().unwrap_or(""),
                ch.model.as_deref().unwrap_or(""),
                ch.owner.as_deref().unwrap_or(""),
                ch.civil_code.as_deref().unwrap_or(""),
                ch.address.as_deref().unwrap_or(""),
                ch.status,
                ch.longitude.unwrap_or(0.0),
                ch.latitude.unwrap_or(0.0)
            ));
        }

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Notify><CmdType>Catalog</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><SumNum>{}</SumNum><DeviceList Num="{}">{}</DeviceList></Notify>"#,
            sn,
            device_id,
            channels.len(),
            channels.len(),
            channel_xml
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    pub device_id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub owner: Option<String>,
    pub civil_code: Option<String>,
    pub address: Option<String>,
    pub status: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub parent_id: Option<String>,
    pub safety_cap: Option<String>,
    pub snapshot_url: Option<String>,
    pub ptz_type: Option<i32>,
    pub stream_count: Option<i32>,
    pub has_audio: Option<bool>,
    pub sub_count: Option<i32>,
    pub register_status: Option<String>,
    pub channel_type: Option<String>,
}

impl XmlParser {
    pub fn parse_catalog_channels(xml: &str) -> (Option<i32>, Vec<ChannelInfo>) {
        let mut reader = Reader::from_str(xml);
        let mut buf = Vec::new();
        let mut channels = Vec::new();
        let mut sum_num = None;
        let mut in_item = false;
        let mut current_tag = String::new();
        let mut current_channel = ChannelInfo::default();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "Item" {
                        in_item = true;
                        current_channel = ChannelInfo::default();
                    }
                    current_tag = name;
                }
                Ok(Event::Empty(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "Item" && in_item {
                        channels.push(current_channel.clone());
                        in_item = false;
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = String::from_utf8_lossy(&e).trim().to_string();
                    if text.is_empty() { continue; }

                    if current_tag == "SumNum" {
                        sum_num = text.parse().ok();
                    }

                    if !in_item { continue; }

                    match current_tag.as_str() {
                        "DeviceID" => current_channel.device_id = text,
                        "Name" => current_channel.name = text,
                        "Manufacturer" => current_channel.manufacturer = Some(text),
                        "Model" => current_channel.model = Some(text),
                        "Owner" => current_channel.owner = Some(text),
                        "CivilCode" => current_channel.civil_code = Some(text),
                        "Address" => current_channel.address = Some(text),
                        "Status" => current_channel.status = text,
                        "Longitude" => current_channel.longitude = text.parse().ok(),
                        "Latitude" => current_channel.latitude = text.parse().ok(),
                        "ParentID" => current_channel.parent_id = Some(text),
                        "SafetyCap" => current_channel.safety_cap = Some(text),
                        "SnapshotURL" => current_channel.snapshot_url = Some(text),
                        "PTZType" => current_channel.ptz_type = text.parse().ok(),
                        "StreamCount" => current_channel.stream_count = text.parse().ok(),
                        "HasAudio" => current_channel.has_audio = Some(text == "1" || text.to_lowercase() == "true"),
                        "SubCount" => current_channel.sub_count = text.parse().ok(),
                        "RegisterStatus" => current_channel.register_status = Some(text),
                        "ChannelType" => current_channel.channel_type = Some(text),
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "Item" && in_item {
                        channels.push(current_channel.clone());
                        in_item = false;
                    }
                    current_tag.clear();
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }

        (sum_num, channels)
    }

    /// 提取首个 `<TagName>...</TagName>` 元素值（不依赖 quick-xml 解析，对同名多标签取首个）
    pub fn find_first_element(xml: &str, tag: &str) -> Option<String> {
        let open_marker = format!("<{}>", tag);
        let close_marker = format!("</{}>", tag);
        let open = xml.find(&open_marker)?;
        let start = open + open_marker.len();
        let end_close = xml[start..].find(&close_marker)?;
        let val = xml[start..start + end_close].trim();
        if val.is_empty() {
            None
        } else {
            Some(val.to_string())
        }
    }

    /// 提取首个 `TagName="..."` 属性值（GB/T 28181 设备常在根标签用属性形式传 DeviceID）
    pub fn find_first_attr(xml: &str, attr: &str) -> Option<String> {
        let marker = format!("{}=\"", attr);
        let idx = xml.find(&marker)?;
        let start = idx + marker.len();
        let end_q = xml[start..].find('"')?;
        let val = xml[start..start + end_q].trim();
        if val.is_empty() {
            None
        } else {
            Some(val.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 元素形式 `<DeviceID>...</DeviceID>` 仍然优先识别（兼容历史路径）
    #[test]
    fn find_first_element_basic() {
        let xml = r#"<Response><DeviceID>34020000002000000001</DeviceID><Result>OK</Result></Response>"#;
        assert_eq!(
            XmlParser::find_first_element(xml, "DeviceID").as_deref(),
            Some("34020000002000000001")
        );
    }

    /// 元素形式不区分大小写也是敏感，标签必须匹配
    #[test]
    fn find_first_element_missing_returns_none() {
        let xml = r#"<Response><Result>OK</Result></Response>"#;
        assert!(XmlParser::find_first_element(xml, "DeviceID").is_none());
    }

    /// 属性形式 `DeviceID="..."`：真实 GB 设备在 Query/Notify/Response 根标签使用
    #[test]
    fn find_first_attr_basic() {
        let xml = r#"<?xml version="1.0"?><Query CmdType="Catalog" DeviceID="34020000002000000001" SN="10"/>"#;
        assert_eq!(
            XmlParser::find_first_attr(xml, "DeviceID").as_deref(),
            Some("34020000002000000001")
        );
    }

    /// 属性不存在时返回 None，不返回空字符串
    #[test]
    fn find_first_attr_missing_returns_none() {
        let xml = r#"<?xml version="1.0"?><Query CmdType="Catalog" SN="10"/>"#;
        assert!(XmlParser::find_first_attr(xml, "DeviceID").is_none());
    }

    /// `get_device_id` 在元素形式存在时优先返回元素值（保持原行为）
    #[test]
    fn get_device_id_prefers_element_over_attr() {
        let xml = r#"<Response><DeviceID>34020000001110000001</DeviceID></Response>"#;
        assert_eq!(
            XmlParser::get_device_id(xml).as_deref(),
            Some("34020000001110000001")
        );
    }

    /// `get_device_id` 在仅属性形式时回退到属性（这是上游真实设备的主要形式）
    #[test]
    fn get_device_id_falls_back_to_attr() {
        let xml = r#"<?xml version="1.0"?><Query CmdType="Catalog" DeviceID="34020000002000000001" SN="10"/>"#;
        assert_eq!(
            XmlParser::get_device_id(xml).as_deref(),
            Some("34020000002000000001")
        );
    }

    /// 空元素/空属性都返回 None（避免把空字符串当合法 ID 传下去）
    #[test]
    fn get_device_id_empty_inputs_return_none() {
        let xml = r#"<Response><DeviceID>   </DeviceID></Response>"#;
        assert!(XmlParser::get_device_id(xml).is_none());

        let xml = r#"<Query CmdType="Catalog" DeviceID="" SN="10"/>"#;
        assert!(XmlParser::get_device_id(xml).is_none());
    }
}
