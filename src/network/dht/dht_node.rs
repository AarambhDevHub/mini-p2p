use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};

use crate::utils::{P2PError, Result};

use super::contact::Contact;
use super::node_id::NodeId;
use super::routing_table::RoutingTable;
use super::rpc::DhtMessage;

const K: usize = 20; // Number of contacts to return

/// DHT Node for distributed peer discovery
pub struct DhtNode {
    node_id: NodeId,
    local_contact: Contact,
    routing_table: Arc<RwLock<RoutingTable>>,
    storage: Arc<RwLock<HashMap<String, Vec<Contact>>>>, // key -> peers
    socket: Arc<UdpSocket>,
}

impl DhtNode {
    /// Create a new DHT node
    pub async fn new(port: u16) -> Result<Self> {
        let node_id = NodeId::random();
        let addr = format!("0.0.0.0:{}", port);
        let socket = UdpSocket::bind(&addr).await?;
        let local_addr = socket.local_addr()?;

        info!("DHT node started on {} with ID: {}", local_addr, node_id);

        let local_contact = Contact::new(node_id, local_addr);
        let routing_table = Arc::new(RwLock::new(RoutingTable::new(node_id)));
        let storage = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            node_id,
            local_contact,
            routing_table,
            storage,
            socket: Arc::new(socket),
        })
    }

    /// Start the DHT service
    pub async fn start(self: Arc<Self>) {
        let dht = self.clone();
        tokio::spawn(async move {
            dht.listen_loop().await;
        });

        // Start periodic maintenance
        let dht = self.clone();
        tokio::spawn(async move {
            dht.maintenance_loop().await;
        });
    }

    /// Main listen loop for incoming messages
    async fn listen_loop(&self) {
        let mut buffer = vec![0u8; 65536];
        info!("DHT listening for messages...");

        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, from)) => {
                    let data = &buffer[..len];
                    if let Err(e) = self.handle_message(data, from).await {
                        warn!("Error handling DHT message from {}: {}", from, e);
                    }
                }
                Err(e) => {
                    error!("DHT socket error: {}", e);
                }
            }
        }
    }

    /// Handle incoming DHT message
    async fn handle_message(&self, data: &[u8], from: SocketAddr) -> Result<()> {
        let message: DhtMessage = serde_json::from_slice(data)
            .map_err(|e| P2PError::SerializationError(e.to_string()))?;

        debug!("Received DHT message from {}: {:?}", from, message);

        // Update routing table with sender
        let sender = message.sender().clone();
        self.routing_table.write().await.add_contact(sender.clone());

        match message {
            DhtMessage::Ping { sender } => {
                self.handle_ping(sender, from).await?;
            }
            DhtMessage::FindNode { sender, target } => {
                self.handle_find_node(sender, target, from).await?;
            }
            DhtMessage::FindValue { sender, key } => {
                self.handle_find_value(sender, key, from).await?;
            }
            DhtMessage::Store { sender, key } => {
                self.handle_store(sender, key, from).await?;
            }
            _ => {
                // Responses are handled by the caller
                debug!("Received response message");
            }
        }

        Ok(())
    }

    /// Handle ping request
    async fn handle_ping(&self, _sender: Contact, from: SocketAddr) -> Result<()> {
        let pong = DhtMessage::Pong {
            sender: self.local_contact.clone(),
        };
        self.send_message(&pong, from).await
    }

    /// Handle find_node request
    async fn handle_find_node(
        &self,
        _sender: Contact,
        target: NodeId,
        from: SocketAddr,
    ) -> Result<()> {
        let contacts = self
            .routing_table
            .read()
            .await
            .find_closest_contacts(&target, K);

        let response = DhtMessage::FindNodeResponse {
            sender: self.local_contact.clone(),
            contacts,
        };

        self.send_message(&response, from).await
    }

    /// Handle find_value request
    async fn handle_find_value(
        &self,
        _sender: Contact,
        key: String,
        from: SocketAddr,
    ) -> Result<()> {
        let storage = self.storage.read().await;

        let response = if let Some(peers) = storage.get(&key) {
            // We have the value
            DhtMessage::FindValueResponse {
                sender: self.local_contact.clone(),
                value: Some(peers.clone()),
                contacts: Vec::new(),
            }
        } else {
            // We don't have it, return closest nodes
            drop(storage);
            let target = NodeId::from_string(&key);
            let contacts = self
                .routing_table
                .read()
                .await
                .find_closest_contacts(&target, K);

            DhtMessage::FindValueResponse {
                sender: self.local_contact.clone(),
                value: None,
                contacts,
            }
        };

        self.send_message(&response, from).await
    }

    /// Handle store request
    async fn handle_store(&self, sender: Contact, key: String, from: SocketAddr) -> Result<()> {
        let mut storage = self.storage.write().await;

        storage
            .entry(key.clone())
            .or_insert_with(Vec::new)
            .push(sender.clone());

        debug!("Stored key {} from {}", key, sender.node_id);

        let response = DhtMessage::StoreResponse {
            sender: self.local_contact.clone(),
            success: true,
        };

        self.send_message(&response, from).await
    }

    /// Send a message to a peer
    async fn send_message(&self, message: &DhtMessage, to: SocketAddr) -> Result<()> {
        let data =
            serde_json::to_vec(message).map_err(|e| P2PError::SerializationError(e.to_string()))?;

        self.socket.send_to(&data, to).await?;
        Ok(())
    }

    /// Bootstrap by connecting to a known peer
    pub async fn bootstrap(&self, bootstrap_addr: SocketAddr) -> Result<()> {
        info!("Bootstrapping DHT via {}", bootstrap_addr);

        // Send a find_node for our own ID to populate routing table
        let find_node = DhtMessage::FindNode {
            sender: self.local_contact.clone(),
            target: self.node_id,
        };

        self.send_message(&find_node, bootstrap_addr).await?;
        Ok(())
    }

    /// Store a key-value pair (announce we have a file)
    pub async fn store(&self, key: String) -> Result<()> {
        let target = NodeId::from_string(&key);
        let closest = self
            .routing_table
            .read()
            .await
            .find_closest_contacts(&target, K);

        for contact in closest {
            let store_msg = DhtMessage::Store {
                sender: self.local_contact.clone(),
                key: key.clone(),
            };

            if let Err(e) = self.send_message(&store_msg, contact.addr).await {
                warn!("Failed to store on {}: {}", contact.addr, e);
            }
        }

        Ok(())
    }

    /// Find value (find peers that have a file)
    pub async fn find_value(&self, key: String) -> Result<Vec<Contact>> {
        let target = NodeId::from_string(&key);
        let closest = self
            .routing_table
            .read()
            .await
            .find_closest_contacts(&target, K);

        for contact in closest {
            let find_value = DhtMessage::FindValue {
                sender: self.local_contact.clone(),
                key: key.clone(),
            };

            if let Err(e) = self.send_message(&find_value, contact.addr).await {
                warn!("Failed to query {}: {}", contact.addr, e);
            }
        }

        // In a real implementation, we'd wait for responses
        // For now, return empty
        Ok(Vec::new())
    }

    /// Periodic maintenance
    async fn maintenance_loop(&self) {
        let mut interval = interval(Duration::from_secs(60));

        loop {
            interval.tick().await;
            debug!(
                "DHT maintenance: {} contacts",
                self.routing_table.read().await.contact_count()
            );
        }
    }

    /// Get node ID
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get contact count
    pub async fn contact_count(&self) -> usize {
        self.routing_table.read().await.contact_count()
    }
}
