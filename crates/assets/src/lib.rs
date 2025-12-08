#![warn(missing_docs)]
//! Asset pack schema + validation helpers.

mod atlas;
mod loader;
mod registry;
mod recipe_registry;

pub use atlas::{AtlasEntry, AtlasError, TextureAtlasMetadata};
pub use loader::{registry_from_file, registry_from_str};
pub use registry::{BlockDescriptor, BlockFace, BlockRegistry, HarvestLevel};
pub use recipe_registry::RecipeRegistry;

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
    /// Atlas entry name to use for all faces (defaults to `name`).
    #[serde(default)]
    pub texture: Option<String>,
    /// Optional per-face textures.
    #[serde(default)]
    pub textures: Option<BlockTextureConfig>,
    /// Required tool tier to successfully harvest this block.
    /// None = no tool required (hand is fine)
    /// "wood" = wooden tool or better
    /// "stone" = stone tool or better
    /// "iron" = iron tool or better
    /// "diamond" = diamond tool required
    #[serde(default)]
    pub harvest_level: Option<String>,
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

/// Configuration for per-face textures.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BlockTextureConfig {
    /// Apply to all faces when specified.
    pub all: Option<String>,
    /// Apply to all side faces when specified.
    pub side: Option<String>,
    /// Specific texture for the top face.
    pub top: Option<String>,
    /// Specific texture for the bottom face.
    pub bottom: Option<String>,
    /// Specific texture for the north (-Z) face.
    pub north: Option<String>,
    /// Specific texture for the south (+Z) face.
    pub south: Option<String>,
    /// Specific texture for the east (+X) face.
    pub east: Option<String>,
    /// Specific texture for the west (-X) face.
    pub west: Option<String>,
}
