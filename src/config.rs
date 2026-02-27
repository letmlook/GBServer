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

pub fn load_config() -> Result<AppConfig> {
    let base = config::Config::builder()
        .add_source(config::File::with_name("config/application").required(false))
        .add_source(config::Environment::with_prefix("WVP").separator("__"));

    let cfg: AppConfig = base.build()?.try_deserialize()?;
    Ok(cfg)
}
