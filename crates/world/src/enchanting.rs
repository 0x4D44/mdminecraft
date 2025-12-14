//! Enchanting table system.
//!
//! Provides enchanting table block functionality with enchantment selection,
//! lapis lazuli consumption, and XP cost management.

use crate::inventory::ItemId;
use mdminecraft_core::{Enchantment, EnchantmentType};
use serde::{Deserialize, Serialize};

// Tool item ID ranges (these should match the item registry)
// For now we'll use constants that can be adjusted later
/// First tool item ID (wood pickaxe)
pub const TOOL_ID_START: ItemId = 1000;
/// Last tool item ID
pub const TOOL_ID_END: ItemId = 1024;
/// Bow item ID
pub const BOW_ID: ItemId = 1100;
/// Lapis Lazuli item ID
pub const LAPIS_ID: ItemId = 400;

/// Maximum number of bookshelves that affect enchanting (vanilla Minecraft limit).
pub const MAX_BOOKSHELVES: u32 = 15;

/// Base XP level required for each enchanting slot.
pub const BASE_LEVEL_COSTS: [u32; 3] = [1, 5, 10];

/// Lapis lazuli required for each enchanting slot.
pub const LAPIS_COSTS: [u32; 3] = [1, 2, 3];

/// State of an enchanting table in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantingTableState {
    /// Item in the enchanting slot (item ID and count) - only 1 item can be enchanted at a time.
    pub item: Option<(ItemId, u32)>,
    /// Lapis lazuli in the lapis slot (count).
    pub lapis_count: u32,
    /// Number of nearby bookshelves (affects enchantment levels).
    pub bookshelf_count: u32,
    /// Seed for randomizing available enchantments (changes when item changes).
    pub enchant_seed: u64,
    /// Available enchantment options (recalculated when item/bookshelves change).
    pub enchant_options: [Option<EnchantmentOffer>; 3],
}

/// An enchantment offer in one of the three slots.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EnchantmentOffer {
    /// The primary enchantment that will be applied.
    pub enchantment: Enchantment,
    /// XP level cost to apply this enchantment.
    pub level_cost: u32,
    /// Number of XP levels consumed (actual XP drain, equals slot index + 1).
    pub levels_consumed: u32,
}

impl Default for EnchantingTableState {
    fn default() -> Self {
        Self::new()
    }
}

impl EnchantingTableState {
    /// Create a new empty enchanting table.
    pub fn new() -> Self {
        Self {
            item: None,
            lapis_count: 0,
            bookshelf_count: 0,
            enchant_seed: 0,
            enchant_options: [None, None, None],
        }
    }

    /// Create with a specific seed (for deterministic testing).
    pub fn with_seed(seed: u64) -> Self {
        Self {
            item: None,
            lapis_count: 0,
            bookshelf_count: 0,
            enchant_seed: seed,
            enchant_options: [None, None, None],
        }
    }

    /// Set the number of nearby bookshelves and recalculate options.
    pub fn set_bookshelf_count(&mut self, count: u32) {
        self.bookshelf_count = count.min(MAX_BOOKSHELVES);
        self.recalculate_options();
    }

    /// Add an item to the enchanting slot.
    ///
    /// # Returns
    /// Number of items that couldn't be added (0 if added successfully).
    pub fn add_item(&mut self, item_id: ItemId, count: u32) -> u32 {
        // Only enchantable items can be placed
        if !is_enchantable_id(item_id) {
            return count;
        }

        match &mut self.item {
            None => {
                // Only accept 1 item at a time for enchanting
                self.item = Some((item_id, 1));
                // Generate new seed when item changes
                self.enchant_seed = self.enchant_seed.wrapping_add(1);
                self.recalculate_options();
                count - 1
            }
            Some(_) => count, // Slot already occupied
        }
    }

    /// Add lapis lazuli to the lapis slot.
    ///
    /// # Returns
    /// Number of lapis that couldn't be added.
    pub fn add_lapis(&mut self, count: u32) -> u32 {
        let max: u32 = 64;
        let space = max.saturating_sub(self.lapis_count);
        let add = count.min(space);
        self.lapis_count += add;
        count - add
    }

