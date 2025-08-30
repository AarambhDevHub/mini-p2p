//! Reliable P2P file sharing demonstration with comprehensive error handling

use mini_p2p::{Config, Node, NodeUtils};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> mini_p2p::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("🚀 Reliable P2P File Sharing Demo");
    println!("=================================");

    // Cleanup previous runs
    cleanup_environment().await;

    // Find available ports
    let seed_port = NodeUtils::find_available_port(8080).await?;
    let download_port = NodeUtils::find_available_port(8081).await?;
    let discovery_port = NodeUtils::find_available_port(9999).await?;

    println!("📍 Using ports:");
    println!("  - Seed node: {}", seed_port);
    println!("  - Download node: {}", download_port);
    println!("  - Discovery: {}", discovery_port);

    // Setup demo files
    setup_demo_files().await?;

    // Start seed node
    let seed_config = Config {
        port: seed_port,
        shared_dir: PathBuf::from("./demo_shared"),
        bootstrap_peer: None,
        node_name: "ReliableSeed".to_string(),
        discovery_port: Some(discovery_port),
    };

    let bootstrap_addr = format!("127.0.0.1:{}", seed_port);
    let seed_handle = start_seed_node(seed_config).await?;

    // Wait for seed node to be ready
    if !NodeUtils::wait_for_port_ready(seed_port, Duration::from_secs(10)).await {
        eprintln!("❌ Seed node failed to start properly");
        return Ok(());
    }

    println!("✅ Seed node verified running on port {}", seed_port);

    // Start download process
    let download_result = perform_download_with_retry(bootstrap_addr, download_port).await;

    match download_result {
        Ok(_) => println!("🎉 Demo completed successfully!"),
        Err(e) => eprintln!("❌ Demo failed: {}", e),
    }

    // Cleanup
    seed_handle.abort();
    sleep(Duration::from_secs(2)).await;

    Ok(())
}

async fn start_seed_node(config: Config) -> mini_p2p::Result<tokio::task::JoinHandle<()>> {
    let handle = tokio::spawn(async move {
        println!("🌱 Starting seed node...");

        match Node::new(config).await {
            Ok(mut node) => {
                match node.start().await {
                    Ok(_) => {
                        println!("✅ Seed node running successfully");
                        // Keep running until aborted
                        loop {
                            sleep(Duration::from_secs(10)).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Seed node start error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to create seed node: {}", e);
            }
        }
    });

    // Give it time to start
    sleep(Duration::from_secs(3)).await;
    Ok(handle)
}

async fn perform_download_with_retry(
    bootstrap_addr: String,
    download_port: u16,
) -> mini_p2p::Result<()> {
    let download_config = Config {
        port: download_port,
        shared_dir: PathBuf::from("./demo_downloads"),
        bootstrap_peer: Some(bootstrap_addr),
        node_name: "ReliableDownloader".to_string(),
        discovery_port: None,
    };

    // ✅ Fixed: Direct download without problematic retry closure
    for attempt in 1..=3 {
        println!("📥 Download attempt {}", attempt);

        match perform_single_download(download_config.clone()).await {
            Ok(_) => {
                println!("✅ Download successful!");
                return Ok(());
            }
            Err(e) => {
                println!("⚠️  Download attempt {} failed: {}", attempt, e);
                if attempt < 3 {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(mini_p2p::P2PError::InvalidResponse(
        "All download attempts failed".to_string(),
    ))
}

async fn perform_single_download(config: Config) -> mini_p2p::Result<()> {
    let mut download_node = Node::new(config).await?;
    println!("📱 Download node created");

    // Give time for connection
    sleep(Duration::from_secs(3)).await;

    // List files
    let files = download_node.list_remote_files().await?;
    if files.is_empty() {
        return Err(mini_p2p::P2PError::InvalidResponse(
            "No files found".to_string(),
        ));
    }

    println!("📋 Found {} files:", files.len());
    for file in &files {
        println!("  - {} ({} bytes)", file.name, file.size);
    }

    // Download first file
    let first_file = &files[0];
    let output_path = PathBuf::from("./demo_downloads").join(&first_file.name);

    println!("📥 Downloading: {}", first_file.name);

    // ✅ Fixed: Direct call without problematic closure
    download_node
        .download_file(&first_file.hash, &output_path)
        .await?;

    // Verify download
    if output_path.exists() {
        let metadata = fs::metadata(&output_path).await?;
        println!("✅ Download verified: {} bytes", metadata.len());

        // Show content preview for text files
        if first_file.name.ends_with(".txt") || first_file.name.ends_with(".md") {
            if let Ok(content) = fs::read_to_string(&output_path).await {
                let preview = if content.len() > 100 {
                    format!("{}...", &content[..100])
                } else {
                    content
                };
                println!("📄 Content: {}", preview);
            }
        }
    } else {
        return Err(mini_p2p::P2PError::InvalidResponse(
            "Downloaded file not found".to_string(),
        ));
    }

    Ok(())
}

async fn cleanup_environment() {
    println!("🧹 Cleaning up environment...");

    // Remove old directories
    let _ = fs::remove_dir_all("./demo_shared").await;
    let _ = fs::remove_dir_all("./demo_downloads").await;

    sleep(Duration::from_secs(1)).await;
}

async fn setup_demo_files() -> mini_p2p::Result<()> {
    fs::create_dir_all("./demo_shared").await?;
    fs::create_dir_all("./demo_downloads").await?;

    let files = vec![
        (
            "hello.txt",
            "Hello, P2P World!\nThis is a demo file for sharing.",
        ),
        (
            "data.json",
            r#"{"message": "P2P data sharing", "version": "1.0", "chunks": true}"#,
        ),
        (
            "readme.md",
            "# P2P Demo\n\nThis file was shared via P2P network.\n\n## Features\n- Distributed\n- Chunked\n- Verified",
        ),
    ];

    for (filename, content) in files {
        fs::write(format!("./demo_shared/{}", filename), content).await?;
        println!("📄 Created: {}", filename);
    }

    // Create small binary file
    let binary_data = vec![0xAB; 1000];
    fs::write("./demo_shared/binary_data.bin", binary_data).await?;
    println!("📄 Created: binary_data.bin (1KB)");

    println!("📁 Demo files ready");
    Ok(())
}
