use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub msg_type: MessageType,
    pub timestamp: u64,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)] // ✅ Added Eq and Hash
pub enum MessageType {
    // Connection management
    Ping,
    Pong,
    Handshake,
    Disconnect,

    // File discovery and metadata
    FileListRequest,
    FileListResponse,
    FileInfoRequest,
    FileInfoResponse,

    // Data transfer
    ChunkRequest,
    ChunkResponse,
    ChunkAvailable,

    // Peer discovery
    PeerDiscovery,
    PeerAnnouncement,

    // Error handling
    Error,
}

// Rest of the implementation remains the same...
impl Message {
    pub fn new(msg_type: MessageType, data: Option<Value>) -> Self {
        Self {
            id: Uuid::new_v4(),
            msg_type,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            data,
        }
    }

    pub fn ping() -> Self {
        Self::new(MessageType::Ping, None)
    }

    pub fn pong() -> Self {
        Self::new(MessageType::Pong, None)
    }

    pub fn handshake(node_id: Uuid, node_name: String) -> Self {
        Self::new(
            MessageType::Handshake,
            Some(serde_json::json!({
                "node_id": node_id,
                "node_name": node_name,
                "protocol_version": "1.0"
            })),
        )
    }

    pub fn file_list_request() -> Self {
        Self::new(MessageType::FileListRequest, None)
    }

    pub fn file_list_response(files: Vec<crate::storage::FileInfo>) -> Self {
        Self::new(
            MessageType::FileListResponse,
            Some(serde_json::json!({
                "files": files
            })),
        )
    }

    pub fn file_info_request(file_hash: String) -> Self {
        Self::new(
            MessageType::FileInfoRequest,
            Some(serde_json::json!({
                "file_hash": file_hash
            })),
        )
    }

    pub fn file_info_response(file_info: crate::storage::FileInfo) -> Self {
        Self::new(
            MessageType::FileInfoResponse,
            Some(serde_json::to_value(file_info).unwrap()),
        )
    }

    pub fn chunk_request(chunk_hash: String) -> Self {
        Self::new(
            MessageType::ChunkRequest,
            Some(serde_json::json!({
                "chunk_hash": chunk_hash
            })),
        )
    }

    pub fn chunk_response(chunk_hash: String, data: Vec<u8>) -> Self {
        Self::new(
            MessageType::ChunkResponse,
            Some(serde_json::json!({
                "chunk_hash": chunk_hash,
                "data": hex::encode(data)
            })),
        )
    }

    pub fn error(message: String) -> Self {
        Self::new(
            MessageType::Error,
            Some(serde_json::json!({
                "error": message
            })),
        )
    }
}
