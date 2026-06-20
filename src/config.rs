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
    /// Phase 7.2: cluster / node-discovery config. When `single_node_mode = true`,
    /// cluster checks are skipped (Redis fallback to local node only).
    #[serde(default)]
    pub cluster: ClusterAppConfig,
    /// Phase 7.4: audit middleware config. When disabled, no audit log is written.
    #[serde(default)]
    pub audit: AuditConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Jt1078Config {
    /// Session inactivity timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Retransmit wait window in milliseconds
    pub retransmit_wait_ms: Option<u64>,
    /// Optional HTTP hook URL to notify about missing sequences
    pub retransmit_hook_url: Option<String>,
    /// TCP listen port for JT1078 (default 60000)
    pub tcp_port: Option<u16>,
    /// UDP listen port for JT1078 (default 60000)
    pub udp_port: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    /// SQLite 模式下允许的最大设备数；超过则拒绝新设备注册。
    /// 默认 500；超出需迁移到 PostgreSQL/MySQL。
    /// PG/MySQL 后端忽略此字段。
    #[serde(default)]
    pub sqlite_max_devices: Option<usize>,
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

/// ZLM 节点健康检查配置（Phase 4 follow-up）
///
/// 控制 `media_node::health_check_loop` 的行为：
/// - `timeout_secs`：单次 keepalive 超时阈值（默认 30s）
/// - `grace_count`：连续 N 次健康检查都判定超时才真正切 offline（默认 3）
/// - `check_interval_secs`：loop 间隔（默认 10s）
#[derive(Debug, Clone, Deserialize)]
pub struct ZlmKeepaliveConfig {
    #[serde(default = "default_keepalive_timeout_secs")]
    pub timeout_secs: i64,
    #[serde(default = "default_keepalive_grace_count")]
    pub grace_count: i32,
    #[serde(default = "default_keepalive_check_interval_secs")]
    pub check_interval_secs: u64,
}

fn default_keepalive_timeout_secs() -> i64 { 30 }
fn default_keepalive_grace_count() -> i32 { 3 }
fn default_keepalive_check_interval_secs() -> u64 { 10 }

impl Default for ZlmKeepaliveConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_keepalive_timeout_secs(),
            grace_count: default_keepalive_grace_count(),
            check_interval_secs: default_keepalive_check_interval_secs(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZlmConfig {
    pub servers: Vec<ZlmServerConfig>,
    pub stream_timeout: u64,
    pub hook_enabled: bool,
    pub hook_url: String,
    /// Phase 4 follow-up: 节点健康检查配置（可从 [zlm.keepalive] 覆盖）
    #[serde(default)]
    pub keepalive: ZlmKeepaliveConfig,
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
            keepalive: ZlmKeepaliveConfig::default(),
        }
    }
}

pub fn load_config() -> Result<AppConfig> {
    let base = config::Config::builder()
        .add_source(config::File::with_name("config/application").required(false))
        .add_source(config::Environment::with_prefix("GBSERVER").separator("__"));

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

/// Phase 7.2: 集群/节点发现配置（来自 [cluster] 段落）。
#[derive(Debug, Clone, Deserialize)]
pub struct ClusterAppConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_cluster_single_node")]
    pub single_node_mode: bool,
    #[serde(default)]
    pub node_id: String,
    #[serde(default)]
    pub addr: String,
    #[serde(default = "default_cluster_role")]
    pub role: String,
    #[serde(default = "default_cluster_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_cluster_heartbeat_ttl")]
    pub heartbeat_ttl_secs: u64,
}

fn default_cluster_single_node() -> bool { true }
fn default_cluster_role() -> String { "primary".to_string() }
fn default_cluster_heartbeat_interval() -> u64 { 10 }
fn default_cluster_heartbeat_ttl() -> u64 { 60 }

impl Default for ClusterAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            single_node_mode: true,
            node_id: format!("node-{}", std::process::id()),
            addr: "http://127.0.0.1:18080".to_string(),
            role: "primary".to_string(),
            heartbeat_interval_secs: 10,
            heartbeat_ttl_secs: 60,
        }
    }
}

impl ClusterAppConfig {
    pub fn heartbeat_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.heartbeat_interval_secs)
    }
    pub fn heartbeat_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.heartbeat_ttl_secs)
    }
}

/// Phase 7.4: audit middleware configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    #[serde(default = "default_audit_enabled")]
    pub enabled: bool,
    #[serde(default = "default_audit_retention_days")]
    pub retention_days: u32,
}

fn default_audit_enabled() -> bool { true }
fn default_audit_retention_days() -> u32 { 90 }

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 90,
        }
    }
}
