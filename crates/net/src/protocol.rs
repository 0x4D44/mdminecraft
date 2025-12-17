//! Protocol message definitions for client-server communication.
//!
//! All messages use postcard serialization for compact binary encoding.

use serde::{Deserialize, Serialize};
use mdminecraft_core::DimensionId;

/// Protocol version for compatibility checking.
pub const PROTOCOL_VERSION: u16 = 2;

/// Protocol magic bytes to identify mdminecraft protocol.
pub const PROTOCOL_MAGIC: &[u8; 10] = b"MDMC\x00\x01\x00\x00\x00\x00";

/// Tick number in the simulation.
pub type SimTick = u64;

/// Entity identifier.
pub type EntityId = u64;

/// Block identifier.
pub type BlockId = u16;

/// Item identifier.
/// Maximum length of a chat message (characters).
pub const MAX_CHAT_LEN: usize = 256;

/// Maximum size of compressed chunk data (bytes).
/// 16KB is enough for typical chunks (avg ~500 bytes), allows for complex ones.
pub const MAX_CHUNK_DATA_LEN: usize = 16 * 1024;

/// Maximum number of block actions per input bundle.
/// Prevents DoS through excessive action spam.
pub const MAX_BLOCK_ACTIONS: usize = 16;

/// Maximum number of inventory actions per input bundle.
pub const MAX_INVENTORY_ACTIONS: usize = 16;

/// Maximum palette size (unique block types per chunk).
/// 256 is the max since palette indices are u8.
pub const MAX_PALETTE_SIZE: usize = 256;

/// Maximum entity updates per delta message.
pub const MAX_ENTITY_UPDATES: usize = 1024;

/// Maximum recipe ID length for crafting.
pub const MAX_RECIPE_ID_LEN: usize = 64;

/// Maximum entity type name length.
pub const MAX_ENTITY_TYPE_LEN: usize = 64;

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    /// Handshake request with protocol version and schema hash.
    Handshake {
        /// Protocol version.
        version: u16,
        /// Schema hash for compatibility.
        schema_hash: u64,
    },

    /// Input bundle containing player actions.
    Input(InputBundle),

    /// Chat message from player.
    Chat {
        /// Message text.
        text: String,
    },

    /// Request server diagnostics.
    DiagnosticsRequest,

    /// Client disconnect notification.
    Disconnect {
        /// Reason for disconnect.
        reason: String,
    },
}

