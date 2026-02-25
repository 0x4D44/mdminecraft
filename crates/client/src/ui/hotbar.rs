//! Hotbar for block selection.
//!
//! The hotbar contains 9 slots that can hold different block types.
//! The player can select which slot is active using number keys 1-9.

use mdminecraft_world::BlockId;

/// Number of slots in the hotbar.
pub const HOTBAR_SIZE: usize = 9;

/// Hotbar managing block selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Hotbar {
    /// Block IDs in each slot (0 = air/empty).
    slots: [BlockId; HOTBAR_SIZE],
    /// Currently selected slot (0-8).
    selected: usize,
}

impl Hotbar {
    /// Create a new hotbar with empty slots.
    pub fn new() -> Self {
        Self {
            slots: [0; HOTBAR_SIZE],
            selected: 0,
        }
    }

    /// Set the block ID in a specific slot.
    ///
    /// # Arguments
    /// * `index` - Slot index (0-8)
    /// * `block_id` - Block ID to place in the slot
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn set_slot(&mut self, index: usize, block_id: BlockId) {
        assert!(index < HOTBAR_SIZE, "Hotbar index out of bounds");
        self.slots[index] = block_id;
    }

    /// Get the block ID in a specific slot.
    ///
    /// # Arguments
    /// * `index` - Slot index (0-8)
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn get_slot(&self, index: usize) -> BlockId {
        assert!(index < HOTBAR_SIZE, "Hotbar index out of bounds");
        self.slots[index]
    }

    /// Get the currently selected block ID.
    pub fn get_selected(&self) -> BlockId {
        self.slots[self.selected]
    }

    /// Get the currently selected slot index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Select a specific slot.
    ///
    /// # Arguments
    /// * `index` - Slot index (0-8)
    ///
    /// # Panics
    /// Panics if index is out of bounds.
    pub fn select(&mut self, index: usize) {
        assert!(index < HOTBAR_SIZE, "Hotbar index out of bounds");
        self.selected = index;
    }

    /// Get all slots.
    pub fn slots(&self) -> &[BlockId; HOTBAR_SIZE] {
        &self.slots
    }
}

impl Default for Hotbar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotbar_creation() {
        let hotbar = Hotbar::new();
        assert_eq!(hotbar.selected_index(), 0);
        assert_eq!(hotbar.get_selected(), 0);

        for i in 0..HOTBAR_SIZE {
            assert_eq!(hotbar.get_slot(i), 0);
        }
    }

    #[test]
    fn set_slot_updates_slot() {
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(0, 1);
        hotbar.set_slot(1, 2);
        hotbar.set_slot(8, 9);

        assert_eq!(hotbar.get_slot(0), 1);
        assert_eq!(hotbar.get_slot(1), 2);
        assert_eq!(hotbar.get_slot(8), 9);
    }

    #[test]
    fn select_changes_selected_slot() {
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(0, 1);
        hotbar.set_slot(5, 5);

        assert_eq!(hotbar.selected_index(), 0);
        assert_eq!(hotbar.get_selected(), 1);

        hotbar.select(5);
        assert_eq!(hotbar.selected_index(), 5);
        assert_eq!(hotbar.get_selected(), 5);
    }

    #[test]
    fn get_selected_returns_current_slot_block() {
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(0, 10);
        hotbar.set_slot(1, 20);
        hotbar.set_slot(2, 30);

        assert_eq!(hotbar.get_selected(), 10);

        hotbar.select(1);
        assert_eq!(hotbar.get_selected(), 20);

        hotbar.select(2);
        assert_eq!(hotbar.get_selected(), 30);
    }

    #[test]
    fn slots_returns_all_slots() {
        let mut hotbar = Hotbar::new();
        for i in 0..HOTBAR_SIZE {
            hotbar.set_slot(i, (i + 1) as BlockId);
        }

        let slots = hotbar.slots();
        for i in 0..HOTBAR_SIZE {
            assert_eq!(slots[i], (i + 1) as BlockId);
        }
    }

    #[test]
    #[should_panic(expected = "Hotbar index out of bounds")]
    fn set_slot_panics_on_out_of_bounds() {
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(HOTBAR_SIZE, 1);
    }

    #[test]
    #[should_panic(expected = "Hotbar index out of bounds")]
    fn get_slot_panics_on_out_of_bounds() {
        let hotbar = Hotbar::new();
        hotbar.get_slot(HOTBAR_SIZE);
    }

    #[test]
    #[should_panic(expected = "Hotbar index out of bounds")]
    fn select_panics_on_out_of_bounds() {
        let mut hotbar = Hotbar::new();
        hotbar.select(HOTBAR_SIZE);
    }

    #[test]
    fn default_creates_empty_hotbar() {
        let hotbar = Hotbar::default();
        assert_eq!(hotbar.selected_index(), 0);
        assert_eq!(hotbar.get_selected(), 0);
    }
}
