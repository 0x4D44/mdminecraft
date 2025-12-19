use mdminecraft_core::ItemStack as CoreItemStack;
use serde::{Deserialize, Serialize};

/// Number of slots in a hopper inventory.
pub const HOPPER_SLOT_COUNT: usize = 5;

/// Persisted inventory state for a hopper block entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HopperState {
    pub slots: [Option<CoreItemStack>; HOPPER_SLOT_COUNT],
    /// Transfer cooldown in ticks (vanilla-ish: 8 ticks per move).
    pub cooldown_ticks: u8,
}

impl Default for HopperState {
    fn default() -> Self {
        Self::new()
    }
}

impl HopperState {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            cooldown_ticks: 0,
        }
    }
}
