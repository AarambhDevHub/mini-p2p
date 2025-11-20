use log::info;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::utils::{P2PError, RateLimiter, Result};

pub struct Transport;

impl Transport {
    pub async fn connect(addr: &str) -> Result<TcpStream> {
        let stream = TcpStream::connect(addr).await.map_err(|e| {
            P2PError::ConnectionFailed(format!("Failed to connect to {}: {}", addr, e))
        })?;

        info!("Connected to {}", addr);
        Ok(stream)
    }

    pub async fn listen(port: u16) -> Result<TcpListener> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| P2PError::NetworkError(format!("Failed to bind to {}: {}", addr, e)))?;

        info!("Listening on {}", addr);
        Ok(listener)
    }

    pub async fn send_data(
        stream: &mut TcpStream,
        data: &[u8],
        limiter: Option<&RateLimiter>,
    ) -> Result<()> {
        let len = data.len() as u32;
        stream.write_u32(len).await?;

        if let Some(limiter) = limiter {
            let chunk_size = 4096;
            for chunk in data.chunks(chunk_size) {
                limiter.acquire(chunk.len() as u64).await;
                stream.write_all(chunk).await?;
            }
        } else {
            stream.write_all(data).await?;
        }

        stream.flush().await?;
        Ok(())
    }

    pub async fn receive_data(
        stream: &mut TcpStream,
        max_size: usize,
        limiter: Option<&RateLimiter>,
    ) -> Result<Vec<u8>> {
        let len = stream.read_u32().await? as usize;

        if len > max_size {
            return Err(P2PError::MessageTooLarge(len));
        }

        let mut buffer = vec![0u8; len];

        if let Some(limiter) = limiter {
            let chunk_size = 4096;
            let mut received = 0;
            while received < len {
                let remaining = len - received;
                let to_read = std::cmp::min(remaining, chunk_size);

                limiter.acquire(to_read as u64).await;
                stream
                    .read_exact(&mut buffer[received..received + to_read])
                    .await?;
                received += to_read;
            }
        } else {
            stream.read_exact(&mut buffer).await?;
        }

        Ok(buffer)
    }
}
