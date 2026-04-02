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
use crate::sip::core::{TransactionManager, DialogManager, SipMessage, SipRequest, SipResponse};
use crate::sip::gb28181::{DeviceManager, SessionManager, XmlParser, TransportMode};
use crate::sip::gb28181::invite::SessionStatus;
use crate::sip::gb28181::xml_parser::ChannelInfo;
use crate::sip::core::parser::Parser;

pub struct SipServer {
    config: Arc<SipConfig>,
    device_manager: Arc<DeviceManager>,
    session_manager: Arc<SessionManager>,
    transaction_manager: Arc<TransactionManager>,
    dialog_manager: Arc<DialogManager>,
    socket: Arc<RwLock<Option<UdpSocket>>>,
    pool: Pool,
}

impl SipServer {
    pub fn new(config: SipConfig, pool: Pool) -> Self {
        Self {
            config: Arc::new(config),
            device_manager: Arc::new(DeviceManager::new()),
            session_manager: Arc::new(SessionManager::new()),
            transaction_manager: Arc::new(TransactionManager::new()),
            dialog_manager: Arc::new(DialogManager::new()),
            socket: Arc::new(RwLock::new(None)),
            pool,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.config.ip, self.config.port);
        let socket = UdpSocket::bind(&addr).await?;
        tracing::info!("SIP Server listening on {}", addr);
        *self.socket.write().await = Some(socket);
        
        let device_manager = self.device_manager.clone();
        let pool = self.pool.clone();
        tokio::spawn(async move {
            loop {
                device_manager.cleanup_expired(60).await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });

        dbg_upsert_device(&self.pool, &self.config.device_id, "WVP Server", Some("Rust"), Some("GBServer"), None, None, None, None, None, true, "zlmediakit-1".to_string()).await;
        
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let socket = self.socket.write().await.take().expect("Server not started");
        let socket = Arc::new(socket);
        let config = self.config.clone();
        let device_manager = self.device_manager.clone();
        let session_manager = self.session_manager.clone();
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
                    let pool = pool.clone();
                    let socket_for_response = socket.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_packet(&data, addr, &config, &device_manager, &session_manager, &pool, &socket_for_response).await {
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
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let msg = Parser::parse(data)?;
        match msg {
            SipMessage::Request(req) => {
                Self::handle_request(req, addr, config, device_manager, session_manager, pool, socket).await
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
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let method = req.method.clone();
        match method.as_str() {
            "REGISTER" => {
                Self::handle_register(req, addr, config, device_manager, pool, socket).await
            }
            "MESSAGE" => {
                Self::handle_message(req, addr, config, device_manager, pool, socket).await
            }
            "INVITE" => {
                Self::handle_invite(req, addr, config, session_manager, pool, socket).await
            }
            "ACK" => {
                Self::handle_ack(req, session_manager).await
            }
            "BYE" => {
                Self::handle_bye(req, session_manager).await
            }
            "OPTIONS" => {
                Self::handle_options(req, addr, config, socket).await
            }
            "INFO" => {
                Self::handle_info(req, addr, config, pool, socket).await
            }
            "CANCEL" => {
                Ok(())
            }
            _ => {
                tracing::warn!("Unhandled SIP method: {}", method);
                Ok(())
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
        let from = req.from().cloned().unwrap_or_default();
        let to = req.to().cloned().unwrap_or_default();
        let via = req.via().cloned().unwrap_or_default();
        let call_id = req.call_id().cloned().unwrap_or_default();
        let cseq = req.cseq().cloned().unwrap_or_default();

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
            let response = Parser::generate_response(
                401,
                "Unauthorized",
                &[
                    ("Via", &via),
                    ("From", &from),
                    ("To", &to),
                    ("Call-ID", &call_id),
                    ("CSeq", &cseq),
                    ("WWW-Authenticate", &format!("Digest realm=\"{}\", nonce=\"{}\", algorithm=MD5, qop=\"auth\"", config.realm, nonce)),
                ],
                None,
            );
            Self::send_response(socket, addr, &response).await?;
            tracing::info!("REGISTER from {} - Challenge sent (nonce: {})", device_id, nonce);
            return Ok(());
        }

        let auth_str = auth.unwrap();
        if !validate_digest(&auth_str, &device_id, &config.password, &config.realm, &format!("sip:{}@{}:{}", device_id, config.ip, config.port)) {
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

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &format!("{};tag={}", to.trim_end_matches('>').trim(), generate_tag())),
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
        let from = req.from().cloned().unwrap_or_default();
        let to = req.to().cloned().unwrap_or_default();
        let via = req.via().cloned().unwrap_or_default();
        let call_id = req.call_id().cloned().unwrap_or_default();
        let cseq = req.cseq().cloned().unwrap_or_default();
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
        pool: &Pool,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let from = req.from().cloned().unwrap_or_default();
        let to = req.to().cloned().unwrap_or_default();
        let via = req.via().cloned().unwrap_or_default();
        let call_id = req.call_id().cloned().unwrap_or_default();
        let cseq = req.cseq().cloned().unwrap_or_default();
        let content_type = req.header("content-type").cloned().unwrap_or_default();

        let from_device = Self::extract_device_id(&from).unwrap_or_default();
        let to_device = Self::extract_device_id(&to).unwrap_or_default();

        tracing::info!("INVITE from {} to {} - CallID: {}", from_device, to_device, call_id);

        let (stream_type, ssrc) = if let Some(body) = &req.body {
            let sdp_info = Self::parse_sdp(body);
            let stream_type = sdp_info.get("s").cloned().unwrap_or_else(|| "Play".to_string());
            let ssrc = sdp_info.get("y").cloned();
            (stream_type, ssrc)
        } else {
            ("Play".to_string(), None)
        };

        let channel_id = Self::extract_channel_id(&req.uri);
        if channel_id.is_empty() {
            tracing::warn!("Cannot extract channel ID from URI: {}", req.uri);
        }

        session_manager.create(&call_id, &from_device, &channel_id, &stream_type).await;

        let tag = generate_tag();
        let response_body = Self::generate_sdp_response(&config.device_id, "Play", None, None);
        let branch = Self::get_branch(&via).unwrap_or_default();
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
        
        tracing::info!("INVITE 200 OK sent - stream: {}", stream_type);
        Ok(())
    }

    async fn handle_ack(
        req: SipRequest,
        session_manager: &Arc<SessionManager>,
    ) -> Result<()> {
        let call_id = req.call_id().cloned().unwrap_or_default();
        session_manager.update_status(&call_id, SessionStatus::Active).await;
        tracing::info!("ACK received - CallID: {}", call_id);
        Ok(())
    }

    async fn handle_bye(
        req: SipRequest,
        session_manager: &Arc<SessionManager>,
    ) -> Result<()> {
        let call_id = req.call_id().cloned().unwrap_or_default();
        let from = req.from().cloned().unwrap_or_default();
        let to = req.to().cloned().unwrap_or_default();
        let via = req.via().cloned().unwrap_or_default();

        session_manager.remove(&call_id).await;
        
        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
        ], None);
        
        tracing::info!("BYE received - CallID: {} - Session terminated", call_id);
        Ok(())
    }

    async fn handle_options(
        req: SipRequest,
        addr: SocketAddr,
        config: &Arc<SipConfig>,
        socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        let via = req.via().cloned().unwrap_or_default();
        let from = req.from().cloned().unwrap_or_default();
        let to = req.to().cloned().unwrap_or_default();
        let call_id = req.call_id().cloned().unwrap_or_default();
        let cseq = req.cseq().cloned().unwrap_or_default();

        let response = Parser::generate_response(200, "OK", &[
            ("Via", &via),
            ("From", &from),
            ("To", &to),
            ("Call-ID", &call_id),
            ("CSeq", &cseq),
            ("Allow", "INVITE, ACK, OPTIONS, INFO, BYE, CANCEL, NOTIFY, MESSAGE, REFER, PRACK, UPDATE"),
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
        let via = req.via().cloned().unwrap_or_default();
        let from = req.from().cloned().unwrap_or_default();
        let to = req.to().cloned().unwrap_or_default();
        let call_id = req.call_id().cloned().unwrap_or_default();
        let cseq = req.cseq().cloned().unwrap_or_default();

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

    async fn handle_response(
        resp: SipResponse,
        session_manager: &Arc<SessionManager>,
    ) -> Result<()> {
        let call_id = resp.headers.get("call-id").cloned().unwrap_or_default();
        let cseq = resp.headers.get("cseq").cloned().unwrap_or_default();
        
        tracing::debug!("SIP Response: {} {} - CallID: {}", resp.status_code, resp.reason, call_id);
        
        if resp.status_code == 200 {
            if cseq.contains("INVITE") {
                session_manager.update_status(&call_id, SessionStatus::Ringing).await;
            } else if cseq.contains("BYE") {
                session_manager.remove(&call_id).await;
            }
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
        
        // Query channels from database
        let channels = match db_device::list_channels_for_device(pool, device_id).await {
            Ok(chs) => chs,
            Err(e) => {
                tracing::error!("Failed to query channels for {}: {}", device_id, e);
                Vec::new()
            }
        };
        
        // Build XML response with channel list
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

    pub fn device_manager(&self) -> Arc<DeviceManager> {
        self.device_manager.clone()
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
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
