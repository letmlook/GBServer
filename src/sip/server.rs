use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use md5::{Digest, Md5};
use rand::Rng;
use tokio::net::UdpSocket;
use tokio::sync::{oneshot, RwLock};
use dashmap::DashMap;

use crate::config::SipConfig;
use crate::db::{device as db_device, Pool};
use crate::db::position_history as ph;
use crate::handlers::websocket::WsState;
use crate::sip::core::parser::Parser;
use crate::sip::core::{
    DialogManager, SipMessage, SipMethod,
    SipRequest, SipResponse, TransactionManager,
};
use crate::sip::gb28181::catalog::{build_catalog_notify_body, CatalogSubscriptionManager, CatalogSubscription};
use crate::sip::gb28181::invite::SessionStatus;
use crate::sip::gb28181::invite_session::{
    build_invite_sdp, build_playback_sdp, InviteSessionManager, InviteSessionStatus, SdpInfo,
};
use crate::sip::gb28181::talk::{build_talk_sdp as build_audio_sdp, TalkManager, TalkStatus};
use crate::sip::gb28181::{DeviceManager, SessionManager, XmlParser};
use crate::sip::gb28181::ssrc::SsrcManager;
use crate::sip::gb28181::stream_reconnect::StreamReconnectManager;
use crate::sip::gb28181::nat_helper::NatHelper;
use crate::sip::transport::tcp::{TcpConnectionManager, TcpListener};
use crate::zlm::ZlmClient;
use crate::cascade::CascadeRegistrar;

pub struct SipServer {
    config: Arc<SipConfig>,
    device_manager: Arc<DeviceManager>,
    session_manager: Arc<SessionManager>,
    invite_session_manager: Arc<InviteSessionManager>,
    talk_manager: Arc<TalkManager>,
    catalog_subscription_manager: Arc<CatalogSubscriptionManager>,
    transaction_manager: Arc<TransactionManager>,
    dialog_manager: Arc<DialogManager>,
    socket: Arc<RwLock<Option<UdpSocket>>>,
    tcp_enabled: bool,
    tcp_listener: Arc<RwLock<Option<TcpListener>>>,
    tcp_connection_manager: Arc<TcpConnectionManager>,
    pool: Pool,
    zlm_client: Option<Arc<ZlmClient>>,
    ws_state: Option<Arc<WsState>>,
    pending_invites: Arc<DashMap<String, oneshot::Sender<String>>>,
    ssrc_manager: Arc<SsrcManager>,
    stream_reconnect_manager: Arc<StreamReconnectManager>,
    nat_helper: Arc<NatHelper>,
    cascade_registrar: Option<Arc<CascadeRegistrar>>,
}

impl SipServer {
    pub fn new(config: SipConfig, pool: Pool) -> Self {
        let ssrc_manager = Arc::new(SsrcManager::new(&config.device_id));
        let nat_helper = Arc::new(NatHelper::new(
            &config.ip,
            config.sdp_ip.as_deref(),
            config.stream_ip.as_deref(),
        ));
        let stream_reconnect = config.stream_reconnect.as_ref()
            .map(|rc| StreamReconnectManager::new(rc.enabled, rc.max_retries, rc.retry_interval_secs))
            .unwrap_or(StreamReconnectManager::new(false, 3, 5));

        Self {
            config: Arc::new(config),
            device_manager: Arc::new(DeviceManager::new()),
            session_manager: Arc::new(SessionManager::new()),
            invite_session_manager: Arc::new(InviteSessionManager::new()),
            talk_manager: Arc::new(TalkManager::new()),
            catalog_subscription_manager: Arc::new(CatalogSubscriptionManager::new()),
            transaction_manager: Arc::new(TransactionManager::new()),
            dialog_manager: Arc::new(DialogManager::new()),
            socket: Arc::new(RwLock::new(None)),
            tcp_enabled: false,
            tcp_listener: Arc::new(RwLock::new(None)),
            tcp_connection_manager: Arc::new(TcpConnectionManager::new()),
            pool,
            zlm_client: None,
            ws_state: None,
            pending_invites: Arc::new(DashMap::new()),
            ssrc_manager,
            stream_reconnect_manager: Arc::new(stream_reconnect),
            nat_helper,
            cascade_registrar: None,
        }
    }

    pub async fn set_ws_state(&mut self, ws: Arc<WsState>) {
        self.stream_reconnect_manager.set_ws_state(ws.clone()).await;
        self.ws_state = Some(ws);
    }

    pub fn set_zlm_client(&mut self, client: Option<Arc<ZlmClient>>) {
        self.zlm_client = client;
    }

    pub fn set_cascade_registrar(&mut self, registrar: Arc<CascadeRegistrar>) {
        self.cascade_registrar = Some(registrar);
    }

    pub fn config(&self) -> &SipConfig {
        &self.config
    }

    pub fn socket(&self) -> &Arc<RwLock<Option<UdpSocket>>> {
        &self.socket
    }

    pub fn ssrc_manager(&self) -> &SsrcManager {
        &self.ssrc_manager
    }

    pub fn nat_helper(&self) -> &NatHelper {
        &self.nat_helper
    }

    pub fn stream_reconnect_manager(&self) -> &StreamReconnectManager {
        &self.stream_reconnect_manager
    }

    pub async fn is_device_online(&self, device_id: &str) -> bool {
        self.device_manager.get(device_id).await
            .map(|d| d.online)
            .unwrap_or(false)
    }

    pub fn set_tcp_enabled(&mut self, enabled: bool) {
        self.tcp_enabled = enabled;
    }

    pub fn talk_manager(&self) -> Arc<TalkManager> {
        self.talk_manager.clone()
    }

    pub async fn start(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.config.ip, self.config.port);
        let socket = UdpSocket::bind(&addr).await?;
        tracing::info!("SIP Server UDP listening on {}", addr);
        *self.socket.write().await = Some(socket);
        
        if self.tcp_enabled {
            let tcp_addr = format!("{}:{}", self.config.ip, self.config.tcp_port);
            match TcpListener::bind(&tcp_addr).await {
                Ok(listener) => {
                    let local_addr = listener.local_addr();
                    tracing::info!("SIP Server TCP listening on {}", local_addr);
                    *self.tcp_listener.write().await = Some(listener);
                }
                Err(e) => {
                    tracing::warn!("Failed to bind TCP listener: {}", e);
                }
            }
        }
        
        let device_manager = self.device_manager.clone();
        let invite_manager = self.invite_session_manager.clone();
        let talk_manager = self.talk_manager.clone();
        let _zlm_client = self.zlm_client.clone();
        let _config = self.config.clone();
        
