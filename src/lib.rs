//! Mini P2P File Sharing Library
//!
//! A BitTorrent-like P2P file sharing system with distributed architecture.

pub mod core;
pub mod network;
pub mod storage;
pub mod transfer;
pub mod utils;

// Re-export main types
pub use core::{Config, Node};
pub use network::Discovery;
pub use storage::{Chunk, FileInfo, FileManager};
pub use utils::{
    NodeUtils,
    error::{P2PError, Result},
};

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
