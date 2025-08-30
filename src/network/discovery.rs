use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use crate::utils::{NodeUtils, P2PError, Result}; // ✅ Import NodeUtils

const DEFAULT_DISCOVERY_PORT: u16 = 9999;
const MULTICAST_ADDR: &str = "239.255.255.250:9999";
const BROADCAST_ADDR: &str = "255.255.255.255:9999";

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub node_id: Uuid,
    pub node_name: String,
    pub listen_port: u16,
    pub timestamp: u64,
    pub message_type: DiscoveryMessageType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DiscoveryMessageType {
    Announcement,
    Request,
    Response,
}

pub struct Discovery {
    node_id: Uuid,
    node_name: String,
    socket: Option<UdpSocket>,
    listen_port: u16,
    discovery_port: Option<u16>,
}

impl Discovery {
    pub fn new(node_id: Uuid, node_name: String) -> Self {
        Self {
            node_id,
            node_name,
            socket: None,
            listen_port: 8080,
            discovery_port: None,
        }
    }

    pub async fn start_listener(
        &mut self,
        listen_port: u16,
        discovery_port: Option<u16>,
    ) -> Result<()> {
        self.listen_port = listen_port;
        self.discovery_port = discovery_port;

        // Only start listener if discovery port is specified
        if let Some(disc_port) = discovery_port {
            let bind_addr = format!("0.0.0.0:{}", disc_port)
                .parse()
                .map_err(|e| P2PError::NetworkError(format!("Invalid address: {}", e)))?;

            // ✅ Use reusable socket creation
            match NodeUtils::create_reusable_udp_socket(bind_addr).await {
                Ok(socket) => {
                    // Enable broadcast
                    socket.set_broadcast(true).map_err(|e| {
                        P2PError::NetworkError(format!("Failed to set broadcast: {}", e))
                    })?;

                    self.socket = Some(socket);
                    info!(
                        "Discovery service listening on port {} with reuse enabled",
                        disc_port
                    );

                    // Start listener task with reusable socket
                    let listener_socket = NodeUtils::create_reusable_udp_socket(bind_addr).await?;
                    let node_id = self.node_id;
                    let node_name = self.node_name.clone();
                    let listen_port = self.listen_port;

                    tokio::spawn(async move {
                        Self::listen_loop(listener_socket, node_id, node_name, listen_port).await;
                    });
                }
                Err(e) => {
                    warn!(
                        "Failed to create discovery socket: {}. Discovery disabled.",
                        e
                    );
                    return Ok(()); // Don't fail the entire node if discovery fails
                }
            }
        } else {
            info!("Discovery listener disabled (no discovery port specified)");
        }

        Ok(())
    }

    async fn listen_loop(socket: UdpSocket, node_id: Uuid, node_name: String, listen_port: u16) {
        let mut buffer = [0u8; 1024];
        info!("Discovery listener started for node: {}", node_name);

        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((len, addr)) => {
                    if let Ok(message) = serde_json::from_slice::<DiscoveryMessage>(&buffer[..len])
                    {
                        debug!(
                            "Received discovery message from {}: {:?}",
                            addr, message.message_type
                        );

                        match message.message_type {
                            DiscoveryMessageType::Request => {
                                let response = DiscoveryMessage {
                                    node_id,
                                    node_name: node_name.clone(),
                                    listen_port,
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    message_type: DiscoveryMessageType::Response,
                                };

                                if let Ok(data) = serde_json::to_vec(&response) {
                                    let _ = socket.send_to(&data, addr).await;
                                }
                            }
                            DiscoveryMessageType::Announcement | DiscoveryMessageType::Response => {
                                info!(
                                    "Discovered peer: {} ({}:{})",
                                    message.node_name,
                                    addr.ip(),
                                    message.listen_port
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Discovery listen error: {}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    pub async fn announce(&self) -> Result<()> {
        let message = DiscoveryMessage {
            node_id: self.node_id,
            node_name: self.node_name.clone(),
            listen_port: self.listen_port,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            message_type: DiscoveryMessageType::Announcement,
        };

        let serialized = serde_json::to_vec(&message)
            .map_err(|e| P2PError::SerializationError(e.to_string()))?;

        // Use a temporary socket for announcements
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
            let _ = socket.set_broadcast(true);
            let _ = socket.send_to(&serialized, MULTICAST_ADDR).await;
            let _ = socket.send_to(&serialized, BROADCAST_ADDR).await;
        }

        debug!("Announced presence on network");
        Ok(())
    }

    pub async fn discover_peers(&self) -> Result<Vec<SocketAddr>> {
        let request = DiscoveryMessage {
            node_id: self.node_id,
            node_name: self.node_name.clone(),
            listen_port: self.listen_port,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            message_type: DiscoveryMessageType::Request,
        };

        let serialized = serde_json::to_vec(&request)
            .map_err(|e| P2PError::SerializationError(e.to_string()))?;

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_broadcast(true)?;

        // Send discovery request
        socket.send_to(&serialized, BROADCAST_ADDR).await?;

        // Collect responses for 2 seconds
        let mut peers = Vec::new();
        let mut buffer = [0u8; 1024];

        let collection_future = async {
            for _ in 0..10 {
                if let Ok((len, addr)) = socket.recv_from(&mut buffer).await {
                    if let Ok(message) = serde_json::from_slice::<DiscoveryMessage>(&buffer[..len])
                    {
                        if message.node_id != self.node_id {
                            let peer_addr = SocketAddr::new(addr.ip(), message.listen_port);
                            peers.push(peer_addr);
                        }
                    }
                }
            }
        };

        let _ = timeout(Duration::from_secs(2), collection_future).await;

        debug!("Discovered {} peers", peers.len());
        Ok(peers)
    }
}
