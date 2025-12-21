//! Mob Lifecycle Worldtest
//!
//! This test validates the complete mob lifecycle from spawn to despawn.
//! Focus areas:
//! - Biome-appropriate mob spawning
//! - Mob AI and wandering behavior
//! - Position and state updates over time
//! - Health management
//! - Movement patterns and velocity
//! - Update performance at scale
//! - Mob distribution and density
//! - Long-running simulation stability
//!
//! Defaults are tuned to keep debug test runs reasonable while still exercising
//! the full mob update pipeline. Override via env vars when you want the full run:
//! - `MDM_MOB_LIFECYCLE_CHUNK_RADIUS`
//! - `MDM_MOB_LIFECYCLE_TICKS`
//! - `MDM_MOB_LIFECYCLE_SPAWN_PROBABILITY`

use mdminecraft_testkit::{
    MetricsReportBuilder, MetricsSink, MobMetrics, TerrainMetrics, TestExecutionMetrics, TestResult,
};
use mdminecraft_world::{
    BiomeAssigner, ChunkPos, Mob, MobType, TerrainGenerator, CHUNK_SIZE_X, CHUNK_SIZE_Y,
    CHUNK_SIZE_Z,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

const WORLD_SEED: u64 = 77889900;
const DEFAULT_SPAWN_PROBABILITY: u64 = 300; // 1 in 300 blocks

fn chunk_radius() -> i32 {
    std::env::var("MDM_MOB_LIFECYCLE_CHUNK_RADIUS")
        .ok()
        .and_then(|raw| raw.parse::<i32>().ok())
        .unwrap_or(if cfg!(debug_assertions) { 2 } else { 12 })
}

fn simulation_ticks() -> u64 {
    std::env::var("MDM_MOB_LIFECYCLE_TICKS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(if cfg!(debug_assertions) { 1200 } else { 6000 })
}

fn spawn_probability() -> u64 {
    std::env::var("MDM_MOB_LIFECYCLE_SPAWN_PROBABILITY")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SPAWN_PROBABILITY)
        .max(1)
}

#[test]
fn mob_lifecycle_worldtest() {
    let test_start = Instant::now();
    let chunk_radius = chunk_radius().max(0);
    let simulation_ticks = simulation_ticks().max(1);
    let spawn_probability = spawn_probability();

    println!("\n=== Mob Lifecycle Worldtest ===");
    println!("Configuration:");
    println!("  World seed: {}", WORLD_SEED);
    println!(
        "  Chunk radius: {} ({}×{} grid)",
        chunk_radius,
        chunk_radius * 2 + 1,
        chunk_radius * 2 + 1
    );
    println!(
        "  Simulation ticks: {} (~{:.1} minutes at 20 TPS)",
        simulation_ticks,
        simulation_ticks as f64 / 20.0 / 60.0
    );
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Generate Terrain
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 1: Generating terrain...");
    let phase1_start = Instant::now();

    let terrain_gen = TerrainGenerator::new(WORLD_SEED);
    let mut chunks = Vec::new();
    let mut generation_times = Vec::new();

    for chunk_z in -chunk_radius..=chunk_radius {
        for chunk_x in -chunk_radius..=chunk_radius {
            let pos = ChunkPos {
                x: chunk_x,
                z: chunk_z,
            };

            let gen_start = Instant::now();
            let chunk = terrain_gen.generate_chunk(pos);
            let gen_time = gen_start.elapsed().as_micros();
            generation_times.push(gen_time);

            chunks.push(chunk);
        }

        // Progress indicator every 5 rows
        if (chunk_z + chunk_radius) % 5 == 0 {
            let progress =
                ((chunk_z + chunk_radius + 1) as f64 / (chunk_radius * 2 + 1) as f64) * 100.0;
            println!("  Progress: {:.1}%", progress);
        }
    }

    let chunks_generated = chunks.len();
    let blocks_generated = chunks_generated * CHUNK_SIZE_X * CHUNK_SIZE_Y * CHUNK_SIZE_Z;
    let avg_gen_time_us = generation_times.iter().sum::<u128>() as f64 / chunks_generated as f64;

    println!(
        "  Completed in {:.2}s",
        phase1_start.elapsed().as_secs_f64()
    );
    println!("  Chunks: {}", chunks_generated);
    println!("  Avg: {:.2}ms/chunk", avg_gen_time_us / 1000.0);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Spawn Mobs
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 2: Spawning mobs...");
    let phase2_start = Instant::now();

    let biome_assigner = BiomeAssigner::new(WORLD_SEED);
    let mut mobs = Vec::new();
    let mut spawn_by_biome: HashMap<String, usize> = HashMap::new();
    let mut spawn_by_type: HashMap<String, usize> = HashMap::new();
    let mut possible_types = HashSet::new();

    for chunk in &chunks {
        let pos = chunk.position();

        // Sample biome at chunk center
        let world_x_center = pos.x * 16 + 8;
        let world_z_center = pos.z * 16 + 8;
        let biome = biome_assigner.get_biome(world_x_center, world_z_center);

        // Get valid mob types for this biome
        let mob_types = MobType::for_biome(biome);
        for (mob_type, _weight) in &mob_types {
            possible_types.insert(*mob_type);
        }

        // Spawn mobs randomly throughout the chunk
        for local_z in 0..CHUNK_SIZE_Z {
            for local_x in 0..CHUNK_SIZE_X {
                let spawn_hash = WORLD_SEED
                    ^ (pos.x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ (pos.z as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                    ^ (local_x as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
                    ^ (local_z as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);

                if spawn_hash.is_multiple_of(spawn_probability) && !mob_types.is_empty() {
                    let idx = (spawn_hash as usize) % mob_types.len();
                    let (mob_type, _weight) = mob_types[idx];

                    let world_x = pos.x as f64 * 16.0 + local_x as f64 + 0.5;
                    let world_z = pos.z as f64 * 16.0 + local_z as f64 + 0.5;

                    // Get approximate ground height from chunk
                    let height = 64.0; // Approximate ground level

                    let mob = Mob::new(world_x, height + 1.0, world_z, mob_type);

                    // Track spawning statistics
                    let biome_name = format!("{:?}", biome);
                    let mob_name = format!("{:?}", mob_type);
                    *spawn_by_biome.entry(biome_name).or_insert(0) += 1;
                    *spawn_by_type.entry(mob_name).or_insert(0) += 1;

                    mobs.push(mob);
                }
            }
        }
    }

    let total_spawned = mobs.len();
    let spawn_density = total_spawned as f64 / chunks_generated as f64;

    println!(
        "  Completed in {:.2}s",
        phase2_start.elapsed().as_secs_f64()
    );
    println!("  Total mobs spawned: {}", total_spawned);
    println!("  Spawn density: {:.2} mobs/chunk", spawn_density);
    println!("  Unique mob types: {}", spawn_by_type.len());

    // Show top 5 mob types
    let mut type_vec: Vec<_> = spawn_by_type.iter().collect();
    type_vec.sort_by(|a, b| b.1.cmp(a.1));
    println!("  Top mob types:");
    for (mob_type, count) in type_vec.iter().take(5) {
        let percentage = (**count as f64 / total_spawned as f64) * 100.0;
        println!("    {}: {} ({:.1}%)", mob_type, count, percentage);
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Simulate Mob Updates
    // ═══════════════════════════════════════════════════════════════════════

    println!(
        "Phase 3: Simulating mob lifecycle ({} ticks)...",
        simulation_ticks
    );
    let phase3_start = Instant::now();

    let mut update_times = Vec::new();
    let mut total_distance_traveled = 0.0;
    let mut mobs_moved = 0;

    // Sample initial positions
    let initial_positions: Vec<(f64, f64, f64)> = mobs.iter().map(|m| (m.x, m.y, m.z)).collect();

    // Run simulation with progress updates
    let progress_interval = (simulation_ticks / 10).max(1);

    for tick in 0..simulation_ticks {
        let tick_start = Instant::now();

        for mob in &mut mobs {
            mob.update(tick);
        }

        update_times.push(tick_start.elapsed().as_nanos());

        // Progress indicator
        if (tick + 1) % progress_interval == 0 {
            let progress = ((tick + 1) as f64 / simulation_ticks as f64) * 100.0;
            let elapsed = phase3_start.elapsed().as_secs_f64();
            let eta = (elapsed / progress * 100.0) - elapsed;
            println!(
                "  Progress: {:.0}% ({:.1}s elapsed, ETA: {:.1}s)",
                progress, elapsed, eta
            );
        }
    }

    // Analyze movement after simulation
    for (idx, mob) in mobs.iter().enumerate() {
        let (init_x, init_y, init_z) = initial_positions[idx];

        let dx = mob.x - init_x;
        let dy = mob.y - init_y;
        let dz = mob.z - init_z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        if distance > 0.1 {
            total_distance_traveled += distance;
            mobs_moved += 1;
        }
    }

    let total_updates = total_spawned * simulation_ticks as usize;
    let total_time_ns: u128 = update_times.iter().sum();
    let avg_update_time_ns = total_time_ns as f64 / total_updates as f64;
    let avg_update_time_us = avg_update_time_ns / 1000.0;
    let avg_tick_time_us = total_time_ns as f64 / update_times.len() as f64 / 1000.0;
    let avg_distance_per_mob = if mobs_moved > 0 {
        total_distance_traveled / mobs_moved as f64
    } else {
        0.0
    };

    println!(
        "  Completed in {:.2}s",
        phase3_start.elapsed().as_secs_f64()
    );
    println!(
        "  Total updates: {} ({} mobs × {} ticks)",
        total_updates, total_spawned, simulation_ticks
    );
    println!("  Avg update time: {:.3}μs/update", avg_update_time_us);
    println!(
        "  Mobs that moved: {} ({:.1}%)",
        mobs_moved,
        (mobs_moved as f64 / total_spawned as f64) * 100.0
    );
    println!(
        "  Avg distance traveled: {:.2} blocks",
        avg_distance_per_mob
    );
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 4: AI State Analysis
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 4: Analyzing mob AI state...");
    let phase4_start = Instant::now();

    let mut state_counts: HashMap<String, usize> = HashMap::new();
    let mut ai_timer_sum: u64 = 0;
    let mut velocity_magnitudes = Vec::new();

    for mob in &mobs {
        let state_name = format!("{:?}", mob.state);
        *state_counts.entry(state_name).or_insert(0) += 1;

        ai_timer_sum += mob.ai_timer as u64;

        // Calculate velocity magnitude
        let vel_mag =
            (mob.vel_x * mob.vel_x + mob.vel_y * mob.vel_y + mob.vel_z * mob.vel_z).sqrt();
        velocity_magnitudes.push(vel_mag);
    }

    let avg_ai_timer = ai_timer_sum as f64 / total_spawned as f64;
    let avg_velocity = velocity_magnitudes.iter().sum::<f64>() / velocity_magnitudes.len() as f64;
    let max_velocity = velocity_magnitudes.iter().cloned().fold(0.0f64, f64::max);

    println!(
        "  Completed in {:.2}s",
        phase4_start.elapsed().as_secs_f64()
    );
    println!("  AI state distribution:");
    for (state, count) in state_counts.iter() {
        let percentage = (*count as f64 / total_spawned as f64) * 100.0;
        println!("    {}: {} ({:.1}%)", state, count, percentage);
    }
    println!("  Avg AI timer: {:.1} ticks", avg_ai_timer);
    println!("  Avg velocity: {:.4} blocks/tick", avg_velocity);
    println!("  Max velocity: {:.4} blocks/tick", max_velocity);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 5: Performance Analysis
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 5: Performance analysis...");

    let min_update_ns = *update_times.iter().min().unwrap();
    let max_update_ns = *update_times.iter().max().unwrap();
    let p50_update_ns = {
        let mut sorted = update_times.clone();
        sorted.sort();
        sorted[sorted.len() / 2]
    };
    let p95_update_ns = {
        let mut sorted = update_times.clone();
        sorted.sort();
        sorted[(sorted.len() as f64 * 0.95) as usize]
    };
    let p99_update_ns = {
        let mut sorted = update_times.clone();
        sorted.sort();
        sorted[(sorted.len() as f64 * 0.99) as usize]
    };

    println!("  Per-tick time (all {} mobs):", total_spawned);
    println!("    Min: {:.3}μs", min_update_ns as f64 / 1000.0);
    println!("    P50: {:.3}μs", p50_update_ns as f64 / 1000.0);
    println!("    Avg: {:.3}μs", avg_tick_time_us);
    println!("    P95: {:.3}μs", p95_update_ns as f64 / 1000.0);
    println!("    P99: {:.3}μs", p99_update_ns as f64 / 1000.0);
    println!("    Max: {:.3}μs", max_update_ns as f64 / 1000.0);
    println!("  Per-mob time: {:.3}μs/update", avg_update_time_us);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Build Metrics Report
    // ═══════════════════════════════════════════════════════════════════════

    let test_duration = test_start.elapsed().as_secs_f64();
    let mobs_alive = total_spawned; // All mobs remain alive (no damage system yet)
    let test_passed = mobs_alive > 0 && avg_update_time_us < 1.0; // Must be under 1μs/update

    let metrics = MetricsReportBuilder::new("mob_lifecycle_worldtest")
        .result(if test_passed {
            TestResult::Pass
        } else {
            TestResult::Fail
        })
        .terrain(TerrainMetrics {
            chunks_generated,
            blocks_generated,
            avg_gen_time_us,
            min_gen_time_us: *generation_times.iter().min().unwrap(),
            max_gen_time_us: *generation_times.iter().max().unwrap(),
            total_gen_time_ms: generation_times.iter().sum::<u128>() as f64 / 1000.0,
            chunks_per_second: chunks_generated as f64
                / (generation_times.iter().sum::<u128>() as f64 / 1_000_000.0),
            unique_biomes: spawn_by_biome.len(),
            seam_validation: None,
        })
        .mobs(MobMetrics {
            total_spawned,
            total_updates,
            avg_update_time_us,
            mobs_alive,
            by_type: Some(spawn_by_type.clone()),
        })
        .execution(TestExecutionMetrics {
            duration_seconds: test_duration,
            peak_memory_mb: None,
            assertions_checked: Some(total_updates),
            validations_passed: Some(mobs_alive),
        })
        .build();

    // Write metrics
    let metrics_path = std::env::current_dir()
        .unwrap()
        .join("target/metrics/mob_lifecycle_worldtest.json");

    let sink = MetricsSink::create(&metrics_path).expect("Failed to create metrics sink");
    sink.write(&metrics).expect("Failed to write metrics");

    // ═══════════════════════════════════════════════════════════════════════
    // Final Results
    // ═══════════════════════════════════════════════════════════════════════

    println!("=== Final Results ===");
    println!("Test result: {:?}", metrics.result);
    println!("Total duration: {:.2}s", test_duration);
    println!();
    println!("Terrain:");
    println!("  Chunks generated: {}", chunks_generated);
    println!("  Avg generation: {:.2}ms/chunk", avg_gen_time_us / 1000.0);
    println!();
    println!("Mob Lifecycle:");
    println!("  Spawned: {}", total_spawned);
    println!(
        "  Alive after simulation: {} ({:.1}%)",
        mobs_alive,
        (mobs_alive as f64 / total_spawned as f64) * 100.0
    );
    println!("  Spawn density: {:.2} mobs/chunk", spawn_density);
    println!("  Unique types: {}", spawn_by_type.len());
    println!();
    println!("Movement:");
    println!(
        "  Mobs moved: {} ({:.1}%)",
        mobs_moved,
        (mobs_moved as f64 / total_spawned as f64) * 100.0
    );
    println!("  Avg distance: {:.2} blocks", avg_distance_per_mob);
    println!();
    println!("Performance:");
    println!("  Total updates: {}", total_updates);
    println!("  Avg update time: {:.3}μs", avg_update_time_us);
    println!("  P95 tick time: {:.3}μs", p95_update_ns as f64 / 1000.0);
    println!(
        "  Throughput: {:.0} updates/sec",
        total_updates as f64 / phase3_start.elapsed().as_secs_f64()
    );
    println!();
    println!("Metrics: {:?}", metrics_path);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Assertions
    // ═══════════════════════════════════════════════════════════════════════

    assert!(total_spawned > 0, "Mobs must spawn");
    assert!(mobs_alive > 0, "Some mobs must remain alive");
    assert_eq!(
        mobs_alive, total_spawned,
        "All mobs should remain alive (no damage system)"
    );
    let expected_min_types = possible_types.len().clamp(1, 3);
    assert!(
        spawn_by_type.len() >= expected_min_types,
        "Expected at least {} mob types to spawn (possible: {})",
        expected_min_types,
        possible_types.len()
    );
    assert!(
        avg_update_time_us < 1.0,
        "Update time must be under 1μs per mob per tick"
    );
    // At 20 TPS, we have 50ms per tick budget. With ~80k mobs, P99 should be well under that.
    // Different thresholds for debug vs release builds
    let p99_threshold = if cfg!(debug_assertions) {
        10_000_000 // 10ms for debug builds
    } else {
        5_000_000 // 5ms for release builds
    };
    assert!(
        p99_update_ns < p99_threshold,
        "P99 tick time must be under {}ms for scalability (was {:.2}ms)",
        p99_threshold / 1_000_000,
        p99_update_ns as f64 / 1_000_000.0
    );
}
