//! World State Round-Trip Worldtest
//!
//! Validates that `world.state` persistence does not introduce simulation drift.
//! The test compares:
//! - A baseline run with continuous simulation
//! - A run that repeatedly save/loads `world.state` during the simulation
//!
//! Focus areas:
//! - Player + entities + block-entities survive roundtrips
//! - Deterministic evolution across save/load boundaries

use mdminecraft_testkit::{MetricsReportBuilder, MetricsSink, PersistenceMetrics, TestExecutionMetrics, TestResult};
use mdminecraft_world::{
    BlockEntitiesState, BlockEntityKey, BrewingStandState, EnchantingTableState, FurnaceState,
    ItemManager, Mob, MobType, Projectile, ProjectileManager, RegionStore, SimTime, StatusEffect,
    StatusEffectType, StatusEffects, WeatherToggle, WorldEntitiesState, WorldMeta, WorldPoint,
    WorldState,
};
use mdminecraft_core::{DimensionId, ItemStack as CoreItemStack, ItemType as CoreItemType, SimTick, ToolMaterial, ToolType};
use std::env;
use std::time::Instant;

const WORLD_SEED: u64 = 11223344556677;
const TOTAL_TICKS: u64 = 400;
const CYCLES: u64 = 10;

fn simulate_tick(state: &mut WorldState) {
    let dt = 1.0 / 20.0;

    state.tick = state.tick.advance(1);
    state.sim_time.advance();

    // Block entities (machines).
    for furnace in state.block_entities.furnaces.values_mut() {
        furnace.update(dt);
    }
    for stand in state.block_entities.brewing_stands.values_mut() {
        stand.update(dt);
    }

    // Entities.
    for mob in &mut state.entities.mobs {
        mob.update(state.tick.0);
    }

    // Dropped items (use a deterministic, fixed ground height).
    let _ = state
        .entities
        .dropped_items
        .update(|_x, _z| 64.0);

    // Projectiles.
    state.entities.projectiles.update();
}

fn make_initial_state() -> WorldState {
    // Player (minimal but non-empty).
    let mut hotbar = std::array::from_fn(|_| None);
    hotbar[0] = Some(CoreItemStack::new(
        CoreItemType::Tool(ToolType::Pickaxe, ToolMaterial::Iron),
        1,
    ));
    hotbar[1] = Some(CoreItemStack::new(CoreItemType::Item(7), 12));

    let mut status_effects = StatusEffects::new();
    status_effects.add(StatusEffect::new(StatusEffectType::Speed, 1, 200));

    let player = mdminecraft_world::PlayerSave {
        transform: mdminecraft_world::PlayerTransform {
            dimension: DimensionId::Overworld,
            x: 10.5,
            y: 64.25,
            z: -3.75,
            yaw: 1.25,
            pitch: -0.5,
        },
        spawn_point: WorldPoint {
            dimension: DimensionId::Overworld,
            x: 0.0,
            y: 65.0,
            z: 0.0,
        },
        hotbar,
        hotbar_selected: 1,
        inventory: mdminecraft_world::Inventory::new(),
        health: 17.0,
        hunger: 13.0,
        xp_level: 4,
        xp_current: 7,
        xp_next_level_xp: 17,
        armor: mdminecraft_world::PlayerArmor::new(),
        status_effects,
    };

    // Entities.
    let mut dropped_items = ItemManager::new();
    dropped_items.spawn_item(1.0, 70.0, 2.0, mdminecraft_world::ItemType::IronIngot, 3);

    let mut projectiles = ProjectileManager::new();
    projectiles.spawn(Projectile::shoot_arrow(0.0, 70.0, 0.0, 0.0, 0.0, 1.0));

    let entities = WorldEntitiesState {
        mobs: vec![Mob::new(2.0, 65.0, 2.0, MobType::Pig)],
        dropped_items,
        projectiles,
    };

    // Block entities.
    let mut block_entities = BlockEntitiesState::default();
    let furnace_key = BlockEntityKey {
        dimension: DimensionId::Overworld,
        x: 4,
        y: 65,
        z: 4,
    };
    block_entities.furnaces.insert(
        furnace_key,
        FurnaceState {
            input: Some((mdminecraft_world::ItemType::IronOre, 1)),
            fuel: Some((mdminecraft_world::ItemType::Coal, 1)),
            output: Some((mdminecraft_world::ItemType::IronIngot, 0)),
            smelt_progress: 0.25,
            fuel_remaining: 0.75,
            is_lit: true,
        },
    );

    let enchanting_key = BlockEntityKey {
        dimension: DimensionId::Overworld,
        x: 8,
        y: 65,
        z: 8,
    };
    block_entities.enchanting_tables.insert(
        enchanting_key,
        EnchantingTableState {
            item: Some((mdminecraft_world::BOW_ID, 1)),
            lapis_count: 12,
            bookshelf_count: 7,
            enchant_seed: 99,
            enchant_options: [None, None, None],
        },
    );

    let brewing_key = BlockEntityKey {
        dimension: DimensionId::Overworld,
        x: -2,
        y: 65,
        z: -2,
    };
    block_entities.brewing_stands.insert(
        brewing_key,
        BrewingStandState {
            bottles: [Some(mdminecraft_world::PotionType::Water), None, None],
            ingredient: Some((102, 1)),
            fuel: 3,
            brew_progress: 0.5,
            is_brewing: true,
        },
    );

    let mut time = SimTime::default();
    time.tick = SimTick::ZERO;

    WorldState {
        tick: SimTick::ZERO,
        sim_time: time,
        weather: WeatherToggle::new(),
        weather_next_change_tick: SimTick(123),
        player: Some(player),
        entities,
        block_entities,
    }
}

