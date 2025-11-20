//! Multi-node network simulation

use mini_p2p::{Config, Node};
use std::path::PathBuf;
use tokio::fs;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> mini_p2p::Result<()> {
    env_logger::init();

    println!("Starting P2P network simulation with 3 nodes...");

    // Create test directories and files
    setup_test_environment().await?;

    // Node configurations - Only the seed node runs discovery
    let nodes = vec![
        ("Seed-Node", 8080, "./shared1", None, Some(9999)), // ✅ Seed has discovery
        (
            "Peer-Node-1",
            8081,
            "./shared2",
            Some("127.0.0.1:8080".to_string()),
            None,
        ), // ✅ Peers don't
        (
            "Peer-Node-2",
            8082,
            "./shared3",
            Some("127.0.0.1:8080".to_string()),
            None,
        ), // ✅ Peers don't
    ];

    let mut handles = Vec::new();

    for (name, port, dir, bootstrap, discovery_port) in nodes {
        let config = Config {
            port,
            shared_dir: PathBuf::from(dir),
            bootstrap_peer: bootstrap,
            node_name: name.to_string(),
            discovery_port, // ✅ Added discovery_port field
            max_upload_speed: None,
            max_download_speed: None,
            dht_enabled: false,
            dht_port: Some(6881),
            dht_bootstrap: None,
            nat_traversal_enabled: false,
        };

        let handle = tokio::spawn(async move {
            println!("Starting {}", name);
            match Node::new(config).await {
                Ok(mut node) => {
                    if let Err(e) = node.start().await {
                        eprintln!("Node {} failed: {}", name, e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create node {}: {}", name, e);
                }
            }
        });

        handles.push(handle);
        sleep(Duration::from_secs(2)).await;
    }

    println!("All nodes started! Network simulation running...");
    println!("Press Ctrl+C to stop the simulation");

    // Wait for all nodes
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

async fn setup_test_environment() -> mini_p2p::Result<()> {
    // Create test directories
    for i in 1..=3 {
        let dir = format!("./shared{}", i);
        fs::create_dir_all(&dir).await?;

        // Create test files
        let test_content = format!(
            "Hello from node {}!\nThis is test content for P2P sharing.",
            i
        );
        fs::write(format!("{}/test_{}.txt", dir, i), test_content).await?;

        // Create a larger test file
        let large_content = "A".repeat(100_000); // 100KB file
        fs::write(format!("{}/large_file_{}.txt", dir, i), large_content).await?;
    }

    println!("Test environment setup complete");
    Ok(())
}
