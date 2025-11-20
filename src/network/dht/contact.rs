use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Instant;

use super::node_id::NodeId;

/// Represents a contact (peer) in the DHT network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contact {
    pub node_id: NodeId,
    pub addr: SocketAddr,
    #[serde(skip)]
    pub last_seen: Option<Instant>,
}

impl Contact {
    /// Create a new contact
    pub fn new(node_id: NodeId, addr: SocketAddr) -> Self {
        Self {
            node_id,
            addr,
            last_seen: Some(Instant::now()),
        }
    }

    /// Update the last seen time
    pub fn update_last_seen(&mut self) {
        self.last_seen = Some(Instant::now());
    }

    /// Check if the contact is stale (not seen for a while)
    pub fn is_stale(&self, timeout_secs: u64) -> bool {
        if let Some(last_seen) = self.last_seen {
            last_seen.elapsed().as_secs() > timeout_secs
        } else {
            true
        }
    }
}

impl PartialEq for Contact {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id && self.addr == other.addr
    }
}

impl Eq for Contact {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_contact_creation() {
        let node_id = NodeId::random();
        let addr = "127.0.0.1:8080".parse().unwrap();
        let contact = Contact::new(node_id, addr);

        assert_eq!(contact.node_id, node_id);
        assert_eq!(contact.addr, addr);
        assert!(contact.last_seen.is_some());
    }

    #[test]
    fn test_staleness() {
        let node_id = NodeId::random();
        let addr = "127.0.0.1:8080".parse().unwrap();
        let mut contact = Contact::new(node_id, addr);

        // Should not be stale immediately
        assert!(!contact.is_stale(1));

        // Manually set last_seen to None
        contact.last_seen = None;
        assert!(contact.is_stale(0));
    }
}
