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

/// Simulation timekeeping that tracks absolute ticks and cyclic day phases.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SimTime {
    tick: SimTick,
    day_ticks: u64,
}

impl SimTime {
    /// Default duration of a full day/night cycle (in simulation ticks).
    pub const DEFAULT_DAY_TICKS: u64 = 24_000;

    /// Construct a new `SimTime` starting at tick zero.
    pub fn new(day_ticks: u64) -> Self {
        Self {
            tick: SimTick::ZERO,
            day_ticks: day_ticks.max(1),
        }
    }

    /// Construct a `SimTime` from an absolute tick.
    pub fn from_tick(day_ticks: u64, tick: SimTick) -> Self {
        Self {
            tick,
            day_ticks: day_ticks.max(1),
        }
    }

    /// Advance the simulation clock by `delta_ticks`.
    pub fn advance(&mut self, delta_ticks: u64) {
        self.tick = self.tick.advance(delta_ticks);
    }

    /// Absolute simulation tick.
    pub fn tick(&self) -> SimTick {
        self.tick
    }

    /// Fractional time-of-day in the range [0, 1).
    pub fn time_of_day(&self) -> f32 {
        let phase = self.tick.0 % self.day_ticks;
        phase as f32 / self.day_ticks as f32
    }

    /// Update the day-length at runtime (used when loading worlds with different configs).
    pub fn set_day_ticks(&mut self, day_ticks: u64) {
        self.day_ticks = day_ticks.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_time_wraps_within_day_cycle() {
        let mut time = SimTime::new(24000);
        time.advance(6000);
        assert!((time.time_of_day() - 0.25).abs() < f32::EPSILON);
        time.advance(18000);
        assert!((time.time_of_day() - 0.0).abs() < f32::EPSILON);
    }
}
