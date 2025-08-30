//! Complete file sharing demonstration

use mini_p2p::{Config, Node};
use std::path::PathBuf;
use tokio::fs;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> mini_p2p::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("P2P File Sharing Demo");
    println!("====================");

    // Setup demo environment
    setup_demo_files().await?;

    // Start seeder node - ONLY the seed node runs discovery service
    let seed_config = Config {
        port: 8080,
        shared_dir: PathBuf::from("./demo_shared"),
        bootstrap_peer: None,
        node_name: "SeedNode".to_string(),
        discovery_port: Some(9999), // ✅ Only seed node runs discovery
    };

    let seed_handle = tokio::spawn(async move {
        match Node::new(seed_config).await {
            Ok(mut seed_node) => {
                println!("🌱 Seed node started on port 8080 with discovery on 9999");
                if let Err(e) = seed_node.start().await {
                    eprintln!("❌ Seed node error: {}", e);
                }
            }
            Err(e) => eprintln!("❌ Failed to create seed node: {}", e),
        }
    });

    // Give seed node time to start
    sleep(Duration::from_secs(3)).await;

    // Start downloader node - NO discovery service, just connects to bootstrap
    let download_handle = tokio::spawn(async move {
        let download_config = Config {
            port: 8081,
            shared_dir: PathBuf::from("./demo_downloads"),
            bootstrap_peer: Some("127.0.0.1:8080".to_string()),
            node_name: "DownloaderNode".to_string(),
            discovery_port: None, // ✅ No discovery service for download node
        };

        match Node::new(download_config).await {
            Ok(mut download_node) => {
                println!("📥 Downloader node started on port 8081 (no discovery)");

                // Give time for connection to establish
                sleep(Duration::from_secs(3)).await;

                // List available files
                println!("🔍 Requesting file list from bootstrap peer...");
                match download_node.list_remote_files().await {
                    Ok(files) => {
                        if files.is_empty() {
                            println!("❌ No files found on network");
                            return;
                        }

                        println!("📋 Available files:");
                        for file in &files {
                            println!(
                                "  - {} ({} bytes) - Hash: {}",
                                file.name,
                                file.size,
                                &file.hash[..16]
                            );
                        }

                        // Download first file
                        if let Some(first_file) = files.first() {
                            let output_path =
                                PathBuf::from("./demo_downloads").join(&first_file.name);
                            println!(
                                "📥 Downloading: {} ({} bytes)",
                                first_file.name, first_file.size
                            );

                            match download_node
                                .download_file(&first_file.hash, &output_path)
                                .await
                            {
                                Ok(_) => {
                                    println!("✅ Download completed: {:?}", output_path);

                                    // Verify file exists and show size
                                    if output_path.exists() {
                                        match fs::metadata(&output_path).await {
                                            Ok(metadata) => {
                                                println!(
                                                    "📊 Downloaded file size: {} bytes",
                                                    metadata.len()
                                                );

                                                // Show first few bytes as verification
                                                if let Ok(content) =
                                                    fs::read_to_string(&output_path).await
                                                {
                                                    let preview = if content.len() > 100 {
                                                        format!("{}...", &content[..100])
                                                    } else {
                                                        content
                                                    };
                                                    println!(
                                                        "📄 File content preview: {}",
                                                        preview
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                println!("⚠️  Could not read file metadata: {}", e)
                                            }
                                        }
                                    } else {
                                        println!("❌ Downloaded file not found!");
                                    }
                                }
                                Err(e) => println!("❌ Download failed: {}", e),
                            }
                        }
                    }
                    Err(e) => println!("❌ Failed to list files: {}", e),
                }

                println!("🎯 Download process completed, keeping connection alive...");
                sleep(Duration::from_secs(5)).await;
            }
            Err(e) => eprintln!("❌ Failed to create download node: {}", e),
        }
    });

    println!("🚀 Demo running... Press Ctrl+C to stop");

    // Wait for completion or interruption
    tokio::select! {
        _ = download_handle => {
            println!("📥 Download completed successfully!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n⏹️  Demo interrupted by user");
        }
    }

    // Clean shutdown
    seed_handle.abort();
    println!("🛑 Demo stopped");
    Ok(())
}

async fn setup_demo_files() -> mini_p2p::Result<()> {
    // Clean up previous runs
    let _ = fs::remove_dir_all("./demo_shared").await;
    let _ = fs::remove_dir_all("./demo_downloads").await;

    fs::create_dir_all("./demo_shared").await?;
    fs::create_dir_all("./demo_downloads").await?;

    // Create demo files
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

    // Create a smaller binary file for faster demo
    let small_data = vec![0xAB; 1000]; // 1KB binary file
    fs::write("./demo_shared/binary_data.bin", small_data).await?;
    println!("📄 Created: binary_data.bin (1KB)");

    println!("📁 Demo files ready in ./demo_shared/");
    Ok(())
}
