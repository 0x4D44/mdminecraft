//! Worldtest: Dropped Item Lifecycle
//!
//! Validates:
//! - Item spawning and physics simulation
//! - Ground collision and resting state
//! - Item pickup mechanics
//! - Item merging/stacking
//! - Lifecycle management and despawning
//! - Performance at scale

use mdminecraft_core::SimTick;
use mdminecraft_testkit::{EventRecord, JsonlSink};
use mdminecraft_world::{DroppedItem, ItemManager, ItemType, ITEM_DESPAWN_TICKS};
use std::time::Instant;

#[test]
fn drop_item_lifecycle_worldtest() {
    let test_start = Instant::now();

    // Create event log for CI artifacts
    let log_path = std::env::temp_dir().join("drop_item_lifecycle_worldtest.jsonl");
    let mut event_log = JsonlSink::create(&log_path).expect("create event log");

    println!("=== Dropped Item Lifecycle Worldtest ===\n");

    // Phase 1: Spawn items and test physics
    println!("Phase 1: Spawning items and testing physics...");
    let mut manager = ItemManager::new();

    // Spawn various item types at different heights
    let spawn_count = 100;
    for i in 0..spawn_count {
        let x = (i % 10) as f64 * 2.0;
        let z = (i / 10) as f64 * 2.0;
        let y = 70.0 + (i % 5) as f64 * 5.0; // Heights from 70 to 90

        let item_type = match i % 4 {
            0 => ItemType::Stone,
            1 => ItemType::Dirt,
            2 => ItemType::OakLog,
            _ => ItemType::Sand,
        };

        manager.spawn_item(x, y, z, item_type, 1);
    }

    assert_eq!(manager.count(), spawn_count);

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(1),
            kind: "ItemSpawn",
            payload: &format!("Spawned {} items", spawn_count),
        })
        .expect("write event");

    // Phase 2: Simulate physics until all items land
    println!("Phase 2: Simulating physics until items land...");
    let ground_height = |_x: f64, _z: f64| 64.0;

    let mut ticks_to_land = 0;
    let max_physics_ticks = 200;

    for tick in 0..max_physics_ticks {
        manager.update(ground_height);

        // Check if all items are on ground
        let all_grounded = manager.items().iter().all(|item| item.on_ground);
        if all_grounded {
            ticks_to_land = tick + 1;
            break;
        }
    }

    assert!(ticks_to_land > 0, "Items should have landed");
    assert!(
        ticks_to_land < max_physics_ticks,
        "Items should land quickly"
    );

    println!("  All items landed in {} ticks", ticks_to_land);

    // Verify all items are at ground level
    for item in manager.items() {
        assert!(item.on_ground, "Item should be on ground");
        assert!(
            (item.y - 64.25).abs() < 0.5,
            "Item Y position should be near ground"
        );
    }

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(ticks_to_land),
            kind: "ItemsLanded",
            payload: &format!(
                "All {} items landed in {} ticks",
                spawn_count, ticks_to_land
            ),
        })
        .expect("write event");

    // Phase 3: Test item merging
    println!("Phase 3: Testing item merging...");

    // Spawn some items close together
    let mut merge_manager = ItemManager::new();
    merge_manager.spawn_item(10.0, 64.25, 20.0, ItemType::Stone, 5);
    merge_manager.spawn_item(10.5, 64.25, 20.0, ItemType::Stone, 3);
    merge_manager.spawn_item(10.2, 64.25, 20.2, ItemType::Stone, 7);
    merge_manager.spawn_item(15.0, 64.25, 25.0, ItemType::Dirt, 10); // Too far to merge

    assert_eq!(merge_manager.count(), 4);

    let merged_count = merge_manager.merge_nearby_items();
    println!("  Merged {} item stacks", merged_count);

    // Should have merged the 3 stone items together
    assert!(merged_count >= 2, "Should have merged at least 2 items");
    assert_eq!(
        merge_manager.count(),
        4 - merged_count,
        "Item count should decrease by merge count"
    );

    // Verify the merged stack
    let all_items = merge_manager.items();
    let stone_items: Vec<_> = all_items
        .iter()
        .filter(|item| item.item_type == ItemType::Stone)
        .collect();
    assert_eq!(stone_items.len(), 1, "Should have 1 merged stone stack");
    assert_eq!(
        stone_items[0].count, 15,
        "Merged stack should have total count"
    );

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(ticks_to_land + 10),
            kind: "ItemMerge",
            payload: &format!("Merged {} item stacks", merged_count),
        })
        .expect("write event");

    // Phase 4: Test item pickup
    println!("Phase 4: Testing item pickup...");

    let mut pickup_manager = ItemManager::new();
    pickup_manager.spawn_item(10.0, 64.0, 20.0, ItemType::Stone, 5);
    pickup_manager.spawn_item(10.5, 64.0, 20.5, ItemType::Dirt, 3);
    pickup_manager.spawn_item(20.0, 64.0, 30.0, ItemType::OakLog, 7);

    // Pickup items near (10, 64, 20)
    let picked_up = pickup_manager.pickup_items(10.0, 64.0, 20.0);
    println!("  Picked up {} item stacks", picked_up.len());

    assert!(!picked_up.is_empty(), "Should pick up at least one item");
    assert!(picked_up.len() <= 2, "Should only pick up nearby items");

    // Verify the distant item wasn't picked up
    assert_eq!(
        pickup_manager.count(),
        3 - picked_up.len(),
        "Remote items should remain"
    );

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(ticks_to_land + 20),
            kind: "ItemPickup",
            payload: &format!("Picked up {} stacks", picked_up.len()),
        })
        .expect("write event");

    // Phase 5: Test lifecycle and despawning
    println!("Phase 5: Testing item despawn lifecycle...");

    let mut despawn_manager = ItemManager::new();
    let id1 = despawn_manager.spawn_item(10.0, 64.25, 20.0, ItemType::Stone, 1);
    let id2 = despawn_manager.spawn_item(15.0, 64.25, 25.0, ItemType::Dirt, 1);

    // Manually set lifetime to near-despawn
    if let Some(item) = despawn_manager.get_mut(id1) {
        item.on_ground = true;
        item.lifetime_ticks = 2;
    }
    if let Some(item) = despawn_manager.get_mut(id2) {
        item.on_ground = true;
        item.lifetime_ticks = 5;
    }

    // Simulate ticks and count despawns
    let mut total_despawned = 0;
    for _ in 0..10 {
        let despawned = despawn_manager.update(ground_height);
        total_despawned += despawned;
    }

    println!("  Despawned {} items over 10 ticks", total_despawned);
    assert_eq!(total_despawned, 2, "Both items should have despawned");
    assert_eq!(despawn_manager.count(), 0, "All items should be gone");

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(ticks_to_land + 30),
            kind: "ItemDespawn",
            payload: &format!("Despawned {} items", total_despawned),
        })
        .expect("write event");

    // Phase 6: Performance test at scale
    println!("Phase 6: Testing performance at scale...");

    let mut perf_manager = ItemManager::new();
    let scale_count = 1000;

    for i in 0..scale_count {
        let x = (i % 32) as f64 * 2.0;
        let z = (i / 32) as f64 * 2.0;
        perf_manager.spawn_item(x, 70.0, z, ItemType::Stone, 1);
    }

    let perf_start = Instant::now();
    let physics_ticks = 100;

    for _ in 0..physics_ticks {
        perf_manager.update(ground_height);
    }

    let perf_duration = perf_start.elapsed();
    let avg_update_time = perf_duration.as_micros() as f64 / physics_ticks as f64;

    println!(
        "  Simulated {} items for {} ticks",
        scale_count, physics_ticks
    );
    println!("  Total time: {:?}", perf_duration);
    println!("  Avg update time: {:.2}μs per tick", avg_update_time);

    // Performance target: <1ms per tick for 1000 items
    assert!(
        avg_update_time < 1000.0,
        "Performance target: <1ms per tick, got {:.2}μs",
        avg_update_time
    );

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(ticks_to_land + 130),
            kind: "PerformanceTest",
            payload: &format!(
                "{} items, {} ticks, {:.2}μs/tick",
                scale_count, physics_ticks, avg_update_time
            ),
        })
        .expect("write event");

    // Final report
    let test_duration = test_start.elapsed();

    println!("\n=== Test Results ===");
    println!("Physics landing time: {} ticks", ticks_to_land);
    println!("Items merged: {}", merged_count);
    println!("Items picked up: {}", picked_up.len());
    println!("Items despawned: {}", total_despawned);
    println!("Performance: {:.2}μs/tick for 1000 items", avg_update_time);
    println!("Total test time: {:?}", test_duration);
    println!("Event log: {}", log_path.display());
    println!("===================\n");

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(ticks_to_land + 200),
            kind: "TestComplete",
            payload: &format!(
                "Landing: {}t, Merged: {}, Picked: {}, Despawned: {}, Perf: {:.2}μs/tick",
                ticks_to_land,
                merged_count,
                picked_up.len(),
                total_despawned,
                avg_update_time
            ),
        })
        .expect("write event");
}

