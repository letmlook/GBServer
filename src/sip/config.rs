//! SIP 配置模块

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SipConfig {
    pub enabled: bool,
    pub ip: String,
    pub port: u16,
    pub tcp_port: u16,
    pub device_id: String,
    pub password: String,
    pub realm: String,
    pub keepalive_timeout: u64,
    pub register_timeout: u64,
    pub charset: String,
}

impl Default for SipConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ip: "0.0.0.0".to_string(),
            port: 5060,
            tcp_port: 5061,
            device_id: "34020000002000000001".to_string(),
            password: "admin123".to_string(),
            realm: "3402000000".to_string(),
            keepalive_timeout: 30,
            register_timeout: 3600,
            charset: "UTF-8".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZlmServerConfig {
    pub id: String,
    pub ip: String,
    pub http_port: u16,
    pub https_port: Option<u16>,
    pub secret: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZlmConfig {
    pub servers: Vec<ZlmServerConfig>,
    pub stream_timeout: u64,
    pub hook_enabled: bool,
    pub hook_url: String,
}

impl Default for ZlmConfig {
    fn default() -> Self {
        Self {
            servers: vec![ZlmServerConfig {
                id: "zlmediakit-1".to_string(),
                ip: "127.0.0.1".to_string(),
                http_port: 8080,
                https_port: None,
                secret: "035c73f7-bb6b-4889-a715-d9eb2d1925cc".to_string(),
                enabled: true,
            }],
            stream_timeout: 10,
            hook_enabled: true,
            hook_url: "http://127.0.0.1:18080/api/zlm/hook".to_string(),
        }
    }
}
