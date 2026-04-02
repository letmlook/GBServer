//! TCP 传输层实现

use std::net::SocketAddr;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct TcpListener {
    listener: TokioTcpListener,
}

impl TcpListener {
    pub async fn bind(addr: &str) -> anyhow::Result<Self> {
        let listener = TokioTcpListener::bind(addr).await?;
        Ok(Self { listener })
    }

    pub async fn accept(&mut self) -> anyhow::Result<(TcpStream, SocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        Ok((TcpStream { stream }, addr))
    }

    pub fn into_inner(self) -> TokioTcpListener {
        self.listener
    }
}

pub struct TcpStream {
    stream: tokio::net::TcpStream,
}

impl TcpStream {
    pub async fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        Ok(self.stream.read(buf).await?)
    }

    pub async fn write_all(&mut self, buf: &[u8]) -> anyhow::Result<()> {
        Ok(self.stream.write_all(buf).await?)
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.local_addr()
    }
}
