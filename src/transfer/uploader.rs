use log::{debug, info};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::core::{Message, MessageType, PeerManager};
use crate::storage::FileManager;
use crate::utils::Result;

pub struct Uploader {
    file_manager: Arc<Mutex<FileManager>>, // ✅ async-safe
}

impl Uploader {
    pub fn new(file_manager: Arc<Mutex<FileManager>>) -> Self {
        Self { file_manager }
    }

    pub async fn handle_request(
        &self,
        peer_id: &Uuid,
        message: Message,
        peer_manager: &Arc<RwLock<PeerManager>>,
    ) -> Result<()> {
        match message.msg_type {
            MessageType::Ping => {
                self.handle_ping(peer_id, peer_manager).await?;
            }
            MessageType::FileListRequest => {
                self.handle_file_list_request(peer_id, peer_manager).await?;
            }
            MessageType::FileInfoRequest => {
                self.handle_file_info_request(peer_id, message, peer_manager)
                    .await?;
            }
            MessageType::ChunkRequest => {
                self.handle_chunk_request(peer_id, message, peer_manager)
                    .await?;
            }
            MessageType::Handshake => {
                self.handle_handshake(peer_id, message, peer_manager)
                    .await?;
            }
            _ => {
                debug!(
                    "Unhandled message type from {}: {:?}",
                    peer_id, message.msg_type
                );
            }
        }
        Ok(())
    }

    async fn handle_ping(
        &self,
        peer_id: &Uuid,
        peer_manager: &Arc<RwLock<PeerManager>>,
    ) -> Result<()> {
        let pm = peer_manager.read().await;
        if let Some(peer) = pm.get_peer(peer_id) {
            let pong = Message::pong();
            peer.send_message(pong).await?;
        }
        Ok(())
    }

    async fn handle_file_list_request(
        &self,
        peer_id: &Uuid,
        peer_manager: &Arc<RwLock<PeerManager>>,
    ) -> Result<()> {
        let fm = self.file_manager.lock().await; // ✅ lock before using
        let files = fm.list_files();
        let file_infos: Vec<_> = files.into_iter().cloned().collect();

        let pm = peer_manager.read().await;
        if let Some(peer) = pm.get_peer(peer_id) {
            let response = Message::file_list_response(file_infos);
            peer.send_message(response).await?;
        }
        Ok(())
    }

    async fn handle_file_info_request(
        &self,
        peer_id: &Uuid,
        message: Message,
        peer_manager: &Arc<RwLock<PeerManager>>,
    ) -> Result<()> {
        if let Some(data) = message.data {
            if let Some(file_hash) = data.get("file_hash").and_then(|v| v.as_str()) {
                let fm = self.file_manager.lock().await; // ✅
                if let Some(file_info) = fm.get_file_info(file_hash) {
                    let pm = peer_manager.read().await;
                    if let Some(peer) = pm.get_peer(peer_id) {
                        let response = Message::file_info_response(file_info.clone());
                        peer.send_message(response).await?;
                    }
                } else {
                    let pm = peer_manager.read().await;
                    if let Some(peer) = pm.get_peer(peer_id) {
                        let error_msg = Message::error(format!("File not found: {}", file_hash));
                        peer.send_message(error_msg).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_chunk_request(
        &self,
        peer_id: &Uuid,
        message: Message,
        peer_manager: &Arc<RwLock<PeerManager>>,
    ) -> Result<()> {
        if let Some(data) = message.data {
            if let Some(chunk_hash) = data.get("chunk_hash").and_then(|v| v.as_str()) {
                let fm = self.file_manager.lock().await; // ✅
                if let Some(chunk) = fm.get_chunk(chunk_hash) {
                    let pm = peer_manager.read().await;
                    if let Some(peer) = pm.get_peer(peer_id) {
                        let response =
                            Message::chunk_response(chunk_hash.to_string(), chunk.data.clone());
                        peer.send_message(response).await?;
                        debug!("Sent chunk {} to peer {}", chunk_hash, peer_id);
                    }
                } else {
                    let pm = peer_manager.read().await;
                    if let Some(peer) = pm.get_peer(peer_id) {
                        let error_msg = Message::error(format!("Chunk not found: {}", chunk_hash));
                        peer.send_message(error_msg).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_handshake(
        &self,
        peer_id: &Uuid,
        message: Message,
        _peer_manager: &Arc<RwLock<PeerManager>>,
    ) -> Result<()> {
        if let Some(data) = message.data {
            let remote_node_id = data.get("node_id").and_then(|v| v.as_str());
            let remote_node_name = data.get("node_name").and_then(|v| v.as_str());

            info!(
                "Handshake from peer {}: {:?} ({})",
                peer_id,
                remote_node_name,
                remote_node_id.unwrap_or("unknown")
            );
        }
        Ok(())
    }
}
