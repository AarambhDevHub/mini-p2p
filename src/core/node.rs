use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::{Duration, interval, sleep};
use uuid::Uuid;

use crate::core::{Config, PeerManager};
use crate::network::Discovery;
use crate::storage::FileInfo;
use crate::storage::FileManager;
use crate::transfer::{Downloader, Uploader};
use crate::utils::{P2PError, Result};

pub struct Node {
    id: Uuid,
    pub config: Config,
    file_manager: Arc<Mutex<FileManager>>,
    peer_manager: Arc<RwLock<PeerManager>>,
    discovery: Arc<Discovery>,
    downloader: Arc<Downloader>,
    uploader: Arc<Uploader>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Node {
    pub async fn new(config: Config) -> Result<Self> {
        let id = Uuid::new_v4();
        let file_manager = Arc::new(Mutex::new(
            FileManager::new(config.shared_dir.clone()).await?,
        ));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new()));
        let mut discovery = Discovery::new(id, config.node_name.clone());

        // ✅ Initialize discovery with configurable port
        if config.port > 0 {
            discovery
                .start_listener(config.port, config.discovery_port)
                .await?;
        }

        let discovery = Arc::new(discovery);
        let downloader = Arc::new(Downloader::new(file_manager.clone(), peer_manager.clone()));
        let uploader = Arc::new(Uploader::new(file_manager.clone()));

        Ok(Self {
            id,
            config,
            file_manager,
            peer_manager,
            discovery,
            downloader,
            uploader,
            shutdown_tx: None,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!(
            "Starting P2P node {} ({}) on port {}",
            self.id, self.config.node_name, self.config.port
        );

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Connect to bootstrap peer if provided
        if let Some(bootstrap) = &self.config.bootstrap_peer {
            self.connect_to_bootstrap(bootstrap).await?;
        }

        // Start file monitoring
        self.start_file_monitoring();

        // Start peer management
        self.start_peer_management();

        // Start discovery service
        self.start_discovery_service().await?;

        // Start accepting connections
        if self.config.port > 0 {
            let listener = TcpListener::bind(format!("0.0.0.0:{}", self.config.port)).await?;
            info!("Listening for connections on port {}", self.config.port);
            self.start_connection_acceptor(listener);
        }

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl+C received, shutting down");
            }
        }

