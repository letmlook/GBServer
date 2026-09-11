use tokio::net::{TcpListener, UdpSocket};
use tokio::io::AsyncReadExt;
use std::error::Error;
use std::sync::Arc;

use super::Jt1078Server;

/// Start lightweight JT1078 TCP and UDP listeners. This spawns per-connection handlers
/// that manage simple authentication, heartbeat, and reassembly using session state.
/// 每条连接/每个对端的 JT808 字节流缓冲（真帧可能跨包到达）。
type Jt808Buffers = std::sync::Mutex<std::collections::HashMap<std::net::SocketAddr, Vec<u8>>>;

static JT808_BUFFERS: std::sync::OnceLock<Jt808Buffers> = std::sync::OnceLock::new();

fn jt808_buffers() -> &'static Jt808Buffers {
    JT808_BUFFERS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 处理一批真实 JT/T 808 字节；返回 `true` 表示按真帧处理过（调用方不必再走旧路径）。
///
/// 这里做三件此前**完全缺失**的事（`Jt1078Manager::process_jt_message` 定义了
/// 却没有任何运行时调用点）：
/// 1. 解析真实帧 → 拿到 `msg_id / 手机号 / 流水号 / 消息体`；
/// 2. 交给 `process_jt_message`（登记会话、匹配命令等待器、落库）；
/// 3. 按消息类型回通用应答 / 注册应答（终端据此认为注册成功）。
async fn handle_jt808_bytes_udp(
    manager: &std::sync::Arc<crate::jt1078::manager::Jt1078Manager>,
    socket: &tokio::net::UdpSocket,
    addr: std::net::SocketAddr,
    data: &[u8],
) -> bool {
    let (replies, handled) = feed_and_process(manager, addr, data).await;
    for (phone, reply) in replies {
        if let Err(e) = socket.send_to(&reply, addr).await {
            tracing::warn!("JT1078 应答发送失败 phone={} -> {}: {}", phone, addr, e);
        } else {
            tracing::debug!("JT1078 应答已发送 phone={} -> {}", phone, addr);
        }
    }
    handled
}

async fn handle_jt808_bytes(
    manager: &std::sync::Arc<crate::jt1078::manager::Jt1078Manager>,
    addr: std::net::SocketAddr,
    data: &[u8],
    tcp: Option<&mut tokio::net::TcpStream>,
) -> bool {
    let (replies, handled) = feed_and_process(manager, addr, data).await;
    if let Some(stream) = tcp {
        for (phone, reply) in replies {
            use tokio::io::AsyncWriteExt;
            if let Err(e) = stream.write_all(&reply).await {
                tracing::warn!("JT1078 TCP 应答写入失败 phone={}: {}", phone, e);
            }
        }
    }
    handled
}

