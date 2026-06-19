use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    NotRegistered,
    Registering,
    Challenged,
    Registered,
    Failed,
}

#[derive(Debug, Clone)]
pub struct RegistrationState {
    pub platform_id: String,
    pub platform_device_id: String,
    pub host: String,
    pub port: u16,
    pub local_device_id: String,
    pub password: String,
    pub realm: String,
    pub status: RegistrationStatus,
    pub register_interval_secs: u64,
    pub last_register: Option<DateTime<Utc>>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub nonce: Option<String>,
    pub opaque: Option<String>,
}

pub struct CascadeRegistrar {
    states: DashMap<String, RegistrationState>,
    sip_server: RwLock<Option<Arc<RwLock<crate::sip::SipServer>>>>,
    pool: RwLock<Option<crate::db::Pool>>,
}

impl CascadeRegistrar {
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
            sip_server: RwLock::new(None),
            pool: RwLock::new(None),
        }
    }

    pub async fn set_sip_server(&self, server: Arc<RwLock<crate::sip::SipServer>>) {
        *self.sip_server.write().await = Some(server);
    }

    pub async fn set_pool(&self, pool: crate::db::Pool) {
        *self.pool.write().await = Some(pool);
    }

    pub async fn get_sip_server(&self) -> Option<Arc<RwLock<crate::sip::SipServer>>> {
        self.sip_server.read().await.clone()
    }

    pub fn add_platform(
        &self,
        platform_id: &str,
        platform_device_id: &str,
        host: &str,
        port: u16,
        local_device_id: &str,
        password: &str,
        realm: &str,
        register_interval_secs: u64,
    ) {
        self.states.insert(platform_id.to_string(), RegistrationState {
            platform_id: platform_id.to_string(),
            platform_device_id: platform_device_id.to_string(),
            host: host.to_string(),
            port,
            local_device_id: local_device_id.to_string(),
            password: password.to_string(),
            realm: realm.to_string(),
            status: RegistrationStatus::NotRegistered,
            register_interval_secs,
            last_register: None,
            retry_count: 0,
            max_retries: 5,
            nonce: None,
            opaque: None,
        });
    }

    pub fn remove_platform(&self, platform_id: &str) {
        self.states.remove(platform_id);
    }

    pub fn get_status(&self, platform_id: &str) -> Option<RegistrationStatus> {
        self.states.get(platform_id).map(|s| s.status)
    }

    pub fn get_all_states(&self) -> Vec<RegistrationState> {
        self.states.iter().map(|s| s.clone()).collect()
    }

    pub fn state(&self, platform_id: &str) -> Option<RegistrationState> {
        self.states.get(platform_id).map(|s| s.clone())
    }

    /// C3.2: 记录最近一次与该平台的双向通信时间（register 成功、keepalive 响应、上级 INVITE 等）
    pub fn note_liveness(&self, platform_id: &str) {
        if let Some(mut s) = self.states.get_mut(platform_id) {
            s.last_register = Some(Utc::now());
        }
    }

    /// C3.2: 检测所有 Registered 平台中超过阈值未响应的，标记为 Failed（外层会触发重试）
    pub fn detect_keepalive_timeouts(&self, max_misses: i64) -> Vec<String> {
        let now = Utc::now();
        let cutoff_secs = max_misses * 60;

        // 第一阶段：仅持读锁遍历收集超时平台 id。
        // DashMap 的 iter() 在迭代期间会持有所有 shard 的读锁，因此必须先把
        // 待处理 key 收集到 Vec 并让迭代器 drop，再走 get_mut 拿写锁 —— 否则
        // iter 的读锁与 get_mut 的写锁会自死锁（futex_wait）。
        let timed_out_keys: Vec<String> = self
            .states
            .iter()
            .filter(|entry| {
                entry.status == RegistrationStatus::Registered
                    && match entry.last_register {
                        Some(t) => (now - t).num_seconds() > cutoff_secs,
                        None => true,
                    }
            })
            .map(|entry| entry.key().clone())
            .collect();

        // 第二阶段：迭代器已 drop，shard 读锁全部释放，可安全 get_mut。
        let mut out = Vec::with_capacity(timed_out_keys.len());
        for key in timed_out_keys {
            if let Some(mut s) = self.states.get_mut(&key) {
                let elapsed = match s.last_register {
                    Some(t) => (now - t).num_seconds(),
                    None => i64::MAX,
                };
                s.status = RegistrationStatus::Failed;
                s.retry_count = s.retry_count.saturating_add(1);
                tracing::warn!(
                    "Cascade platform {} keepalive timeout ({}s > {}s * 60), marked Failed",
                    key, elapsed, max_misses
                );
            }
            out.push(key);
        }

        out
    }

    /// C3.3: 从 DB 重新加载平台列表
    /// - 新出现的平台：add_platform
    /// - DB 中 enable=false 的平台：remove_platform（外层负责发 UNREGISTER）
    /// - 已存在但 DB 中字段变化：更新 host/port/password/realm
    pub async fn reload_from_db(&self, local_device_id: &str, realm: &str) -> (Vec<String>, Vec<String>) {
        let pool = match self.pool.read().await.clone() {
            Some(p) => p,
            None => return (Vec::new(), Vec::new()),
        };

        let platforms = match crate::db::platform::list_platforms(&pool).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("reload_from_db: list_platforms failed: {}", e);
                return (Vec::new(), Vec::new());
            }
        };

        let mut added = Vec::new();
        let mut removed = Vec::new();

        // 1) 收集 DB 中的有效 platform_id（仅 enable=true 的）
        let mut active_ids = std::collections::HashSet::new();
        for p in platforms.iter().filter(|p| p.enable.unwrap_or(false)) {
            let server_gb_id = p.server_gb_id.clone().unwrap_or_default();
            let server_ip = p.server_ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());
            let server_port = p.server_port.unwrap_or(5060) as u16;
            let password = p.password.clone().unwrap_or_default();
            let expires_secs: u64 = p.expires.as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600);
            let platform_id = p.device_gb_id.clone().unwrap_or_else(|| server_gb_id.clone());

            active_ids.insert(platform_id.clone());

            match self.states.get_mut(&platform_id) {
                Some(mut s) => {
                    // 更新可能变化的字段
                    let changed = s.host != server_ip
                        || s.port != server_port
                        || s.password != password
                        || s.realm != realm;
                    if changed {
                        s.host = server_ip;
                        s.port = server_port;
                        s.password = password;
                        s.realm = realm.to_string();
                        // 字段变了，强制重注册
                        s.status = RegistrationStatus::NotRegistered;
                        s.nonce = None;
                        s.opaque = None;
                        tracing::info!("Cascade platform {} config changed, will re-register", platform_id);
                    }
                }
                None => {
                    self.add_platform(
                        &platform_id,
                        &server_gb_id,
                        &server_ip,
                        server_port,
                        local_device_id,
                        &password,
                        realm,
                        expires_secs,
                    );
                    added.push(platform_id);
                }
            }
        }

        // 2) 移除在内存中存在但 DB 中已 disable / 删除的平台
        let to_remove: Vec<String> = self
            .states
            .iter()
            .filter(|r| !active_ids.contains(r.key()))
            .map(|r| r.key().clone())
            .collect();

        for id in to_remove {
            self.states.remove(&id);
            removed.push(id.clone());
            tracing::info!("Cascade platform {} removed by reload (DB disable/delete)", id);
        }

        (added, removed)
    }

    /// C3.2: 构造 Keepalive MESSAGE 报文（GB28181 平台级 Keepalive）
    pub fn build_keepalive_message(&self, platform_id: &str, sn: u32) -> Option<String> {
        let state = self.states.get(platform_id)?;
        let branch = format!("z9hG4bKka{}", rand_nonce());
        let call_id = format!("cascade_ka_{}_{}", platform_id, sn);
        let tag = format!("local_ka_{}", rand_nonce());

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
<CmdType>Keepalive</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Status>OK</Status>
</Notify>"#,
            sn, state.local_device_id
        );

        let msg = format!(
            "MESSAGE sip:{}@{}:{} SIP/2.0\r\n\
             Via: SIP/2.0/UDP {}:5060;rport;branch={}\r\n\
             From: <sip:{}@{}>;tag={}\r\n\
             To: <sip:{}@{}:{}>\r\n\
             Call-ID: {}\r\n\
             CSeq: {} MESSAGE\r\n\
             Max-Forwards: 70\r\n\
             User-Agent: GBServer/1.0\r\n\
             Content-Type: APPLICATION/MANSCDP+XML\r\n\
             Content-Length: {}\r\n\r\n\
             {}",
            state.platform_device_id, state.host, state.port,
            state.local_device_id, state.local_device_id, branch,
            state.local_device_id, tag,
            state.platform_device_id, state.host, state.port,
            call_id,
            sn,
            body.len(),
            body
        );
        Some(msg)
    }

    /// C3.2: 给所有 Registered 平台周期性发送 Keepalive（外层 loop 驱动）
    pub async fn send_keepalive_all(&self) {
        let registered: Vec<(String, String, u16)> = self
            .states
            .iter()
            .filter(|s| s.status == RegistrationStatus::Registered)
            .map(|s| (s.platform_id.clone(), s.host.clone(), s.port))
            .collect();

        if registered.is_empty() {
            return;
        }

        let sip_server = match self.get_sip_server().await {
            Some(s) => s,
            None => return,
        };

        for (platform_id, host, port) in registered {
            let sn = (Utc::now().timestamp() as u32) & 0xFFFF;
            let msg = match self.build_keepalive_message(&platform_id, sn) {
                Some(m) => m,
                None => continue,
            };

            let sip = sip_server.read().await;
            let socket_arc = sip.socket().clone();
            drop(sip);
            let socket_guard = socket_arc.read().await;
            if let Some(ref udp) = *socket_guard {
                let ip = match host.parse::<std::net::IpAddr>() {
                    Ok(ip) => ip,
                    Err(_) => continue,
                };
                let addr = std::net::SocketAddr::new(ip, port);
                if let Err(e) = udp.send_to(msg.as_bytes(), addr).await {
                    tracing::warn!("Cascade keepalive send to {} failed: {}", platform_id, e);
                } else {
                    tracing::debug!("Cascade keepalive sent to {}", platform_id);
                }
            }
        }
    }

    /// C3.3: 平台被 DB 禁用后立即 UNREGISTER 并移除会话（由 handler 调用）
    pub async fn unregister_and_remove(&self, platform_id: &str, expires: u32) -> Result<(), String> {
        // 构造 Expires: 0 的 REGISTER 报文
        let msg = self
            .build_register_request(platform_id, 99, 0)
            .ok_or_else(|| format!("Platform {} not in registrar", platform_id))?;

        let sip_server = self
            .get_sip_server()
            .await
            .ok_or_else(|| "SipServer not wired".to_string())?;

        let state = self
            .state(platform_id)
            .ok_or_else(|| format!("Platform {} state not found", platform_id))?;

        let sip = sip_server.read().await;
        let socket_arc = sip.socket().clone();
        drop(sip);
        let socket_guard = socket_arc.read().await;
        let udp = (*socket_guard)
            .as_ref()
            .ok_or_else(|| "SIP socket not initialized".to_string())?;
        let ip: std::net::IpAddr = state
            .host
            .parse()
            .map_err(|e| format!("Invalid host: {}", e))?;
        let addr = std::net::SocketAddr::new(ip, state.port);
        udp.send_to(msg.as_bytes(), addr)
            .await
            .map_err(|e| format!("send UNREGISTER failed: {}", e))?;

        // 标记为 NotRegistered，从内存中移除（让 reload_from_db 不再拉回）
        if let Some(mut s) = self.states.get_mut(platform_id) {
            s.status = RegistrationStatus::NotRegistered;
        }
        self.states.remove(platform_id);
        tracing::info!("Cascade platform {} unregistered and removed", platform_id);
        let _ = expires; // unused for now
        Ok(())
    }

    pub fn handle_401_challenge(&self, platform_id: &str, nonce: &str, opaque: Option<&str>, realm: &str) {
        if let Some(mut state) = self.states.get_mut(platform_id) {
            state.status = RegistrationStatus::Challenged;
            state.nonce = Some(nonce.to_string());
            state.opaque = opaque.map(|s| s.to_string());
            state.realm = realm.to_string();
            tracing::info!("Cascade platform {} challenged, nonce={}", platform_id, nonce);
        }
    }

    pub fn mark_registered(&self, platform_id: &str) {
        if let Some(mut state) = self.states.get_mut(platform_id) {
            state.status = RegistrationStatus::Registered;
            state.last_register = Some(Utc::now());
            state.retry_count = 0;
            tracing::info!("Cascade platform {} registered successfully", platform_id);
        }
    }

    pub fn mark_failed(&self, platform_id: &str) {
        if let Some(mut state) = self.states.get_mut(platform_id) {
            state.retry_count += 1;
            if state.retry_count >= state.max_retries {
                state.status = RegistrationStatus::Failed;
                tracing::error!("Cascade platform {} registration failed after {} retries", platform_id, state.retry_count);
            } else {
                state.status = RegistrationStatus::NotRegistered;
                tracing::warn!("Cascade platform {} registration failed, retry {}/{}", 
                    platform_id, state.retry_count, state.max_retries);
            }
        }
    }

    pub fn build_register_request(&self, platform_id: &str, cseq: u32, expires: u32) -> Option<String> {
        let state = self.states.get(platform_id)?;
        let branch = format!("z9hG4bK{}", rand_nonce());
        let call_id = format!("cascade_{}_{}", platform_id, rand_nonce());
        let tag = format!("local_{}", rand_nonce());

        let auth_header = if state.status == RegistrationStatus::Challenged {
            if let Some(ref nonce) = state.nonce {
                let ha1 = md5_hex(&format!("{}:{}:{}", state.local_device_id, state.realm, state.password));
                let ha2 = md5_hex(&format!("REGISTER:sip:{}@{}:{}", state.platform_device_id, state.host, state.port));
                let response = md5_hex(&format!("{}:{}:{}", ha1, nonce, ha2));

                let mut auth = format!(
                    "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"sip:{}@{}:{}\", response=\"{}\"",
                    state.local_device_id, state.realm, nonce,
                    state.platform_device_id, state.host, state.port, response
                );
                if let Some(ref opaque) = state.opaque {
                    auth.push_str(&format!(", opaque=\"{}\"", opaque));
                }
                Some(auth)
            } else {
                None
            }
        } else {
            None
        };

        let mut headers = format!(
            "REGISTER sip:{}@{}:{} SIP/2.0\r\n\
             Via: SIP/2.0/UDP {}:{};rport;branch={}\r\n\
             From: <sip:{}@{}:{}>;tag={}\r\n\
             To: <sip:{}@{}:{}>\r\n\
             Call-ID: {}\r\n\
             CSeq: {} REGISTER\r\n\
             Max-Forwards: 70\r\n\
             Expires: {}\r\n\
             User-Agent: GBServer/1.0\r\n",
            state.platform_device_id, state.host, state.port,
            state.local_device_id, 5060, branch,
            state.local_device_id, state.local_device_id, 5060, tag,
            state.platform_device_id, state.host, state.port,
            call_id, cseq, expires
        );

        if let Some(auth) = auth_header {
            headers.push_str(&format!("Authorization: {}\r\n", auth));
        }

        headers.push_str("Content-Length: 0\r\n\r\n");
        Some(headers)
    }

    pub async fn load_platforms_from_db(&self, pool: &crate::db::Pool, local_device_id: &str, realm: &str) {
        // Also store the pool for later use
        self.set_pool(pool.clone()).await;
        match crate::db::platform::list_platforms(pool).await {
            Ok(platforms) => {
                for p in platforms {
                    if p.enable.unwrap_or(false) {
                        let server_gb_id = p.server_gb_id.clone().unwrap_or_default();
                        let server_ip = p.server_ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());
                        let server_port = p.server_port.unwrap_or(5060) as u16;
                        let password = p.password.clone().unwrap_or_default();
                        let expires_secs: u64 = p.expires.as_deref()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(3600);
                        let platform_id = p.device_gb_id.as_deref().unwrap_or(&server_gb_id).to_string();

                        self.add_platform(
                            &platform_id,
                            &server_gb_id,
                            &server_ip,
                            server_port,
                            local_device_id,
                            &password,
                            realm,
                            expires_secs,
                        );
                    }
                }
                tracing::info!("Loaded {} cascade platforms from DB", self.states.len());
            }
            Err(e) => {
                tracing::warn!("Failed to load cascade platforms: {}", e);
            }
        }
    }

    pub async fn run_registration_loop(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            interval.tick().await;

            // 第一阶段：仅持读锁遍历收集待 REGISTER 平台。
            // DashMap::iter() 在迭代期间持有所有 shard 的读锁，因此必须先把
            // 目标平台的信息收集到 Vec 并 drop 迭代器，再走 get_mut 拿写锁
            // 与 send_to 网络 I/O —— 否则 iter 的读锁与 get_mut 的写锁会自死锁。
            let to_register: Vec<(String, String, u16, u64)> = self
                .states
                .iter()
                .filter(|entry| match entry.status {
                    RegistrationStatus::NotRegistered | RegistrationStatus::Failed => true,
                    RegistrationStatus::Registered => match entry.last_register {
                        Some(last) => {
                            let elapsed = (Utc::now() - last).num_seconds() as u64;
                            elapsed >= entry.register_interval_secs / 2
                        }
                        None => true,
                    },
                    _ => false,
                })
                .map(|entry| {
                    (
                        entry.platform_id.clone(),
                        entry.host.clone(),
                        entry.port,
                        entry.register_interval_secs,
                    )
                })
                .collect();

            // 第二阶段：迭代器已 drop，可安全调用 get_mut 与 send_to。
            for (platform_id, host, port, register_interval_secs) in to_register {
                let sip_server = match self.get_sip_server().await {
                    Some(s) => s,
                    None => continue,
                };

                let sip = sip_server.read().await;
                let socket_arc = sip.socket().clone();
                drop(sip);

                let socket_guard = socket_arc.read().await;
                let udp_socket = match socket_guard.as_ref() {
                    Some(s) => s,
                    None => continue,
                };

                let cseq = 1u32;
                let expires = register_interval_secs as u32;

                let register_msg = match self.build_register_request(&platform_id, cseq, expires) {
                    Some(m) => m,
                    None => continue,
                };

                let ip = match host.parse::<std::net::IpAddr>() {
                    Ok(ip) => ip,
                    Err(_) => continue,
                };
                let addr = std::net::SocketAddr::new(ip, port);

                match udp_socket.send_to(register_msg.as_bytes(), addr).await {
                    Ok(_) => {
                        tracing::debug!("Sent REGISTER to cascade platform {}", platform_id);
                        if let Some(mut state) = self.states.get_mut(&platform_id) {
                            if state.status != RegistrationStatus::Challenged {
                                state.status = RegistrationStatus::Registering;
                            }
                            state.last_register = Some(Utc::now());
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to send REGISTER to {}: {}", platform_id, e);
                        self.mark_failed(&platform_id);
                    }
                }
            }
        }
    }
}

