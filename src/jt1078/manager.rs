use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use std::time::Instant;
use tokio::sync::Mutex;

use crate::jt1078::command;
use crate::jt1078::command_waiter::JtCommandWaiter;
use crate::jt1078::jt_media_session::JtMediaSessionManager;
use crate::jt1078::session::Jt1078Session;

/// 那些"数据应答是独立报文"的命令：终端会**先**回 0x0001 通用应答，**再**回
/// 一条数据报文（如 0x8201 → 0x0201）。通用应答只能代表"收到"，
/// 不能让等待方提前完成，否则真正的数据会被丢弃。
const COMMANDS_WITH_DATA_REPLY: &[u16] = &[0x8201]; // 位置信息查询 → 0x0201

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
    tcp_senders: Arc<
        Mutex<std::collections::HashMap<SocketAddr, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
    >,
    /// 终端多媒体检索结果缓存（0x8802 请求 → 0x0802 应答）。
    ///
    /// 请求与应答是两条独立的上行/下行消息：HTTP 侧发完 0x8802 后需要等待
    /// 终端的 0x0802，这里用一块共享缓存做交接（带采集时间，避免拿到上一次
    /// 检索的陈旧结果）。
    media_search_results: Arc<
        Mutex<
            std::collections::HashMap<
                String,
                (
                    std::time::Instant,
                    Vec<crate::jt1078::response_parser::MediaSearchItem>,
                ),
            >,
        >,
    >,
    /// 下发命令用的 UDP socket（与监听同一端口）。
    ///
    /// 此前 `send_raw` 每条命令都 `UdpSocket::bind("0.0.0.0:0")` 新建临时端口，
    /// 终端看到的来源地址**不是**它配置的服务器地址，于是按源地址过滤的终端
    /// （含本仓库的模拟器：它用 `connect()` 到平台地址）**一条命令都收不到**
    /// —— 表现为平台侧命令全部超时。真实平台都用同一个监听端口下发。
    send_socket: std::sync::OnceLock<Arc<tokio::net::UdpSocket>>,
    /// Phase 6.3: media session manager (live/playback/download)
    media_session_manager: Arc<JtMediaSessionManager>,
    /// 已通过 0x0102 鉴权的终端（手机号）。
    ///
    /// 终端鉴权（JT/T 808 0x0102）是"注册之后、干活之前"的一次证明：平台在
    /// 0x8100 里下发了鉴权码，终端把它回送上来。鉴权过的终端记在这里。
    authenticated_terminals: Arc<Mutex<std::collections::HashSet<String>>>,
    /// 是否**强制**终端鉴权（JT/T 808 0x0102）。
    ///
    /// * 宽松（默认 `false`）：终端不鉴权也能正常用 —— 大量真实终端与本仓库的
    ///   模拟器都不会回 0x0102，强制会让它们直接不可用。
    /// * 强制（`true`）：终端必须先通过 0x0102 鉴权，才算注册成功；鉴权码
    ///   不匹配的终端不登记为在线。
    auth_required: bool,
}

