#![warn(missing_docs)]
//! Core primitives shared across the workspace.

/// Deterministic metadata components for persistence/networking.
pub mod components;
pub mod crafting;
/// Dimension identifiers shared across simulation, persistence, and networking.
pub mod dimension;
/// Enchantment types and data structures for the enchanting system.
pub mod enchantment;
pub mod item;
/// Namespaced registry keys for blocks/items/entities/tags.
pub mod registry;

use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};

// Re-export commonly used types
pub use components::{ComponentMap, ComponentValue};
pub use crafting::{Recipe, ToolRecipes};
pub use dimension::DimensionId;
pub use enchantment::{Enchantment, EnchantmentType};
pub use item::{ItemStack, ItemType, ToolMaterial, ToolType};
pub use registry::RegistryKey;

/// Fixed tick type (20 TPS => 50 ms per tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SimTick(pub u64);

impl SimTick {
    /// First tick in any deterministic timeline.
    pub const ZERO: Self = Self(0);

    /// Advance by `delta` ticks.
    pub fn advance(self, delta: u64) -> Self {
        Self(self.0 + delta)
    }
}

/// Helper to derive a reproducible RNG seeded by world + tick domains.
pub fn scoped_rng(world_seed: u64, chunk_hash: u64, tick: SimTick) -> StdRng {
    let seed = world_seed ^ chunk_hash ^ tick.0;
    StdRng::seed_from_u64(seed)
}
