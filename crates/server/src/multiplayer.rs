//! Multiplayer server with network replication.

use anyhow::{Context, Result};
use bevy_ecs::schedule::Schedules;
use bevy_ecs::world::World;
use mdminecraft_core::DimensionId;
use mdminecraft_core::SimTick;
use mdminecraft_ecs::{build_default_schedule, run_tick};
use mdminecraft_net::{
    ChunkStreamer, EntityReplicationTracker, EventLogger, InputLogger, NetworkEvent,
    ServerConnection, ServerEndpoint, ServerMessage, Transform,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{debug, info, instrument, warn};

/// Client state tracked by the server.
pub struct ConnectedClient {
    /// Network connection to client.
    connection: ServerConnection,

    /// Player entity ID assigned to this client.
    player_entity_id: u64,

    /// Chunk streamer for this client.
    chunk_streamer: ChunkStreamer,

    /// Entity replication tracker.
    entity_tracker: EntityReplicationTracker,

    /// Last tick acknowledged by client.
    last_ack_tick: SimTick,
}

/// Multiplayer server with networking.
pub struct MultiplayerServer {
    /// Core server state.
    world: World,
    schedules: Schedules,
    current_tick: SimTick,

    /// Network transport.
    endpoint: ServerEndpoint,

    /// Connected clients indexed by socket address.
    clients: HashMap<SocketAddr, ConnectedClient>,

    /// Next entity ID to assign.
    next_entity_id: u64,

    /// Optional input logger for replay.
    input_logger: Option<InputLogger>,

    /// Optional event logger for replay.
    event_logger: Option<EventLogger>,
}

impl MultiplayerServer {
    /// Create a new multiplayer server bound to the specified address.
    pub fn bind(addr: SocketAddr) -> Result<Self> {
        let endpoint = ServerEndpoint::bind(addr).context("Failed to bind server endpoint")?;

        let local_addr = endpoint.local_addr();
        info!("Multiplayer server bound to {}", local_addr);

        Ok(Self {
            world: World::default(),
            schedules: build_default_schedule(),
            current_tick: SimTick::ZERO,
            endpoint,
            clients: HashMap::new(),
            next_entity_id: 1,
            input_logger: None,
            event_logger: None,
        })
    }

    /// Enable replay logging to specified directory.
    pub fn enable_replay_logging(&mut self, log_dir: PathBuf) -> Result<()> {
        std::fs::create_dir_all(&log_dir)?;

        let input_log_path = log_dir.join("inputs.jsonl");
        let event_log_path = log_dir.join("events.jsonl");

        self.input_logger = Some(InputLogger::create(input_log_path)?);
        self.event_logger = Some(EventLogger::create(event_log_path)?);

        info!("Replay logging enabled to {:?}", log_dir);
        Ok(())
    }

    /// Run a single simulation tick with network updates.
    ///
    /// This is a simplified version that advances the simulation and sends
    /// state updates to connected clients. Full implementation would:
    /// - Accept new connections asynchronously
    /// - Process client inputs
    /// - Apply inputs to player entities
    /// - Stream chunks based on player position
    /// - Send entity deltas
    #[instrument(skip(self), fields(tick = self.current_tick.0, client_count = self.clients.len()))]
    pub async fn tick(&mut self) -> Result<()> {
        debug!("Running server tick");

        // Run deterministic simulation
        run_tick(&mut self.world, &mut self.schedules, self.current_tick);

        // Send server state to all clients
        for client in self.clients.values_mut() {
            // Send server state update
            let state_message = ServerMessage::ServerState {
                tick: self.current_tick.0,
                player_transform: Transform {
                    dimension: DimensionId::DEFAULT,
                    x: 0,
                    y: 0,
                    z: 0,
                    yaw: 0,
                    pitch: 0,
                },
            };

            if let Err(e) = client.connection.send(state_message).await {
                warn!("Failed to send state update: {}", e);
            }
        }

        // Flush replay logs
        if let Some(input_logger) = &mut self.input_logger {
            input_logger.flush()?;
        }
        if let Some(event_logger) = &mut self.event_logger {
            event_logger.flush()?;
        }

        self.current_tick = self.current_tick.advance(1);
        Ok(())
    }

    /// Accept a new client connection.
    ///
    /// This should be called in a loop to handle incoming connections.
    #[instrument(skip(self))]
    pub async fn accept_client(&mut self) -> Result<()> {
        if let Some(incoming) = self.endpoint.accept().await {
            let addr = incoming.remote_address();
            info!("New connection from {}", addr);

            match incoming.await {
                Ok(quinn_connection) => {
                    self.handle_new_client(addr, quinn_connection).await?;
                }
                Err(e) => {
                    warn!("Failed to establish connection from {}: {}", addr, e);
                }
            }
        }
        Ok(())
    }

    /// Handle a newly connected client.
    #[instrument(skip(self, quinn_connection), fields(addr = %addr))]
    async fn handle_new_client(
        &mut self,
        addr: SocketAddr,
        quinn_connection: quinn::Connection,
    ) -> Result<()> {
        debug!("Processing new client handshake");
        let connection = ServerConnection::new(quinn_connection);

        // Perform handshake
        match connection.accept_handshake().await {
            Ok(_schema_hash) => {
                // Assign player entity
                let player_entity_id = self.next_entity_id;
                self.next_entity_id += 1;

                info!(player_entity_id, "Client authenticated successfully");

                // Create client state
                let client = ConnectedClient {
                    connection,
                    player_entity_id,
                    chunk_streamer: ChunkStreamer::new(),
                    entity_tracker: EntityReplicationTracker::new(8), // 8 chunk view distance
                    last_ack_tick: self.current_tick,
                };

                self.clients.insert(addr, client);

                // Log spawn event
                if let Some(event_logger) = &mut self.event_logger {
                    event_logger.log(NetworkEvent::PlayerPosition {
                        tick: self.current_tick.0,
                        player_id: player_entity_id,
                        transform: Transform {
                            dimension: DimensionId::DEFAULT,
                            x: 0,
                            y: 0,
                            z: 0,
                            yaw: 0,
                            pitch: 0,
                        },
                    })?;
                }
            }
            Err(e) => {
                warn!("Handshake failed for {}: {}", addr, e);
            }
        }

        Ok(())
    }

    /// Log an input entry for replay.
    pub fn log_input(&mut self, player_id: u64, input: mdminecraft_net::InputBundle) -> Result<()> {
        if let Some(input_logger) = &mut self.input_logger {
            input_logger.log(self.current_tick.0, player_id, input)?;
        }
        Ok(())
    }

    /// Log a network event for replay.
    pub fn log_event(&mut self, event: NetworkEvent) -> Result<()> {
        if let Some(event_logger) = &mut self.event_logger {
            event_logger.log(event)?;
        }
        Ok(())
    }

    /// Get current tick.
    pub fn current_tick(&self) -> SimTick {
        self.current_tick
    }

    /// Get number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get local address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.endpoint.local_addr()
    }

    /// Get reference to connected clients.
    pub fn clients(&self) -> &HashMap<SocketAddr, ConnectedClient> {
        &self.clients
    }

    /// Get mutable reference to connected clients.
    pub fn clients_mut(&mut self) -> &mut HashMap<SocketAddr, ConnectedClient> {
        &mut self.clients
    }
}

