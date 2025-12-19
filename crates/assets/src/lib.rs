#![warn(missing_docs)]
//! Asset pack schema + validation helpers.

mod atlas;
mod loader;
mod recipe_registry;
mod registry;

pub use atlas::{AtlasEntry, AtlasError, TextureAtlasMetadata};
pub use loader::{
    recipe_registry_from_file, recipe_registry_from_str, registry_from_file, registry_from_str,
};
pub use recipe_registry::RecipeRegistry;
pub use recipe_registry::{parse_item_type, parse_item_type_with_blocks};
pub use registry::{BlockDescriptor, BlockFace, BlockRegistry, HarvestLevel};

use serde::Deserialize;
use thiserror::Error;

/// Minimal block definition used to sanity-check packs.
#[derive(Debug, Deserialize)]
pub struct BlockDefinition {
    /// Human-readable identifier (e.g., "stone").
    pub name: String,
    /// Stable namespaced registry key (e.g., "mdm:stone").
    ///
    /// When omitted, defaults to `mdm:<name>`.
    #[serde(default)]
    pub key: Option<String>,
    /// Optional tag keys applied to the block (e.g., "mdm:mineable/pickaxe").
    ///
    /// Tags are used by recipes, loot, and spawn rules as deterministic sets.
    #[serde(default)]
    pub tags: Vec<String>,
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
    /// Validation error when parsing registry keys.
    #[error("invalid registry key: {0}")]
    InvalidRegistryKey(String),
    /// Validation error when parsing tag keys.
    #[error("invalid tag key: {0}")]
    InvalidTagKey(String),
}

/// Parse a JSON string into a list of blocks.
pub fn load_blocks_from_str(input: &str) -> Result<Vec<BlockDefinition>, AssetError> {
    Ok(serde_json::from_str(input)?)
}

/// Recipe definition loaded from JSON config.
#[derive(Debug, Deserialize)]
pub struct RecipeDefinition {
    /// Unique identifier for this recipe (e.g., "wooden_pickaxe")
    pub name: String,
    /// List of input items required (item type string + count)
    pub inputs: Vec<RecipeInput>,
    /// Output item produced
    pub output: RecipeOutput,
}

/// Input item for a recipe.
#[derive(Debug, Deserialize)]
pub struct RecipeInput {
    /// Item type identifier (e.g., "block:planks", "item:stick", "tool:pickaxe:wood")
    pub item: String,
    /// Number of items required
    pub count: u32,
}

/// Output item from a recipe.
#[derive(Debug, Deserialize)]
pub struct RecipeOutput {
    /// Item type identifier
    pub item: String,
    /// Number of items produced (default 1)
    #[serde(default = "default_output_count")]
    pub count: u32,
}

fn default_output_count() -> u32 {
    1
}

/// Parse a JSON string into a list of recipes.
pub fn load_recipes_from_str(input: &str) -> Result<Vec<RecipeDefinition>, AssetError> {
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
