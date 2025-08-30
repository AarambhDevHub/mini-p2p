//! Performance benchmarking for P2P network
//! Fixed version with proper node lifecycle management and error handling

use mini_p2p::{Config, Node, NodeUtils};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::time::{sleep, timeout};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub test_name: String,
    pub total_duration: Duration,
    pub files_transferred: usize,
    pub total_bytes: u64,
    pub throughput_mbps: f64,
    pub avg_latency_ms: f64,
    pub success_rate: f64,
    pub peer_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub struct BenchmarkConfig {
    pub test_duration: Duration,
    pub file_sizes: Vec<usize>,
    pub peer_counts: Vec<usize>,
    pub concurrent_downloads: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            test_duration: Duration::from_secs(30), // Reduced for faster testing
            file_sizes: vec![1024, 10_240, 102_400], // Smaller set for reliability
            peer_counts: vec![2, 3],
            concurrent_downloads: 2,
        }
    }
}

#[tokio::main]
async fn main() -> mini_p2p::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("🚀 P2P Network Performance Benchmark");
    println!("====================================");

    let config = BenchmarkConfig::default();
    let mut all_metrics = Vec::new();

    // Cleanup previous benchmark data
    cleanup_benchmark_environment().await;

    println!("\n📊 Running benchmark scenarios...\n");

    // 1. Single file transfer benchmark
    println!("🔄 Test 1: Single File Transfer Performance");
    match benchmark_single_file_transfer(&config).await {
        Ok(metrics) => all_metrics.push(metrics),
        Err(e) => println!("❌ Test 1 failed: {}", e),
    }

    // Brief pause between tests
    sleep(Duration::from_secs(2)).await;

    // 2. Multiple file sizes benchmark
    println!("\n🔄 Test 2: Multiple File Sizes Performance");
    match benchmark_multiple_file_sizes(&config).await {
        Ok(mut metrics) => all_metrics.append(&mut metrics),
        Err(e) => println!("❌ Test 2 failed: {}", e),
    }

    // Generate report
    generate_benchmark_report(&all_metrics).await?;
    cleanup_benchmark_environment().await;

    println!("\n✅ Benchmark completed! Check benchmark_report.json for detailed results.");
    Ok(())
}

async fn benchmark_single_file_transfer(
    config: &BenchmarkConfig,
) -> mini_p2p::Result<BenchmarkMetrics> {
    let start_time = Instant::now();
    let mut errors = Vec::new();

    // Setup test environment
    let test_dir = "./benchmark_single";
    setup_test_files(test_dir, &[51_200]).await?; // 50KB file for reliability

    // Find available ports
    let seed_port = NodeUtils::find_available_port(8100).await?;
    let download_port = NodeUtils::find_available_port(8101).await?;
    let discovery_port = NodeUtils::find_available_port(9900).await?;

    println!(
        "  Using ports - Seed: {}, Download: {}, Discovery: {}",
        seed_port, download_port, discovery_port
    );

    // Start seed node
    let seed_config = Config {
        port: seed_port,
        shared_dir: PathBuf::from(test_dir),
        bootstrap_peer: None,
        node_name: "BenchmarkSeed".to_string(),
        discovery_port: Some(discovery_port),
    };

    let seed_handle = start_benchmark_node(seed_config, "seed".to_string()).await;

    // Wait for seed to be ready with timeout
    if !NodeUtils::wait_for_port_ready(seed_port, Duration::from_secs(10)).await {
        errors.push("Seed node failed to start within timeout".to_string());
        seed_handle.abort();
        return Ok(create_error_metrics("Single File Transfer", errors));
    }

    println!("  ✅ Seed node ready");

    // Perform download with timeout
    let download_result = timeout(Duration::from_secs(30), async {
        let download_config = Config {
            port: download_port,
            shared_dir: PathBuf::from("./benchmark_single_downloads"),
            bootstrap_peer: Some(format!("127.0.0.1:{}", seed_port)),
            node_name: "BenchmarkDownloader".to_string(),
            discovery_port: None,
        };

        perform_benchmark_download(download_config).await
    })
    .await;

    let (bytes_transferred, files_transferred) = match download_result {
        Ok(Ok((bytes, files))) => {
            println!("  ✅ Download completed: {} bytes, {} files", bytes, files);
            (bytes, files)
        }
        Ok(Err(e)) => {
            errors.push(format!("Download failed: {}", e));
            (0, 0)
        }
        Err(_) => {
            errors.push("Download timed out".to_string());
            (0, 0)
        }
    };

    let total_duration = start_time.elapsed();
    let throughput_mbps = if total_duration.as_secs_f64() > 0.0 {
        (bytes_transferred as f64 / 1_048_576.0) / total_duration.as_secs_f64()
    } else {
        0.0
    };

    // Proper cleanup
    seed_handle.abort();
    sleep(Duration::from_millis(500)).await; // Allow cleanup
    let _ = fs::remove_dir_all(test_dir).await;
    let _ = fs::remove_dir_all("./benchmark_single_downloads").await;

    Ok(BenchmarkMetrics {
        test_name: "Single File Transfer".to_string(),
        total_duration,
        files_transferred,
        total_bytes: bytes_transferred,
        throughput_mbps,
        avg_latency_ms: total_duration.as_millis() as f64 / files_transferred.max(1) as f64,
        success_rate: if errors.is_empty() && files_transferred > 0 {
            100.0
        } else {
            0.0
        },
        peer_count: 2,
        errors,
    })
}

