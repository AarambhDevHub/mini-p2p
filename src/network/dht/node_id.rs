use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// 160-bit node identifier for DHT
/// Uses the first 160 bits of SHA-256 for compatibility
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 20]);

impl NodeId {
    /// Create a new NodeId from raw bytes
    pub fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Generate a random NodeId
    pub fn random() -> Self {
        let random_bytes: [u8; 20] = rand::random();
        Self(random_bytes)
    }

    /// Create a NodeId from a string (hash the string)
    pub fn from_string(s: &str) -> Self {
        let hash = Sha256::digest(s.as_bytes());
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&hash[..20]);
        Self(bytes)
    }

    /// Calculate XOR distance between two NodeIds
    /// This is the core metric in Kademlia
    pub fn distance(&self, other: &NodeId) -> NodeId {
        let mut result = [0u8; 20];
        for i in 0..20 {
            result[i] = self.0[i] ^ other.0[i];
        }
        NodeId(result)
    }

    /// Get the bucket index for this distance
    /// Returns the position of the first differing bit (0-159)
    pub fn bucket_index(&self, other: &NodeId) -> Option<usize> {
        let distance = self.distance(other);

        // Find the first non-zero byte
        for (byte_idx, &byte) in distance.0.iter().enumerate() {
            if byte != 0 {
                // Find the first set bit in this byte
                let bit_idx = 7 - byte.leading_zeros() as usize;
                return Some(byte_idx * 8 + bit_idx);
            }
        }

        None // Same ID
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Create from hex string
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 20 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut array = [0u8; 20];
        array.copy_from_slice(&bytes);
        Ok(Self(array))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({}...)", &self.to_hex()[..8])
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.to_hex()[..8])
    }
}

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance() {
        let id1 = NodeId::from_string("node1");
        let id2 = NodeId::from_string("node2");
        let dist = id1.distance(&id2);

        // Distance should be symmetric
        assert_eq!(dist, id2.distance(&id1));

        // Distance to self should be zero
        let zero_dist = id1.distance(&id1);
        assert_eq!(zero_dist.0, [0u8; 20]);
    }

    #[test]
    fn test_bucket_index() {
        let id1 = NodeId::new([0xFF; 20]);
        let id2 = NodeId::new([0x7F; 20]);

        // First bit differs, so bucket index should be 159
        assert_eq!(id1.bucket_index(&id2), Some(159));
    }

    #[test]
    fn test_hex_conversion() {
        let id = NodeId::random();
        let hex = id.to_hex();
        let id2 = NodeId::from_hex(&hex).unwrap();
        assert_eq!(id, id2);
    }
}
