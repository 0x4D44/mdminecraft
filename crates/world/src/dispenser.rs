use mdminecraft_core::ItemStack as CoreItemStack;
use serde::{Deserialize, Serialize};

/// Number of slots in a dispenser/dropper inventory.
pub const DISPENSER_SLOT_COUNT: usize = 9;

/// Persisted inventory state for dispenser-like block entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispenserState {
    pub slots: [Option<CoreItemStack>; DISPENSER_SLOT_COUNT],
    /// Transfer/activation cooldown in ticks.
    pub cooldown_ticks: u8,
    /// Last observed redstone powered state (edge-detection).
    #[serde(default)]
    pub was_powered: bool,
}

impl Default for DispenserState {
    fn default() -> Self {
        Self::new()
    }
}

impl DispenserState {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            cooldown_ticks: 0,
            was_powered: false,
        }
    }
}
