#![warn(missing_docs)]
//! Asset pack schema + validation helpers.

mod loader;
mod registry;

pub use loader::{registry_from_file, registry_from_str};
pub use registry::{BlockDescriptor, BlockRegistry};

use serde::Deserialize;
use thiserror::Error;

/// Minimal block definition used to sanity-check packs.
#[derive(Debug, Deserialize)]
pub struct BlockDefinition {
    /// Human-readable identifier (e.g., "stone").
    pub name: String,
    /// Whether the block is opaque.
    #[serde(default)]
    pub opaque: bool,
}

/// Errors emitted during pack loading.
#[derive(Debug, Error)]
pub enum AssetError {
    /// Wrap IO errors when reading packs.
    #[error("failed to read asset pack: {0}")]
    Io(#[from] std::io::Error),
    /// Wrap serde parsing issues.
    #[error("failed to parse asset pack: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Parse a JSON string into a list of blocks.
pub fn load_blocks_from_str(input: &str) -> Result<Vec<BlockDefinition>, AssetError> {
    Ok(serde_json::from_str(input)?)
}
