//! Channel multiplexing for different message types over QUIC.
//!
//! Provides reliable and unreliable channels for efficient message delivery.

use anyhow::{Context, Result};
use quinn::Connection;
use serde::{Deserialize, Serialize};
use tracing::trace;

/// Channel type identifier for message routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChannelType {
    /// Input messages from client to server (unreliable, ordered).
    Input = 0,
    /// Chunk streaming from server to client (reliable, ordered).
    ChunkStream = 1,
    /// Entity delta updates (unreliable, ordered).
    EntityDelta = 2,
    /// Chat messages (reliable, ordered).
    Chat = 3,
    /// Diagnostics and debug info (reliable, ordered).
    Diagnostics = 4,
}

impl ChannelType {
    /// Check if this channel type should use reliable delivery.
    pub fn is_reliable(&self) -> bool {
        matches!(
            self,
            ChannelType::ChunkStream | ChannelType::Chat | ChannelType::Diagnostics
        )
    }

    /// Check if this channel type should use unreliable delivery.
    pub fn is_unreliable(&self) -> bool {
        !self.is_reliable()
    }
}

impl TryFrom<u8> for ChannelType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(ChannelType::Input),
            1 => Ok(ChannelType::ChunkStream),
            2 => Ok(ChannelType::EntityDelta),
            3 => Ok(ChannelType::Chat),
            4 => Ok(ChannelType::Diagnostics),
            _ => Err(anyhow::anyhow!("Invalid channel type: {}", value)),
        }
    }
}

/// Multiplexed channel manager for QUIC connections.
pub struct ChannelManager {
    connection: Connection,
}

impl ChannelManager {
    /// Create a new channel manager for the given connection.
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    /// Send a message on a reliable channel (QUIC stream).
    ///
    /// Opens a new unidirectional stream for each message.
    pub async fn send_reliable(&self, channel: ChannelType, data: &[u8]) -> Result<()> {
        debug_assert!(
            channel.is_reliable(),
            "Channel {:?} is not reliable",
            channel
        );

        trace!("Sending {} bytes on reliable {:?}", data.len(), channel);

        // Open a new unidirectional stream
        let mut send_stream = self
            .connection
            .open_uni()
            .await
            .context("Failed to open unidirectional stream")?;

        // Write channel type header
        send_stream
            .write_all(&[channel as u8])
            .await
            .context("Failed to write channel type")?;

        // Write length prefix
        let len = data.len() as u32;
        send_stream
            .write_all(&len.to_le_bytes())
            .await
            .context("Failed to write length prefix")?;

        // Write data
        send_stream
            .write_all(data)
            .await
            .context("Failed to write data")?;

        // Finish the stream
        send_stream.finish().context("Failed to finish stream")?;

        trace!("Sent {} bytes on reliable {:?}", data.len(), channel);

        Ok(())
    }

    /// Send a message on an unreliable channel (QUIC datagram).
    pub async fn send_unreliable(&self, channel: ChannelType, data: &[u8]) -> Result<()> {
        debug_assert!(
            channel.is_unreliable(),
            "Channel {:?} is not unreliable",
            channel
        );

        trace!("Sending {} bytes on unreliable {:?}", data.len(), channel);

        // Build datagram: [channel_type: u8][data: bytes]
        let mut datagram = Vec::with_capacity(1 + data.len());
        datagram.push(channel as u8);
        datagram.extend_from_slice(data);

        // Send datagram
        self.connection
            .send_datagram(datagram.into())
            .context("Failed to send datagram")?;

        trace!("Sent {} bytes on unreliable {:?}", data.len(), channel);

        Ok(())
    }

    /// Receive the next message on a reliable channel (QUIC stream).
    ///
    /// Returns the channel type and message data.
    pub async fn recv_reliable(&self) -> Result<(ChannelType, Vec<u8>)> {
        // Accept the next unidirectional stream
        let mut recv_stream = self
            .connection
            .accept_uni()
            .await
            .context("Failed to accept unidirectional stream")?;

        // Read channel type header
        let mut channel_byte = [0u8; 1];
        recv_stream
            .read_exact(&mut channel_byte)
            .await
            .context("Failed to read channel type")?;
        let channel = ChannelType::try_from(channel_byte[0])?;

        // Read length prefix
        let mut len_bytes = [0u8; 4];
        recv_stream
            .read_exact(&mut len_bytes)
            .await
            .context("Failed to read length prefix")?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read data
        let mut data = vec![0u8; len];
        recv_stream
            .read_exact(&mut data)
            .await
            .context("Failed to read data")?;

        trace!("Received {} bytes on reliable {:?}", data.len(), channel);

        Ok((channel, data))
    }

