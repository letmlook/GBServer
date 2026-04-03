use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use anyhow::Result;
use md5::{Md5, Digest};
use rand::Rng;
use chrono::Utc;

use crate::config::SipConfig;
use crate::db::{Pool, device as db_device};
use crate::sip::core::{
    TransactionManager, DialogManager, SipMessage, SipRequest, SipResponse,
    SipMethod, StatusCode, ViaHeader, NameAddr, CSeq, Contact,
    Authorization, Challenge, SubscriptionState
};
use crate::sip::gb28181::{DeviceManager, SessionManager, XmlParser, TransportMode};
use crate::sip::gb28181::invite::SessionStatus;
use crate::sip::gb28181::xml_parser::ChannelInfo;
use crate::sip::gb28181::invite_session::{
    InviteSessionManager, InviteSessionStatus, StreamType, SdpInfo,
    build_invite_sdp, build_playback_sdp
};
use crate::sip::gb28181::talk::{TalkManager, TalkStatus, build_talk_sdp as build_audio_sdp};
use crate::sip::gb28181::catalog::{CatalogSubscriptionManager, build_catalog_notify_body};
use crate::sip::core::parser::Parser;
use crate::zlm::ZlmClient;

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
    pool: Pool,
    zlm_client: Option<Arc<ZlmClient>>,
}

