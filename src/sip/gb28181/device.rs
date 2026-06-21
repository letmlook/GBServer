//! 设备注册/注销管理

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Device {
    pub device_id: String,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub transport: TransportMode,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub online: bool,
    pub register_time: DateTime<Utc>,
    pub keepalive_time: DateTime<Utc>,
    pub expires: u64,
    pub username: Option<String>,
    pub password: Option<String>,
    pub custom_name: Option<String>,
    pub addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransportMode {
    UDP,
    TCP,
}

pub struct DeviceManager {
    devices: Arc<RwLock<HashMap<String, Device>>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, device_id: &str, addr: SocketAddr) {
        let mut guard = self.devices.write().await;
        let now = Utc::now();
        let device = Device {
            device_id: device_id.to_string(),
            name: None,
            manufacturer: None,
            model: None,
            firmware: None,
            transport: TransportMode::UDP,
            ip: Some(addr.ip().to_string()),
            port: Some(addr.port()),
            online: true,
            register_time: now,
            keepalive_time: now,
            expires: 3600,
            username: None,
            password: None,
            custom_name: None,
            addr: Some(addr),
        };
        guard.insert(device_id.to_string(), device);
    }

    pub async fn unregister(&self, device_id: &str) {
        let mut guard = self.devices.write().await;
        if let Some(d) = guard.get_mut(device_id) {
            d.online = false;
        }
    }

    pub async fn get(&self, device_id: &str) -> Option<Device> {
        self.devices.read().await.get(device_id).cloned()
    }

    /// 从 DB 恢复 online 设备到内存(用于服务重启后保持设备可达)。
    /// 优先用 DB 里的 ip/port 构造 addr,缺失时退化为 None。
    pub async fn restore(&self, device: &crate::db::device::Device) {
        let addr = match (device.ip.as_deref(), device.port) {
            (Some(ip), Some(port)) => format!("{}:{}", ip, port).parse().ok(),
            _ => None,
        };
        let dev = Device {
            device_id: device.device_id.clone(),
            name: device.name.clone(),
            manufacturer: device.manufacturer.clone(),
            model: device.model.clone(),
            firmware: device.firmware.clone(),
            transport: if device.transport.as_deref().unwrap_or("UDP").to_uppercase() == "TCP" {
                TransportMode::TCP
            } else {
                TransportMode::UDP
            },
            ip: device.ip.clone(),
            port: device.port.map(|p| p as u16),
            online: true,
            register_time: Utc::now(),
            keepalive_time: Utc::now(),
            expires: 3600,
            username: None,
            password: None,
            custom_name: device.custom_name.clone(),
            addr,
        };
        self.devices
            .write()
            .await
            .insert(device.device_id.clone(), dev);
    }

    pub async fn get_address(&self, device_id: &str) -> Option<SocketAddr> {
        self.devices.read().await.get(device_id).and_then(|d| d.addr)
    }

    pub async fn set_online(&self, device_id: &str, online: bool) {
        let mut guard = self.devices.write().await;
        if let Some(d) = guard.get_mut(device_id) {
            d.online = online;
            d.keepalive_time = Utc::now();
        }
    }

    pub async fn update_keepalive(&self, device_id: &str, addr: SocketAddr) {
        let mut guard = self.devices.write().await;
        if let Some(d) = guard.get_mut(device_id) {
            d.keepalive_time = Utc::now();
            d.online = true;
            d.ip = Some(addr.ip().to_string());
            d.port = Some(addr.port());
            d.addr = Some(addr);
        }
    }

    pub async fn list_online(&self) -> Vec<Device> {
        self.devices.read().await
            .values()
            .filter(|d| d.online)
            .cloned()
            .collect()
    }

    pub async fn list_all(&self) -> Vec<Device> {
        self.devices.read().await.values().cloned().collect()
    }

    pub async fn cleanup_expired(&self, timeout_secs: i64) {
        let now = Utc::now();
        let mut guard = self.devices.write().await;
        guard.retain(|_, d| {
            if !d.online { return true; }
            let elapsed = (now - d.keepalive_time).num_seconds();
            elapsed < timeout_secs as i64
        });
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}