    /// Receive the next message on an unreliable channel (QUIC datagram).
    ///
    /// Returns the channel type and message data.
    pub async fn recv_unreliable(&self) -> Result<(ChannelType, Vec<u8>)> {
        // Receive the next datagram
        let datagram = self
            .connection
            .read_datagram()
            .await
            .context("Failed to read datagram")?;

        if datagram.is_empty() {
            return Err(anyhow::anyhow!("Received empty datagram"));
        }

        // Parse channel type
        let channel = ChannelType::try_from(datagram[0])?;

        // Extract data
        let data = datagram[1..].to_vec();

        trace!("Received {} bytes on unreliable {:?}", data.len(), channel);

        Ok((channel, data))
    }

    /// Get the remote address of this connection.
    pub fn remote_address(&self) -> std::net::SocketAddr {
        self.connection.remote_address()
    }

    /// Close the connection gracefully.
    pub fn close(&self, reason: &str) {
        self.connection.close(0u32.into(), reason.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{ClientEndpoint, ServerEndpoint, TlsMode};

    #[tokio::test]
    async fn test_reliable_channel() {
        // Start server
        let server =
            ServerEndpoint::bind("127.0.0.1:0".parse().unwrap()).expect("Failed to bind server");
        let server_addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let incoming = server.accept().await.expect("No incoming connection");
            let connection = incoming.await.expect("Failed to accept connection");
            let manager = ChannelManager::new(connection);

            // Receive message
            let (channel, data) = manager
                .recv_reliable()
                .await
                .expect("Failed to receive message");
            assert_eq!(channel, ChannelType::Chat);
            assert_eq!(data, b"Hello, server!");

            // Send response
            manager
                .send_reliable(ChannelType::Chat, b"Hello, client!")
                .await
                .expect("Failed to send response");

            // Keep connection alive briefly
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Small delay to ensure server is listening
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect client
        let client = ClientEndpoint::new(TlsMode::InsecureSkipVerify)
            .expect("Failed to create client");
        let connection = client
            .connect(server_addr)
            .await
            .expect("Failed to connect");
        let manager = ChannelManager::new(connection);

        // Send message
        manager
            .send_reliable(ChannelType::Chat, b"Hello, server!")
            .await
            .expect("Failed to send message");

        // Receive response
        let (channel, data) = manager
            .recv_reliable()
            .await
            .expect("Failed to receive response");
        assert_eq!(channel, ChannelType::Chat);
        assert_eq!(data, b"Hello, client!");

        // Wait for server task
        server_handle.await.expect("Server task panicked");
    }

    #[tokio::test]
    async fn test_unreliable_channel() {
        // Start server
        let server =
            ServerEndpoint::bind("127.0.0.1:0".parse().unwrap()).expect("Failed to bind server");
        let server_addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let incoming = server.accept().await.expect("No incoming connection");
            let connection = incoming.await.expect("Failed to accept connection");
            let manager = ChannelManager::new(connection);

            // Receive message
            let (channel, data) = manager
                .recv_unreliable()
                .await
                .expect("Failed to receive message");
            assert_eq!(channel, ChannelType::Input);
            assert_eq!(data, b"Move forward");

            // Send response
            manager
                .send_unreliable(ChannelType::EntityDelta, b"Position update")
                .await
                .expect("Failed to send response");

            // Keep connection alive briefly
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Small delay to ensure server is listening
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect client
        let client = ClientEndpoint::new(TlsMode::InsecureSkipVerify)
            .expect("Failed to create client");
        let connection = client
            .connect(server_addr)
            .await
            .expect("Failed to connect");
        let manager = ChannelManager::new(connection);

        // Send message
        manager
            .send_unreliable(ChannelType::Input, b"Move forward")
            .await
            .expect("Failed to send message");

        // Receive response
        let (channel, data) = manager
            .recv_unreliable()
            .await
            .expect("Failed to receive response");
        assert_eq!(channel, ChannelType::EntityDelta);
        assert_eq!(data, b"Position update");

        // Wait for server task
        server_handle.await.expect("Server task panicked");
    }
}
