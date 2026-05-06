use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use std::time::Instant;

use crate::jt1078::session::Jt1078Session;
use crate::jt1078::command;

#[derive(Clone)]
pub struct Jt1078Manager {
    sessions: Arc<Mutex<HashMap<SocketAddr, Jt1078Session>>>,
    /// Phone number → SocketAddr mapping for command dispatch
    terminal_addrs: Arc<Mutex<HashMap<String, SocketAddr>>>,
    /// Per-session sequence counter
    seq_counters: Arc<Mutex<HashMap<SocketAddr, u16>>>,
    timeout: Duration,
    retransmit_wait: Duration,
    retransmit_hook: Option<String>,
    retransmit_send_to_device: bool,
}

impl Jt1078Manager {
    pub fn new(timeout: Duration, retransmit_wait: Duration, retransmit_hook: Option<String>, retransmit_send_to_device: bool) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            terminal_addrs: Arc::new(Mutex::new(HashMap::new())),
            seq_counters: Arc::new(Mutex::new(HashMap::new())),
            timeout,
            retransmit_wait,
            retransmit_hook,
            retransmit_send_to_device,
        }
    }

    /// Feed bytes from a peer address into its session, creating the session if needed.
    pub async fn feed_bytes(&self, addr: SocketAddr, data: &[u8]) -> Vec<Vec<u8>> {
        let mut map = self.sessions.lock().await;
        let frames = {
            let session = map.entry(addr).or_insert_with(|| Jt1078Session::new(addr));
            session.last_heartbeat = Instant::now();
            session.feed_bytes(data)
        };
        crate::metrics::set_active_sessions(map.len());
        frames
    }

    pub async fn remove(&self, addr: &SocketAddr) {
        let mut map = self.sessions.lock().await;
        map.remove(addr);
        // Also remove from terminal registry
        let mut terms = self.terminal_addrs.lock().await;
        terms.retain(|_, a| a != addr);
    }

    pub async fn count(&self) -> usize {
        self.sessions.lock().await.len()
    }

    /// Register a phone number for a connected terminal so commands can be sent to it.
    pub async fn register_terminal(&self, phone: &str, addr: SocketAddr) {
        self.terminal_addrs.lock().await.insert(phone.to_string(), addr);
    }

    /// Get the SocketAddr for a registered terminal by phone number.
    pub async fn get_terminal_addr(&self, phone: &str) -> Option<SocketAddr> {
        self.terminal_addrs.lock().await.get(phone).copied()
    }

    /// Check if a terminal is connected (has an active session and address).
    pub async fn is_terminal_online(&self, phone: &str) -> bool {
        if let Some(addr) = self.get_terminal_addr(phone).await {
            let map = self.sessions.lock().await;
            map.contains_key(&addr)
        } else {
            false
        }
    }

    /// Get next sequence number for an address.
    async fn next_seq(&self, addr: SocketAddr) -> u16 {
        let mut counters = self.seq_counters.lock().await;
        let seq = counters.entry(addr).or_insert(0);
        let current = *seq;
        *seq = seq.wrapping_add(1);
        current
    }

    /// Send a raw byte payload to a connected terminal by phone number.
    pub async fn send_raw(&self, phone: &str, data: &[u8]) -> Result<(), String> {
        let addr = self.get_terminal_addr(phone).await
            .ok_or_else(|| format!("终端 {} 未连接", phone))?;

        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("绑定UDP失败: {}", e))?;
        socket.send_to(data, addr)
            .await
            .map_err(|e| format!("发送失败: {}", e))?;
        Ok(())
    }

    /// Send a JT808-framed command to a terminal.
    pub async fn send_command(&self, phone: &str, msg_id: u16, body: &[u8]) -> Result<(), String> {
        let addr = self.get_terminal_addr(phone).await
            .ok_or_else(|| format!("终端 {} 未连接", phone))?;
        let seq = self.next_seq(addr).await;
        let frame = command::build_jt808_frame(msg_id, phone, seq, body);
        self.send_raw(phone, &frame).await
    }

    /// Send a PTZ control command (0x9301)
    pub async fn send_ptz(&self, phone: &str, channel_id: u8, direction: &str, speed: u8) -> Result<(), String> {
        let (b1, b2, h, v) = command::ptz_direction_bytes(direction, speed);
        let body = command::build_ptz_control(channel_id, b1, b2, h, v, 0);
        self.send_command(phone, 0x9301, &body).await
    }

    /// Send live video start/stop (0x9101)
    pub async fn send_live_video(&self, phone: &str, channel_id: u8, stream_type: u8, close: bool) -> Result<(), String> {
        let body = command::build_live_video_request(channel_id, stream_type, close);
        self.send_command(phone, 0x9101, &body).await
    }

    /// Send live video control (0x9102)
    pub async fn send_live_video_control(&self, phone: &str, channel_id: u8, control: u8, close: bool) -> Result<(), String> {
        let body = command::build_live_video_control(channel_id, control, close);
        self.send_command(phone, 0x9102, &body).await
    }

    /// Send playback request (0x9201)
    pub async fn send_playback(&self, phone: &str, channel_id: u8, stream_type: u8, storage_type: u8,
        speed: u8, start_time: &str, end_time: &str) -> Result<(), String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_playback_request(channel_id, stream_type, storage_type, 0, speed, &st, &et);
        self.send_command(phone, 0x9201, &body).await
    }

    /// Send playback control (0x9202)
    pub async fn send_playback_control(&self, phone: &str, channel_id: u8, control: u8, speed: u8, seek_time: &str) -> Result<(), String> {
        let st = command::encode_time_bcd(seek_time);
        let body = command::build_playback_control(channel_id, control, speed, &st);
        self.send_command(phone, 0x9202, &body).await
    }

    /// Send wiper control
    pub async fn send_wiper(&self, phone: &str, on: bool) -> Result<(), String> {
        let body = command::build_wiper_control(on);
        self.send_command(phone, 0x8103, &body).await
    }

    /// Send fill light control
    pub async fn send_fill_light(&self, phone: &str, on: bool) -> Result<(), String> {
        let body = command::build_fill_light_control(on);
        self.send_command(phone, 0x8103, &body).await
    }

    /// Send terminal control (reset, factory reset)
    pub async fn send_terminal_control(&self, phone: &str, cmd: u8) -> Result<(), String> {
        let body = command::build_terminal_control(cmd);
        self.send_command(phone, 0x8105, &body).await
    }

    /// Send text message to terminal (0x8300)
    pub async fn send_text_message(&self, phone: &str, text: &str, emergency: bool) -> Result<(), String> {
        let body = command::build_text_message(text, emergency);
        self.send_command(phone, 0x8300, &body).await
    }

    /// Send phone callback (0x8400)
    pub async fn send_phone_callback(&self, phone: &str, sign: u8, dest_phone: &str) -> Result<(), String> {
        let body = command::build_phone_callback(sign, dest_phone);
        self.send_command(phone, 0x8400, &body).await
    }

    /// Send vehicle control (door) (0x8500)
    pub async fn send_vehicle_control(&self, phone: &str, control_type: u8, value: bool) -> Result<(), String> {
        let body = command::build_vehicle_control(control_type, value);
        self.send_command(phone, 0x8500, &body).await
    }

    /// Send take photo command (0x8801)
    pub async fn send_take_photo(&self, phone: &str, channel_id: u8) -> Result<(), String> {
        let body = command::build_take_photo(channel_id, 0x0001, 5, 0, 0x02, 0x05, 0x80, 0x80, 0x80, 0x80);
        self.send_command(phone, 0x8801, &body).await
    }

    /// Send media search (0x8802)
    pub async fn send_media_search(&self, phone: &str, channel_id: u8, start_time: &str, end_time: &str) -> Result<(), String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_media_search(0, channel_id, 0, &st, &et);
        self.send_command(phone, 0x8802, &body).await
    }

    /// Send media upload command (0x8803)
    pub async fn send_media_upload(&self, phone: &str, media_id: u32) -> Result<(), String> {
        let body = command::build_media_upload(media_id, 0);
        self.send_command(phone, 0x8803, &body).await
    }

    /// Send set phone book (0x8401)
    pub async fn send_set_phone_book(&self, phone: &str, contacts: &[(String, String)]) -> Result<(), String> {
        let body = command::build_set_phone_book(contacts);
        self.send_command(phone, 0x8401, &body).await
    }

    /// Send query attributes (0x8106)
    pub async fn send_query_attributes(&self, phone: &str) -> Result<(), String> {
        let body = command::build_query_attributes();
        self.send_command(phone, 0x8106, &body).await
    }

    /// Send set params (0x8103)
    pub async fn send_set_params(&self, phone: &str, apn: &str, ip: &str, port: u16) -> Result<(), String> {
        let port_bytes = port.to_be_bytes();
        let param_data = [
            (0x0010u32, apn.as_bytes()),
            (0x0013u32, ip.as_bytes()),
            (0x0018u32, port_bytes.as_slice()),
        ];
        let body = command::build_set_params(&param_data);
        self.send_command(phone, 0x8103, &body).await
    }

    /// Send query location (0x8201)
    pub async fn send_query_location(&self, phone: &str) -> Result<(), String> {
        self.send_command(phone, 0x8201, &[]).await
    }

    /// Send location report trigger (0x8203)
    pub async fn send_location_trigger(&self, phone: &str) -> Result<(), String> {
        self.send_command(phone, 0x8203, &[]).await
    }

    /// Send connection control - switch server address
    pub async fn send_connection_control(&self, phone: &str, ip: &str, port: u16) -> Result<(), String> {
        let ip_parts: Vec<u8> = ip.split('.').filter_map(|s| s.parse().ok()).collect();
        if ip_parts.len() != 4 {
            return Err("无效IP地址".to_string());
        }
        let mut body = vec![ip_parts[0], ip_parts[1], ip_parts[2], ip_parts[3]];
        body.extend_from_slice(&port.to_be_bytes());
        self.send_command(phone, 0x8103, &body).await
    }

    pub async fn process_payload_for(&self, addr: SocketAddr, payload: &[u8]) -> crate::jt1078::session::FrameKind {
        let mut map = self.sessions.lock().await;
        let session = map.entry(addr).or_insert_with(|| Jt1078Session::new(addr));
        session.process_payload(payload)
    }

    pub async fn cleanup_once(&self) -> usize {
        let mut map = self.sessions.lock().await;
        let now = Instant::now();
        let mut removed_addrs = Vec::new();
        for (k, v) in map.iter() {
            if now.duration_since(v.last_heartbeat) > self.timeout {
                removed_addrs.push(*k);
            }
        }
        for k in removed_addrs.iter() {
            map.remove(k);
        }
        // Also clean terminal registry
        if !removed_addrs.is_empty() {
            let mut terms = self.terminal_addrs.lock().await;
            terms.retain(|_, a| !removed_addrs.contains(a));
        }
        removed_addrs.len()
    }

    pub async fn cleanup_loop(&self, tick: Duration) {
        loop {
            tokio::time::sleep(tick).await;
            let removed = self.cleanup_once().await;
            if removed > 0 {
                tracing::info!("JT1078 manager cleanup removed {} timed-out sessions", removed);
            }

            let mut map = self.sessions.lock().await;
            for (addr, sess) in map.iter_mut() {
                let timed_out = sess.collect_timed_out_missing(self.retransmit_wait);
                if !timed_out.is_empty() {
                    tracing::warn!("JT1078 missing sequences timed out for {}: {:?}", addr, timed_out);
                    if sess.should_trigger_missing_alert(self.retransmit_wait) {
                        tracing::warn!("JT1078 missing seqs for {}: {:?}", addr, timed_out);
                        crate::metrics::inc_missing(timed_out.len() as u64);
                        if let Some(hook) = &self.retransmit_hook {
                            let addr_s = addr.to_string();
                            let missing = timed_out.clone();
                            let hook_url = hook.clone();
                            tokio::spawn(async move {
                                let client = reqwest::Client::new();
                                #[derive(serde::Serialize)]
                                struct MissingReport { addr: String, missing: Vec<u16>, timestamp_ms: u128 }
                                let report = MissingReport { addr: addr_s, missing, timestamp_ms: chrono::Utc::now().timestamp_millis() as u128 };
                                let _ = client.post(&hook_url).json(&report).send().await;
                            });
                        }
                        if self.retransmit_send_to_device {
                            let addr_copy = *addr;
                            let missing_copy = timed_out.clone();
                            tokio::spawn(async move {
                                let _ = Jt1078Manager::send_retransmit_request(addr_copy, missing_copy).await;
                            });
                        }
                    }
                }
            }
        }
    }

    pub async fn send_retransmit_request(addr: SocketAddr, missing: Vec<u16>) -> anyhow::Result<()> {
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
        let msg = serde_json::json!({ "type": "retransmit_request", "missing": missing });
        let data = msg.to_string().into_bytes();
        let _ = socket.send_to(&data, addr).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    #[tokio::test]
    async fn test_feed_and_count_and_cleanup() {
        let manager = Jt1078Manager::new(Duration::from_millis(100), Duration::from_millis(200), None, false);
        let addr1 = make_addr(60001);
        let payload = b"abc";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);

        let frames = manager.feed_bytes(addr1, &buf).await;
        assert_eq!(frames.len(), 1);
        assert_eq!(manager.count().await, 1);

        tokio::time::sleep(Duration::from_millis(200)).await;
        let removed = manager.cleanup_once().await;
        assert_eq!(removed, 1);
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_terminal_registry() {
        let manager = Jt1078Manager::new(Duration::from_secs(60), Duration::from_millis(200), None, false);
        let addr = make_addr(60002);
        // Feed bytes to create a session first, then register terminal
        let payload = b"test";
        let len = (payload.len() as u32).to_be_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&len);
        buf.extend_from_slice(payload);
        manager.feed_bytes(addr, &buf).await;
        manager.register_terminal("13812345678", addr).await;
        assert!(manager.is_terminal_online("13812345678").await);
        manager.remove(&addr).await;
        assert!(!manager.is_terminal_online("13812345678").await);
    }
}
