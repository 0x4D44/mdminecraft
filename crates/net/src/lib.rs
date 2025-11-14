#![warn(missing_docs)]
//! Networking abstractions shared by the client/server.

mod channel;
mod chunk_encoding;
mod chunk_streaming;
mod codec;
mod connection;
mod entity_replication;
mod prediction;
mod protocol;
mod replay;
mod transport;

pub use channel::{ChannelManager, ChannelType};
pub use chunk_encoding::{compression_ratio, decode_chunk_data, encode_chunk_data};
pub use chunk_streaming::{ChunkStreamer, StreamingMetrics};
pub use codec::{
    compute_schema_hash, decode_client_message, decode_server_message, encode_client_message,
    encode_server_message,
};
pub use connection::{ClientConnection, ServerConnection};
pub use entity_replication::{create_entity_state, EntityReplicationTracker};
pub use prediction::{
    ClientPredictor, EntityInterpolator, PredictionMetrics, ReconciliationResult, ServerSnapshot,
    SnapshotBuffer,
};
pub use protocol::{
    BlockAction, ChunkDataMessage, ClientMessage, EntityDeltaMessage, EntityUpdate,
    EntityUpdateType, InputBundle, InventoryAction, MovementInput, ServerMessage, Transform,
    PROTOCOL_MAGIC, PROTOCOL_VERSION,
};
pub use replay::{
    EventLogger, InputLogEntry, InputLogger, NetworkEvent, ReplayPlayer, ReplayValidator,
    ValidationError,
};
pub use transport::{ClientEndpoint, ServerEndpoint};

use serde::{Deserialize, Serialize};

/// Schema hash placeholder for on-the-wire compatibility checks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaHash(pub u64);

impl SchemaHash {
    /// Default development hash; replace when the protocol stabilizes.
    pub const DEV: Self = Self(0xDEADBEEFDEADBEEF);
}

/// Message envelope used by early-stage prototypes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope<T> {
    /// Schema hash to reject incompatible builds.
    pub schema: SchemaHash,
    /// Simulation tick the payload references.
    pub tick: u64,
    /// Payload data.
    pub payload: T,
}

impl<T> MessageEnvelope<T> {
    /// Wrap the payload with the development schema hash.
    pub fn dev(payload: T, tick: u64) -> Self {
        Self {
            schema: SchemaHash::DEV,
            tick,
            payload,
        }
    }
}
