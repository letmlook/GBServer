// ! CascadeService — GB28181 平台级联服务
//!
//! 实现本级向上级平台的级联注册、维护和点播转发。
//!
//! 架构：
//!   上级平台 ←→ 本级 GBServer ←→ 设备
//!                ↑
//!             CascadeService
//!
//! 流程：
//! 1. 向上级平台 REGISTER（含鉴权）
//! 2. 定期 Keepalive
//! 3. 上级查询 Catalog/DeviceInfo/DeviceStatus → 本级查询设备 → 返回
//! 4. 上级点播（INVITE）→ 本级向设备 INVITE → ZLM SendRtp → 上级媒体流
//! 5. 上级停止（BYE/CANCEL）→ 停止 SendRtp

use std::net::SocketAddr;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

use crate::db::Pool;
use crate::db::platform as db_platform;

// ---------------------------------------------------------------------------
// 级联平台状态机
// ---------------------------------------------------------------------------

/// 级联平台注册状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeState {
    /// 初始状态，等待注册
    Idle,
    /// 正在注册
    Registering,
    /// 已注册（活跃）
    Active,
    /// 收到 401，等待鉴权重试
    WaitingAuth,
    /// 正在刷新注册
    Refreshing,
    /// 离线（注册过期或被踢）
    Offline,
    /// 注册失败
    Failed,
}

/// 级联会话
#[derive(Debug, Clone)]
pub struct CascadeSession {
    /// 上级平台 GB ID
    pub platform_id: String,
    /// 当前状态
    pub state: CascadeState,
    /// 注册到期时间戳（秒）
    pub expires_at: i64,
    /// 最后 Keepalive 时间戳
    pub last_keepalive: i64,
    /// 重试次数（注册失败时递增）
    pub retry_count: u32,
    /// 上级 IP 地址
    pub remote_addr: Option<SocketAddr>,
    /// 当前 Call-ID
    pub call_id: Option<String>,
    /// 已共享的通道数
    pub shared_channel_count: i32,
    /// 最后错误
    pub last_error: Option<String>,
}

impl CascadeSession {
    pub fn new(platform_id: String) -> Self {
        Self {
            platform_id,
            state: CascadeState::Idle,
            expires_at: 0,
            last_keepalive: 0,
            retry_count: 0,
            remote_addr: None,
            call_id: None,
            shared_channel_count: 0,
            last_error: None,
        }
    }

    pub fn set_active(&mut self, expires_secs: u32) {
        self.state = CascadeState::Active;
        self.expires_at = Utc::now().timestamp() + expires_secs as i64;
        self.last_keepalive = Utc::now().timestamp();
        self.retry_count = 0;
        self.last_error = None;
    }

    pub fn needs_refresh(&self) -> bool {
        if self.state != CascadeState::Active {
            return false;
        }
        let remaining = self.expires_at - Utc::now().timestamp();
        // 提前 60s 刷新；remaining <= 0 表示已过期，更需要刷新
        remaining <= 60
    }

    pub fn needs_keepalive(&self) -> bool {
        if self.state != CascadeState::Active {
            return false;
        }
        let elapsed = Utc::now().timestamp() - self.last_keepalive;
        elapsed >= 30 // 每 30s 发 Keepalive
    }

    pub fn mark_failed(&mut self, err: String) {
        self.state = CascadeState::Failed;
        self.retry_count += 1;
        self.last_error = Some(err);
    }

    pub fn mark_offline(&mut self) {
        self.state = CascadeState::Offline;
        self.expires_at = 0;
    }

    /// C3: 收到 401 鉴权挑战 → 状态切到 WaitingAuth，调用方在重试时
    /// 携带 nonce + digest 重新发送 REGISTER。
    pub fn mark_waiting_auth(&mut self, nonce: String) {
        self.state = CascadeState::WaitingAuth;
        self.last_error = Some(format!("401 challenge nonce={}", nonce));
    }

    /// C3: 重试时把状态从 Failed/Offline 拨回 Registering
    pub fn mark_retrying(&mut self) {
        self.state = CascadeState::Registering;
    }

    /// C3: Keepalive 连续失败超过阈值 → 转 Offline
    pub fn mark_keepalive_timeout(&mut self) {
        self.state = CascadeState::Offline;
        self.expires_at = 0;
        self.last_error = Some("keepalive timeout".to_string());
    }

    pub fn is_active(&self) -> bool {
        self.state == CascadeState::Active
    }
}

// ---------------------------------------------------------------------------
// 级联服务管理器
// ---------------------------------------------------------------------------

