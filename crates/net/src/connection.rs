//! High-level connection management integrating transport, channels, and protocol.
//!
//! Provides a unified interface for sending/receiving typed messages.

use crate::channel::{ChannelManager, ChannelType};
use crate::codec::{
    compute_schema_hash, decode_client_message, decode_server_message, encode_client_message,
    encode_server_message,
};
use crate::protocol::{ClientMessage, ServerMessage, PROTOCOL_VERSION};
use anyhow::Result;
use std::net::SocketAddr;
use tracing::{debug, info, warn};

/// Client-side connection wrapping QUIC transport and protocol handling.
pub struct ClientConnection {
    channel_manager: ChannelManager,
    schema_hash: u64,
}

impl ClientConnection {
    /// Create a new client connection from a QUIC connection.
    pub fn new(connection: quinn::Connection) -> Self {
        let schema_hash = compute_schema_hash();
        Self {
            channel_manager: ChannelManager::new(connection),
            schema_hash,
        }
    }

    /// Perform handshake with server.
    ///
    /// Returns the assigned player entity ID on success.
    pub async fn handshake(&self) -> Result<u64> {
        info!("Starting handshake with server");

        // Send handshake request
        let handshake = ClientMessage::Handshake {
            version: PROTOCOL_VERSION,
            schema_hash: self.schema_hash,
        };

        self.send_reliable(handshake).await?;

        // Wait for handshake response
        let response = self.recv_reliable().await?;

        match response {
            ServerMessage::HandshakeResponse {
                accepted,
                reason,
                player_entity_id,
            } => {
                if accepted {
                    let entity_id = player_entity_id.ok_or_else(|| {
                        anyhow::anyhow!("Server accepted but didn't assign entity ID")
                    })?;
                    info!("Handshake successful, assigned entity ID: {}", entity_id);
                    Ok(entity_id)
                } else {
                    let reason = reason.unwrap_or_else(|| "Unknown reason".to_string());
                    Err(anyhow::anyhow!("Handshake rejected: {}", reason))
                }
            }
            msg => Err(anyhow::anyhow!("Expected HandshakeResponse, got {:?}", msg)),
        }
    }

    /// Send a client message on the appropriate channel.
    pub async fn send(&self, msg: ClientMessage) -> Result<()> {
        let channel = select_client_channel(&msg);

        let data = encode_client_message(&msg)?;

        if channel.is_reliable() {
            self.channel_manager.send_reliable(channel, &data).await
        } else {
            self.channel_manager.send_unreliable(channel, &data).await
        }
    }

    /// Send a message on a reliable channel.
    async fn send_reliable(&self, msg: ClientMessage) -> Result<()> {
        let channel = select_client_channel(&msg);
        debug_assert!(channel.is_reliable());

        let data = encode_client_message(&msg)?;
        self.channel_manager.send_reliable(channel, &data).await
    }

    /// Receive the next server message on reliable channels.
    pub async fn recv_reliable(&self) -> Result<ServerMessage> {
        let (_channel, data) = self.channel_manager.recv_reliable().await?;
        decode_server_message(&data)
    }

    /// Receive the next server message on unreliable channels.
    pub async fn recv_unreliable(&self) -> Result<ServerMessage> {
        let (_channel, data) = self.channel_manager.recv_unreliable().await?;
        decode_server_message(&data)
    }

    /// Get the remote server address.
    pub fn remote_address(&self) -> SocketAddr {
        self.channel_manager.remote_address()
    }

    /// Close the connection gracefully.
    pub fn close(&self, reason: &str) {
        info!("Closing connection: {}", reason);
        self.channel_manager.close(reason);
    }
}

/// Server-side connection wrapping QUIC transport and protocol handling.
pub struct ServerConnection {
    channel_manager: ChannelManager,
    schema_hash: u64,
}

impl ServerConnection {
    /// Create a new server connection from a QUIC connection.
    pub fn new(connection: quinn::Connection) -> Self {
        let schema_hash = compute_schema_hash();
        Self {
            channel_manager: ChannelManager::new(connection),
            schema_hash,
        }
    }

