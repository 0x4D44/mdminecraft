//! Inventory system for player and container storage.
//!
//! Provides 36-slot inventory with ItemStack management including
//! stack merging, splitting, and slot validation.

use serde::{Deserialize, Serialize};

/// Item identifier referencing the item registry.
pub type ItemId = u16;

/// Maximum stack size for most items.
pub const DEFAULT_STACK_SIZE: u8 = 64;

/// Number of slots in player inventory.
pub const INVENTORY_SIZE: usize = 36;

/// Represents a stack of items in an inventory slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    /// Item type identifier.
    pub item_id: ItemId,
    /// Number of items in this stack (1-64 typically).
    pub count: u8,
    /// Optional item metadata (for damage, enchantments, etc.).
    pub metadata: Option<Vec<u8>>,
}

impl ItemStack {
    /// Create a new item stack.
    pub fn new(item_id: ItemId, count: u8) -> Self {
        Self {
            item_id,
            count,
            metadata: None,
        }
    }

    /// Create an item stack with metadata.
    pub fn with_metadata(item_id: ItemId, count: u8, metadata: Vec<u8>) -> Self {
        Self {
            item_id,
            count,
            metadata: Some(metadata),
        }
    }

    /// Check if this stack can merge with another stack.
    pub fn can_merge(&self, other: &ItemStack) -> bool {
        self.item_id == other.item_id && self.metadata == other.metadata
    }

    /// Get the maximum stack size for this item (future: query from registry).
    pub fn max_stack_size(&self) -> u8 {
        DEFAULT_STACK_SIZE
    }

    /// Check if this stack is at max capacity.
    pub fn is_full(&self) -> bool {
        self.count >= self.max_stack_size()
    }

    /// Get remaining space in this stack.
    pub fn remaining_space(&self) -> u8 {
        self.max_stack_size().saturating_sub(self.count)
    }

    /// Try to add items to this stack, returning the amount that didn't fit.
    pub fn add(&mut self, amount: u8) -> u8 {
        let space = self.remaining_space();
        let added = amount.min(space);
        self.count += added;
        amount - added
    }

    /// Try to remove items from this stack, returning the amount actually removed.
    pub fn remove(&mut self, amount: u8) -> u8 {
        let removed = amount.min(self.count);
        self.count -= removed;
        removed
    }

    /// Split this stack, taking the specified amount into a new stack.
    pub fn split(&mut self, amount: u8) -> Option<ItemStack> {
        if amount == 0 || amount > self.count {
            return None;
        }

        self.count -= amount;
        Some(ItemStack {
            item_id: self.item_id,
            count: amount,
            metadata: self.metadata.clone(),
        })
    }
}

/// Player or container inventory with multiple slots.
#[derive(Debug, Clone)]
pub struct Inventory {
    slots: [Option<ItemStack>; INVENTORY_SIZE],
}

impl Serialize for Inventory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(INVENTORY_SIZE))?;
        for slot in &self.slots {
            seq.serialize_element(slot)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Inventory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let slots: Vec<Option<ItemStack>> = Vec::deserialize(deserializer)?;
        if slots.len() != INVENTORY_SIZE {
            return Err(serde::de::Error::custom(format!(
                "Expected {} slots, got {}",
                INVENTORY_SIZE,
                slots.len()
            )));
        }

        let slots_array: [Option<ItemStack>; INVENTORY_SIZE] = slots
            .try_into()
            .map_err(|_| serde::de::Error::custom("Failed to convert to array"))?;

        Ok(Inventory { slots: slots_array })
    }
}