        tokio::spawn(async move {
            loop {
                device_manager.cleanup_expired(60).await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });

        let zlm_invite = self.zlm_client.clone();
        tokio::spawn(async move {
            loop {
                if let Some(ref zlm) = zlm_invite {
                    let sessions = invite_manager.get_pending_sessions().await;
                    for session in sessions {
                        let elapsed = (Utc::now() - session.last_activity).num_seconds();
                        if elapsed > session.timeout_seconds as i64 {
                            tracing::warn!("Invite session timeout: {}", session.call_id);
                            if let Some(ref stream_id) = session.zlm_stream_id {
                                let _ = zlm.close_rtp_server(stream_id).await;
                            }
                            invite_manager.update_status(&session.call_id, InviteSessionStatus::Terminated).await;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });

        let zlm_talk = self.zlm_client.clone();
        tokio::spawn(async move {
            loop {
                if let Some(ref _zlm) = zlm_talk {
                    let removed = talk_manager.cleanup_expired(60).await;
                    for call_id in removed {
                        tracing::info!("Talk session cleaned up: {}", call_id);
                    }
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });

        dbg_upsert_device(&self.pool, &self.config.device_id, "WVP Server", Some("Rust"), Some("GBServer"), None, None, None, None, None, true, "zlmediakit-1".to_string()).await?;
        
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let socket = self.socket.write().await.take().expect("Server not started");
        let socket = Arc::new(socket);
        let config = self.config.clone();
        let device_manager = self.device_manager.clone();
        let session_manager = self.session_manager.clone();
        let invite_session_manager = self.invite_session_manager.clone();
        let talk_manager = self.talk_manager.clone();
        let catalog_subscription_manager = self.catalog_subscription_manager.clone();
        let zlm_client = self.zlm_client.clone();
        let pool = self.pool.clone();
        let tcp_connection_manager = self.tcp_connection_manager.clone();

        let tcp_listener = self.tcp_listener.write().await.take();
        
        let udp_socket = socket.clone();
        let udp_config = config.clone();
        let udp_device_manager = device_manager.clone();
        let udp_session_manager = session_manager.clone();
        let udp_invite_manager = invite_session_manager.clone();
        let udp_talk_manager = talk_manager.clone();
        let udp_catalog_manager = catalog_subscription_manager.clone();
        let udp_zlm = zlm_client.clone();
        let udp_pool = pool.clone();
        let udp_ws_state = self.ws_state.clone();
        let udp_pending_invites = self.pending_invites.clone();
        let udp_cascade_registrar = self.cascade_registrar.clone();
        
        tokio::spawn(async move {
            loop {
                let mut buf = vec![0u8; 65535];
                let socket_clone = udp_socket.clone();
                match socket_clone.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        let data = buf[..len].to_vec();
                        let config = udp_config.clone();
                        let device_manager = udp_device_manager.clone();
                        let session_manager = udp_session_manager.clone();
                        let invite_session_manager = udp_invite_manager.clone();
                        let talk_manager = udp_talk_manager.clone();
                        let catalog_subscription_manager = udp_catalog_manager.clone();
                        let zlm_client = udp_zlm.clone();
                        let pool = udp_pool.clone();
                        let socket_for_response = udp_socket.clone();

                        let ws_state = udp_ws_state.clone();
                        let pending_invites = udp_pending_invites.clone();
                        let cascade_registrar = udp_cascade_registrar.clone();
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_packet(&data, addr, &config, &device_manager, &session_manager, &invite_session_manager, &talk_manager, &catalog_subscription_manager, &zlm_client, &pool, &socket_for_response, false, &ws_state, &pending_invites, &cascade_registrar).await {
                                tracing::error!("SIP handler error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("UDP recv error: {}", e);
                    }
                }
            }
        });

        if let Some(mut listener) = tcp_listener {
            let tcp_config = config.clone();
            let tcp_device_manager = device_manager.clone();
            let tcp_session_manager = session_manager.clone();
            let tcp_invite_manager = invite_session_manager.clone();
            let tcp_talk_manager = talk_manager.clone();
            let tcp_catalog_manager = catalog_subscription_manager.clone();
            let tcp_zlm_client = zlm_client.clone();
            let tcp_pool = pool.clone();
            let tcp_conn_mgr = tcp_connection_manager.clone();

            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, addr)) => {
                            tracing::debug!("TCP connection from: {}", addr);
                            
                            let config = tcp_config.clone();
                            let device_manager = tcp_device_manager.clone();
                            let session_manager = tcp_session_manager.clone();
                            let invite_session_manager = tcp_invite_manager.clone();
                            let talk_manager = tcp_talk_manager.clone();
                            let catalog_subscription_manager = tcp_catalog_manager.clone();
                            let zlm_client = tcp_zlm_client.clone();
                            let pool = tcp_pool.clone();
                            let conn_manager = tcp_conn_mgr.clone();
                            
                            conn_manager.add_connection(addr, stream).await;
                            
                            tokio::spawn(async move {
                                Self::handle_tcp_connection(addr, &config, &device_manager, &session_manager, &invite_session_manager, &talk_manager, &catalog_subscription_manager, &zlm_client, &pool, &conn_manager).await;
                            });
                        }
                        Err(e) => {
                            tracing::error!("TCP accept error: {}", e);
                        }
                    }
                }
            });
        }

        let renewal_pool = pool.clone();
        let renewal_catalog_manager = catalog_subscription_manager.clone();
        let renewal_device_manager = device_manager.clone();
        let renewal_config = config.clone();
        let renewal_socket = socket.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                
                match db_device::get_devices_for_catalog_renewal(&renewal_pool).await {
                    Ok(devices) => {
                        for (device_id, cycle) in devices {
                            let subs = renewal_catalog_manager.get_by_device(&device_id).await;
                            let needs_renewal = subs.iter().any(|s| {
                                let elapsed = (Utc::now() - s.created_at).num_seconds() as u32;
                                let remaining = s.expires.saturating_sub(elapsed);
                                remaining < 30
                            });
                            
                            if needs_renewal {
                                if let Some(device) = renewal_device_manager.get(&device_id).await {
                                    if device.online {
                                        tracing::info!("Renewing catalog subscription for device {}", device_id);
                                        let _ = send_subscribe_internal(
                                            &device_id,
                                            "Catalog",
                                            cycle as u32,
                                            &renewal_config,
                                            &renewal_device_manager,
                                            &renewal_catalog_manager,
                                            &renewal_socket,
                                        ).await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to get devices for catalog renewal: {}", e);
                    }
                }
            }
        });

        let mobile_pool = pool.clone();
        let mobile_config = config.clone();
        let mobile_device_manager = device_manager.clone();
        let mobile_socket = socket.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                
                match db_device::get_devices_for_mobile_position_renewal(&mobile_pool).await {
                    Ok(devices) => {
                        for (device_id, cycle) in devices {
                            if let Some(device) = mobile_device_manager.get(&device_id).await {
                                if device.online {
                                    tracing::info!("Renewing mobile position subscription for device {}", device_id);
                                    let _ = send_subscribe_internal(
                                        &device_id,
                                        "MobilePosition",
                                        cycle as u32,
                                        &mobile_config,
                                        &mobile_device_manager,
                                        &Arc::new(CatalogSubscriptionManager::new()),
                                        &mobile_socket,
                                    ).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to get devices for mobile position renewal: {}", e);
                    }
                }
            }
        });

        // Start stream reconnect monitor loop
        let reconnect_mgr = self.stream_reconnect_manager.clone();
        tokio::spawn(async move {
            reconnect_mgr.run_reconnect_loop().await;
        });

        // Active heartbeat: periodically ping online devices to verify liveness
        let heartbeat_config = config.clone();
        let heartbeat_device_manager = device_manager.clone();
        let heartbeat_pool = pool.clone();
        let heartbeat_socket = socket.clone();
        let heartbeat_ws = self.ws_state.clone();

        tokio::spawn(async move {
            let check_interval = heartbeat_config
                .heartbeat
                .as_ref()
                .map(|h| h.check_interval_secs)
                .unwrap_or(30);
            let timeout_multiplier = heartbeat_config
                .heartbeat
                .as_ref()
                .map(|h| h.timeout_multiplier)
                .unwrap_or(3);

            loop {
                tokio::time::sleep(Duration::from_secs(check_interval)).await;

                let devices = heartbeat_device_manager.list_all().await;
                let now = Utc::now();
                let keepalive_timeout = heartbeat_config.keepalive_timeout as i64;

                // Update metrics
                let online_count = devices.iter().filter(|d| d.online).count();
                crate::metrics::set_sip_devices_online(online_count);

                for device in &devices {
                    if !device.online {
                        continue;
                    }
                    let elapsed = now.timestamp() - device.keepalive_time.timestamp();
                    let threshold = keepalive_timeout * timeout_multiplier as i64;

                    if elapsed > threshold {
                        // Device hasn't sent keepalive for too long — mark offline
                        tracing::info!(
                            "Device {} keepalive timeout ({}s > {}s), marking offline",
                            device.device_id, elapsed, threshold
                        );
                        heartbeat_device_manager.set_online(&device.device_id, false).await;

                        // Update DB
                        let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
                        let _ = crate::db::device::update_device_online(
                            &heartbeat_pool,
                            &device.device_id,
                            false,
                            None,
                            None,
                            &now_str,
                        ).await;

                        // Push WebSocket notification
                        if let Some(ref ws) = heartbeat_ws {
                            ws.broadcast("device", serde_json::json!({
                                "deviceId": device.device_id,
                                "online": false,
                                "reason": "keepalive_timeout",
                            })).await;
                        }
                    } else if elapsed > keepalive_timeout {
                        // Device might need a keepalive ping — send MESSAGE query
                        if let Some(addr) = device.addr {
                            let keepalive_xml = format!(
                                r#"<?xml version="1.0" encoding="UTF-8"?>
<Message>
<CmdType>Keepalive</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Message>"#,
                                Utc::now().timestamp() % 10000,
                                device.device_id
                            );

                            // Build a simple SIP MESSAGE with the keepalive body
                            let branch = format!("z9hG4bK{}", rand::random::<u32>());
                            let call_id = format!("keepalive_{}_{}", device.device_id, Utc::now().timestamp());
                            let local_tag = format!("keepalive_{}", rand::random::<u32>());

                            let sip_msg = format!(
                                "MESSAGE sip:{}@{}:{} SIP/2.0\r\n\
                                 Via: SIP/2.0/UDP {}:{};rport;branch={}\r\n\
                                 From: <sip:{}@{}:{}>;tag={}\r\n\
                                 To: <sip:{}@{}:{}>\r\n\
                                 Call-ID: {}\r\n\
                                 CSeq: 1 MESSAGE\r\n\
                                 Max-Forwards: 70\r\n\
                                 Content-Type: Application/MANSCDP+xml\r\n\
                                 Content-Length: {}\r\n\
                                 \r\n\
                                 {}",
                                device.device_id, addr.ip(), addr.port(),
                                heartbeat_config.ip, heartbeat_config.port, branch,
                                heartbeat_config.device_id, heartbeat_config.ip, heartbeat_config.port, local_tag,
                                device.device_id, addr.ip(), addr.port(),
                                call_id,
                                keepalive_xml.len(),
                                keepalive_xml
                            );

                            if let Err(e) = heartbeat_socket.send_to(sip_msg.as_bytes(), addr).await {
                                tracing::debug!("Failed to send keepalive to {} at {}: {}", device.device_id, addr, e);
                            }
                        }
                    }
                }
            }
        });

        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }

    async fn handle_tcp_connection(
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        catalog_subscription_manager: &Arc<CatalogSubscriptionManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        pool: &Pool,
        conn_manager: &TcpConnectionManager,
    ) {
        // 创建一个虚拟 UDP socket 仅用于传递给 handle_packet's 接口
        // 实际回复通过 TcpConnectionManager.send_to 进行
        // 注意：这里使用一个专用的虚拟封装 TcpSendSocket
        

        // 创建 TCP 可写代理: 侧听 UDP socket 发出的内容将被拦截并通过 TCP 发出
        // 更简洁的方法：我们直接在这里处理消息和发送
        if let Some(conn) = conn_manager.get_connection(&addr).await {
            // 没有天然的，我们需要一个临时的 UDP socket 来将回复转发到 TCP
            // 创建一个虚拟 UDP socket，收到内容后再通过 TCP 发送
            // 为了简化：我们使用内部通道模式
            let _conn_mgr_clone = conn_manager.get_connection(&addr).await;
            let mut stream_guard = conn.write().await;
            loop {
                match stream_guard.read_message().await {
                    Ok(Some((msg, _peer))) => {
                        // 将 SipMessage 转回字节以便重新解析
                        let raw = format!("{}", msg);
                        let data_bytes = raw.as_bytes();

                        // 创建一个尠1次性 UDP 通道代替博羘的 socket
                        // 注意：这个临时 socket 不能真正回复 TCP
                        // 正确方案：直接调用 conn_manager.send_to 发送回复
                        let dummy_socket_result = tokio::net::UdpSocket::bind("0.0.0.0:0").await;
                        if let Ok(udp) = dummy_socket_result {
                            let _udp_arc = Arc::new(udp);
                            let _conn_mgr_for_reply = conn_manager.clone();
                            let _addr_for_reply = addr;
                            // 包裃一个代理子结构使 handle_packet 宽心发送的所有内容代行路由到 TCP
                            if let Err(e) = Self::process_tcp_message(
                                data_bytes,
                                addr,
                                config,
                                device_manager,
                                session_manager,
                                invite_session_manager,
                                talk_manager,
                                catalog_subscription_manager,
                                zlm_client,
                                pool,
                                conn_manager.clone(),
                            ).await {
                                tracing::error!("TCP SIP handler error: {}", e);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("TCP connection closed: {}", addr);
                        break;
                    }
                    Err(e) => {
                        tracing::error!("TCP read error from {}: {}", addr, e);
                        break;
                    }
                }
            }
            conn_manager.remove_connection(&addr).await;
        }
    }

    /// 处理来自 TCP 连接的 SIP 消息，通过 TcpConnectionManager 发送回复
    async fn process_tcp_message(
        data: &[u8],
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        catalog_subscription_manager: &Arc<CatalogSubscriptionManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        pool: &Pool,
        conn_manager: TcpConnectionManager,
    ) -> Result<()> {
        let msg = Parser::parse(data)?;
        match msg {
            SipMessage::Request(req) => {
                // 生成回复内容存入 buffer，然后通过 TCP 发送
                let (_response_tx, _response_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                // 创建一个内部虚拟 socket，用于捕获回复
                // 简化方式：创建一个局域 UDP socket 监听，得到地址后用于中转
                let dummy_socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
                let dummy_local = dummy_socket.local_addr()?;
                let dummy_arc = Arc::new(dummy_socket);

                // 异步启动: 监听 dummy socket 的内容并通过 TCP 发出
                let conn_mgr_clone = conn_manager.clone();
                let dummy_clone = dummy_arc.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65535];
                    while let Ok((n, _)) = dummy_clone.recv_from(&mut buf).await {
                        let data = buf[..n].to_vec();
                        if let Err(e) = conn_mgr_clone.send_to(&addr, &String::from_utf8_lossy(&data)).await {
                            tracing::error!("TCP send failed for {}: {}", addr, e);
                            break;
                        }
                    }
                });

                // 将 socket 发送到自身，这样 handle_packet 的回复就会被上面的 spawn 捕获
                dummy_arc.connect(dummy_local).await?;
                dummy_arc.send_to(&[], dummy_local).await?; // 就绪

                // 简化: 直接用 UDP 将回复发送到 TCP 代理
                // 最终实现：将 dummy_arc 绑定到 addr，这样 send_to 就会发到 TCP socket
                // 由于 UDP/TCP 工作机制不同，这里居中转发
                Self::handle_request(
                    req,
                    addr,
                    config,
                    device_manager,
                    session_manager,
                    invite_session_manager,
                    talk_manager,
                    catalog_subscription_manager,
                    zlm_client,
                    pool,
                    &dummy_arc,
                    &None,
                ).await
            }
            SipMessage::Response(resp) => {
                Self::handle_response(resp, session_manager, &Arc::new(DashMap::new()), &None).await
            }
        }
    }

    async fn handle_packet(
        data: &[u8],
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        catalog_subscription_manager: &Arc<CatalogSubscriptionManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
        _is_tcp: bool,
        ws_state: &Option<Arc<WsState>>,
        pending_invites: &Arc<DashMap<String, oneshot::Sender<String>>>,
        cascade_registrar: &Option<Arc<CascadeRegistrar>>,
    ) -> Result<()> {
        let msg = Parser::parse(data)?;
        match msg {
            SipMessage::Request(req) => {
                Self::handle_request(
                    req,
                    addr,
                    config,
                    device_manager,
                    session_manager,
                    invite_session_manager,
                    talk_manager,
                    catalog_subscription_manager,
                    zlm_client,
                    pool,
                    socket,
                    ws_state,
                )
                .await
            }
            SipMessage::Response(resp) => {
                Self::handle_response(resp, session_manager, pending_invites, cascade_registrar).await
            }
        }
    }

    async fn handle_request(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        catalog_subscription_manager: &Arc<CatalogSubscriptionManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
        ws_state: &Option<Arc<WsState>>,
    ) -> Result<()> {
        let method = req.method;
        match method {
            SipMethod::Register => {
                Self::handle_register(req, addr, config, device_manager, pool, socket, ws_state).await
            }
            SipMethod::Message => {
                Self::handle_message(req, addr, config, device_manager, pool, socket, ws_state, zlm_client).await
            }
            SipMethod::Invite => {
                Self::handle_invite(req, addr, config, session_manager, invite_session_manager, talk_manager, zlm_client, pool, socket).await
            }
            SipMethod::Ack => {
                Self::handle_ack(req, session_manager, invite_session_manager, talk_manager).await
            }
            SipMethod::Bye => {
                Self::handle_bye(req, session_manager, invite_session_manager, talk_manager, zlm_client, socket, addr).await
            }
            SipMethod::Options => {
                Self::handle_options(req, addr, config, socket).await
            }
            SipMethod::Info => {
                Self::handle_info(req, addr, config, pool, socket).await
            }
            SipMethod::Cancel => {
                Self::handle_cancel(req, addr, config, session_manager, invite_session_manager, talk_manager, zlm_client, pool, socket).await
            }
            SipMethod::Prack => {
                Self::handle_prack(req, addr, config, session_manager, socket).await
            }
            SipMethod::Update => {
                Self::handle_update(req, addr, config, session_manager, socket).await
            }
            SipMethod::Subscribe => {
                Self::handle_subscribe(req, addr, config, device_manager, catalog_subscription_manager, pool, socket).await
            }
            SipMethod::Notify => {
                Self::handle_notify(req, addr, config, pool, socket).await
            }
            SipMethod::Refer => {
                Self::handle_refer(req, addr, config, socket).await
            }
            _ => {
                tracing::warn!("Unhandled SIP method: {}", method.as_str());
                Self::send_error_response(501, "Not Implemented", &req, addr, socket).await
            }
        }
    }

    async fn handle_register(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
        ws_state: &Option<Arc<WsState>>,
    ) -> Result<()> {
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        let device_id = Self::extract_device_id(&from).or_else(|| Self::extract_device_id(&to)).unwrap_or_default();
        if device_id.is_empty() {
            tracing::warn!("REGISTER: Cannot extract device ID");
            return Ok(());
        }

        let expires: u64 = req.header("expires")
            .and_then(|s| s.parse().ok())
            .unwrap_or(config.register_timeout);

        let auth = req.header("authorization").cloned();
        
        if auth.is_none() {
            let nonce = generate_nonce();
            let auth_header = Parser::generate_www_authenticate_response(&config.realm, &nonce, None);
            let response = Parser::generate_response(401, "Unauthorized", &[
                ("Via", &via),
                ("From", &from),
                ("To", &to),
                ("Call-ID", &call_id),
                ("CSeq", &cseq),
                ("WWW-Authenticate", &auth_header),
            ], None);
            Self::send_response(socket, addr, &response).await?;
            tracing::info!("REGISTER from {} - Challenge sent (nonce: {})", device_id, nonce);
            return Ok(());
        }

        let auth_str = auth.unwrap();
        if !Self::validate_digest(&auth_str, &device_id, &config.password, &config.realm, &format!("sip:{}@{}:{}", device_id, config.ip, config.port)) {
            tracing::warn!("REGISTER from {} - Invalid credentials", device_id);
            let response = Parser::generate_response(403, "Forbidden", &[
                ("Via", &via), ("From", &from), ("To", &to),
                ("Call-ID", &call_id), ("CSeq", &cseq),
            ], None);
            Self::send_response(socket, addr, &response).await?;
            return Ok(());
        }

        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if expires == 0 {
            db_device::update_device_online(pool, &device_id, false, None, None, &now).await?;
            device_manager.unregister(&device_id).await;
            if let Some(ref ws) = ws_state {
                ws.broadcast("deviceOffline", serde_json::json!({
                    "deviceId": device_id,
                    "status": "offline",
                })).await;
            }
            tracing::info!("Device unregistered: {}", device_id);
        } else {
            let ip_str = addr.ip().to_string();
            db_device::upsert_device(pool, &device_id, None, None, None, None, None, None, 
                Some(&ip_str), Some(addr.port() as i32), true, Some("zlmediakit-1"), &now).await?;
            device_manager.register(&device_id, addr).await;
            if let Some(ref ws) = ws_state {
                ws.broadcast("deviceOnline", serde_json::json!({
                    "deviceId": device_id,
                    "status": "online",
                })).await;
            }
            tracing::info!("Device registered: {} (expires: {})", device_id, expires);
        }

        let to_tag = generate_tag();
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), to_tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>;expires={}", device_id, addr.ip(), addr.port(), expires)),
            ("Expires", &expires.to_string()),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        
        Ok(())
    }

