use serde::{Deserialize, Serialize};

use super::contact::Contact;
use super::node_id::NodeId;

/// DHT RPC messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtMessage {
    /// Ping - check if a node is alive
    Ping { sender: Contact },

    /// Pong - response to ping
    Pong { sender: Contact },

    /// FindNode - find K closest nodes to a target ID
    FindNode { sender: Contact, target: NodeId },

    /// FindNodeResponse - return K closest nodes
    FindNodeResponse {
        sender: Contact,
        contacts: Vec<Contact>,
    },

    /// FindValue - find peers that have a specific file
    FindValue {
        sender: Contact,
        key: String, // File hash
    },

    /// FindValueResponse - return peers or closest nodes
    FindValueResponse {
        sender: Contact,
        value: Option<Vec<Contact>>, // Some(peers) if found, None if not
        contacts: Vec<Contact>,      // Closest nodes if value not found
    },

    /// Store - announce that we have a file
    Store {
        sender: Contact,
        key: String, // File hash
    },

    /// StoreResponse - acknowledge storage
    StoreResponse { sender: Contact, success: bool },
}

impl DhtMessage {
    /// Get the sender contact from any message
    pub fn sender(&self) -> &Contact {
        match self {
            DhtMessage::Ping { sender } => sender,
            DhtMessage::Pong { sender } => sender,
            DhtMessage::FindNode { sender, .. } => sender,
            DhtMessage::FindNodeResponse { sender, .. } => sender,
            DhtMessage::FindValue { sender, .. } => sender,
            DhtMessage::FindValueResponse { sender, .. } => sender,
            DhtMessage::Store { sender, .. } => sender,
            DhtMessage::StoreResponse { sender, .. } => sender,
        }
    }

    /// Check if this is a request message (needs a response)
    pub fn is_request(&self) -> bool {
        matches!(
            self,
            DhtMessage::Ping { .. }
                | DhtMessage::FindNode { .. }
                | DhtMessage::FindValue { .. }
                | DhtMessage::Store { .. }
        )
    }

    /// Check if this is a response message
    pub fn is_response(&self) -> bool {
        !self.is_request()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn test_message_serialization() {
        let node_id = NodeId::random();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let contact = Contact::new(node_id, addr);

        let ping = DhtMessage::Ping {
            sender: contact.clone(),
        };

        // Serialize and deserialize
        let serialized = serde_json::to_vec(&ping).unwrap();
        let deserialized: DhtMessage = serde_json::from_slice(&serialized).unwrap();

        assert!(matches!(deserialized, DhtMessage::Ping { .. }));
    }

    #[test]
    fn test_is_request() {
        let node_id = NodeId::random();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let contact = Contact::new(node_id, addr);

        let ping = DhtMessage::Ping {
            sender: contact.clone(),
        };
        assert!(ping.is_request());

        let pong = DhtMessage::Pong {
            sender: contact.clone(),
        };
        assert!(pong.is_response());
    }
}