impl ClientMessage {
    /// Verify message limits and validity.
    ///
    /// This should be called on all received messages to prevent DoS attacks.
    pub fn verify(&self) -> Result<(), &'static str> {
        match self {
            ClientMessage::Input(bundle) => {
                bundle.verify()?;
            }
            ClientMessage::Chat { text } => {
                if text.len() > MAX_CHAT_LEN {
                    return Err("Chat message too long");
                }
            }
            ClientMessage::Disconnect { reason } => {
                if reason.len() > 256 {
                    return Err("Disconnect reason too long");
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    /// Handshake response accepting or rejecting connection.
    HandshakeResponse {
        /// Whether handshake was accepted.
        accepted: bool,
        /// Reason for rejection (if not accepted).
        reason: Option<String>,
        /// Assigned entity ID for the player.
        player_entity_id: Option<EntityId>,
    },

    /// Chunk data for streaming to client.
    ChunkData(ChunkDataMessage),

    /// Entity delta updates.
    EntityDelta(EntityDeltaMessage),

    /// Chat message from server or another player.
    Chat {
        /// Sender name (or "Server").
        sender: String,
        /// Message text.
        text: String,
    },

    /// Server state snapshot for reconciliation.
    ServerState {
        /// Tick number for this state.
        tick: SimTick,
        /// Player entity transform.
        player_transform: Transform,
    },

    /// Diagnostics response.
    DiagnosticsResponse {
        /// Server tick rate (TPS).
        tick_rate: f32,
        /// Connected players count.
        player_count: u32,
        /// Loaded chunks count.
        chunk_count: u32,
    },

    /// Server disconnect notification.
    Disconnect {
        /// Reason for disconnect.
        reason: String,
    },
}

impl ServerMessage {
    /// Verify message limits and validity.
    ///
    /// This should be called on all received messages to prevent DoS attacks.
    pub fn verify(&self) -> Result<(), &'static str> {
        match self {
            ServerMessage::Chat { text, sender } => {
                if text.len() > MAX_CHAT_LEN {
                    return Err("Chat message too long");
                }
                if sender.len() > 32 {
                    return Err("Sender name too long");
                }
            }
            ServerMessage::ChunkData(data) => {
                data.verify()?;
            }
            ServerMessage::EntityDelta(delta) => {
                delta.verify()?;
            }
            ServerMessage::HandshakeResponse {
                reason: Some(r), ..
            } => {
                if r.len() > 256 {
                    return Err("Handshake rejection reason too long");
                }
            }
            ServerMessage::Disconnect { reason } => {
                if reason.len() > 256 {
                    return Err("Disconnect reason too long");
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Input bundle containing player actions and tick information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputBundle {
    /// Tick number when input was generated.
    pub tick: SimTick,

    /// Sequence number for ordering.
    pub sequence: u32,

    /// Last acknowledged server tick.
    pub last_ack_tick: SimTick,

    /// Movement input (compressed delta).
    pub movement: MovementInput,

    /// Block interaction actions.
    pub block_actions: Vec<BlockAction>,

    /// Inventory operations.
    pub inventory_actions: Vec<InventoryAction>,
}

impl InputBundle {
    /// Verify input bundle limits and validity.
    ///
    /// Returns an error if any limits are exceeded, preventing DoS attacks.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.block_actions.len() > MAX_BLOCK_ACTIONS {
            return Err("Too many block actions");
        }
        if self.inventory_actions.len() > MAX_INVENTORY_ACTIONS {
            return Err("Too many inventory actions");
        }

        // Validate inventory actions
        for action in &self.inventory_actions {
            if let InventoryAction::Craft { recipe_id } = action {
                if recipe_id.len() > MAX_RECIPE_ID_LEN {
                    return Err("Recipe ID too long");
                }
            }
        }

        // Validate movement values are in expected range
        if self.movement.forward < -1 || self.movement.forward > 1 {
            return Err("Invalid forward movement value");
        }
        if self.movement.strafe < -1 || self.movement.strafe > 1 {
            return Err("Invalid strafe movement value");
        }

        Ok(())
    }
}

/// Movement input with delta compression.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MovementInput {
    /// Forward/backward (-1.0 to 1.0).
    pub forward: i8,

    /// Left/right (-1.0 to 1.0).
    pub strafe: i8,

    /// Jump flag.
    pub jump: bool,

    /// Sprint flag.
    pub sprint: bool,

    /// Yaw angle (quantized to 256 steps).
    pub yaw: u8,

    /// Pitch angle (quantized to 256 steps).
    pub pitch: u8,
}

impl MovementInput {
    /// Create a zero movement input (no movement).
    pub fn zero() -> Self {
        Self {
            forward: 0,
            strafe: 0,
            jump: false,
            sprint: false,
            yaw: 0,
            pitch: 0,
        }
    }
}

/// Block interaction action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlockAction {
    /// Place a block at position.
    Place {
        /// Block position (world coordinates).
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Z coordinate.
        z: i32,
        /// Block ID to place.
        block_id: BlockId,
    },

    /// Break a block at position.
    Break {
        /// Block position (world coordinates).
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Z coordinate.
        z: i32,
    },

    /// Interact with a block (e.g., open chest).
    Interact {
        /// Block position (world coordinates).
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Z coordinate.
        z: i32,
    },
}

/// Inventory operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InventoryAction {
    /// Move item between slots.
    Move {
        /// Source slot index.
        from_slot: u8,
        /// Destination slot index.
        to_slot: u8,
        /// Amount to move.
        amount: u8,
    },

    /// Drop item from slot.
    Drop {
        /// Slot index.
        slot: u8,
        /// Amount to drop.
        amount: u8,
    },

    /// Craft item using recipe.
    Craft {
        /// Recipe ID.
        recipe_id: String,
    },
}

/// Chunk data message with delta compression.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkDataMessage {
    /// Dimension this chunk belongs to.
    pub dimension: DimensionId,
    /// Chunk X coordinate.
    pub chunk_x: i32,

    /// Chunk Z coordinate.
    pub chunk_z: i32,

    /// Palette of unique block IDs in this chunk.
    pub palette: Vec<BlockId>,

    /// RLE-compressed palette indices.
    pub compressed_data: Vec<u8>,

    /// CRC32 checksum for validation.
    pub crc32: u32,
}

impl ChunkDataMessage {
    /// Verify chunk data message limits.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.palette.len() > MAX_PALETTE_SIZE {
            return Err("Palette too large");
        }
        if self.compressed_data.len() > MAX_CHUNK_DATA_LEN {
            return Err("Chunk data too large");
        }
        Ok(())
    }
}

