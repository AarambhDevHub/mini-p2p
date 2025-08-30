use sha2::{Digest, Sha256};

pub struct HashUtils;

impl HashUtils {
    pub fn hash_data(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    pub fn hash_file_path(path: &std::path::Path) -> crate::utils::Result<String> {
        let content = std::fs::read(path)
            .map_err(|e| crate::utils::P2PError::IoError(format!("Failed to read file: {}", e)))?;
        Ok(Self::hash_data(&content))
    }

    pub fn verify_data(data: &[u8], expected_hash: &str) -> bool {
        let actual_hash = Self::hash_data(data);
        actual_hash == expected_hash
    }
}
