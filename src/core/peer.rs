use log::{debug, info};
use serde_json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::core::protocol::{Message, MessageType};
use crate::network::Transport;
use crate::storage::FileInfo;
use crate::utils::{P2PError, RateLimiter, Result};

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: Uuid,
    pub addr: SocketAddr,
    pub last_seen: Instant,
    pub name: String,
}

pub struct Peer {
    pub id: Uuid,
    pub addr: SocketAddr,
    pub name: String,
    stream: Arc<Mutex<TcpStream>>,
    last_ping: Arc<Mutex<Instant>>,
    rate_limiter: Option<Arc<RateLimiter>>,
}

impl Peer {
    pub async fn new(
        id: Uuid,
        addr: SocketAddr,
        stream: TcpStream,
        rate_limiter: Option<Arc<RateLimiter>>,
    ) -> Result<Self> {
        // ✅ Enable TCP keep-alive to maintain connections
        stream
            .set_nodelay(true)
            .map_err(|e| P2PError::NetworkError(format!("Failed to set TCP_NODELAY: {}", e)))?;

        Ok(Self {
            id,
            addr,
            name: format!("Peer-{}", addr.port()),
            stream: Arc::new(Mutex::new(stream)),
            last_ping: Arc::new(Mutex::new(Instant::now())),
            rate_limiter,
        })
    }

    pub async fn send_message(&self, message: Message) -> Result<()> {
        let mut stream = self.stream.lock().await;
        let serialized = serde_json::to_vec(&message)
            .map_err(|e| P2PError::SerializationError(e.to_string()))?;

        // Add timeout for network operations
        tokio::time::timeout(Duration::from_secs(5), async {
            Transport::send_data(&mut stream, &serialized, self.rate_limiter.as_deref()).await
        })
        .await
        .map_err(|_| P2PError::NetworkError("Send timeout".to_string()))??;

        debug!("Sent message to peer {}: {:?}", self.addr, message.msg_type);
        Ok(())
    }

    pub async fn receive_message(&self) -> Result<Message> {
        let mut stream = self.stream.lock().await;

        // Add timeout for receive operations
        let buffer = tokio::time::timeout(Duration::from_secs(10), async {
            Transport::receive_data(&mut stream, 10_000_000, self.rate_limiter.as_deref()).await
        })
        .await
        .map_err(|_| P2PError::NetworkError("Receive timeout".to_string()))??;

        let message: Message = serde_json::from_slice(&buffer)
            .map_err(|e| P2PError::SerializationError(e.to_string()))?;

        debug!(
            "Received message from peer {}: {:?}",
            self.addr, message.msg_type
        );

        // Update last ping
        *self.last_ping.lock().await = Instant::now();

        Ok(message)
    }

    pub async fn request_file_list(&self) -> Result<Vec<FileInfo>> {
        let request = Message::new(MessageType::FileListRequest, None);
        self.send_message(request).await?;

        let response = self.receive_message().await?;
        if let MessageType::FileListResponse = response.msg_type {
            if let Some(data) = response.data {
                let files: Vec<FileInfo> = serde_json::from_value(data["files"].clone())
                    .map_err(|e| P2PError::SerializationError(e.to_string()))?;
                return Ok(files);
            }
        }

        Err(P2PError::InvalidResponse(
            "Expected file list response".to_string(),
        ))
    }

    pub async fn request_file_info(&self, file_hash: &str) -> Result<FileInfo> {
        let request = Message::file_info_request(file_hash.to_string());
        self.send_message(request).await?;

        let response = self.receive_message().await?;
        if let MessageType::FileInfoResponse = response.msg_type {
            if let Some(data) = response.data {
                let file_info: FileInfo = serde_json::from_value(data)
                    .map_err(|e| P2PError::SerializationError(e.to_string()))?;
                return Ok(file_info);
            }
        }

        Err(P2PError::InvalidResponse(
            "Expected file info response".to_string(),
        ))
    }

    pub async fn request_chunk(&self, chunk_hash: &str) -> Result<Vec<u8>> {
        let request = Message::chunk_request(chunk_hash.to_string());
        self.send_message(request).await?;

        let response = self.receive_message().await?;
        if let MessageType::ChunkResponse = response.msg_type {
            if let Some(data) = response.data {
                let chunk_data = data["data"]
                    .as_str()
                    .ok_or_else(|| P2PError::InvalidResponse("Missing chunk data".to_string()))?;
                let bytes = hex::decode(chunk_data)
                    .map_err(|e| P2PError::InvalidResponse(format!("Invalid hex data: {}", e)))?;
                return Ok(bytes);
            }
        }

        Err(P2PError::InvalidResponse(
            "Expected chunk response".to_string(),
        ))
    }

    pub async fn ping(&self) -> Result<Duration> {
        let start = Instant::now();
        let ping = Message::ping();
        self.send_message(ping).await?;

        let response = self.receive_message().await?;
        if let MessageType::Pong = response.msg_type {
            Ok(start.elapsed())
        } else {
            Err(P2PError::InvalidResponse(
                "Expected pong response".to_string(),
            ))
        }
    }

    pub fn is_alive(&self) -> bool {
        self.last_ping
            .try_lock()
            .map(|ping| ping.elapsed() < Duration::from_secs(60))
            .unwrap_or(false)
    }
}

pub struct PeerManager {
    peers: HashMap<Uuid, Arc<Peer>>,
    discovered_peers: Vec<SocketAddr>,
    max_peers: usize,
}

impl PeerManager {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            discovered_peers: Vec::new(),
            max_peers: 50,
        }
    }

    pub async fn add_peer(&mut self, peer: Peer) -> Result<()> {
        if self.peers.len() >= self.max_peers {
            return Err(P2PError::TooManyPeers(self.max_peers));
        }

        let peer_id = peer.id;
        let peer_arc = Arc::new(peer);

        self.peers.insert(peer_id, peer_arc);
        info!("Added peer: {} ({})", peer_id, self.peers.len());

        Ok(())
    }

    pub fn add_discovered_peer(&mut self, addr: SocketAddr) {
        if !self.discovered_peers.contains(&addr) && self.discovered_peers.len() < 100 {
            self.discovered_peers.push(addr);
            info!("Discovered new peer: {}", addr);
        }
    }

    pub fn get_peer(&self, peer_id: &Uuid) -> Option<Arc<Peer>> {
        self.peers.get(peer_id).cloned()
    }

    pub fn get_all_peers(&self) -> Vec<Arc<Peer>> {
        self.peers.values().cloned().collect()
    }

    pub fn remove_peer(&mut self, peer_id: &Uuid) {
        if self.peers.remove(peer_id).is_some() {
            info!("Removed peer: {} ({})", peer_id, self.peers.len());
        }
    }

    pub async fn cleanup_dead_peers(&mut self) {
        let dead_peers: Vec<Uuid> = self
            .peers
            .iter()
            .filter(|(_, peer)| !peer.is_alive())
            .map(|(id, _)| *id)
            .collect();

        for peer_id in dead_peers {
            self.remove_peer(&peer_id);
        }
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}