/// Entity delta update message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityDeltaMessage {
    /// Tick number for this update.
    pub tick: SimTick,

    /// Entity updates.
    pub entities: Vec<EntityUpdate>,
}

impl EntityDeltaMessage {
    /// Verify entity delta message limits.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.entities.len() > MAX_ENTITY_UPDATES {
            return Err("Too many entity updates");
        }

        // Validate each entity update
        for update in &self.entities {
            update.verify()?;
        }

        Ok(())
    }
}

/// Update for a single entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityUpdate {
    /// Entity ID.
    pub entity_id: EntityId,

    /// Entity update type.
    pub update: EntityUpdateType,
}

impl EntityUpdate {
    /// Verify entity update limits.
    pub fn verify(&self) -> Result<(), &'static str> {
        if let EntityUpdateType::Spawn { entity_type, .. } = &self.update {
            if entity_type.len() > MAX_ENTITY_TYPE_LEN {
                return Err("Entity type name too long");
            }
        }
        Ok(())
    }
}

/// Type of entity update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityUpdateType {
    /// Entity spawned.
    Spawn {
        /// Entity transform.
        transform: Transform,
        /// Entity type name.
        entity_type: String,
    },

    /// Entity despawned.
    Despawn,

    /// Transform update (delta-encoded).
    Transform(Transform),

    /// Health update.
    Health {
        /// Current health.
        current: f32,
        /// Maximum health.
        max: f32,
    },
}

/// Entity transform with quantized values for network efficiency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    /// Dimension this transform belongs to.
    pub dimension: DimensionId,
    /// X position (quantized to 1/16 block precision).
    pub x: i32,

    /// Y position (quantized to 1/16 block precision).
    pub y: i32,

    /// Z position (quantized to 1/16 block precision).
    pub z: i32,

    /// Yaw rotation (quantized to 256 steps).
    pub yaw: u8,

    /// Pitch rotation (quantized to 256 steps).
    pub pitch: u8,
}

impl Transform {
    /// Create transform from floating-point position and rotation in [`DimensionId::DEFAULT`].
    ///
    /// Position is quantized to 1/16 block precision.
    /// Rotation is quantized to 256 steps (0-255 = 0-360 degrees).
    pub fn from_f32(x: f32, y: f32, z: f32, yaw: f32, pitch: f32) -> Self {
        Self::from_f32_in_dimension(DimensionId::DEFAULT, x, y, z, yaw, pitch)
    }

    /// Create transform from floating-point position and rotation in a specific dimension.
    ///
    /// Position is quantized to 1/16 block precision.
    /// Rotation is quantized to 256 steps (0-255 = 0-360 degrees).
    pub fn from_f32_in_dimension(
        dimension: DimensionId,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        pitch: f32,
    ) -> Self {
        Self {
            dimension,
            x: (x * 16.0) as i32,
            y: (y * 16.0) as i32,
            z: (z * 16.0) as i32,
            yaw: ((yaw / 360.0 * 256.0) as i32 & 0xFF) as u8,
            pitch: ((pitch / 360.0 * 256.0) as i32 & 0xFF) as u8,
        }
    }