    async fn handle_message(
        req: SipRequest,
        addr: SocketAddr,
        _config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
        ws_state: &Option<Arc<WsState>>,
        zlm_client: &Option<Arc<ZlmClient>>,
    ) -> Result<()> {
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let _content_type = req.header("content-type").cloned().unwrap_or_default();

        let device_id = Self::extract_device_id(&from).unwrap_or_default();
        let sn = req.header("cseq").and_then(|s| s.split_whitespace().nth(1)).unwrap_or("1").to_string();

        if let Some(body) = &req.body {
            let cmd_type = XmlParser::get_cmd_type(body);
            tracing::debug!("MESSAGE from {} - CmdType: {:?}", device_id, cmd_type);

            match cmd_type.as_deref() {
                Some("Keepalive") | Some("keepalive") => {
                    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    if let Some(dev_id) = XmlParser::get_device_id(body) {
                        let ip_str = addr.ip().to_string();
                        db_device::update_device_online(pool, &dev_id, true, Some(&ip_str), Some(addr.port() as i32), &now).await?;
                        device_manager.update_keepalive(&dev_id, addr).await;
                    }
                    tracing::debug!("Keepalive from device: {}", device_id);
                }
                Some("Catalog") => {
                    Self::handle_catalog(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket).await?;
                    return Ok(());
                }
                Some("DeviceInfo") => {
                    Self::handle_device_info(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket).await?;
                    return Ok(());
                }
                Some("DeviceStatus") => {
                    Self::handle_device_status(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket).await?;
                    return Ok(());
                }
                Some("MobilePosition") => {
                    Self::handle_mobile_position(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket).await?;
                    return Ok(());
                }
                Some("Alarm") => {
                    Self::handle_alarm(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket, ws_state).await?;
                    return Ok(());
                }
                Some("RecordInfo") => {
                    Self::handle_record_info(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket, zlm_client).await?;
                    return Ok(());
                }
                _ => {
                    tracing::debug!("Unhandled MESSAGE body: {}", body);
                }
            }
        }

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_invite(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        _pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let _content_type = req.header("content-type").cloned().unwrap_or_default();

        let from_device = Self::extract_device_id(&from).unwrap_or_default();
        let to_device = Self::extract_device_id(&to).unwrap_or_default();

        tracing::info!("INVITE from {} to {} - CallID: {}", from_device, to_device, call_id);

        let sdp_request_body = req.body.clone();
        let (stream_type, ssrc, _sdp_info) = if let Some(body) = &req.body {
            let sdp_info = Self::parse_sdp(body);
            let stream_type = sdp_info.get("s").cloned().unwrap_or_else(|| "Play".to_string());
            let ssrc = sdp_info.get("y").cloned();
            (stream_type, ssrc, Some(sdp_info))
        } else {
            ("Play".to_string(), None, None)
        };

        // Route to Talk handler for audio-only sessions
        if stream_type == "Talk" || stream_type == "InviteBack" {
            return Self::handle_talk_invite(
                req, addr, config, session_manager, talk_manager, zlm_client, socket
            ).await;
        }

        let channel_id = Self::extract_channel_id(&req.uri);
        if channel_id.is_empty() {
            tracing::warn!("Cannot extract channel ID from URI: {}", req.uri);
            Self::send_error_response(404, "Not Found", &req, addr, socket).await?;
            return Ok(());
        }

        session_manager.create(&call_id, &from_device, &channel_id, &stream_type).await;

        let tag = generate_tag();
        let branch = Self::get_branch(&via).unwrap_or_else(|| generate_branch());
        
        let response = Parser::generate_response(100, "Trying", &[
            ("Via", &format!("{};rport={};branch={}", via, addr.port(), branch)),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let response = Parser::generate_response(180, "Ringing", &[
            ("Via", &format!("{};rport={};branch={}", via, addr.port(), branch)),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
            ("RSeq", "1"),
        ], None);
        Self::send_response(socket, addr, &response).await?;

        let mut media_port = 10000u16;
        let mut zlm_stream_key: Option<String> = None;
        let mut error_occurred = false;

        if let Some(zlm) = zlm_client {
            if let Some(ref body) = sdp_request_body {
                if let Some(info) = SdpInfo::parse(body) {
                    let device_port = info.get_video_port().unwrap_or(5000);
                    
                    let rtp_server = zlm.open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                        secret: zlm.secret.clone(),
                        stream_id: format!("{}${}", from_device, channel_id),
                        port: None,
                        use_tcp: Some(false),
                        rtp_type: Some(0),
                        recv_port: None,
                    }).await;

                    match rtp_server {
                        Ok(rtp_info) => {
                            tracing::info!("RTP server opened: port={}, stream_id={}", rtp_info.port, rtp_info.stream_id);
                            media_port = rtp_info.port;
                            zlm_stream_key = Some(rtp_info.stream_id);
                            
                            let device_ip = if let Some(received) = Self::get_received_from_via(&via) {
                                received
                            } else {
                                addr.ip().to_string()
                            };
                            
                            let add_proxy_req = crate::zlm::AddStreamProxyRequest {
                                secret: zlm.secret.clone(),
                                vhost: "__defaultVhost__".to_string(),
                                app: "rtp".to_string(),
                                stream: format!("{}${}", from_device, channel_id),
                                url: format!("rtsp://{}:{}/{}", device_ip, device_port, channel_id),
                                rtp_type: Some(0),
                                timeout_sec: Some(3600.0),
                                enable_hls: Some(false),
                                enable_mp4: Some(false),
                                enable_rtsp: Some(true),
                                enable_rtmp: Some(false),
                                enable_fmp4: Some(false),
                                enable_ts: Some(false),
                                enableAAC: Some(false),
                            };

                            match zlm.add_stream_proxy(&add_proxy_req).await {
                                Ok(proxy_key) => {
                                    tracing::info!("ZLM stream proxy started: {}", proxy_key);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to start ZLM stream proxy: {}", e);
                                    error_occurred = true;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to open RTP server: {}", e);
                            error_occurred = true;
                        }
                    }
                }
            }
        }

        if error_occurred {
            Self::send_error_response(503, "Service Unavailable", &req, addr, socket).await?;
            return Ok(());
        }

        let ssrc_str = ssrc.unwrap_or_else(|| "0100000001".to_string());
        let response_body = build_invite_sdp(&config.ip, media_port, &stream_type, Some(&ssrc_str));
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &format!("{};rport={};branch={}", via, addr.port(), branch)),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
            ("Content-Type", "Application/SDP"),
        ], Some(&response_body));
        Self::send_response(socket, addr, &response).await?;
        
        if let Some(ref stream_key) = zlm_stream_key {
            let mut invite_session = crate::sip::gb28181::invite_session::InviteSession::new(
                &call_id,
                &from_device,
                &channel_id,
                crate::sip::gb28181::invite_session::StreamType::Play,
                addr,
            );
            invite_session.set_sdp(sdp_request_body.as_deref().unwrap_or(""));
            invite_session.set_zlm_stream(stream_key, "rtp");
            invite_session.status = InviteSessionStatus::Ringing;
            invite_session_manager.create(invite_session).await;
        }
        
        tracing::info!("INVITE 200 OK sent - stream: {}, port: {}", stream_type, media_port);
        Ok(())
    }