        Ok(())
    }

    async fn start_discovery_service(&self) -> Result<()> {
        let discovery = self.discovery.clone();
        let peer_manager = self.peer_manager.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                if let Err(e) = discovery.announce().await {
                    warn!("Discovery announcement failed: {}", e);
                }

                match discovery.discover_peers().await {
                    Ok(peers) => {
                        let mut pm = peer_manager.write().await;
                        for peer_addr in peers {
                            pm.add_discovered_peer(peer_addr);
                        }
                    }
                    Err(e) => warn!("Peer discovery failed: {}", e),
                }
            }
        });

        Ok(())
    }

    fn start_file_monitoring(&self) {
        let file_manager = self.file_manager.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = file_manager.lock().await.scan_files().await {
                    warn!("File scan failed: {}", e);
                }
            }
        });
    }

    fn start_peer_management(&self) {
        let peer_manager = self.peer_manager.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                peer_manager.write().await.cleanup_dead_peers().await;
            }
        });
    }

    fn start_connection_acceptor(&self, listener: TcpListener) {
        let peer_manager = self.peer_manager.clone();
        let uploader = self.uploader.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        info!("New connection from: {}", addr);

                        let peer_manager = peer_manager.clone();
                        let uploader = uploader.clone();

                        tokio::spawn(async move {
                            if let Err(e) =
                                Self::handle_new_connection(stream, addr, peer_manager, uploader)
                                    .await
                            {
                                error!("Failed to handle connection from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    async fn handle_new_connection(
        stream: TcpStream,
        addr: SocketAddr,
        peer_manager: Arc<RwLock<PeerManager>>,
        uploader: Arc<Uploader>,
    ) -> Result<()> {
        let peer = crate::core::Peer::new(Uuid::new_v4(), addr, stream).await?;
        let peer_id = peer.id;

        info!("New peer connected: {} from {}", peer_id, addr);

        // Send handshake
        let handshake = crate::core::Message::handshake(peer_id, format!("Node-{}", addr.port()));
        if let Err(e) = peer.send_message(handshake).await {
            warn!("Failed to send handshake to {}: {}", addr, e);
            return Err(e);
        }

        // Add peer to manager
        {
            let mut pm = peer_manager.write().await;
            pm.add_peer(peer).await?;
        }

        // Handle peer communication
        loop {
            let message = {
                let pm = peer_manager.read().await;
                if let Some(peer) = pm.get_peer(&peer_id) {
                    match peer.receive_message().await {
                        Ok(msg) => {
                            debug!("Received message from {}: {:?}", peer_id, msg.msg_type);
                            msg
                        }
                        Err(e) => {
                            debug!("Connection lost with {}: {}", peer_id, e);
                            break; // Connection lost
                        }
                    }
                } else {
                    break; // Peer removed
                }
            };

            // Handle the message
            if let Err(e) = uploader
                .handle_request(&peer_id, message, &peer_manager)
                .await
            {
                warn!("Failed to handle request from {}: {}", peer_id, e);
            }
        }

        // Remove peer on disconnect
        {
            let mut pm = peer_manager.write().await;
            pm.remove_peer(&peer_id);
        }

        info!("Peer disconnected: {}", peer_id);
        Ok(())
    }

    async fn connect_to_bootstrap(&self, bootstrap: &str) -> Result<()> {
        info!("Connecting to bootstrap peer: {}", bootstrap);

        // ✅ Add connection timeout to prevent 23-second hangs
        let stream = tokio::time::timeout(
            Duration::from_secs(5), // Reduced from default timeout
            TcpStream::connect(bootstrap),
        )
        .await
        .map_err(|_| P2PError::ConnectionFailed("Bootstrap connection timeout".to_string()))?
        .map_err(|e| P2PError::ConnectionFailed(format!("Bootstrap connection failed: {}", e)))?;

        let addr = stream.peer_addr()?;
        let peer = crate::core::Peer::new(Uuid::new_v4(), addr, stream).await?;

        // ✅ Add handshake timeout
        let handshake = crate::core::Message::handshake(self.id, self.config.node_name.clone());
        peer.send_message(handshake).await?;

        // Add timeout for handshake response
        match tokio::time::timeout(Duration::from_secs(3), peer.receive_message()).await {
            Ok(Ok(msg)) if msg.msg_type == crate::core::MessageType::Handshake => {
                info!("Handshake completed with bootstrap peer");
            }
            Ok(Ok(msg)) => {
                warn!("Unexpected message from bootstrap: {:?}", msg.msg_type);
            }
            Ok(Err(e)) => {
                return Err(P2PError::ConnectionFailed(format!(
                    "Handshake failed: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(P2PError::ConnectionFailed("Handshake timeout".to_string()));
            }
        }

        let mut pm = self.peer_manager.write().await;
        pm.add_peer(peer).await?;

        info!("Successfully connected to bootstrap peer: {}", bootstrap);
        Ok(())
    }

    pub async fn download_file(&mut self, file_hash: &str, output_path: &PathBuf) -> Result<()> {
        info!("Starting download of file: {}", file_hash);

        if let Some(bootstrap) = &self.config.bootstrap_peer {
            self.connect_to_bootstrap(bootstrap).await?;
        }

        self.downloader
            .download_file(file_hash, output_path)
            .await?;
        Ok(())
    }

    pub async fn list_remote_files(&mut self) -> Result<Vec<FileInfo>> {
        if let Some(bootstrap) = &self.config.bootstrap_peer {
            self.connect_to_bootstrap(bootstrap).await?;
        }

        let pm = self.peer_manager.read().await;
        let peers = pm.get_all_peers();

        let mut all_files = Vec::new();
        for peer in peers {
            if let Ok(files) = peer.request_file_list().await {
                all_files.extend(files);
            }
        }

        Ok(all_files)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        Ok(())
    }
}