impl Default for CascadeRegistrar {
    fn default() -> Self {
        Self::new()
    }
}

/// C3: 周期任务入口（lib.rs 中 `tokio::spawn` 启动）
///
/// - 每 30s 调一次 `reload_from_db`，拾起新增 / 移除 / 配置变更
/// - 每 30s 给所有 Registered 平台发 Keepalive
/// - 每 60s 检测超时平台，标记为 Failed 让 `run_registration_loop` 触发重试
pub async fn cascade_periodic_tasks(
    registrar: Arc<CascadeRegistrar>,
    local_device_id: String,
    realm: String,
) {
    let mut keepalive_tick = tokio::time::interval(std::time::Duration::from_secs(30));
    let mut reload_tick = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut timeout_tick = tokio::time::interval(std::time::Duration::from_secs(60));

    // 第一次 tick 立即触发 → 等待
    keepalive_tick.tick().await;
    reload_tick.tick().await;
    timeout_tick.tick().await;

    loop {
        tokio::select! {
            _ = keepalive_tick.tick() => {
                registrar.send_keepalive_all().await;
            }
            _ = reload_tick.tick() => {
                let (added, removed) = registrar.reload_from_db(&local_device_id, &realm).await;
                if !added.is_empty() {
                    tracing::info!("Cascade reload added platforms: {:?}", added);
                }
                if !removed.is_empty() {
                    tracing::info!("Cascade reload removed platforms: {:?}", removed);
                }
            }
            _ = timeout_tick.tick() => {
                let timed_out = registrar.detect_keepalive_timeouts(3); // 3 次 keepalive 未响应
                if !timed_out.is_empty() {
                    tracing::warn!("Cascade platforms marked Failed (keepalive timeout): {:?}", timed_out);
                }
            }
        }
    }
}

