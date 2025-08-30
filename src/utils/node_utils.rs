use crate::{Config, Node, P2PError, Result};
use log::{error, info, warn};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tokio::net::{TcpListener, UdpSocket};
use tokio::time::{Duration, sleep, timeout};

pub struct NodeUtils;

impl NodeUtils {
    /// Create a UDP socket with SO_REUSEADDR (and SO_REUSEPORT on Unix if available)
    pub async fn create_reusable_udp_socket(addr: SocketAddr) -> Result<UdpSocket> {
        let domain = if addr.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|e| P2PError::NetworkError(format!("Failed to create socket: {}", e)))?;

        // Enable SO_REUSEADDR (cross-platform)
        socket
            .set_reuse_address(true)
            .map_err(|e| P2PError::NetworkError(format!("Failed to set reuse_address: {}", e)))?;

        // Enable SO_REUSEPORT on Unix systems only (conditional compilation)
        #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
        {
            // Only try to set reuse_port if the method exists and we're on a supported Unix system
            if let Err(e) = socket.set_reuse_port(true) {
                warn!("Could not set SO_REUSEPORT (not critical): {}", e);
                // Don't fail - SO_REUSEPORT is optional for basic functionality
            }
        }

        socket
            .bind(&addr.into())
            .map_err(|e| P2PError::NetworkError(format!("Failed to bind to {}: {}", addr, e)))?;

        socket
            .set_nonblocking(true)
            .map_err(|e| P2PError::NetworkError(format!("Failed to set nonblocking: {}", e)))?;

        let std_socket = socket.into();
        UdpSocket::from_std(std_socket).map_err(|e| {
            P2PError::NetworkError(format!("Failed to convert to tokio socket: {}", e))
        })
    }

    /// Check if a TCP port is available for binding
    pub async fn is_port_available(port: u16) -> bool {
        match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Find the next available port starting from a given port
    pub async fn find_available_port(start_port: u16) -> Result<u16> {
        for port in start_port..start_port + 100 {
            if Self::is_port_available(port).await {
                return Ok(port);
            }
        }
        Err(P2PError::NetworkError(
            "No available ports found".to_string(),
        ))
    }

    /// Create a node with automatic port selection if the specified port is unavailable
    pub async fn create_node_with_port_check(mut config: Config) -> Result<Node> {
        if config.port > 0 && !Self::is_port_available(config.port).await {
            warn!(
                "Port {} is already in use, finding alternative...",
                config.port
            );
            config.port = Self::find_available_port(config.port + 1).await?;
            info!("Using alternative port: {}", config.port);
        }

        Node::new(config).await
    }

    /// Wait for a port to be ready (listening)
    pub async fn wait_for_port_ready(port: u16, timeout_duration: Duration) -> bool {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .is_ok()
            {
                return true;
            }
            sleep(Duration::from_millis(200)).await;
        }
        false
    }

    /// Comprehensive node startup with all checks
    pub async fn start_node_safely(config: Config) -> Result<Node> {
        info!(
            "Starting node: {} on port {}",
            config.node_name, config.port
        );

        // Create node with port checking
        let mut node = Self::create_node_with_port_check(config).await?;

        // Start the node
        node.start().await?;

        Ok(node)
    }
}
