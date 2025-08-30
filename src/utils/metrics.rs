use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub connections_established: u64,
    pub connections_failed: u64,
    pub files_shared: u64,
    pub files_downloaded: u64,
    pub chunks_uploaded: u64,
    pub chunks_downloaded: u64,
    pub uptime: Duration,
    pub start_time: Instant,
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
            connections_established: 0,
            connections_failed: 0,
            files_shared: 0,
            files_downloaded: 0,
            chunks_uploaded: 0,
            chunks_downloaded: 0,
            uptime: Duration::new(0, 0),
            start_time: Instant::now(),
        }
    }
}

pub struct MetricsCollector {
    metrics: Arc<RwLock<NetworkMetrics>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(NetworkMetrics::default())),
        }
    }

    pub async fn record_bytes_sent(&self, bytes: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.bytes_sent += bytes;
    }

    pub async fn record_bytes_received(&self, bytes: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.bytes_received += bytes;
    }

    pub async fn record_message_sent(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.messages_sent += 1;
    }

    pub async fn record_message_received(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.messages_received += 1;
    }

    pub async fn record_connection_established(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.connections_established += 1;
    }

    pub async fn record_connection_failed(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.connections_failed += 1;
    }

    pub async fn record_file_downloaded(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.files_downloaded += 1;
    }

    pub async fn record_chunk_uploaded(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.chunks_uploaded += 1;
    }

    pub async fn record_chunk_downloaded(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.chunks_downloaded += 1;
    }

    pub async fn get_metrics(&self) -> NetworkMetrics {
        let mut metrics = self.metrics.read().await.clone();
        metrics.uptime = metrics.start_time.elapsed();
        metrics
    }

    pub async fn print_stats(&self) {
        let metrics = self.get_metrics().await;

        println!("\n=== P2P Network Statistics ===");
        println!("Uptime: {:.2?}", metrics.uptime);
        println!(
            "Connections: {} established, {} failed",
            metrics.connections_established, metrics.connections_failed
        );
        println!(
            "Data Transfer: {} bytes sent, {} bytes received",
            metrics.bytes_sent, metrics.bytes_received
        );
        println!(
            "Messages: {} sent, {} received",
            metrics.messages_sent, metrics.messages_received
        );
        println!(
            "Files: {} shared, {} downloaded",
            metrics.files_shared, metrics.files_downloaded
        );
        println!(
            "Chunks: {} uploaded, {} downloaded",
            metrics.chunks_uploaded, metrics.chunks_downloaded
        );
        println!("===============================\n");
    }
}
