use log::{debug, warn};
use std::collections::VecDeque;

use super::contact::Contact;
use super::node_id::NodeId;

const K: usize = 20; // K-bucket size (Kademlia parameter)
const BUCKET_COUNT: usize = 160; // 160 bits in NodeId

/// A K-bucket stores up to K contacts
#[derive(Clone)]
struct KBucket {
    contacts: VecDeque<Contact>,
}

impl KBucket {
    fn new() -> Self {
        Self {
            contacts: VecDeque::new(),
        }
    }

    /// Add or update a contact in the bucket
    fn add_contact(&mut self, contact: Contact) -> bool {
        // Check if contact already exists
        if let Some(pos) = self
            .contacts
            .iter()
            .position(|c| c.node_id == contact.node_id)
        {
            // Move to back (most recently seen)
            let mut existing = self.contacts.remove(pos).unwrap();
            existing.update_last_seen();
            self.contacts.push_back(existing);
            return true;
        }

        // If bucket is not full, add to back
        if self.contacts.len() < K {
            self.contacts.push_back(contact);
            return true;
        }

        // Bucket is full - in real Kademlia, we'd ping the oldest contact
        // For simplicity, we'll just reject the new contact
        warn!("K-bucket full, rejecting new contact");
        false
    }

    /// Remove a contact from the bucket
    fn remove_contact(&mut self, node_id: &NodeId) -> bool {
        if let Some(pos) = self.contacts.iter().position(|c| &c.node_id == node_id) {
            self.contacts.remove(pos);
            return true;
        }
        false
    }

    /// Get all contacts in the bucket
    fn get_contacts(&self) -> Vec<Contact> {
        self.contacts.iter().cloned().collect()
    }
}

/// Routing table for DHT
pub struct RoutingTable {
    local_id: NodeId,
    buckets: Vec<KBucket>,
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new(local_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(BUCKET_COUNT);
        for _ in 0..BUCKET_COUNT {
            buckets.push(KBucket::new());
        }

        Self { local_id, buckets }
    }

    /// Add or update a contact
    pub fn add_contact(&mut self, contact: Contact) -> bool {
        // Don't add ourselves
        if contact.node_id == self.local_id {
            return false;
        }

        if let Some(bucket_idx) = self.local_id.bucket_index(&contact.node_id) {
            debug!("Adding contact to bucket {}: {:?}", bucket_idx, contact);
            self.buckets[bucket_idx].add_contact(contact)
        } else {
            // Same ID as us, ignore
            false
        }
    }

    /// Remove a contact
    pub fn remove_contact(&mut self, node_id: &NodeId) -> bool {
        if let Some(bucket_idx) = self.local_id.bucket_index(node_id) {
            self.buckets[bucket_idx].remove_contact(node_id)
        } else {
            false
        }
    }

    /// Find the K closest contacts to a target ID
    pub fn find_closest_contacts(&self, target: &NodeId, count: usize) -> Vec<Contact> {
        let mut all_contacts: Vec<Contact> = self
            .buckets
            .iter()
            .flat_map(|bucket| bucket.get_contacts())
            .collect();

        // Sort by distance to target
        all_contacts.sort_by(|a, b| {
            let dist_a = a.node_id.distance(target);
            let dist_b = b.node_id.distance(target);
            dist_a.cmp(&dist_b)
        });

        // Return up to 'count' closest contacts
        all_contacts.into_iter().take(count).collect()
    }

    /// Get the total number of contacts
    pub fn contact_count(&self) -> usize {
        self.buckets.iter().map(|b| b.contacts.len()).sum()
    }

    /// Get all contacts
    pub fn get_all_contacts(&self) -> Vec<Contact> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.get_contacts())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn test_routing_table() {
        let local_id = NodeId::random();
        let mut table = RoutingTable::new(local_id);

        // Add some contacts
        for i in 0..5 {
            let node_id = NodeId::random();
            let addr: SocketAddr = format!("127.0.0.1:{}", 8080 + i).parse().unwrap();
            let contact = Contact::new(node_id, addr);
            table.add_contact(contact);
        }

        assert_eq!(table.contact_count(), 5);
    }

    #[test]
    fn test_find_closest() {
        let local_id = NodeId::random();
        let mut table = RoutingTable::new(local_id);

        // Add contacts
        let mut contacts = Vec::new();
        for i in 0..10 {
            let node_id = NodeId::random();
            let addr: SocketAddr = format!("127.0.0.1:{}", 8080 + i).parse().unwrap();
            let contact = Contact::new(node_id, addr);
            contacts.push(contact.clone());
            table.add_contact(contact);
        }

        // Find closest to a target
        let target = NodeId::random();
        let closest = table.find_closest_contacts(&target, 3);
        assert_eq!(closest.len(), 3);
    }
}
