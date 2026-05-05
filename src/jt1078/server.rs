use tokio::net::{TcpListener, UdpSocket};
use tokio::io::AsyncReadExt;
use std::error::Error;

use super::Jt1078Server;

/// Start lightweight JT1078 TCP and UDP listeners. This spawns per-connection handlers
/// that manage simple authentication, heartbeat, and reassembly using session state.
pub async fn start(_server: &Jt1078Server) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Bind TCP listener on a default port (can be configured later)
    let tcp_addr = "0.0.0.0:60000";
    match TcpListener::bind(tcp_addr).await {
        Ok(listener) => {
            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut socket, addr)) => {
                            tracing::info!("JT1078 TCP connection accepted from {}", addr);
                            tokio::spawn(async move {
                                let mut session = crate::jt1078::session::Jt1078Session::new(addr);
                                let mut read_buf = [0u8; 4096];
                                loop {
                                    match socket.read(&mut read_buf).await {
                                        Ok(0) => {
                                            tracing::info!("JT1078 TCP connection closed by {}", addr);
                                            break;
                                        }
                                        Ok(n) => {
                                            let frames = session.feed_bytes(&read_buf[..n]);
                                            for f in frames {
                                                match session.process_payload(&f) {
                                                    crate::jt1078::session::FrameKind::AuthFailure => {
                                                        tracing::warn!("JT1078 auth failed from {}", addr);
                                                        // close connection by returning from task
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
                                                        // TODO: dispatch data to processing pipeline
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
    let udp_addr = "0.0.0.0:60000";
    match UdpSocket::bind(udp_addr).await {
        Ok(socket) => {
            tokio::spawn(async move {
                let mut buf = vec![0u8; 1500];
                loop {
                    match socket.recv_from(&mut buf).await {
                        Ok((n, addr)) => {
                            tracing::info!("JT1078 UDP packet {} bytes from {}", n, addr);
                            // For UDP, do a lightweight session per-packet (persistent sessions could be tracked if needed)
                            let mut session = crate::jt1078::session::Jt1078Session::new(addr);
                            let frames = session.feed_bytes(&buf[..n]);
                            for f in frames {
                                match session.process_payload(&f) {
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