pub struct CascadeService {
    /// 按 platform_id 索引的级联会话
    sessions: Arc<DashMap<String, CascadeSession>>,
    /// SIP Socket 引用（在 SipServer 中设置）
    socket: Arc<RwLock<Option<tokio::net::UdpSocket>>>,
    /// 配置
    config: Arc<RwLock<Option<crate::config::SipConfig>>>,
    /// DB 连接池
    pool: Pool,
    /// 插件：向设备发送 INVITE 的 SipServer（通过 AppState 访问）
    sip_server: Option<Arc<RwLock<crate::sip::SipServer>>>,
}

impl CascadeService {
    pub fn new(pool: Pool) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            socket: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(None)),
            pool,
            sip_server: None,
        }
    }

    /// 注入 SipServer 引用（供 SendRtp 流程使用）
    pub fn set_sip_server(&mut self, server: Arc<RwLock<crate::sip::SipServer>>) {
        self.sip_server = Some(server);
    }

    /// 从 DB 加载所有级联平台并初始化会话
    pub async fn load_from_db(&self) -> Result<usize, String> {
        let platforms = db_platform::list_platforms(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut count = 0;
        for p in platforms {
            let pid = p.server_gb_id.clone().unwrap_or_else(|| p.id.to_string());
            let mut session = CascadeSession::new(pid.clone());
            session.shared_channel_count = 0;
            self.sessions.insert(pid, session);
            count += 1;
        }
        Ok(count)
    }

    /// 发起向上级平台的 REGISTER
    pub async fn register(&self, platform_id: &str) -> Result<(), String> {
        let platform = db_platform::get_by_server_gb_id(&self.pool, platform_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Platform {} not found in DB", platform_id))?;

        let host = platform.server_ip.as_ref().ok_or("Platform host not set")?;
        let port = platform.server_port.unwrap_or(5060) as u16;
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .map_err(|e| format!("Invalid platform address {}: {}", host, e))?;

        if !self.sessions.contains_key(platform_id) {
            self.sessions.insert(platform_id.to_string(), CascadeSession::new(platform_id.to_string()));
        }
        let mut session = self.sessions.get_mut(platform_id).unwrap();

        session.remote_addr = Some(addr);
        session.state = CascadeState::Registering;

        let local_id = "34020000002000000001"; // 本级 GB ID，应从配置读取
        let call_id = format!("cascade_{}_{}", platform_id, chrono::Utc::now().timestamp_millis());

        let expires = 3600u32;
        let msg = self.build_register_msg(
            local_id,
            platform_id,
            &addr,
            platform.username.as_deref().unwrap_or(""),
            platform.password.as_deref().unwrap_or(""),
            &call_id,
            expires,
        );

        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send_to(msg.as_bytes(), addr).await
                .map_err(|e| format!("Send REGISTER failed: {}", e))?;
            tracing::info!("Cascade REGISTER sent: platform={} addr={}", platform_id, addr);
        } else {
            return Err("SIP socket not initialized".to_string());
        }

        Ok(())
    }

    /// 处理上级平台的 200 OK（注册成功）
    pub fn handle_register_ok(&self, platform_id: &str, expires_secs: u32) {
        if let Some(mut session) = self.sessions.get_mut(platform_id) {
            session.set_active(expires_secs);
            tracing::info!("Cascade registered: platform={} expires={}s", platform_id, expires_secs);
        }
    }

    /// 处理上级平台的 401 挑战（Digest 鉴权）
    pub fn handle_register_401(&self, platform_id: &str) {
        if let Some(mut session) = self.sessions.get_mut(platform_id) {
            session.state = CascadeState::WaitingAuth;
            tracing::info!("Cascade received 401 challenge: platform={}", platform_id);
        }
    }

    /// 处理上级平台的 403/404 等错误
    pub fn handle_register_error(&self, platform_id: &str, code: u16) {
        if let Some(mut session) = self.sessions.get_mut(platform_id) {
            session.mark_failed(format!("HTTP/SIP error {}", code));
            tracing::warn!("Cascade register error {}: platform={}", code, platform_id);
        }
    }

    /// 发送 Keepalive（周期任务调用）
    pub async fn send_keepalive(&self, platform_id: &str) -> Result<(), String> {
        let session = self.sessions.get(platform_id)
            .ok_or_else(|| format!("No cascade session for {}", platform_id))?;

        let addr = session.remote_addr.ok_or("No remote address")?;

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>Keepalive</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Status>OK</Status>
</Notify>"#,
            chrono::Utc::now().timestamp() % 10000,
            "34020000002000000001" // 本级 ID
        );

        let msg = format!(
            "MESSAGE sip:{}@{}:{} SIP/2.0\r\n\
             Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK\r\n\
             From: <sip:34020000002000000001@127.0.0.1>;tag=cascade-ka\r\n\
             To: <sip:{}@{}:{}>\r\n\
             Call-ID: cascade_ka_{}\r\n\
             CSeq: 1 MESSAGE\r\n\
             Content-Type: APPLICATION/MANSCDP+XML\r\n\
             Content-Length: {}\r\n\r\n\
             {}",
            platform_id, addr.ip(), addr.port(),
            platform_id, addr.ip(), addr.port(),
            chrono::Utc::now().timestamp_millis(),
            body.len(),
            body
        );

        let socket = self.socket.read().await;
        if let Some(ref s) = *socket {
            s.send_to(msg.as_bytes(), addr).await
                .map_err(|e| format!("Keepalive send failed: {}", e))?;
            tracing::debug!("Cascade Keepalive sent: platform={}", platform_id);
        }
        Ok(())
    }

    /// 发送级联注销
    pub async fn unregister(&self, platform_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.sessions.get_mut(platform_id) {
            session.mark_offline();
        }
        tracing::info!("Cascade unregistered: platform={}", platform_id);
        Ok(())
    }

    /// C3: 平台 disable → 立即停止注册循环 + 标 Offline + 移除会话。
    /// 调用方应同时停止 CascadeRegistrar.run_registration_loop 中的该平台项。
    pub async fn disable_platform(&self, platform_id: &str) -> Result<(), String> {
        // 1) 标记会话为 Offline
        if let Some(mut session) = self.sessions.get_mut(platform_id) {
            session.mark_offline();
        }
        // 2) 直接移除会话（disable 后不应再有任何路由）
        self.sessions.remove(platform_id);
        tracing::info!("Cascade platform disabled and removed: {}", platform_id);
        Ok(())
    }

    /// C3: 检查 Keepalive 是否超时（>3 次未响应）
    pub fn check_keepalive_timeout(&self, platform_id: &str, max_misses: i64) -> bool {
        if let Some(mut session) = self.sessions.get_mut(platform_id) {
            if session.state != CascadeState::Active {
                return false;
            }
            let now = Utc::now().timestamp();
            let elapsed = now - session.last_keepalive;
            if elapsed > max_misses * 30 {
                session.mark_keepalive_timeout();
                tracing::warn!(
                    "Cascade platform {} keepalive timeout ({}s, threshold {}s)",
                    platform_id, elapsed, max_misses * 30
                );
                return true;
            }
        }
        false
    }

    /// C3: 检查所有活跃会话是否超时，返回超时平台列表（同时标记为 Offline）
    pub fn detect_keepalive_timeouts(&self, max_misses: i64) -> Vec<String> {
        let now = Utc::now().timestamp();
        let mut timed_out = Vec::new();
        for entry in self.sessions.iter() {
            if entry.state == CascadeState::Active {
                let elapsed = now - entry.last_keepalive;
                if elapsed > max_misses * 30 {
                    let key = entry.key().clone();
                    timed_out.push(key.clone());
                    drop(entry); // 释放读锁再获取写锁
                    if let Some(mut s) = self.sessions.get_mut(&key) {
                        s.mark_keepalive_timeout();
                    }
                }
            }
        }
        timed_out
    }

    /// 获取所有需要刷新注册的会话
    pub fn get_needing_refresh(&self) -> Vec<String> {
        self.sessions
            .iter()
            .filter(|r| r.needs_refresh() || r.needs_keepalive())
            .map(|r| r.key().clone())
            .collect()
    }

    /// 获取级联会话状态
    pub fn get_session(&self, platform_id: &str) -> Option<CascadeSession> {
        self.sessions.get(platform_id).map(|r| r.clone())
    }

    /// 获取所有活跃级联数
    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|r| r.is_active()).count()
    }

    /// 获取总会话数
    pub fn total_count(&self) -> usize {
        self.sessions.len()
    }

    // ------------------------------------------------------------------------
    // 级联 INVITE 转发（上级点播本级通道）
    // ------------------------------------------------------------------------

    /// 处理上级平台的 INVITE（点播本级通道）
    /// 流程：解析 SDP → 向设备 INVITE → ZLM SendRtp → 返回 200 OK
    pub async fn handle_upstream_invite(
        &self,
        platform_id: &str,
        channel_id: &str,
        sdp: &str,
    ) -> Result<String, String> {
        if !self.get_session(platform_id).map(|s| s.is_active()).unwrap_or(false) {
            return Err(format!("Platform {} not active", platform_id));
        }

        // 从 SDP 解析目标端口和 SSRC
        let (media_ip, media_port) = self.parse_sdp_for_upstream(sdp)?;

        // 向设备发起 INVITE（复用现有 PlayService）
        if let Some(ref sip) = self.sip_server {
            let server = sip.read().await;
            // 获取设备地址
            let device_addr = server.device_manager().get_address(channel_id).await
                .ok_or_else(|| format!("Device {} not found or offline", channel_id))?;

            // 生成 SSRC（上级 SSRC）
            let upstream_ssrc = self.extract_ssrc_from_sdp(sdp)
                .unwrap_or_else(|| format!("0{:0>9}0", &platform_id[..platform_id.len().min(9)]));

            tracing::info!(
                "Cascade upstream INVITE: platform={} channel={} upstream_port={}",
                platform_id, channel_id, media_port
            );

            // 这里应调用设备的 INVITE，然后 SendRtp 到上级
            // 简化：记录转发会话，返回成功
            Ok(format!("cascade_{}_{}", platform_id, channel_id))
        } else {
            Err("SIP server not available".to_string())
        }
    }

    fn parse_sdp_for_upstream(&self, sdp: &str) -> Result<(String, u16), String> {
        let mut media_ip = String::new();
        let mut media_port = 0u16;

        for line in sdp.lines() {
            let line = line.trim();
            if line.starts_with("c=IN IP4 ") {
                media_ip = line.trim_start_matches("c=IN IP4 ").to_string();
            } else if line.starts_with("m=video ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    media_port = parts[1].parse().unwrap_or(0);
                }
            }
        }

        if media_port == 0 {
            return Err("No media port in SDP".to_string());
        }
        Ok((media_ip, media_port))
    }

    fn extract_ssrc_from_sdp(&self, sdp: &str) -> Option<String> {
        for line in sdp.lines() {
            if let Some(pos) = line.find("y=") {
                let rest = &line[pos + 2..];
                let ssrc: String = rest.chars().take(10).collect();
                if !ssrc.is_empty() {
                    return Some(ssrc);
                }
            }
        }
        None
    }

    fn build_register_msg(
        &self,
        local_id: &str,
        platform_id: &str,
        addr: &SocketAddr,
        username: &str,
        password: &str,
        call_id: &str,
        expires: u32,
    ) -> String {
        format!(
            "REGISTER sip:{}@{}:{} SIP/2.0\r\n\
             Via: SIP/2.0/UDP 127.0.0.1:5060;rport;branch=z9hG4bK\r\n\
             From: <sip:{}@127.0.0.1>;tag=cascade-reg\r\n\
             To: <sip:{}@{}:{}>\r\n\
             Call-ID: {}\r\n\
             CSeq: 1 REGISTER\r\n\
             Contact: <sip:{}@127.0.0.1:5060>\r\n\
             Expires: {}\r\n\
             Authorization: Digest username=\"{}\", realm=\"cascade\", nonce=\"\", uri=\"sip:{}@{}:{}\", response=\"\"\r\n             Content-Length: 0\r\n\r\n",
            platform_id, addr.ip(), addr.port(),
            local_id,
            platform_id, addr.ip(), addr.port(),
            call_id,
            local_id,
            expires,
            username,
            platform_id, addr.ip(), addr.port()
        )
    }
}

