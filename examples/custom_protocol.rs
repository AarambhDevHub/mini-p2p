//! Custom protocol extension example

use mini_p2p::{Config, Node};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomMessage {
    pub command: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

#[tokio::main]
async fn main() -> mini_p2p::Result<()> {
    env_logger::init();

    println!("Custom Protocol Extension Demo");
    println!("=============================");

    let config = Config {
        port: 8080,
        shared_dir: PathBuf::from("./custom_shared"),
        bootstrap_peer: None,
        node_name: "CustomProtocolNode".to_string(),
        discovery_port: Some(9999), // ✅ Added discovery_port
    };

    let mut node = Node::new(config).await?;

    println!("🔧 Node with custom protocol extensions created");
    println!("This example demonstrates how to extend the P2P protocol");
    println!("with custom message types and handlers.");

    // In a real implementation, you would:
    // 1. Extend the MessageType enum
    // 2. Add custom message handlers
    // 3. Implement custom peer discovery mechanisms
    // 4. Add application-specific data structures

    println!("\nCustom Protocol Features:");
    println!("- Extended message types");
    println!("- Custom peer authentication");
    println!("- Application-specific routing");
    println!("- Enhanced metadata exchange");

    Ok(())
}