impl Inventory {
    /// Create a new empty inventory.
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }

    /// Get an item stack from a slot.
    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        if slot >= INVENTORY_SIZE {
            return None;
        }
        self.slots[slot].as_ref()
    }

    /// Get a mutable reference to an item stack in a slot.
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut ItemStack> {
        if slot >= INVENTORY_SIZE {
            return None;
        }
        self.slots[slot].as_mut()
    }

    /// Set an item stack in a slot.
    pub fn set(&mut self, slot: usize, stack: Option<ItemStack>) -> bool {
        if slot >= INVENTORY_SIZE {
            return false;
        }
        self.slots[slot] = stack;
        true
    }

    /// Take an item stack from a slot, leaving it empty.
    pub fn take(&mut self, slot: usize) -> Option<ItemStack> {
        if slot >= INVENTORY_SIZE {
            return None;
        }
        self.slots[slot].take()
    }

    /// Try to add an item stack to the inventory, merging with existing stacks if possible.
    /// Returns the remaining items that couldn't fit (if any).
    pub fn add_item(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        // First pass: try to merge with existing stacks.
        for existing in self.slots.iter_mut().flatten() {
            if existing.can_merge(&stack) && !existing.is_full() {
                let remainder = existing.add(stack.count);
                if remainder == 0 {
                    return None; // All items added
                }
                stack.count = remainder;
            }
        }

        // Second pass: find empty slot for remainder.
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(stack);
                return None; // All items added
            }
        }

        // Couldn't fit all items.
        Some(stack)
    }

    /// Remove a specific amount of an item type from the inventory.
    /// Returns the actual amount removed.
    pub fn remove_item(&mut self, item_id: ItemId, amount: u8) -> u8 {
        let mut remaining = amount;

        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }

            if let Some(stack) = slot {
                if stack.item_id == item_id {
                    let removed = stack.remove(remaining);
                    remaining -= removed;

                    // Remove empty stacks.
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }

        amount - remaining
    }

    /// Count the total number of a specific item in the inventory.
    pub fn count_item(&self, item_id: ItemId) -> u32 {
        self.slots
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|stack| stack.item_id == item_id)
            .map(|stack| stack.count as u32)
            .sum()
    }

    /// Check if the inventory contains at least the specified amount of an item.
    pub fn has_item(&self, item_id: ItemId, amount: u8) -> bool {
        self.count_item(item_id) >= amount as u32
    }

    /// Find the first slot containing a specific item.
    pub fn find_item(&self, item_id: ItemId) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|s| s.item_id == item_id))
    }

    /// Get the number of empty slots.
    pub fn empty_slots(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_none()).count()
    }

    /// Check if the inventory is completely empty.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.is_none())
    }

    /// Check if the inventory is completely full.
    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|slot| slot.is_some())
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
    fn item_stack_merge_and_split() {
        let mut stack1 = ItemStack::new(1, 32);
        let stack2 = ItemStack::new(1, 16);

        assert!(stack1.can_merge(&stack2));
        assert!(!stack1.is_full());

        let remainder = stack1.add(stack2.count);
        assert_eq!(remainder, 0);
        assert_eq!(stack1.count, 48);

        let split = stack1.split(16).unwrap();
        assert_eq!(split.count, 16);
        assert_eq!(stack1.count, 32);
    }

    #[test]
    fn item_stack_overflow() {
        let mut stack = ItemStack::new(1, 60);
        let remainder = stack.add(10);

        assert_eq!(remainder, 6); // Only 4 could fit
        assert_eq!(stack.count, 64);
        assert!(stack.is_full());
    }

    #[test]
    fn inventory_add_and_merge() {
        let mut inv = Inventory::new();

        // Add first stack.
        let stack1 = ItemStack::new(1, 32);
        assert!(inv.add_item(stack1).is_none());

        // Add second stack of same item - should merge.
        let stack2 = ItemStack::new(1, 16);
        assert!(inv.add_item(stack2).is_none());

        // Check that it merged.
        let slot0 = inv.get(0).unwrap();
        assert_eq!(slot0.count, 48);
        assert!(inv.get(1).is_none());
    }

    #[test]
    fn inventory_add_different_items() {
        let mut inv = Inventory::new();

        inv.add_item(ItemStack::new(1, 10));
        inv.add_item(ItemStack::new(2, 20));

        assert_eq!(inv.count_item(1), 10);
        assert_eq!(inv.count_item(2), 20);
        assert_eq!(inv.empty_slots(), 34);
    }

    #[test]
    fn inventory_remove_item() {
        let mut inv = Inventory::new();

        inv.add_item(ItemStack::new(1, 64));
        inv.add_item(ItemStack::new(1, 32));

        let removed = inv.remove_item(1, 80);
        assert_eq!(removed, 80);
        assert_eq!(inv.count_item(1), 16);
    }

    #[test]
    fn inventory_has_item() {
        let mut inv = Inventory::new();
        inv.add_item(ItemStack::new(1, 10));

        assert!(inv.has_item(1, 10));
        assert!(inv.has_item(1, 5));
        assert!(!inv.has_item(1, 11));
        assert!(!inv.has_item(2, 1));
    }

    #[test]
    fn inventory_full_handling() {
        let mut inv = Inventory::new();

        // Fill inventory with different item types (to prevent merging).
        for i in 0..36 {
            inv.add_item(ItemStack::new(i as u16, 1));
        }

        assert!(inv.is_full());
        assert_eq!(inv.empty_slots(), 0);

        // Try to add more - should return remainder.
        let remainder = inv.add_item(ItemStack::new(100, 5));
        assert!(remainder.is_some());
        assert_eq!(remainder.unwrap().count, 5);
    }

    #[test]
    fn inventory_find_item() {
        let mut inv = Inventory::new();
        inv.set(5, Some(ItemStack::new(42, 1)));

        assert_eq!(inv.find_item(42), Some(5));
        assert_eq!(inv.find_item(99), None);
    }
}
