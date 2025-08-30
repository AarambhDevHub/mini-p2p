use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::core::{Message, MessageType, PeerManager};
use crate::utils::{P2PError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub from: Uuid,
    pub to: Option<Uuid>, // None for broadcast
    pub message: Message,
}

pub struct MessageHandler {
    message_queue: Arc<RwLock<Vec<MessageEnvelope>>>,
    handlers: Vec<(MessageType, Box<dyn MessageProcessor + Send + Sync>)>, // ✅ Using Vec instead
    peer_manager: Arc<RwLock<PeerManager>>,
}

#[async_trait::async_trait]
pub trait MessageProcessor {
    async fn process(&self, envelope: MessageEnvelope) -> Result<Option<Message>>;
}

impl MessageHandler {
    pub fn new(peer_manager: Arc<RwLock<PeerManager>>) -> Self {
        Self {
            message_queue: Arc::new(RwLock::new(Vec::new())),
            handlers: Vec::new(),
            peer_manager,
        }
    }

    pub fn register_handler<T>(&mut self, msg_type: MessageType, handler: T)
    where
        T: MessageProcessor + Send + Sync + 'static,
    {
        self.handlers.push((msg_type, Box::new(handler))); // ✅ Push to Vec
    }

    pub async fn handle_message(&self, envelope: MessageEnvelope) -> Result<()> {
        debug!(
            "Handling message: {:?} from {}",
            envelope.message.msg_type, envelope.from
        );

        // Add to queue for processing
        {
            let mut queue = self.message_queue.write().await;
            queue.push(envelope.clone());
        }

        // Process immediately if handler exists
        for (msg_type, handler) in &self.handlers {
            // ✅ Iterate through Vec
            if *msg_type == envelope.message.msg_type {
                match handler.process(envelope.clone()).await {
                    Ok(Some(response)) => {
                        self.send_response(envelope.from, response).await?;
                    }
                    Ok(None) => {
                        // No response needed
                    }
                    Err(e) => {
                        error!("Message processing failed: {}", e);
                    }
                }
                break; // Found handler, stop searching
            }
        }

        Ok(())
    }

    // Rest of the methods remain the same...
    pub async fn send_message(&self, to: Uuid, message: Message) -> Result<()> {
        let pm = self.peer_manager.read().await;
        if let Some(peer) = pm.get_peer(&to) {
            peer.send_message(message).await?;
        } else {
            return Err(P2PError::InvalidResponse("Peer not found".to_string()));
        }
        Ok(())
    }

    pub async fn broadcast_message(&self, message: Message) -> Result<()> {
        let pm = self.peer_manager.read().await;
        let peers = pm.get_all_peers();

        for peer in peers {
            if let Err(e) = peer.send_message(message.clone()).await {
                warn!("Failed to send broadcast to {}: {}", peer.addr, e);
            }
        }

        Ok(())
    }

    async fn send_response(&self, to: Uuid, response: Message) -> Result<()> {
        self.send_message(to, response).await
    }

    pub async fn process_queue(&self) -> Result<()> {
        let mut queue = self.message_queue.write().await;
        let messages = queue.drain(..).collect::<Vec<_>>();
        drop(queue);

        for envelope in messages {
            if let Err(e) = self.handle_message(envelope).await {
                error!("Failed to process queued message: {}", e);
            }
        }

        Ok(())
    }
}

// Default message processors remain the same...
pub struct PingProcessor;

#[async_trait::async_trait]
impl MessageProcessor for PingProcessor {
    async fn process(&self, envelope: MessageEnvelope) -> Result<Option<Message>> {
        if envelope.message.msg_type == MessageType::Ping {
            Ok(Some(Message::pong()))
        } else {
            Ok(None)
        }
    }
}

pub struct HandshakeProcessor {
    node_id: Uuid,
    node_name: String,
}

impl HandshakeProcessor {
    pub fn new(node_id: Uuid, node_name: String) -> Self {
        Self { node_id, node_name }
    }
}

#[async_trait::async_trait]
impl MessageProcessor for HandshakeProcessor {
    async fn process(&self, envelope: MessageEnvelope) -> Result<Option<Message>> {
        if envelope.message.msg_type == MessageType::Handshake {
            Ok(Some(Message::handshake(
                self.node_id,
                self.node_name.clone(),
            )))
        } else {
            Ok(None)
        }
    }
}
