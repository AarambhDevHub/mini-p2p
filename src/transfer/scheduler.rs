use log::debug;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ChunkRequest {
    pub chunk_hash: String,
    pub chunk_index: usize,
    pub requested_at: Instant,
    pub peer_id: Option<Uuid>,
}

pub struct ChunkScheduler {
    pending_requests: VecDeque<ChunkRequest>,
    in_progress: HashMap<String, ChunkRequest>,
    completed: HashMap<String, Vec<u8>>,
    rarest_first: bool,
}

impl ChunkScheduler {
    pub fn new() -> Self {
        Self {
            pending_requests: VecDeque::new(),
            in_progress: HashMap::new(),
            completed: HashMap::new(),
            rarest_first: true,
        }
    }

    pub fn add_chunk_request(&mut self, chunk_hash: String, chunk_index: usize) {
        let request = ChunkRequest {
            chunk_hash: chunk_hash.clone(),
            chunk_index,
            requested_at: Instant::now(),
            peer_id: None,
        };

        self.pending_requests.push_back(request);
        debug!(
            "Added chunk request: {} (index: {})",
            chunk_hash, chunk_index
        );
    }

    pub fn get_next_request(&mut self) -> Option<ChunkRequest> {
        self.pending_requests.pop_front()
    }

    pub fn mark_in_progress(&mut self, chunk_hash: String, peer_id: Uuid) {
        if let Some(mut request) = self
            .pending_requests
            .iter()
            .position(|r| r.chunk_hash == chunk_hash)
            .and_then(|pos| Some(self.pending_requests.remove(pos).unwrap()))
        {
            request.peer_id = Some(peer_id);
            request.requested_at = Instant::now();
            self.in_progress.insert(chunk_hash, request);
        }
    }

    pub fn mark_completed(&mut self, chunk_hash: String, data: Vec<u8>) {
        self.in_progress.remove(&chunk_hash);
        self.completed.insert(chunk_hash.clone(), data);
        debug!("Chunk completed: {}", chunk_hash);
    }

    pub fn mark_failed(&mut self, chunk_hash: String) {
        if let Some(request) = self.in_progress.remove(&chunk_hash) {
            self.pending_requests.push_back(request);
            debug!("Chunk failed, re-queued: {}", chunk_hash);
        }
    }

    pub fn get_progress(&self) -> (usize, usize, usize) {
        (
            self.completed.len(),
            self.in_progress.len(),
            self.pending_requests.len(),
        )
    }

    pub fn cleanup_stale_requests(&mut self, timeout_secs: u64) {
        let now = Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);

        let stale_requests: Vec<String> = self
            .in_progress
            .iter()
            .filter(|(_, request)| now.duration_since(request.requested_at) > timeout)
            .map(|(hash, _)| hash.clone())
            .collect();

        for chunk_hash in stale_requests {
            self.mark_failed(chunk_hash);
        }
    }
}
