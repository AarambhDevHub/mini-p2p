use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::storage::{CHUNK_SIZE, Chunk, ChunkManager, HashUtils};
use crate::utils::{P2PError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub hash: String,
    pub name: String,
    pub size: u64,
    pub chunk_count: usize,
    pub chunk_hashes: Vec<String>,
    pub created_at: u64,
}

pub struct FileManager {
    shared_dir: PathBuf,
    files: HashMap<String, FileInfo>,
    chunk_manager: ChunkManager,
}

impl FileManager {
    pub async fn new(shared_dir: PathBuf) -> Result<Self> {
        async_fs::create_dir_all(&shared_dir)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to create directory: {}", e)))?;

        let mut manager = Self {
            shared_dir,
            files: HashMap::new(),
            chunk_manager: ChunkManager::new(),
        };

        manager.scan_files().await?;
        Ok(manager)
    }

    pub async fn scan_files(&mut self) -> Result<()> {
        info!("Scanning files in: {:?}", self.shared_dir);

        let mut entries = async_fs::read_dir(&self.shared_dir)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to read directory: {}", e)))?;

        let mut file_count = 0;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_file() {
                match self.process_file(&path).await {
                    Ok(_) => file_count += 1,
                    Err(e) => warn!("Failed to process file {:?}: {}", path, e),
                }
            }
        }

        info!("Scanned {} files successfully", file_count);
        Ok(())
    }

    async fn process_file(&mut self, path: &Path) -> Result<()> {
        let mut file = async_fs::File::open(path)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to open file: {}", e)))?;

        let metadata = file
            .metadata()
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to read metadata: {}", e)))?;
        let file_size = metadata.len();

        // Read file content
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to read file: {}", e)))?;

        // Calculate file hash
        let file_hash = HashUtils::hash_data(&content);

        // Create chunks
        let chunks = self.create_chunks(&content).await?;
        let chunk_hashes: Vec<String> = chunks.iter().map(|c| c.hash.clone()).collect();

        // Store chunks in chunk manager
        for chunk in chunks {
            self.chunk_manager.add_chunk(chunk)?;
        }

        let file_info = FileInfo {
            hash: file_hash.clone(),
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            size: file_size,
            chunk_count: chunk_hashes.len(),
            chunk_hashes,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.files.insert(file_hash.clone(), file_info);
        debug!("Processed file: {} -> {}", path.display(), file_hash);

        Ok(())
    }

    async fn create_chunks(&self, content: &[u8]) -> Result<Vec<Chunk>> {
        let mut chunks = Vec::new();
        let mut offset = 0;
        let mut chunk_index = 0;

        // ✅ Use dynamic chunk size based on file size
        let chunk_size = crate::storage::chunk::optimal_chunk_size(content.len());
        debug!(
            "Using chunk size {} for file of {} bytes",
            chunk_size,
            content.len()
        );

        while offset < content.len() {
            let end = std::cmp::min(offset + chunk_size, content.len());
            let chunk_data = content[offset..end].to_vec();

            let chunk = Chunk::new(chunk_index, chunk_data);
            chunks.push(chunk);

            offset = end;
            chunk_index += 1;
        }

        debug!("Created {} chunks with size {}", chunks.len(), chunk_size);
        Ok(chunks)
    }

    pub fn get_file_info(&self, hash: &str) -> Option<&FileInfo> {
        self.files.get(hash)
    }

    pub fn get_chunk(&self, hash: &str) -> Option<&Chunk> {
        self.chunk_manager.get_chunk(hash)
    }

    pub fn list_files(&self) -> Vec<&FileInfo> {
        self.files.values().collect()
    }

    pub async fn reconstruct_file(&self, chunks: Vec<Vec<u8>>, output_path: &Path) -> Result<()> {
        let mut file = async_fs::File::create(output_path)
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to create output file: {}", e)))?;

        for chunk_data in chunks {
            file.write_all(&chunk_data)
                .await
                .map_err(|e| P2PError::IoError(format!("Failed to write chunk: {}", e)))?;
        }

        file.flush()
            .await
            .map_err(|e| P2PError::IoError(format!("Failed to flush file: {}", e)))?;

        info!("File reconstructed: {:?}", output_path);
        Ok(())
    }

    pub fn get_chunk_count(&self) -> usize {
        self.chunk_manager.chunk_count()
    }

    pub fn get_file_count(&self) -> usize {
        self.files.len()
    }
}
