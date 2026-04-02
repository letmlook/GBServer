use std::net::SocketAddr;
use tokio::net::UdpSocket as TokioUdpSocket;
use bytes::BytesMut;
use anyhow::Result;

pub struct UdpSocket {
    socket: TokioUdpSocket,
    buffer: BytesMut,
}

impl UdpSocket {
    pub async fn bind(addr: &str) -> Result<Self> {
        let socket = TokioUdpSocket::bind(addr).await?;
        Ok(Self {
            socket,
            buffer: BytesMut::with_capacity(65535),
        })
    }

    pub async fn recv_from(&mut self) -> Result<(Vec<u8>, SocketAddr)> {
        let mut buf = vec![0u8; 65535];
        let (len, addr) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(len);
        Ok((buf, addr))
    }

    pub async fn send_to(&mut self, buf: &[u8], addr: &SocketAddr) -> Result<usize> {
        Ok(self.socket.send_to(buf, addr).await?)
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }
}
