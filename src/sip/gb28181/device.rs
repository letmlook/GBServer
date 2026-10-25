//! 设备注册/注销管理

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

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
        self.devices
            .read()
            .await
            .get(device_id)
            .and_then(|d| d.addr)
    }

    pub async fn set_online(&self, device_id: &str, online: bool) {
        let mut guard = self.devices.write().await;
        if let Some(d) = guard.get_mut(device_id) {
            d.online = online;
            d.keepalive_time = Utc::now();
        }
    }

    /// 记录一次 Keepalive：**只刷新存活状态，不改写信令地址**。
    ///
    /// ## 为什么不能在这里写地址
    ///
    /// 信令地址是平台**全部出站信令**的目标（19 处 `get_address()` 调用点：
    /// 点播/回放/下载的 INVITE、PTZ、目录查询、SUBSCRIBE、BYE……）。
    ///
    /// 写地址的**唯一合法入口**是 `handle_register` —— 那条路径带 digest 摘要
    /// 鉴权（401 挑战 → 校验 → 403 拒绝），并且写的就是 REGISTER 的**源地址**。
    ///
    /// 此前本函数把 Keepalive 的来源地址也写了进去。而 Keepalive 走的是
    /// **MESSAGE** —— GB28181 对 MESSAGE 没有鉴权要求，平台这条路径上**没有任何
    /// 校验**，而设备号还取自 XML 正文。于是任何能连到 SIP 端口的人发一条
    /// MESSAGE，就能把**任意已注册设备**的信令地址改到自己名下：平台的出站
    /// 信令随之全部改发给攻击者（既劫持了设备，又把平台变成一个可探测任意
    /// host:port 的出站跳板）。
    ///
    /// 现在地址只由 REGISTER 决定；Keepalive 只刷新 `keepalive_time` / `online`。
    pub async fn update_keepalive(&self, device_id: &str) {
        let mut guard = self.devices.write().await;
        if let Some(d) = guard.get_mut(device_id) {
            d.keepalive_time = Utc::now();
            d.online = true;
        }
    }

    pub async fn list_online(&self) -> Vec<Device> {
        self.devices
            .read()
            .await
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
            if !d.online {
                return true;
            }
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

/// **Keepalive 的来源校验闸门**：来源地址是否与该设备**已登记**的地址一致。
///
/// ## 为什么需要它
///
/// Keepalive 走 SIP 的 MESSAGE，**没有任何鉴权**（GB28181 对 MESSAGE 不做摘要
/// 认证，平台这条路径上也没有），而设备号取自 XML 正文 —— 正文里的
/// `<DeviceID>` 可以任意填写。
///
/// 于是修复前任何能连到 SIP 端口的人，发一条 MESSAGE 就能把**任意已注册设备**
/// 的信令地址改到自己名下；而信令地址正是平台**全部出站信令**的目标
/// （19 处 `get_address()` 调用点：点播/回放/下载 INVITE、PTZ、目录查询、
/// SUBSCRIBE、BYE……）—— 后果是设备被劫持，且平台变成可探测任意 host:port
/// 的出站跳板。
///
/// ## 判据为什么用 IP 而不是 IP:port
///
/// - **IP** 是网络层事实，UDP 上伪造不了（SIP 头 From/Call-ID 可以随意伪造，
///   所以不能拿它们当判据）；
/// - 但**端口**可能被 NAT 改写 —— 同一台设备的心跳换一个源端口是很常见的，
///   端口一变就拒收会把正常设备误判离线。因此只比 IP：IP 对不上就绝不可能是
///   同一台设备，IP 对上则允许刷新存活状态（但**不写地址**）。
///
/// 纯函数，便于单测 —— 这是安全闸门，必须有测试守着。
pub fn keepalive_source_allowed(source: &SocketAddr, registered_ip: Option<&str>) -> bool {
    match registered_ip {
        Some(ip) if !ip.trim().is_empty() => source.ip().to_string() == ip.trim(),
        // 查不到已登记地址 ⇒ 无法确认来源身份 ⇒ 不认（宁可不心跳，不可错认）
        _ => false,
    }
}

#[cfg(test)]
mod keepalive_gate_tests {
    use super::*;

    /// **安全回归**：陌生来源 IP 的 Keepalive 必须被拒 —— 否则它能把已注册
    /// 设备的信令地址改到自己名下（设备劫持 + 平台变成出站跳板）。
    #[test]
    fn rejects_foreign_source_ip() {
        let source: SocketAddr = "10.0.0.9:5060".parse().unwrap();
        // 被劫持目标设备登记在 192.168.1.7
        assert!(
            !keepalive_source_allowed(&source, Some("192.168.1.7")),
            "陌生来源 IP 必须被拒"
        );
    }

    /// 同一来源 IP（哪怕端口被 NAT 改过）必须放行，否则正常设备会被误判离线。
    #[test]
    fn accepts_same_ip_with_different_port() {
        let source: SocketAddr = "192.168.1.7:45000".parse().unwrap();
        assert!(
            keepalive_source_allowed(&source, Some("192.168.1.7")),
            "同 IP 不同端口应放行（NAT 端口漂移）"
        );
    }

    /// **安全回归**：Keepalive 刷新**不得**改写内存里的信令地址。
    ///
    /// 内存地址是平台**全部出站信令**的目标（19 处 `get_address()` 调用点）。
    /// 修复前 `update_keepalive` 会把来源地址写进 `ip`/`port`/`addr` ——
    /// 而 Keepalive（MESSAGE）**不带鉴权**，设备号还取自 XML 正文，于是任何人
    /// 都能把已注册设备的信令地址改到自己名下（设备劫持 + 平台变成出站跳板）。
    #[tokio::test]
    async fn keepalive_refresh_preserves_registered_address() {
        let mgr = DeviceManager::new();
        let registered: SocketAddr = "192.168.1.7:5060".parse().unwrap();
        mgr.register("34020000001320000001", registered).await;

        // 心跳刷新（注意：修复后这个函数**不再接收**来源地址）
        mgr.update_keepalive("34020000001320000001").await;

        assert_eq!(
            mgr.get_address("34020000001320000001").await,
            Some(registered),
            "Keepalive 不得改写已登记的信令地址"
        );
        let dev = mgr
            .list_all()
            .await
            .into_iter()
            .find(|d| d.device_id == "34020000001320000001")
            .expect("device must be present");
        assert_eq!(dev.ip.as_deref(), Some("192.168.1.7"));
        assert_eq!(dev.port, Some(5060));
        assert!(dev.online, "心跳应刷新在线状态");
    }

    /// 设备从未登记（查不到地址）⇒ 无法确认身份 ⇒ 不认。
    #[test]
    fn rejects_when_no_registered_address() {
        let source: SocketAddr = "10.0.0.9:5060".parse().unwrap();
        assert!(!keepalive_source_allowed(&source, None));
        assert!(!keepalive_source_allowed(&source, Some("")));
        assert!(!keepalive_source_allowed(&source, Some("   ")));
    }
}
