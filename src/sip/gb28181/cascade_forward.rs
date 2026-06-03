// ! CascadeForward — 级联订阅转发与 SendRtp 会话管理
//!
//! 功能：
//! 1. Catalog 转发（向上级 NOTIFY 已共享通道）
//! 2. MobilePosition 转发（订阅位置上报到上级）
//! 3. Alarm 转发（告警上报到上级）
//! 4. SendRtp 会话管理（设备拉流→推送到上级）

use std::collections::HashMap;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// SendRtp 会话（设备推流到本级 → SendRtp 到上级）
#[derive(Debug, Clone)]
pub struct SendRtpSession {
    /// 级联会话 ID
    pub cascade_call_id: String,
    /// 上级平台 ID
    pub platform_id: String,
    /// 通道 ID
    pub channel_id: String,
    /// 上级媒体地址
    pub upstream_host: String,
    /// 上级媒体端口
    pub upstream_port: u16,
    /// 上级 SSRC
    pub upstream_ssrc: String,
    /// 活跃状态
    pub active: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl SendRtpSession {
    pub fn new(
        cascade_call_id: String,
        platform_id: String,
        channel_id: String,
        upstream_host: String,
        upstream_port: u16,
        upstream_ssrc: String,
    ) -> Self {
        Self {
            cascade_call_id,
            platform_id,
            channel_id,
            upstream_host,
            upstream_port,
            upstream_ssrc,
            active: true,
            created_at: Utc::now(),
        }
    }
}

/// SendRtp 会话管理器
pub struct SendRtpManager {
    /// 按 cascade_call_id 索引
    sessions: Arc<DashMap<String, SendRtpSession>>,
}

impl SendRtpManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// 创建 SendRtp 会话
    pub fn create(&self, session: SendRtpSession) {
        self.sessions.insert(session.cascade_call_id.clone(), session);
    }

    /// 获取会话
    pub fn get(&self, cascade_call_id: &str) -> Option<SendRtpSession> {
        self.sessions.get(cascade_call_id).map(|r| r.clone())
    }

    /// 关闭会话
    pub fn close(&self, cascade_call_id: &str) -> Option<SendRtpSession> {
        if let Some(mut s) = self.sessions.get_mut(cascade_call_id) {
            s.active = false;
        }
        self.sessions.remove(cascade_call_id).map(|(_, v)| v)
    }

    /// 按通道关闭所有会话
    pub fn close_by_channel(&self, channel_id: &str) -> Vec<SendRtpSession> {
        let snap: Vec<_> = self.sessions
            .iter()
            .filter(|r| r.channel_id == channel_id && r.active)
            .map(|r| r.clone())
            .collect();

        let mut removed = Vec::new();
        for s in &snap {
            self.close(&s.cascade_call_id);
            removed.push(s.clone());
        }
        removed
    }

    /// 获取活跃会话数
    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|r| r.active).count()
    }

    /// 获取通道相关的所有活跃会话
    pub fn get_by_channel(&self, channel_id: &str) -> Vec<SendRtpSession> {
        self.sessions
            .iter()
            .filter(|r| r.channel_id == channel_id && r.active)
            .map(|r| r.clone())
            .collect()
    }
}

impl Default for SendRtpManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 级联转发器
// ---------------------------------------------------------------------------

/// 级联转发配置
#[derive(Debug, Clone)]
pub struct CascadeForwardConfig {
    /// 是否转发目录
    pub forward_catalog: bool,
    /// 是否转发移动位置
    pub forward_position: bool,
    /// 是否转发告警
    pub forward_alarm: bool,
    /// 上级媒体流推送地址
    pub send_rtp_enabled: bool,
}

impl Default for CascadeForwardConfig {
    fn default() -> Self {
        Self {
            forward_catalog: true,
            forward_position: true,
            forward_alarm: true,
            send_rtp_enabled: true,
        }
    }
}

/// 级联转发器（将本级事件转发到上级平台）
pub struct CascadeForwarder {
    config: CascadeForwardConfig,
    /// 上级平台地址
    upstream_addr: Option<std::net::SocketAddr>,
    /// SIP Socket
    socket: Arc<RwLock<Option<tokio::net::UdpSocket>>>,
    /// 本级 GB ID
    local_id: String,
}

impl CascadeForwarder {
    pub fn new(local_id: String) -> Self {
        Self {
            config: CascadeForwardConfig::default(),
            upstream_addr: None,
            socket: Arc::new(RwLock::new(None)),
            local_id,
        }
    }

    pub fn set_upstream(&mut self, host: String, port: u16) {
        self.upstream_addr = format!("{}:{}", host, port)
            .parse()
            .ok();
    }

    /// 转发 Catalog NOTIFY 到上级平台
    pub async fn forward_catalog(&self, channels: &[CascadeChannelInfo]) -> Result<(), String> {
        if !self.config.forward_catalog {
            return Ok(());
        }
        let addr = self.upstream_addr.ok_or("Upstream not configured")?;
        let sn = chrono::Utc::now().timestamp() % 10000;

        let mut items = String::new();
        for ch in channels {
            items.push_str(&format!(
                r#"<Item><DeviceID>{}</DeviceID><Name>{}</Name><Status>{}</Status></Item>"#,
                ch.device_id, ch.name, ch.status
            ));
        }

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<SumNum>{}</SumNum>
<DeviceList Num="{}">
{}
</DeviceList>
</Notify>"#,
            sn,
            self.local_id,
            channels.len(),
            channels.len(),
            items
        );

