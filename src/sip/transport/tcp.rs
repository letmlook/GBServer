use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpListener as TokioTcpListener;
#[cfg(test)]
use tokio::net::TcpStream as TokioTcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{Mutex, RwLock};
use bytes::BytesMut;
use dashmap::DashMap;

use crate::sip::core::parser::Parser;
use crate::sip::core::SipMessage;

/// RFC 3261 §18.2.2：对经 TCP 到达的请求，响应必须经同一 TCP 连接返回。
///
/// `process_tcp_message` 处理请求前登记 (TCP 对端地址 → 连接管理器)，
/// `SipServer::send_response` 发送响应时优先查此表走 TCP，查不到再走 UDP。
/// 连接关闭时移除对应表项。
static TCP_RESPONSE_ROUTES: OnceLock<DashMap<SocketAddr, TcpConnectionManager>> = OnceLock::new();

pub fn tcp_response_routes() -> &'static DashMap<SocketAddr, TcpConnectionManager> {
    TCP_RESPONSE_ROUTES.get_or_init(DashMap::new)
}

pub struct TcpListener {
    listener: TokioTcpListener,
    addr: SocketAddr,
}

impl TcpListener {
    pub async fn bind(addr: &str) -> anyhow::Result<Self> {
        let listener = TokioTcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self { listener, addr: local_addr })
    }

    /// 接受连接并**拆成读写两半**。
    ///
    /// 必须拆开：读循环会长时间阻塞在读上，如果和响应发送共用同一把锁，
    /// 「读循环持锁等数据」与「响应处理持锁写数据」会互相阻塞甚至死锁
    /// （实测：TCP 设备 REGISTER 后平台一个字节都回不出来）。
    pub async fn accept(&mut self) -> anyhow::Result<(TcpReader, OwnedWriteHalf, SocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        let (read_half, write_half) = stream.into_split();
        Ok((TcpReader::new(read_half), write_half, addr))
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

/// TCP 连接的**读半**：把字节流按 Content-Length 切成一条条 SIP 消息。
///
/// 与写半分开持有，读循环阻塞等待数据时不会占住发送路径。
pub struct TcpReader {
    stream: OwnedReadHalf,
    read_buffer: BytesMut,
}

impl TcpReader {
    pub fn new(stream: OwnedReadHalf) -> Self {
        Self {
            stream,
            read_buffer: BytesMut::with_capacity(65535),
        }
    }

    /// 读取一条完整消息，同时返回**原始字节**。
    ///
    /// 返回原始字节是刻意的：此前的做法是「读到消息 → `format!("{}", msg)`
    /// 重新序列化 → 再交给 `handle_packet` 解析」，而 `Display` 当时只写
    /// `\n`，导致重解析后**头字段全部丢失**。直接下发原始字节，
    /// 既省一次往返，也不再把正确性押在序列化实现上。
    pub async fn read_message(&mut self) -> anyhow::Result<Option<(SipMessage, Vec<u8>)>> {
        loop {
            if let Some(msg) = self.try_parse_message()? {
                return Ok(Some(msg));
            }

            let mut buf = vec![0u8; 8192];
            let n = self.stream.read(&mut buf).await?;
            if n == 0 {
                return Ok(None);
            }
            self.read_buffer.extend_from_slice(&buf[..n]);
        }
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.local_addr()
    }

    fn try_parse_message(&mut self) -> anyhow::Result<Option<(SipMessage, Vec<u8>)>> {
        let data = &self.read_buffer[..];
        if data.is_empty() {
            return Ok(None);
        }

        let text = match String::from_utf8(data.to_vec()) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        if !text.contains("\r\n\r\n") {
            return Ok(None);
        }

        let header_end = text.find("\r\n\r\n").unwrap();
        let header_text = &text[..header_end];
        let header_lines: Vec<&str> = header_text.split("\r\n").collect();

        let mut content_length: Option<usize> = None;
        for line in &header_lines {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") || lower.starts_with("l:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().ok();
                    break;
                }
            }
        }

        let body_start = header_end + 4;
        let body_len = content_length.unwrap_or(0);
        let total_len = body_start + body_len;

        if self.read_buffer.len() < total_len {
            return Ok(None);
        }

        let message_bytes = self.read_buffer.split_to(total_len).freeze();
        let message = Parser::parse(&message_bytes)?;
        Ok(Some((message, message_bytes.to_vec())))
    }

}

