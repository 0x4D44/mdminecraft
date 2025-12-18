use mdminecraft_core::ItemStack as CoreItemStack;
use serde::{Deserialize, Serialize};

/// Number of slots in a single chest inventory (3 rows × 9 columns).
pub const CHEST_SLOT_COUNT: usize = 27;

/// Persisted inventory state for a chest block entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChestState {
    pub slots: [Option<CoreItemStack>; CHEST_SLOT_COUNT],
}

impl Default for ChestState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChestState {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }
}
