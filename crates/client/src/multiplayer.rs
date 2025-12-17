//! Multiplayer client with prediction and reconciliation.

use anyhow::{Context, Result};
use mdminecraft_core::SimTick;
use mdminecraft_core::DimensionId;
use mdminecraft_net::{
    ClientConnection, ClientEndpoint, ClientMessage, ClientPredictor, EntityInterpolator,
    InputBundle, MovementInput, ReconciliationResult, ServerMessage, ServerSnapshot, TlsMode,
    Transform,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Multiplayer client with networking and prediction.
pub struct MultiplayerClient {
    /// Network connection to server.
    connection: Option<ClientConnection>,

    /// Client predictor for rollback/replay.
    predictor: ClientPredictor,

    /// Entity interpolator for smooth remote entities.
    interpolator: EntityInterpolator,

    /// Current client tick (may be ahead of server).
    client_tick: SimTick,

    /// Player entity ID assigned by server.
    player_entity_id: Option<u64>,

    /// Current player transform (predicted).
    player_transform: Transform,

    /// Remote entity transforms.
    remote_entities: HashMap<u64, Transform>,
}

impl MultiplayerClient {
    /// Create a new multiplayer client (not yet connected).
    pub fn new() -> Self {
        Self {
            connection: None,
            predictor: ClientPredictor::new(),
            interpolator: EntityInterpolator::new(0.2), // 20% interpolation per tick
            client_tick: SimTick::ZERO,
            player_entity_id: None,
            player_transform: Transform {
                dimension: DimensionId::DEFAULT,
                x: 0,
                y: 0,
                z: 0,
                yaw: 0,
                pitch: 0,
            },
            remote_entities: HashMap::new(),
        }
    }

    /// Connect to a server at the specified address.
    pub async fn connect(&mut self, server_addr: SocketAddr) -> Result<()> {
        info!("Connecting to server at {}", server_addr);

        // Create client endpoint
        let endpoint =
            ClientEndpoint::new(TlsMode::from_env()).context("Failed to create client endpoint")?;

        // Connect to server
        let quinn_connection = endpoint
            .connect(server_addr)
            .await
            .context("Failed to connect to server")?;

        // Create client connection
        let connection = ClientConnection::new(quinn_connection);

        // Perform handshake
        let player_entity_id = connection.handshake().await.context("Handshake failed")?;

        info!(
            "Connected to server, assigned entity ID: {}",
            player_entity_id
        );

        self.player_entity_id = Some(player_entity_id);
        self.connection = Some(connection);

        Ok(())
    }

    /// Run a single client tick with prediction and network updates.
    pub async fn tick(&mut self, movement_input: MovementInput) -> Result<()> {
        // Create input bundle
        let input_bundle = InputBundle {
            tick: self.client_tick.0,
            sequence: self.client_tick.0 as u32, // Simple sequence for now
            last_ack_tick: self.predictor.last_confirmed_tick(),
            movement: movement_input.clone(),
            block_actions: Vec::new(),
            inventory_actions: Vec::new(),
        };

        // Record input for prediction
        self.predictor
            .record_input(self.client_tick.0, input_bundle.clone());

        // Send input to server
        if let Some(connection) = &self.connection {
            let msg = ClientMessage::Input(input_bundle);
            if let Err(e) = connection.send(msg).await {
                warn!("Failed to send input: {}", e);
            }
        }

        // Apply input locally (client prediction)
        self.apply_movement(&movement_input);

        // Process server messages
        self.process_server_messages().await?;

        // Interpolate remote entities
        self.update_entity_interpolation();

        self.client_tick = self.client_tick.advance(1);
        Ok(())
    }

    /// Process incoming messages from server.
    async fn process_server_messages(&mut self) -> Result<()> {
        if self.connection.is_none() {
            return Ok(());
        }

        const MAX_RELIABLE: usize = 32;
        const MAX_UNRELIABLE: usize = 32;

        // Drain reliable channel without blocking the frame.
        for _ in 0..MAX_RELIABLE {
            let connection = match &self.connection {
                Some(conn) => conn,
                None => break,
            };
            match timeout(Duration::from_millis(0), connection.recv_reliable()).await {
                Ok(Ok(msg)) => self.handle_server_message(msg)?,
                Ok(Err(e)) => {
                    warn!("Reliable channel closed: {}", e);
                    self.connection = None;
                    return Err(e);
                }
                Err(_) => break, // no more messages ready
            }
        }

        // Drain unreliable channel (datagrams).
        for _ in 0..MAX_UNRELIABLE {
            let connection = match &self.connection {
                Some(conn) => conn,
                None => break,
            };
            match timeout(Duration::from_millis(0), connection.recv_unreliable()).await {
                Ok(Ok(msg)) => self.handle_server_message(msg)?,
                Ok(Err(e)) => {
                    warn!("Unreliable channel closed: {}", e);
                    self.connection = None;
                    return Err(e);
                }
                Err(_) => break,
            }
        }

        Ok(())
    }

    /// Handle a message from the server.
    fn handle_server_message(&mut self, message: ServerMessage) -> Result<()> {
        match message {
            ServerMessage::ServerState {
                tick,
                player_transform,
            } => {
                debug!("Received server state for tick {}", tick);

                // Create server snapshot
                let snapshot = ServerSnapshot {
                    tick,
                    player_transform: player_transform.clone(),
                    entities: self.remote_entities.clone(),
                };

                // Reconcile with client prediction
                match self.predictor.reconcile(snapshot, &self.player_transform) {
                    ReconciliationResult::Match { .. } => {
                        // Prediction was correct, continue
                        debug!("Client prediction matched server state");
                    }
                    ReconciliationResult::Mismatch {
                        server_transform,
                        inputs_to_replay,
                        error_distance,
                        ..
                    } => {
                        // Prediction mismatch - rollback and replay
                        warn!(
                            "Client prediction mismatch: {} blocks error",
                            error_distance
                        );

                        // Rollback to server state
                        self.player_transform = server_transform;

                        // Replay pending inputs
                        for (_, input) in inputs_to_replay {
                            self.apply_movement(&input.movement);
                        }
                    }
                }
            }
            ServerMessage::EntityDelta(delta) => {
                debug!(
                    "Received entity delta with {} updates",
                    delta.entities.len()
                );

                // Process entity updates
                for update in delta.entities {
                    match update.update {
                        mdminecraft_net::EntityUpdateType::Spawn {
                            transform,
                            entity_type: _,
                        } => {
                            self.remote_entities
                                .insert(update.entity_id, transform.clone());
                            self.interpolator.set_target(update.entity_id, transform);
                        }
                        mdminecraft_net::EntityUpdateType::Transform(transform) => {
                            if self.remote_entities.contains_key(&update.entity_id) {
                                // Set interpolation target
                                self.interpolator
                                    .set_target(update.entity_id, transform.clone());
                            }
                            self.remote_entities.insert(update.entity_id, transform);
                        }
                        mdminecraft_net::EntityUpdateType::Despawn => {
                            self.remote_entities.remove(&update.entity_id);
                            self.interpolator.remove(update.entity_id);
                        }
                        mdminecraft_net::EntityUpdateType::Health { .. } => {
                            // Health updates don't affect rendering position
                        }
                    }
                }
            }
            ServerMessage::ChunkData(_chunk) => {
                debug!("Received chunk data");
                // TODO: Apply chunk data to world
            }
            ServerMessage::Chat { sender, text } => {
                info!("Chat from {}: {}", sender, text);
            }
            ServerMessage::Disconnect { reason } => {
                warn!("Server disconnected: {}", reason);
                self.connection = None;
            }
            _ => {
                debug!("Received unhandled server message");
            }
        }

        Ok(())
    }

    /// Apply movement input to player transform (client prediction).
    fn apply_movement(&mut self, movement: &MovementInput) {
        // Simple movement application (placeholder)
        // In full implementation, this would call physics/movement systems
        const MOVE_SPEED: i32 = 16; // 1 block per input in quantized units

        if movement.forward != 0 {
            // Move forward/backward based on yaw
            let yaw_radians =
                (self.player_transform.yaw as f32 / 256.0) * 2.0 * std::f32::consts::PI;
            let dx = (yaw_radians.sin() * movement.forward as f32 * MOVE_SPEED as f32) as i32;
            let dz = (yaw_radians.cos() * movement.forward as f32 * MOVE_SPEED as f32) as i32;

            self.player_transform.x += dx;
            self.player_transform.z += dz;
        }

        if movement.strafe != 0 {
            // Strafe left/right
            let yaw_radians =
                (self.player_transform.yaw as f32 / 256.0) * 2.0 * std::f32::consts::PI;
            let dx = (yaw_radians.cos() * movement.strafe as f32 * MOVE_SPEED as f32) as i32;
            let dz = (-yaw_radians.sin() * movement.strafe as f32 * MOVE_SPEED as f32) as i32;

            self.player_transform.x += dx;
            self.player_transform.z += dz;
        }

        // Update look direction
        self.player_transform.yaw = movement.yaw;
        self.player_transform.pitch = movement.pitch;

        // Jump (placeholder - just move up)
        if movement.jump {
            self.player_transform.y += MOVE_SPEED;
        }
    }

    /// Update entity interpolation.
    fn update_entity_interpolation(&mut self) {
        for (entity_id, current_transform) in &mut self.remote_entities {
            if let Some(interpolated) = self.interpolator.interpolate(*entity_id, current_transform)
            {
                *current_transform = interpolated;
            }
        }
    }

    /// Get current client tick.
    pub fn client_tick(&self) -> SimTick {
        self.client_tick
    }

    /// Get player entity ID.
    pub fn player_entity_id(&self) -> Option<u64> {
        self.player_entity_id
    }

    /// Get player transform.
    pub fn player_transform(&self) -> &Transform {
        &self.player_transform
    }

    /// Get remote entity transforms.
    pub fn remote_entities(&self) -> &HashMap<u64, Transform> {
        &self.remote_entities
    }

    /// Get prediction metrics.
    pub fn prediction_metrics(&self) -> &mdminecraft_net::PredictionMetrics {
        self.predictor.metrics()
    }

    /// Check if connected to server.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Disconnect from server.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(connection) = &self.connection {
            let msg = ClientMessage::Disconnect {
                reason: "Client disconnecting".to_string(),
            };
            let _ = connection.send(msg).await;
        }

        self.connection = None;
        info!("Disconnected from server");
        Ok(())
    }
}

impl Default for MultiplayerClient {
    fn default() -> Self {
        Self::new()
    }
}
