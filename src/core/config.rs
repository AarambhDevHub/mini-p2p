use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub shared_dir: PathBuf,
    pub bootstrap_peer: Option<String>,
    pub node_name: String,
    pub discovery_port: Option<u16>, // ✅ Add discovery port option
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            shared_dir: PathBuf::from("./shared"),
            bootstrap_peer: None,
            node_name: "DefaultNode".to_string(),
            discovery_port: None, // ✅ None means no discovery listener
        }
    }
}
