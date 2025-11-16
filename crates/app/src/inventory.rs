//! Player inventory system.

/// Maximum number of items in a single stack.
pub const MAX_STACK_SIZE: u32 = 64;

/// Number of hotbar slots.
pub const HOTBAR_SIZE: usize = 9;

/// Total inventory slots (9 hotbar + 27 main + 4 armor).
pub const TOTAL_SLOTS: usize = 40;

/// Inventory slot containing an item stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemStack {
    pub item_id: u16,
    pub count: u32,
}

impl ItemStack {
    pub fn new(item_id: u16, count: u32) -> Self {
        Self { item_id, count }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn is_full(&self) -> bool {
        self.count >= MAX_STACK_SIZE
    }

    pub fn can_stack_with(&self, other_id: u16) -> bool {
        self.item_id == other_id && !self.is_full()
    }
}

/// Player inventory with hotbar, main inventory, and armor slots.
#[derive(Debug, Clone)]
pub struct Inventory {
    /// All inventory slots (hotbar is slots 0-8).
    slots: [Option<ItemStack>; TOTAL_SLOTS],

    /// Currently selected hotbar slot (0-8).
    selected_hotbar_slot: usize,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: [None; TOTAL_SLOTS],
            selected_hotbar_slot: 0,
        }
    }

    /// Get the currently selected hotbar slot index (0-8).
    pub fn selected_slot(&self) -> usize {
        self.selected_hotbar_slot
    }

    /// Set the selected hotbar slot (0-8).
    pub fn set_selected_slot(&mut self, slot: usize) {
        if slot < HOTBAR_SIZE {
            self.selected_hotbar_slot = slot;
        }
    }

    /// Get the item in the currently selected hotbar slot.
    pub fn selected_item(&self) -> Option<ItemStack> {
        self.slots[self.selected_hotbar_slot]
    }

    /// Get the block ID in the selected hotbar slot (for placing).
    pub fn selected_block_id(&self) -> Option<u16> {
        self.selected_item().map(|stack| stack.item_id)
    }

    /// Get the item stack at a specific slot.
    pub fn get_slot(&self, slot: usize) -> Option<ItemStack> {
        if slot < TOTAL_SLOTS {
            self.slots[slot]
        } else {
            None
        }
    }

    /// Get the hotbar slots (0-8).
    pub fn hotbar_slots(&self) -> &[Option<ItemStack>] {
        &self.slots[0..HOTBAR_SIZE]
    }

    /// Add an item to the inventory. Returns the number of items that couldn't fit.
    pub fn add_item(&mut self, item_id: u16, mut count: u32) -> u32 {
        // Try to stack with existing items first
        for slot in &mut self.slots {
            if let Some(stack) = slot {
                if stack.can_stack_with(item_id) {
                    let space = MAX_STACK_SIZE - stack.count;
                    let to_add = count.min(space);
                    stack.count += to_add;
                    count -= to_add;

                    if count == 0 {
                        return 0;
                    }
                }
            }
        }

        // Try to add to empty slots
        for slot in &mut self.slots {
            if slot.is_none() {
                let to_add = count.min(MAX_STACK_SIZE);
                *slot = Some(ItemStack::new(item_id, to_add));
                count -= to_add;

                if count == 0 {
                    return 0;
                }
            }
        }

        // Return remaining count that didn't fit
        count
    }

    /// Remove items from inventory. Returns the number of items actually removed.
    pub fn remove_item(&mut self, item_id: u16, mut count: u32) -> u32 {
        let mut removed = 0;

        for slot in &mut self.slots {
            if let Some(stack) = slot {
                if stack.item_id == item_id {
                    let to_remove = count.min(stack.count);
                    stack.count -= to_remove;
                    removed += to_remove;
                    count -= to_remove;

                    if stack.count == 0 {
                        *slot = None;
                    }

                    if count == 0 {
                        break;
                    }
                }
            }
        }

        removed
    }

    /// Check if inventory has at least the specified count of an item.
    pub fn has_item(&self, item_id: u16, count: u32) -> bool {
        let mut total = 0;
        for slot in &self.slots {
            if let Some(stack) = slot {
                if stack.item_id == item_id {
                    total += stack.count;
                    if total >= count {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_item() {
        let mut inv = Inventory::new();

        // Add 32 stone
        assert_eq!(inv.add_item(1, 32), 0);
        assert_eq!(inv.get_slot(0), Some(ItemStack::new(1, 32)));

        // Add 32 more stone (should stack)
        assert_eq!(inv.add_item(1, 32), 0);
        assert_eq!(inv.get_slot(0), Some(ItemStack::new(1, 64)));

        // Add 1 more stone (should go to new slot since first is full)
        assert_eq!(inv.add_item(1, 1), 0);
        assert_eq!(inv.get_slot(1), Some(ItemStack::new(1, 1)));
    }

    #[test]
    fn test_remove_item() {
        let mut inv = Inventory::new();
        inv.add_item(1, 64);
        inv.add_item(1, 32);

        // Remove 80 stone
        assert_eq!(inv.remove_item(1, 80), 80);
        assert_eq!(inv.get_slot(0), Some(ItemStack::new(1, 16)));
        assert_eq!(inv.get_slot(1), None);
    }

    #[test]
    fn test_selected_slot() {
        let mut inv = Inventory::new();
        inv.add_item(2, 10);

        assert_eq!(inv.selected_slot(), 0);
        assert_eq!(inv.selected_block_id(), Some(2));

        inv.set_selected_slot(3);
        assert_eq!(inv.selected_slot(), 3);
        assert_eq!(inv.selected_block_id(), None);
    }
}