    /// Convert to floating-point position and rotation.
    pub fn to_f32(&self) -> (f32, f32, f32, f32, f32) {
        (
            self.x as f32 / 16.0,
            self.y as f32 / 16.0,
            self.z as f32 / 16.0,
            self.yaw as f32 / 256.0 * 360.0,
            self.pitch as f32 / 256.0 * 360.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_quantization() {
        let transform = Transform::from_f32(10.5, 64.0, -5.25, 90.0, -45.0);
        assert_eq!(transform.dimension, DimensionId::DEFAULT);
        let (x, y, z, yaw, pitch) = transform.to_f32();

        // Check precision (within 1/16 block)
        assert!((x - 10.5).abs() < 0.1);
        assert!((y - 64.0).abs() < 0.1);
        assert!((z - (-5.25)).abs() < 0.1);

        // Check rotation (within 1/256 of 360 degrees)
        assert!((yaw - 90.0).abs() < 2.0);
        assert!((pitch - (-45.0 + 360.0)).abs() < 2.0); // Pitch wraps around
    }

    #[test]
    fn test_movement_input_zero() {
        let input = MovementInput::zero();
        assert_eq!(input.forward, 0);
        assert_eq!(input.strafe, 0);
        assert!(!input.jump);
        assert!(!input.sprint);
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::Handshake {
            version: PROTOCOL_VERSION,
            schema_hash: 0xDEADBEEF,
        };

        let encoded = postcard::to_allocvec(&msg).expect("Failed to encode");
        let decoded: ClientMessage = postcard::from_bytes(&encoded).expect("Failed to decode");

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::HandshakeResponse {
            accepted: true,
            reason: None,
            player_entity_id: Some(42),
        };

        let encoded = postcard::to_allocvec(&msg).expect("Failed to encode");
        let decoded: ServerMessage = postcard::from_bytes(&encoded).expect("Failed to decode");

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_input_bundle_serialization() {
        let bundle = InputBundle {
            tick: 1000,
            sequence: 42,
            last_ack_tick: 995,
            movement: MovementInput::zero(),
            block_actions: vec![BlockAction::Place {
                x: 10,
                y: 64,
                z: -5,
                block_id: 1,
            }],
            inventory_actions: vec![],
        };

        let encoded = postcard::to_allocvec(&bundle).expect("Failed to encode");
        let decoded: InputBundle = postcard::from_bytes(&encoded).expect("Failed to decode");

        assert_eq!(bundle, decoded);
    }

    // === Validation Tests ===

    #[test]
    fn test_valid_input_bundle() {
        let bundle = InputBundle {
            tick: 1000,
            sequence: 42,
            last_ack_tick: 995,
            movement: MovementInput::zero(),
            block_actions: vec![],
            inventory_actions: vec![],
        };
        assert!(bundle.verify().is_ok());
    }

    #[test]
    fn test_input_bundle_too_many_block_actions() {
        let mut bundle = InputBundle {
            tick: 1000,
            sequence: 42,
            last_ack_tick: 995,
            movement: MovementInput::zero(),
            block_actions: vec![],
            inventory_actions: vec![],
        };

        // Add more than MAX_BLOCK_ACTIONS
        for i in 0..(MAX_BLOCK_ACTIONS + 1) {
            bundle.block_actions.push(BlockAction::Break {
                x: i as i32,
                y: 64,
                z: 0,
            });
        }

        assert!(bundle.verify().is_err());
        assert_eq!(bundle.verify().unwrap_err(), "Too many block actions");
    }

    #[test]
    fn test_input_bundle_too_many_inventory_actions() {
        let mut bundle = InputBundle {
            tick: 1000,
            sequence: 42,
            last_ack_tick: 995,
            movement: MovementInput::zero(),
            block_actions: vec![],
            inventory_actions: vec![],
        };

        // Add more than MAX_INVENTORY_ACTIONS
        for i in 0..(MAX_INVENTORY_ACTIONS + 1) {
            bundle.inventory_actions.push(InventoryAction::Drop {
                slot: i as u8,
                amount: 1,
            });
        }

        assert!(bundle.verify().is_err());
        assert_eq!(bundle.verify().unwrap_err(), "Too many inventory actions");
    }

    #[test]
    fn test_input_bundle_recipe_id_too_long() {
        let bundle = InputBundle {
            tick: 1000,
            sequence: 42,
            last_ack_tick: 995,
            movement: MovementInput::zero(),
            block_actions: vec![],
            inventory_actions: vec![InventoryAction::Craft {
                recipe_id: "x".repeat(MAX_RECIPE_ID_LEN + 1),
            }],
        };

        assert!(bundle.verify().is_err());
        assert_eq!(bundle.verify().unwrap_err(), "Recipe ID too long");
    }

    #[test]
    fn test_chat_message_too_long() {
        let msg = ClientMessage::Chat {
            text: "x".repeat(MAX_CHAT_LEN + 1),
        };
        assert!(msg.verify().is_err());
        assert_eq!(msg.verify().unwrap_err(), "Chat message too long");
    }

    #[test]
    fn test_valid_chat_message() {
        let msg = ClientMessage::Chat {
            text: "Hello, world!".to_string(),
        };
        assert!(msg.verify().is_ok());
    }

    #[test]
    fn test_chunk_data_palette_too_large() {
        let msg = ChunkDataMessage {
            dimension: DimensionId::DEFAULT,
            chunk_x: 0,
            chunk_z: 0,
            palette: vec![0u16; MAX_PALETTE_SIZE + 1],
            compressed_data: vec![],
            crc32: 0,
        };
        assert!(msg.verify().is_err());
        assert_eq!(msg.verify().unwrap_err(), "Palette too large");
    }

    #[test]
    fn test_chunk_data_too_large() {
        let msg = ChunkDataMessage {
            dimension: DimensionId::DEFAULT,
            chunk_x: 0,
            chunk_z: 0,
            palette: vec![],
            compressed_data: vec![0u8; MAX_CHUNK_DATA_LEN + 1],
            crc32: 0,
        };
        assert!(msg.verify().is_err());
        assert_eq!(msg.verify().unwrap_err(), "Chunk data too large");
    }

    #[test]
    fn test_entity_delta_too_many_updates() {
        let msg = EntityDeltaMessage {
            tick: 1000,
            entities: (0..(MAX_ENTITY_UPDATES + 1) as u64)
                .map(|id| EntityUpdate {
                    entity_id: id,
                    update: EntityUpdateType::Despawn,
                })
                .collect(),
        };
        assert!(msg.verify().is_err());
        assert_eq!(msg.verify().unwrap_err(), "Too many entity updates");
    }

    #[test]
    fn test_entity_type_name_too_long() {
        let update = EntityUpdate {
            entity_id: 1,
            update: EntityUpdateType::Spawn {
                transform: Transform::from_f32(0.0, 0.0, 0.0, 0.0, 0.0),
                entity_type: "x".repeat(MAX_ENTITY_TYPE_LEN + 1),
            },
        };
        assert!(update.verify().is_err());
        assert_eq!(update.verify().unwrap_err(), "Entity type name too long");
    }

    #[test]
    fn test_constants_values() {
        assert_eq!(MAX_CHAT_LEN, 256);
        assert_eq!(MAX_CHUNK_DATA_LEN, 16 * 1024);
        assert_eq!(MAX_BLOCK_ACTIONS, 16);
        assert_eq!(MAX_INVENTORY_ACTIONS, 16);
        assert_eq!(MAX_PALETTE_SIZE, 256);
        assert_eq!(MAX_ENTITY_UPDATES, 1024);
        assert_eq!(MAX_RECIPE_ID_LEN, 64);
        assert_eq!(MAX_ENTITY_TYPE_LEN, 64);
    }
}
