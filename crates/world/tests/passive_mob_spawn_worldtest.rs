//! Worldtest: Passive Mob Spawning and Movement
//!
//! Validates:
//! - Deterministic mob spawning across multiple chunks
//! - Biome-specific spawn rules
//! - Mob AI state transitions
//! - Movement behavior over time
//! - Performance of mob updates

use mdminecraft_core::SimTick;
use mdminecraft_testkit::{EventRecord, JsonlSink};
use mdminecraft_world::{
    BiomeAssigner, Heightmap, Mob, MobSpawner, MobState, MobType,
};
use std::collections::HashMap;
use std::time::Instant;

const WORLD_SEED: u64 = 42;
const CHUNK_RADIUS: i32 = 4; // 9×9 grid = 81 chunks

#[test]
fn passive_mob_spawn_worldtest() {
    let test_start = Instant::now();

    // Create event log for CI artifacts
    let log_path = std::env::temp_dir().join("passive_mob_spawn_worldtest.jsonl");
    let mut event_log = JsonlSink::create(&log_path).expect("create event log");

    // Initialize world generation systems
    let biome_assigner = BiomeAssigner::new(WORLD_SEED);
    let mob_spawner = MobSpawner::new(WORLD_SEED);

    let mut all_mobs = Vec::new();
    let mut mob_type_counts: HashMap<MobType, usize> = HashMap::new();
    let mut biome_spawn_counts: HashMap<String, usize> = HashMap::new();

    // Phase 1: Generate chunks and spawn mobs
    println!("Phase 1: Generating chunks and spawning mobs...");

    for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
            // Generate heightmap for surface heights
            let heightmap = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);

            // Get primary biome for chunk center
            let chunk_center_x = chunk_x * 16 + 8;
            let chunk_center_z = chunk_z * 16 + 8;
            let biome = biome_assigner.get_biome(chunk_center_x, chunk_center_z);

            // Spawn mobs
            let mobs = mob_spawner.generate_spawns(
                chunk_x,
                chunk_z,
                biome,
                heightmap.heights(),
            );

            // Track statistics
            for mob in &mobs {
                *mob_type_counts.entry(mob.mob_type).or_insert(0) += 1;
            }

            if !mobs.is_empty() {
                let biome_name = format!("{:?}", biome);
                *biome_spawn_counts.entry(biome_name).or_insert(0) += mobs.len();
            }

            all_mobs.extend(mobs);
        }
    }

    let total_mobs = all_mobs.len();

    // Log spawning phase
    let tick = SimTick::ZERO.advance(1);
    event_log
        .write(&EventRecord {
            tick,
            kind: "MobSpawnPhase",
            payload: &format!(
                "Spawned {} mobs across {} chunks",
                total_mobs,
                (CHUNK_RADIUS * 2 + 1) * (CHUNK_RADIUS * 2 + 1)
            ),
        })
        .expect("write event");

    // Phase 2: Simulate mob behavior for 100 ticks
    println!("Phase 2: Simulating mob behavior...");

    let mut state_transitions = 0;
    let mut total_distance_moved = 0.0;
    let simulation_ticks = 100;

    let sim_start = Instant::now();

    for tick in 0..simulation_ticks {
        for mob in &mut all_mobs {
            let old_state = mob.state;
            let old_x = mob.x;
            let old_z = mob.z;

            mob.update(tick);

            // Track state transitions
            if old_state != mob.state {
                state_transitions += 1;
            }

            // Track movement
            let dx = mob.x - old_x;
            let dz = mob.z - old_z;
            let distance = (dx * dx + dz * dz).sqrt();
            total_distance_moved += distance;
        }
    }

    let sim_duration = sim_start.elapsed();
    let avg_update_time = sim_duration.as_micros() as f64 / (total_mobs as u64 * simulation_ticks) as f64;

    // Phase 3: Validation
    println!("Phase 3: Validating results...");

    // Check mob distribution
    assert!(
        total_mobs > 0,
        "Should spawn at least some mobs across {} chunks",
        (CHUNK_RADIUS * 2 + 1) * (CHUNK_RADIUS * 2 + 1)
    );

    assert!(
        total_mobs < 1000,
        "Should not spawn excessive mobs (got {})",
        total_mobs
    );

    // Check mob type diversity
    assert!(
        mob_type_counts.len() >= 2,
        "Should have at least 2 different mob types"
    );

    // Check that mobs actually moved
    assert!(
        total_distance_moved > 0.0,
        "Mobs should have moved during simulation"
    );

    // Check that state transitions happened
    assert!(
        state_transitions > 0,
        "Mobs should transition between states"
    );

    // Check all mobs are in valid states
    for mob in &all_mobs {
        assert!(
            mob.state == MobState::Idle || mob.state == MobState::Wandering,
            "Mob in invalid state"
        );
    }

    // Performance check: mob updates should be fast (target: <1μs per update)
    assert!(
        avg_update_time < 1.0,
        "Mob update too slow: {:.2}μs (target: <1μs)",
        avg_update_time
    );

    // Phase 4: Determinism check
    println!("Phase 4: Checking determinism...");

    let spawner2 = MobSpawner::new(WORLD_SEED);
    let heightmap2 = Heightmap::generate(WORLD_SEED, 0, 0);
    let biome2 = biome_assigner.get_biome(8, 8);
    let mobs2 = spawner2.generate_spawns(0, 0, biome2, heightmap2.heights());

    let spawner3 = MobSpawner::new(WORLD_SEED);
    let heightmap3 = Heightmap::generate(WORLD_SEED, 0, 0);
    let biome3 = biome_assigner.get_biome(8, 8);
    let mobs3 = spawner3.generate_spawns(0, 0, biome3, heightmap3.heights());

    assert_eq!(
        mobs2.len(),
        mobs3.len(),
        "Same seed should produce same number of mobs"
    );

    for (m2, m3) in mobs2.iter().zip(mobs3.iter()) {
        assert_eq!(m2.x, m3.x, "Mob X position should be deterministic");
        assert_eq!(m2.z, m3.z, "Mob Z position should be deterministic");
        assert_eq!(
            m2.mob_type, m3.mob_type,
            "Mob type should be deterministic"
        );
    }

    // Phase 5: Report results
    let test_duration = test_start.elapsed();

    println!("\n=== Passive Mob Spawn Worldtest Results ===");
    println!("Total chunks: {}", (CHUNK_RADIUS * 2 + 1) * (CHUNK_RADIUS * 2 + 1));
    println!("Total mobs spawned: {}", total_mobs);
    println!("\nMob type distribution:");
    for (mob_type, count) in &mob_type_counts {
        println!("  {:?}: {}", mob_type, count);
    }
    println!("\nBiome spawn distribution:");
    for (biome, count) in &biome_spawn_counts {
        println!("  {}: {} mobs", biome, count);
    }
    println!("\nSimulation ({} ticks):", simulation_ticks);
    println!("  State transitions: {}", state_transitions);
    println!("  Total distance moved: {:.2} blocks", total_distance_moved);
    println!("  Avg distance per mob: {:.2} blocks", total_distance_moved / total_mobs as f64);
    println!("\nPerformance:");
    println!("  Simulation time: {:?}", sim_duration);
    println!("  Avg update time: {:.3}μs per mob per tick", avg_update_time);
    println!("  Total test time: {:?}", test_duration);

    // Write final summary to event log
    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(simulation_ticks),
            kind: "TestComplete",
            payload: &format!(
                "Spawned {} mobs, {} state transitions, {:.2} blocks moved, {:.3}μs/update",
                total_mobs, state_transitions, total_distance_moved, avg_update_time
            ),
        })
        .expect("write event");

    println!("\nEvent log written to: {}", log_path.display());
    println!("===========================================\n");
}

