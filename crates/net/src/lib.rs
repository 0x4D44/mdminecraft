#![warn(missing_docs)]
//! Networking abstractions shared by the client/server.

mod channel;
mod transport;

pub use channel::{ChannelManager, ChannelType};
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
