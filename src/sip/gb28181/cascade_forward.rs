// ! CascadeForward — 级联订阅转发与 SendRtp 会话管理
//!
//! 功能：
//! 1. Catalog 转发（向上级 NOTIFY 已共享通道）
//! 2. MobilePosition 转发（订阅位置上报到上级）
//! 3. Alarm 转发（告警上报到上级）
//! 4. SendRtp 会话管理（设备拉流→推送到上级）

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
    /// E1: 可选的 StateStore（用于跨节点共享）
    state_store: Option<Arc<crate::state_store::StateStore>>,
}

impl SendRtpManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            state_store: None,
        }
    }

    /// E1: 注入 StateStore（让 SendRtp 会话在多节点间共享）
    pub fn set_state_store(&mut self, store: Arc<crate::state_store::StateStore>) {
        self.state_store = Some(store);
    }
    /// 创建 SendRtp 会话
    pub fn create(&self, session: SendRtpSession) {
        // E1: 同步到 StateStore（如果有）
        if let Some(ref store) = self.state_store {
            let state = crate::state_store::CascadeSendRtpState {
                cascade_call_id: session.cascade_call_id.clone(),
                platform_id: session.platform_id.clone(),
                channel_id: session.channel_id.clone(),
                upstream_host: session.upstream_host.clone(),
                upstream_port: session.upstream_port,
                active: session.active,
                started_at: session.created_at,
            };
            store.set_cascade_sendrtp(&session.cascade_call_id, state);
        }
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
        // E1: 从 StateStore 删除
        if let Some(ref store) = self.state_store {
            store.remove_cascade_sendrtp(cascade_call_id);
        }
        self.sessions.remove(cascade_call_id).map(|(_, v)| v)
    }

    /// Phase 5.4: 按 ZLM 推送的 stream_id 关闭 SendRtp 会话
    ///
    /// 用途：`on_send_rtp_stopped` hook 收到 ZLM 通知时按 `data.stream` 查表清理。
    /// 匹配规则：
    /// 1. 精确等于 `cascade_call_id`（大多数情况，cascade_call_id 形如 `cascade_{platform}_{channel}`）
    /// 2. 前缀匹配（容忍 ZLM 在 stream 末尾追加 `.ts` / `-h264` 等后缀）
    ///
    /// 关闭后：
    /// - `active` 置 false（保留 session 供查询）
    /// - 同步到 StateStore（如已注入），用于跨节点一致性
    /// - 返回被关闭的 session（供调用方做后续处理，如发 BYE 给上级）
    pub fn close_by_stream(&self, stream_id: &str) -> Option<SendRtpSession> {
        // 先收集匹配 key（DashMap iter 持 shard 读锁，必须先 drop 再 remove）
        let matched_key: Option<String> = self
            .sessions
            .iter()
            .find(|entry| {
                entry.value().cascade_call_id == stream_id
                    || stream_id.starts_with(&entry.value().cascade_call_id)
            })
            .map(|entry| entry.key().clone());

        if let Some(key) = matched_key {
            if let Some(mut entry) = self.sessions.get_mut(&key) {
                entry.active = false;
            }
            // 从 StateStore 删除
            if let Some(ref store) = self.state_store {
                store.remove_cascade_sendrtp(&key);
            }
            return self.sessions.remove(&key).map(|(_, v)| v);
        }
        None
    }

    /// 按通道关闭所有会话
    pub fn close_by_channel(&self, channel_id: &str) -> Vec<SendRtpSession> {
        let snap: Vec<_> = self
            .sessions
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

    /// 处理上级 INVITE — 创建 SendRtp 会话并返回 SSRC 协商结果
    ///
    /// B3: 上级平台点播本级通道时，先记录 SendRtp 会话；
    /// 待设备 INVITE 200 OK 后用 SSRC/port 反向 startSendRtp 指向上级 IP:port。
    pub fn handle_upstream_invite(
        &self,
        cascade_call_id: String,
        platform_id: String,
        channel_id: String,
        upstream_host: String,
        upstream_port: u16,
        upstream_ssrc: String,
    ) -> SendRtpSession {
        let session = SendRtpSession::new(
            cascade_call_id,
            platform_id,
            channel_id,
            upstream_host,
            upstream_port,
            upstream_ssrc,
        );
        self.create(session.clone());
        session
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
        self.upstream_addr = format!("{}:{}", host, port).parse().ok();
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
            s.send_to(msg.as_bytes(), addr)
                .await
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
            s.send_to(msg.as_bytes(), addr)
                .await
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
            s.send_to(msg.as_bytes(), addr)
                .await
                .map_err(|e| format!("Forward Alarm failed: {}", e))?;
        }
        Ok(())
    }

    fn build_notify_msg(&self, cmd_type: &str, body: &str, sn: i64) -> Result<String, String> {
        let addr = self.upstream_addr.ok_or("Upstream not configured")?;
        let call_id = format!("fwd_{}_{}", cmd_type.to_lowercase(), sn);
        let ip = addr.ip().to_string();
        let port = addr.port().to_string();
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
            ip,
            port,
            self.local_id,
            ip,
            port,
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

    #[tokio::test]
    async fn test_forward_catalog() {
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
        let result = forwarder.forward_catalog(&channels).await;
        assert!(result.is_err()); // upstream not configured
    }

    /// B3: SendRtpManager 完整生命周期
    #[test]
    fn test_sendrtp_manager_full_lifecycle() {
        let mgr = SendRtpManager::new();
        assert_eq!(mgr.active_count(), 0);

        let s1 = mgr.handle_upstream_invite(
            "call-1".into(),
            "plat-a".into(),
            "ch-1".into(),
            "10.0.0.1".into(),
            9000,
            "0xAAAA".into(),
        );
        assert!(s1.active);
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.get("call-1").is_some());
        assert_eq!(mgr.get("call-1").unwrap().upstream_ssrc, "0xAAAA");

        let s2 = mgr.handle_upstream_invite(
            "call-2".into(),
            "plat-a".into(),
            "ch-1".into(),
            "10.0.0.2".into(),
            9002,
            "0xBBBB".into(),
        );
        assert_eq!(mgr.active_count(), 2);
        let closed = mgr.close_by_channel("ch-1");
        assert_eq!(closed.len(), 2);
        assert_eq!(mgr.active_count(), 0);
        assert_eq!(closed[0].channel_id, "ch-1"); // snapshot is pre-close, still active=true
    }

    /// B3: close_by_channel 不影响其他通道
    #[test]
    fn test_sendrtp_manager_channel_isolation() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "c1".into(),
            "p".into(),
            "ch-A".into(),
            "10.0.0.1".into(),
            9000,
            "0x1".into(),
        );
        mgr.handle_upstream_invite(
            "c2".into(),
            "p".into(),
            "ch-B".into(),
            "10.0.0.2".into(),
            9002,
            "0x2".into(),
        );
        let closed = mgr.close_by_channel("ch-A");
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].cascade_call_id, "c1");
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.get("c2").is_some());
    }

    /// B3: 重复 create 同 cascade_call_id 覆盖
    #[test]
    fn test_sendrtp_manager_recreate_overrides() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "c1".into(),
            "p".into(),
            "ch".into(),
            "10.0.0.1".into(),
            9000,
            "0xAAAA".into(),
        );
        mgr.handle_upstream_invite(
            "c1".into(),
            "p".into(),
            "ch".into(),
            "10.0.0.99".into(),
            9999,
            "0xBBBB".into(),
        );
        assert_eq!(mgr.active_count(), 1);
        let stored = mgr.get("c1").unwrap();
        assert_eq!(stored.upstream_host, "10.0.0.99");
        assert_eq!(stored.upstream_port, 9999);
        assert_eq!(stored.upstream_ssrc, "0xBBBB");
    }

    /// B3: get_by_channel 返回该通道所有活跃会话
    #[test]
    fn test_sendrtp_manager_get_by_channel() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "c1".into(),
            "p".into(),
            "ch-A".into(),
            "h".into(),
            9000,
            "0x1".into(),
        );
        mgr.handle_upstream_invite(
            "c2".into(),
            "p".into(),
            "ch-A".into(),
            "h".into(),
            9002,
            "0x2".into(),
        );
        mgr.handle_upstream_invite(
            "c3".into(),
            "p".into(),
            "ch-B".into(),
            "h".into(),
            9004,
            "0x3".into(),
        );
        let a = mgr.get_by_channel("ch-A");
        assert_eq!(a.len(), 2);
        let b = mgr.get_by_channel("ch-B");
        assert_eq!(b.len(), 1);
    }

    /// B3: 单条 close 后 active_count 正确归零
    #[test]
    fn test_sendrtp_manager_close_single_session() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "c1".into(),
            "p".into(),
            "ch".into(),
            "h".into(),
            9000,
            "0x1".into(),
        );
        mgr.handle_upstream_invite(
            "c2".into(),
            "p".into(),
            "ch".into(),
            "h".into(),
            9002,
            "0x2".into(),
        );
        assert_eq!(mgr.active_count(), 2);
        let closed = mgr.close("c1");
        assert!(closed.is_some());
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.get("c1").is_none());
        assert!(mgr.get("c2").is_some());
    }

    /// B3: BYE 后 close 应当让对应 cascade_call_id 立即不可查
    #[test]
    fn test_sendrtp_manager_bye_cleanup() {
        let mgr = SendRtpManager::new();
        let s = mgr.handle_upstream_invite("call-bye".into(), "plat".into(), "ch-bye".into(),
            "10.0.0.5".into(), 9100, "0xDEAD".into());
        assert!(s.active);
        let removed = mgr.close("call-bye");
        assert!(removed.is_some(), "close should return the removed session");
        assert!(!removed.unwrap().active, "removed session should be inactive");
        assert!(mgr.get("call-bye").is_none());
        assert_eq!(mgr.active_count(), 0);
    }

    /// B3: CANCEL 处理等同于 BYE — close 应彻底移除会话
    #[test]
    fn test_sendrtp_manager_cancel_removes_session() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite("call-cancel".into(), "plat".into(), "ch".into(),
            "h".into(), 9000, "0x1".into());
        let snap_before = mgr.get_by_channel("ch");
        assert_eq!(snap_before.len(), 1);

        // CANCEL cleanup path — same as BYE
        mgr.close("call-cancel");

        let snap_after = mgr.get_by_channel("ch");
        assert!(snap_after.is_empty(), "channel snapshot should be empty after cancel");
        assert_eq!(mgr.active_count(), 0);
    }

    /// B3: close_by_channel 在 BYE 场景下可关闭整通道的所有级联 SendRtp
    #[test]
    fn test_sendrtp_manager_bye_close_by_channel() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite("c1".into(), "p1".into(), "ch-bye".into(),
            "h1".into(), 9000, "0x1".into());
        mgr.handle_upstream_invite("c2".into(), "p2".into(), "ch-bye".into(),
            "h2".into(), 9002, "0x2".into());
        assert_eq!(mgr.active_count(), 2);

        let closed = mgr.close_by_channel("ch-bye");
        assert_eq!(closed.len(), 2);
        assert_eq!(mgr.active_count(), 0);
    }

    /// E1: create 会话后，StateStore 应当存有对应记录
    #[test]
    fn test_sendrtp_manager_state_store_create_sync() {
        use crate::state_store::StateStore;
        use std::sync::Arc;

        let store = Arc::new(StateStore::in_memory());
        let mut mgr = SendRtpManager::new();
        mgr.set_state_store(store.clone());

        let s = mgr.handle_upstream_invite(
            "call-store".into(), "plat".into(), "ch-store".into(),
            "10.0.0.1".into(), 9000, "0xAAAA".into(),
        );
        assert!(s.active);

        // StateStore 应当有这条记录
        let stored = store.get_cascade_sendrtp("call-store");
        assert!(stored.is_some(), "StateStore 应存有 SendRtp 会话");
        let stored = stored.unwrap();
        assert_eq!(stored.cascade_call_id, "call-store");
        assert_eq!(stored.platform_id, "plat");
        assert_eq!(stored.channel_id, "ch-store");
        assert_eq!(stored.upstream_host, "10.0.0.1");
        assert_eq!(stored.upstream_port, 9000);
        assert!(stored.active);
    }

    /// E1: close 会话后，StateStore 应当同步删除
    #[test]
    fn test_sendrtp_manager_state_store_close_del() {
        use crate::state_store::StateStore;
        use std::sync::Arc;

        let store = Arc::new(StateStore::in_memory());
        let mut mgr = SendRtpManager::new();
        mgr.set_state_store(store.clone());

        mgr.handle_upstream_invite("c1".into(), "p".into(), "ch".into(),
            "h".into(), 9000, "0x1".into());
        assert!(store.get_cascade_sendrtp("c1").is_some());

        mgr.close("c1");
        assert!(store.get_cascade_sendrtp("c1").is_none(),
            "close 后 StateStore 应被删除");
    }

    /// E1: 不注入 StateStore 时不应 panic
    #[test]
    fn test_sendrtp_manager_no_state_store_works() {
        let mgr = SendRtpManager::new();
        // 没有 set_state_store，直接 create 应正常工作
        mgr.handle_upstream_invite("c1".into(), "p".into(), "ch".into(),
            "h".into(), 9000, "0x1".into());
        assert_eq!(mgr.active_count(), 1);
        mgr.close("c1");
        assert_eq!(mgr.active_count(), 0);
    }

    // ============ Phase 5.4: close_by_stream 单测 ============

    /// close_by_stream 精确匹配 cascade_call_id
    #[test]
    fn phase5_close_by_stream_exact_match() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "cascade_plat1_ch1".into(), "plat1".into(), "ch1".into(),
            "192.168.1.10".into(), 9000, "0x1234".into()
        );
        assert_eq!(mgr.active_count(), 1);

        let closed = mgr.close_by_stream("cascade_plat1_ch1");
        assert!(closed.is_some(), "精确匹配应能关闭 session");
        let session = closed.unwrap();
        assert_eq!(session.cascade_call_id, "cascade_plat1_ch1");
        assert_eq!(session.platform_id, "plat1");
        assert_eq!(mgr.active_count(), 0, "关闭后 active_count 应为 0");
    }

    /// close_by_stream 前缀匹配（容忍 ZLM 在 stream 末尾追加 .ts 等后缀）
    #[test]
    fn phase5_close_by_stream_prefix_match() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "cascade_plat1_ch1".into(), "plat1".into(), "ch1".into(),
            "192.168.1.10".into(), 9000, "0x1234".into()
        );

        // ZLM 实际推送的 stream 可能带 .ts 后缀
        let closed = mgr.close_by_stream("cascade_plat1_ch1.h264");
        assert!(closed.is_some(), "前缀匹配应能命中");
        assert_eq!(mgr.active_count(), 0);
    }

    /// close_by_stream 不匹配时返回 None 且不 panic
    #[test]
    fn phase5_close_by_stream_no_match() {
        let mgr = SendRtpManager::new();
        mgr.handle_upstream_invite(
            "cascade_plat1_ch1".into(), "plat1".into(), "ch1".into(),
            "192.168.1.10".into(), 9000, "0x1234".into()
        );

        let closed = mgr.close_by_stream("cascade_other_ch1");
        assert!(closed.is_none(), "不应匹配其他 session");
        assert_eq!(mgr.active_count(), 1, "无关 stream 不应影响 active session");

        // 空字符串也不应 panic
        let none = mgr.close_by_stream("");
        assert!(none.is_none());
    }

    /// close_by_stream 同步到 StateStore
    #[test]
    fn phase5_close_by_stream_state_store_sync() {
        use crate::state_store::StateStore;
        use std::sync::Arc;

        let store = Arc::new(StateStore::in_memory());
        let mut mgr = SendRtpManager::new();
        mgr.set_state_store(store.clone());

        mgr.handle_upstream_invite(
            "cascade_sync_test".into(), "plat".into(), "ch".into(),
            "h".into(), 9000, "0x1".into()
        );
        assert!(store.get_cascade_sendrtp("cascade_sync_test").is_some());

        let closed = mgr.close_by_stream("cascade_sync_test");
        assert!(closed.is_some());
        assert!(
            store.get_cascade_sendrtp("cascade_sync_test").is_none(),
            "close_by_stream 应同步从 StateStore 删除"
        );
    }
}
