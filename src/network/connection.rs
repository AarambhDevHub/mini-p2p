use log::{info, warn};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

use crate::core::Peer;
use crate::utils::{P2PError, Result};

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_id: Uuid,
    pub addr: SocketAddr,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    max_connections: usize,
    connection_timeout: Duration,
}

impl ConnectionManager {
    pub fn new(max_connections: usize) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections,
            connection_timeout: Duration::from_secs(300), // 5 minutes
        }
    }

    pub async fn add_connection(&self, peer: &Peer) -> Result<()> {
        let mut connections = self.connections.write().await;

        if connections.len() >= self.max_connections {
            return Err(P2PError::TooManyPeers(self.max_connections));
        }

        let connection_info = ConnectionInfo {
            peer_id: peer.id,
            addr: peer.addr,
            connected_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
        };

        connections.insert(peer.id, connection_info);
        info!("Connection added: {} -> {}", peer.id, peer.addr);
        Ok(())
    }

    pub async fn remove_connection(&self, peer_id: &Uuid) -> bool {
        let mut connections = self.connections.write().await;
        if connections.remove(peer_id).is_some() {
            info!("Connection removed: {}", peer_id);
            true
        } else {
            false
        }
    }

    pub async fn update_activity(&self, peer_id: &Uuid, bytes_sent: u64, bytes_received: u64) {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(peer_id) {
            conn.last_activity = Instant::now();
            conn.bytes_sent += bytes_sent;
            conn.bytes_received += bytes_received;
        }
    }

    pub async fn cleanup_stale_connections(&self) -> Vec<Uuid> {
        let mut connections = self.connections.write().await;
        let now = Instant::now();

        let stale_peers: Vec<Uuid> = connections
            .iter()
            .filter(|(_, conn)| now.duration_since(conn.last_activity) > self.connection_timeout)
            .map(|(id, _)| *id)
            .collect();

        for peer_id in &stale_peers {
            connections.remove(peer_id);
            warn!("Removed stale connection: {}", peer_id);
        }

        stale_peers
    }

    pub async fn get_connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    pub async fn get_connection_info(&self, peer_id: &Uuid) -> Option<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.get(peer_id).cloned()
    }

    pub async fn list_connections(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.values().cloned().collect()
    }
}