#[test]
fn test_mob_spawning_respects_biome_rules() {
    let spawner = MobSpawner::new(12345);
    let biome_assigner = BiomeAssigner::new(12345);

    // Test various biomes
    let test_cases = vec![
        (0, 0),
        (5, 5),
        (-3, 7),
        (10, -10),
    ];

    for (chunk_x, chunk_z) in test_cases {
        let heightmap = Heightmap::generate(12345, chunk_x, chunk_z);
        let biome = biome_assigner.get_biome(chunk_x * 16 + 8, chunk_z * 16 + 8);
        let mobs = spawner.generate_spawns(chunk_x, chunk_z, biome, heightmap.heights());

        // Verify all spawned mobs are valid for the biome
        let valid_types = MobType::for_biome(biome);
        let valid_type_set: Vec<MobType> = valid_types.iter().map(|(t, _)| *t).collect();

        for mob in &mobs {
            assert!(
                valid_type_set.contains(&mob.mob_type) || valid_types.is_empty(),
                "Mob type {:?} not valid for biome {:?} at chunk ({}, {})",
                mob.mob_type,
                biome,
                chunk_x,
                chunk_z
            );
        }
    }
}

#[test]
fn test_mob_ai_consistency() {
    let mob = Mob::new(0.0, 64.0, 0.0, MobType::Pig);

    // Run simulation twice with same starting state
    let mut mob1 = mob.clone();
    let mut mob2 = mob.clone();

    for tick in 0..200 {
        mob1.update(tick);
        mob2.update(tick);

        assert_eq!(mob1.state, mob2.state, "State should be deterministic at tick {}", tick);
        assert_eq!(mob1.x, mob2.x, "X position should be deterministic at tick {}", tick);
        assert_eq!(mob1.z, mob2.z, "Z position should be deterministic at tick {}", tick);
        assert_eq!(mob1.ai_timer, mob2.ai_timer, "AI timer should be deterministic at tick {}", tick);
    }
}
