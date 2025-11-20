use igd_next::{PortMappingProtocol, SearchOptions, search_gateway};
use log::{debug, info};
use rand::random;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::utils::{P2PError, Result};

/// Manages NAT traversal using UPnP and STUN
pub struct NatManager {
    enabled: bool,
}

impl NatManager {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Attempt to map a port using UPnP
    pub async fn map_port(&self, port: u16, protocol: PortMappingProtocol) -> Result<SocketAddr> {
        if !self.enabled {
            return Err(P2PError::NetworkError("NAT traversal disabled".to_string()));
        }

        info!("Attempting to map port {} via UPnP...", port);

        // Perform UPnP discovery and mapping in a blocking task since igd-next is synchronous
        let result = tokio::task::spawn_blocking(move || {
            let search_options = SearchOptions {
                timeout: Some(Duration::from_secs(5)),
                ..Default::default()
            };

            match search_gateway(search_options) {
                Ok(gateway) => {
                    let local_ip = gateway.addr.ip();
                    let local_addr = SocketAddr::new(local_ip, port);

                    debug!("Found UPnP gateway: {}", gateway);

                    match gateway.add_port(
                        protocol, port, local_addr, 0, // lease duration (0 = permanent)
                        "mini-p2p",
                    ) {
                        Ok(_) => {
                            let external_ip = gateway.get_external_ip().map_err(|e| {
                                P2PError::NetworkError(format!("Failed to get external IP: {}", e))
                            })?;
                            let external_addr = SocketAddr::new(external_ip, port);
                            info!(
                                "Successfully mapped {} port {} -> {}",
                                match protocol {
                                    PortMappingProtocol::TCP => "TCP",
                                    PortMappingProtocol::UDP => "UDP",
                                },
                                port,
                                external_addr
                            );
                            Ok(external_addr)
                        }
                        Err(e) => Err(P2PError::NetworkError(format!(
                            "UPnP port mapping failed: {}",
                            e
                        ))),
                    }
                }
                Err(e) => Err(P2PError::NetworkError(format!(
                    "UPnP gateway search failed: {}",
                    e
                ))),
            }
        })
        .await
        .map_err(|e| P2PError::SystemError(format!("JoinError: {}", e)))??;

        Ok(result)
    }

    /// Discover public IP using STUN
    pub async fn discover_public_ip(&self) -> Result<IpAddr> {
        if !self.enabled {
            return Err(P2PError::NetworkError("NAT traversal disabled".to_string()));
        }

        info!("Discovering public IP via STUN...");

        // Use Google's public STUN server
        let stun_server = "stun.l.google.com:19302";

        // Create a UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        // STUN Binding Request
        // Header: Type (0x0001), Length (0), Cookie (0x2112A442), Transaction ID (12 random bytes)
        let mut packet = vec![0u8; 20];
        packet[0] = 0x00;
        packet[1] = 0x01; // Type: Binding Request
        packet[2] = 0x00;
        packet[3] = 0x00; // Length: 0
        packet[4] = 0x21;
        packet[5] = 0x12;
        packet[6] = 0xA4;
        packet[7] = 0x42; // Magic Cookie

        // Generate random transaction ID
        let transaction_id: Vec<u8> = (0..12).map(|_| random::<u8>()).collect();
        packet[8..20].copy_from_slice(&transaction_id);

        // Send request
        socket.send_to(&packet, stun_server).await?;

        // Wait for response
        let mut buf = vec![0u8; 1024];
        let (len, _) = timeout(Duration::from_secs(3), socket.recv_from(&mut buf))
            .await
            .map_err(|_| P2PError::NetworkError("STUN request timed out".to_string()))??;

        // Parse response
        if len < 20 {
            return Err(P2PError::NetworkError(
                "STUN response too short".to_string(),
            ));
        }

        // Check message type (Binding Response: 0x0101)
        if buf[0] != 0x01 || buf[1] != 0x01 {
            return Err(P2PError::NetworkError(
                "Invalid STUN response type".to_string(),
            ));
        }

        // Check cookie
        if buf[4] != 0x21 || buf[5] != 0x12 || buf[6] != 0xA4 || buf[7] != 0x42 {
            return Err(P2PError::NetworkError("Invalid STUN cookie".to_string()));
        }

        // Parse attributes
        let mut pos = 20;
        while pos + 4 <= len {
            let attr_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
            let attr_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
            pos += 4;

            if pos + attr_len > len {
                break;
            }

            // XOR-MAPPED-ADDRESS (0x0020)
            if attr_type == 0x0020 {
                if attr_len < 8 {
                    continue;
                }

                let family = buf[pos + 1];
                if family == 0x01 {
                    // IPv4
                    let mut ip_bytes = [0u8; 4];
                    ip_bytes.copy_from_slice(&buf[pos + 4..pos + 8]);

                    // XOR with cookie
                    ip_bytes[0] ^= 0x21;
                    ip_bytes[1] ^= 0x12;
                    ip_bytes[2] ^= 0xA4;
                    ip_bytes[3] ^= 0x42;

                    info!("Public IP discovered via STUN: {}", IpAddr::from(ip_bytes));
                    return Ok(IpAddr::from(ip_bytes));
                }
            }

            // MAPPED-ADDRESS (0x0001) - Fallback
            if attr_type == 0x0001 {
                if attr_len < 8 {
                    continue;
                }

                let family = buf[pos + 1];
                if family == 0x01 {
                    // IPv4
                    let mut ip_bytes = [0u8; 4];
                    ip_bytes.copy_from_slice(&buf[pos + 4..pos + 8]);
                    info!(
                        "Public IP discovered via STUN (MAPPED-ADDRESS fallback): {}",
                        IpAddr::from(ip_bytes)
                    );
                    return Ok(IpAddr::from(ip_bytes));
                }
            }

            // Align to 4 bytes
            let padding = (4 - (attr_len % 4)) % 4;
            pos += attr_len + padding;
        }

        Err(P2PError::NetworkError(
            "Public IP not found in STUN response".to_string(),
        ))
    }
}