    async fn handle_talk_invite(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        _session_manager: &Arc<SessionManager>,
        talk_manager: &Arc<TalkManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        let from_device = Self::extract_device_id(&from).unwrap_or_default();
        let channel_id = Self::extract_channel_id(&req.uri);

        tracing::info!("TALK INVITE from {} channel {} - CallID: {}", from_device, channel_id, call_id);

        let tag = generate_tag();
        let branch = Self::get_branch(&via).unwrap_or_else(|| generate_branch());

        let response = Parser::generate_response(100, "Trying", &[
            ("Via", &format!("{};rport={};branch={}", via, addr.port(), branch)),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;

        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut media_port = 15000u16;
        let mut zlm_stream_id: Option<String> = None;

        if let Some(zlm) = zlm_client {
            let device_ip = if let Some(received) = Self::get_received_from_via(&via) {
                received
            } else {
                addr.ip().to_string()
            };

            let talk_stream_id = format!("talk/{}/{}", from_device, channel_id);

            let rtp_server = zlm.open_rtp_server(&crate::zlm::OpenRtpServerRequest {
                secret: zlm.secret.clone(),
                stream_id: talk_stream_id.clone(),
                port: None,
                use_tcp: Some(false),
                rtp_type: Some(1),
                recv_port: None,
            }).await;

            match rtp_server {
                Ok(rtp_info) => {
                    tracing::info!("Talk RTP server opened: port={}, stream_id={}", rtp_info.port, rtp_info.stream_id);
                    media_port = rtp_info.port;
                    zlm_stream_id = Some(rtp_info.stream_id);

                    let add_proxy_req = crate::zlm::AddStreamProxyRequest {
                        secret: zlm.secret.clone(),
                        vhost: "__defaultVhost__".to_string(),
                        app: "rtp".to_string(),
                        stream: talk_stream_id.clone(),
                        url: format!("rtsp://{}:8554/{}", device_ip, channel_id),
                        rtp_type: Some(1),
                        timeout_sec: Some(3600.0),
                        enable_hls: Some(false),
                        enable_mp4: Some(false),
                        enable_rtsp: Some(true),
                        enable_rtmp: Some(false),
                        enable_fmp4: Some(false),
                        enable_ts: Some(false),
                        enableAAC: Some(true),
                    };

                    match zlm.add_stream_proxy(&add_proxy_req).await {
                        Ok(proxy_key) => {
                            tracing::info!("Talk ZLM stream proxy started: {}", proxy_key);
                        }
                        Err(e) => {
                            tracing::error!("Failed to start talk ZLM stream proxy: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to open talk RTP server: {}", e);
                    Self::send_error_response(503, "Service Unavailable", &req, addr, socket).await?;
                    return Ok(());
                }
            }
        }

        let mut talk_session = crate::sip::gb28181::talk::TalkSession::new(&call_id, &from_device, &channel_id);
        talk_session.set_device_info(&addr.ip().to_string(), addr.port());
        talk_session.set_local_port(media_port);
        if let Some(ref stream_id) = zlm_stream_id {
            talk_session.set_zlm_stream(stream_id);
        }
        talk_session.status = TalkStatus::Ringing;
        talk_manager.create(&call_id, &from_device, &channel_id).await;
        talk_manager.update(&talk_session).await;

        let response_body = build_audio_sdp(&config.ip, media_port);
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &format!("{};rport={};branch={}", via, addr.port(), branch)),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
            ("Content-Type", "Application/SDP"),
        ], Some(&response_body));
        Self::send_response(socket, addr, &response).await?;

        tracing::info!("TALK INVITE 200 OK sent - port: {}", media_port);
        Ok(())
    }

    async fn handle_ack(
        req: SipRequest,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
    ) -> Result<()> {
        let call_id = req.call_id().cloned().unwrap_or_default();
        session_manager.update_status(&call_id, SessionStatus::Active).await;
        
        if let Some(mut session) = invite_session_manager.get(&call_id).await {
            session.status = InviteSessionStatus::Active;
            session.update_activity();
            invite_session_manager.update(&session).await;
        }
        
        if let Some(mut session) = talk_manager.get(&call_id).await {
            session.status = TalkStatus::Active;
            session.update_activity();
            talk_manager.update(&session).await;
        }
        
        tracing::info!("ACK received - CallID: {}", call_id);
        Ok(())
    }

    async fn handle_bye(
        req: SipRequest,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        socket: &Arc<UdpSocket>,
        addr: SocketAddr,
    ) -> Result<()> {
        let call_id = req.call_id().cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        if let Some(mut session) = invite_session_manager.get(&call_id).await {
            if let Some(ref zlm) = zlm_client {
                if let Some(ref stream_id) = session.zlm_stream_id {
                    if let Err(e) = zlm.close_rtp_server(stream_id).await {
                        tracing::error!("Failed to close ZLM stream: {}", e);
                    } else {
                        tracing::info!("ZLM stream closed: {}", stream_id);
                    }
                }
            }
            session.status = InviteSessionStatus::Terminated;
            invite_session_manager.update(&session).await;
        }

        if let Some(mut session) = talk_manager.get(&call_id).await {
            if let Some(ref zlm) = zlm_client {
                if let Some(ref stream_id) = session.zlm_stream_id {
                    if let Err(e) = zlm.close_rtp_server(stream_id).await {
                        tracing::error!("Failed to close talk ZLM stream: {}", e);
                    } else {
                        tracing::info!("Talk ZLM stream closed: {}", stream_id);
                    }
                }
            }
            session.status = TalkStatus::Terminated;
            talk_manager.update(&session).await;
        }

        session_manager.remove(&call_id).await;
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        
        Self::send_response(socket, addr, &response).await?;
        tracing::info!("BYE received - CallID: {} - Session terminated", call_id);
        Ok(())
    }

    async fn handle_options(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Allow", "INVITE, ACK, OPTIONS, INFO, BYE, CANCEL, NOTIFY, MESSAGE, REFER, PRACK, UPDATE, SUBSCRIBE"),
            ("Accept", "APPLICATION/SDP, APPLICATION/MANSCDP+XML, APPLICATION/MESSAGE+XML, MULTIPART/MIXED"),
            ("Supported", "replaces,100rel,eventlist"),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
            ("User-Agent", "GBServer/1.0"),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_info(
        req: SipRequest,
        addr: SocketAddr,
        _config: &Arc<SipConfig>,
        _pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        if let Some(body) = &req.body {
            let cmd_type = XmlParser::get_cmd_type(body);
            tracing::debug!("INFO CmdType: {:?}", cmd_type);

            if cmd_type.as_deref() == Some("DeviceControl") {
                let ptz_cmd = Self::parse_ptz_cmd(body);
                if let Some(cmd) = ptz_cmd {
                    tracing::info!("PTZ Command: {}", cmd);
                }
            }
        }

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_cancel(
        req: SipRequest,
        addr: SocketAddr,
        _config: &Arc<SipConfig>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        _pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        tracing::info!("CANCEL received - CallID: {}", call_id);
        
        if let Some(mut session) = invite_session_manager.get(&call_id).await {
            if let Some(ref zlm) = zlm_client {
                if let Some(ref stream_id) = session.zlm_stream_id {
                    if let Err(e) = zlm.close_rtp_server(stream_id).await {
                        tracing::error!("Failed to close ZLM stream on CANCEL: {}", e);
                    } else {
                        tracing::info!("ZLM stream closed on CANCEL: {}", stream_id);
                    }
                }
            }
            session.status = InviteSessionStatus::Terminated;
            invite_session_manager.update(&session).await;
        }
        
        if let Some(mut session) = talk_manager.get(&call_id).await {
            if let Some(ref zlm) = zlm_client {
                if let Some(ref stream_id) = session.zlm_stream_id {
                    if let Err(e) = zlm.close_rtp_server(stream_id).await {
                        tracing::error!("Failed to close talk ZLM stream on CANCEL: {}", e);
                    } else {
                        tracing::info!("Talk ZLM stream closed on CANCEL: {}", stream_id);
                    }
                }
            }
            session.status = TalkStatus::Terminated;
            talk_manager.update(&session).await;
        }
        
        session_manager.remove(&call_id).await;
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        
        Self::send_response(socket, addr, &response).await?;
        
        let response = Parser::generate_response(487, "Request Terminated", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        
        Ok(())
    }

    async fn handle_prack(
        req: SipRequest,
        addr: SocketAddr,
        _config: &Arc<SipConfig>,
        _session_manager: &Arc<SessionManager>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let rack = req.header("rack").cloned().unwrap_or_default();

        tracing::info!("PRACK received - CallID: {}, RAck: {}", call_id, rack);

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_update(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        _session_manager: &Arc<SessionManager>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        tracing::info!("UPDATE received - CallID: {}", call_id);

        let response_body = if let Some(_body) = &req.body {
            Self::generate_sdp_response(&config.device_id, "Update", None, None)
        } else {
            String::new()
        };

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
            ("Content-Type", "Application/SDP"),
        ], if !response_body.is_empty() { Some(&response_body) } else { None });
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_subscribe(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        _device_manager: &Arc<DeviceManager>,
        catalog_subscription_manager: &Arc<CatalogSubscriptionManager>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let event = req.header("event").cloned().unwrap_or_default();
        let expires: u32 = req.header("expires")
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        tracing::info!("SUBSCRIBE received - CallID: {}, Event: {}", call_id, event);

        let tag = generate_tag();
        let from_tag = Self::extract_tag_from_header(&from).unwrap_or_else(generate_tag);
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), tag)),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
            ("Expires", &expires.to_string()),
            ("Allow-Events", "presence,message-summary,catalog,keep-alive"),
        ], None);
        Self::send_response(socket, addr, &response).await?;

        if expires > 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            let event_lower = event.to_lowercase();
            let notify_body;
            
            if event_lower.contains("catalog") {
                let device_id = Self::extract_device_id(&from).unwrap_or_default();
                if !device_id.is_empty() {
                    let channels = db_device::list_channels_for_device(pool, &device_id).await
                        .unwrap_or_default();
                    notify_body = build_catalog_notify_body(&channels, 1, &device_id);
                    
                    let subscription = crate::sip::gb28181::catalog::CatalogSubscription::new(
                        &call_id,
                        &device_id,
                        addr,
                        &via,
                        &from_tag,
                        &tag,
                        expires,
                    );
                    catalog_subscription_manager.subscribe(subscription).await;
                    tracing::info!("Catalog subscription stored for device: {}", device_id);
                } else {
                    notify_body = Self::generate_notify_body(&event);
                }
            } else {
                notify_body = Self::generate_notify_body(&event);
            }
            
