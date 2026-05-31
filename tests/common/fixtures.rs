// 后端测试数据加载器
// 本模块仅加载后端测试数据，不加载前端测试数据

use std::path::Path;
use serde::de::DeserializeOwned;
use serde_json;

/// 后端测试数据加载器
pub struct FixtureLoader;

impl FixtureLoader {
    /// 加载JSON格式的测试数据
    pub fn load<T: DeserializeOwned>(name: &str) -> T {
        let path = Path::new("tests/fixtures").join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name));
        serde_json::from_str(&content)
            .unwrap_or_else(|_| panic!("Failed to parse fixture: {}", name))
    }

    /// 加载设备测试数据
    pub fn load_device(name: &str) -> serde_json::Value {
        Self::load(&format!("devices/{}.json", name))
    }

    /// 加载用户测试数据
    pub fn load_user(name: &str) -> serde_json::Value {
        Self::load(&format!("users/{}.json", name))
    }

    /// 加载平台测试数据
    pub fn load_platform(name: &str) -> serde_json::Value {
        Self::load(&format!("platforms/{}.json", name))
    }

    /// 加载SIP消息测试数据
    pub fn load_sip_message(name: &str) -> String {
        let path = Path::new("tests/fixtures/sip").join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to load SIP message: {}", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_device() {
        let device = FixtureLoader::load_device("device_001");
        assert_eq!(device["device_id"], "34020000001320000001");
    }

    #[test]
    fn test_load_user() {
        let user = FixtureLoader::load_user("admin");
        assert_eq!(user["username"], "admin");
    }
}

// ============================================================================
// SIP 模拟测试数据
// ============================================================================

/// 模拟通道数据（用于 Catalog 测试）
#[derive(Debug, Clone)]
pub struct SimChannel {
    pub device_id: String,
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub owner: String,
    pub civil_code: String,
    pub address: String,
    pub parental: i32,
    pub parent_id: String,
    pub safety_way: i32,
    pub register_way: i32,
    pub cert_num: String,
    pub certifiable: i32,
    pub err_code: i32,
    pub ptz_type: i32,
    pub status: String,
}

impl SimChannel {
    /// 创建通道数组（用于多包 Catalog 测试）
    pub fn channel_set(count: usize, device_id_base: &str) -> Vec<SimChannel> {
        (0..count)
            .map(|i| {
                let ch_id = format!("{}{:04}", device_id_base, i + 1);
                SimChannel {
                    device_id: ch_id,
                    name: format!("Camera-{:04}", i + 1),
                    manufacturer: "TestVendor".to_string(),
                    model: "IPC-100".to_string(),
                    owner: "Admin".to_string(),
                    civil_code: "110000".to_string(),
                    address: format!("Address-{:04}", i + 1),
                    parental: 0,
                    parent_id: device_id_base.to_string(),
                    safety_way: 0,
                    register_way: 1,
                    cert_num: format!("CERT{:06}", i + 1),
                    certifiable: 0,
                    err_code: 0,
                    ptz_type: 2,
                    status: "ON".to_string(),
                }
            })
            .collect()
    }
}