async fn benchmark_multiple_file_sizes(
    config: &BenchmarkConfig,
) -> mini_p2p::Result<Vec<BenchmarkMetrics>> {
    let mut results = Vec::new();

    for (i, &file_size) in config.file_sizes.iter().enumerate() {
        println!(
            "  📄 Testing file size: {} bytes ({}/{})",
            file_size,
            i + 1,
            config.file_sizes.len()
        );

        let start_time = Instant::now();
        let mut errors = Vec::new();

        // Setup
        let test_dir = format!("./benchmark_size_{}", file_size);
        setup_test_files(&test_dir, &[file_size]).await?;

        let seed_port = NodeUtils::find_available_port(8200 + i as u16 * 10).await?;
        let download_port = NodeUtils::find_available_port(8201 + i as u16 * 10).await?;
        let discovery_port = NodeUtils::find_available_port(9901 + i as u16).await?;

        // Run test with timeout
        let test_result = timeout(Duration::from_secs(45), async {
            let seed_config = Config {
                port: seed_port,
                shared_dir: PathBuf::from(&test_dir),
                bootstrap_peer: None,
                node_name: format!("SizeBenchmarkSeed_{}", file_size),
                discovery_port: Some(discovery_port),
            };

            let seed_handle = start_benchmark_node(seed_config, "seed".to_string()).await;

            if !NodeUtils::wait_for_port_ready(seed_port, Duration::from_secs(10)).await {
                seed_handle.abort();
                return Err(mini_p2p::P2PError::NetworkError(
                    "Seed failed to start".to_string(),
                ));
            }

            let download_config = Config {
                port: download_port,
                shared_dir: PathBuf::from(format!("./benchmark_size_{}_downloads", file_size)),
                bootstrap_peer: Some(format!("127.0.0.1:{}", seed_port)),
                node_name: format!("SizeBenchmarkDownloader_{}", file_size),
                discovery_port: None,
            };

            let result = perform_benchmark_download(download_config).await;

            // Proper cleanup
            seed_handle.abort();
            sleep(Duration::from_millis(500)).await;

            result
        })
        .await;

        let (bytes_transferred, files_transferred) = match test_result {
            Ok(Ok((bytes, files))) => {
                println!("    ✅ Completed: {} bytes", bytes);
                (bytes, files)
            }
            Ok(Err(e)) => {
                errors.push(format!("Test failed: {}", e));
                (0, 0)
            }
            Err(_) => {
                errors.push("Test timed out".to_string());
                (0, 0)
            }
        };

        let total_duration = start_time.elapsed();
        let throughput_mbps = if total_duration.as_secs_f64() > 0.0 {
            (bytes_transferred as f64 / 1_048_576.0) / total_duration.as_secs_f64()
        } else {
            0.0
        };

        results.push(BenchmarkMetrics {
            test_name: format!("File Size {} bytes", file_size),
            total_duration,
            files_transferred,
            total_bytes: bytes_transferred,
            throughput_mbps,
            avg_latency_ms: total_duration.as_millis() as f64 / files_transferred.max(1) as f64,
            success_rate: if errors.is_empty() && files_transferred > 0 {
                100.0
            } else {
                0.0
            },
            peer_count: 2,
            errors,
        });

        // Cleanup after each test
        let _ = fs::remove_dir_all(&test_dir).await;
        let _ = fs::remove_dir_all(&format!("./benchmark_size_{}_downloads", file_size)).await;

        // Pause between tests
        sleep(Duration::from_secs(1)).await;
    }

    Ok(results)
}