#[test]
fn test_item_type_block_mapping() {
    // Verify all block types have proper item mappings (based on blocks.json)
    assert_eq!(ItemType::from_block(0), None); // Air
    assert_eq!(ItemType::from_block(1), Some((ItemType::Cobblestone, 1))); // Stone drops cobblestone (like Minecraft)
    assert_eq!(ItemType::from_block(2), Some((ItemType::Dirt, 1)));
    assert_eq!(ItemType::from_block(3), Some((ItemType::Dirt, 1))); // Grass drops dirt
    assert_eq!(ItemType::from_block(4), Some((ItemType::Sand, 1)));
    assert_eq!(ItemType::from_block(5), Some((ItemType::Gravel, 1)));
    assert_eq!(ItemType::from_block(7), None); // Ice requires Silk Touch
    assert_eq!(ItemType::silk_touch_drop(7), Some((ItemType::Ice, 1)));
    assert_eq!(ItemType::from_block(8), Some((ItemType::Snow, 1)));
    assert_eq!(ItemType::from_block(9), Some((ItemType::Clay, 1)));
    assert_eq!(ItemType::from_block(11), Some((ItemType::OakLog, 1)));
    assert_eq!(ItemType::from_block(12), Some((ItemType::OakPlanks, 1)));
    // Ore blocks (coal ore drops coal directly, like Minecraft)
    assert_eq!(ItemType::from_block(14), Some((ItemType::Coal, 1)));
    assert_eq!(ItemType::from_block(15), Some((ItemType::IronOre, 1)));
    assert_eq!(ItemType::from_block(16), Some((ItemType::GoldOre, 1)));
    assert_eq!(ItemType::from_block(17), Some((ItemType::Diamond, 1)));
    // No drops
    assert_eq!(ItemType::from_block(6), None); // Water
    assert_eq!(ItemType::from_block(10), None); // Bedrock
    assert_eq!(ItemType::from_block(13), Some((ItemType::CraftingTable, 1))); // Crafting table
    assert_eq!(ItemType::from_block(69), Some((ItemType::Torch, 1))); // Torch
}