impl SipServer {
    pub fn new(config: SipConfig, pool: Pool) -> Self {
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
            pool,
            zlm_client: None,
        }
    }

    pub fn set_zlm_client(&mut self, client: Option<Arc<ZlmClient>>) {
        self.zlm_client = client;
    }

    pub async fn start(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.config.ip, self.config.port);
        let socket = UdpSocket::bind(&addr).await?;
        tracing::info!("SIP Server listening on {}", addr);
        *self.socket.write().await = Some(socket);
        
        let device_manager = self.device_manager.clone();
        let invite_manager = self.invite_session_manager.clone();
        let talk_manager = self.talk_manager.clone();
        let zlm_client = self.zlm_client.clone();
        let config = self.config.clone();
        
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
                    for mut session in sessions {
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

        loop {
            let mut buf = vec![0u8; 65535];
            let socket_clone = socket.clone();
            match socket_clone.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = buf[..len].to_vec();
                    let config = config.clone();
                    let device_manager = device_manager.clone();
                    let session_manager = session_manager.clone();
                    let invite_session_manager = invite_session_manager.clone();
                    let talk_manager = talk_manager.clone();
                    let catalog_subscription_manager = catalog_subscription_manager.clone();
                    let zlm_client = zlm_client.clone();
                    let pool = pool.clone();
                    let socket_for_response = socket.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_packet(&data, addr, &config, &device_manager, &session_manager, &invite_session_manager, &talk_manager, &catalog_subscription_manager, &zlm_client, &pool, &socket_for_response).await {
                            tracing::error!("SIP handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("UDP recv error: {}", e);
                }
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
    ) -> Result<()> {
        let msg = Parser::parse(data)?;
        match msg {
            SipMessage::Request(req) => {
                Self::handle_request(req, addr, config, device_manager, session_manager, invite_session_manager, talk_manager, catalog_subscription_manager, zlm_client, pool, socket).await
            }
            SipMessage::Response(resp) => {
                Self::handle_response(resp, session_manager).await
            }
        }
    }

    async fn handle_request(
        mut req: SipRequest,
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
    ) -> Result<()> {
        let method = req.method;
        match method {
            SipMethod::Register => {
                Self::handle_register(req, addr, config, device_manager, pool, socket).await
            }
            SipMethod::Message => {
                Self::handle_message(req, addr, config, device_manager, pool, socket).await
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
            tracing::info!("Device unregistered: {}", device_id);
        } else {
            let ip_str = addr.ip().to_string();
            db_device::upsert_device(pool, &device_id, None, None, None, None, None, None, 
                Some(&ip_str), Some(addr.port() as i32), true, Some("zlmediakit-1"), &now).await?;
            device_manager.register(&device_id, addr).await;
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
        config: &Arc<SipConfig>,
        device_manager: &Arc<DeviceManager>,
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let content_type = req.header("content-type").cloned().unwrap_or_default();

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
                    Self::handle_alarm(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket).await?;
                    return Ok(());
                }
                Some("RecordInfo") => {
                    Self::handle_record_info(body, &device_id, &sn, pool, addr, &from, &to, &via, &call_id, &cseq, socket).await?;
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
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let via = req.header("via").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();
        let content_type = req.header("content-type").cloned().unwrap_or_default();

        let from_device = Self::extract_device_id(&from).unwrap_or_default();
        let to_device = Self::extract_device_id(&to).unwrap_or_default();

        tracing::info!("INVITE from {} to {} - CallID: {}", from_device, to_device, call_id);

        let sdp_request_body = req.body.clone();
        let (stream_type, ssrc, sdp_info) = if let Some(body) = &req.body {
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
        session_manager: &Arc<SessionManager>,
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
        config: &Arc<SipConfig>,
        pool: &Pool,
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
        config: &Arc<SipConfig>,
        session_manager: &Arc<SessionManager>,
        invite_session_manager: &Arc<InviteSessionManager>,
        talk_manager: &Arc<TalkManager>,
        zlm_client: &Option<Arc<ZlmClient>>,
        pool: &Pool,
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
        config: &Arc<SipConfig>,
        session_manager: &Arc<SessionManager>,
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
        session_manager: &Arc<SessionManager>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.header("via").cloned().unwrap_or_default();
        let from = req.header("from").cloned().unwrap_or_default();
        let to = req.header("to").cloned().unwrap_or_default();
        let call_id = req.header("call-id").cloned().unwrap_or_default();
        let cseq = req.header("cseq").cloned().unwrap_or_default();

        tracing::info!("UPDATE received - CallID: {}", call_id);

        let response_body = if let Some(body) = &req.body {
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
        device_manager: &Arc<DeviceManager>,
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
        config: &Arc<SipConfig>,
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
    ) -> Result<()> {
        let call_id = resp.headers.get("call-id").cloned().unwrap_or_default();
        let cseq = resp.headers.get("cseq").cloned().unwrap_or_default();
        
        tracing::debug!("SIP Response: {} {} - CallID: {}", resp.status_code(), resp.reason, call_id);
        
        if resp.status_code() == 200 {
            if cseq.contains("INVITE") {
                session_manager.update_status(&call_id, SessionStatus::Ringing).await;
            } else if cseq.contains("BYE") {
                session_manager.remove(&call_id).await;
            }
        } else if resp.status_code() == 487 {
            session_manager.remove(&call_id).await;
        }
        
        Ok(())
    }

    async fn handle_catalog(
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
    ) -> Result<()> {
        let parsed = XmlParser::parse(body);
        let alarm_type = parsed.get("AlarmType").cloned().unwrap_or_else(|| "Unknown".to_string());
        
        tracing::info!("Alarm from {}: {}", device_id, alarm_type);
        
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
        pool: &Pool,
        addr: SocketAddr,
        from: &str,
        to: &str,
        via: &str,
        call_id: &str,
        cseq: &str,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        tracing::debug!("RecordInfo from {}", device_id);
        
        let response_body = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Response><CmdType>RecordInfo</CmdType><SN>{}</SN><DeviceID>{}</DeviceID><SumNum>0</SumNum><RecordList Num="0"></RecordList></Response>"#,
            sn, device_id);
        
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
        let a_port = audio_port.unwrap_or(0);
        
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

    pub async fn send_subscribe(&self, device_id: &str, event: &str, expires: u32) -> Result<()> {
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
        
        self.send_message_to_device(device_id, SipMethod::Subscribe, Some(&body), Some("Application/MANSCDP+xml")).await
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