            let notify_response = Parser::generate_notify(
                &format!("sip:{}@{}:{}", config.device_id, addr.ip(), addr.port()),
                &via,
                &format!("<sip:{}@{}:{}>;tag={}", config.device_id, config.ip, config.port, tag),
                &to,
                &call_id,
                1,
                &event,
                "active;expires=300",
            );
            let notify_with_body = format!("{}\r\n\r\n{}", notify_response.trim_end(), notify_body);
            Self::send_response(socket, addr, &notify_with_body).await?;
        } else {
            catalog_subscription_manager.unsubscribe(&call_id).await;
            tracing::info!("Catalog subscription removed: {}", call_id);
        }

        Ok(())
    }

    async fn handle_notify(
        req: SipRequest,
        addr: SocketAddr,
        _config: &Arc<SipConfig>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let event = req.header("event").cloned().unwrap_or_default();
        let subscription_state = req.header("subscription-state").cloned().unwrap_or_default();

        tracing::info!("NOTIFY received - CallID: {}, Event: {}, State: {}", 
            call_id, event, subscription_state);

        if let Some(body) = &req.body {
            let cmd_type = XmlParser::get_cmd_type(body);
            match cmd_type.as_deref() {
                Some("Catalog") => {
                    tracing::debug!("NOTIFY Catalog body: {}", body);
                    let device_id_for_catalog = XmlParser::get_device_id(body)
                        .unwrap_or_else(|| Self::extract_device_id(&from).unwrap_or_default());
                    let (sum_num, channels) = XmlParser::parse_catalog_channels(body);
                    tracing::info!("Catalog NOTIFY from {}: {} channels (SumNum={:?})", 
                        device_id_for_catalog, channels.len(), sum_num);

                    for ch in &channels {
                        let status = if ch.status == "ON" || ch.status == "online" { true } else { false };
                        let parent_id = ch.parent_id.as_deref().or(Some(&device_id_for_catalog));
                        match db_device::upsert_channel_from_catalog(
                            pool,
                            &device_id_for_catalog,
                            &ch.device_id,
                            &ch.name,
                            ch.manufacturer.as_deref(),
                            ch.model.as_deref(),
                            ch.owner.as_deref(),
                            ch.civil_code.as_deref(),
                            ch.address.as_deref(),
                            parent_id,
                            status,
                            ch.longitude,
                            ch.latitude,
                            ch.ptz_type,
                            ch.has_audio,
                            ch.sub_count,
                        ).await {
                            Ok(_) => {},
                            Err(e) => tracing::warn!("Failed to upsert channel {}: {}", ch.device_id, e),
                        }
                    }

                    if let (Some(total), false) = (sum_num, channels.is_empty()) {
                        if (channels.len() as i32) < total {
                            tracing::info!("Partial catalog received: {}/{}, more expected", channels.len(), total);
                        }
                    }
                }
                _ => {
                    tracing::debug!("NOTIFY body: {}", body);
                }
            }
        }

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_refer(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let refer_to = req.header("refer-to").cloned().unwrap_or_default();

        tracing::info!("REFER received - CallID: {}, Refer-To: {}", call_id, refer_to);

        let response = Parser::generate_response(202, "Accepted", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port)),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn send_error_response(
        status_code: u16,
        reason: &str,
        req: &SipRequest,
        addr: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        let response = Parser::generate_response(status_code, reason, &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
        ], None);
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_response(
        resp: SipResponse,
        session_manager: &Arc<SessionManager>,
        pending_invites: &Arc<DashMap<String, oneshot::Sender<String>>>,
        cascade_registrar: &Option<Arc<CascadeRegistrar>>,
    ) -> Result<()> {
        let call_id = resp.headers.get("call-id").cloned().unwrap_or_default();
        let cseq = resp.headers.get("cseq").cloned().unwrap_or_default();

        tracing::debug!("SIP Response: {} {} - CallID: {}", resp.status_code(), resp.reason, call_id);

        // Route REGISTER responses to cascade registrar
        if cseq.contains("REGISTER") {
            if let Some(ref registrar) = cascade_registrar {
                let platform_id = call_id
                    .strip_prefix("cascade_")
                    .and_then(|s| s.rsplit('_').next())
                    .map(|s| s.to_string());

                if let Some(ref pid) = platform_id {
                    if resp.status_code() == 401 || resp.status_code() == 407 {
                        let nonce = resp.headers.get("www-authenticate")
                            .or_else(|| resp.headers.get("proxy-authenticate"))
                            .and_then(|auth| {
                                auth.split("nonce=\"")
                                    .nth(1)
                                    .and_then(|s| s.split('"').next())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_default();
                        let opaque = resp.headers.get("www-authenticate")
                            .or_else(|| resp.headers.get("proxy-authenticate"))
                            .and_then(|auth| {
                                auth.split("opaque=\"")
                                    .nth(1)
                                    .and_then(|s| s.split('"').next())
                                    .map(|s| s.to_string())
                            });
                        let realm = resp.headers.get("www-authenticate")
                            .or_else(|| resp.headers.get("proxy-authenticate"))
                            .and_then(|auth| {
                                auth.split("realm=\"")
                                    .nth(1)
                                    .and_then(|s| s.split('"').next())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_default();
                        if !nonce.is_empty() {
                            registrar.handle_401_challenge(pid, &nonce, opaque.as_deref(), &realm);
                            tracing::info!("Cascade {} received 401 challenge", pid);
                        }
                    } else if resp.status_code() == 200 {
                        registrar.mark_registered(pid);
                    } else if resp.status_code() >= 400 {
                        registrar.mark_failed(pid);
                    }
                }
            }
            return Ok(());
        }

        if resp.status_code() == 200 {
            if cseq.contains("INVITE") {
                session_manager.update_status(&call_id, SessionStatus::Ringing).await;
                if let Some((_, tx)) = pending_invites.remove(&call_id) {
                    let contact = resp.headers.get("contact").cloned().unwrap_or_default();
                    let _ = tx.send(contact);
                }
            } else if cseq.contains("BYE") {
                session_manager.remove(&call_id).await;
            }
        } else if resp.status_code() == 487 {
            session_manager.remove(&call_id).await;
            if let Some((_, tx)) = pending_invites.remove(&call_id) {
                let _ = tx.send(String::new());
            }
        } else if resp.status_code() >= 400 {
            if let Some((_, tx)) = pending_invites.remove(&call_id) {
                let _ = tx.send(String::new());
            }
        }

        Ok(())
    }

    async fn handle_catalog(
        _body: &str,
        device_id: &str,
        sn: &str,
        pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        tracing::info!("Catalog query from {}", device_id);
        
        let channels = match db_device::list_channels_for_device(pool, device_id).await {
            Ok(chs) => chs,
            Err(e) => {
                tracing::error!("Failed to query channels for {}: {}", device_id, e);
                Vec::new()
            }
        };
        
        let mut channel_xml = String::new();
        for ch in &channels {
            let name = ch.name.as_deref().unwrap_or("未知通道");
            let gb_id = ch.gb_device_id.as_deref().unwrap_or("");
            let status = ch.status.as_deref().unwrap_or("OFF");
            let has_audio = ch.has_audio.unwrap_or(false);
            
            channel_xml.push_str(&format!(
                r#"<Item>
<DeviceID>{}</DeviceID>
<Name>{}</Name>
<Status>{}</Status>
<ParentID>{}</ParentID>
<Online>{}</Online>
<Status>{}</Status>
<SubCount>{}</SubCount>
<HasAudio>{}</HasAudio>
</Item>"#,
                gb_id, name, status, device_id,
                if status == "ON" { "true" } else { "false" },
                status,
                ch.sub_count.unwrap_or(0),
                has_audio
            ));
        }
        
        let num = channels.len();
        let response_body = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<SumNum>{}</SumNum>
<DeviceList Num="{}">{}</DeviceList>
</Response>"#, sn, device_id, num, num, channel_xml);
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", via), ("From", from), ("To", to),
            ("Call-ID", call_id), ("CSeq", cseq),
            ("Content-Type", "Application/MANSCDP+xml"),
        ], Some(&response_body));
        
        Self::send_response(socket, addr, &response).await?;
        tracing::info!("Catalog response sent: {} channels for {}", num, device_id);
        Ok(())
    }

    async fn handle_device_info(
        _body: &str,
        device_id: &str,
        sn: &str,
        pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        tracing::info!("DeviceInfo query from {}", device_id);
        
        let db_device = db_device::get_device_by_device_id(pool, device_id).await?;
        let (name, manufacturer, model) = if let Some(d) = db_device {
            (
                d.name.unwrap_or_else(|| "Unknown".to_string()),
                d.manufacturer.unwrap_or_else(|| "Unknown".to_string()),
                d.model.unwrap_or_else(|| "Unknown".to_string()),
            )
        } else {
            ("Unknown Device".to_string(), "Unknown".to_string(), "Unknown".to_string())
        };

        let response_body = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>DeviceInfo</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result><DeviceName>{}</DeviceName><Manufacturer>{}</Manufacturer><Model>{}</Model><Channel>1</Channel></Response>"#,
            sn, device_id, name, manufacturer, model);
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", via), ("From", from), ("To", to),
            ("Call-ID", call_id), ("CSeq", cseq),
            ("Content-Type", "Application/MANSCDP+xml"),
        ], Some(&response_body));
        
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_device_status(
        _body: &str,
        device_id: &str,
        sn: &str,
        _pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        tracing::debug!("DeviceStatus from {}", device_id);
        
        let response_body = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>DeviceStatus</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result><Online>ON</Online><Status>OK</Status><DeviceTime>{}</DeviceTime></Response>"#,
            sn, device_id, Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string());
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", via), ("From", from), ("To", to),
            ("Call-ID", call_id), ("CSeq", cseq),
            ("Content-Type", "Application/MANSCDP+xml"),
        ], Some(&response_body));
        
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_mobile_position(
        body: &str,
        device_id: &str,
        sn: &str,
        pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let parsed = XmlParser::parse(body);
        let time = parsed.get("Time").cloned().unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string());
        let longitude: f64 = parsed.get("Longitude").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let latitude: f64 = parsed.get("Latitude").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let speed: f64 = parsed.get("Speed").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let direction: f64 = parsed.get("Direction").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let altitude: f64 = parsed.get("Altitude").and_then(|s| s.parse().ok()).unwrap_or(0.0);

        tracing::info!("MobilePosition from {}: {}, {} (speed: {}, direction: {})", 
            device_id, longitude, latitude, speed, direction);
        // Persist position history to DB
        let _ = ph::insert_position(
            pool,
            device_id,
            &time,
            longitude,
            latitude,
            altitude,
            speed,
            direction,
        ).await;
        
        let response_body = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>MobilePosition</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result></Response>"#,
            sn, device_id);
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", via), ("From", from), ("To", to),
            ("Call-ID", call_id), ("CSeq", cseq),
            ("Content-Type", "Application/MANSCDP+xml"),
        ], Some(&response_body));
        
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_alarm(
        body: &str,
        device_id: &str,
        sn: &str,
        pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
        ws_state: &Option<Arc<WsState>>,
    ) -> Result<()> {
        let parsed = XmlParser::parse_fields(body);
        
        let alarm_type = parsed.get("AlarmType").cloned().unwrap_or_else(|| "Unknown".to_string());
        let alarm_priority = parsed.get("AlarmPriority").cloned();
        let alarm_method = parsed.get("AlarmMethod").cloned();
        let alarm_time = parsed.get("AlarmTime").cloned();
        let alarm_description = parsed.get("AlarmDescription").cloned();
        let channel_id = parsed.get("DeviceID").cloned().unwrap_or_else(|| device_id.to_string());
        
        let longitude = parsed.get("Longitude").and_then(|s| s.parse::<f64>().ok());
        let latitude = parsed.get("Latitude").and_then(|s| s.parse::<f64>().ok());
        
        let create_time = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        
        tracing::info!(
            "Alarm from {}: type={}, priority={:?}, method={:?}, channel={}",
            device_id, alarm_type, alarm_priority, alarm_method, channel_id
        );
        
        let alarm = crate::db::alarm::AlarmInsert {
            device_id: device_id.to_string(),
            channel_id: channel_id.clone(),
            alarm_priority: alarm_priority.clone(),
            alarm_method: alarm_method.clone(),
            alarm_time: alarm_time.clone(),
            alarm_description: alarm_description.clone(),
            longitude,
            latitude,
            alarm_type: Some(alarm_type.clone()),
            create_time: create_time.clone(),
        };
        
        if let Err(e) = crate::db::alarm::insert_alarm(pool, &alarm).await {
            tracing::error!("Failed to insert alarm to database: {}", e);
        }
        
        if let Some(ref ws) = ws_state {
            let alarm_data = serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "alarmType": alarm_type,
                "alarmPriority": alarm_priority,
                "alarmMethod": alarm_method,
                "alarmTime": alarm_time,
                "alarmDescription": alarm_description,
                "longitude": longitude,
                "latitude": latitude,
                "createTime": create_time,
            });
            ws.broadcast("alarm", alarm_data).await;
        }
        
        let response_body = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>Alarm</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><Result>OK</Result></Response>"#,
            sn, device_id);
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", via), ("From", from), ("To", to),
            ("Call-ID", call_id), ("CSeq", cseq),
            ("Content-Type", "Application/MANSCDP+xml"),
        ], Some(&response_body));
        
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    async fn handle_record_info(
        body: &str,
        device_id: &str,
        sn: &str,
        _pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
        zlm_client: &Option<Arc<ZlmClient>>,
    ) -> Result<()> {
        tracing::debug!("RecordInfo from {}", device_id);
        
        // 解析 RecordInfo 请求中的参数
        let fields = XmlParser::parse_fields(body);
        let target_device_id = fields.get("DeviceID").map(|s| s.as_str()).unwrap_or(device_id);
        let start_time = fields.get("StartTime").map(|s| s.as_str());
        let end_time = fields.get("EndTime").map(|s| s.as_str());
        let secrecy = fields.get("Secrecy").map(|s| s.as_str()).unwrap_or("0");
        let _type = fields.get("Type").map(|s| s.as_str()).unwrap_or("all");
        
        tracing::debug!(
            "RecordInfo query: device={}, start={:?}, end={:?}, type={}",
            target_device_id, start_time, end_time, _type
        );
        
        // 尝试从 ZLM 查询录像文件
        let mut record_items = Vec::new();
        let mut sum_num = 0;
        
        if let Some(zlm) = zlm_client {
            // 解析通道ID：target_device_id 可能是通道ID（20位）
            // 录像流的 app 通常是 "rtp"，stream 是 "device_id$channel_id" 格式
            let app = "rtp";
            
            // 尝试两种 stream 格式：
            // 1. device_id$channel_id (如果 target_device_id 是通道ID)
            // 2. target_device_id (如果 target_device_id 本身就是 stream)
            let stream1 = format!("{}${}", device_id, target_device_id);
            let stream2 = target_device_id.to_string();
            
            // 转换时间格式：GB28181 使用 "yyyy-MM-dd HH:mm:ss"，ZLM 使用时间戳或相同格式
            let zlm_start = start_time.map(|s| s.replace(" ", "T").replace(":", "-"));
            let zlm_end = end_time.map(|s| s.replace(" ", "T").replace(":", "-"));
            
            // 尝试查询录像文件
            let files = if let Ok(list) = zlm.get_mp4_record_file(app, &stream1, None, zlm_start.as_deref(), zlm_end.as_deref()).await {
                list
            } else if let Ok(list) = zlm.get_mp4_record_file(app, &stream2, None, zlm_start.as_deref(), zlm_end.as_deref()).await {
                list
            } else {
                Vec::new()
            };
            
            // 转换为 GB28181 RecordItem 格式
            for file in files {
                // 解析文件名获取时间信息
                let name = &file.name;
                let start = file.create_time.clone();
                let duration = file.duration.unwrap_or(0.0) as i64;
                
                // 计算结束时间
                let end = if duration > 0 {
                    // 简单处理：假设 create_time 是开始时间
                    format!("{}+{}s", start, duration)
                } else {
                    start.clone()
                };
                
                record_items.push(format!(
                    r#"<Item>
<DeviceID>{}</DeviceID>
<Name>{}</Name>
<FilePath>{}</FilePath>
<Address>{}</Address>
<StartTime>{}</StartTime>
<EndTime>{}</EndTime>
<Secrecy>{}</Secrecy>
<Type>{}</Type>
<RecorderID>{}</RecorderID>
<FileSize>{}</FileSize>
</Item>"#,
                    target_device_id,
                    name,
                    file.path,
                    file.path,
                    start,
                    end,
                    secrecy,
                    _type,
                    device_id,
                    file.size
                ));
                sum_num += 1;
            }
        }
        
        // 构建响应 XML
        let record_list = if record_items.is_empty() {
            "".to_string()
        } else {
            format!("<RecordList Num=\"{}\">\n{}\n</RecordList>", sum_num, record_items.join("\n"))
        };
        
        let response_body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>RecordInfo</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Name></Name>
<SumNum>{}</SumNum>
{}
</Response>"#,
            sn, target_device_id, sum_num, record_list
        );
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", via), ("From", from), ("To", to),
            ("Call-ID", call_id), ("CSeq", cseq),
            ("Content-Type", "Application/MANSCDP+xml"),
        ], Some(&response_body));
        
        Self::send_response(socket, addr, &response).await?;
        Ok(())
    }

    fn generate_notify_body(event: &str) -> String {
        match event.to_lowercase().as_str() {
            "presence" => r#"<?xml version="1.0" encoding="UTF-8"?>
<presence xmlns="urn:ietf:params:xml:ns:pidf" entity="sip:device@example.com">
  <tuple id="device">
    <status><basic>open</basic></status>
  </tuple>
</presence>"#.to_string(),
            "catalog" => r#"<?xml version="1.0" encoding="UTF-8"?>
<Notify>
  <CmdType>Catalog</CmdType>
  <SN>1</SN>
  <DeviceID>device001</DeviceID>
  <SumNum>0</SumNum>
  <DeviceList Num="0"></DeviceList>
</Notify>"#.to_string(),
            _ => format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
  <CmdType>{}</CmdType>
  <SN>1</SN>
  <DeviceID>device001</DeviceID>
  <Result>OK</Result>
</Response>"#, event),
        }
    }

    async fn send_response(socket: &Arc<UdpSocket>, addr: SocketAddr, response: &str) -> Result<()> {
        socket.send_to(response.as_bytes(), addr).await?;
        Ok(())
    }

    fn extract_device_id(sip_uri: &str) -> Option<String> {
        let uri = sip_uri.trim();
        let start = uri.find('<')? + 1;
        let end = uri.find('>').unwrap_or(uri.len());
        let uri = &uri[start..end];
        let uri = uri.trim_start_matches("sip:");
        let parts: Vec<&str> = uri.split('@').collect();
        let user = parts.first()?;
        let user = user.trim();
        if user.len() == 20 || user.len() == 22 {
            Some(user.to_string())
        } else {
            None
        }
    }

    fn extract_channel_id(uri: &str) -> String {
        let uri = uri.trim();
        if let Some(pos) = uri.find('@') {
            uri[..pos].trim_start_matches("sip:").to_string()
        } else {
            uri.trim_start_matches("sip:").to_string()
        }
    }

    fn get_branch(via: &str) -> Option<String> {
        for part in via.split(';') {
            if part.trim().starts_with("branch=") {
                return Some(part.trim_start_matches("branch=").to_string());
            }
        }
        None
    }

    fn parse_sdp(sdp: &str) -> std::collections::HashMap<String, String> {
        let mut info = std::collections::HashMap::new();
        for line in sdp.lines() {
            if let Some(pos) = line.find('=') {
                let key = line[..pos].to_string();
                let value = line[pos + 1..].to_string();
                info.insert(key, value);
            }
        }
        info
    }

    fn generate_sdp_response(device_id: &str, mode: &str, video_port: Option<u16>, audio_port: Option<u16>) -> String {
        let v_port = video_port.unwrap_or(10000);
        let _a_port = audio_port.unwrap_or(0);
        
        format!(r#"v=0
o={} 0 0 IN IP4 127.0.0.1
s={}
c=IN IP4 127.0.0.1
t=0 0
m=video {} RTP/AVP 96
a=rtpmap:96 PS/90000
a=sendonly
y=0100000001
f=v/1/96/1/2/1/1/0
"#,
            device_id, mode, v_port)
    }

    fn parse_ptz_cmd(body: &str) -> Option<String> {
        let parsed = XmlParser::parse(body);
        parsed.get("PTZCmd").cloned()
    }

    fn validate_digest(auth: &str, username: &str, password: &str, realm: &str, uri: &str) -> bool {
        let mut params = std::collections::HashMap::new();
        for part in auth.split(',') {
            let part = part.trim();
            if let Some(pos) = part.find('=') {
                let key = part[..pos].trim().to_string();
                let mut value = part[pos + 1..].trim().to_string();
                value = value.trim_matches('"').to_string();
                params.insert(key, value);
            }
        }

        let username2 = params.get("username").map(|s| s.as_str()).unwrap_or("");
        let response = params.get("response").map(|s| s.as_str()).unwrap_or("");
        let nonce = params.get("nonce").map(|s| s.as_str()).unwrap_or("");
        let qop = params.get("qop").map(|s| s.as_str()).unwrap_or("auth");

        if username2 != username {
            return false;
        }

        let ha1 = format!("{:x}", Md5::digest(format!("{}:{}:{}", username, realm, password)));
        let ha2 = format!("{:x}", Md5::digest(format!("{}:{}", "REGISTER", uri)));
        let expected = if qop == "auth" {
            let cnonce = "00000001";
            format!("{:x}", Md5::digest(format!("{}:{}:{}:{}:{}", ha1, nonce, cnonce, "auth", ha2)))
        } else {
            format!("{:x}", Md5::digest(format!("{}:{}:{}", ha1, nonce, ha2)))
        };

        expected == response
    }

    pub fn device_manager(&self) -> Arc<DeviceManager> {
        self.device_manager.clone()
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    pub fn catalog_subscription_manager(&self) -> Arc<CatalogSubscriptionManager> {
        self.catalog_subscription_manager.clone()
    }

    pub async fn send_sip_message(&self, addr: SocketAddr, message: &str) -> Result<()> {
        let socket = self.socket.read().await;
        if let Some(ref sock) = *socket {
            sock.send_to(message.as_bytes(), addr).await?;
            tracing::debug!("SIP message sent to {}: {} bytes", addr, message.len());
        }
        Ok(())
    }

    pub async fn send_message_to_device(
        &self,
        device_id: &str,
        method: SipMethod,
        body: Option<&str>,
        content_type: Option<&str>,
    ) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = format!("msg_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = format!("{} {}", 1, method.as_str());
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", device_id, device_addr.ip(), device_addr.port());
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        
        let mut headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
        ];
        
        if let Some(ct) = content_type {
            headers.push(("Content-Type", ct));
        }
        
        let uri = format!("sip:{}@{}:{}", device_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request_from_method(method, &uri, &headers, body);
        socket.send_to(message.as_bytes(), device_addr).await?;
        
        tracing::info!("Sent {} to device {} at {}", method.as_str(), device_id, device_addr);
        Ok(())
    }

    pub async fn send_catalog_query(&self, device_id: &str) -> Result<()> {
        let sn = chrono::Utc::now().timestamp();
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>Catalog</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            sn, device_id
        );
        
        self.send_message_to_device(device_id, SipMethod::Message, Some(&body), Some("Application/MANSCDP+xml")).await
    }

    pub async fn send_device_control(&self, device_id: &str, channel_id: &str, cmd_type: &str, body: &str) -> Result<()> {
        let sn = chrono::Utc::now().timestamp();
        let xml_body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Control>
<CmdType>{}</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<ChannelID>{}</ChannelID>
{}
</Control>"#,
            cmd_type, sn, device_id, channel_id, body
        );
        
        self.send_message_to_device(device_id, SipMethod::Message, Some(&xml_body), Some("Application/MANSCDP+xml")).await
    }

    pub async fn send_device_config_query(&self, device_id: &str, config_type: &str) -> Result<()> {
        let sn = chrono::Utc::now().timestamp();
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>ConfigDownload</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<ConfigType>{}</ConfigType>
</Query>"#,
            sn, device_id, config_type
        );
        
        self.send_message_to_device(device_id, SipMethod::Message, Some(&body), Some("Application/MANSCDP+xml")).await
    }

    /// Send an Alarm subscription to a device so it pushes alarm notifications
    pub async fn send_alarm_subscribe(&self, device_id: &str, expires: u32) -> Result<()> {
        self.send_subscribe(device_id, "Alarm", expires).await
    }

    pub async fn send_subscribe(&self, device_id: &str, event: &str, expires: u32) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;

        let device_addr = self
            .device_manager
            .get_address(device_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;

        let sn = chrono::Utc::now().timestamp();
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>{}</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
            event, sn, device_id
        );

        let call_id = format!("sub_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = "1 SUBSCRIBE".to_string();
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", self.config.ip, self.config.port, branch);
        let from = format!(
            "<sip:{}@{}:{}>;tag={}",
            self.config.device_id,
            self.config.ip,
            self.config.port,
            generate_tag()
        );
        let to = format!("<sip:{}@{}:{}>", device_id, device_addr.ip(), device_addr.port());
        let contact = format!(
            "<sip:{}@{}:{}>",
            self.config.device_id, self.config.ip, self.config.port
        );
        let expires_header = expires.to_string();

        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Expires", &expires_header),
            ("Max-Forwards", "70"),
            ("User-Agent", "WVP-GB28181-Rust"),
            ("Event", event),
            ("Accept", "Application/MANSCDP+xml"),
            ("Content-Type", "Application/MANSCDP+xml"),
        ];

        let uri = format!("sip:{}@{}:{}", device_id, device_addr.ip(), device_addr.port());
        let request = Parser::generate_request("SUBSCRIBE", &uri, &headers, Some(&body));
        socket.send_to(request.as_bytes(), device_addr).await?;

        if event.eq_ignore_ascii_case("Catalog") {
            let subscription = CatalogSubscription::new(
                &call_id,
                device_id,
                device_addr,
                &via,
                &from,
                &to,
                expires,
            );
            self.catalog_subscription_manager.subscribe(subscription).await;
        }

        tracing::info!("Sent SUBSCRIBE {} to device {} at {}", event, device_id, device_addr);
        Ok(())
    }

    pub async fn send_talk_invite(&self, device_id: &str, channel_id: &str) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = format!("talk_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = format!("INVITE {}", 1);
        let from_tag = generate_tag();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, from_tag);
        let to = format!("<sip:{}@{}:{}>", channel_id, device_addr.ip(), device_addr.port());
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        
        let sdp = build_audio_sdp(&self.config.ip, 0);
        
        let subject = format!("{}:{},{}:{}", self.config.device_id, channel_id, self.config.device_id, 0);
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
            ("User-Agent", "GBServer/1.0"),
            ("Subject", &subject),
            ("Content-Type", "application/sdp"),
        ];
        
        let uri = format!("sip:{}@{}:{}", channel_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request("INVITE", &uri, &headers, Some(&sdp));
        
        socket.send_to(message.as_bytes(), device_addr).await?;
        tracing::info!("Sent TALK INVITE to device {} channel {} at {}", device_id, channel_id, device_addr);
        
        Ok(())
    }
    
    pub async fn send_talk_bye(&self, device_id: &str, channel_id: &str) -> Result<()> {
        let session = self.talk_manager.get_by_device_channel(device_id, channel_id).await
            .ok_or_else(|| anyhow::anyhow!("No active talk session for {}/{}", device_id, channel_id))?;
        
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = &session.call_id;
        let branch = generate_branch();
        let cseq = "BYE 1".to_string();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", channel_id, device_addr.ip(), device_addr.port());
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", call_id),
            ("CSeq", &cseq),
            ("Max-Forwards", "70"),
        ];
        
        let uri = format!("sip:{}@{}:{}", channel_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request("BYE", &uri, &headers, None);
        
        socket.send_to(message.as_bytes(), device_addr).await?;
        tracing::info!("Sent TALK BYE to device {} channel {}", device_id, channel_id);
        
        self.talk_manager.update_status(call_id, TalkStatus::Terminating).await;
        
        if let Some(ref stream_id) = session.zlm_stream_id {
            if let Some(ref zlm) = self.zlm_client {
                let _ = zlm.close_rtp_server(stream_id).await;
            }
        }
        
        Ok(())
    }

    /// 根据 InviteSessionManager 中的 active session 发送 BYE（用于 Play/Playback/Download/Broadcast 停止）
    pub async fn send_session_bye(&self, device_id: &str, channel_id: &str) -> Result<String> {
        let session = self.invite_session_manager.get_by_device_channel(device_id, channel_id).await
            .ok_or_else(|| anyhow::anyhow!("No active invite session for {}/{}", device_id, channel_id))?;
        
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = session.call_id.clone();
        let branch = generate_branch();
        let cseq = "BYE 1".to_string();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", channel_id, device_addr.ip(), device_addr.port());
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Max-Forwards", "70"),
        ];
        
        let uri = format!("sip:{}@{}:{}", channel_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request("BYE", &uri, &headers, None);
        
        socket.send_to(message.as_bytes(), device_addr).await?;
        tracing::info!("Sent session BYE to device {} channel {} call_id={}", device_id, channel_id, call_id);
        
        self.invite_session_manager.update_status(&call_id, InviteSessionStatus::Terminating).await;
        
        if let Some(ref stream_id) = session.zlm_stream_id {
            if let Some(ref zlm) = self.zlm_client {
                let _ = zlm.close_rtp_server(stream_id).await;
            }
        }
        
        self.invite_session_manager.update_status(&call_id, InviteSessionStatus::Terminated).await;
        
        Ok(call_id)
    }

    /// 发送 GB28181 RecordInfo 查询请求（设备侧历史录像检索）
    pub async fn send_record_info_query(
        &self,
        device_id: &str,
        channel_id: &str,
        start_time: &str,
        end_time: &str,
        sn: i64,
    ) -> Result<String> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = format!("recinfo_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = "MESSAGE 1".to_string();
        
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>RecordInfo</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<StartTime>{}</StartTime>
<EndTime>{}</EndTime>
</Query>"#,
            sn, channel_id, start_time, end_time
        );
        
        let content_length = body.len().to_string();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", device_id, device_addr.ip(), device_addr.port());
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
            ("User-Agent", "GBServer/1.0"),
            ("Content-Type", "Application/MANSCDP+xml"),
            ("Content-Length", &content_length),
        ];
        
        let uri = format!("sip:{}@{}:{}", device_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request("MESSAGE", &uri, &headers, Some(&body));
        
        socket.send_to(message.as_bytes(), device_addr).await?;
        tracing::info!("Sent RecordInfo query to device {} channel {} [{}-{}]", device_id, channel_id, start_time, end_time);
        
        Ok(call_id)
    }

    /// 旧 fire-and-forget 接口（保留兼容）
    pub async fn send_play_invite(&self, device_id: &str, channel_id: &str) -> Result<()> {
        let _ = self.send_play_invite_and_wait(device_id, channel_id, 0, None).await;
        Ok(())
    }

    /// 完整的 play_start 信令：
    /// 1. 先由调用方分配好 ZLM RTP 端口（media_port）、ssrc
    /// 2. 构建规范 GB28181 SDP（s=Play）
    /// 3. 发送 SIP INVITE 到设备
    /// 4. 等待设备回 200 OK（超时 15s）
    /// 返回: Ok(call_id)  设备接受； Err(e) 超时或设备拒绝
    pub async fn send_play_invite_and_wait(
        &self,
        device_id: &str,
        channel_id: &str,
        media_port: u16,
        ssrc: Option<&str>,
    ) -> Result<String> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = format!("play_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = "INVITE 1".to_string();
        let from_tag = generate_tag();
        
        // 生成合规 SSRC（20 位 GB28181 SSRC = 0 + CivilCode(10) + 通道序号（0 实时）)
        let ssrc_str = ssrc.map(|s| s.to_string()).unwrap_or_else(|| {
            format!("0{:0>9}0", &device_id[..device_id.len().min(9)])
        });
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, from_tag);
        let to = format!("<sip:{}@{}:{}>", channel_id, device_addr.ip(), device_addr.port());
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        // Subject: serverGbId:ssrc,deviceGbId:0
        let subject = format!("{}:{},{}:0", self.config.device_id, ssrc_str, channel_id);
        
        // 规范 SDP – s=Play，使用真实 RTP 端口
        let sdp = build_invite_sdp(&self.config.ip, media_port, "Play", Some(&ssrc_str));
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
            ("User-Agent", "GBServer/1.0"),
            ("Subject", &subject),
            ("Content-Type", "application/sdp"),
        ];
        
        // 注册等待 channel，_必须_ 在发包之前注册，防止竞态
        let (tx, rx) = oneshot::channel::<String>();
        self.pending_invites.insert(call_id.clone(), tx);
        
        let uri = format!("sip:{}@{}:{}", channel_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request("INVITE", &uri, &headers, Some(&sdp));
        socket.send_to(message.as_bytes(), device_addr).await?;
        tracing::info!("Sent PLAY INVITE to device={} channel={} port={} ssrc={} call_id={}",
            device_id, channel_id, media_port, ssrc_str, call_id);
        drop(socket); // 释放读锁，避免死锁
        
        // 等待 200 OK（15 秒超时）
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(Ok(_)) => {
                tracing::info!("INVITE 200 OK received for call_id={}", call_id);
                Ok(call_id)
            }
            Ok(Err(_)) => {
                self.pending_invites.remove(&call_id);
                Err(anyhow::anyhow!("INVITE cancelled or device rejected"))
            }
            Err(_) => {
                self.pending_invites.remove(&call_id);
                Err(anyhow::anyhow!("INVITE timeout – device did not respond in 15s"))
            }
        }
    }
    
    pub async fn send_playback_invite(&self, device_id: &str, channel_id: &str, start_time: &str, end_time: &str) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let device_addr = self.device_manager.get_address(device_id).await
            .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
        
        let call_id = format!("playback_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = format!("INVITE {}", 1);
        let from_tag = generate_tag();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, from_tag);
        let to = format!("<sip:{}@{}:{}>", channel_id, device_addr.ip(), device_addr.port());
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        
        let sdp = build_playback_sdp(&self.config.ip, 0, start_time, end_time);
        let subject = format!("{}:{},{}:{}", self.config.device_id, channel_id, self.config.device_id, 1);
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
            ("User-Agent", "GBServer/1.0"),
            ("Subject", &subject),
            ("Content-Type", "application/sdp"),
        ];
        
        let uri = format!("sip:{}@{}:{}", channel_id, device_addr.ip(), device_addr.port());
        let message = Parser::generate_request("INVITE", &uri, &headers, Some(&sdp));
        
        socket.send_to(message.as_bytes(), device_addr).await?;
        tracing::info!("Sent PLAYBACK INVITE to device {} channel {} [{}-{}] at {}", device_id, channel_id, start_time, end_time, device_addr);
        
        Ok(())
    }

    pub async fn send_platform_invite(&self, platform_gb_id: &str, channel_id: &str, sdp_port: u16) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let platform = crate::db::platform::get_by_server_gb_id(&self.pool, platform_gb_id).await?
            .ok_or_else(|| anyhow::anyhow!("Platform {} not found", platform_gb_id))?;
        
        let server_ip = platform.server_ip.as_ref().ok_or_else(|| anyhow::anyhow!("Platform IP not set"))?;
        let server_port = platform.server_port.unwrap_or(5060) as u16;
        
        let call_id = format!("plat_{}_{}", platform_gb_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = format!("INVITE {}", 1);
        let from_tag = generate_tag();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, from_tag);
        let to = format!("<sip:{}@{}:{}>", channel_id, server_ip, server_port);
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        
        let sdp = build_playback_sdp(&self.config.ip, sdp_port, "0", "0");
        
        let subject = format!("{}:{},{}:{}", self.config.device_id, channel_id, platform_gb_id, 0);
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
            ("User-Agent", "GBServer/1.0"),
            ("Subject", &subject),
            ("Content-Type", "application/sdp"),
        ];
        
        let addr: std::net::SocketAddr = format!("{}:{}", server_ip, server_port).parse()?;
        let uri = format!("sip:{}@{}:{}", channel_id, server_ip, server_port);
        let message = Parser::generate_request("INVITE", &uri, &headers, Some(&sdp));
        
        socket.send_to(message.as_bytes(), addr).await?;
        tracing::info!("Sent platform INVITE for channel {} to platform {} at {}", channel_id, platform_gb_id, addr);
        
        Ok(())
    }
    
    pub async fn send_platform_message(&self, platform_gb_id: &str, cmd_type: &str, sn: i64, device_id: &str, content: Option<&str>) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let platform = crate::db::platform::get_by_server_gb_id(&self.pool, platform_gb_id).await?
            .ok_or_else(|| anyhow::anyhow!("Platform {} not found", platform_gb_id))?;
        
        let server_ip = platform.server_ip.as_ref().ok_or_else(|| anyhow::anyhow!("Platform IP not set"))?;
        let server_port = platform.server_port.unwrap_or(5060) as u16;
        
        let call_id = format!("plat_msg_{}_{}", platform_gb_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        let cseq = "MESSAGE 1".to_string();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            self.config.device_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", platform_gb_id, server_ip, server_port);
        let contact = format!("<sip:{}@{}:{}>", self.config.device_id, self.config.ip, self.config.port);
        
        let body = if let Some(c) = content {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<{}>
<CmdType>{}</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
{}</{}>"#,
                cmd_type, cmd_type, sn, device_id, c, cmd_type
            )
        } else {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<{}>
<CmdType>{}</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</{}>"#,
                cmd_type, cmd_type, sn, device_id, cmd_type
            )
        };
        
        let headers: Vec<(&str, &str)> = vec![
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Contact", &contact),
            ("Max-Forwards", "70"),
            ("Content-Type", "Application/MANSCDP+xml"),
        ];
        
        let addr: std::net::SocketAddr = format!("{}:{}", server_ip, server_port).parse()?;
        let uri = format!("sip:{}@{}:{}", platform_gb_id, server_ip, server_port);
        let message = Parser::generate_request("MESSAGE", &uri, &headers, Some(&body));
        
        socket.send_to(message.as_bytes(), addr).await?;
        tracing::info!("Sent {} to platform {} at {}", cmd_type, platform_gb_id, addr);
        
        Ok(())
    }
    
    pub async fn send_platform_catalog(&self, platform_gb_id: &str) -> Result<()> {
        let sn = chrono::Utc::now().timestamp();
        
        let channels = crate::db::device::list_all_channels(&self.pool).await?;
        
        let mut items = String::new();
        for channel in channels.iter().take(100) {
            let channel_id = channel.gb_device_id.as_deref().unwrap_or("");
            let name = channel.name.as_deref().unwrap_or("");
            let has_audio = channel.has_audio.unwrap_or(false);
            let has_audio_str = if has_audio { "true" } else { "false" };
            let status = if channel.status.as_deref().unwrap_or("off") == "on" { "ON" } else { "OFF" };
            let longitude = channel.longitude.unwrap_or(0.0);
            let latitude = channel.latitude.unwrap_or(0.0);
            
            items.push_str(&format!(
                r#"<Item>
<DeviceID>{}</DeviceID>
<Name>{}</Name>
<HasAudio>{}</HasAudio>
<Status>{}</Status>
<Longitude>{}</Longitude>
<Latitude>{}</Latitude>
</Item>"#,
                channel_id, name, has_audio_str, status, longitude, latitude
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
            sn, self.config.device_id, channels.len(), channels.len().min(100), items
        );
        
        self.send_platform_message(platform_gb_id, "Notify", sn, &self.config.device_id, Some(&body)).await
    }

    pub async fn register_to_platform(&self, platform_gb_id: &str) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let platform = crate::db::platform::get_by_server_gb_id(&self.pool, platform_gb_id).await?
            .ok_or_else(|| anyhow::anyhow!("Platform {} not found", platform_gb_id))?;
        
        let server_ip = platform.server_ip.as_ref().ok_or_else(|| anyhow::anyhow!("Platform IP not set"))?;
        let server_port = platform.server_port.unwrap_or(5060) as u16;
        
        let device_gb_id = platform.device_gb_id.as_ref().ok_or_else(|| anyhow::anyhow!("Device GB ID not set"))?;
        let username = platform.username.as_deref().unwrap_or("");
        let password = platform.password.as_deref().unwrap_or("");
        let expires = platform.expires.as_deref().unwrap_or("3600");
        
        let call_id = format!("reg_{}_{}", platform_gb_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            device_gb_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", device_gb_id, server_ip, server_port);
        let contact = format!("<sip:{}@{}:{}>", device_gb_id, self.config.ip, self.config.port);
        
        let auth = if !password.is_empty() {
            let nonce = generate_nonce();
            let realm = platform.server_gb_domain.as_deref().unwrap_or("GBServer");
            let response = Self::compute_digest_auth(username, password, realm, "REGISTER", "/", &nonce);
            format!(r#"Proxy-Authenticate: Digest realm="{}",nonce="{}",charset=utf-8,algorithm=MD5,qop="auth"
Authentication-Info: qop=auth,rspauth="{}",cnonce="{}",nc=00000001"#,
                realm, nonce, response, nonce)
        } else {
            String::new()
        };
        
        let message = format!(
            "REGISTER sip:{}:{} SIP/2.0\r\n\
             Via: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             Call-ID: {}\r\n\
             CSeq: 1 REGISTER\r\n\
             Max-Forwards: 70\r\n\
             Expires: {}\r\n\
             Contact: {}\r\n\
             User-Agent: GBServer/1.0\r\n\
             {}\
             Content-Length: 0\r\n\r\n",
            device_gb_id, server_port, via, from, to, call_id, expires, contact, auth
        );
        
        let addr: std::net::SocketAddr = format!("{}:{}", server_ip, server_port).parse()?;
        socket.send_to(message.as_bytes(), addr).await?;
        tracing::info!("Sent REGISTER to platform {} at {}", platform_gb_id, addr);
        
        Ok(())
    }
    
    pub async fn unregister_from_platform(&self, platform_gb_id: &str) -> Result<()> {
        let socket = self.socket.read().await;
        let socket = socket.as_ref().ok_or_else(|| anyhow::anyhow!("Socket not initialized"))?;
        
        let platform = crate::db::platform::get_by_server_gb_id(&self.pool, platform_gb_id).await?
            .ok_or_else(|| anyhow::anyhow!("Platform {} not found", platform_gb_id))?;
        
        let server_ip = platform.server_ip.as_ref().ok_or_else(|| anyhow::anyhow!("Platform IP not set"))?;
        let server_port = platform.server_port.unwrap_or(5060) as u16;
        
        let device_gb_id = platform.device_gb_id.as_ref().ok_or_else(|| anyhow::anyhow!("Device GB ID not set"))?;
        
        let call_id = format!("unreg_{}_{}", platform_gb_id, chrono::Utc::now().timestamp_millis());
        let branch = generate_branch();
        
        let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
            self.config.ip, self.config.port, branch);
        let from = format!("<sip:{}@{}:{}>;tag={}", 
            device_gb_id, self.config.ip, self.config.port, generate_tag());
        let to = format!("<sip:{}@{}:{}>", device_gb_id, server_ip, server_port);
        let contact = format!("<sip:{}@{}:{}>", device_gb_id, self.config.ip, self.config.port);
        
        let message = format!(
            "REGISTER sip:{}:{} SIP/2.0\r\n\
             Via: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             Call-ID: {}\r\n\
             CSeq: 1 REGISTER\r\n\
             Max-Forwards: 70\r\n\
             Expires: 0\r\n\
             Contact: {}\r\n\
             User-Agent: GBServer/1.0\r\n\
             Content-Length: 0\r\n\r\n",
            device_gb_id, server_port, via, from, to, call_id, contact
        );
        
        let addr: std::net::SocketAddr = format!("{}:{}", server_ip, server_port).parse()?;
        socket.send_to(message.as_bytes(), addr).await?;
        tracing::info!("Sent unREGISTER to platform {} at {}", platform_gb_id, addr);
        
        Ok(())
    }
    
    fn compute_digest_auth(username: &str, password: &str, realm: &str, method: &str, uri: &str, nonce: &str) -> String {
        use md5::{Md5, Digest};
        let ha1 = format!("{:x}", Md5::digest(format!("{}:{}:{}", username, realm, password)));
        let ha2 = format!("{:x}", Md5::digest(format!("{}:{}", method, uri)));
        format!("{:x}", Md5::digest(format!("{}:{}:{}", ha1, nonce, ha2)))
    }

    fn extract_tag_from_header(header: &str) -> Option<String> {
        for part in header.split(';') {
            let part = part.trim();
            if part.starts_with("tag=") {
                return Some(part.trim_start_matches("tag=").to_string());
            }
        }
        None
    }

    fn get_received_from_via(via: &str) -> Option<String> {
        for part in via.split(';') {
            if part.trim().starts_with("received=") {
                return Some(part.trim_start_matches("received=").to_string());
            }
        }
        None
    }
}

