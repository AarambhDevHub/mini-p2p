use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::storage::HashUtils;
use crate::utils::{P2PError, Result};

// ✅ Make chunk size dynamic based on file size
pub fn optimal_chunk_size(file_size: usize) -> usize {
    match file_size {
        0..=4096 => file_size,   // Don't chunk tiny files (≤4KB)
        4097..=32768 => 4096,    // 4KB chunks for small files
        32769..=131072 => 16384, // 16KB chunks for medium files
        _ => 64 * 1024,          // 64KB chunks for large files
    }
}

pub const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub index: usize,
    pub hash: String,
    pub data: Vec<u8>,
    pub size: usize,
}

impl Chunk {
    pub fn new(index: usize, data: Vec<u8>) -> Self {
        let hash = HashUtils::hash_data(&data);
        let size = data.len();

        Self {
            index,
            hash,
            data,
            size,
        }
    }

    pub fn verify(&self) -> bool {
        let computed_hash = HashUtils::hash_data(&self.data);
        computed_hash == self.hash
    }

    pub fn is_complete(&self) -> bool {
        !self.data.is_empty() && self.verify()
    }
}

pub struct ChunkManager {
    chunks: HashMap<String, Chunk>,
    chunk_by_index: HashMap<usize, String>, // index -> hash mapping
}

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            chunk_by_index: HashMap::new(),
        }
    }

    pub fn add_chunk(&mut self, chunk: Chunk) -> Result<()> {
        if !chunk.verify() {
            return Err(P2PError::ChunkVerificationFailed(chunk.hash.clone()));
        }

        self.chunk_by_index.insert(chunk.index, chunk.hash.clone());
        self.chunks.insert(chunk.hash.clone(), chunk.clone());

        debug!(
            "Added chunk: {} (index: {})",
            &self
                .chunks
                .get(&self.chunk_by_index[&chunk.index])
                .unwrap()
                .hash[..8],
            chunk.index
        );
        Ok(())
    }

    pub fn get_chunk(&self, hash: &str) -> Option<&Chunk> {
        self.chunks.get(hash)
    }

    pub fn get_chunk_by_index(&self, index: usize) -> Option<&Chunk> {
        self.chunk_by_index
            .get(&index)
            .and_then(|hash| self.chunks.get(hash))
    }

    pub fn has_chunk(&self, hash: &str) -> bool {
        self.chunks.contains_key(hash)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn remove_chunk(&mut self, hash: &str) -> Option<Chunk> {
        if let Some(chunk) = self.chunks.remove(hash) {
            self.chunk_by_index.remove(&chunk.index);
            debug!("Removed chunk: {}", hash);
            Some(chunk)
        } else {
            None
        }
    }

    pub fn get_chunks_ordered(&self, chunk_hashes: &[String]) -> Result<Vec<Vec<u8>>> {
        let mut chunks = Vec::new();

        for chunk_hash in chunk_hashes {
            if let Some(chunk) = self.get_chunk(chunk_hash) {
                chunks.push(chunk.data.clone());
            } else {
                return Err(P2PError::ChunkNotFound(chunk_hash.clone()));
            }
        }

        Ok(chunks)
    }
}
