use tokio::net::{TcpListener, UdpSocket};
use tokio::io::AsyncReadExt;
use std::error::Error;

use super::Jt1078Server;

/// Start lightweight JT1078 TCP and UDP listeners. This spawns per-connection handlers
/// that manage simple authentication, heartbeat, and reassembly using session state.
pub async fn start(_server: &Jt1078Server) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Bind TCP listener on a default port (can be configured later)
    let tcp_addr = "0.0.0.0:60000";

    // Create a manager used by both TCP and UDP listeners and start cleanup
    let manager = crate::jt1078::manager::Jt1078Manager::new(std::time::Duration::from_secs(60), std::time::Duration::from_millis(200));
    let manager_for_cleanup = manager.clone();
    tokio::spawn(async move {
        manager_for_cleanup.cleanup_loop(std::time::Duration::from_secs(30)).await;
    });

    match TcpListener::bind(tcp_addr).await {
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
                                            let frames = manager.feed_bytes(addr, &read_buf[..n]).await;
                                            for f in frames {
                                                // process payload via manager which owns sessions
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
            // For UDP, use the same manager to maintain sessions per peer
            let manager_udp = manager.clone();
            let socket_udp = socket;
            tokio::spawn(async move {
                let mut buf = vec![0u8; 1500];
                loop {
                    match socket_udp.recv_from(&mut buf).await {
                        Ok((n, addr)) => {
                            tracing::info!("JT1078 UDP packet {} bytes from {}", n, addr);
                            // feed bytes into manager-managed session
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
