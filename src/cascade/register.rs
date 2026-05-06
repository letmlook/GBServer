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

            for entry in self.states.iter() {
                let should_register = match entry.status {
                    RegistrationStatus::NotRegistered | RegistrationStatus::Failed => true,
                    RegistrationStatus::Registered => {
                        if let Some(last) = entry.last_register {
                            let elapsed = (Utc::now() - last).num_seconds() as u64;
                            elapsed >= entry.register_interval_secs / 2
                        } else {
                            true
                        }
                    }
                    _ => false,
                };

                if should_register {
                    if let Some(sip_server) = self.get_sip_server().await {
                        let platform_id = entry.platform_id.clone();
                        let _platform_device_id = entry.platform_device_id.clone();
                        let host = entry.host.clone();
                        let port = entry.port;
                        let _local_device_id = entry.local_device_id.clone();
                        let register_interval = entry.register_interval_secs;

                        let sip = sip_server.read().await;
                        let socket_arc = sip.socket().clone();
                        drop(sip);

                        let socket_guard = socket_arc.read().await;
                        if let Some(ref udp_socket) = *socket_guard {
                            let cseq = 1u32;
                            let expires = register_interval as u32;

                            if let Some(register_msg) = self.build_register_request(&platform_id, cseq, expires) {
                                let addr_result = host.parse::<std::net::IpAddr>();
                                if let Ok(ip) = addr_result {
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
