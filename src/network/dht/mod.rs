//! Distributed Hash Table (DHT) implementation
//!
//! This module implements a Kademlia-inspired DHT for decentralized
//! peer discovery and content routing.

mod contact;
mod dht_node;
mod node_id;
mod routing_table;
mod rpc;

pub use contact::Contact;
pub use dht_node::DhtNode;
pub use node_id::NodeId;
pub use rpc::DhtMessage;