    /// Take the item from the enchanting slot.
    pub fn take_item(&mut self) -> Option<(ItemId, u32)> {
        let item = self.item.take();
        if item.is_some() {
            self.enchant_options = [None, None, None];
        }
        item
    }

    /// Take lapis from the lapis slot.
    ///
    /// # Returns
    /// Number of lapis taken.
    pub fn take_lapis(&mut self, count: u32) -> u32 {
        let take = count.min(self.lapis_count);
        self.lapis_count -= take;
        take
    }

    /// Check if an enchantment slot can be selected.
    ///
    /// # Arguments
    /// * `slot` - Slot index (0, 1, or 2)
    /// * `player_level` - Player's current XP level
    pub fn can_enchant(&self, slot: usize, player_level: u32) -> bool {
        if slot >= 3 {
            return false;
        }

        if let Some(offer) = &self.enchant_options[slot] {
            // Check player has enough levels
            if player_level < offer.level_cost {
                return false;
            }
            // Check enough lapis
            if self.lapis_count < LAPIS_COSTS[slot] {
                return false;
            }
            // Check item is present
            if self.item.is_none() {
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Apply an enchantment from a slot.
    ///
    /// # Arguments
    /// * `slot` - Slot index (0, 1, or 2)
    ///
    /// # Returns
    /// The enchantment applied and XP levels consumed, or None if can't enchant.
    pub fn apply_enchantment(&mut self, slot: usize) -> Option<(Enchantment, u32)> {
        if slot >= 3 {
            return None;
        }

        let offer = self.enchant_options[slot]?;

        // Consume lapis
        self.lapis_count = self.lapis_count.saturating_sub(LAPIS_COSTS[slot]);

        // Clear options (need to place new item)
        self.enchant_options = [None, None, None];

        // Generate new seed for next enchant
        self.enchant_seed = self
            .enchant_seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);

        Some((offer.enchantment, offer.levels_consumed))
    }

    /// Recalculate available enchantment options based on item and bookshelves.
    fn recalculate_options(&mut self) {
        // Clear options first
        self.enchant_options = [None, None, None];

        // Need an item to offer enchantments
        let Some((item_id, _)) = &self.item else {
            return;
        };

        // Get valid enchantments for this item ID
        let valid_enchants = get_valid_enchantments_for_id(*item_id);
        if valid_enchants.is_empty() {
            return;
        }

        // Calculate level range based on bookshelves
        // Vanilla formula: base = 1 + random(0, bookshelves/2) + random(0, bookshelves/2)
        // Simplified: level_modifier = bookshelves (0-15 range)
        let level_modifier = self.bookshelf_count;

        // Generate options for each slot
        for (slot, base_cost) in BASE_LEVEL_COSTS.iter().enumerate() {
            let level_cost = *base_cost + level_modifier;

            // Use seed to deterministically pick enchantment
            let enchant_seed = self.enchant_seed.wrapping_add(slot as u64 * 12345);
            let enchant_index = (enchant_seed % valid_enchants.len() as u64) as usize;
            let enchant_type = valid_enchants[enchant_index];

            // Calculate enchantment level based on cost
            let max_level = enchant_type.max_level();
            let enchant_level = calculate_enchant_level(level_cost, max_level);

            self.enchant_options[slot] = Some(EnchantmentOffer {
                enchantment: Enchantment::new(enchant_type, enchant_level),
                level_cost,
                levels_consumed: (slot as u32) + 1,
            });
        }
    }
}

/// Check if an item ID represents an enchantable item.
pub fn is_enchantable_id(item_id: ItemId) -> bool {
    // Tools are in range TOOL_ID_START to TOOL_ID_END
    (TOOL_ID_START..=TOOL_ID_END).contains(&item_id) || item_id == BOW_ID
}

/// Get the tool category from an item ID.
/// Returns: 0 = not a tool, 1 = mining tool, 2 = weapon
fn get_tool_category(item_id: ItemId) -> u8 {
    if !(TOOL_ID_START..=TOOL_ID_END).contains(&item_id) {
        if item_id == BOW_ID {
            return 2; // Bow is a weapon
        }
        return 0; // Not a tool
    }

    // Within tool range, determine category based on ID offset
    // Assuming tool IDs are organized: pickaxes, axes, shovels, hoes, then swords
    let offset = item_id - TOOL_ID_START;
    let tool_index = offset / 5; // 5 tiers per tool type

    match tool_index {
        0..=3 => 1, // Pickaxe, axe, shovel, hoe = mining tools
        4 => 2,     // Sword = weapon
        _ => 0,
    }
}

/// Get valid enchantments for an item ID.
pub fn get_valid_enchantments_for_id(item_id: ItemId) -> Vec<EnchantmentType> {
    let category = get_tool_category(item_id);

    match category {
        1 => {
            // Mining tools (pickaxe, axe, shovel, hoe)
            vec![
                EnchantmentType::Efficiency,
                EnchantmentType::Unbreaking,
                EnchantmentType::SilkTouch,
                EnchantmentType::Fortune,
                EnchantmentType::Mending,
            ]
        }
        2 => {
            // Weapons (sword, bow)
            vec![
                EnchantmentType::Sharpness,
                EnchantmentType::Knockback,
                EnchantmentType::FireAspect,
                EnchantmentType::Unbreaking,
                EnchantmentType::Mending,
            ]
        }
        _ => vec![],
    }
}

/// Calculate enchantment level based on XP cost and max level.
fn calculate_enchant_level(level_cost: u32, max_level: u8) -> u8 {
    // Simple formula: higher cost = higher enchantment level
    // Cost 1-10 = level 1, 11-20 = level 2, etc.
    let level = ((level_cost / 10) + 1).min(max_level as u32) as u8;
    level.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test item IDs
    const TEST_PICKAXE_ID: ItemId = TOOL_ID_START; // First tool = pickaxe
    const TEST_SWORD_ID: ItemId = TOOL_ID_START + 20; // Sword range (4 * 5 = 20)
    const TEST_STONE_ID: ItemId = 1; // A block, not a tool

    #[test]
    fn test_enchanting_table_new() {
        let table = EnchantingTableState::new();
        assert!(table.item.is_none());
        assert_eq!(table.lapis_count, 0);
        assert_eq!(table.bookshelf_count, 0);
    }

    #[test]
    fn test_add_enchantable_item() {
        let mut table = EnchantingTableState::new();

        let remaining = table.add_item(TEST_PICKAXE_ID, 5);
        assert_eq!(remaining, 4); // Only 1 item accepted
        assert!(table.item.is_some());
        assert_eq!(table.item.unwrap().1, 1);
    }

    #[test]
    fn test_add_non_enchantable_item() {
        let mut table = EnchantingTableState::new();

        let remaining = table.add_item(TEST_STONE_ID, 1);
        assert_eq!(remaining, 1); // Rejected
        assert!(table.item.is_none());
    }

    #[test]
    fn test_add_lapis() {
        let mut table = EnchantingTableState::new();

        let remaining = table.add_lapis(10);
        assert_eq!(remaining, 0);
        assert_eq!(table.lapis_count, 10);

        // Test stack limit
        let remaining = table.add_lapis(60);
        assert_eq!(remaining, 6); // Only 54 more fit (64 - 10)
        assert_eq!(table.lapis_count, 64);
    }

    #[test]
    fn test_bookshelf_count() {
        let mut table = EnchantingTableState::new();

        table.set_bookshelf_count(20);
        assert_eq!(table.bookshelf_count, MAX_BOOKSHELVES); // Clamped to 15
    }

    #[test]
    fn test_enchant_options_generated() {
        let mut table = EnchantingTableState::with_seed(42);

        table.add_item(TEST_SWORD_ID, 1);
        table.set_bookshelf_count(15);

        // Should have 3 options
        assert!(table.enchant_options[0].is_some());
        assert!(table.enchant_options[1].is_some());
        assert!(table.enchant_options[2].is_some());
    }

    #[test]
    fn test_can_enchant() {
        let mut table = EnchantingTableState::with_seed(42);

        table.add_item(TEST_SWORD_ID, 1);
        table.add_lapis(5);
        table.set_bookshelf_count(5);

        // Get the level cost from slot 0
        let level_cost = table.enchant_options[0].as_ref().unwrap().level_cost;

        // Player with enough levels can enchant
        assert!(table.can_enchant(0, level_cost));
        // Player without enough levels cannot
        assert!(!table.can_enchant(0, 0));
    }

    #[test]
    fn test_apply_enchantment() {
        let mut table = EnchantingTableState::with_seed(42);

        table.add_item(TEST_SWORD_ID, 1);
        table.add_lapis(5);
        table.set_bookshelf_count(5);

        let initial_lapis = table.lapis_count;

        let result = table.apply_enchantment(0);
        assert!(result.is_some());

        let (enchantment, levels_consumed) = result.unwrap();
        assert_eq!(levels_consumed, 1); // Slot 0 consumes 1 level
        assert!(enchantment.level >= 1);

        // Lapis should be consumed
        assert_eq!(table.lapis_count, initial_lapis - LAPIS_COSTS[0]);

        // Options should be cleared
        assert!(table.enchant_options[0].is_none());
    }

    #[test]
    fn test_is_enchantable_id() {
        assert!(is_enchantable_id(TEST_PICKAXE_ID));
        assert!(is_enchantable_id(TEST_SWORD_ID));
        assert!(is_enchantable_id(BOW_ID));
        assert!(!is_enchantable_id(TEST_STONE_ID));
        assert!(!is_enchantable_id(0)); // Air
    }

    #[test]
    fn test_valid_enchantments_sword() {
        let enchants = get_valid_enchantments_for_id(TEST_SWORD_ID);

        assert!(enchants.contains(&EnchantmentType::Sharpness));
        assert!(enchants.contains(&EnchantmentType::Knockback));
        assert!(enchants.contains(&EnchantmentType::FireAspect));
        assert!(!enchants.contains(&EnchantmentType::Efficiency)); // Not for swords
    }

    #[test]
    fn test_valid_enchantments_pickaxe() {
        let enchants = get_valid_enchantments_for_id(TEST_PICKAXE_ID);

        assert!(enchants.contains(&EnchantmentType::Efficiency));
        assert!(enchants.contains(&EnchantmentType::SilkTouch));
        assert!(enchants.contains(&EnchantmentType::Fortune));
        assert!(!enchants.contains(&EnchantmentType::Sharpness)); // Not for pickaxes
    }

    #[test]
    fn test_take_item() {
        let mut table = EnchantingTableState::new();
        table.add_item(TEST_PICKAXE_ID, 1);

        let taken = table.take_item();
        assert!(taken.is_some());
        assert_eq!(taken.unwrap(), (TEST_PICKAXE_ID, 1));
        assert!(table.item.is_none());

        // Take again - should be None
        assert!(table.take_item().is_none());
    }

    #[test]
    fn test_take_lapis() {
        let mut table = EnchantingTableState::new();
        table.add_lapis(10);

        // Take partial amount
        let taken = table.take_lapis(5);
        assert_eq!(taken, 5);
        assert_eq!(table.lapis_count, 5);

        // Take all remaining
        let taken = table.take_lapis(10); // Request more than available
        assert_eq!(taken, 5); // Only 5 available
        assert_eq!(table.lapis_count, 0);

        // Take again - should be 0
        assert_eq!(table.take_lapis(10), 0);
    }

    #[test]
    fn test_apply_enchantment_no_item() {
        let mut table = EnchantingTableState::with_seed(42);
        table.add_lapis(5);
        table.set_bookshelf_count(5);

        // No item - should fail
        let result = table.apply_enchantment(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_can_enchant_no_lapis() {
        let mut table = EnchantingTableState::with_seed(42);
        table.add_item(TEST_SWORD_ID, 1);
        table.set_bookshelf_count(5);
        // No lapis added

        // can_enchant checks lapis and should return false
        let level_cost = table.enchant_options[0].as_ref().unwrap().level_cost;
        assert!(!table.can_enchant(0, level_cost));
    }

    #[test]
    fn test_apply_enchantment_invalid_slot() {
        let mut table = EnchantingTableState::with_seed(42);
        table.add_item(TEST_SWORD_ID, 1);
        table.add_lapis(5);
        table.set_bookshelf_count(5);

        // Invalid slot
        let result = table.apply_enchantment(3);
        assert!(result.is_none());
    }

    #[test]
    fn test_valid_enchantments_bow() {
        let enchants = get_valid_enchantments_for_id(BOW_ID);

        // Bow is a weapon, gets weapon enchantments
        assert!(enchants.contains(&EnchantmentType::Sharpness));
        assert!(enchants.contains(&EnchantmentType::Knockback));
        assert!(enchants.contains(&EnchantmentType::Unbreaking));
    }

    #[test]
    fn test_valid_enchantments_non_tool() {
        let enchants = get_valid_enchantments_for_id(TEST_STONE_ID);
        assert!(enchants.is_empty());
    }

    #[test]
    fn test_enchanting_table_default() {
        let table = EnchantingTableState::default();
        assert!(table.item.is_none());
        assert_eq!(table.lapis_count, 0);
    }

    #[test]
    fn test_enchanting_table_with_seed() {
        let table = EnchantingTableState::with_seed(12345);
        assert_eq!(table.enchant_seed, 12345);
    }

    #[test]
    fn test_enchant_level_cost_affects_level() {
        let mut table = EnchantingTableState::with_seed(42);

        // With no bookshelves
        table.add_item(TEST_SWORD_ID, 1);
        table.set_bookshelf_count(0);
        let low_cost = table.enchant_options[0].as_ref().unwrap().level_cost;

        // With max bookshelves
        table.set_bookshelf_count(MAX_BOOKSHELVES);
        let high_cost = table.enchant_options[0].as_ref().unwrap().level_cost;

        assert!(high_cost > low_cost);
    }

    #[test]
    fn test_enchanting_table_serialization() {
        let mut table = EnchantingTableState::with_seed(42);
        table.add_item(TEST_PICKAXE_ID, 1);
        table.add_lapis(5);
        table.set_bookshelf_count(8);

        // Note: add_item increments seed (42 -> 43)
        let expected_seed = table.enchant_seed;

        let serialized = serde_json::to_string(&table).unwrap();
        let deserialized: EnchantingTableState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.lapis_count, 5);
        assert_eq!(deserialized.bookshelf_count, 8);
        assert_eq!(deserialized.enchant_seed, expected_seed);
        assert!(deserialized.item.is_some());
    }

    #[test]
    fn test_enchantment_offer_serialization() {
        let offer = EnchantmentOffer {
            enchantment: Enchantment::new(EnchantmentType::Sharpness, 3),
            level_cost: 15,
            levels_consumed: 2,
        };

        let serialized = serde_json::to_string(&offer).unwrap();
        let deserialized: EnchantmentOffer = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.level_cost, 15);
        assert_eq!(deserialized.levels_consumed, 2);
    }

    #[test]
    fn test_apply_all_three_slots() {
        // Test applying enchantment from slot 1 and 2
        let mut table = EnchantingTableState::with_seed(42);
        table.add_item(TEST_SWORD_ID, 1);
        table.add_lapis(10);
        table.set_bookshelf_count(5);

        // Apply from slot 1 (costs 2 lapis)
        let result = table.apply_enchantment(1);
        assert!(result.is_some());
        let (_, levels_consumed) = result.unwrap();
        assert_eq!(levels_consumed, 2); // Slot 1 consumes 2 levels
    }

    #[test]
    fn test_add_item_replaces_existing() {
        let mut table = EnchantingTableState::new();
        table.add_item(TEST_PICKAXE_ID, 1);

        // Adding another item should fail (slot full)
        let remaining = table.add_item(TEST_SWORD_ID, 1);
        assert_eq!(remaining, 1);

        // Take the item first
        table.take_item();

        // Now add new item
        let remaining = table.add_item(TEST_SWORD_ID, 1);
        assert_eq!(remaining, 0);
    }
}
