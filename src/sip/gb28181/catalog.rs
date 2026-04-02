//! 目录查询处理

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogQuery {
    pub cmd_type: String,
    pub sn: u32,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogResponse {
    pub cmd_type: String,
    pub sn: u32,
    pub device_id: String,
    pub sum_num: u32,
    pub channel_list: Vec<ChannelItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelItem {
    pub device_id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub owner: Option<String>,
    pub civil_code: Option<String>,
    pub address: Option<String>,
    pub parental: Option<u32>,
    pub safety_way: Option<u32>,
    pub register_way: Option<u32>,
    pub cert_num: Option<String>,
    pub cert_type: Option<u32>,
    pub ip_address: Option<String>,
    pub port: Option<u32>,
    pub password: Option<String>,
    pub status: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
}

impl Default for CatalogResponse {
    fn default() -> Self {
        Self {
            cmd_type: "Catalog".to_string(),
            sn: 0,
            device_id: String::new(),
            sum_num: 0,
            channel_list: Vec::new(),
        }
    }
}
