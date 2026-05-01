use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoResponse {
    pub device_name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub channel_count: Option<i32>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatusResponse {
    pub online: Option<String>,
    pub status: Option<String>,
    pub device_time: Option<String>,
    pub encode_channel_count: Option<i32>,
    pub decode_channel_count: Option<i32>,
    pub record_channel_count: Option<i32>,
    pub storage_space: Option<StorageSpace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSpace {
    pub total: Option<i64>,
    pub remain: Option<i64>,
}

pub struct DeviceQueryManager {
    pending_queries: Arc<DashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

use dashmap::DashMap;

impl DeviceQueryManager {
    pub fn new() -> Self {
        Self {
            pending_queries: Arc::new(DashMap::new()),
        }
    }

    pub fn register_pending(&self, call_id: &str, sender: tokio::sync::oneshot::Sender<String>) {
        self.pending_queries.insert(call_id.to_string(), sender);
    }

    pub fn complete_pending(&self, call_id: &str, response: String) -> bool {
        if let Some((_, sender)) = self.pending_queries.remove(call_id) {
            let _ = sender.send(response);
            true
        } else {
            false
        }
    }

    pub fn build_device_info_xml(device_id: &str, sn: u32) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>DeviceInfo</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            sn, device_id
        )
    }

    pub fn build_device_status_xml(device_id: &str, sn: u32) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>DeviceStatus</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            sn, device_id
        )
    }

    pub fn parse_device_info(xml: &str) -> DeviceInfoResponse {
        let fields = crate::sip::gb28181::XmlParser::parse_fields(xml);
        DeviceInfoResponse {
            device_name: fields.get("DeviceName").cloned(),
            manufacturer: fields.get("Manufacturer").cloned(),
            model: fields.get("Model").cloned(),
            firmware: fields.get("FirmwareVersion").or_else(|| fields.get("Firmware")).cloned(),
            channel_count: fields.get("Channel").and_then(|s| s.parse().ok()),
            serial_number: fields.get("SerialNumber").cloned(),
        }
    }

    pub fn parse_device_status(xml: &str) -> DeviceStatusResponse {
        let fields = crate::sip::gb28181::XmlParser::parse_fields(xml);
        let storage = if fields.contains_key("StorageSpace") || fields.contains_key("TotalSpace") {
            Some(StorageSpace {
                total: fields.get("TotalSpace").and_then(|s| s.parse().ok()),
                remain: fields.get("RemainSpace").and_then(|s| s.parse().ok()),
            })
        } else {
            None
        };

        DeviceStatusResponse {
            online: fields.get("Online").cloned(),
            status: fields.get("Status").cloned(),
            device_time: fields.get("DeviceTime").cloned(),
            encode_channel_count: fields.get("EncodeChannel").or_else(|| fields.get("ChannelNum")).and_then(|s| s.parse().ok()),
            decode_channel_count: fields.get("DecodeChannel").and_then(|s| s.parse().ok()),
            record_channel_count: fields.get("RecordChannel").and_then(|s| s.parse().ok()),
            storage_space: storage,
        }
    }
}

impl Default for DeviceQueryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_device_info_xml() {
        let xml = DeviceQueryManager::build_device_info_xml("34020000001320000001", 1);
        assert!(xml.contains("DeviceInfo"));
        assert!(xml.contains("34020000001320000001"));
    }

    #[test]
    fn test_parse_device_info() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<DeviceName>Camera1</DeviceName>
<Manufacturer>Hikvision</Manufacturer>
<Model>DS-2CD</Model>
<FirmwareVersion>V5.3</FirmwareVersion>
<Channel>4</Channel>
</Response>"#;
        let info = DeviceQueryManager::parse_device_info(xml);
        assert_eq!(info.device_name.as_deref(), Some("Camera1"));
        assert_eq!(info.manufacturer.as_deref(), Some("Hikvision"));
        assert_eq!(info.channel_count, Some(4));
    }

    #[test]
    fn test_parse_device_status() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceStatus</CmdType>
<Online>ONLINE</Online>
<Status>OK</Status>
<DeviceTime>2024-01-01T00:00:00</DeviceTime>
<EncodeChannel>4</EncodeChannel>
</Response>"#;
        let status = DeviceQueryManager::parse_device_status(xml);
        assert_eq!(status.online.as_deref(), Some("ONLINE"));
        assert_eq!(status.encode_channel_count, Some(4));
    }
}
