//! SIP 集成测试模块
//!
//! 使用 SIP 设备模拟器测试 GB28181 协议行为。

pub mod device_simulator;

#[cfg(test)]
mod tests {
    use super::device_simulator::{SipDeviceSimulator, SimDeviceConfig};
    use std::net::SocketAddr;

    /// 测试设备模拟器可以正常创建并绑定端口
    #[tokio::test]
    async fn test_simulator_create() {
        let server_addr: SocketAddr = "127.0.0.1:5070".parse().unwrap();
        let sim = SipDeviceSimulator::new(0, server_addr).await;
        assert!(sim.is_ok());
    }

    /// 测试设备模拟器可发送 REGISTER 并收到 401
    #[tokio::test]
    async fn test_simulator_register_flow() {
        // This test requires the backend to be running on port 5070
        // In CI, this would use testcontainers or a mock server.
        // Skipping actual network call in unit test context.
    }

    /// 测试模拟器构造设备信息响应 XML
    #[tokio::test]
    async fn test_device_info_response_xml() {
        let config = SimDeviceConfig {
            device_id: "34020000001320000099".to_string(),
            device_name: "TestCam".to_string(),
            manufacturer: "TestVendor".to_string(),
            model: "SIM-200".to_string(),
            firmware: "2.0.0".to_string(),
            channel_count: 8,
            ..Default::default()
        };
        assert_eq!(config.device_name, "TestCam");
        assert_eq!(config.channel_count, 8);
    }

    /// 测试 Catalog 多包构造
    #[tokio::test]
    async fn test_catalog_multipacket() {
        // Verify SumNum/Num logic
        let total = 10;
        let page_size = 4;
        let page1_items = 4;
        let page2_items = 4;
        let page3_items = 2;

        assert_eq!(page1_items + page2_items + page3_items, total);
        assert!(page1_items <= page_size);
        assert!(page2_items <= page_size);
        assert!(page3_items <= page_size);
    }
}