fn rand_nonce() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:08x}{:08x}", rng.gen::<u32>(), rng.gen::<u32>())
}

fn md5_hex(input: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod c3_tests {
    use super::*;

    fn make_registrar() -> CascadeRegistrar {
        CascadeRegistrar::new()
    }

    /// C3.1: 401 鉴权挑战 → NotRegistered → Challenged → Registered → digest 正确计算
    #[test]
    fn c3_401_digest_full_cycle() {
        let r = make_registrar();
        r.add_platform("plat-401", "37010000002000000001", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);
        assert_eq!(r.get_status("plat-401"), Some(RegistrationStatus::NotRegistered));

        // 收到 401 挑战
        r.handle_401_challenge("plat-401", "nonce_abc", Some("opaque_xyz"), "GBServer");
        assert_eq!(r.get_status("plat-401"), Some(RegistrationStatus::Challenged));
        let s = r.state("plat-401").unwrap();
        assert_eq!(s.nonce.as_deref(), Some("nonce_abc"));
        assert_eq!(s.opaque.as_deref(), Some("opaque_xyz"));

        // 构造带 digest 的 REGISTER 报文
        let msg = r.build_register_request("plat-401", 2, 3600).unwrap();
        assert!(msg.contains("Authorization: Digest"));
        assert!(msg.contains("nonce=\"nonce_abc\""));
        assert!(msg.contains("opaque=\"opaque_xyz\""));
        assert!(msg.contains("realm=\"GBServer\""));
        // response 必须是 32 字符 md5 hex
        assert!(msg.contains("response=\""));
        let resp_start = msg.find("response=\"").unwrap() + 10;
        let resp_end = msg[resp_start..].find('"').unwrap();
        assert_eq!(resp_end, 32);

        // 注册成功
        r.mark_registered("plat-401");
        assert_eq!(r.get_status("plat-401"), Some(RegistrationStatus::Registered));
        assert_eq!(r.state("plat-401").unwrap().retry_count, 0);
    }

    /// C3.2: Failed → NotRegistered 重试，retry_count 单调递增
    #[test]
    fn c3_failed_to_retry_state_machine() {
        let r = make_registrar();
        r.add_platform("plat-retry", "37010000002000000002", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);

        r.mark_failed("plat-retry");
        let s1 = r.state("plat-retry").unwrap();
        assert_eq!(s1.status, RegistrationStatus::NotRegistered);
        assert_eq!(s1.retry_count, 1);

        r.mark_failed("plat-retry");
        r.mark_failed("plat-retry");
        let s3 = r.state("plat-retry").unwrap();
        assert_eq!(s3.status, RegistrationStatus::NotRegistered);
        assert_eq!(s3.retry_count, 3);

        // 达到 max_retries (5) → Failed
        r.mark_failed("plat-retry");
        r.mark_failed("plat-retry");
        let s5 = r.state("plat-retry").unwrap();
        assert_eq!(s5.status, RegistrationStatus::Failed);
        assert_eq!(s5.retry_count, 5);

        // 注册成功后 retry_count 归零
        r.mark_registered("plat-retry");
        assert_eq!(r.state("plat-retry").unwrap().retry_count, 0);
    }

    /// C3.2: Keepalive 超时检测：Registered 超过阈值未被 liveness 更新 → Failed
    #[test]
    fn c3_keepalive_timeout_detection() {
        let r = make_registrar();
        r.add_platform("plat-ka", "37010000002000000003", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);
        r.mark_registered("plat-ka");

        // 模拟 10 分钟前最后活跃
        let old = Utc::now() - chrono::Duration::seconds(600);
        if let Some(mut s) = r.states.get_mut("plat-ka") {
            s.last_register = Some(old);
        }

        // 阈值 3 次（180s）→ 600s 超时
        let timed_out = r.detect_keepalive_timeouts(3);
        assert_eq!(timed_out, vec!["plat-ka".to_string()]);
        assert_eq!(r.get_status("plat-ka"), Some(RegistrationStatus::Failed));
    }

    /// C3.2: note_liveness 推回 last_register，阻止超时
    #[test]
    fn c3_note_liveness_resets_timer() {
        let r = make_registrar();
        r.add_platform("plat-live", "37010000002000000004", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);
        r.mark_registered("plat-live");

        // 5 分钟前
        if let Some(mut s) = r.states.get_mut("plat-live") {
            s.last_register = Some(Utc::now() - chrono::Duration::seconds(300));
        }
        // 检测会超时
        assert_eq!(r.detect_keepalive_timeouts(2), vec!["plat-live".to_string()]);
        // 重新 mark_registered 后再 note_liveness
        r.mark_registered("plat-live");
        r.note_liveness("plat-live");
        let live = r.state("plat-live").unwrap();
        let elapsed = (Utc::now() - live.last_register.unwrap()).num_seconds();
        assert!(elapsed < 5, "note_liveness 应该重置 last_register");
    }

    /// C3.3: remove_platform → 完全移除（disable 路径）
    #[test]
    fn c3_disable_removes_session() {
        let r = make_registrar();
        r.add_platform("plat-dis", "37010000002000000005", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);
        r.mark_registered("plat-dis");
        assert!(r.get_status("plat-dis").is_some());

        r.remove_platform("plat-dis");
        assert_eq!(r.get_status("plat-dis"), None);
        assert_eq!(r.get_all_states().len(), 0);
    }

    /// C3.1: build_keepalive_message 包含必要字段
    #[test]
    fn c3_build_keepalive_message_format() {
        let r = make_registrar();
        r.add_platform("plat-msg", "37010000002000000006", "192.168.1.10", 5061,
                       "34020000002000000001", "secret", "GBServer", 3600);
        let msg = r.build_keepalive_message("plat-msg", 123).unwrap();
        assert!(msg.starts_with("MESSAGE sip:37010000002000000006@192.168.1.10:5061 SIP/2.0"));
        assert!(msg.contains("CSeq: 123 MESSAGE"));
        assert!(msg.contains("Content-Type: APPLICATION/MANSCDP+XML"));
        assert!(msg.contains("<CmdType>Keepalive</CmdType>"));
        assert!(msg.contains("<DeviceID>34020000002000000001</DeviceID>"));
        assert!(msg.contains("<SN>123</SN>"));
    }

    /// C3.2: send_keepalive_all 仅作用于 Registered 平台（无 panic / 无副作用）
    #[tokio::test]
    async fn c3_send_keepalive_all_skips_unregistered() {
        let r = make_registrar();
        r.add_platform("plat-noreg", "37010000002000000007", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);
        // 没 mark_registered → NotRegistered → 不发送
        r.send_keepalive_all().await; // 应立即返回，不 panic
        let s = r.state("plat-noreg").unwrap();
        assert_eq!(s.status, RegistrationStatus::NotRegistered);
    }

    /// C3.2: detect_keepalive_timeouts 跳过 NotRegistered / Failed 平台
    #[test]
    fn c3_timeout_detection_skips_non_registered() {
        let r = make_registrar();
        r.add_platform("plat-nr", "37010000002000000008", "127.0.0.1", 5060,
                       "34020000002000000001", "secret", "GBServer", 3600);
        // 默认 NotRegistered + last_register=None
        let timed_out = r.detect_keepalive_timeouts(0); // 阈值 0
        // None 状态视为无限远，应当被报告
        // 但因为状态不是 Registered → 跳过
        assert!(timed_out.is_empty());
    }
}
