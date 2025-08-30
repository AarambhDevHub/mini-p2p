use log::{debug, info};
use std::path::PathBuf;
use tokio::fs as async_fs;
use tokio::io::AsyncWriteExt;

use crate::utils::{P2PError, Result};

pub struct StorageEngine {
    base_dir: PathBuf,
}

impl StorageEngine {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub async fn init(&self) -> Result<()> {
        async_fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to create storage directory: {}", e)))?;

        // Create subdirectories
        let chunks_dir = self.base_dir.join("chunks");
        let temp_dir = self.base_dir.join("temp");

        async_fs::create_dir_all(&chunks_dir)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to create chunks directory: {}", e)))?;
        async_fs::create_dir_all(&temp_dir)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to create temp directory: {}", e)))?;

        info!("Storage engine initialized at: {:?}", self.base_dir);
        Ok(())
    }

    pub async fn store_chunk(&self, chunk_hash: &str, data: &[u8]) -> Result<()> {
        let chunk_path = self
            .base_dir
            .join("chunks")
            .join(format!("{}.chunk", chunk_hash));

        let mut file = async_fs::File::create(&chunk_path)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to create chunk file: {}", e)))?;

        file.write_all(data)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to write chunk: {}", e)))?;

        debug!("Stored chunk: {}", chunk_hash);
        Ok(())
    }

    pub async fn load_chunk(&self, chunk_hash: &str) -> Result<Vec<u8>> {
        let chunk_path = self
            .base_dir
            .join("chunks")
            .join(format!("{}.chunk", chunk_hash));

        if !chunk_path.exists() {
            return Err(P2PError::ChunkNotFound(chunk_hash.to_string()));
        }

        let data = async_fs::read(&chunk_path)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to read chunk: {}", e)))?;

        debug!("Loaded chunk: {} ({} bytes)", chunk_hash, data.len());
        Ok(data)
    }

    pub async fn chunk_exists(&self, chunk_hash: &str) -> bool {
        let chunk_path = self
            .base_dir
            .join("chunks")
            .join(format!("{}.chunk", chunk_hash));
        chunk_path.exists()
    }

    pub async fn cleanup_temp_files(&self) -> Result<()> {
        let temp_dir = self.base_dir.join("temp");

        if temp_dir.exists() {
            async_fs::remove_dir_all(&temp_dir).await.map_err(|e| {
                P2PError::IoError(format!("Failed to cleanup temp directory: {}", e))
            })?;
            async_fs::create_dir_all(&temp_dir).await.map_err(|e| {
                P2PError::IoError(format!("Failed to recreate temp directory: {}", e))
            })?;
        }

        info!("Cleaned up temporary files");
        Ok(())
    }
}