/// 解析 + 处理 + 生成应答；返回 `(应答列表, 是否识别为真帧)`。
async fn feed_and_process(
    manager: &std::sync::Arc<crate::jt1078::manager::Jt1078Manager>,
    addr: std::net::SocketAddr,
    data: &[u8],
) -> (Vec<(String, Vec<u8>)>, bool) {
    use crate::jt1078::{command, frame, session::ParsedMessage};

    let mut replies = Vec::new();
    let mut handled = false;
    let to_process;

    {
        let mut buffers = jt808_buffers().lock().unwrap();
        let buf = buffers.entry(addr).or_default();
        buf.extend_from_slice(data);
        let (consumed, msgs, bad) = frame::split_jt808(buf);
        if bad > 0 {
            tracing::warn!("JT1078 丢弃 {} 个校验/格式非法的帧 from {}", bad, addr);
        }
        buf.drain(0..consumed);
        if buf.len() > 64 * 1024 {
            tracing::warn!("JT1078 缓冲超过 64KB，已清空 from {}", addr);
            buf.clear();
        }
        to_process = msgs;
    }

    if to_process.is_empty() {
        return (replies, handled);
    }
    handled = true;

    for msg in to_process {
        tracing::debug!(
            "JT1078 帧 msg_id=0x{:04x} phone={} serial={} body={}B sub={}/{} from {}",
            msg.msg_id,
            msg.phone,
            msg.serial,
            msg.body.len(),
            msg.packet_index,
            msg.total_packets,
            addr
        );

        let parsed = manager
            .process_jt_message(addr, &msg.phone, msg.msg_id, msg.serial, &msg.body)
            .await;

        match parsed {
            ParsedMessage::Register(_reg) => {
                manager.register_terminal(&msg.phone, addr).await;
                replies.push((
                    msg.phone.clone(),
                    command::build_register_response(&msg.phone, msg.serial, 0, "GBServer"),
                ));
                tracing::info!("JT1078 终端注册成功 phone={} from {}", msg.phone, addr);
            }
            ParsedMessage::Heartbeat => {
                manager.register_terminal(&msg.phone, addr).await;
                replies.push((msg.phone.clone(), build_common_ack(&msg.phone, msg.serial)));
            }
            ParsedMessage::CommonResponse { .. } => {
                // 终端通用应答（0x0001）**不需要**平台再回 0x8001 ——
                // 否则会形成"应答的应答"无限乒乓（实测 mock 与平台互相刷
                // 通用应答）。命令等待器已在 process_jt_message 里完成匹配。
                tracing::debug!(
                    "JT1078 终端通用应答 phone={} serial={}（不再回 0x8001）",
                    msg.phone,
                    msg.serial
                );
            }
            _ => {}
        }
    }
    (replies, handled)
}

/// 0x8001 通用应答：<应答流水号><应答ID><结果>
fn build_common_ack(phone: &str, serial: u16) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&serial.to_be_bytes());
    body.extend_from_slice(&0x8001u16.to_be_bytes());
    body.push(0); // 0 = 成功
    crate::jt1078::command::build_jt808_frame(0x8001, phone, 0, &body)
}

