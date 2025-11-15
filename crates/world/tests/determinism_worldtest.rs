//! Determinism Validation Worldtest
//!
//! This test validates that world generation is completely deterministic.
//! Focus areas:
//! - Same seed produces identical chunks
//! - Chunk generation order independence
//! - Biome assignment consistency
//! - Heightmap reproducibility
//! - Voxel data exact matching
//! - Cross-platform determinism

use mdminecraft_testkit::{
    MetricsReportBuilder, MetricsSink, TestResult, TerrainMetrics,
    TestExecutionMetrics,
};
use mdminecraft_world::{
    BiomeAssigner, ChunkPos, Heightmap, TerrainGenerator,
    CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use std::collections::HashMap;
use std::time::Instant;

const WORLD_SEED: u64 = 11223344556677;
const CHUNK_RADIUS: i32 = 8; // 17×17 grid = 289 chunks
const VERIFICATION_ROUNDS: usize = 3; // Regenerate 3 times to verify consistency

#[test]
fn determinism_worldtest() {
    let test_start = Instant::now();

    println!("\n=== Determinism Validation Worldtest ===");
    println!("Configuration:");
    println!("  World seed: {}", WORLD_SEED);
    println!("  Chunk radius: {} ({}×{} grid)", CHUNK_RADIUS, CHUNK_RADIUS * 2 + 1, CHUNK_RADIUS * 2 + 1);
    println!("  Verification rounds: {}", VERIFICATION_ROUNDS);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Initial Generation (Sequential)
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 1: Initial generation (sequential order)...");
    let phase1_start = Instant::now();

    let terrain_gen = TerrainGenerator::new(WORLD_SEED);
    let mut chunks_sequential = Vec::new();
    let mut positions = Vec::new();
    let mut generation_times = Vec::new();

    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let pos = ChunkPos { x: chunk_x, z: chunk_z };
            positions.push(pos);

            let gen_start = Instant::now();
            let chunk = terrain_gen.generate_chunk(pos);
            let gen_time = gen_start.elapsed().as_micros();
            generation_times.push(gen_time);

            chunks_sequential.push(chunk);
        }
    }

    let chunks_generated = chunks_sequential.len();
    let blocks_generated = chunks_generated * CHUNK_SIZE_X * CHUNK_SIZE_Y * CHUNK_SIZE_Z;
    let avg_gen_time_us = generation_times.iter().sum::<u128>() as f64 / chunks_generated as f64;

    println!("  Completed in {:.2}s", phase1_start.elapsed().as_secs_f64());
    println!("  Chunks: {}", chunks_generated);
    println!("  Avg: {:.2}ms/chunk", avg_gen_time_us / 1000.0);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Regeneration (Random Order)
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 2: Regeneration (randomized order)...");
    let phase2_start = Instant::now();

    // Generate in different order to verify order independence
    let mut randomized_positions = positions.clone();
    // Simple deterministic shuffle based on seed
    for i in 0..randomized_positions.len() {
        let j = ((i as u64).wrapping_mul(WORLD_SEED) % randomized_positions.len() as u64) as usize;
        randomized_positions.swap(i, j);
    }

    let mut chunks_randomized = HashMap::new();
    for pos in &randomized_positions {
        let chunk = terrain_gen.generate_chunk(*pos);
        chunks_randomized.insert(*pos, chunk);
    }

    println!("  Completed in {:.2}s", phase2_start.elapsed().as_secs_f64());
    println!("  Verified order independence");
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Voxel-Level Comparison
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 3: Voxel-level comparison...");
    let phase3_start = Instant::now();

    let mut voxel_mismatches = 0;
    let mut total_voxels_checked = 0;
    let mut chunks_with_mismatches = 0;

    for (idx, original_chunk) in chunks_sequential.iter().enumerate() {
        let pos = positions[idx];
        let regenerated_chunk = chunks_randomized.get(&pos).unwrap();

        assert_eq!(original_chunk.position(), regenerated_chunk.position(),
            "Chunk positions must match");

        let mut chunk_has_mismatch = false;

        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let original = original_chunk.voxel(x, y, z);
                    let regenerated = regenerated_chunk.voxel(x, y, z);

                    total_voxels_checked += 1;

                    if original.id != regenerated.id ||
                       original.state != regenerated.state ||
                       original.light_sky != regenerated.light_sky ||
                       original.light_block != regenerated.light_block {
                        voxel_mismatches += 1;
                        chunk_has_mismatch = true;
                    }
                }
            }
        }

        if chunk_has_mismatch {
            chunks_with_mismatches += 1;
        }
    }

    let fidelity_rate = (total_voxels_checked - voxel_mismatches) as f64 / total_voxels_checked as f64 * 100.0;

    println!("  Completed in {:.2}s", phase3_start.elapsed().as_secs_f64());
    println!("  Total voxels checked: {}", total_voxels_checked);
    println!("  Mismatches: {}", voxel_mismatches);
    println!("  Fidelity: {:.12}%", fidelity_rate);
    println!("  Chunks with mismatches: {}/{}", chunks_with_mismatches, chunks_generated);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 4: Multiple Regeneration Rounds
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 4: Multiple regeneration rounds...");
    let phase4_start = Instant::now();

    let mut round_mismatches = vec![0; VERIFICATION_ROUNDS];

    for round in 0..VERIFICATION_ROUNDS {
        for (idx, original_chunk) in chunks_sequential.iter().enumerate() {
            let pos = positions[idx];
            let regenerated = terrain_gen.generate_chunk(pos);

            // Spot check voxels (sample every 8th block to save time)
            for y in (0..CHUNK_SIZE_Y).step_by(8) {
                for z in (0..CHUNK_SIZE_Z).step_by(2) {
                    for x in (0..CHUNK_SIZE_X).step_by(2) {
                        let original = original_chunk.voxel(x, y, z);
                        let regen = regenerated.voxel(x, y, z);

                        if original.id != regen.id {
                            round_mismatches[round] += 1;
                        }
                    }
                }
            }
        }

        println!("  Round {}: {} mismatches", round + 1, round_mismatches[round]);
    }

    println!("  Completed in {:.2}s", phase4_start.elapsed().as_secs_f64());
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 5: Biome Consistency
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 5: Biome consistency validation...");
    let phase5_start = Instant::now();

    let biome_assigner_1 = BiomeAssigner::new(WORLD_SEED);
    let biome_assigner_2 = BiomeAssigner::new(WORLD_SEED);

    let mut biome_mismatches = 0;
    let mut total_biome_samples = 0;

    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
            for local_z in (0..CHUNK_SIZE_Z).step_by(4) {
                for local_x in (0..CHUNK_SIZE_X).step_by(4) {
                    let world_x = chunk_x * 16 + local_x as i32;
                    let world_z = chunk_z * 16 + local_z as i32;

                    let biome_1 = biome_assigner_1.get_biome(world_x, world_z);
                    let biome_2 = biome_assigner_2.get_biome(world_x, world_z);

                    total_biome_samples += 1;

                    if biome_1 as u8 != biome_2 as u8 {
                        biome_mismatches += 1;
                    }
                }
            }
        }
    }

    println!("  Completed in {:.2}s", phase5_start.elapsed().as_secs_f64());
    println!("  Biome samples: {}", total_biome_samples);
    println!("  Biome mismatches: {}", biome_mismatches);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 6: Heightmap Consistency
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 6: Heightmap consistency validation...");
    let phase6_start = Instant::now();

    let mut heightmap_mismatches = 0;
    let mut total_height_samples = 0;

    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let hm1 = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);
            let hm2 = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);

            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let h1 = hm1.get(x, z);
                    let h2 = hm2.get(x, z);

                    total_height_samples += 1;

                    if h1 != h2 {
                        heightmap_mismatches += 1;
                    }
                }
            }
        }
    }

    println!("  Completed in {:.2}s", phase6_start.elapsed().as_secs_f64());
    println!("  Height samples: {}", total_height_samples);
    println!("  Height mismatches: {}", heightmap_mismatches);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Build Metrics Report
    // ═══════════════════════════════════════════════════════════════════════

    let test_duration = test_start.elapsed().as_secs_f64();
    let total_regenerations = chunks_generated * (1 + VERIFICATION_ROUNDS);
    let all_checks_passed = voxel_mismatches == 0 &&
                            biome_mismatches == 0 &&
                            heightmap_mismatches == 0 &&
                            round_mismatches.iter().sum::<usize>() == 0;

    let test_passed = all_checks_passed;

    let metrics = MetricsReportBuilder::new("determinism_worldtest")
        .result(if test_passed { TestResult::Pass } else { TestResult::Fail })
        .terrain(TerrainMetrics {
            chunks_generated: total_regenerations,
            blocks_generated,
            avg_gen_time_us,
            min_gen_time_us: *generation_times.iter().min().unwrap(),
            max_gen_time_us: *generation_times.iter().max().unwrap(),
            total_gen_time_ms: generation_times.iter().sum::<u128>() as f64 / 1000.0,
            chunks_per_second: chunks_generated as f64 / (generation_times.iter().sum::<u128>() as f64 / 1_000_000.0),
            unique_biomes: 0, // Not measured in this test
            seam_validation: None,
        })
        .execution(TestExecutionMetrics {
            duration_seconds: test_duration,
            peak_memory_mb: None,
            assertions_checked: Some(total_voxels_checked + total_biome_samples + total_height_samples),
            validations_passed: Some(total_voxels_checked - voxel_mismatches),
        })
        .build();

    // Write metrics
    let metrics_path = std::env::current_dir()
        .unwrap()
        .join("target/metrics/determinism_worldtest.json");

    let sink = MetricsSink::create(&metrics_path)
        .expect("Failed to create metrics sink");
    sink.write(&metrics).expect("Failed to write metrics");

    // ═══════════════════════════════════════════════════════════════════════
    // Final Results
    // ═══════════════════════════════════════════════════════════════════════

    println!("=== Final Results ===");
    println!("Test result: {:?}", metrics.result);
    println!("Total duration: {:.2}s", test_duration);
    println!();
    println!("Generation:");
    println!("  Chunks: {} (initial) + {} (regenerations) = {} total",
        chunks_generated, total_regenerations - chunks_generated, total_regenerations);
    println!("  Avg generation: {:.2}ms/chunk", avg_gen_time_us / 1000.0);
    println!();
    println!("Determinism Validation:");
    println!("  Voxel fidelity: {:.12}% ({}/{} voxels)",
        fidelity_rate, total_voxels_checked - voxel_mismatches, total_voxels_checked);
    println!("  Biome consistency: {}/{} samples",
        total_biome_samples - biome_mismatches, total_biome_samples);
    println!("  Heightmap consistency: {}/{} heights",
        total_height_samples - heightmap_mismatches, total_height_samples);
    println!("  Multi-round verification: {} rounds, {} total mismatches",
        VERIFICATION_ROUNDS, round_mismatches.iter().sum::<usize>());
    println!();
    println!("Metrics: {:?}", metrics_path);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Assertions
    // ═══════════════════════════════════════════════════════════════════════

    assert_eq!(voxel_mismatches, 0, "All voxels must match exactly between generations");
    assert_eq!(chunks_with_mismatches, 0, "No chunks should have any mismatches");
    assert_eq!(biome_mismatches, 0, "Biome assignment must be deterministic");
    assert_eq!(heightmap_mismatches, 0, "Heightmap generation must be deterministic");
    assert_eq!(round_mismatches.iter().sum::<usize>(), 0,
        "All regeneration rounds must produce identical results");
    assert!(test_passed, "All determinism checks must pass");
}
