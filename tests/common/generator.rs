// 后端测试数据生成器
// 本模块仅生成后端测试数据，不生成前端测试数据

use fake::{Fake, Faker};
use fake::faker::*;
use chrono::Utc;

/// 后端测试数据生成器
pub struct TestDataGenerator;

impl TestDataGenerator {
    /// 生成设备测试数据
    pub fn generate_device() -> serde_json::Value {
        serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "device_id": format!("34020000001320{}", (0..100000000).fake::<u32>()),
            "name": fake::faker::name::Name.fake::<String>(),
            "ip": fake::faker::internet::IPv4.fake::<String>(),
            "port": (1..65535).fake::<u16>(),
            "status": "Online",
            "create_time": Utc::now().to_rfc3339()
        })
    }

    /// 生成通道测试数据
    pub fn generate_channel(device_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "device_id": device_id,
            "channel_id": format!("34020000001320{}", (0..100000000).fake::<u32>()),
            "name": fake::faker::name::Name.fake::<String>(),
            "status": "Online",
            "create_time": Utc::now().to_rfc3339()
        })
    }

    /// 生成用户测试数据
    pub fn generate_user() -> serde_json::Value {
        serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "username": fake::faker::internet::Username.fake::<String>(),
            "password": fake::faker::internet::Password(8..16).fake::<String>(),
            "role": "user",
            "create_time": Utc::now().to_rfc3339()
        })
    }

    /// 批量生成设备测试数据
    pub fn generate_devices(count: usize) -> Vec<serde_json::Value> {
        (0..count).map(|_| Self::generate_device()).collect()
    }

    /// 批量生成通道测试数据
    pub fn generate_channels(device_id: &str, count: usize) -> Vec<serde_json::Value> {
        (0..count).map(|_| Self::generate_channel(device_id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_device() {
        let device = TestDataGenerator::generate_device();
        assert!(device["device_id"].as_str().unwrap().starts_with("34020000001320"));
        assert_eq!(device["status"], "Online");
    }

    #[test]
    fn test_generate_channel() {
        let device_id = "34020000001320000001";
        let channel = TestDataGenerator::generate_channel(device_id);
        assert_eq!(channel["device_id"], device_id);
    }

    #[test]
    fn test_generate_user() {
        let user = TestDataGenerator::generate_user();
        assert!(user["username"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn test_generate_devices() {
        let devices = TestDataGenerator::generate_devices(5);
        assert_eq!(devices.len(), 5);
    }
}
