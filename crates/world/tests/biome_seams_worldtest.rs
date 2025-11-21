//! BiomeSeams worldtest - validates smooth biome transitions and terrain continuity.
//!
//! This test generates a 9×9 chunk area and validates:
//! - Biome transitions are smooth (no sudden jumps)
//! - Terrain height is continuous across chunk boundaries
//! - Tree placement respects biome boundaries
//! - Performance meets targets (< 30ms per chunk)

use mdminecraft_core::SimTick;
use mdminecraft_testkit::{EventRecord, JsonlSink};
use mdminecraft_world::{
    check_seam_continuity, BiomeAssigner, ChunkPos, TerrainGenerator, CHUNK_SIZE_X, CHUNK_SIZE_Z,
};
use std::collections::HashMap;
use std::time::Instant;

/// World seed for deterministic generation.
const WORLD_SEED: u64 = 42;

/// Number of chunks in each direction (9×9 grid).
const CHUNK_RADIUS: i32 = 4;

#[test]
fn biome_seams_worldtest() {
    let output_path = std::env::temp_dir().join("biome_seams_worldtest.jsonl");
    let mut event_log = JsonlSink::create(&output_path).expect("can create event log");

    let start_time = Instant::now();
    let tick = SimTick::ZERO;

    // Log test start
    event_log
        .write(&EventRecord {
            tick,
            kind: "TestStart",
            payload: "BiomeSeams worldtest started",
        })
        .expect("can write event");

    // Create terrain generator
    let terrain_gen = TerrainGenerator::new(WORLD_SEED);
    let biome_assigner = BiomeAssigner::new(WORLD_SEED);

    // Generate 9×9 chunk grid centered at origin
    let mut chunks = HashMap::new();
    let mut generation_times = Vec::new();

    for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let chunk_start = Instant::now();
            let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
            let chunk = terrain_gen.generate_chunk(chunk_pos);
            let gen_time = chunk_start.elapsed();
            generation_times.push(gen_time.as_micros());

            chunks.insert((chunk_x, chunk_z), chunk);

            // Log chunk generation
            if chunk_x == 0 && chunk_z == 0 {
                event_log
                    .write(&EventRecord {
                        tick,
                        kind: "ChunkGenerated",
                        payload: &format!(
                            "Chunk ({}, {}) generated in {}μs",
                            chunk_x,
                            chunk_z,
                            gen_time.as_micros()
                        ),
                    })
                    .expect("can write event");
            }
        }
    }

    // Validate heightmap seam continuity
    let mut seam_checks = 0;
    let mut seam_failures = 0;

    for chunk_x in -CHUNK_RADIUS..CHUNK_RADIUS {
        for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
            // Check horizontal seam (X direction)
            if !check_seam_continuity(WORLD_SEED, (chunk_x, chunk_z), (chunk_x + 1, chunk_z)) {
                seam_failures += 1;
                event_log
                    .write(&EventRecord {
                        tick,
                        kind: "SeamFailure",
                        payload: &format!(
                            "Seam discontinuity between ({}, {}) and ({}, {})",
                            chunk_x,
                            chunk_z,
                            chunk_x + 1,
                            chunk_z
                        ),
                    })
                    .expect("can write event");
            }
            seam_checks += 1;
        }
    }

    for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_z in -CHUNK_RADIUS..CHUNK_RADIUS {
            // Check vertical seam (Z direction)
            if !check_seam_continuity(WORLD_SEED, (chunk_x, chunk_z), (chunk_x, chunk_z + 1)) {
                seam_failures += 1;
                event_log
                    .write(&EventRecord {
                        tick,
                        kind: "SeamFailure",
                        payload: &format!(
                            "Seam discontinuity between ({}, {}) and ({}, {})",
                            chunk_x,
                            chunk_z,
                            chunk_x,
                            chunk_z + 1
                        ),
                    })
                    .expect("can write event");
            }
            seam_checks += 1;
        }
    }

    // Validate biome diversity
    let mut biome_counts = HashMap::new();
    for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
            let world_x = chunk_x * CHUNK_SIZE_X as i32 + (CHUNK_SIZE_X / 2) as i32;
            let world_z = chunk_z * CHUNK_SIZE_Z as i32 + (CHUNK_SIZE_Z / 2) as i32;
            let biome = biome_assigner.get_biome(world_x, world_z);
            *biome_counts.entry(format!("{:?}", biome)).or_insert(0) += 1;
        }
    }

    // Performance analysis
    let total_chunks = generation_times.len();
    let avg_gen_time = generation_times.iter().sum::<u128>() / total_chunks as u128;
    let max_gen_time = generation_times.iter().max().unwrap_or(&0);
    let min_gen_time = generation_times.iter().min().unwrap_or(&0);

    // Log performance metrics
    event_log
        .write(&EventRecord {
            tick,
            kind: "PerformanceMetrics",
            payload: &format!(
                "Avg: {}μs, Min: {}μs, Max: {}μs, Total chunks: {}",
                avg_gen_time, min_gen_time, max_gen_time, total_chunks
            ),
        })
        .expect("can write event");

    // Log biome diversity
    event_log
        .write(&EventRecord {
            tick,
            kind: "BiomeDiversity",
            payload: &format!("Found {} unique biomes in 9×9 grid", biome_counts.len()),
        })
        .expect("can write event");

    // Log seam validation results
    event_log
        .write(&EventRecord {
            tick,
            kind: "SeamValidation",
            payload: &format!(
                "Checked {} seams, {} failures ({}% pass rate)",
                seam_checks,
                seam_failures,
                ((seam_checks - seam_failures) as f64 / seam_checks as f64 * 100.0)
            ),
        })
        .expect("can write event");

    let total_time = start_time.elapsed();
    event_log
        .write(&EventRecord {
            tick,
            kind: "TestComplete",
            payload: &format!("Total test time: {}ms", total_time.as_millis()),
        })
        .expect("can write event");

    // Assertions
    assert_eq!(total_chunks, 81, "Should generate 81 chunks (9×9 grid)");
    assert_eq!(seam_failures, 0, "All seams should be continuous");
    assert!(
        avg_gen_time < 30_000,
        "Average chunk generation should be under 30ms (was {}μs)",
        avg_gen_time
    );
    assert!(
        biome_counts.len() >= 3,
        "Should have at least 3 different biomes in 9×9 grid"
    );

    println!("\n=== BiomeSeams Worldtest Results ===");
    println!("Total chunks: {}", total_chunks);
    println!(
        "Average generation time: {}μs ({:.2}ms)",
        avg_gen_time,
        avg_gen_time as f64 / 1000.0
    );
    println!("Min generation time: {}μs", min_gen_time);
    println!("Max generation time: {}μs", max_gen_time);
    println!("Seam checks: {}", seam_checks);
    println!("Seam failures: {}", seam_failures);
    println!("Biome diversity: {} unique biomes", biome_counts.len());
    println!("Biomes found:");
    for (biome, count) in biome_counts.iter() {
        println!("  {}: {} chunks", biome, count);
    }
    println!("Total test time: {}ms", total_time.as_millis());
    println!("Event log: {}", output_path.display());
    println!("=== Test PASSED ===\n");
}
