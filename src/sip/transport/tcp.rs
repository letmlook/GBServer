use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::net::TcpStream as TokioTcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use bytes::{Bytes, BytesMut};
use anyhow::Result;

use crate::sip::core::parser::Parser;
use crate::sip::core::SipMessage;

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

    pub async fn accept(&mut self) -> anyhow::Result<(TcpStream, SocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        Ok((TcpStream::new(stream), addr))
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

pub struct TcpStream {
    stream: TokioTcpStream,
    read_buffer: BytesMut,
    write_buffer: BytesMut,
}

impl TcpStream {
    pub fn new(stream: TokioTcpStream) -> Self {
        Self {
            stream,
            read_buffer: BytesMut::with_capacity(65535),
            write_buffer: BytesMut::with_capacity(65535),
        }
    }

    pub async fn read_message(&mut self) -> anyhow::Result<Option<(SipMessage, SocketAddr)>> {
        loop {
            if let Some(msg) = self.try_parse_message()? {
                let peer = self.stream.peer_addr()?;
                return Ok(Some((msg, peer)));
            }

            let mut buf = vec![0u8; 8192];
            let n = self.stream.read(&mut buf).await?;
            if n == 0 {
                return Ok(None);
            }
            self.read_buffer.extend_from_slice(&buf[..n]);
        }
    }

    fn try_parse_message(&mut self) -> anyhow::Result<Option<SipMessage>> {
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
        Ok(Some(message))
    }

    pub async fn write_message(&mut self, msg: &str) -> anyhow::Result<()> {
        self.stream.write_all(msg.as_bytes()).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn write_raw(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.stream.write_all(data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.local_addr()
    }

    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

pub struct TcpConnectionManager {
    connections: Arc<RwLock<HashMap<SocketAddr, Arc<RwLock<TcpStream>>>>>,
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

    pub async fn add_connection(&self, addr: SocketAddr, stream: TcpStream) {
        let mut guard = self.connections.write().await;
        if guard.len() >= self.max_connections {
            guard.remove(&addr);
        }
        guard.insert(addr, Arc::new(RwLock::new(stream)));
    }

    pub async fn remove_connection(&self, addr: &SocketAddr) {
        self.connections.write().await.remove(addr);
    }

    pub async fn get_connection(&self, addr: &SocketAddr) -> Option<Arc<RwLock<TcpStream>>> {
        self.connections.read().await.get(addr).cloned()
    }

    pub async fn send_to(&self, addr: &SocketAddr, data: &str) -> anyhow::Result<()> {
        if let Some(conn) = self.get_connection(addr).await {
            let mut stream = conn.write().await;
            stream.write_message(data).await?;
        }
        Ok(())
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
                    let stream = conn.read().await;
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