// Helper functions

async fn setup_test_files(dir: &str, file_sizes: &[usize]) -> mini_p2p::Result<()> {
    fs::create_dir_all(dir).await?;

    for (i, &size) in file_sizes.iter().enumerate() {
        let content = vec![b'A' + (i as u8 % 26); size];
        let filename = format!("{}/benchmark_file_{}_{}.dat", dir, i, size);
        fs::write(&filename, content).await?;
    }

    Ok(())
}

async fn start_benchmark_node(config: Config, node_type: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        match Node::new(config).await {
            Ok(mut node) => {
                if let Err(e) = node.start().await {
                    eprintln!("❌ {} node error: {}", node_type, e);
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to create {} node: {}", node_type, e);
            }
        }
    })
}

async fn perform_benchmark_download(config: Config) -> mini_p2p::Result<(u64, usize)> {
    let shared_dir = config.shared_dir.clone();
    let mut download_node = Node::new(config).await?;

    // ✅ Reduce connection wait time but add retry logic
    for attempt in 1..=3 {
        sleep(Duration::from_secs(1)).await; // Shorter initial wait

        match timeout(Duration::from_secs(8), download_node.list_remote_files()).await {
            Ok(Ok(files)) if !files.is_empty() => {
                println!("  📡 Connected to peer (attempt {})", attempt);
                return download_files_with_verification(download_node, files, shared_dir).await;
            }
            Ok(Ok(_)) => {
                println!("  ⚠️  No files found (attempt {})", attempt);
            }
            Ok(Err(e)) => {
                println!("  ❌ Connection error (attempt {}): {}", attempt, e);
            }
            Err(_) => {
                println!("  ⏰ Connection timeout (attempt {})", attempt);
            }
        }

        if attempt < 3 {
            sleep(Duration::from_secs(1)).await;
        }
    }

    Err(mini_p2p::P2PError::InvalidResponse(
        "Failed to connect after 3 attempts".to_string(),
    ))
}

