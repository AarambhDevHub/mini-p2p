use thiserror::Error;

pub type Result<T> = std::result::Result<T, P2PError>;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("I/O error: {0}")]
    IoError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Chunk not found: {0}")]
    ChunkNotFound(String),

    #[error("Chunk verification failed: {0}")]
    ChunkVerificationFailed(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Message too large: {0} bytes")]
    MessageTooLarge(usize),

    #[error("No peers available")]
    NoPeersAvailable,

    #[error("Too many peers: {0}")]
    TooManyPeers(usize),

    #[error("Download incomplete: {0}")]
    IncompleteDownload(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl From<std::io::Error> for P2PError {
    fn from(err: std::io::Error) -> Self {
        P2PError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for P2PError {
    fn from(err: serde_json::Error) -> Self {
        P2PError::SerializationError(err.to_string())
    }
}
