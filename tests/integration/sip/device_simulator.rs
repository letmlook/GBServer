//! SIP GB28181 设备模拟器
//!
//! 用于测试 Rust GBServer 后端在无真实硬件环境下的 SIP 协议行为。
//!
//! 模拟能力：
//! - REGISTER / Keepalive / UNREGISTER 生命周期
//! - Catalog 响应（单包 + 多包）
//! - DeviceInfo / DeviceStatus 响应
//! - RecordInfo 响应（多包聚合）
//! - INVITE 200 OK + SDP + ACK + BYE
//! - MESSAGE 响应路由（测试 PendingRequest）

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::sleep;

use super::super::super::common::fixtures as fixtures;

/// GB28181 设备模拟器配置
#[derive(Debug, Clone)]
pub struct SimDeviceConfig {
    pub device_id: String,
    pub device_name: String,
    pub manufacturer: String,
    pub model: String,
    pub firmware: String,
    pub channel_count: i32,
    pub username: String,
    pub password: String,
    pub expires_secs: u32,
}

impl Default for SimDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: "34020000001320000001".to_string(),
            device_name: "SimCamera-01".to_string(),
            manufacturer: "GBServer-Test".to_string(),
            model: "SIM-100".to_string(),
            firmware: "1.0.0-test".to_string(),
            channel_count: 4,
            username: "admin".to_string(),
            password: "admin".to_string(),
            expires_secs: 3600,
        }
    }
}

/// 单个模拟设备的会话状态
#[derive(Debug)]
pub struct SimDeviceSession {
    pub config: SimDeviceConfig,
    pub registered: bool,
    pub keepalive_count: u32,
    pub catalog_sn: u32,
    pub registered_addr: Option<SocketAddr>,
    pub cseq: u32,
}

impl SimDeviceSession {
    pub fn new(config: SimDeviceConfig) -> Self {
        Self {
            config,
            registered: false,
            keepalive_count: 0,
            catalog_sn: 0,
            registered_addr: None,
            cseq: 0,
        }
    }

    pub fn next_cseq(&mut self) -> u32 {
        self.cseq += 1;
        self.cseq
    }
}

/// SIP GB28181 设备模拟器
///
/// 使用方式：
/// ```rust,ignore
/// let sim = SipDeviceSimulator::new(config);
/// sim.register().await?;       // 发起 REGISTER
/// sim.send_keepalive().await?; // 发送保活
/// let catalog = sim.query_catalog().await?; // 查询目录
/// ```
pub struct SipDeviceSimulator {
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    session: Arc<RwLock<SimDeviceSession>>,
    socket: Arc<RwLock<Option<UdpSocket>>>,
}

impl SipDeviceSimulator {
    /// 创建模拟器（绑定随机本地端口）
    pub async fn new(local_port: u16, server_addr: SocketAddr) -> std::io::Result<Self> {
        let local_addr = format!("127.0.0.1:{}", local_port);
        let socket = UdpSocket::bind(&local_addr).await?;
        socket.connect(server_addr).await?;
        Ok(Self {
            local_addr: socket.local_addr()?,
            server_addr,
            session: Arc::new(RwLock::new(SimDeviceSession::new(
                SimDeviceConfig::default(),
            ))),
            socket: Arc::new(RwLock::new(Some(socket))),
        })
    }

    /// 创建带自定义配置的模拟器
    pub async fn with_config(
        local_port: u16,
        server_addr: SocketAddr,
        config: SimDeviceConfig,
    ) -> std::io::Result<Self> {
        let local_addr = format!("127.0.0.1:{}", local_port);
        let socket = UdpSocket::bind(&local_addr).await?;
        socket.connect(server_addr).await?;
        Ok(Self {
            local_addr: socket.local_addr()?,
            server_addr,
            session: Arc::new(RwLock::new(SimDeviceSession::new(config))),
            socket: Arc::new(RwLock::new(Some(socket))),
        })
    }