    /// Wait for and validate client handshake.
    ///
    /// Returns Ok(schema_hash) if handshake is valid, Err otherwise.
    pub async fn accept_handshake(&self) -> Result<u64> {
        info!(
            "Waiting for handshake from {}",
            self.channel_manager.remote_address()
        );

        // Receive handshake request
        let request = self.recv_reliable().await?;

        match request {
            ClientMessage::Handshake {
                version,
                schema_hash,
            } => {
                debug!(
                    "Received handshake: version={}, schema_hash={:016x}",
                    version, schema_hash
                );

                // Validate version
                if version != PROTOCOL_VERSION {
                    warn!(
                        "Protocol version mismatch: client={}, server={}",
                        version, PROTOCOL_VERSION
                    );
                    self.reject_handshake(&format!(
                        "Protocol version mismatch: server uses v{}",
                        PROTOCOL_VERSION
                    ))
                    .await?;
                    return Err(anyhow::anyhow!(
                        "Protocol version mismatch: {} != {}",
                        version,
                        PROTOCOL_VERSION
                    ));
                }

                // Validate schema hash
                if schema_hash != self.schema_hash {
                    warn!(
                        "Schema hash mismatch: client={:016x}, server={:016x}",
                        schema_hash, self.schema_hash
                    );
                    self.reject_handshake("Schema mismatch: incompatible client version")
                        .await?;
                    return Err(anyhow::anyhow!(
                        "Schema hash mismatch: {:016x} != {:016x}",
                        schema_hash,
                        self.schema_hash
                    ));
                }

                Ok(schema_hash)
            }
            msg => {
                warn!("Expected Handshake, got {:?}", msg);
                self.reject_handshake("Expected handshake message").await?;
                Err(anyhow::anyhow!("Expected Handshake, got {:?}", msg))
            }
        }
    }

    /// Accept handshake and assign entity ID to the client.
    pub async fn accept_handshake_with_entity(&self, entity_id: u64) -> Result<()> {
        let response = ServerMessage::HandshakeResponse {
            accepted: true,
            reason: None,
            player_entity_id: Some(entity_id),
        };

        self.send_reliable(response).await
    }

    /// Reject handshake with a reason.
    async fn reject_handshake(&self, reason: &str) -> Result<()> {
        let response = ServerMessage::HandshakeResponse {
            accepted: false,
            reason: Some(reason.to_string()),
            player_entity_id: None,
        };

        self.send_reliable(response).await
    }

    /// Send a server message on the appropriate channel.
    pub async fn send(&self, msg: ServerMessage) -> Result<()> {
        let channel = select_server_channel(&msg);

        let data = encode_server_message(&msg)?;

        if channel.is_reliable() {
            self.channel_manager.send_reliable(channel, &data).await
        } else {
            self.channel_manager.send_unreliable(channel, &data).await
        }
    }

    /// Send a message on a reliable channel.
    async fn send_reliable(&self, msg: ServerMessage) -> Result<()> {
        let channel = select_server_channel(&msg);
        debug_assert!(channel.is_reliable());

        let data = encode_server_message(&msg)?;
        self.channel_manager.send_reliable(channel, &data).await
    }

    /// Receive the next client message on reliable channels.
    pub async fn recv_reliable(&self) -> Result<ClientMessage> {
        let (_channel, data) = self.channel_manager.recv_reliable().await?;
        decode_client_message(&data)
    }

    /// Receive the next client message on unreliable channels.
    pub async fn recv_unreliable(&self) -> Result<ClientMessage> {
        let (_channel, data) = self.channel_manager.recv_unreliable().await?;
        decode_client_message(&data)
    }

    /// Get the remote client address.
    pub fn remote_address(&self) -> SocketAddr {
        self.channel_manager.remote_address()
    }

    /// Close the connection gracefully.
    pub fn close(&self, reason: &str) {
        info!("Closing connection: {}", reason);
        self.channel_manager.close(reason);
    }
}

/// Select the appropriate channel for a client message.
fn select_client_channel(msg: &ClientMessage) -> ChannelType {
    match msg {
        ClientMessage::Handshake { .. } => ChannelType::Chat, // Use reliable for handshake
        ClientMessage::Input(_) => ChannelType::Input,
        ClientMessage::Chat { .. } => ChannelType::Chat,
        ClientMessage::DiagnosticsRequest => ChannelType::Diagnostics,
        ClientMessage::Disconnect { .. } => ChannelType::Chat,
    }
}

