use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub shared_dir: PathBuf,
    pub bootstrap_peer: Option<String>,
    pub node_name: String,
    pub discovery_port: Option<u16>, // ✅ Add discovery port option
    pub max_upload_speed: Option<u64>,
    pub max_download_speed: Option<u64>,
    pub dht_enabled: bool,
    pub dht_port: Option<u16>,
    pub dht_bootstrap: Option<String>,
    /// Enable NAT traversal (UPnP/STUN)
    pub nat_traversal_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            shared_dir: PathBuf::from("./shared"),
            bootstrap_peer: None,
            node_name: "DefaultNode".to_string(),
            discovery_port: None, // ✅ None means no discovery listener
            max_upload_speed: None,
            max_download_speed: None,
            dht_enabled: false,
            dht_port: Some(6881),
            dht_bootstrap: None,
            nat_traversal_enabled: true,
        }
    }
}
