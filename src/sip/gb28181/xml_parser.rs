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

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = String::from_utf8_lossy(&attr.value).to_string();
                        result.insert(key, value);
                    }
                    result.insert("__tag".to_string(), name);
                }
                Ok(Event::Text(e)) => {
                    let text = String::from_utf8_lossy(&e).trim().to_string();
                    if !text.is_empty() {
                        result.insert("__text".to_string(), text);
                    }
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }
        result
    }

    pub fn get_device_id(xml: &str) -> Option<String> {
        let parsed = Self::parse(xml);
        parsed.get("DeviceID").cloned()
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
}