fn generate_nonce() -> String {
    let mut rng = rand::thread_rng();
    (0..32).map(|_| format!("{:02x}", rng.gen::<u8>())).collect()
}

fn generate_tag() -> String {
    let mut rng = rand::thread_rng();
    (0..8).map(|_| format!("{:02x}", rng.gen::<u8>())).collect()
}

fn generate_branch() -> String {
    let mut rng = rand::thread_rng();
    format!("z9hG4bK{:08x}", rng.gen::<u32>())
}

async fn dbg_upsert_device(
    pool: &Pool,
    device_id: &str,
    name: &str,
    manufacturer: Option<&str>,
    model: Option<&str>,
    firmware: Option<&str>,
    transport: Option<&str>,
    stream_mode: Option<&str>,
    ip: Option<&str>,
    port: Option<i32>,
    online: bool,
    media_server_id: String,
) -> Result<()> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    db_device::upsert_device(pool, device_id, Some(name), manufacturer, model, firmware, transport, stream_mode, ip, port, online, Some(media_server_id.as_str()), &now).await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

async fn send_subscribe_internal(
    device_id: &str,
    event: &str,
    expires: u32,
    config: &Arc<SipConfig>,
    device_manager: &Arc<DeviceManager>,
    catalog_subscription_manager: &Arc<CatalogSubscriptionManager>,
    socket: &Arc<UdpSocket>,
) -> Result<()> {
    let device_addr = device_manager.get_address(device_id).await
        .ok_or_else(|| anyhow::anyhow!("Device {} not registered", device_id))?;
    
    let sn = chrono::Utc::now().timestamp();
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Query>
<CmdType>{}</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
</Query>"#,
        event, sn, device_id
    );
    
    let call_id = format!("sub_{}_{}", device_id, chrono::Utc::now().timestamp_millis());
    let branch = generate_branch();
    let cseq = format!("{} SUBSCRIBE", 1);
    
    let via = format!("SIP/2.0/UDP {}:{};branch={};rport", 
        config.ip, config.port, branch);
    let from = format!("<sip:{}@{}:{}>;tag={}", 
        config.device_id, config.ip, config.port, generate_tag());
    let to = format!("<sip:{}@{}:{}>", device_id, device_addr.ip(), device_addr.port());
    let contact = format!("<sip:{}@{}:{}>", config.device_id, config.ip, config.port);
    
    let expires_header = expires.to_string();
    let content_length = body.len().to_string();
    
    let headers: Vec<(&str, &str)> = vec![
        ("Via", &via),
        ("From", &from),
        ("To", &to),
        ("Call-ID", &call_id),
        ("CSeq", &cseq),
        ("Contact", &contact),
        ("Expires", &expires_header),
        ("Max-Forwards", "70"),
        ("User-Agent", "WVP-GB28181-Rust"),
        ("Event", event),
        ("Accept", "Application/MANSCDP+xml"),
        ("Content-Type", "Application/MANSCDP+xml"),
        ("Content-Length", &content_length),
    ];
    
    let request = Parser::generate_request(
        "SUBSCRIBE",
        &format!("sip:{}@{}:{}", device_id, device_addr.ip(), device_addr.port()),
        &headers,
        Some(&body),
    );
    
    socket.send_to(request.as_bytes(), device_addr).await?;
    tracing::debug!("SUBSCRIBE sent to {} for event {}", device_id, event);
    
    let subscription = CatalogSubscription::new(
        &call_id,
        device_id,
        device_addr,
        &via,
        &from,
        &to,
        expires,
    );
    catalog_subscription_manager.subscribe(subscription).await;
    
    Ok(())
}