#[test]
fn test_item_despawn_constant() {
    // Verify despawn time is 5 minutes (6000 ticks at 20 TPS)
    assert_eq!(ITEM_DESPAWN_TICKS, 6000);

    // Create an item and verify it starts with full lifetime
    let item = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 1);
    assert_eq!(item.lifetime_ticks, ITEM_DESPAWN_TICKS);
}

#[test]
fn test_item_stack_merging_limits() {
    let mut item_a = DroppedItem::new(1, 10.0, 64.0, 20.0, ItemType::Stone, 60);
    let item_b = DroppedItem::new(2, 10.0, 64.0, 20.0, ItemType::Stone, 10);

    // Can only merge 4 items (64 max - 60 current)
    let merged = item_a.try_merge(&item_b);
    assert_eq!(merged, 4);
    assert_eq!(item_a.count, 64);

    // Verify food items have 16 stack limit
    let mut food_a = DroppedItem::new(3, 10.0, 64.0, 20.0, ItemType::RawPork, 15);
    let food_b = DroppedItem::new(4, 10.0, 64.0, 20.0, ItemType::RawPork, 5);

    let merged_food = food_a.try_merge(&food_b);
    assert_eq!(merged_food, 1); // Can only add 1 more (16 max - 15 current)
    assert_eq!(food_a.count, 16);
}