impl ConnectedClient {
    /// Get player entity ID.
    pub fn player_entity_id(&self) -> u64 {
        self.player_entity_id
    }

    /// Get reference to connection.
    pub fn connection(&self) -> &ServerConnection {
        &self.connection
    }

    /// Get mutable reference to connection.
    pub fn connection_mut(&mut self) -> &mut ServerConnection {
        &mut self.connection
    }

    /// Get reference to chunk streamer.
    pub fn chunk_streamer(&self) -> &ChunkStreamer {
        &self.chunk_streamer
    }

    /// Get mutable reference to chunk streamer.
    pub fn chunk_streamer_mut(&mut self) -> &mut ChunkStreamer {
        &mut self.chunk_streamer
    }

    /// Get reference to entity tracker.
    pub fn entity_tracker(&self) -> &EntityReplicationTracker {
        &self.entity_tracker
    }

    /// Get mutable reference to entity tracker.
    pub fn entity_tracker_mut(&mut self) -> &mut EntityReplicationTracker {
        &mut self.entity_tracker
    }

    /// Get last acknowledged tick.
    pub fn last_ack_tick(&self) -> SimTick {
        self.last_ack_tick
    }

    /// Set last acknowledged tick.
    pub fn set_last_ack_tick(&mut self, tick: SimTick) {
        self.last_ack_tick = tick;
    }
}