#[test]
fn world_state_roundtrip_worldtest() {
    let test_start = Instant::now();

    println!("\n=== World State Roundtrip Worldtest ===");
    println!("Configuration:");
    println!("  World seed: {}", WORLD_SEED);
    println!("  Total ticks: {}", TOTAL_TICKS);
    println!("  Save/load cycles: {}", CYCLES);
    println!();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = env::temp_dir().join(format!("mdminecraft_state_roundtrip_{}", timestamp));
    let store = RegionStore::new(&temp_dir).expect("failed to create region store");
    store
        .save_world_meta(&WorldMeta {
            world_seed: WORLD_SEED,
        })
        .expect("failed to save world meta");

    let baseline_start = Instant::now();
    let mut baseline = make_initial_state();
    for _ in 0..TOTAL_TICKS {
        simulate_tick(&mut baseline);
    }
    let baseline_time = baseline_start.elapsed();

    let mut state = make_initial_state();
    let mut save_times_us = Vec::new();
    let mut load_times_us = Vec::new();

    let ticks_per_cycle = (TOTAL_TICKS / CYCLES).max(1);
    let mut ticks_remaining = TOTAL_TICKS;
    for _cycle in 0..CYCLES {
        let ticks_this_cycle = ticks_remaining.min(ticks_per_cycle);
        if ticks_this_cycle == 0 {
            break;
        }

        for _ in 0..ticks_this_cycle {
            simulate_tick(&mut state);
        }

        // Save/load boundary.
        let save_start = Instant::now();
        store.save_world_state(&state).expect("failed to save world state");
        save_times_us.push(save_start.elapsed().as_micros());

        let load_start = Instant::now();
        state = store.load_world_state().expect("failed to load world state");
        load_times_us.push(load_start.elapsed().as_micros());

        ticks_remaining = ticks_remaining.saturating_sub(ticks_this_cycle);
    }

    // If TOTAL_TICKS isn't divisible by CYCLES, finish any remaining ticks without another roundtrip.
    while ticks_remaining > 0 {
        simulate_tick(&mut state);
        ticks_remaining -= 1;
    }

    let baseline_bytes = bincode::serialize(&baseline).expect("serialize baseline");
    let interrupted_bytes = bincode::serialize(&state).expect("serialize interrupted");
    assert_eq!(
        interrupted_bytes, baseline_bytes,
        "Simulation drift detected across save/load boundaries"
    );

    let world_state_path = temp_dir.join("world.state");
    let bytes_written = world_state_path
        .metadata()
        .map(|m| m.len())
        .unwrap_or(0);
    let bytes_read = bytes_written;
    let uncompressed_size = bincode::serialize(&state)
        .map(|v| v.len() as u64)
        .unwrap_or(0);
    let compression_ratio = if bytes_written > 0 {
        uncompressed_size as f64 / bytes_written as f64
    } else {
        0.0
    };

    let avg_save_time_us = if save_times_us.is_empty() {
        0.0
    } else {
        save_times_us.iter().sum::<u128>() as f64 / save_times_us.len() as f64
    };
    let avg_load_time_us = if load_times_us.is_empty() {
        0.0
    } else {
        load_times_us.iter().sum::<u128>() as f64 / load_times_us.len() as f64
    };

    let metrics = MetricsReportBuilder::new("world_state_roundtrip_worldtest")
        .result(TestResult::Pass)
        .persistence(PersistenceMetrics {
            chunks_saved: 0,
            chunks_loaded: 0,
            avg_save_time_us,
            avg_load_time_us,
            bytes_written,
            bytes_read,
            compression_ratio,
        })
        .execution(TestExecutionMetrics {
            duration_seconds: test_start.elapsed().as_secs_f64(),
            peak_memory_mb: None,
            assertions_checked: Some(1),
            validations_passed: Some(1),
        })
        .build();

    let metrics_path = std::env::current_dir()
        .unwrap()
        .join("target/metrics/world_state_roundtrip_worldtest.json");
    let sink = MetricsSink::create(&metrics_path).expect("failed to create metrics sink");
    sink.write(&metrics).expect("failed to write metrics");

    println!("Baseline time: {:.2}s", baseline_time.as_secs_f64());
    println!(
        "Avg save/load: {:.2}ms / {:.2}ms",
        avg_save_time_us / 1000.0,
        avg_load_time_us / 1000.0
    );
    println!("Compression ratio: {:.2}×", compression_ratio);
    println!("Metrics: {:?}", metrics_path);

    std::fs::remove_dir_all(&temp_dir).ok();
}

