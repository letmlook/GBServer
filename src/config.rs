use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: Option<RedisConfig>,
    pub jwt: JwtConfig,
    pub static_dir: Option<String>,
    pub user_settings: Option<UserSettings>,
    pub sip: Option<SipConfig>,
    pub zlm: Option<ZlmConfig>,
    pub map: Option<MapConfig>,
    pub jt1078: Option<Jt1078Config>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Jt1078Config {
    /// Session inactivity timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Retransmit wait window in milliseconds
    pub retransmit_wait_ms: Option<u64>,
    /// Optional HTTP hook URL to notify about missing sequences
    pub retransmit_hook_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_minutes: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserSettings {
    pub server_id: Option<String>,
}

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
    pub sdp_ip: Option<String>,
    pub stream_ip: Option<String>,
    pub stream_reconnect: Option<StreamReconnectConfig>,
    pub heartbeat: Option<HeartbeatConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamReconnectConfig {
    pub enabled: bool,
    pub max_retries: u32,
    pub retry_interval_secs: u64,
}

impl Default for StreamReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            retry_interval_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeartbeatConfig {
    pub timeout_multiplier: u32,
    pub check_interval_secs: u64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            timeout_multiplier: 3,
            check_interval_secs: 10,
        }
    }
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
    pub hook_url: Option<String>,
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
                hook_url: None,
            }],
            stream_timeout: 10,
            hook_enabled: true,
            hook_url: "http://127.0.0.1:18080/api/zlm/hook".to_string(),
        }
    }
}

pub fn load_config() -> Result<AppConfig> {
    let base = config::Config::builder()
        .add_source(config::File::with_name("config/application").required(false))
        .add_source(config::Environment::with_prefix("WVP").separator("__"));

    let cfg: AppConfig = base.build()?.try_deserialize()?;
    Ok(cfg)
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapConfig {
    /// 天地图 API Key
    pub tianditu_key: Option<String>,
    /// 地图中心经度
    pub center_lng: Option<f64>,
    /// 地图中心纬度
    pub center_lat: Option<f64>,
    /// 默认缩放级别
    pub zoom: Option<i32>,
    /// 坐标系: WGS84 或 GCJ02
    pub coord_sys: Option<String>,
}
