//! SIP 配置模块

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SipConfig {
    pub enabled: bool,
    pub ip: String,
    pub port: u16,
    /// SIP **TCP** 监听端口。
    ///
    /// 国标设备只配置"平台的 IP + 端口 + 传输方式"，用同一个端口选 TCP 或
    /// UDP。因此这个值**默认与 `port` 相同**（5060）—— 若单独放在 5061，
    /// 以 TCP 配置为 5060 的设备会连不上。
    pub tcp_port: u16,
    /// 是否启用 SIP TCP 监听。
    ///
    /// 默认**开启**：TCP 信令的解析/分帧/响应回程/出站发送都已实现，但
    /// `SipServer::tcp_enabled` 之前硬编码 false 且 `set_tcp_enabled` 从未被
    /// 调用 —— TCP 监听器**从未启动**，以 TCP 注册的设备完全无法接入
    /// （UDP 端口收不到它们的 REGISTER）。
    #[serde(default = "default_true")]
    pub tcp_enabled: bool,
    pub device_id: String,
    pub password: String,
    pub realm: String,
    pub keepalive_timeout: u64,
    pub register_timeout: u64,
    pub charset: String,
    pub sdp_ip: Option<String>,
    pub stream_ip: Option<String>,
    pub stream_reconnect: Option<crate::config::StreamReconnectConfig>,
    pub heartbeat: Option<crate::config::HeartbeatConfig>,
}

impl Default for SipConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ip: "0.0.0.0".to_string(),
            port: 5060,
            tcp_port: 5060,
            tcp_enabled: true,
            device_id: "34020000002000000001".to_string(),
            password: "admin123".to_string(),
            realm: "3402000000".to_string(),
            keepalive_timeout: 30,
            register_timeout: 3600,
            charset: "UTF-8".to_string(),
            sdp_ip: None,
            stream_ip: None,
            stream_reconnect: None,
            heartbeat: None,
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

fn default_true() -> bool {
    true
}
