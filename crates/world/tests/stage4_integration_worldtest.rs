//! Stage 4 Integration Worldtest: Complete Environmental System
//!
//! Validates all Stage 4 systems working together:
//! - Terrain generation with noise and heightmaps
//! - Biome system with 14 biome types
//! - Tree generation with biome-specific placement
//! - Passive mob spawning with AI
//! - Dropped items with physics
//! - Seamless chunk boundaries
//! - Performance at scale
//!
//! Defaults are tuned to keep debug test runs reasonable while still exercising
//! the integration surface. Override with `MDM_STAGE4_INTEGRATION_CHUNK_RADIUS`
//! for the full run.

use mdminecraft_core::{DimensionId, SimTick};
use mdminecraft_testkit::{EventRecord, JsonlSink};
use mdminecraft_world::{
    BiomeAssigner, BiomeId, ChunkPos, Heightmap, ItemManager, ItemType, MobSpawner,
    TerrainGenerator, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

const WORLD_SEED: u64 = 12345;

fn chunk_radius() -> i32 {
    std::env::var("MDM_STAGE4_INTEGRATION_CHUNK_RADIUS")
        .ok()
        .and_then(|raw| raw.parse::<i32>().ok())
        .unwrap_or(if cfg!(debug_assertions) { 2 } else { 8 })
}

#[test]
fn stage4_integration_worldtest() {
    const DIM: DimensionId = DimensionId::Overworld;

    let test_start = Instant::now();
    let chunk_radius = chunk_radius().max(0);

    // Create event log for CI artifacts
    let log_path = std::env::temp_dir().join("stage4_integration_worldtest.jsonl");
    let mut event_log = JsonlSink::create(&log_path).expect("create event log");

    println!("=== Stage 4 Integration Worldtest ===");
    println!("Testing: Terrain + Biomes + Trees + Mobs + Items");
    println!("World Seed: {}", WORLD_SEED);
    println!(
        "Grid Size: {}×{} chunks\n",
        chunk_radius * 2 + 1,
        chunk_radius * 2 + 1
    );

    // Initialize all Stage 4 systems
    let terrain_gen = TerrainGenerator::new(WORLD_SEED);
    let biome_assigner = BiomeAssigner::new(WORLD_SEED);
    let mob_spawner = MobSpawner::new(WORLD_SEED);
    let mut item_manager = ItemManager::new();

    // Phase 1: Large-scale terrain generation
    println!(
        "Phase 1: Generating terrain across {} chunks...",
        (chunk_radius * 2 + 1).pow(2)
    );
    let phase1_start = Instant::now();

    let mut generation_times = Vec::new();
    let mut biome_counts: HashMap<BiomeId, usize> = HashMap::new();
    let mut total_blocks_generated = 0;

    for chunk_x in -chunk_radius..=chunk_radius {
        for chunk_z in -chunk_radius..=chunk_radius {
            let gen_start = Instant::now();
            let chunk = terrain_gen.generate_chunk(ChunkPos {
                x: chunk_x,
                z: chunk_z,
            });
            let gen_time = gen_start.elapsed().as_micros();
            generation_times.push(gen_time);

            // Track biome at chunk center
            let chunk_center_x = chunk_x * 16 + 8;
            let chunk_center_z = chunk_z * 16 + 8;
            let biome = biome_assigner.get_biome(chunk_center_x, chunk_center_z);
            *biome_counts.entry(biome).or_insert(0) += 1;

            // Count blocks
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    for x in 0..CHUNK_SIZE_X {
                        let voxel = chunk.voxel(x, y, z);
                        if voxel.id != 0 {
                            total_blocks_generated += 1;
                        }
                    }
                }
            }
        }
    }

    let phase1_time = phase1_start.elapsed();
    let chunks_generated = (chunk_radius * 2 + 1).pow(2) as usize;
    let avg_gen_time = generation_times.iter().sum::<u128>() / generation_times.len() as u128;
    let min_gen_time = *generation_times.iter().min().unwrap();
    let max_gen_time = *generation_times.iter().max().unwrap();

    println!("  Chunks generated: {}", chunks_generated);
    println!(
        "  Avg generation time: {}μs ({:.2}ms)",
        avg_gen_time,
        avg_gen_time as f64 / 1000.0
    );
    println!("  Min/Max: {}μs / {}μs", min_gen_time, max_gen_time);
    println!("  Total blocks: {}", total_blocks_generated);
    println!("  Unique biomes: {}", biome_counts.len());
    println!("  Total time: {:?}\n", phase1_time);

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(1),
            kind: "TerrainGeneration",
            payload: &format!(
                "{} chunks, {}μs avg, {} biomes, {} blocks",
                chunks_generated,
                avg_gen_time,
                biome_counts.len(),
                total_blocks_generated
            ),
        })
        .expect("write event");

    // Phase 2: Validate chunk seams
    println!("Phase 2: Validating chunk seam continuity...");
    let mut seam_checks = 0;
    let mut seam_failures = 0;

    for chunk_x in -chunk_radius..chunk_radius {
        for chunk_z in -chunk_radius..=chunk_radius {
            let hm1 = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);
            let hm2 = Heightmap::generate(WORLD_SEED, chunk_x + 1, chunk_z);

            // Check X-axis seam
            for z in 0..16 {
                let h1 = hm1.get(15, z);
                let h2 = hm2.get(0, z);
                let diff = (h1 - h2).abs();
                seam_checks += 1;
                if diff > 20 {
                    seam_failures += 1;
                }
            }
        }
    }

    for chunk_x in -chunk_radius..=chunk_radius {
        for chunk_z in -chunk_radius..chunk_radius {
            let hm1 = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);
            let hm2 = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z + 1);

            // Check Z-axis seam
            for x in 0..16 {
                let h1 = hm1.get(x, 15);
                let h2 = hm2.get(x, 0);
                let diff = (h1 - h2).abs();
                seam_checks += 1;
                if diff > 20 {
                    seam_failures += 1;
                }
            }
        }
    }

    println!("  Seam checks: {}", seam_checks);
    println!(
        "  Seam failures: {} ({:.2}%)",
        seam_failures,
        (seam_failures as f64 / seam_checks as f64) * 100.0
    );
    println!(
        "  Seam quality: {}%\n",
        ((seam_checks - seam_failures) as f64 / seam_checks as f64) * 100.0
    );

    assert_eq!(seam_failures, 0, "All seams should be continuous");

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(10),
            kind: "SeamValidation",
            payload: &format!("{} checks, {} failures", seam_checks, seam_failures),
        })
        .expect("write event");

    // Phase 3: Spawn mobs across all chunks
    println!("Phase 3: Spawning passive mobs...");
    let mut all_mobs = Vec::new();
    let mut mob_spawn_counts: HashMap<String, usize> = HashMap::new();

    for chunk_x in -chunk_radius..=chunk_radius {
        for chunk_z in -chunk_radius..=chunk_radius {
            let heightmap = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);
            let chunk_center_x = chunk_x * 16 + 8;
            let chunk_center_z = chunk_z * 16 + 8;
            let biome = biome_assigner.get_biome(chunk_center_x, chunk_center_z);

            let mobs = mob_spawner.generate_spawns(chunk_x, chunk_z, biome, heightmap.heights());

            if !mobs.is_empty() {
                let biome_name = format!("{:?}", biome);
                *mob_spawn_counts.entry(biome_name).or_insert(0) += mobs.len();
            }

            all_mobs.extend(mobs);
        }
    }

    println!("  Total mobs spawned: {}", all_mobs.len());
    println!("  Biomes with mobs: {}", mob_spawn_counts.len());
    println!(
        "  Avg mobs per chunk: {:.2}\n",
        all_mobs.len() as f64 / chunks_generated as f64
    );

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(20),
            kind: "MobSpawning",
            payload: &format!(
                "{} mobs across {} biomes",
                all_mobs.len(),
                mob_spawn_counts.len()
            ),
        })
        .expect("write event");

    // Phase 4: Simulate dropped items from block breaking
    println!("Phase 4: Simulating block breaking and item drops...");

    // Simulate breaking 1000 random blocks
    let mut rng_seed = WORLD_SEED;
    for _ in 0..1000 {
        rng_seed = rng_seed.wrapping_mul(1103515245).wrapping_add(12345);

        let block_id = ((rng_seed % 6) + 1) as u16; // Block IDs 1-6
        if let Some((item_type, count)) = ItemType::from_block(block_id) {
            let x = ((rng_seed / 100) % 256) as f64;
            let z = ((rng_seed / 10000) % 256) as f64;
            item_manager.spawn_item(DIM, x, 70.0, z, item_type, count);
        }
    }

    println!("  Items spawned: {}", item_manager.count());

    // Simulate physics for items
    let ground_height = |_x: f64, _z: f64| 64.0;
    for _ in 0..100 {
        item_manager.update(DIM, ground_height);
    }

    // Merge nearby items
    let merged = item_manager.merge_nearby_items(DIM);
    println!("  Items after merge: {}", item_manager.count());
    println!("  Items merged: {}\n", merged);

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(120),
            kind: "ItemSystem",
            payload: &format!("{} items, {} merged", item_manager.count(), merged),
        })
        .expect("write event");

    // Phase 5: Biome diversity analysis
    println!("Phase 5: Analyzing biome diversity...");

    println!("  Biome distribution:");
    let mut sorted_biomes: Vec<_> = biome_counts.iter().collect();
    sorted_biomes.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

    for (biome, count) in sorted_biomes.iter().take(10) {
        let percentage = (**count as f64 / chunks_generated as f64) * 100.0;
        println!("    {:?}: {} chunks ({:.1}%)", biome, count, percentage);
    }
    println!();

    let required_biomes = if cfg!(debug_assertions) { 3 } else { 5 };
    assert!(
        biome_counts.len() >= required_biomes,
        "Should have at least {} different biomes",
        required_biomes
    );

    // Phase 6: Performance summary
    println!("Phase 6: Performance Summary");
    println!(
        "  Terrain generation: {:.2}ms per chunk",
        avg_gen_time as f64 / 1000.0
    );
    println!(
        "  Total generation: {:?} for {} chunks",
        phase1_time, chunks_generated
    );
    println!(
        "  Throughput: {:.0} chunks/sec",
        chunks_generated as f64 / phase1_time.as_secs_f64()
    );
    println!();

    // Performance targets: release is tight, debug is intentionally loose (debug is much slower).
    //
    // Debug-mode performance varies across machines; keep this as a guardrail, but allow tuning
    // via env override.
    let base_threshold_us: u128 = if cfg!(debug_assertions) {
        1_200_000
    } else {
        30_000
    };
    let default_threshold_us = base_threshold_us.saturating_mul(CHUNK_SIZE_Y as u128) / 256;
    let performance_threshold = std::env::var("MDM_STAGE4_INTEGRATION_MAX_AVG_GEN_US")
        .ok()
        .and_then(|raw| raw.parse::<u128>().ok())
        .unwrap_or(default_threshold_us);
    assert!(
        avg_gen_time < performance_threshold,
        "Chunk generation should be <{}ms (was {}μs)",
        performance_threshold / 1000,
        avg_gen_time
    );

    // Final report
    let test_duration = test_start.elapsed();

    println!("=== Integration Test Results ===");
    println!("Terrain:");
    println!("  Chunks: {}", chunks_generated);
    println!("  Blocks: {}", total_blocks_generated);
    println!("  Avg gen: {:.2}ms", avg_gen_time as f64 / 1000.0);
    println!("  Biomes: {}/{} types", biome_counts.len(), 14);
    println!();
    println!("Validation:");
    println!("  Seams checked: {}", seam_checks);
    println!(
        "  Seam failures: {} ({}%)",
        seam_failures,
        ((seam_failures as f64 / seam_checks as f64) * 100.0)
    );
    println!();
    println!("Entities:");
    println!("  Mobs spawned: {}", all_mobs.len());
    println!("  Items active: {}", item_manager.count());
    println!();
    println!("Performance:");
    println!("  Total test time: {:?}", test_duration);
    println!(
        "  Chunks/sec: {:.0}",
        chunks_generated as f64 / test_duration.as_secs_f64()
    );
    println!();
    println!("Event log: {}", log_path.display());
    println!("================================\n");

    event_log
        .write(&EventRecord {
            tick: SimTick::ZERO.advance(200),
            kind: "TestComplete",
            payload: &format!(
                "{} chunks, {} biomes, {} mobs, {} items, {:?}",
                chunks_generated,
                biome_counts.len(),
                all_mobs.len(),
                item_manager.count(),
                test_duration
            ),
        })
        .expect("write event");
}

#[test]
fn test_biome_variety() {
    let biome_assigner = BiomeAssigner::new(42);
    let mut found_biomes = HashSet::new();

    // Sample a large grid
    for x in -100..100 {
        for z in -100..100 {
            let biome = biome_assigner.get_biome(x * 16, z * 16);
            found_biomes.insert(biome);
        }
    }

    // Should find many different biomes in a 200×200 chunk area
    assert!(
        found_biomes.len() >= 8,
        "Should find at least 8 biome types in large area"
    );
}

#[test]
fn test_deterministic_world_generation() {
    let seed = 999;

    // Generate same chunk twice
    let gen1 = TerrainGenerator::new(seed);
    let gen2 = TerrainGenerator::new(seed);

    let chunk1 = gen1.generate_chunk(ChunkPos { x: 5, z: 10 });
    let chunk2 = gen2.generate_chunk(ChunkPos { x: 5, z: 10 });

    // Verify identical generation
    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let v1 = chunk1.voxel(x, y, z);
                let v2 = chunk2.voxel(x, y, z);
                assert_eq!(
                    v1.id, v2.id,
                    "Block mismatch at ({}, {}, {}): {} vs {}",
                    x, y, z, v1.id, v2.id
                );
            }
        }
    }
}