#[derive(Clone)]
pub struct TcpConnectionManager {
    /// 对端地址 → **写半**。写半单独加锁：读循环不持有它，
    /// 因此"平台主动发请求"与"回响应"不会再与读阻塞互相顶住。
    connections: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<OwnedWriteHalf>>>>>,
    max_connections: usize,
}

impl TcpConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections: 1000,
        }
    }

    pub fn with_max_connections(max: usize) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections: max,
        }
    }

    pub async fn add_connection(&self, addr: SocketAddr, stream: OwnedWriteHalf) {
        let mut guard = self.connections.write().await;
        // 同址重连：旧写半已被新连接取代
        guard.remove(&addr);
        if guard.len() >= self.max_connections {
            tracing::warn!(
                "TCP 连接数已达上限 {}，仍接受新连接 {}（避免设备静默失联）",
                self.max_connections,
                addr
            );
        }
        guard.insert(addr, Arc::new(Mutex::new(stream)));
    }

    pub async fn remove_connection(&self, addr: &SocketAddr) {
        self.connections.write().await.remove(addr);
    }

    pub async fn get_connection(&self, addr: &SocketAddr) -> Option<Arc<Mutex<OwnedWriteHalf>>> {
        self.connections.read().await.get(addr).cloned()
    }

    /// 经 TCP 发送一条 SIP 消息。
    ///
    /// 返回 `false` 表示**没有该对端的 TCP 连接**（调用方应回落到 UDP），
    /// 而不是"发送成功但什么都没做"。
    ///
    /// 此前这里是 `if let Some(conn) = … { … } Ok(())`：连接不存在时静默
    /// 返回成功，调用方无从判断消息到底有没有出去。`send_response` 正是靠
    /// "正好有连接"才没暴露该问题，而所有**出站请求**都只走 UDP。
    pub async fn send_to(&self, addr: &SocketAddr, data: &str) -> anyhow::Result<bool> {
        match self.get_connection(addr).await {
            Some(conn) => {
                let mut stream = conn.lock().await;
                stream.write_all(data.as_bytes()).await?;
                stream.flush().await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// 按 IP 找连接（端口可能因 NAT/重连而变化）
    pub async fn find_by_ip(&self, ip: std::net::IpAddr) -> Option<SocketAddr> {
        let guard = self.connections.read().await;
        guard.keys().find(|a| a.ip() == ip).copied()
    }

    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    pub async fn cleanup_stale_connections(&self) {
        let mut guard = self.connections.write().await;
        let addrs: Vec<SocketAddr> = guard.keys().cloned().collect();
        for addr in addrs {
            if let Some(conn) = guard.get(&addr) {
                let is_stale = {
                    let stream = conn.lock().await;
                    stream.local_addr().is_err()
                };
                if is_stale {
                    guard.remove(&addr);
                }
            }
        }
    }
}

impl Default for TcpConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 出站 SIP 请求的统一发送口。
///
/// # 为什么必须统一
///
/// RFC 3261 §18.1.1：请求必须发往目标 URI 的传输方式所对应的通道。
/// 以 **TCP 注册**的国标设备只在 TCP 连接上监听，平台若用 UDP 发请求，
/// 设备侧什么都不会收到（也不会有 ICMP 提示）——现象是"平台发了 BYE /
/// 目录查询 / PTZ / 云台控制，设备毫无反应"，而平台日志显示"已发送"。
///
/// 此前只有**响应**走了 TCP（`send_response` 按 `tcp_response_routes` 回同一
/// 连接），所有**出站请求**（INVITE/ACK/BYE/MESSAGE/INFO/SUBSCRIBE）都硬编码
/// 发 UDP：TCP 信令设备的可控性为零。
///
/// 匹配顺序：精确 `addr` → 同 IP（TCP 重连/NAT 后端口变化）→ UDP。
pub async fn send_sip_out(
    udp: &tokio::net::UdpSocket,
    addr: std::net::SocketAddr,
    message: &str,
) -> anyhow::Result<bool> {
    // 精确地址命中优先
    let exact = {
        let routes = tcp_response_routes();
        routes.get(&addr).map(|r| r.value().clone())
    };
    if let Some(mgr) = exact {
        match mgr.send_to(&addr, message).await {
            Ok(true) => {
                tracing::debug!("SIP request sent over TCP to {}", addr);
                return Ok(true);
            }
            Ok(false) => {
                tracing::debug!("TCP 连接 {} 已不存在，回落发送（目标 {}）", addr, addr);
                tcp_response_routes().remove(&addr);
            }
            Err(e) => {
                tracing::warn!("TCP 发送到 {} 失败，回落 UDP: {}", addr, e);
                tcp_response_routes().remove(&addr);
            }
        }
    }

    // 精确地址没命中：按 IP 兜底（同一设备重连后源端口会变）。
    //
    // **必须唯一**：同一 NAT 出口后面通常挂着多台设备，各自一条 TCP 连接。
    // 如果同 IP 有多条连接还硬挑第一条，就会把给 A 设备的 BYE 发到 B 设备
    // 的连接上 —— 比回落到 UDP 更糟（错误地"成功"了）。因此只在
    // 该 IP 恰好只有一条连接时才兜底。
    let by_ip: Option<(TcpConnectionManager, SocketAddr)> = {
        let routes = tcp_response_routes();
        let mut candidates: Vec<(TcpConnectionManager, SocketAddr)> = Vec::new();
        for entry in routes.iter() {
            if let Some(alt) = entry.value().find_by_ip(addr.ip()).await {
                candidates.push((entry.value().clone(), alt));
            }
        }
        match candidates.len() {
            1 => candidates.pop(),
            n if n > 1 => {
                tracing::debug!(
                    "{} 有 {} 条 TCP 连接且精确地址未命中，放弃 IP 兜底改走 UDP（避免发错设备）",
                    addr.ip(),
                    n
                );
                None
            }
            _ => None,
        }
    };
    if let Some((mgr, tcp_addr)) = by_ip {
        match mgr.send_to(&tcp_addr, message).await {
            Ok(true) => {
                tracing::debug!("SIP request sent over TCP to {} (按 IP 匹配)", tcp_addr);
                return Ok(true);
            }
            Ok(false) => {
                tcp_response_routes().remove(&tcp_addr);
            }
            Err(e) => {
                tracing::warn!("TCP 发送到 {} 失败，回落 UDP: {}", tcp_addr, e);
                tcp_response_routes().remove(&tcp_addr);
            }
        }
    }

    udp.send_to(message.as_bytes(), addr).await?;
    Ok(false)
}

#[cfg(test)]
mod outbound_transport_tests {
    use super::*;

    /// `tcp_response_routes()` 是进程级全局表，而这些用例都在 127.0.0.1 上
    /// 建连接 —— 并行执行会互相看到对方的连接（IP 兜底尤其明显）。
    /// 用一把异步锁把它们串行化，保证用例之间互不干扰。
    fn route_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    /// 以 TCP 注册的设备，出站请求必须走 TCP。
    ///
    /// 背景：本项目此前只有**响应**会按 RFC 3261 §18.2.2 回到同一 TCP 连接，
    /// 所有**出站请求**（INVITE/ACK/BYE/MESSAGE/INFO/SUBSCRIBE）都硬编码走
    /// UDP —— TCP 信令设备完全不可控（发了没反应，日志却显示"已发送"）。
    #[tokio::test]
    async fn outbound_request_prefers_tcp_connection() {
        let _guard = route_lock().lock().await;
        // 模拟设备侧：监听一个 TCP 端口，等平台连上来
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();

        // 平台侧作为 TCP 客户端接入，并把连接登记进路由表（真实流程里
        // 是设备连平台，这里用「平台连模拟设备」同样能验证发送通道选择）
        let client = TokioTcpStream::connect(listen_addr).await.unwrap();
        let (_r, w) = client.into_split();
        let mgr = TcpConnectionManager::new();
        mgr.add_connection(listen_addr, w).await;
        tcp_response_routes().insert(listen_addr, mgr.clone());

        // 一个只会被 UDP 命中的假目标（不会被调用）
        let udp = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_peer_addr = udp_peer.local_addr().unwrap();

        let msg = "BYE sip:dev@127.0.0.1 SIP/2.0\r\nCall-ID: tcp-test\r\n\r\n";
        let over_tcp = send_sip_out(&udp, listen_addr, msg).await.unwrap();
        assert!(over_tcp, "存在 TCP 连接时必须走 TCP");

        // 设备侧应收到完整消息
        let (mut accepted, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; msg.len()];
        accepted.read_exact(&mut buf).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&buf), msg);

        // UDP 侧不应有任何数据
        let mut probe = [0u8; 8];
        match tokio::time::timeout(
            std::time::Duration::from_millis(200),
            udp_peer.recv_from(&mut probe),
        )
        .await
        {
            Err(_) => {}
            Ok(Ok((n, from))) => panic!("不应走 UDP，但收到 {} 字节来自 {}", n, from),
            Ok(Err(e)) => panic!("UDP 探测失败: {}", e),
        }

        // 精确地址未命中时按 IP 兜底：换一个端口仍应命中同 IP 的连接
        let alt_addr: SocketAddr = format!("127.0.0.1:1").parse().unwrap();
        assert!(send_sip_out(&udp, alt_addr, msg).await.unwrap());

        tcp_response_routes().remove(&listen_addr);
        let _ = udp_peer_addr;
    }

    /// 没有任何 TCP 连接时必须回落 UDP，并如实返回 `false`。
    #[tokio::test]
    async fn outbound_request_falls_back_to_udp() {
        let _guard = route_lock().lock().await;
        let udp = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let peer_addr = peer.local_addr().unwrap();

        let msg = "MESSAGE sip:dev@127.0.0.1 SIP/2.0\r\nCall-ID: udp-test\r\n\r\n";
        let over_tcp = send_sip_out(&udp, peer_addr, msg).await.unwrap();
        assert!(!over_tcp, "无 TCP 连接时必须如实报告走了 UDP");

        let mut buf = vec![0u8; msg.len()];
        let (n, _) = peer.recv_from(&mut buf).await.unwrap();
        assert_eq!(n, msg.len());
        assert_eq!(String::from_utf8_lossy(&buf), msg);
    }

    /// `send_to` 在连接不存在时必须返回 `Ok(false)` ——
    /// 此前固定返回 `Ok(())`，"发送成功"里混着"什么都没发"。
    #[tokio::test]
    async fn send_to_reports_missing_connection() {
        let mgr = TcpConnectionManager::new();
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        assert!(!mgr.send_to(&addr, "x").await.unwrap());
    }

    /// 连接建立后写入的是一整条 SIP 消息（含 `Content-Length`），
    /// 而不是裸字节 —— 否则设备侧无法按 SIP 分帧。
    #[tokio::test]
    async fn tcp_write_frames_message_with_content_length() {
        let _guard = route_lock().lock().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();
        let client = TokioTcpStream::connect(listen_addr).await.unwrap();
        let (_r, w) = client.into_split();
        let mgr = TcpConnectionManager::new();
        mgr.add_connection(listen_addr, w).await;

        let msg = "MESSAGE sip:dev@127.0.0.1 SIP/2.0\r\nCall-ID: frame-test\r\n\r\n";
        assert!(mgr.send_to(&listen_addr, msg).await.unwrap());

        let (mut accepted, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 512];
        let n = accepted.read(&mut buf).await.unwrap();
        let text = String::from_utf8_lossy(&buf[..n]).to_string();
        assert!(text.starts_with("MESSAGE sip:dev@127.0.0.1 SIP/2.0\r\n"), "{}", text);
        assert!(text.contains("Call-ID: frame-test"), "{}", text);
    }
}
