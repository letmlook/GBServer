use std::error::Error;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, UdpSocket};

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
/// 处理一批真实 JT/T 808 字节，并把应答经**该终端自己的下行通道**发回。
///
/// 下行通道的选择统一交给 `Jt1078Manager::send_raw`：TCP 终端走它那条 TCP
/// 连接，UDP 终端走监听 socket（来源端口 = 服务端口）。
/// 此前 TCP 路径把应答直接写在读循环持有的 socket 上（拆读写后写不了），
/// 结果 TCP 终端收不到 0x8100 注册应答、只能无限重注册。
async fn handle_jt808_bytes(
    manager: &std::sync::Arc<crate::jt1078::manager::Jt1078Manager>,
    addr: std::net::SocketAddr,
    data: &[u8],
) -> bool {
    let (replies, handled) = feed_and_process(manager, addr, data).await;
    for (phone, reply) in replies {
        match manager.send_raw(&phone, &reply).await {
            Ok(()) => tracing::debug!("JT1078 应答已发送 phone={} -> {}", phone, addr),
            Err(e) => tracing::warn!("JT1078 应答发送失败 phone={} -> {}: {}", phone, addr, e),
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
                // 注册应答（0x8100）里的鉴权码：优先用库里配置的，没配置就生成
                // 一个并入库 —— 平台下发的与库里存的必须是同一个值，终端随后
                // 用 0x0102 把它回送，平台才能校验。此前这里写死字面量
                // "GBServer"，`gb_jt_terminal.auth_code` 因此只写不读。
                let auth_code = match stored_auth_code(manager, &msg.phone).await {
                    Some(code) => code,
                    None => issue_auth_code(manager, &msg.phone).await,
                };
                replies.push((
                    msg.phone.clone(),
                    command::build_register_response(&msg.phone, msg.serial, 0, &auth_code),
                ));
                tracing::info!(
                    "JT1078 终端注册成功 phone={} from {} (鉴权码已下发)",
                    msg.phone,
                    addr
                );
            }
            ParsedMessage::Authenticate(code) => {
                // 终端鉴权（JT/T 808 0x0102 → 0x8102）。
                // 这里**只读**库里已配置的鉴权码，不签发新的：
                // 签发是有副作用的（生成 + 写库），放在校验路径上会出问题 ——
                // 库里没有配置时会现生成一个随机码，再拿它跟终端回送的码比对，
                // 必然对不上，终端从此再也鉴权不过（而库里还多了一条随机码）。
                let expected = stored_auth_code(manager, &msg.phone).await;
                let matched =
                    matches!(&expected, Some(expect) if !expect.is_empty() && code == *expect);
                if matched {
                    manager.mark_authenticated(&msg.phone).await;
                    // 强制鉴权时，终端要等这一刻才算上线（注册时还没鉴权）。
                    manager.persist_terminal_online(&msg.phone, None).await;
                    tracing::info!("JT1078 终端鉴权通过 phone={}", msg.phone);
                } else {
                    tracing::warn!(
                        "JT1078 终端鉴权失败 phone={}（收到的鉴权码与平台下发的不一致）",
                        msg.phone
                    );
                }
                replies.push((
                    msg.phone.clone(),
                    command::build_auth_response(&msg.phone, msg.serial, u8::from(!matched)),
                ));
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

/// 读终端**已配置**的鉴权码（纯读，无副作用）。
///
/// 没有配置过（或读失败）返回 `None`。
async fn stored_auth_code(
    manager: &std::sync::Arc<crate::jt1078::manager::Jt1078Manager>,
    phone: &str,
) -> Option<String> {
    let pool = manager.pool()?;
    match crate::db::jt1078::get_auth_code_by_phone(&pool, phone).await {
        Ok(Some(code)) => {
            let trimmed = code.trim().trim_end_matches('\0').trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("JT1078 读取终端鉴权码失败 phone={phone}: {e}");
            None
        }
    }
}

/// 签发并持久化一个鉴权码（**有副作用**：生成 + 写库）。
///
/// 只在**注册应答**时调用 —— 这样 0x8100 里下发的与库里存的永远是同一个值，
/// 终端随后用 0x0102 回送时才能校验通过。没有数据库时退化为默认串
/// （此时平台只能"宽松"工作，无法校验终端回送的鉴权码）。
async fn issue_auth_code(
    manager: &std::sync::Arc<crate::jt1078::manager::Jt1078Manager>,
    phone: &str,
) -> String {
    let Some(pool) = manager.pool() else {
        return DEFAULT_AUTH_CODE.to_string();
    };
    match crate::db::jt1078::get_auth_code_by_phone(&pool, phone).await {
        Ok(Some(code)) => {
            let trimmed = code.trim().trim_end_matches('\0').trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("JT1078 读取终端鉴权码失败 phone={phone}: {e}"),
    }
    // 没有配置：生成一个并写回库里。
    let generated = format!("{:016x}", rand::random::<u64>());
    match crate::db::jt1078::update_auth_code(&pool, phone, &generated).await {
        Ok(n) if n > 0 => {}
        Ok(_) => tracing::warn!(
            "JT1078 终端尚未入库，鉴权码未能持久化 phone={phone}（本次仍可用，但不参与校验）"
        ),
        Err(e) => tracing::warn!("JT1078 写入终端鉴权码失败 phone={phone}: {e}"),
    }
    generated
}

/// 没有数据库时的兜底鉴权码（宽松模式下不参与校验）。
const DEFAULT_AUTH_CODE: &str = "GBServer";

/// 0x8001 通用应答：<应答流水号><应答ID><结果>
fn build_common_ack(phone: &str, serial: u16) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&serial.to_be_bytes());
    body.extend_from_slice(&0x8001u16.to_be_bytes());
    body.push(0); // 0 = 成功
    crate::jt1078::command::build_jt808_frame(0x8001, phone, 0, &body)
}

pub async fn start(
    server: &Jt1078Server,
    cfg: Option<crate::config::Jt1078Config>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Phase 6.1: TCP/UDP ports now read from Jt1078Config (default 60000)
    let tcp_port = cfg.as_ref().and_then(|c| c.tcp_port).unwrap_or(60000);
    let udp_port = cfg.as_ref().and_then(|c| c.udp_port).unwrap_or(60000);
    let tcp_addr = format!("0.0.0.0:{}", tcp_port);
    let udp_addr = format!("0.0.0.0:{}", udp_port);

    // Create a manager used by both TCP and UDP listeners and start cleanup
    // Use injected cfg when available, otherwise fall back to environment or defaults
    let timeout = cfg
        .as_ref()
        .and_then(|c| c.timeout_ms)
        .map(|m| std::time::Duration::from_millis(m))
        .unwrap_or(std::time::Duration::from_secs(60));
    let retransmit_wait = cfg
        .as_ref()
        .and_then(|c| c.retransmit_wait_ms)
        .map(|m| std::time::Duration::from_millis(m))
        .unwrap_or(std::time::Duration::from_millis(200));
    let retransmit_hook = cfg
        .as_ref()
        .and_then(|c| c.retransmit_hook_url.clone())
        .or_else(|| std::env::var("GBSERVER__JT1078__RETRANSMIT_HOOK").ok());
    let retransmit_send_to_device = std::env::var("GBSERVER__JT1078__RETRANSMIT_SEND_TO_DEVICE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut manager = crate::jt1078::manager::Jt1078Manager::new(
        timeout,
        retransmit_wait,
        retransmit_hook,
        retransmit_send_to_device,
    );
    // 是否强制终端鉴权（JT/T 808 0x0102）。默认宽松 —— 大量真实终端与本仓库的
    // 模拟器都不会回 0x0102，强制会让它们直接不可用。
    let auth_required = cfg
        .as_ref()
        .and_then(|c| c.auth_required)
        .or_else(|| {
            std::env::var("GBSERVER__JT1078__AUTH_REQUIRED")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        })
        .unwrap_or(false);
    manager.set_auth_required(auth_required);
    let manager = Arc::new(manager);
    // 注入数据库连接池：终端注册/心跳要写 gb_jt_terminal
    if let Some(pool) = server.pool() {
        manager.set_pool(pool);
    }
    // Store manager on server so handlers can access it
    server.set_manager(manager.clone()).await;
    let manager_for_cleanup = manager.clone();
    tokio::spawn(async move {
        manager_for_cleanup
            .cleanup_loop(std::time::Duration::from_secs(30))
            .await;
    });

    match TcpListener::bind(&tcp_addr).await {
        Ok(listener) => {
            // Spawn TCP accept loop — clone manager for TCP tasks when needed
            let manager_for_tcp = manager.clone();
            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((socket, addr)) => {
                            tracing::info!("JT1078 TCP connection accepted from {}", addr);
                            let manager = manager_for_tcp.clone();
                            // 读写拆开：写半交给独立任务消费下行队列，
                            // 读循环不再独占 socket —— 否则平台无法经同一条
                            // TCP 连接给终端下发命令（TCP 终端一直收不到命令）。
                            let (mut read_half, mut write_half) = socket.into_split();
                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                            manager.register_tcp_sender(addr, tx).await;
                            let manager_writer = manager.clone();
                            tokio::spawn(async move {
                                use tokio::io::AsyncWriteExt;
                                while let Some(bytes) = rx.recv().await {
                                    if let Err(e) = write_half.write_all(&bytes).await {
                                        tracing::warn!("JT1078 TCP 下行写入失败 {}: {}", addr, e);
                                        break;
                                    }
                                }
                                let _ = write_half.shutdown().await;
                                manager_writer.remove_tcp_sender(&addr).await;
                                tracing::debug!("JT1078 TCP 下行通道关闭 {}", addr);
                            });
                            tokio::spawn(async move {
                                let mut read_buf = [0u8; 4096];
                                loop {
                                    match read_half.read(&mut read_buf).await {
                                        Ok(0) => {
                                            tracing::info!(
                                                "JT1078 TCP connection closed by {}",
                                                addr
                                            );
                                            break;
                                        }
                                        Ok(n) => {
                                            // 真实 JT/T 808 帧优先；自造的 demo 帧走旧路径
                                            if !handle_jt808_bytes(&manager, addr, &read_buf[..n])
                                                .await
                                            {
                                                let frames =
                                                    manager.feed_bytes(addr, &read_buf[..n]).await;
                                                for f in frames {
                                                    let kind =
                                                        manager.process_payload_for(addr, &f).await;
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
                                            tracing::warn!(
                                                "JT1078 TCP read error from {}: {}",
                                                addr,
                                                e
                                            );
                                            break;
                                        }
                                    }
                                }
                                manager.remove_tcp_sender(&addr).await;
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
                            if !handle_jt808_bytes(&manager_udp, addr, &buf[..n]).await {
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
                                            tracing::debug!(
                                                "JT1078 UDP data {} bytes from {}",
                                                d.len(),
                                                addr
                                            );
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

#[cfg(all(test, feature = "sqlite"))]
mod terminal_auth_tests {
    use super::*;

    const PHONE: &str = "13812345678";
    const CONFIGURED_AUTH_CODE: &str = "VENDOR-SECRET-CODE";

    fn manager_with_pool(pool: crate::db::Pool) -> Arc<crate::jt1078::manager::Jt1078Manager> {
        let manager = crate::jt1078::manager::Jt1078Manager::new(
            std::time::Duration::from_secs(30),
            std::time::Duration::from_millis(200),
            None,
            false,
        );
        manager.set_pool(pool);
        Arc::new(manager)
    }

    /// 构造一个 0x0100 终端注册帧（正文满足 REGISTER_MIN_LEN）。
    fn register_frame(seq: u16) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes()); // 省
        body.extend_from_slice(&2u16.to_be_bytes()); // 市
        body.extend_from_slice(b"TESTM"); // 厂商 5
        body.extend_from_slice(b"MODEL-0000000000000000"); // 型号 20
        body.extend_from_slice(b"TERM001"); // 终端编号 7
        body.push(0); // 车牌颜色
        body.extend_from_slice(b"\x00\x00"); // 车牌（GBK，空）
        crate::jt1078::command::build_jt808_frame(0x0100, PHONE, seq, &body)
    }

    async fn seed_terminal(pool: &crate::db::Pool, auth_code: Option<&str>) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let fields = crate::db::jt1078::JtTerminalWrite {
            terminal_id: Some("TERM001"),
            plate_no: Some(""),
            plate_color: Some(0),
            maker_id: Some("TESTM"),
            model: Some("MODEL-0000000000000000"),
            media_server_id: None,
            province_id: None,
            city_id: None,
        };
        crate::db::jt1078::insert_terminal(pool, PHONE, &fields, &now)
            .await
            .unwrap();
        if let Some(code) = auth_code {
            crate::db::jt1078::update_auth_code(pool, PHONE, code)
                .await
                .unwrap();
        }
    }

    /// 从 0x8100 注册应答里取出鉴权码。
    fn auth_code_from_register_reply(reply: &[u8]) -> String {
        let (_, msg) = crate::jt1078::frame::parse_jt808_frame(reply).expect("reply parses");
        assert_eq!(msg.msg_id, 0x8100, "注册应答的 msg_id 必须是 0x8100");
        // 正文：应答流水号(2) + 结果(1) + 鉴权码
        assert_eq!(msg.body[2], 0, "注册结果应为 0（成功）");
        String::from_utf8_lossy(&msg.body[3..])
            .trim_end_matches('\0')
            .trim()
            .to_string()
    }

    /// C3：注册应答必须下发**库里配置的**鉴权码，而不是写死的字面量。
    #[tokio::test]
    async fn register_reply_carries_configured_auth_code() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed_terminal(&pool, Some(CONFIGURED_AUTH_CODE)).await;
        let manager = manager_with_pool(pool.clone());

        let addr: std::net::SocketAddr = "127.0.0.1:60001".parse().unwrap();
        let (replies, handled) = feed_and_process(&manager, addr, &register_frame(1)).await;
        assert!(handled);
        assert_eq!(replies.len(), 1, "注册必须回一条 0x8100");
        let code = auth_code_from_register_reply(&replies[0].1);
        assert_eq!(code, CONFIGURED_AUTH_CODE);
    }

    /// 终端没配置鉴权码时，平台生成一个**并存进库** ——
    /// 下发的与存下的必须是同一个值，否则终端回送的 0x0102 永远校验不过。
    #[tokio::test]
    async fn register_generates_and_persists_auth_code_when_unconfigured() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed_terminal(&pool, None).await;
        let manager = manager_with_pool(pool.clone());

        let addr: std::net::SocketAddr = "127.0.0.1:60002".parse().unwrap();
        let (replies, _) = feed_and_process(&manager, addr, &register_frame(1)).await;
        let issued = auth_code_from_register_reply(&replies[0].1);
        assert!(!issued.is_empty(), "必须下发一个鉴权码");
        assert_ne!(issued, "GBServer", "不应再写死默认字面量");

        let stored = crate::db::jt1078::get_auth_code_by_phone(&pool, PHONE)
            .await
            .unwrap()
            .expect("鉴权码必须落库");
        assert_eq!(stored.trim_end_matches('\0'), issued);
    }

    /// C3：0x0102 是**终端鉴权**。鉴权码对得上 → 0x8102 结果 0 并记为已鉴权；
    /// 对不上 → 0x8102 结果 1，且不记为已鉴权。
    #[tokio::test]
    async fn terminal_authentication_accepts_matching_code_only() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed_terminal(&pool, Some(CONFIGURED_AUTH_CODE)).await;
        let manager = manager_with_pool(pool.clone());
        let addr: std::net::SocketAddr = "127.0.0.1:60003".parse().unwrap();

        // 先注册，拿到平台下发的鉴权码。
        let (replies, _) = feed_and_process(&manager, addr, &register_frame(1)).await;
        let issued = auth_code_from_register_reply(&replies[0].1);

        // 错误的鉴权码 → 失败。
        let bad = crate::jt1078::command::build_jt808_frame(0x0102, PHONE, 2, b"WRONG-CODE");
        let (replies, _) = feed_and_process(&manager, addr, &bad).await;
        assert_eq!(replies.len(), 1);
        let (_, msg) = crate::jt1078::frame::parse_jt808_frame(&replies[0].1).unwrap();
        assert_eq!(msg.msg_id, 0x8102, "鉴权应答必须是 0x8102");
        assert_eq!(msg.body[2], 1, "鉴权码不对时结果必须是 1（失败）");
        assert!(!manager.is_authenticated(PHONE).await);

        // 正确的鉴权码 → 通过。
        let good = crate::jt1078::command::build_jt808_frame(0x0102, PHONE, 3, issued.as_bytes());
        let (replies, _) = feed_and_process(&manager, addr, &good).await;
        let (_, msg) = crate::jt1078::frame::parse_jt808_frame(&replies[0].1).unwrap();
        assert_eq!(msg.msg_id, 0x8102);
        assert_eq!(msg.body[2], 0, "鉴权码正确时结果必须是 0（成功）");
        assert!(manager.is_authenticated(PHONE).await);
    }

    /// 强制鉴权档：未通过鉴权的终端**不置为在线**；通过后才置为在线。
    #[tokio::test]
    async fn strict_mode_keeps_unauthenticated_terminal_offline() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed_terminal(&pool, Some(CONFIGURED_AUTH_CODE)).await;
        let mut m = crate::jt1078::manager::Jt1078Manager::new(
            std::time::Duration::from_secs(30),
            std::time::Duration::from_millis(200),
            None,
            false,
        );
        m.set_pool(pool.clone());
        m.set_auth_required(true);
        let manager = Arc::new(m);
        let addr: std::net::SocketAddr = "127.0.0.1:60004".parse().unwrap();

        feed_and_process(&manager, addr, &register_frame(1)).await;
        let status: Option<bool> =
            sqlx::query_scalar("SELECT status FROM gb_jt_terminal WHERE phone_number = ?")
                .bind(PHONE)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, Some(false), "未鉴权时不应置为在线");

        // 鉴权通过后才置为在线。
        let good = crate::jt1078::command::build_jt808_frame(
            0x0102,
            PHONE,
            2,
            CONFIGURED_AUTH_CODE.as_bytes(),
        );
        feed_and_process(&manager, addr, &good).await;
        let status: Option<bool> =
            sqlx::query_scalar("SELECT status FROM gb_jt_terminal WHERE phone_number = ?")
                .bind(PHONE)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, Some(true), "鉴权通过后应置为在线");
    }

    /// 宽松档（默认）：终端不鉴权也能注册成功并在线 ——
    /// 真实终端与本仓库的模拟器都不会回 0x0102。
    #[tokio::test]
    async fn lenient_mode_registers_without_authentication() {
        let pool = crate::test_support::sqlite_pool_with_schema().await;
        seed_terminal(&pool, Some(CONFIGURED_AUTH_CODE)).await;
        let manager = manager_with_pool(pool.clone());
        let addr: std::net::SocketAddr = "127.0.0.1:60005".parse().unwrap();

        feed_and_process(&manager, addr, &register_frame(1)).await;
        let status: Option<bool> =
            sqlx::query_scalar("SELECT status FROM gb_jt_terminal WHERE phone_number = ?")
                .bind(PHONE)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, Some(true), "宽松档下不鉴权也应在线");
    }
}
