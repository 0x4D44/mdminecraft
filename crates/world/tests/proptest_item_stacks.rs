//! Property-based tests for item stack mechanics
//!
//! Validates item stack invariants:
//! - Stack sizes never exceed max_stack_size
//! - Merging preserves total item count
//! - Stack splitting maintains conservation
//! - Different item types don't merge

use mdminecraft_core::DimensionId;
use mdminecraft_world::{DroppedItem, ItemManager, ItemType};
use proptest::prelude::*;

const DIM: DimensionId = DimensionId::Overworld;

proptest! {
    /// Property: Max stack size is always positive and reasonable
    ///
    /// For any item type, max_stack_size should be a reasonable value.
    #[test]
    fn stack_size_limits_are_valid(
        item_type in prop_oneof![
            Just(ItemType::Stone),
            Just(ItemType::Dirt),
            Just(ItemType::RawPork),
            Just(ItemType::RawBeef),
            Just(ItemType::Sand),
            Just(ItemType::Gravel),
        ],
    ) {
        let max_stack = item_type.max_stack_size();

        prop_assert!(
            max_stack > 0 && max_stack <= 64,
            "Max stack size {} out of reasonable range [1, 64]",
            max_stack
        );
    }

    /// Property: Merging same-type items conserves total count
    ///
    /// When merging two stacks of the same type, the total count
    /// should be conserved (sum of both stacks, up to stack limit).
    #[test]
    fn merge_conserves_count(
        item_type in prop_oneof![
            Just(ItemType::Stone),
            Just(ItemType::Dirt),
        ],
        count1 in 1u32..32,
        count2 in 1u32..32,
    ) {
        let mut item1 = DroppedItem::new(1, DIM, 0.0, 64.0, 0.0, item_type, count1);
        let item2 = DroppedItem::new(2, DIM, 0.0, 64.0, 0.0, item_type, count2);

        let total_before = count1 + count2;
        let merged = item1.try_merge(&item2);

        let max_stack = item_type.max_stack_size();
        let expected_merged = total_before.min(max_stack);
        let expected_merged_amount = (count1 + count2).min(max_stack) - count1;

        prop_assert_eq!(
            item1.count, expected_merged,
            "Merged count {} doesn't match expected {}",
            item1.count, expected_merged
        );

        prop_assert_eq!(
            merged, expected_merged_amount,
            "Merge return {} doesn't match expected {}",
            merged, expected_merged_amount
        );
    }

    /// Property: Different item types cannot merge
    ///
    /// Attempting to merge stacks of different types should fail
    /// and return 0 items merged.
    #[test]
    fn different_types_dont_merge(
        type1 in prop_oneof![Just(ItemType::Stone), Just(ItemType::Dirt)],
        type2 in prop_oneof![Just(ItemType::Sand), Just(ItemType::Gravel)],
        count in 1u32..32,
    ) {
        let mut item1 = DroppedItem::new(1, DIM, 0.0, 64.0, 0.0, type1, count);
        let item2 = DroppedItem::new(2, DIM, 0.0, 64.0, 0.0, type2, count);

        let original_count = item1.count;
        let merged = item1.try_merge(&item2);

        prop_assert_eq!(
            merged, 0,
            "Different types should not merge"
        );
        prop_assert_eq!(
            item1.count, original_count,
            "Count should not change when merge fails"
        );
    }

    /// Property: Item manager maintains item count
    ///
    /// When spawning items, the manager should track the correct count.
    #[test]
    fn item_manager_count_tracking(
        spawn_count in 1usize..50,
    ) {
        let mut manager = ItemManager::new();

        for i in 0..spawn_count {
            manager.spawn_item(DIM, i as f64, 64.0, 0.0, ItemType::Stone, 1);
        }

        prop_assert_eq!(
            manager.count(), spawn_count,
            "Manager count {} doesn't match spawned {}",
            manager.count(), spawn_count
        );
    }

    /// Property: Pickup removes items
    ///
    /// When picking up items within radius, those items should be removed.
    #[test]
    fn pickup_removes_items(
        item_count in 1usize..20,
    ) {
        let mut manager = ItemManager::new();

        // Spawn items at same location
        for _ in 0..item_count {
            manager.spawn_item(DIM, 10.0, 64.0, 20.0, ItemType::Stone, 1);
        }

        let before_count = manager.count();
        let picked_up = manager.pickup_items(DIM, 10.0, 64.0, 20.0);

        prop_assert_eq!(
            picked_up.len(), before_count,
            "Should pick up all items at same location"
        );

        prop_assert_eq!(
            manager.count(), 0,
            "All items should be removed after pickup"
        );
    }

    /// Property: Merged items reduce total count
    ///
    /// When merging nearby items, the item count should decrease.
    #[test]
    fn merge_reduces_count(
        base_count in 2usize..10,
    ) {
        let mut manager = ItemManager::new();

        // Spawn items very close together (will merge)
        for i in 0..base_count {
            let x = 10.0 + (i as f64 * 0.1); // Within 1 block
            manager.spawn_item(DIM, x, 64.0, 20.0, ItemType::Stone, 5);
        }

        let before_count = manager.count();
        let merged = manager.merge_nearby_items(DIM);

        prop_assert!(
            manager.count() < before_count || merged == 0,
            "Merge should reduce count or have no merges"
        );

        if merged > 0 {
            prop_assert!(
                manager.count() == before_count - merged,
                "Count reduction should match merge count"
            );
        }
    }

    /// Property: Food items have stack limit of 16
    ///
    /// Food items should have a maximum stack size of 16.
    #[test]
    fn food_stack_limit(
        food_type in prop_oneof![
            Just(ItemType::RawPork),
            Just(ItemType::RawBeef),
            Just(ItemType::Egg),
        ],
    ) {
        let max_stack = food_type.max_stack_size();
        prop_assert_eq!(
            max_stack, 16,
            "Food items should have max stack of 16, got {}",
            max_stack
        );
    }

    /// Property: Block items have stack limit of 64
    ///
    /// Block items should have a maximum stack size of 64.
    #[test]
    fn block_stack_limit(
        block_type in prop_oneof![
            Just(ItemType::Stone),
            Just(ItemType::Dirt),
            Just(ItemType::Sand),
            Just(ItemType::Gravel),
        ],
    ) {
        let max_stack = block_type.max_stack_size();
        prop_assert_eq!(
            max_stack, 64,
            "Block items should have max stack of 64, got {}",
            max_stack
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn stone_stacks_to_64() {
        assert_eq!(ItemType::Stone.max_stack_size(), 64);
    }

    #[test]
    fn pork_stacks_to_16() {
        assert_eq!(ItemType::RawPork.max_stack_size(), 16);
    }

    #[test]
    fn merge_same_type_works() {
        let mut item1 = DroppedItem::new(1, DIM, 0.0, 64.0, 0.0, ItemType::Stone, 10);
        let item2 = DroppedItem::new(2, DIM, 0.0, 64.0, 0.0, ItemType::Stone, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 5);
        assert_eq!(item1.count, 15);
    }

    #[test]
    fn merge_different_type_fails() {
        let mut item1 = DroppedItem::new(1, DIM, 0.0, 64.0, 0.0, ItemType::Stone, 10);
        let item2 = DroppedItem::new(2, DIM, 0.0, 64.0, 0.0, ItemType::Dirt, 5);

        let merged = item1.try_merge(&item2);
        assert_eq!(merged, 0);
        assert_eq!(item1.count, 10);
    }
}