impl Jt1078Manager {
    pub fn new(
        timeout: Duration,
        retransmit_wait: Duration,
        retransmit_hook: Option<String>,
        retransmit_send_to_device: bool,
    ) -> Self {
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
            authenticated_terminals: Arc::new(Mutex::new(std::collections::HashSet::new())),
            auth_required: false,
        }
    }

    /// 设置是否强制终端鉴权。
    pub fn set_auth_required(&mut self, required: bool) {
        self.auth_required = required;
    }

    /// 是否强制终端鉴权。
    pub fn auth_required(&self) -> bool {
        self.auth_required
    }

    /// 数据库连接池（终端注册/鉴权码落库用；未绑定时为 `None`）。
    pub fn pool(&self) -> Option<crate::db::Pool> {
        self.pool.get().cloned()
    }

    /// 记下已通过鉴权的终端。
    pub async fn mark_authenticated(&self, phone: &str) {
        self.authenticated_terminals
            .lock()
            .await
            .insert(phone.to_string());
    }

    /// 该终端是否已通过鉴权。
    pub async fn is_authenticated(&self, phone: &str) -> bool {
        self.authenticated_terminals.lock().await.contains(phone)
    }

    /// 终端会话结束（超时/注销）时清掉鉴权状态。
    pub async fn clear_authenticated(&self, phone: &str) {
        self.authenticated_terminals.lock().await.remove(phone);
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
        let addr = self
            .get_terminal_addr(phone)
            .await
            .ok_or_else(|| format!("终端 {} 未连接", phone))?;
        let seq = self.next_seq(addr).await;
        let frame = command::build_jt808_frame(msg_id, phone, seq, body);

        // Register waiter
        let (_key, rx) = self
            .command_waiter
            .register(phone, msg_id, seq, Some(timeout_secs));

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
        self.terminal_addrs
            .lock()
            .await
            .insert(phone.to_string(), addr);
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
    pub async fn persist_terminal_online(
        &self,
        phone: &str,
        reg: Option<&crate::jt1078::response_parser::RegisterRequest>,
    ) {
        let Some(pool) = self.pool.get() else { return };
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let existing = crate::db::jt1078::get_terminal_by_phone(pool, phone)
            .await
            .ok()
            .flatten();
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
                if let Err(e) = crate::db::jt1078::insert_terminal(pool, phone, &fields, &now).await
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
                if let Err(e) = crate::db::jt1078::update_terminal(pool, phone, &fields, &now).await
                {
                    tracing::warn!("JT1078 终端更新失败 phone={}: {}", phone, e);
                }
            }
            _ => {}
        }
        // 强制鉴权（JT/T 808 0x0102）时，**没通过鉴权的终端不算在线**：
        // 登记与置在线是两件事 —— 元数据照常入库，在线状态要等鉴权通过。
        if self.auth_required && !self.authenticated_terminals.lock().await.contains(phone) {
            tracing::warn!(
                "JT1078 终端尚未通过鉴权（0x0102），不置为在线 phone={}",
                phone
            );
            return;
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
        let addr = self
            .get_terminal_addr(phone)
            .await
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
        socket
            .send_to(data, addr)
            .await
            .map_err(|e| format!("发送失败: {}", e))?;
        Ok(())
    }

    /// Send a JT808-framed command to a terminal.
    pub async fn send_command(&self, phone: &str, msg_id: u16, body: &[u8]) -> Result<(), String> {
        let addr = self
            .get_terminal_addr(phone)
            .await
            .ok_or_else(|| format!("终端 {} 未连接", phone))?;
        let seq = self.next_seq(addr).await;
        let frame = command::build_jt808_frame(msg_id, phone, seq, body);
        self.send_raw(phone, &frame).await
    }

    /// Send a PTZ control command (0x9301)
    pub async fn send_ptz(
        &self,
        phone: &str,
        channel_id: u8,
        direction: &str,
        speed: u8,
    ) -> Result<(), String> {
        let (b1, b2, h, v) = command::ptz_direction_bytes(direction, speed);
        let body = command::build_ptz_control(channel_id, b1, b2, h, v, 0);
        self.send_command(phone, 0x9301, &body).await
    }

    /// Send live video start/stop (0x9101)
    pub async fn send_live_video(
        &self,
        phone: &str,
        channel_id: u8,
        stream_type: u8,
        close: bool,
    ) -> Result<(), String> {
        let body = command::build_live_video_request(channel_id, stream_type, close);
        self.send_command(phone, 0x9101, &body).await
    }

    /// Send live video control (0x9102)
    pub async fn send_live_video_control(
        &self,
        phone: &str,
        channel_id: u8,
        control: u8,
        close: bool,
    ) -> Result<(), String> {
        let body = command::build_live_video_control(channel_id, control, close);
        self.send_command(phone, 0x9102, &body).await
    }

    /// Send playback request (0x9201)
    pub async fn send_playback(
        &self,
        phone: &str,
        channel_id: u8,
        stream_type: u8,
        storage_type: u8,
        speed: u8,
        start_time: &str,
        end_time: &str,
    ) -> Result<(), String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_playback_request(
            channel_id,
            stream_type,
            storage_type,
            0,
            speed,
            &st,
            &et,
        );
        self.send_command(phone, 0x9201, &body).await
    }

    /// Send playback control (0x9202)
    pub async fn send_playback_control(
        &self,
        phone: &str,
        channel_id: u8,
        control: u8,
        speed: u8,
        seek_time: &str,
    ) -> Result<(), String> {
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
    pub async fn send_text_message(
        &self,
        phone: &str,
        text: &str,
        emergency: bool,
    ) -> Result<(), String> {
        let body = command::build_text_message(text, emergency);
        self.send_command(phone, 0x8300, &body).await
    }

    /// Send phone callback (0x8400)
    pub async fn send_phone_callback(
        &self,
        phone: &str,
        sign: u8,
        dest_phone: &str,
    ) -> Result<(), String> {
        let body = command::build_phone_callback(sign, dest_phone);
        self.send_command(phone, 0x8400, &body).await
    }

    /// Send vehicle control (door) (0x8500)
    pub async fn send_vehicle_control(
        &self,
        phone: &str,
        control_type: u8,
        value: bool,
    ) -> Result<(), String> {
        let body = command::build_vehicle_control(control_type, value);
        self.send_command(phone, 0x8500, &body).await
    }

    /// Send take photo command (0x8801)
    pub async fn send_take_photo(&self, phone: &str, channel_id: u8) -> Result<(), String> {
        let body =
            command::build_take_photo(channel_id, 0x0001, 5, 0, 0x02, 0x05, 0x80, 0x80, 0x80, 0x80);
        self.send_command(phone, 0x8801, &body).await
    }

    /// Send media search (0x8802)
    pub async fn send_media_search(
        &self,
        phone: &str,
        channel_id: u8,
        start_time: &str,
        end_time: &str,
    ) -> Result<(), String> {
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
    pub async fn send_set_phone_book(
        &self,
        phone: &str,
        contacts: &[(String, String)],
    ) -> Result<(), String> {
        let body = command::build_set_phone_book(contacts);
        self.send_command(phone, 0x8401, &body).await
    }

    /// Send query attributes (0x8106)
    pub async fn send_query_attributes(&self, phone: &str) -> Result<(), String> {
        let body = command::build_query_attributes();
        self.send_command(phone, 0x8106, &body).await
    }

    /// Send set params (0x8103)
    pub async fn send_set_params(
        &self,
        phone: &str,
        apn: &str,
        ip: &str,
        port: u16,
    ) -> Result<(), String> {
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
    pub async fn send_connection_control(
        &self,
        phone: &str,
        ip: &str,
        port: u16,
    ) -> Result<(), String> {
        let ip_parts: Vec<u8> = ip.split('.').filter_map(|s| s.parse().ok()).collect();
        if ip_parts.len() != 4 {
            return Err("无效IP地址".to_string());
        }
        let mut body = vec![ip_parts[0], ip_parts[1], ip_parts[2], ip_parts[3]];
        body.extend_from_slice(&port.to_be_bytes());
        self.send_command(phone, 0x8103, &body).await
    }

    pub async fn process_payload_for(
        &self,
        addr: SocketAddr,
        payload: &[u8],
    ) -> crate::jt1078::session::FrameKind {
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
        // 0x0201（位置信息查询应答）是一条**独立报文**，不是 0x0001 通用应答，
        // 因此这里按回显的流水号把消息体交回等待中的 `0x8201` 命令。
        if msg_id == 0x0201 {
            if self
                .command_waiter
                .complete_by_serial(serial, body.to_vec())
            {
                tracing::debug!(
                    "JT1078 0x0201 位置应答已交回等待中的 0x8201 命令 serial={}",
                    serial
                );
            }
        }
        // Resolve command waiter for 0x0001 general common response
        if msg_id == 0x0001 && body.len() >= 5 {
            let reply_serial = u16::from_be_bytes([body[0], body[1]]);
            let reply_msg_id = u16::from_be_bytes([body[2], body[3]]);
            let result = body[4];
            // 有些命令的**数据**应答是一条独立报文（先回 0x0001 通用应答，
            // 再回数据报文）。若用 0x0001 去 complete，等待方会拿到 1 字节的
            // 结果码；真正的数据报文到达时等待方已经不在了 ——
            // 实测：0x8201 位置查询就是这样被"抢答"成失败，接口只能走兜底。
            if !COMMANDS_WITH_DATA_REPLY.contains(&reply_msg_id) {
                self.command_waiter.try_resolve_by_response(
                    phone,
                    reply_msg_id,
                    reply_serial,
                    result,
                );
            } else {
                tracing::debug!(
                    "JT1078 0x0001 通用应答（reply_msg_id=0x{:04X}）不完成等待方：该命令有独立数据应答报文",
                    reply_msg_id
                );
            }
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
            crate::jt1078::session::ParsedMessage::LocationReport(loc) => {
                // 位置汇报（0x0200）/ 位置查询应答（0x0201）此前**被直接丢弃**：
                // `process_jt_message` 只处理 Register/Heartbeat/MediaSearchResult，
                // 其余落 `_ => {}`，于是 `gb_jt_terminal.longitude/latitude` 与
                // `gb_position_history` 永远没有 JT1078 的数据
                // （`db::jt1078::update_terminal_position` 零调用），
                // `/api/jt1078/position-info` 只能返回空。第四十八轮修复。
                if let Some(pool) = self.pool.get() {
                    if let Err(e) = crate::db::jt1078::update_terminal_position(
                        pool,
                        phone,
                        loc.longitude,
                        loc.latitude,
                    )
                    .await
                    {
                        tracing::warn!(
                            "JT1078 位置回写 gb_jt_terminal 失败 phone={}: {}",
                            phone,
                            e
                        );
                    }
                    // 同一份位置也写进 `gb_position_history`（地图打点/轨迹用）
                    let ts = loc.time.format("%Y-%m-%d %H:%M:%S").to_string();
                    if let Err(e) = crate::db::position_history::insert_position(
                        pool,
                        phone,
                        &ts,
                        loc.longitude,
                        loc.latitude,
                        loc.altitude as f64,
                        loc.speed as f64,
                        loc.direction as f64,
                    )
                    .await
                    {
                        tracing::warn!(
                            "JT1078 位置写 gb_position_history 失败 phone={}: {}",
                            phone,
                            e
                        );
                    }
                }
            }
            crate::jt1078::session::ParsedMessage::MediaSearchResult(items) => {
                tracing::info!(
                    "JT1078 收到多媒体检索应答 phone={} items={}",
                    phone,
                    items.len()
                );
                // 落库：`gb_jt_media_item` 此前后**没有任何写入方**
                // （`db::jt1078::insert_media_item` 零调用），于是
                // `/api/jt1078/record/list` 的"先查库"分支永远为空，只能退化成
                // ZLM/云录像兜底 —— 终端自己检索到的录像列表在平台上拿不到
                // （第四十八轮发现）。
                if let Some(pool) = self.pool.get() {
                    for it in items.iter() {
                        if let Err(e) = crate::db::jt1078::insert_media_item(
                            pool,
                            phone,
                            it.channel_id as i32,
                            it.media_id,
                            it.media_type as i32,
                            0, // 0x0802 报文里没有"媒体格式"字段
                            it.event_code as i32,
                            &it.start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                            &it.end_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                        )
                        .await
                        {
                            tracing::warn!(
                                "JT1078 多媒体检索结果落库失败 phone={} media_id={}: {}",
                                phone,
                                it.media_id,
                                e
                            );
                        }
                    }
                }
                self.store_media_search_result(phone, items.clone()).await;
            }
            _ => {}
        }
        parsed
    }

    pub async fn cleanup_once(&self) -> usize {
        // **锁的作用域要收窄**：下面几段各自用独立的作用域把锁取出来又放掉，
        // 最后一段（DB 写）在**所有锁之外**执行。
        //
        // 原因：`self.sessions` 是**所有**入站 JT1078 报文都要拿的锁
        // （`process_jt_message` 一进来就 `sessions.lock().await`）。如果在这把
        // 锁的持有期内做 DB 往返（置终端离线），那么 DB 一慢、连接池一满，
        // **整个 JT1078 信令面**（注册、心跳、0x0200 位置汇报、指令应答）
        // 都会被卡住 —— 一个后台清理任务不该有能力拖垮信令。
        //
        // 第一段 + 第二段：**都在 `sessions` 锁内**（纯内存，无 IO）。
        //
        // **合并这两个作用域本身就是那道保护**：第三轮把它们分成两段，
        // 于是"摘会话 ↔ 清地址表"之间可能有报文把会话**复活**
        // （`process_jt_message` 的 `map.entry(addr).or_insert_with`），
        // 活着的终端会丢掉 `terminal_addrs` 映射 → `get_terminal_addr` 返回 None
        // → `is_terminal_online` 为 false → **命令下发被拒**，直到下一个心跳。
        // 合并之后，这段窗口**不存在**了：我们持着锁时谁也进不来。
        //
        // （注意：这里**不需要**再"复查会话是否已复活"—— 持锁期间不可能有复活，
        //   那种复查是恒真的空操作，写在那里只会**看起来**像有保护。）
        //
        // `deadline` 在**摘掉会话之后立刻**取：此刻仍持锁，不可能有心跳插进来，
        // 所以它就是"这些会话从现在起算已经失效"的时刻基准。第四段的条件写要靠它
        // 区分"这行在会话失效之后又被更新过"（→ 终端其实是活的，不能置离线）与
        // "这行在会话失效之后没被动过"（→ 确实可以置离线）。
        // 若把 deadline 拖到 DB 写之前才取，那么"摘会话 ↔ 取 deadline"之间
        // 到达的心跳，其 `update_time` 会落在 deadline **之前** → 被误判成
        // "没有活动" → **活着的终端被标成离线**。
        let (removed_addrs, deadline, stale_phones): (Vec<SocketAddr>, String, Vec<String>) = {
            let mut map = self.sessions.lock().await;
            let now = Instant::now();
            let mut removed = Vec::new();
            for (k, v) in map.iter() {
                if now.duration_since(v.last_heartbeat) > self.timeout {
                    removed.push(*k);
                }
            }
            for k in removed.iter() {
                map.remove(k);
            }
            let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

            // 第二段：清终端地址登记表，并收集"因此超时"的终端（只动内存）。
            // 锁序仍是 sessions → terminal_addrs（与 `remove()` 一致，不成环）。
            //
            // `removed` 用 `HashSet` 而不是 `Vec`：`terms.retain` 里要做
            // `removed.contains(a)`，Vec 是 O(R) 线性扫描 → 整体 **O(T×R)**。
            // 实测：终端表 1 万条 + 同时掉线 1000 台时，锁内约 **8ms**；
            // 10 万条时约 **56ms** —— 而这段是**持着 `sessions` 锁**的，
            // 期间**所有**入站 JT1078 报文都要等。换成 HashSet 后是 O(T+R)。
            let removed_set: std::collections::HashSet<SocketAddr> =
                removed.iter().copied().collect();
            let mut stale = Vec::new();
            {
                let mut terms = self.terminal_addrs.lock().await;
                terms.retain(|phone, a| {
                    let keep = !removed_set.contains(a);
                    if !keep {
                        stale.push(phone.clone());
                    }
                    keep
                });
            }
            (removed, deadline, stale)
        };

        if removed_addrs.is_empty() {
            return 0;
        }

        if stale_phones.is_empty() {
            return removed_addrs.len();
        }

        // 第三段：清掉内存里的"已鉴权"标记（复用现成的 helper，别让两份
        // 清理逻辑分叉）。会话超时 = 终端已经不在了，留着"已鉴权"会让它
        // 下次进来时被当成已鉴权通过（强制鉴权档下尤其不对）。
        for phone in &stale_phones {
            self.clear_authenticated(phone).await;
        }

        // 第四段：**在所有锁之外**把库里的在线标志置回 false。
        //
        // `gb_jt_terminal.status` 全仓库只有一处会写（`persist_terminal_online`），
        // 而且参数恒为 `true` —— 也就是"粘 true"的：终端超时后库里的在线标志
        // 纹丝不动，心跳路径还会把地址重新登记回来。结果是
        // "会话超时=终端已经不在了"这条语义**没有真正落地**：
        // 超时的终端仍被当作在线、平台照样能给它下发命令，
        // 而它其实从未重新鉴权过。
        // 这里补上写 false，"超时 ⇒ 真离线 ⇒ 需重新注册+重新鉴权"才闭环。
        //
        // **条件写**（`mark_terminal_offline_if_stale`）：第一段与第四段之间不是原子的，
        // 终端若在这期间发过心跳，`update_time` 已被刷新 —— 此时**不能**再把它标成
        // 离线，否则活着的终端在库里被显示成"离线"，而平台其实仍能给它下发命令。
        // 条件是 `update_time < deadline`（**严格小于**）：
        // `deadline` 取在会话被摘掉的那一刻，而 `update_time` 是**秒级**精度 ——
        // 同一秒内到达的心跳，其 `update_time` 会**等于** deadline，用 `<=`
        // 就会把它误判成"没有活动"。用严格小于，方向是安全的：
        // **宁可暂时不置离线，也不能把活着的终端误标成离线**。
        //
        // 代价要说清楚（上一版注释写"宁可晚一个清理周期"，**那是不准确的**）：
        // 会话被摘掉后该 phone 已不在 `terminal_addrs` 里，**下一轮清理不会再看到它**
        // ⇒ 条件不成立就是**一直不置离线**（不是"晚一轮"）。
        // 对**活着的**终端这是正确的（它会自己发心跳刷新 `update_time`，
        // 下一轮就能判出它还活着）；但对"历史遗留格式导致条件恒不成立"的行
        // （见 `db::jt1078::parse_update_time` 的说明）就是**永久粘住在线** ——
        // 那需要一次数据订正，不是在这里兜底。
        if let Some(pool) = self.pool.get() {
            for phone in &stale_phones {
                match crate::db::jt1078::mark_terminal_offline_if_stale(&pool, phone, &deadline)
                    .await
                {
                    Ok(0) => {
                        tracing::debug!("JT1078 终端在会话超时后又有活动，不置离线 phone={phone}")
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("JT1078 置终端离线失败 phone={phone}（会话已超时）: {e}");
                    }
                }
            }
        }

        removed_addrs.len()
    }

    pub async fn cleanup_loop(&self, tick: Duration) {
        loop {
            tokio::time::sleep(tick).await;
            let removed = self.cleanup_once().await;
            if removed > 0 {
                tracing::info!(
                    "JT1078 manager cleanup removed {} timed-out sessions",
                    removed
                );
            }

            let mut map = self.sessions.lock().await;
            for (addr, sess) in map.iter_mut() {
                let timed_out = sess.collect_timed_out_missing(self.retransmit_wait);
                if !timed_out.is_empty() {
                    tracing::warn!(
                        "JT1078 missing sequences timed out for {}: {:?}",
                        addr,
                        timed_out
                    );
                    if sess.should_trigger_missing_alert(self.retransmit_wait) {
                        tracing::warn!("JT1078 missing seqs for {}: {:?}", addr, timed_out);
                        crate::metrics::inc_missing(timed_out.len() as u64);
                        if let Some(hook) = &self.retransmit_hook {
                            let addr_s = addr.to_string();
                            let missing = timed_out.clone();
                            let hook_url = hook.clone();
                            tokio::spawn(async move {
                                let client = reqwest::Client::builder()
                                    .no_proxy()
                                    .build()
                                    .unwrap_or_else(|_| reqwest::Client::new());
                                #[derive(serde::Serialize)]
                                struct MissingReport {
                                    addr: String,
                                    missing: Vec<u16>,
                                    timestamp_ms: u128,
                                }
                                let report = MissingReport {
                                    addr: addr_s,
                                    missing,
                                    timestamp_ms: chrono::Utc::now().timestamp_millis() as u128,
                                };
                                let _ = client.post(&hook_url).json(&report).send().await;
                            });
                        }
                        if self.retransmit_send_to_device {
                            let addr_copy = *addr;
                            let missing_copy = timed_out.clone();
                            tokio::spawn(async move {
                                let _ =
                                    Jt1078Manager::send_retransmit_request(addr_copy, missing_copy)
                                        .await;
                            });
                        }
                    }
                }
            }
        }
    }

    pub async fn send_retransmit_request(
        addr: SocketAddr,
        missing: Vec<u16>,
    ) -> anyhow::Result<()> {
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
        &self,
        phone: &str,
        channel_id: u8,
        direction: &str,
        speed: u8,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let (b1, b2, h, v) = command::ptz_direction_bytes(direction, speed);
        let body = command::build_ptz_control(channel_id, b1, b2, h, v, 0);
        let resp = self
            .send_command_and_wait(phone, 0x9301, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9101 Live video start/stop.
    /// Phase 6.3: when start (close=false), also creates a media session and
    /// waits for ZLM on_stream_changed hook to resolve the session to Active.
    pub async fn send_live_video_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        stream_type: u8,
        close: bool,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_live_video_request(channel_id, stream_type, close);
        let resp = self
            .send_command_and_wait(phone, 0x9101, &body, timeout_secs)
            .await?;
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
        &self,
        phone: &str,
        channel_id: u8,
        timeout_secs: u64,
    ) -> Result<crate::jt1078::jt_media_session::JtMediaSession, String> {
        self.media_session_manager
            .wait_for_media(
                phone,
                channel_id,
                std::time::Duration::from_secs(timeout_secs),
            )
            .await
    }

    /// 0x9102 Live video control
    pub async fn send_live_video_control_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        control: u8,
        close: bool,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_live_video_control(channel_id, control, close);
        let resp = self
            .send_command_and_wait(phone, 0x9102, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9201 Playback request
    pub async fn send_playback_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        stream_type: u8,
        storage_type: u8,
        speed: u8,
        start_time: &str,
        end_time: &str,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_playback_request(
            channel_id,
            stream_type,
            storage_type,
            0,
            speed,
            &st,
            &et,
        );
        let resp = self
            .send_command_and_wait(phone, 0x9201, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x9202 Playback control
    pub async fn send_playback_control_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        control: u8,
        speed: u8,
        seek_time: &str,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let st = command::encode_time_bcd(seek_time);
        let body = command::build_playback_control(channel_id, control, speed, &st);
        let resp = self
            .send_command_and_wait(phone, 0x9202, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8103 Set terminal parameters (wiper, fill light, APN, IP, port, etc.)
    pub async fn send_set_params_and_wait(
        &self,
        phone: &str,
        params: &[(u32, &[u8])],
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_set_params(params);
        let resp = self
            .send_command_and_wait(phone, 0x8103, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8104 Query terminal parameters (waits for 0x0107 response)
    pub async fn send_query_params_and_wait(
        &self,
        phone: &str,
        param_ids: &[u32],
        timeout_secs: u64,
    ) -> Result<Vec<crate::jt1078::response_parser::TerminalParam>, String> {
        let body = command::build_query_params(param_ids);
        let _ = self
            .send_command_and_wait(phone, 0x8104, &body, timeout_secs)
            .await?;
        // The actual 0x0107 message body is delivered through process_jt_message;
        // for now we return an empty list (full impl in Phase 6.5).
        Ok(Vec::new())
    }

    /// 0x8105 Terminal control (reset, factory reset, etc.)
    pub async fn send_terminal_control_and_wait(
        &self,
        phone: &str,
        cmd: u8,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_terminal_control(cmd);
        let resp = self
            .send_command_and_wait(phone, 0x8105, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8106 Query terminal attributes
    pub async fn send_query_attributes_and_wait(
        &self,
        phone: &str,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_query_attributes();
        let resp = self
            .send_command_and_wait(phone, 0x8106, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8201 Query location (waits for 0x0201 response)
    ///
    /// 终端回的是 **0x0201**（消息体与 0x0200 一致），由
    /// `process_jt_message` 按回显流水号交回这里的等待方；此前这里发完请求就
    /// 直接返回 `Err("not yet wired")`，于是 `/api/jt1078/position-info` 的
    /// 实时查询分支永远失败。
    pub async fn send_query_location_and_wait(
        &self,
        phone: &str,
        timeout_secs: u64,
    ) -> Result<crate::jt1078::response_parser::LocationReport, String> {
        let body = command::build_query_location();
        let resp = self
            .send_command_and_wait(phone, 0x8201, &body, timeout_secs)
            .await?;
        crate::jt1078::response_parser::parse_location_report(&resp)
    }

    /// 0x8300 Text message dispatch
    pub async fn send_text_message_and_wait(
        &self,
        phone: &str,
        text: &str,
        emergency: bool,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_text_message(text, emergency);
        let resp = self
            .send_command_and_wait(phone, 0x8300, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8400 Phone callback
    pub async fn send_phone_callback_and_wait(
        &self,
        phone: &str,
        sign: u8,
        dest_phone: &str,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_phone_callback(sign, dest_phone);
        let resp = self
            .send_command_and_wait(phone, 0x8400, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8401 Set phone book
    pub async fn send_set_phone_book_and_wait(
        &self,
        phone: &str,
        contacts: &[(String, String)],
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_set_phone_book(contacts);
        let resp = self
            .send_command_and_wait(phone, 0x8401, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8500 Vehicle control (door open/close)
    pub async fn send_vehicle_control_and_wait(
        &self,
        phone: &str,
        control_type: u8,
        value: bool,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_vehicle_control(control_type, value);
        let resp = self
            .send_command_and_wait(phone, 0x8500, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8801 Take photo
    pub async fn send_take_photo_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body =
            command::build_take_photo(channel_id, 0x0001, 5, 0, 0x02, 0x05, 0x80, 0x80, 0x80, 0x80);
        let resp = self
            .send_command_and_wait(phone, 0x8801, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8802 Media search
    pub async fn send_media_search_and_wait(
        &self,
        phone: &str,
        channel_id: u8,
        start_time: &str,
        end_time: &str,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let st = command::encode_time_bcd(start_time);
        let et = command::encode_time_bcd(end_time);
        let body = command::build_media_search(0, channel_id, 0, &st, &et);
        let resp = self
            .send_command_and_wait(phone, 0x8802, &body, timeout_secs)
            .await?;
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
        let resp = self
            .send_command_and_wait(phone, 0x8801, &body, timeout_secs)
            .await?;
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
        let resp = self
            .send_command_and_wait(phone, 0x8202, &body, timeout_secs)
            .await?;
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
        let resp = self
            .send_command_and_wait(phone, 0x8203, &body, timeout_secs)
            .await?;
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
        let body =
            command::build_file_upload_request(0, channel_id, &st, &et, 0, 0xFFFF_FFFF, 0, 1, 0);
        let resp = self
            .send_command_and_wait(phone, 0x9205, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }

    /// 0x8803 Media upload
    pub async fn send_media_upload_and_wait(
        &self,
        phone: &str,
        media_id: u32,
        timeout_secs: u64,
    ) -> Result<u8, String> {
        let body = command::build_media_upload(media_id, 0);
        let resp = self
            .send_command_and_wait(phone, 0x8803, &body, timeout_secs)
            .await?;
        Ok(resp.first().copied().unwrap_or(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    /// 回归（**源码契约**）：`cleanup_once` 里「摘会话」与「清终端地址表」必须
    /// **在同一个 `sessions` 锁作用域内**。
    ///
    /// 为什么这条要有：这两步**分成两个作用域**时，中间存在一个窗口 ——
    /// 会话被摘掉之后、地址表被清理之前，若有报文把会话**复活**
    /// （`process_jt_message` 的 `map.entry(addr).or_insert_with`），活着的终端就会
    /// 丢掉 `terminal_addrs` 映射 → `get_terminal_addr` 返回 None →
    /// `is_terminal_online` 为 false → **命令下发被拒**，直到下一个心跳。
    ///
    /// **为什么是源码契约而不是功能测试**：变异测试实测 —— 把两段拆回独立作用域，
    /// **所有 jt1078 测试仍然全绿**。这个竞态**没有功能测试能覆盖**（要在两段之间
    /// 精确注入一次心跳，得在生产代码里加测试钩子，代价大于收益）。
    /// 所以用源码契约守住这条**结构性**不变量。
    ///
    /// 判定方式：找 `self.sessions.lock().await` 与 `terms.retain(` 两行的位置，
    /// 再找**缩进比 sessions 锁更浅**的 `};`（那才是 sessions 作用域的结束点，
    /// 函数里还有更深的内层代码块）—— `retain` 必须落在它之前。
    /// （用缩进而不是"第一个 `};`"：内层代码块的 `};` 更深。）
    ///
    /// 判定**取 `retain` 调用行**而不是 `terminal_addrs` 的取锁行 —— 后者有个
    /// 残余缺口：若有人在作用域内 `drop(map)` 提前释放 `sessions` 锁（取锁行仍在
    /// 作用域内），只查取锁行会**抓不住**；查 `retain` 调用行才抓得住。
    #[test]
    fn cleanup_keeps_session_removal_and_address_cleanup_in_one_scope() {
        let src = include_str!("manager.rs");
        let body = src
            .split("pub async fn cleanup_once")
            .nth(1)
            .expect("cleanup_once 存在")
            .split("\n    pub ")
            .next()
            .expect("cleanup_once 函数体");

        let lines: Vec<&str> = body.lines().collect();
        let sessions_at = lines
            .iter()
            .position(|l| l.contains("self.sessions.lock().await"))
            .expect("cleanup_once 里取了 sessions 锁");
        let retain_at = lines
            .iter()
            .position(|l| l.contains("terms.retain("))
            .expect("cleanup_once 里清了终端地址表");
        let sessions_indent = lines[sessions_at].len() - lines[sessions_at].trim_start().len();
        // sessions 作用域的结束点：之后第一个**缩进更浅**的 `};`
        let scope_end = lines
            .iter()
            .enumerate()
            .skip(sessions_at + 1)
            .find(|(_, l)| {
                let t = l.trim();
                t == "};" && (l.len() - l.trim_start().len()) < sessions_indent
            })
            .map(|(i, _)| i)
            .expect("sessions 作用域的结束点");

        assert!(
            retain_at < scope_end,
            "清终端地址表（terms.retain）必须在 sessions 作用域**内**完成 \
             （当前 retain_at={retain_at}, scope_end={scope_end}）—— \
             拆成两个作用域就有『会话被复活』的窗口"
        );
    }

    fn make_addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    /// 回归：会话超时时，库里的 `gb_jt_terminal.status` 必须被置回 **0**。
    ///
    /// `status` 全仓库只有一处会写（`persist_terminal_online`）且参数恒为 `true`
    /// —— "粘 true"。会话超时时若不把它置 0，终端超时后**仍被当作在线**，
    /// 平台照样能给它下发命令，而它其实从未重新鉴权过。
    /// "超时 ⇒ 真离线 ⇒ 需重新注册 + 重新鉴权"这条语义要闭环。
    #[tokio::test]
    async fn session_timeout_marks_terminal_offline_in_db() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        const PHONE: &str = "13812345678";
        // 这一行的时间戳要**早于**清理时刻 —— 真实场景里终端最后上报是
        // 超时之前（默认超时 60s），`update_time` 自然比清理时刻早。
        // 若写成"此刻"，就会与清理时刻同秒，条件写（严格小于）不会命中。
        let old = chrono::Utc::now() - chrono::Duration::minutes(2);
        let old = old.format("%Y-%m-%d %H:%M:%S").to_string();
        let ins_sql = pool.sql(
            "INSERT INTO gb_jt_terminal (phone_number, status, create_time, update_time) \
             VALUES (?, 1, ?, ?)",
        );
        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&ins_sql)
                .bind(PHONE)
                .bind(&old)
                .bind(&old)
                .execute(p)
        })
        .unwrap();

        let manager = Jt1078Manager::new(
            Duration::from_millis(100),
            Duration::from_millis(200),
            None,
            false,
        );
        manager.set_pool(pool.clone());
        let addr = make_addr(60099);
        // 造一个会话 + 地址登记（心跳路径做的两件事）
        manager.register_terminal(PHONE, addr).await;
        {
            let mut map = manager.sessions.lock().await;
            map.insert(addr, crate::jt1078::session::Jt1078Session::new(addr));
        }

        // 会话超时 → 清理
        tokio::time::sleep(Duration::from_millis(200)).await;
        let removed = manager.cleanup_once().await;
        assert_eq!(removed, 1, "超时会话应被清掉");

        let sel_sql = pool.sql("SELECT status FROM gb_jt_terminal WHERE phone_number = ?");
        let status: i64 = crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sel_sql)
                .bind(PHONE)
                .fetch_one(p)
        })
        .unwrap();
        assert_eq!(status, 0, "会话超时时必须把终端置为离线");
    }

    /// 回归：**同一秒**内到达的心跳也不能被误置离线。
    ///
    /// `update_time` 与 `deadline` 都是**秒级**精度，所以"摘掉会话那一刻"与
    /// "心跳到达"落在同一秒时，两者**相等**。条件写若用 `<=` 就会把这次心跳
    /// 误判成"没有活动" → **活着的终端被标成离线**。
    /// 方向上宁可晚一个清理周期再置离线（终端下次心跳本来也会把自己置回在线）。
    #[tokio::test]
    async fn terminal_heartbeat_in_the_same_second_is_not_marked_offline() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        const PHONE: &str = "13812345678";
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let ins_sql = pool.sql(
            "INSERT INTO gb_jt_terminal (phone_number, status, create_time, update_time) \
             VALUES (?, 1, ?, ?)",
        );
        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&ins_sql)
                .bind(PHONE)
                .bind(&now)
                .bind(&now)
                .execute(p)
        })
        .unwrap();

        // `deadline` 与 `update_time` 落在同一秒 → 严格小于不成立 → 不置离线
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let n = crate::db::jt1078::mark_terminal_offline_if_stale(&pool, PHONE, &deadline)
            .await
            .unwrap();
        assert_eq!(n, 0, "同一秒内的心跳不应被置离线");

        let sel_sql = pool.sql("SELECT status FROM gb_jt_terminal WHERE phone_number = ?");
        let status: i64 = crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sel_sql)
                .bind(PHONE)
                .fetch_one(p)
        })
        .unwrap();
        assert_eq!(status, 1, "刚活跃过的终端必须保持在线");
    }

    /// 回归：**条件写** —— 清理与心跳之间若终端又有活动，就**不能**再把它置离线。
    ///
    /// 清理分两段（锁内摘会话、锁外写库），两段之间不是原子的：终端若在这期间
    /// 发过心跳，`update_time` 已被刷新 —— 此时无条件写 `status=0` 会把**活着的**
    /// 终端在库里标成离线，而平台其实仍能给它下发命令。
    #[tokio::test]
    async fn terminal_with_activity_after_timeout_is_not_marked_offline() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        const PHONE: &str = "13812345678";
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let ins_sql = pool.sql(
            "INSERT INTO gb_jt_terminal (phone_number, status, create_time, update_time) \
             VALUES (?, 1, ?, ?)",
        );
        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&ins_sql)
                .bind(PHONE)
                .bind(&now)
                .bind(&now)
                .execute(p)
        })
        .unwrap();

        // 模拟"清理开始之后终端又发了心跳"：把 update_time 刷新成更新的值
        let later = chrono::Utc::now() + chrono::Duration::seconds(5);
        let upd_sql = pool.sql("UPDATE gb_jt_terminal SET update_time = ? WHERE phone_number = ?");
        crate::dispatch_exec!(pool, |p| {
            sqlx::query(&upd_sql)
                .bind(later.format("%Y-%m-%d %H:%M:%S").to_string())
                .bind(PHONE)
                .execute(p)
        })
        .unwrap();

        // 用一个"截止时刻"早于该心跳的时刻去条件置离线 → 不应命中
        let deadline = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let n = crate::db::jt1078::mark_terminal_offline_if_stale(&pool, PHONE, &deadline)
            .await
            .unwrap();
        assert_eq!(n, 0, "期间有更新的行不应被置离线");

        let sel_sql = pool.sql("SELECT status FROM gb_jt_terminal WHERE phone_number = ?");
        let status: i64 = crate::dispatch!(pool, |p| {
            sqlx::query_scalar::<_, i64>(&sel_sql)
                .bind(PHONE)
                .fetch_one(p)
        })
        .unwrap();
        assert_eq!(status, 1, "有活动的终端必须保持在线");
    }

    #[tokio::test]
    async fn test_feed_and_count_and_cleanup() {
        let manager = Jt1078Manager::new(
            Duration::from_millis(100),
            Duration::from_millis(200),
            None,
            false,
        );
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
        let manager = Jt1078Manager::new(
            Duration::from_secs(60),
            Duration::from_millis(200),
            None,
            false,
        );
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
            Duration::from_secs(60),
            Duration::from_millis(200),
            None,
            false,
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
            Duration::from_secs(60),
            Duration::from_millis(200),
            None,
            false,
        );
        // 终端侧 UDP socket（模拟终端监听端口）
        let terminal =
            std::sync::Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let terminal_addr = terminal.local_addr().unwrap();
        // 平台侧"监听 socket"（下行必须用它，来源端口才是服务端口）
        let server = std::sync::Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let server_addr = server.local_addr().unwrap();
        manager.set_send_socket(server.clone());
        manager
            .register_terminal("13900000001", terminal_addr)
            .await;

        manager.send_raw("13900000001", b"HELLO-UDP").await.unwrap();
        let mut buf = [0u8; 32];
        let (n, from) =
            tokio::time::timeout(Duration::from_millis(500), terminal.recv_from(&mut buf))
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
            Duration::from_secs(60),
            Duration::from_millis(200),
            None,
            false,
        );
        let err = manager.send_raw("13900000000", b"x").await.unwrap_err();
        assert!(err.contains("未连接"), "{}", err);
    }

    /// 多媒体检索结果：取走即清空，过期结果不被复用。
    #[tokio::test]
    async fn test_media_search_result_store_and_take() {
        use crate::jt1078::response_parser::MediaSearchItem;
        let manager = Jt1078Manager::new(
            Duration::from_secs(60),
            Duration::from_millis(200),
            None,
            false,
        );
        assert!(manager
            .take_media_search_result("13912345678", Duration::from_secs(30))
            .await
            .is_none());
        manager
            .store_media_search_result(
                "13912345678",
                vec![MediaSearchItem {
                    media_id: 1,
                    media_type: 2,
                    channel_id: 1,
                    event_code: 0,
                    start_time: chrono::Local::now(),
                    end_time: chrono::Local::now(),
                    longitude: None,
                    latitude: None,
                }],
            )
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
