//! Basic usage example for Mini P2P File Sharing

use mini_p2p::{Config, Node};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> mini_p2p::Result<()> {
    env_logger::init();

    println!("Mini P2P File Sharing - Basic Usage Example");

    // Create configuration
    let config = Config {
        port: 8080,
        shared_dir: PathBuf::from("./shared"),
        bootstrap_peer: None,
        node_name: "ExampleNode".to_string(),
        discovery_port: Some(9999), // ✅ Added discovery_port
    };

    // Create and start node
    let mut node = Node::new(config).await?;

    println!("Node created successfully!");
    println!("Configuration:");
    println!("  - Port: 8080");
    println!("  - Shared directory: ./shared");
    println!("  - Node name: ExampleNode");
    println!("  - Discovery port: 9999");

    // In a real application, you would call node.start().await?
    // For this example, we'll just demonstrate the setup

    println!("\nTo actually start the node, uncomment the following line:");
    println!("// node.start().await?;");

    println!("\nExample usage:");
    println!("1. Start first peer:  cargo run -- start --port 8080 --dir ./shared --discovery");
    println!("2. Start second peer: cargo run -- start --port 8081 --bootstrap 127.0.0.1:8080");
    println!("3. List files:        cargo run -- list --peer 127.0.0.1:8080");
    println!(
        "4. Download file:     cargo run -- download --hash <hash> --output file.txt --peer 127.0.0.1:8080"
    );

    Ok(())
}
