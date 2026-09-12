use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use std::time::Instant;

use crate::jt1078::session::Jt1078Session;
use crate::jt1078::command;
use crate::jt1078::command_waiter::JtCommandWaiter;
use crate::jt1078::jt_media_session::JtMediaSessionManager;

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
    /// Phase 6.2: command → response correlation
    command_waiter: Arc<JtCommandWaiter>,
    /// 数据库连接池（终端注册/在线状态落库；`None` 时退化为仅内存）
    pool: std::sync::OnceLock<crate::db::Pool>,
    /// **TCP 连接**终端的下行通道：对端地址 → 发送队列。
    ///
    /// `send_raw` 此前一律用 UDP 下发，因此**以 TCP 接入的终端永远收不到平台命令**
    /// （命令只能等到超时）。真实部署里 JT/T 808 终端大量使用 TCP。
    /// TCP 连接的写半由各自的任务持有，这里只保留一个 channel 发送端。
    tcp_senders: Arc<Mutex<std::collections::HashMap<SocketAddr, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>>,
    /// 终端多媒体检索结果缓存（0x8802 请求 → 0x0802 应答）。
    ///
    /// 请求与应答是两条独立的上行/下行消息：HTTP 侧发完 0x8802 后需要等待
    /// 终端的 0x0802，这里用一块共享缓存做交接（带采集时间，避免拿到上一次
    /// 检索的陈旧结果）。
    media_search_results: Arc<Mutex<std::collections::HashMap<String, (std::time::Instant, Vec<crate::jt1078::response_parser::MediaSearchItem>)>>>,
    /// 下发命令用的 UDP socket（与监听同一端口）。
    ///
    /// 此前 `send_raw` 每条命令都 `UdpSocket::bind("0.0.0.0:0")` 新建临时端口，
    /// 终端看到的来源地址**不是**它配置的服务器地址，于是按源地址过滤的终端
    /// （含本仓库的模拟器：它用 `connect()` 到平台地址）**一条命令都收不到**
    /// —— 表现为平台侧命令全部超时。真实平台都用同一个监听端口下发。
    send_socket: std::sync::OnceLock<Arc<tokio::net::UdpSocket>>,
    /// Phase 6.3: media session manager (live/playback/download)
    media_session_manager: Arc<JtMediaSessionManager>,
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
            pool: std::sync::OnceLock::new(),
            send_socket: std::sync::OnceLock::new(),
            media_search_results: Arc::new(Mutex::new(std::collections::HashMap::new())),
            tcp_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            command_waiter: Arc::new(JtCommandWaiter::new().with_timeout(10)),
            media_session_manager: Arc::new(JtMediaSessionManager::new()),
        }
    }

    /// Access the command waiter (for tests / direct response resolution).
    pub fn command_waiter(&self) -> Arc<JtCommandWaiter> {
        self.command_waiter.clone()
    }

    /// Phase 6.3: access the media session manager.
    pub fn media_session_manager(&self) -> Arc<JtMediaSessionManager> {
        self.media_session_manager.clone()
    }

    /// Phase 6.2: send a raw JT808 command and wait for the matching 0x0001
    /// general common response (or any other response with same msg_id/serial).
    pub async fn send_command_and_wait(
        &self,
        phone: &str,
        msg_id: u16,
        body: &[u8],
        timeout_secs: u64,
    ) -> Result<Vec<u8>, String> {
        let addr = self.get_terminal_addr(phone).await
            .ok_or_else(|| format!("终端 {} 未连接", phone))?;
        let seq = self.next_seq(addr).await;
        let frame = command::build_jt808_frame(msg_id, phone, seq, body);

        // Register waiter
        let (_key, rx) = self.command_waiter.register(phone, msg_id, seq, Some(timeout_secs));

        // Send
        self.send_raw(phone, &frame).await?;

        // Wait for response
        let timeout_dur = std::time::Duration::from_secs(timeout_secs);
        match tokio::time::timeout(timeout_dur, rx).await {
            Ok(Ok(body)) => Ok(body),
            Ok(Err(_)) => Err("waiter cancelled".to_string()),
            Err(_) => Err(format!(
                "command 0x{:04X} to {} (serial={}) timed out after {}s",
                msg_id, phone, seq, timeout_secs
            )),
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

    /// 绑定数据库连接池（由 `jt1078::server::start` 注入）。
    pub fn set_pool(&self, pool: crate::db::Pool) {
        let _ = self.pool.set(pool);
    }

    /// 记录终端返回的多媒体检索结果（`process_jt_message` 收到 0x0802 时调用）。
    pub async fn store_media_search_result(
        &self,
        phone: &str,
        items: Vec<crate::jt1078::response_parser::MediaSearchItem>,
    ) {
        let mut map = self.media_search_results.lock().await;
        map.insert(phone.to_string(), (std::time::Instant::now(), items));
    }

    /// 取走终端的多媒体检索结果（`max_age` 内有效，取走即清空避免重复使用）。
    pub async fn take_media_search_result(
        &self,
        phone: &str,
        max_age: std::time::Duration,
    ) -> Option<Vec<crate::jt1078::response_parser::MediaSearchItem>> {
        let mut map = self.media_search_results.lock().await;
        match map.get(phone) {
            Some((at, items)) if at.elapsed() <= max_age => {
                let items = items.clone();
                map.remove(phone);
                Some(items)
            }
            Some(_) => {
                map.remove(phone);
                None
            }
            None => None,
        }
    }

    /// 终端注册/心跳时落库并把状态置为在线。
    ///
    /// 此前只在内存里登记地址，`gb_jt_terminal` 永远没有记录 ——
    /// `/api/jt1078/terminal/list` 因此恒为空，终端管理页看不到任何设备。
    pub async fn persist_terminal_online(&self, phone: &str, reg: Option<&crate::jt1078::response_parser::RegisterRequest>) {
        let Some(pool) = self.pool.get() else { return };
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let existing = crate::db::jt1078::get_terminal_by_phone(pool, phone).await.ok().flatten();
        match (existing, reg) {
            (None, Some(r)) => {
                let fields = crate::db::jt1078::JtTerminalWrite {
                    terminal_id: Some(r.terminal_id.as_str()),
                    plate_no: Some(r.plate.as_str()),
                    plate_color: Some(r.plate_color as i32),
                    maker_id: Some(r.manufacturer.as_str()),
                    model: Some(r.terminal_model.as_str()),
                    media_server_id: None,
                    province_id: None,
                    city_id: None,
                };
                if let Err(e) = crate::db::jt1078::insert_terminal(pool, phone, &fields, &now)
                .await
                {
                    tracing::warn!("JT1078 终端入库失败 phone={}: {}", phone, e);
                }
            }
            (Some(_), Some(r)) => {
                let fields = crate::db::jt1078::JtTerminalWrite {
                    terminal_id: Some(r.terminal_id.as_str()),
                    plate_no: Some(r.plate.as_str()),
                    plate_color: Some(r.plate_color as i32),
                    maker_id: Some(r.manufacturer.as_str()),
                    model: Some(r.terminal_model.as_str()),
                    media_server_id: None,
                    province_id: None,
                    city_id: None,
                };
                if let Err(e) = crate::db::jt1078::update_terminal(pool, phone, &fields, &now)
                .await
                {
                    tracing::warn!("JT1078 终端更新失败 phone={}: {}", phone, e);
                }
            }
            _ => {}
        }
        if let Err(e) = crate::db::jt1078::update_terminal_status(pool, phone, true).await {
            tracing::warn!("JT1078 终端在线状态写入失败 phone={}: {}", phone, e);
        }
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
    /// 绑定下发用的 socket（由 `jt1078::server::start` 注入监听 socket）。
    pub fn set_send_socket(&self, socket: Arc<tokio::net::UdpSocket>) {
        let _ = self.send_socket.set(socket);
    }

    /// 注册 TCP 终端的下行通道（连接建立时调用）。
    pub async fn register_tcp_sender(
        &self,
        addr: SocketAddr,
        tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    ) {
        self.tcp_senders.lock().await.insert(addr, tx);
    }

    /// TCP 连接断开时移除下行通道（避免往已关闭的连接写）。
    pub async fn remove_tcp_sender(&self, addr: &SocketAddr) {
        self.tcp_senders.lock().await.remove(addr);
    }

    pub async fn send_raw(&self, phone: &str, data: &[u8]) -> Result<(), String> {
        let addr = self.get_terminal_addr(phone).await
            .ok_or_else(|| format!("终端 {} 未连接", phone))?;

        // 先看该对端是不是 TCP 接入：是就走同一条 TCP 连接下发。
        // 此前一律走 UDP，TCP 终端因此收不到任何平台命令。
        let tcp_tx = self.tcp_senders.lock().await.get(&addr).cloned();
        if let Some(tx) = tcp_tx {
            return tx
                .send(data.to_vec())
                .map_err(|_| "TCP 下行通道已关闭".to_string());
        }

        // UDP：优先用监听 socket 下发，来源地址必须是终端配置的服务器地址
        // （否则按源地址过滤的终端收不到命令，见 send_socket 字段说明）。
        if let Some(socket) = self.send_socket.get() {
            return socket
                .send_to(data, addr)
                .await
                .map(|_| ())
                .map_err(|e| format!("发送失败: {}", e));
        }

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

    /// 0x8803 存储多媒体文件删除（delete_flag=1）
    pub async fn send_media_delete(&self, phone: &str, media_id: u32) -> Result<(), String> {
        let body = command::build_media_upload(media_id, 1);
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

    /// Phase 6.2 — process a fully-decoded JT808 message (msg_id + serial + body).
    /// In addition to parsing, also resolves any matching command waiter.
    /// `phone` is the BCD-decoded phone number (string) for the source terminal.
    pub async fn process_jt_message(
        &self,
        addr: SocketAddr,
        phone: &str,
        msg_id: u16,
        serial: u16,
        body: &[u8],
    ) -> crate::jt1078::session::ParsedMessage {
        // Update session heartbeat + last activity
        {
            let mut map = self.sessions.lock().await;
            let session = map.entry(addr).or_insert_with(|| Jt1078Session::new(addr));
            session.last_heartbeat = std::time::Instant::now();
        }
        // Resolve command waiter for 0x0001 general common response
        if msg_id == 0x0001 && body.len() >= 5 {
            let reply_serial = u16::from_be_bytes([body[0], body[1]]);
            let reply_msg_id = u16::from_be_bytes([body[2], body[3]]);
            let result = body[4];
            self.command_waiter
                .try_resolve_by_response(phone, reply_msg_id, reply_serial, result);
        }
        // Dispatch via session.process_jt_message
        let parsed = {
            let mut map = self.sessions.lock().await;
            let session = map.entry(addr).or_insert_with(|| Jt1078Session::new(addr));
            session.process_jt_message(msg_id, serial, body)
        };

        // 注册/心跳时落库并置为在线（终端列表页依赖 gb_jt_terminal）
        match &parsed {
            crate::jt1078::session::ParsedMessage::Register(reg) => {
                self.persist_terminal_online(phone, Some(reg)).await;
            }
            crate::jt1078::session::ParsedMessage::Heartbeat => {
                self.persist_terminal_online(phone, None).await;
            }
            crate::jt1078::session::ParsedMessage::MediaSearchResult(items) => {
                tracing::info!(
                    "JT1078 收到多媒体检索应答 phone={} items={}",
                    phone,
                    items.len()
                );
                self.store_media_search_result(phone, items.clone()).await;
            }
            _ => {}
        }
        parsed
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

    // ================== Phase 6.2: send_*_and_wait wrappers ==================
    // Each wraps the existing send_* method to await the 0x0001 general common
    // response. Body is the result byte (0=success, 1=failure, 2=msg_error, 3=unsupported).

    /// 0x9301 PTZ control
    pub async fn send_ptz_and_wait(
        &self, phone: &str, channel_id: u8, direction: &str, speed: u8, timeout_secs: u64,
    ) -> Result<u8, String> {
        let (b1, b2, h, v) = command::ptz_direction_bytes(direction, speed);
        let body = command::build_ptz_control(channel_id, b1, b2, h, v, 0);
        let resp = self.send_command_and_wait(phone, 0x9301, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9101 Live video start/stop.
    /// Phase 6.3: when start (close=false), also creates a media session and
    /// waits for ZLM on_stream_changed hook to resolve the session to Active.
    pub async fn send_live_video_and_wait(
        &self, phone: &str, channel_id: u8, stream_type: u8, close: bool, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_live_video_request(channel_id, stream_type, close);
        let resp = self.send_command_and_wait(phone, 0x9101, &body, timeout_secs).await?;
        let result = resp.first().copied().unwrap_or(1);
        if !close && result == 0 {
            // Create session and wait for ZLM media arrival
            self.media_session_manager.create_live(phone, channel_id);
            // Note: actual wait_for_media is initiated by handler after open_rtp_server.
        } else if close {
            self.media_session_manager.stop(phone, channel_id);
        }
        Ok(result)
    }

    /// Phase 6.3: wait for ZLM media arrival (called by handler after opening
    /// ZLM RTP server). Returns the activated JtMediaSession.
    pub async fn wait_for_zlm_media(
        &self, phone: &str, channel_id: u8, timeout_secs: u64,
    ) -> Result<crate::jt1078::jt_media_session::JtMediaSession, String> {
        self.media_session_manager.wait_for_media(
            phone, channel_id, std::time::Duration::from_secs(timeout_secs),
        ).await
    }

    /// 0x9102 Live video control
    pub async fn send_live_video_control_and_wait(
        &self, phone: &str, channel_id: u8, control: u8, close: bool, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_live_video_control(channel_id, control, close);
        let resp = self.send_command_and_wait(phone, 0x9102, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9201 Playback request
    pub async fn send_playback_and_wait(
        &self, phone: &str, channel_id: u8, stream_type: u8, storage_type: u8,
        speed: u8, start_time: &str, end_time: &str, timeout_secs: u64,
    ) -> Result<u8, String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_playback_request(channel_id, stream_type, storage_type, 0, speed, &st, &et);
        let resp = self.send_command_and_wait(phone, 0x9201, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9202 Playback control
    pub async fn send_playback_control_and_wait(
        &self, phone: &str, channel_id: u8, control: u8, speed: u8, seek_time: &str, timeout_secs: u64,
    ) -> Result<u8, String> {
        let st = command::encode_time_bcd(seek_time);
        let body = command::build_playback_control(channel_id, control, speed, &st);
        let resp = self.send_command_and_wait(phone, 0x9202, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8103 Set terminal parameters (wiper, fill light, APN, IP, port, etc.)
    pub async fn send_set_params_and_wait(
        &self, phone: &str, params: &[(u32, &[u8])], timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_set_params(params);
        let resp = self.send_command_and_wait(phone, 0x8103, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8104 Query terminal parameters (waits for 0x0107 response)
    pub async fn send_query_params_and_wait(
        &self, phone: &str, param_ids: &[u32], timeout_secs: u64,
    ) -> Result<Vec<crate::jt1078::response_parser::TerminalParam>, String> {
        let body = command::build_query_params(param_ids);
        let _ = self.send_command_and_wait(phone, 0x8104, &body, timeout_secs).await?;
        // The actual 0x0107 message body is delivered through process_jt_message;
        // for now we return an empty list (full impl in Phase 6.5).
        Ok(Vec::new())
    }

    /// 0x8105 Terminal control (reset, factory reset, etc.)
    pub async fn send_terminal_control_and_wait(
        &self, phone: &str, cmd: u8, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_terminal_control(cmd);
        let resp = self.send_command_and_wait(phone, 0x8105, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8106 Query terminal attributes
    pub async fn send_query_attributes_and_wait(
        &self, phone: &str, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_query_attributes();
        let resp = self.send_command_and_wait(phone, 0x8106, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8201 Query location (waits for 0x0201 response)
    pub async fn send_query_location_and_wait(
        &self, phone: &str, timeout_secs: u64,
    ) -> Result<crate::jt1078::response_parser::LocationReport, String> {
        let body = command::build_query_location();
        let _ = self.send_command_and_wait(phone, 0x8201, &body, timeout_secs).await?;
        // The 0x0201 message body is delivered through process_jt_message.
        // For now we return a default location (full impl in Phase 6.5).
        Err("location response not yet wired to handler — use process_jt_message".to_string())
    }

    /// 0x8300 Text message dispatch
    pub async fn send_text_message_and_wait(
        &self, phone: &str, text: &str, emergency: bool, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_text_message(text, emergency);
        let resp = self.send_command_and_wait(phone, 0x8300, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8400 Phone callback
    pub async fn send_phone_callback_and_wait(
        &self, phone: &str, sign: u8, dest_phone: &str, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_phone_callback(sign, dest_phone);
        let resp = self.send_command_and_wait(phone, 0x8400, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8401 Set phone book
    pub async fn send_set_phone_book_and_wait(
        &self, phone: &str, contacts: &[(String, String)], timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_set_phone_book(contacts);
        let resp = self.send_command_and_wait(phone, 0x8401, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8500 Vehicle control (door open/close)
    pub async fn send_vehicle_control_and_wait(
        &self, phone: &str, control_type: u8, value: bool, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_vehicle_control(control_type, value);
        let resp = self.send_command_and_wait(phone, 0x8500, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8801 Take photo
    pub async fn send_take_photo_and_wait(
        &self, phone: &str, channel_id: u8, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_take_photo(channel_id, 0x0001, 5, 0, 0x02, 0x05, 0x80, 0x80, 0x80, 0x80);
        let resp = self.send_command_and_wait(phone, 0x8801, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8802 Media search
    pub async fn send_media_search_and_wait(
        &self, phone: &str, channel_id: u8, start_time: &str, end_time: &str, timeout_secs: u64,
    ) -> Result<u8, String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_media_search(0, channel_id, 0, &st, &et);
        let resp = self.send_command_and_wait(phone, 0x8802, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8801 摄像头立即拍摄命令 —— 用作**录像开始/停止**控制。
    ///
    /// 复用拍照原语的「拍摄命令」字段（`1`=开始录像 / `0`=停止录像）。
    pub async fn send_record_control_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        start: bool,
        duration_secs: u16,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_record_control(channel_id, start, duration_secs, true);
        let resp = self.send_command_and_wait(phone, 0x8801, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8202 临时位置跟踪控制（时间间隔 + 有效期），等待终端通用应答。
    pub async fn send_temp_position_tracking_and_wait(
        &self,
        phone: &str,
        interval_secs: u16,
        validity_secs: u32,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_temp_position_tracking(interval_secs, validity_secs);
        let resp = self.send_command_and_wait(phone, 0x8202, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8203 人工确认报警消息（报警流水号 + 确认类型位标志），等待终端通用应答。
    pub async fn send_confirm_alarm_and_wait(
        &self,
        phone: &str,
        alarm_seq: u16,
        alarm_type: u32,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_confirm_alarm(alarm_seq, alarm_type);
        let resp = self.send_command_and_wait(phone, 0x8203, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9205 文件上传指令 —— 请求终端上传指定时间段的音视频资源（录像下载）。
    pub async fn send_file_upload_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        start_time: &str,
        end_time: &str,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        // 严格解析：解析失败必须显式报错，而不是静默用「当前时间」下发给终端
        let st = command::try_encode_time_bcd(start_time)
            .ok_or_else(|| format!("无法解析开始时间: {}", start_time))?;
        let et = command::try_encode_time_bcd(end_time)
            .ok_or_else(|| format!("无法解析结束时间: {}", end_time))?;
        // 资源类型 0=音视频；报警标志 0=不筛选；资源掩码全 1=全部资源；
        // 存储器 0=主存储器；上传方式 1=手动上传；最大文件大小 0=不限制
        let body = command::build_file_upload_request(
            0,
            channel_id,
            &st,
            &et,
            0,
            0xFFFF_FFFF,
            0,
            1,
            0,
        );
        let resp = self.send_command_and_wait(phone, 0x9205, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8803 Media upload
    pub async fn send_media_upload_and_wait(
        &self, phone: &str, media_id: u32, timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_media_upload(media_id, 0);
        let resp = self.send_command_and_wait(phone, 0x8803, &body, timeout_secs).await?;
        Ok(resp.first().copied().unwrap_or(1))
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
    /// TCP 终端必须经**同一条 TCP 连接**收到下行命令。
    ///
    /// 回归：`send_raw` 此前一律用 UDP 下发，而 TCP 接入的终端只在 TCP 上监听，
    /// 于是所有平台命令（注册应答/查询/控制）都收不到，只能等超时。
    #[tokio::test]
    async fn test_send_raw_prefers_tcp_channel() {
        let manager = Jt1078Manager::new(
            Duration::from_secs(60), Duration::from_millis(200), None, false,
        );
        let addr = make_addr(61000);
        manager.register_terminal("13912345678", addr).await;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        manager.register_tcp_sender(addr, tx).await;

        manager.send_raw("13912345678", b"HELLO-TCP").await.unwrap();
        let got = tokio::time::timeout(Duration::from_millis(500), rx.recv())
            .await
            .expect("应收到下行数据")
            .expect("channel 不应关闭");
        assert_eq!(got, b"HELLO-TCP");

        // 连接断开后必须移除通道，避免往死连接写
        manager.remove_tcp_sender(&addr).await;
        assert!(manager.tcp_senders.lock().await.get(&addr).is_none());
    }

    /// UDP 终端（没有 TCP 通道）走监听 socket 下发 —— 来源端口即服务端口。
    #[tokio::test]
    async fn test_send_raw_udp_uses_listen_socket() {
        let manager = Jt1078Manager::new(
            Duration::from_secs(60), Duration::from_millis(200), None, false,
        );
        // 终端侧 UDP socket（模拟终端监听端口）
        let terminal = std::sync::Arc::new(
            tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap(),
        );
        let terminal_addr = terminal.local_addr().unwrap();
        // 平台侧"监听 socket"（下行必须用它，来源端口才是服务端口）
        let server = std::sync::Arc::new(
            tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap(),
        );
        let server_addr = server.local_addr().unwrap();
        manager.set_send_socket(server.clone());
        manager.register_terminal("13900000001", terminal_addr).await;

        manager.send_raw("13900000001", b"HELLO-UDP").await.unwrap();
        let mut buf = [0u8; 32];
        let (n, from) = tokio::time::timeout(
            Duration::from_millis(500),
            terminal.recv_from(&mut buf),
        )
        .await
        .expect("应收到下行数据")
        .unwrap();
        assert_eq!(&buf[..n], b"HELLO-UDP");
        assert_eq!(from, server_addr, "来源必须是平台的监听端口");
    }

    /// 未连接的终端必须显式报错，不能静默丢弃命令。
    #[tokio::test]
    async fn test_send_raw_reports_unknown_terminal() {
        let manager = Jt1078Manager::new(
            Duration::from_secs(60), Duration::from_millis(200), None, false,
        );
        let err = manager.send_raw("13900000000", b"x").await.unwrap_err();
        assert!(err.contains("未连接"), "{}", err);
    }

    /// 多媒体检索结果：取走即清空，过期结果不被复用。
    #[tokio::test]
    async fn test_media_search_result_store_and_take() {
        use crate::jt1078::response_parser::MediaSearchItem;
        let manager = Jt1078Manager::new(
            Duration::from_secs(60), Duration::from_millis(200), None, false,
        );
        assert!(manager
            .take_media_search_result("13912345678", Duration::from_secs(30))
            .await
            .is_none());
        manager
            .store_media_search_result("13912345678", vec![MediaSearchItem {
                media_id: 1,
                media_type: 2,
                channel_id: 1,
                event_code: 0,
                start_time: chrono::Utc::now(),
                end_time: chrono::Utc::now(),
                longitude: None,
                latitude: None,
            }])
            .await;
        let taken = manager
            .take_media_search_result("13912345678", Duration::from_secs(30))
            .await
            .expect("应能取到结果");
        assert_eq!(taken.len(), 1);
        // 取走即清空，避免同一批结果被重复返回
        assert!(manager
            .take_media_search_result("13912345678", Duration::from_secs(30))
            .await
            .is_none());
        // 过期（max_age=0）不再返回
        manager
            .store_media_search_result("13912345678", Vec::new())
            .await;
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(manager
            .take_media_search_result("13912345678", std::time::Duration::from_millis(1))
            .await
            .is_none());
    }
}