/// Select the appropriate channel for a server message.
fn select_server_channel(msg: &ServerMessage) -> ChannelType {
    match msg {
        ServerMessage::HandshakeResponse { .. } => ChannelType::Chat,
        ServerMessage::ChunkData(_) => ChannelType::ChunkStream,
        ServerMessage::EntityDelta(_) => ChannelType::EntityDelta,
        ServerMessage::Chat { .. } => ChannelType::Chat,
        ServerMessage::ServerState { .. } => ChannelType::EntityDelta,
        ServerMessage::DiagnosticsResponse { .. } => ChannelType::Diagnostics,
        ServerMessage::Disconnect { .. } => ChannelType::Chat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{ClientEndpoint, ServerEndpoint, TlsMode};
    use crate::{InputBundle, MovementInput, Transform};
    use mdminecraft_core::DimensionId;

    #[tokio::test]
    async fn test_handshake_success() {
        // Start server
        let server =
            ServerEndpoint::bind("127.0.0.1:0".parse().unwrap()).expect("Failed to bind server");
        let server_addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let incoming = server.accept().await.expect("No incoming connection");
            let connection = incoming.await.expect("Failed to accept connection");
            let server_conn = ServerConnection::new(connection);

            // Accept handshake
            server_conn
                .accept_handshake()
                .await
                .expect("Failed to accept handshake");

            // Assign entity ID
            server_conn
                .accept_handshake_with_entity(42)
                .await
                .expect("Failed to send handshake response");

            // Keep connection alive
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Small delay for server
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect client
        let client_endpoint =
            ClientEndpoint::new(TlsMode::InsecureSkipVerify).expect("Failed to create client");
        let connection = client_endpoint
            .connect(server_addr)
            .await
            .expect("Failed to connect");
        let client_conn = ClientConnection::new(connection);

        // Perform handshake
        let entity_id = client_conn.handshake().await.expect("Handshake failed");

        assert_eq!(entity_id, 42);

        // Wait for server task
        server_handle.await.expect("Server task panicked");
    }

    #[tokio::test]
    async fn test_handshake_version_mismatch() {
        // Start server
        let server =
            ServerEndpoint::bind("127.0.0.1:0".parse().unwrap()).expect("Failed to bind server");
        let server_addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let incoming = server.accept().await.expect("No incoming connection");
            let connection = incoming.await.expect("Failed to accept connection");
            let server_conn = ServerConnection::new(connection);

            // Try to accept handshake (should fail)
            let result = server_conn.accept_handshake().await;
            assert!(result.is_err());

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Small delay for server
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect client
        let client_endpoint =
            ClientEndpoint::new(TlsMode::InsecureSkipVerify).expect("Failed to create client");
        let connection = client_endpoint
            .connect(server_addr)
            .await
            .expect("Failed to connect");
        let client_conn = ClientConnection::new(connection);

        // Send handshake with wrong version
        let bad_handshake = ClientMessage::Handshake {
            version: 999,
            schema_hash: compute_schema_hash(),
        };

        client_conn
            .send_reliable(bad_handshake)
            .await
            .expect("Failed to send");

        // Try to receive response (should get rejection)
        let response = client_conn.recv_reliable().await.expect("Failed to recv");

        match response {
            ServerMessage::HandshakeResponse { accepted, .. } => {
                assert!(!accepted);
            }
            _ => panic!("Expected HandshakeResponse"),
        }

        // Wait for server task
        server_handle.await.expect("Server task panicked");
    }

    #[tokio::test]
    async fn test_input_roundtrip() {
        let server =
            ServerEndpoint::bind("127.0.0.1:0".parse().unwrap()).expect("Failed to bind server");
        let server_addr = server.local_addr();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let incoming = server.accept().await.expect("No incoming connection");
            let connection = incoming.await.expect("Failed to accept connection");
            let server_conn = ServerConnection::new(connection);

            // Handshake
            server_conn.accept_handshake().await.expect("handshake");
            server_conn
                .accept_handshake_with_entity(7)
                .await
                .expect("assign entity");

            // Receive input (unreliable channel)
            let msg = server_conn.recv_unreliable().await.expect("recv input");
            match msg {
                ClientMessage::Input(bundle) => {
                    assert_eq!(bundle.tick, 0);
                }
                other => panic!("expected input, got {other:?}"),
            }

            // Send server state (unreliable entity delta channel)
            let state = ServerMessage::ServerState {
                tick: 1,
                player_transform: Transform {
                    dimension: DimensionId::DEFAULT,
                    x: 1,
                    y: 2,
                    z: 3,
                    yaw: 4,
                    pitch: 5,
                },
            };
            server_conn.send(state).await.expect("send state");

            // Keep connection alive briefly to allow client read.
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        });

        // Small delay for server startup
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Client side
        let client_endpoint =
            ClientEndpoint::new(TlsMode::InsecureSkipVerify).expect("create client");
        let connection = client_endpoint.connect(server_addr).await.expect("connect");
        let client_conn = ClientConnection::new(connection);

        // Perform handshake
        let entity = client_conn.handshake().await.expect("handshake");
        assert_eq!(entity, 7);

        // Send input bundle
        let input = ClientMessage::Input(InputBundle {
            tick: 0,
            sequence: 0,
            last_ack_tick: 0,
            movement: MovementInput::zero(),
            block_actions: Vec::new(),
            inventory_actions: Vec::new(),
        });
        client_conn.send(input).await.expect("send input");

        // Receive server state
        let msg = client_conn.recv_unreliable().await.expect("recv state");
        match msg {
            ServerMessage::ServerState {
                tick,
                player_transform,
            } => {
                assert_eq!(tick, 1);
                assert_eq!(player_transform.x, 1);
            }
            other => panic!("expected server state, got {other:?}"),
        }

        server_handle.await.expect("server task");
    }
}