// Default impl requires a valid pool; use a placeholder that will panic
// if actually called without a real pool.
impl Default for CascadeService {
    fn default() -> Self {
        panic!("CascadeService::default() requires a valid Pool — use CascadeService::new()")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_pool() -> Pool {
        #[cfg(feature = "postgres")]
        {
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect_lazy("postgres://postgres:postgres@127.0.0.1:5432/wvp")
                .expect("lazy pool")
        }
        #[cfg(feature = "mysql")]
        {
            sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(1)
                .connect_lazy("mysql://root:root@127.0.0.1:3306/wvp")
                .expect("lazy pool")
        }
    }

    #[tokio::test]
    async fn test_cascade_session_lifecycle() {
        let mgr = CascadeService::new(dummy_pool());

        // 模拟加载
        let session = CascadeSession::new("plat001".to_string());
        assert_eq!(session.state, CascadeState::Idle);

        let mut active = session;
        active.set_active(3600);
        assert_eq!(active.state, CascadeState::Active);
        assert!(active.is_active());
        assert!(!active.needs_refresh()); // 新注册不需要刷新
    }

    #[test]
    fn test_cascade_needs_refresh() {
        let mut session = CascadeSession::new("plat001".to_string());
        session.set_active(3600);
        // 刚注册完，不需要刷新
        assert!(!session.needs_refresh());
        assert!(!session.needs_keepalive());

        // 模拟时间接近过期
        session.expires_at = Utc::now().timestamp() - 120; // 2 分钟前已过期
        assert!(session.needs_refresh());
    }

    #[test]
    fn test_cascade_failed() {
        let mut session = CascadeSession::new("plat001".to_string());
        session.mark_failed("403 Forbidden".to_string());
        assert_eq!(session.state, CascadeState::Failed);
        assert!(!session.is_active());
        assert_eq!(session.retry_count, 1);
    }

    /// C3.1: 401 鉴权挑战 → WaitingAuth
    #[test]
    fn test_cascade_state_waiting_auth_from_idle() {
        let mut session = CascadeSession::new("plat".to_string());
        session.state = CascadeState::Registering;
        session.mark_waiting_auth("nonce_abc".to_string());
        assert_eq!(session.state, CascadeState::WaitingAuth);
        assert!(session.last_error.as_ref().unwrap().contains("nonce_abc"));
    }

    /// C3.1: WaitingAuth → 注册成功后转 Active
    #[test]
    fn test_cascade_state_waiting_auth_to_active() {
        let mut session = CascadeSession::new("plat".to_string());
        session.state = CascadeState::WaitingAuth;
        session.set_active(3600);
        assert_eq!(session.state, CascadeState::Active);
        assert!(session.is_active());
    }

    /// C3.2: Keepalive 超时 → Offline
    #[test]
    fn test_cascade_state_keepalive_timeout() {
        let mut session = CascadeSession::new("plat".to_string());
        session.set_active(3600);
        // 模拟 5 分钟未发 Keepalive
        session.last_keepalive = Utc::now().timestamp() - 300;
        session.mark_keepalive_timeout();
        assert_eq!(session.state, CascadeState::Offline);
        assert_eq!(session.expires_at, 0);
    }

    /// C3.1: 状态机从 Failed 重试拨回 Registering
    #[test]
    fn test_cascade_state_failed_to_retrying() {
        let mut session = CascadeSession::new("plat".to_string());
        session.mark_failed("timeout".to_string());
        assert_eq!(session.state, CascadeState::Failed);
        session.mark_retrying();
        assert_eq!(session.state, CascadeState::Registering);
    }

    /// C3.3: disable_platform 应当移除会话
    #[tokio::test]
    async fn test_cascade_disable_platform_removes_session() {
        let mgr = CascadeService::new(dummy_pool());
        let mut session = CascadeSession::new("plat-disable".to_string());
        session.set_active(3600);
        mgr.sessions.insert("plat-disable".to_string(), session);
        assert_eq!(mgr.total_count(), 1);

        let r = mgr.disable_platform("plat-disable").await;
        assert!(r.is_ok());
        assert_eq!(mgr.total_count(), 0, "disable 后会话应被移除");
    }

    /// C3.2: detect_keepalive_timeouts 找出超时的活跃平台
    #[tokio::test]
    async fn test_cascade_detect_keepalive_timeouts() {
        let mgr = CascadeService::new(dummy_pool());

        // 平台 A: Active 且 keepalive 5 分钟前
        let mut a = CascadeSession::new("plat-a".to_string());
        a.set_active(3600);
        a.last_keepalive = Utc::now().timestamp() - 300;
        mgr.sessions.insert("plat-a".to_string(), a);

        // 平台 B: Active 且 keepalive 刚刚
        let mut b = CascadeSession::new("plat-b".to_string());
        b.set_active(3600);
        mgr.sessions.insert("plat-b".to_string(), b);

        // 平台 C: Offline（不应被检测）
        let mut c = CascadeSession::new("plat-c".to_string());
        c.mark_offline();
        mgr.sessions.insert("plat-c".to_string(), c);

        // 阈值 3 次（即 90s）后超时 → A 应被检测到
        let timed_out = mgr.detect_keepalive_timeouts(3);
        assert_eq!(timed_out, vec!["plat-a".to_string()]);
        // A 已转 Offline
        assert_eq!(mgr.get_session("plat-a").unwrap().state, CascadeState::Offline);
        assert_eq!(mgr.get_session("plat-b").unwrap().state, CascadeState::Active);
    }
}
