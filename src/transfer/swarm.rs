use log::{debug, info};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::core::PeerManager;

#[derive(Debug, Clone)]
pub struct SwarmInfo {
    pub file_hash: String,
    pub peers: HashSet<Uuid>,
    pub completed_chunks: HashSet<String>,
    pub total_chunks: usize,
}

pub struct SwarmManager {
    swarms: HashMap<String, SwarmInfo>,
    peer_manager: Arc<RwLock<PeerManager>>,
}

impl SwarmManager {
    pub fn new(peer_manager: Arc<RwLock<PeerManager>>) -> Self {
        Self {
            swarms: HashMap::new(),
            peer_manager,
        }
    }

    pub fn join_swarm(&mut self, file_hash: String, total_chunks: usize) {
        let swarm = SwarmInfo {
            file_hash: file_hash.clone(),
            peers: HashSet::new(),
            completed_chunks: HashSet::new(),
            total_chunks,
        };

        self.swarms.insert(file_hash.clone(), swarm);
        info!("Joined swarm for file: {}", file_hash);
    }

    pub fn add_peer_to_swarm(&mut self, file_hash: &str, peer_id: Uuid) {
        if let Some(swarm) = self.swarms.get_mut(file_hash) {
            swarm.peers.insert(peer_id);
            debug!("Added peer {} to swarm {}", peer_id, file_hash);
        }
    }

    pub fn mark_chunk_completed(&mut self, file_hash: &str, chunk_hash: String) {
        if let Some(swarm) = self.swarms.get_mut(file_hash) {
            swarm.completed_chunks.insert(chunk_hash);
        }
    }

    pub fn get_swarm_peers(&self, file_hash: &str) -> Vec<Uuid> {
        self.swarms
            .get(file_hash)
            .map(|swarm| swarm.peers.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_completion_ratio(&self, file_hash: &str) -> f64 {
        if let Some(swarm) = self.swarms.get(file_hash) {
            swarm.completed_chunks.len() as f64 / swarm.total_chunks as f64
        } else {
            0.0
        }
    }

    pub fn leave_swarm(&mut self, file_hash: &str) {
        if self.swarms.remove(file_hash).is_some() {
            info!("Left swarm for file: {}", file_hash);
        }
    }
}