pub async fn start(server: &Jt1078Server, cfg: Option<crate::config::Jt1078Config>) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Phase 6.1: TCP/UDP ports now read from Jt1078Config (default 60000)
    let tcp_port = cfg.as_ref().and_then(|c| c.tcp_port).unwrap_or(60000);
    let udp_port = cfg.as_ref().and_then(|c| c.udp_port).unwrap_or(60000);
    let tcp_addr = format!("0.0.0.0:{}", tcp_port);
    let udp_addr = format!("0.0.0.0:{}", udp_port);

    // Create a manager used by both TCP and UDP listeners and start cleanup
    // Use injected cfg when available, otherwise fall back to environment or defaults
    let timeout = cfg.as_ref().and_then(|c| c.timeout_ms).map(|m| std::time::Duration::from_millis(m)).unwrap_or(std::time::Duration::from_secs(60));
    let retransmit_wait = cfg.as_ref().and_then(|c| c.retransmit_wait_ms).map(|m| std::time::Duration::from_millis(m)).unwrap_or(std::time::Duration::from_millis(200));
    let retransmit_hook = cfg.as_ref().and_then(|c| c.retransmit_hook_url.clone()).or_else(|| std::env::var("GBSERVER__JT1078__RETRANSMIT_HOOK").ok());
    let retransmit_send_to_device = std::env::var("GBSERVER__JT1078__RETRANSMIT_SEND_TO_DEVICE").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

    let manager = Arc::new(crate::jt1078::manager::Jt1078Manager::new(
        timeout,
        retransmit_wait,
        retransmit_hook,
        retransmit_send_to_device,
    ));
    // 注入数据库连接池：终端注册/心跳要写 gb_jt_terminal
    if let Some(pool) = server.pool() {
        manager.set_pool(pool);
    }
    // Store manager on server so handlers can access it
    server.set_manager(manager.clone()).await;
    let manager_for_cleanup = manager.clone();
    tokio::spawn(async move {
        manager_for_cleanup.cleanup_loop(std::time::Duration::from_secs(30)).await;
    });

    match TcpListener::bind(&tcp_addr).await {
        Ok(listener) => {
            // Spawn TCP accept loop — clone manager for TCP tasks when needed
            let manager_for_tcp = manager.clone();
            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut socket, addr)) => {
                            tracing::info!("JT1078 TCP connection accepted from {}", addr);
                            let manager = manager_for_tcp.clone();
                            tokio::spawn(async move {
                                let mut read_buf = [0u8; 4096];
                                loop {
                                    match socket.read(&mut read_buf).await {
                                        Ok(0) => {
                                            tracing::info!("JT1078 TCP connection closed by {}", addr);
                                            break;
                                        }
                                        Ok(n) => {
                                            // 真实 JT/T 808 帧优先；自造的 demo 帧走旧路径
                                            if !handle_jt808_bytes(
                                                &manager, addr, &read_buf[..n], Some(&mut socket),
                                            )
                                            .await
                                            {
                                                let frames = manager.feed_bytes(addr, &read_buf[..n]).await;
                                                for f in frames {
                                                    let kind = manager.process_payload_for(addr, &f).await;
                                                    match kind {
                                                        crate::jt1078::session::FrameKind::AuthFailure => {
                                                            tracing::warn!("JT1078 auth failed from {}", addr);
                                                            return;
                                                        }
                                                        crate::jt1078::session::FrameKind::AuthSuccess => {
                                                            tracing::info!("JT1078 auth success from {}", addr);
                                                        }
                                                        crate::jt1078::session::FrameKind::Heartbeat => {
                                                            tracing::debug!("JT1078 heartbeat from {}", addr);
                                                        }
                                                        crate::jt1078::session::FrameKind::Data(d) => {
                                                            tracing::debug!("JT1078 data {} bytes from {}", d.len(), addr);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!("JT1078 TCP read error from {}: {}", addr, e);
                                            break;
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            tracing::warn!("JT1078 TCP accept error: {}", e);
                            break;
                        }
                    }
                }
            });
            tracing::info!("JT1078 TCP listener spawned on {}", tcp_addr);
        }
        Err(e) => tracing::warn!("Failed to bind JT1078 TCP listener {}: {}", tcp_addr, e),
    }

    // Bind UDP socket
    match UdpSocket::bind(&udp_addr).await {
        Ok(socket) => {
            // For UDP, use the same manager to maintain sessions per peer
            let manager_udp = manager.clone();
            let socket_udp = std::sync::Arc::new(socket);
            // 下发命令共用同一个 socket（来源端口 = 监听端口）
            manager.set_send_socket(socket_udp.clone());
            tokio::spawn(async move {
                let mut buf = vec![0u8; 1500];
                loop {
                    match socket_udp.recv_from(&mut buf).await {
                        Ok((n, addr)) => {
                            // 真实 JT/T 808 帧优先；自造的 demo 帧走旧路径
                            if !handle_jt808_bytes_udp(&manager_udp, &socket_udp, addr, &buf[..n]).await {
                                let frames = manager_udp.feed_bytes(addr, &buf[..n]).await;
                                for f in frames {
                                    let kind = manager_udp.process_payload_for(addr, &f).await;
                                    match kind {
                                        crate::jt1078::session::FrameKind::AuthFailure => {
                                            tracing::warn!("JT1078 UDP auth failed from {}", addr);
                                        }
                                        crate::jt1078::session::FrameKind::AuthSuccess => {
                                            tracing::info!("JT1078 UDP auth success from {}", addr);
                                        }
                                        crate::jt1078::session::FrameKind::Heartbeat => {
                                            tracing::debug!("JT1078 UDP heartbeat from {}", addr);
                                        }
                                        crate::jt1078::session::FrameKind::Data(d) => {
                                            tracing::debug!("JT1078 UDP data {} bytes from {}", d.len(), addr);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("JT1078 UDP recv error: {}", e);
                            break;
                        }
                    }
                }
            });
            tracing::info!("JT1078 UDP listener spawned on {}", udp_addr);
        }
        Err(e) => tracing::warn!("Failed to bind JT1078 UDP socket {}: {}", udp_addr, e),
    }

    Ok(())
}
