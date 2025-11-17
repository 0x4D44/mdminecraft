#![warn(missing_docs)]
//! Core primitives shared across the workspace.

pub mod item;

use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};

// Re-export commonly used types
pub use item::{ItemStack, ItemType, ToolMaterial, ToolType};

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
