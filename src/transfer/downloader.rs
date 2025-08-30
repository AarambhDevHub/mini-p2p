use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, sleep};

use crate::core::PeerManager;
use crate::storage::{FileInfo, FileManager};
use crate::transfer::ChunkScheduler;
use crate::utils::{P2PError, Result};

#[derive(Clone)]
pub struct DownloadProgress {
    pub total_chunks: usize,
    pub completed_chunks: usize,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub peers_count: usize,
}

pub struct Downloader {
    file_manager: Arc<Mutex<FileManager>>,
    peer_manager: Arc<RwLock<PeerManager>>,
    scheduler: ChunkScheduler,
    active_downloads: Arc<RwLock<HashMap<String, DownloadProgress>>>,
}

impl Downloader {
    pub fn new(
        file_manager: Arc<Mutex<FileManager>>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Self {
        Self {
            file_manager,
            peer_manager,
            scheduler: ChunkScheduler::new(),
            active_downloads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn download_file(&self, file_hash: &str, output_path: &Path) -> Result<()> {
        info!("Starting download: {} -> {:?}", file_hash, output_path);

        // Get file info from a peer
        let file_info = self.request_file_info(file_hash).await?;
        info!(
            "File info received: {} chunks, {} bytes",
            file_info.chunk_count, file_info.size
        );

        // Initialize download progress
        {
            let mut downloads = self.active_downloads.write().await;
            downloads.insert(
                file_hash.to_string(),
                DownloadProgress {
                    total_chunks: file_info.chunk_count,
                    completed_chunks: 0,
                    bytes_downloaded: 0,
                    total_bytes: file_info.size,
                    peers_count: 0,
                },
            );
        }

        // Download chunks
        let chunks = self.download_chunks(&file_info).await?;
        info!("All chunks downloaded, reconstructing file...");

        // Reconstruct file
        {
            let fm = self.file_manager.lock().await;
            fm.reconstruct_file(chunks, output_path).await?;
        }

        // Clean up download progress
        {
            let mut downloads = self.active_downloads.write().await;
            downloads.remove(file_hash);
        }

        info!("Download completed successfully: {}", file_hash);
        Ok(())
    }

    async fn request_file_info(&self, file_hash: &str) -> Result<FileInfo> {
        let pm = self.peer_manager.read().await;
        let peers = pm.get_all_peers();

        if peers.is_empty() {
            return Err(P2PError::NoPeersAvailable);
        }

        for peer in &peers {
            match peer.request_file_info(file_hash).await {
                Ok(file_info) => {
                    debug!("Got file info from peer {}", peer.addr);
                    return Ok(file_info);
                }
                Err(e) => {
                    warn!("Failed to get file info from peer {}: {}", peer.addr, e);
                }
            }
        }

        Err(P2PError::FileNotFound(file_hash.to_string()))
    }

    async fn download_chunks(&self, file_info: &FileInfo) -> Result<Vec<Vec<u8>>> {
        let chunk_count = file_info.chunk_hashes.len();
        let mut completed_chunks = HashMap::new();
        let mut pending_chunks: HashSet<usize> = (0..chunk_count).collect();

        info!("Downloading {} chunks...", chunk_count);

        while !pending_chunks.is_empty() {
            let pm = self.peer_manager.read().await;
            let peers = pm.get_all_peers();

            if peers.is_empty() {
                warn!("No peers available for download, retrying...");
                drop(pm);
                sleep(Duration::from_secs(2)).await;
                continue;
            }

            info!("Using {} peers for download", peers.len());

            // Update progress
            {
                let mut downloads = self.active_downloads.write().await;
                if let Some(progress) = downloads.get_mut(&file_info.hash) {
                    progress.completed_chunks = completed_chunks.len();
                    progress.peers_count = peers.len();
                }
            }

            // Download chunks in parallel
            let mut tasks = Vec::new();
            let chunk_iter: Vec<_> = pending_chunks.iter().take(5).cloned().collect(); // Max 5 concurrent

            for chunk_index in chunk_iter {
                if chunk_index >= file_info.chunk_hashes.len() {
                    continue;
                }

                let chunk_hash = file_info.chunk_hashes[chunk_index].clone();
                let peer = peers[chunk_index % peers.len()].clone();

                debug!("Requesting chunk {} from peer {}", chunk_index, peer.addr);

                let task = tokio::spawn(async move {
                    match peer.request_chunk(&chunk_hash).await {
                        Ok(data) => {
                            debug!(
                                "Successfully downloaded chunk {} ({} bytes)",
                                chunk_index,
                                data.len()
                            );
                            Some((chunk_index, data))
                        }
                        Err(e) => {
                            warn!(
                                "Failed to download chunk {} from {}: {}",
                                chunk_index, peer.addr, e
                            );
                            None
                        }
                    }
                });

                tasks.push(task);
            }

            drop(pm); // Release the lock before awaiting

            // Wait for chunk downloads
            let mut downloaded_any = false;
            for task in tasks {
                if let Ok(Some((chunk_index, data))) = task.await {
                    completed_chunks.insert(chunk_index, data);
                    pending_chunks.remove(&chunk_index);
                    downloaded_any = true;
                }
            }

            if !downloaded_any && !pending_chunks.is_empty() {
                warn!("No chunks downloaded in this round, waiting before retry...");
                sleep(Duration::from_secs(1)).await;
            }

            info!(
                "Progress: {}/{} chunks completed",
                completed_chunks.len(),
                chunk_count
            );
        }

        // Order chunks by index
        let mut ordered_chunks = Vec::new();
        for i in 0..chunk_count {
            if let Some(chunk_data) = completed_chunks.get(&i) {
                ordered_chunks.push(chunk_data.clone());
            } else {
                return Err(P2PError::IncompleteDownload(format!("Missing chunk {}", i)));
            }
        }

        info!("All chunks downloaded successfully");
        Ok(ordered_chunks)
    }

    pub async fn get_download_progress(&self, file_hash: &str) -> Option<DownloadProgress> {
        let downloads = self.active_downloads.read().await;
        downloads.get(file_hash).cloned()
    }
}
