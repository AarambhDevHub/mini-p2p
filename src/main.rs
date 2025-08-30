use clap::{Parser, Subcommand};
use mini_p2p::{Config, Node, Result};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mini-p2p")]
#[command(about = "A mini P2P file sharing system like BitTorrent")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a peer node
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
        /// Directory to share files from
        #[arg(short, long, default_value = "./shared")]
        dir: PathBuf,
        /// Bootstrap peer address (host:port)
        #[arg(short, long)]
        bootstrap: Option<String>,
        /// Node name for identification
        #[arg(short, long)]
        name: Option<String>,
        /// Enable discovery service (only one node per network should enable this)
        #[arg(long)]
        discovery: bool,
    },
    /// Download a file
    Download {
        /// File hash to download
        #[arg(long)]
        hash: String,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Peer address to connect to
        #[arg(short, long)]
        peer: String,
    },
    /// List available files
    List {
        /// Peer address to query
        #[arg(long)]
        peer: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            port,
            dir,
            bootstrap,
            name,
            discovery,
        } => {
            let config = Config {
                port,
                shared_dir: dir,
                bootstrap_peer: bootstrap,
                node_name: name.unwrap_or_else(|| format!("Node-{}", port)),
                discovery_port: if discovery { Some(9999) } else { None }, // ✅ Added discovery_port
            };

            let mut node = Node::new(config).await?;
            node.start().await?;
        }
        Commands::Download { hash, output, peer } => {
            let config = Config {
                port: 0,
                shared_dir: PathBuf::from("./downloads"),
                bootstrap_peer: Some(peer),
                node_name: "Downloader".to_string(),
                discovery_port: None, // ✅ Added discovery_port
            };

            let mut node = Node::new(config).await?;
            node.download_file(&hash, &output).await?;
            println!("Download completed: {:?}", output);
        }
        Commands::List { peer } => {
            let config = Config {
                port: 0,
                shared_dir: PathBuf::from("./downloads"),
                bootstrap_peer: Some(peer),
                node_name: "Lister".to_string(),
                discovery_port: None, // ✅ Added discovery_port
            };

            let mut node = Node::new(config).await?;
            let files = node.list_remote_files().await?;

            println!("Available files:");
            for file in files {
                println!("  {} - {} ({} bytes)", file.hash, file.name, file.size);
            }
        }
    }

    Ok(())
}