        let msg = self.build_notify_msg("Catalog", &body, sn)?;
        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send_to(msg.as_bytes(), addr).await
                .map_err(|e| format!("Forward Catalog failed: {}", e))?;
        }
        tracing::debug!("Forwarded Catalog to upstream: {} channels", channels.len());
        Ok(())
    }

    /// 转发移动位置到上级平台
    pub async fn forward_position(&self, position: &MobilePositionInfo) -> Result<(), String> {
        if !self.config.forward_position {
            return Ok(());
        }
        let addr = self.upstream_addr.ok_or("Upstream not configured")?;
        let sn = chrono::Utc::now().timestamp() % 10000;

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>MobilePosition</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<MobilePosition>
<DeviceID>{}</DeviceID>
<Latitude>{}</Latitude>
<Longitude>{}</Longitude>
<Speed>{}</Speed>
<Direction>{}</Direction>
<Time>{}</Time>
</MobilePosition>
</Notify>"#,
            sn,
            position.device_id,
            position.device_id,
            position.latitude,
            position.longitude,
            position.speed.unwrap_or(0.0),
            position.direction.unwrap_or(0),
            position.time
        );

        let msg = self.build_notify_msg("MobilePosition", &body, sn)?;
        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send_to(msg.as_bytes(), addr).await
                .map_err(|e| format!("Forward Position failed: {}", e))?;
        }
        Ok(())
    }

    /// 转发告警到上级平台
    pub async fn forward_alarm(&self, alarm: &AlarmInfo) -> Result<(), String> {
        if !self.config.forward_alarm {
            return Ok(());
        }
        let addr = self.upstream_addr.ok_or("Upstream not configured")?;
        let sn = chrono::Utc::now().timestamp() % 10000;

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>Alarm</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<AlarmType>{}</AlarmType>
<AlarmPriority>{}</AlarmPriority>
<AlarmTime>{}</AlarmTime>
</Notify>"#,
            sn,
            alarm.device_id,
            alarm.alarm_type,
            alarm.priority.unwrap_or(0),
            alarm.time
        );

        let msg = self.build_notify_msg("Alarm", &body, sn)?;
        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send_to(msg.as_bytes(), addr).await
                .map_err(|e| format!("Forward Alarm failed: {}", e))?;
        }
        Ok(())
    }

    fn build_notify_msg(&self, cmd_type: &str, body: &str, sn: i64) -> Result<String, String> {
        let addr = self.upstream_addr.ok_or("Upstream not configured")?;
        let call_id = format!("fwd_{}_{}", cmd_type.to_lowercase(), sn);

        let msg = format!(
            "MESSAGE sip:upstream@{}:{} SIP/2.0\r\n\
             Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK\r\n\
             From: <sip:{}@127.0.0.1:5060>;tag=fwd-from\r\n\
             To: <sip:upstream@{}:{}>\r\n\
             Call-ID: {}\r\n\
             CSeq: 1 MESSAGE\r\n\
             Content-Type: APPLICATION/MANSCDP+XML\r\n\
             Content-Length: {}\r\n\r\n\
             {}",
            addr.ip(), addr.port(),
            self.local_id,
            addr.ip(), addr.port(),
            call_id,
            body.len(),
            body
        );
        Ok(msg)
    }
}

// ---------------------------------------------------------------------------
// 辅助数据结构
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CascadeChannelInfo {
    pub device_id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct MobilePositionInfo {
    pub device_id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub speed: Option<f64>,
    pub direction: Option<i32>,
    pub time: String,
}

#[derive(Debug, Clone)]
pub struct AlarmInfo {
    pub device_id: String,
    pub alarm_type: String,
    pub priority: Option<i32>,
    pub time: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sendrtp_session_lifecycle() {
        let mgr = SendRtpManager::new();
        let session = SendRtpSession::new(
            "cascade_abc".to_string(),
            "plat001".to_string(),
            "ch001".to_string(),
            "192.168.1.100".to_string(),
            30000,
            "0100000001".to_string(),
        );
        mgr.create(session);
        assert_eq!(mgr.active_count(), 1);

        // 通过 cascade_call_id 查找
        assert!(mgr.get("cascade_abc").is_some());

        // 通过 channel_id 关闭
        let closed = mgr.close_by_channel("ch001");
        assert_eq!(closed.len(), 1);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_forwarder_config() {
        let mut forwarder = CascadeForwarder::new("34020000002000000001".to_string());
        forwarder.set_upstream("192.168.1.200".to_string(), 5060);
        assert!(forwarder.upstream_addr.is_some());
    }

    #[test]
    fn test_forward_catalog() {
        let forwarder = CascadeForwarder::new("34020000002000000001".to_string());
        let channels = vec![
            CascadeChannelInfo {
                device_id: "34020000001320000001001".to_string(),
                name: "Cam001".to_string(),
                status: "ON".to_string(),
            },
            CascadeChannelInfo {
                device_id: "34020000001320000001002".to_string(),
                name: "Cam002".to_string(),
                status: "OFF".to_string(),
            },
        ];
        // 没有配置 upstream，forward_catalog 应返回错误
        // （不实际发包，测试构建逻辑）
        let result = forwarder.forward_catalog(&channels);
        assert!(result.is_err()); // upstream not configured
    }
}
