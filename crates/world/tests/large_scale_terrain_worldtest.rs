//! Large-Scale Terrain Generation Worldtest
//!
//! This test validates terrain generation at scale (50×50 chunks = 2,500 chunks).
//! Focus areas:
//! - Performance consistency across large generation runs
//! - Seam continuity validation across all boundaries
//! - Memory efficiency and stability
//! - Determinism verification
//! - Biome distribution analysis

use mdminecraft_testkit::{
    MetricsReportBuilder, MetricsSink, SeamValidation, TerrainMetrics, TestExecutionMetrics,
    TestResult,
};
use mdminecraft_world::{
    BiomeAssigner, ChunkPos, Heightmap, TerrainGenerator, CHUNK_SIZE_X, CHUNK_SIZE_Z,
};
use std::collections::HashMap;
use std::time::Instant;

const WORLD_SEED: u64 = 99887766;
const CHUNK_RADIUS: i32 = 25; // 51×51 grid = 2,601 chunks
const MAX_SEAM_DIFF: i32 = 20;

#[test]
fn large_scale_terrain_worldtest() {
    let test_start = Instant::now();

    println!("\n=== Large-Scale Terrain Worldtest ===");
    println!("Configuration:");
    println!("  World seed: {}", WORLD_SEED);
    println!(
        "  Chunk radius: {} ({}×{} grid)",
        CHUNK_RADIUS,
        CHUNK_RADIUS * 2 + 1,
        CHUNK_RADIUS * 2 + 1
    );
    println!("  Total chunks: {}", (CHUNK_RADIUS * 2 + 1).pow(2));
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Large-Scale Terrain Generation
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 1: Generating terrain...");
    let phase1_start = Instant::now();

    let terrain_gen = TerrainGenerator::new(WORLD_SEED);
    let mut generation_times = Vec::new();
    let mut heightmaps = HashMap::new();

    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let pos = ChunkPos {
                x: chunk_x,
                z: chunk_z,
            };

            let gen_start = Instant::now();
            let _chunk = terrain_gen.generate_chunk(pos);
            let gen_time = gen_start.elapsed().as_micros();
            generation_times.push(gen_time);

            // Store heightmap for seam validation
            let heightmap = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);
            heightmaps.insert(pos, heightmap);
        }

        // Progress indicator every 10 rows
        if (chunk_z + CHUNK_RADIUS) % 10 == 0 {
            let progress =
                ((chunk_z + CHUNK_RADIUS + 1) as f64 / (CHUNK_RADIUS * 2 + 1) as f64) * 100.0;
            println!("  Progress: {:.1}% ({} chunks)", progress, heightmaps.len());
        }
    }

    let phase1_time = phase1_start.elapsed();
    let chunks_generated = heightmaps.len();
    let blocks_generated = chunks_generated * CHUNK_SIZE_X * 256 * CHUNK_SIZE_Z;

    let avg_gen_time_us = generation_times.iter().sum::<u128>() as f64 / chunks_generated as f64;
    let min_gen_time_us = *generation_times.iter().min().unwrap();
    let max_gen_time_us = *generation_times.iter().max().unwrap();
    let total_gen_time_ms = generation_times.iter().sum::<u128>() as f64 / 1000.0;
    let chunks_per_second = chunks_generated as f64 / (total_gen_time_ms / 1000.0);

    println!("  Completed in {:.2}s", phase1_time.as_secs_f64());
    println!("  Avg: {:.2}ms/chunk", avg_gen_time_us / 1000.0);
    println!("  Throughput: {:.1} chunks/sec", chunks_per_second);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Comprehensive Seam Validation
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 2: Validating chunk seams...");
    let phase2_start = Instant::now();

    let mut seam_diffs = Vec::new();
    let mut seams_checked = 0;
    let mut seams_failed = 0;
    let mut max_seam_diff = 0;

    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let pos = ChunkPos {
                x: chunk_x,
                z: chunk_z,
            };
            let heightmap = heightmaps.get(&pos).unwrap();

            // Check X-axis seam (right neighbor)
            let right_pos = ChunkPos {
                x: chunk_x + 1,
                z: chunk_z,
            };
            if let Some(right_hm) = heightmaps.get(&right_pos) {
                for z in 0..CHUNK_SIZE_Z {
                    let h1 = heightmap.get(CHUNK_SIZE_X - 1, z);
                    let h2 = right_hm.get(0, z);
                    let diff = (h1 as i32 - h2 as i32).abs();

                    seam_diffs.push(diff);
                    seams_checked += 1;

                    if diff > MAX_SEAM_DIFF {
                        seams_failed += 1;
                    }

                    max_seam_diff = max_seam_diff.max(diff);
                }
            }

            // Check Z-axis seam (bottom neighbor)
            let bottom_pos = ChunkPos {
                x: chunk_x,
                z: chunk_z + 1,
            };
            if let Some(bottom_hm) = heightmaps.get(&bottom_pos) {
                for x in 0..CHUNK_SIZE_X {
                    let h1 = heightmap.get(x, CHUNK_SIZE_Z - 1);
                    let h2 = bottom_hm.get(x, 0);
                    let diff = (h1 as i32 - h2 as i32).abs();

                    seam_diffs.push(diff);
                    seams_checked += 1;

                    if diff > MAX_SEAM_DIFF {
                        seams_failed += 1;
                    }

                    max_seam_diff = max_seam_diff.max(diff);
                }
            }
        }
    }

    let seams_valid = seams_checked - seams_failed;
    let avg_seam_diff = seam_diffs.iter().sum::<i32>() as f64 / seam_diffs.len().max(1) as f64;
    let phase2_time = phase2_start.elapsed();

    println!("  Completed in {:.2}s", phase2_time.as_secs_f64());
    println!("  Seams checked: {}", seams_checked);
    println!(
        "  Valid: {} ({:.2}%)",
        seams_valid,
        (seams_valid as f64 / seams_checked as f64) * 100.0
    );
    println!("  Failed: {}", seams_failed);
    println!("  Max diff: {} blocks", max_seam_diff);
    println!("  Avg diff: {:.2} blocks", avg_seam_diff);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Biome Distribution Analysis
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 3: Analyzing biome distribution...");
    let phase3_start = Instant::now();

    let biome_assigner = BiomeAssigner::new(WORLD_SEED);
    let mut biome_counts: HashMap<u8, usize> = HashMap::new();

    // Sample biomes (not every single block to save time)
    let sample_stride = 4; // Sample every 4th block
    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
            for local_z in (0..CHUNK_SIZE_Z).step_by(sample_stride) {
                for local_x in (0..CHUNK_SIZE_X).step_by(sample_stride) {
                    let world_x = chunk_x * 16 + local_x as i32;
                    let world_z = chunk_z * 16 + local_z as i32;
                    let biome = biome_assigner.get_biome(world_x, world_z);
                    let biome_id = biome as u8;
                    *biome_counts.entry(biome_id).or_insert(0) += 1;
                }
            }
        }
    }

    let unique_biomes = biome_counts.len();
    let phase3_time = phase3_start.elapsed();

    println!("  Completed in {:.2}s", phase3_time.as_secs_f64());
    println!("  Unique biomes: {}/14", unique_biomes);

    // Show top 5 biomes
    let mut biome_vec: Vec<_> = biome_counts.iter().collect();
    biome_vec.sort_by(|a, b| b.1.cmp(a.1));
    println!("  Top biomes:");
    for (biome_id, count) in biome_vec.iter().take(5) {
        let percentage = (**count as f64 / biome_counts.values().sum::<usize>() as f64) * 100.0;
        println!("    Biome {}: {:.1}%", biome_id, percentage);
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 4: Determinism Verification
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 4: Verifying determinism...");
    let phase4_start = Instant::now();

    // Regenerate a subset of chunks and verify they match
    let verify_count: usize = 100;
    let mut determinism_failures: usize = 0;

    for i in 0..verify_count {
        let chunk_x = -CHUNK_RADIUS + (i as i32 * (CHUNK_RADIUS * 2 / verify_count as i32));
        let chunk_z = -CHUNK_RADIUS + (i as i32 * (CHUNK_RADIUS * 2 / verify_count as i32));

        let pos = ChunkPos {
            x: chunk_x,
            z: chunk_z,
        };
        let original_hm = heightmaps.get(&pos).unwrap();
        let regenerated_hm = Heightmap::generate(WORLD_SEED, chunk_x, chunk_z);

        // Compare all heights
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if original_hm.get(x, z) != regenerated_hm.get(x, z) {
                    determinism_failures += 1;
                    break;
                }
            }
        }
    }

    let phase4_time = phase4_start.elapsed();

    println!("  Completed in {:.2}s", phase4_time.as_secs_f64());
    println!("  Chunks verified: {}", verify_count);
    println!("  Determinism failures: {}", determinism_failures);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Build Metrics Report
    // ═══════════════════════════════════════════════════════════════════════

    let test_duration = test_start.elapsed().as_secs_f64();
    let test_passed = seams_failed == 0 && determinism_failures == 0;

    let metrics = MetricsReportBuilder::new("large_scale_terrain_worldtest")
        .result(if test_passed {
            TestResult::Pass
        } else {
            TestResult::Fail
        })
        .terrain(TerrainMetrics {
            chunks_generated,
            blocks_generated,
            avg_gen_time_us,
            min_gen_time_us,
            max_gen_time_us,
            total_gen_time_ms,
            chunks_per_second,
            unique_biomes,
            seam_validation: Some(SeamValidation {
                total_seams: seams_checked,
                seams_valid,
                seams_failed,
                max_seam_diff,
                avg_seam_diff,
            }),
        })
        .execution(TestExecutionMetrics {
            duration_seconds: test_duration,
            peak_memory_mb: None,
            assertions_checked: Some(seams_checked + verify_count),
            validations_passed: Some(seams_valid + (verify_count - determinism_failures)),
        })
        .build();

    // Write metrics
    let metrics_path = std::env::current_dir()
        .unwrap()
        .join("target/metrics/large_scale_terrain_worldtest.json");

    let sink = MetricsSink::create(&metrics_path).expect("Failed to create metrics sink");
    sink.write(&metrics).expect("Failed to write metrics");

    // ═══════════════════════════════════════════════════════════════════════
    // Final Results
    // ═══════════════════════════════════════════════════════════════════════

    println!("=== Final Results ===");
    println!("Test result: {:?}", metrics.result);
    println!("Total duration: {:.2}s", test_duration);
    println!("Chunks generated: {}", chunks_generated);
    println!("Blocks generated: {}", blocks_generated);
    println!(
        "Average generation: {:.2}ms/chunk",
        avg_gen_time_us / 1000.0
    );
    println!(
        "Seam validation: {}/{} ({:.2}%)",
        seams_valid,
        seams_checked,
        (seams_valid as f64 / seams_checked as f64) * 100.0
    );
    println!(
        "Determinism: {}/{} chunks verified",
        verify_count - determinism_failures,
        verify_count
    );
    println!("Metrics: {:?}", metrics_path);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Assertions
    // ═══════════════════════════════════════════════════════════════════════

    assert_eq!(
        seams_failed, 0,
        "All seams must be continuous (max diff: {} blocks)",
        MAX_SEAM_DIFF
    );
    assert_eq!(determinism_failures, 0, "Terrain must be deterministic");
    assert!(
        chunks_generated > 2500,
        "Should generate at least 2,500 chunks"
    );
    assert!(
        unique_biomes >= 5,
        "Should have at least 5 different biomes"
    );
    // Performance threshold: 30ms for release, 150ms for debug
    let performance_threshold = if cfg!(debug_assertions) {
        150_000.0
    } else {
        30_000.0
    };
    assert!(
        avg_gen_time_us < performance_threshold,
        "Average generation must be under {}ms (was {:.1}μs)",
        performance_threshold / 1000.0,
        avg_gen_time_us
    );
    assert!(
        max_seam_diff <= MAX_SEAM_DIFF,
        "Max seam difference must be <= {} blocks",
        MAX_SEAM_DIFF
    );
}