    /// 发送 SIP REGISTER 请求
    pub async fn send_register(&self) -> std::io::Result<String> {
        let sess = self.session.read().await;
        let cseq = sess.next_cseq();
        let call_id = format!("sim-register-{}", cseq);
        drop(sess);

        let msg = format!(
            "REGISTER sip:{}@{}:{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP {};rport;branch=z9hG4bK{}\r\n\
            From: <sip:{}@{}:{}>;tag=from-tag-{}\r\n\
            To: <sip:{}@{}:{}>\r\n\
            Call-ID: {}\r\n\
            CSeq: {} REGISTER\r\n\
            Contact: <sip:{}@{}:{}>\r\n\
            Expires: {}\r\n\
            Authorization: Digest username=\"{}\", realm=\"GBServer\", nonce=\"\", uri=\"sip:{}@{}:{}\", response=\"\"\r\n\
            Content-Length: 0\r\n\r\n",
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.local_addr,
            cseq,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            call_id,
            cseq,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            cseq,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id,
        );

        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send(msg.as_bytes()).await?;
        }
        Ok(call_id)
    }

    /// 发送保活（Keepalive）
    pub async fn send_keepalive(&self) -> std::io::Result<String> {
        let mut sess = self.session.write().await;
        let cseq = sess.next_cseq();
        let sn = format!("{:010}", cseq);
        sess.keepalive_count += 1;
        let call_id = format!("sim-ka-{}", cseq);
        drop(sess);

        let msg = format!(
            "MESSAGE sip:{}@{}:{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP {};rport;branch=z9hG4bK{}\r\n\
            From: <sip:{}@{}:{}>;tag=from-tag-{}\r\n\
            To: <sip:{}@{}:{}>\r\n\
            Call-ID: {}\r\n\
            CSeq: {} MESSAGE\r\n\
            Content-Type: APPLICATION/MANSCDP+XML\r\n\
            Content-Length: {}\r\n\r\n\
            <?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
            <Notify>\r\n\
            <CmdType>Keepalive</CmdType>\r\n\
            <SN>{}</SN>\r\n\
            <DeviceID>{}</DeviceID>\r\n\
            <Status>OK</Status>\r\n\
            </Notify>",
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.local_addr,
            cseq,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id,
            call_id,
            cseq,
            sn.len(),
            sn,
            self.session.read().await.config.device_id,
        );

        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send(msg.as_bytes()).await?;
        }
        Ok(call_id)
    }

    /// 查询设备信息（DeviceInfo）
    pub async fn respond_to_device_info_query(&self, sn: &str, call_id: &str) -> std::io::Result<()> {
        let cseq = self.session.write().await.next_cseq();
        let resp_body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>DeviceInfo</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<DeviceName>{}</DeviceName>
<Manufacturer>{}</Manufacturer>
<Model>{}</Model>
<FirmwareVersion>{}</FirmwareVersion>
<Channel>{}</Channel>
</Response>"#,
            sn,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_name,
            self.session.read().await.config.manufacturer,
            self.session.read().await.config.model,
            self.session.read().await.config.firmware,
            self.session.read().await.config.channel_count,
        );

        let msg = format!(
            "MESSAGE sip:{}@{}:{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP {};rport;branch=z9hG4bK{}\r\n\
            From: <sip:{}@{}:{}>;tag=from-tag{}\r\n\
            To: <sip:{}@{}:{}>;tag=to-tag{}\r\n\
            Call-ID: {}\r\n\
            CSeq: {} MESSAGE\r\n\
            Content-Type: APPLICATION/MANSCDP+XML\r\n\
            Content-Length: {}\r\n\r\n{}",
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.local_addr,
            cseq,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            call_id,
            cseq,
            resp_body.len(),
            resp_body,
        );

        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send(msg.as_bytes()).await?;
        }
        Ok(())
    }

    /// 发送 Catalog 响应（测试多包聚合）
    pub async fn send_catalog(&self, sn: u32, channels: Vec<fixtures::SimChannel>, sum_num: i32, current_num: i32) -> std::io::Result<()> {
        let cseq = self.session.write().await.next_cseq();

        let mut channel_xml = String::new();
        for ch in &channels {
            channel_xml.push_str(&format!(
                "<Item>
                <DeviceID>{}</DeviceID>
                <Name>{}</Name>
                <Manufacturer>{}</Manufacturer>
                <Model>{}</Model>
                <Owner>{}</Owner>
                <CivilCode>{}</CivilCode>
                <Address>{}</Address>
                <Parental>{}</Parental>
                <ParentID>{}</ParentID>
                <SafetyWay>{}</SafetyWay>
                <RegisterWay>{}</RegisterWay>
                <CertNum>{}</CertNum>
                <Certifiable>{}</Certifiable>
                <ErrCode>{}</ErrCode>
                <PTZType>{}</PTZType>
                <Status>{}</Status>
                <Longitude>{}</Longitude>
                <Latitude>{}</Latitude>
                </Item>",
                ch.device_id, ch.name, ch.manufacturer, ch.model,
                ch.owner, ch.civil_code, ch.address,
                ch.parental, ch.parent_id,
                ch.safety_way, ch.register_way,
                ch.cert_num, ch.certifiable, ch.err_code,
                ch.ptz_type, ch.status,
            ));
        }

        let resp_body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<SumNum>{}</SumNum>
<Num>{}</Num>
<DeviceList Name="Record\" Num="{}">
{}
</DeviceList>
</Response>"#,
            sn,
            self.session.read().await.config.device_id,
            sum_num,
            current_num,
            channels.len(),
            channel_xml,
        );

        let msg = format!(
            "MESSAGE sip:{}@{}:{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP {};rport;branch=z9hG4bK{}\r\n\
            From: <sip:{}@{}:{}>;tag=cat-from{}\r\n\
            To: <sip:{}@{}:{}>;tag=cat-to{}\r\n\
            Call-ID: sim-cat-{}\r\n\
            CSeq: {} MESSAGE\r\n\
            Content-Type: APPLICATION/MANSCDP+XML\r\n\
            Content-Length: {}\r\n\r\n{}",
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.local_addr,
            cseq,
            self.session.read().await.config.device_id,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            self.session.read().await.config.device_id.split_at(16).0,
            cseq,
            cseq,
            resp_body.len(),
            resp_body,
        );

        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send(msg.as_bytes()).await?;
        }
        Ok(())
    }

    /// 接收并解析 SIP 响应（用于验证 InviteSessionStore 状态）
    pub async fn recv_response(&self, timeout_secs: u64) -> std::io::Result<Option<String>> {
        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            let mut buf = [0u8; 4096];
            tokio::time::timeout(Duration::from_secs(timeout_secs as u64), s.recv(&mut buf)).await??;
            Ok(Some(String::from_utf8_lossy(&buf).to_string()))
        } else {
            Ok(None)
        }
    }

    /// 关闭模拟器
    pub async fn shutdown(&self) {
        let mut socket = self.socket.write().await;
        *socket = None;
    }
}