async fn download_files_with_verification(
    mut download_node: Node,
    files: Vec<mini_p2p::FileInfo>,
    shared_dir: std::path::PathBuf,
) -> mini_p2p::Result<(u64, usize)> {
    let mut total_bytes = 0u64;
    let mut successful_downloads = 0usize;

    for file in &files {
        let output_path = shared_dir.join(&file.name);

        // ✅ Add per-file timeout based on size
        let timeout_secs = std::cmp::max(10, file.size / 10000); // 10KB/sec minimum

        let download_result = timeout(
            Duration::from_secs(timeout_secs),
            download_node.download_file(&file.hash, &output_path),
        )
        .await;

        match download_result {
            Ok(Ok(_)) => {
                // Verify actual file size
                if let Ok(metadata) = tokio::fs::metadata(&output_path).await {
                    let actual_size = metadata.len();
                    if actual_size == file.size {
                        total_bytes += actual_size;
                        successful_downloads += 1;
                        println!("    ✅ Verified: {} ({} bytes)", file.name, actual_size);
                    } else {
                        println!(
                            "    ❌ Size mismatch: {} (expected: {}, got: {})",
                            file.name, file.size, actual_size
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                println!("    ❌ Download failed: {} - {}", file.name, e);
            }
            Err(_) => {
                println!("    ⏰ Download timeout: {} ({}s)", file.name, timeout_secs);
            }
        }
    }

    Ok((total_bytes, successful_downloads))
}

fn create_error_metrics(test_name: &str, errors: Vec<String>) -> BenchmarkMetrics {
    BenchmarkMetrics {
        test_name: test_name.to_string(),
        total_duration: Duration::from_secs(0),
        files_transferred: 0,
        total_bytes: 0,
        throughput_mbps: 0.0,
        avg_latency_ms: 0.0,
        success_rate: 0.0,
        peer_count: 0,
        errors,
    }
}

async fn generate_benchmark_report(metrics: &[BenchmarkMetrics]) -> mini_p2p::Result<()> {
    // Generate JSON report
    let json_report = serde_json::to_string_pretty(metrics)?;
    fs::write("benchmark_report.json", json_report).await?;

    // Generate human-readable summary
    let mut summary = String::new();
    summary.push_str("🚀 P2P Network Benchmark Report\n");
    summary.push_str("===============================\n\n");

    let mut total_throughput = 0.0;
    let mut total_success_rate = 0.0;
    let mut test_count = 0;

    for metric in metrics {
        summary.push_str(&format!("📊 Test: {}\n", metric.test_name));
        summary.push_str(&format!("   Duration: {:.2?}\n", metric.total_duration));
        summary.push_str(&format!(
            "   Files Transferred: {}\n",
            metric.files_transferred
        ));
        summary.push_str(&format!(
            "   Total Bytes: {} ({:.2} MB)\n",
            metric.total_bytes,
            metric.total_bytes as f64 / 1_048_576.0
        ));
        summary.push_str(&format!(
            "   Throughput: {:.2} MB/s\n",
            metric.throughput_mbps
        ));
        summary.push_str(&format!(
            "   Avg Latency: {:.2} ms\n",
            metric.avg_latency_ms
        ));
        summary.push_str(&format!("   Success Rate: {:.1}%\n", metric.success_rate));
        summary.push_str(&format!("   Peer Count: {}\n", metric.peer_count));

        if !metric.errors.is_empty() {
            summary.push_str("   Errors:\n");
            for error in &metric.errors {
                summary.push_str(&format!("     - {}\n", error));
            }
        }
        summary.push_str("\n");

        total_throughput += metric.throughput_mbps;
        total_success_rate += metric.success_rate;
        test_count += 1;
    }

    if test_count > 0 {
        summary.push_str("📈 Overall Summary:\n");
        summary.push_str(&format!(
            "   Average Throughput: {:.2} MB/s\n",
            total_throughput / test_count as f64
        ));
        summary.push_str(&format!(
            "   Average Success Rate: {:.1}%\n",
            total_success_rate / test_count as f64
        ));
        summary.push_str(&format!("   Total Tests: {}\n", test_count));

        println!("\n📊 Benchmark Summary:");
        println!(
            "   Average Throughput: {:.2} MB/s",
            total_throughput / test_count as f64
        );
        println!(
            "   Average Success Rate: {:.1}%",
            total_success_rate / test_count as f64
        );
        println!("   Total Tests: {}", test_count);
    }

    fs::write("benchmark_summary.txt", summary).await?;
    Ok(())
}

async fn cleanup_benchmark_environment() {
    let directories = [
        "./benchmark_single",
        "./benchmark_single_downloads",
        "./benchmark_concurrent",
        "./benchmark_resilience",
    ];

    for dir in &directories {
        let _ = fs::remove_dir_all(dir).await;
    }

    // Clean up any remaining benchmark directories
    if let Ok(mut entries) = fs::read_dir("./").await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("benchmark_") {
                    let _ = fs::remove_dir_all(entry.path()).await;
                }
            }
        }
    }
}
