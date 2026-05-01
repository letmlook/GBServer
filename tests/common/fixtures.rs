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
