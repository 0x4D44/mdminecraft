//! Persistence Round-Trip Worldtest
//!
//! This test validates chunk persistence through complete save/load cycles.
//! Focus areas:
//! - Chunk serialization correctness
//! - Region file format integrity
//! - Compression and CRC validation
//! - Save/load performance
//! - Multi-chunk region handling
//! - Data fidelity across round-trips

use mdminecraft_testkit::{
    MetricsReportBuilder, MetricsSink, PersistenceMetrics, TerrainMetrics, TestExecutionMetrics,
    TestResult,
};
use mdminecraft_world::{
    ChunkPos, RegionStore, TerrainGenerator, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use std::env;
use std::time::Instant;

const WORLD_SEED: u64 = 55667788;
const CHUNK_RADIUS: i32 = 4; // 9×9 grid = 81 chunks (spread across multiple regions)

#[test]
fn persistence_roundtrip_worldtest() {
    let test_start = Instant::now();

    println!("\n=== Persistence Round-Trip Worldtest ===");
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

    // Create temporary directory for region files
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = env::temp_dir().join(format!("mdminecraft_persist_test_{}", timestamp));
    println!("Temp directory: {:?}", temp_dir);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Generate Chunks
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 1: Generating chunks...");
    let phase1_start = Instant::now();

    let terrain_gen = TerrainGenerator::new(WORLD_SEED);
    let mut chunks = Vec::new();
    let mut generation_times = Vec::new();

    for chunk_z in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for chunk_x in -CHUNK_RADIUS..=CHUNK_RADIUS {
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
    }

    let chunks_generated = chunks.len();
    let blocks_generated = chunks_generated * CHUNK_SIZE_X * CHUNK_SIZE_Y * CHUNK_SIZE_Z;
    let avg_gen_time_us = generation_times.iter().sum::<u128>() as f64 / chunks_generated as f64;

    println!(
        "  Generated {} chunks in {:.2}s",
        chunks_generated,
        phase1_start.elapsed().as_secs_f64()
    );
    println!("  Average: {:.2}ms/chunk", avg_gen_time_us / 1000.0);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Save Chunks to Region Files
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 2: Saving chunks to region files...");
    let phase2_start = Instant::now();

    let store = RegionStore::new(&temp_dir).expect("Failed to create region store");
    let mut save_times = Vec::new();
    let mut bytes_written: u64 = 0;

    for chunk in &chunks {
        let save_start = Instant::now();
        store.save_chunk(chunk).expect("Failed to save chunk");
        let save_time = save_start.elapsed().as_micros();
        save_times.push(save_time);
    }

    // Count region files created
    let region_files: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rg"))
        .collect();

    for entry in &region_files {
        bytes_written += entry.metadata().unwrap().len();
    }

    let avg_save_time_us = save_times.iter().sum::<u128>() as f64 / chunks_generated as f64;

    println!(
        "  Saved {} chunks in {:.2}s",
        chunks_generated,
        phase2_start.elapsed().as_secs_f64()
    );
    println!("  Average: {:.2}ms/chunk", avg_save_time_us / 1000.0);
    println!("  Region files: {}", region_files.len());
    println!(
        "  Bytes written: {} ({:.2} MB)",
        bytes_written,
        bytes_written as f64 / 1_048_576.0
    );
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Load Chunks from Region Files
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 3: Loading chunks from region files...");
    let phase3_start = Instant::now();

    let mut load_times = Vec::new();
    let mut loaded_chunks = Vec::new();

    for chunk in &chunks {
        let pos = chunk.position();

        let load_start = Instant::now();
        let loaded = store.load_chunk(pos).expect("Failed to load chunk");
        let load_time = load_start.elapsed().as_micros();
        load_times.push(load_time);

        loaded_chunks.push(loaded);
    }

    let avg_load_time_us = load_times.iter().sum::<u128>() as f64 / chunks_generated as f64;

    println!(
        "  Loaded {} chunks in {:.2}s",
        chunks_generated,
        phase3_start.elapsed().as_secs_f64()
    );
    println!("  Average: {:.2}ms/chunk", avg_load_time_us / 1000.0);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 4: Verify Data Fidelity
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 4: Verifying data fidelity...");
    let phase4_start = Instant::now();

    let mut fidelity_failures = 0;
    let mut total_voxels_checked = 0;

    for (original, loaded) in chunks.iter().zip(loaded_chunks.iter()) {
        assert_eq!(
            original.position(),
            loaded.position(),
            "Chunk position mismatch"
        );

        // Sample voxels throughout the chunk (not all to save time)
        for y in (0..CHUNK_SIZE_Y).step_by(16) {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let original_voxel = original.voxel(x, y, z);
                    let loaded_voxel = loaded.voxel(x, y, z);

                    total_voxels_checked += 1;

                    if original_voxel.id != loaded_voxel.id
                        || original_voxel.state != loaded_voxel.state
                        || original_voxel.light_sky != loaded_voxel.light_sky
                        || original_voxel.light_block != loaded_voxel.light_block
                    {
                        fidelity_failures += 1;
                    }
                }
            }
        }
    }

    let fidelity_rate =
        (total_voxels_checked - fidelity_failures) as f64 / total_voxels_checked as f64 * 100.0;

    println!(
        "  Verified {} voxels in {:.2}s",
        total_voxels_checked,
        phase4_start.elapsed().as_secs_f64()
    );
    println!(
        "  Fidelity: {:.6}% ({} mismatches)",
        fidelity_rate, fidelity_failures
    );
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 5: Test Region File Reloading
    // ═══════════════════════════════════════════════════════════════════════

    println!("Phase 5: Testing region file reloading...");
    let phase5_start = Instant::now();

    // Drop the store and create a new one to ensure files are properly closed
    drop(store);

    let store2 = RegionStore::new(&temp_dir).expect("Failed to recreate region store");
    let mut reload_failures = 0;

    for chunk in &chunks {
        let pos = chunk.position();

        match store2.load_chunk(pos) {
            Ok(reloaded) => {
                assert_eq!(chunk.position(), reloaded.position());

                // Spot check a few voxels
                for sample in 0..10 {
                    let x = (sample * 7) % CHUNK_SIZE_X;
                    let y = (sample * 13) % CHUNK_SIZE_Y;
                    let z = (sample * 11) % CHUNK_SIZE_Z;

                    let original_voxel = chunk.voxel(x, y, z);
                    let reloaded_voxel = reloaded.voxel(x, y, z);

                    if original_voxel.id != reloaded_voxel.id {
                        reload_failures += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to reload chunk {:?}: {}", pos, e);
                reload_failures += 1;
            }
        }
    }

    println!(
        "  Reloaded {} chunks in {:.2}s",
        chunks_generated,
        phase5_start.elapsed().as_secs_f64()
    );
    println!("  Reload failures: {}", reload_failures);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Calculate Compression Ratio
    // ═══════════════════════════════════════════════════════════════════════

    let uncompressed_size = blocks_generated * std::mem::size_of::<Voxel>();
    let compression_ratio = uncompressed_size as f64 / bytes_written as f64;

    println!("Compression Analysis:");
    println!(
        "  Uncompressed: {:.2} MB",
        uncompressed_size as f64 / 1_048_576.0
    );
    println!("  Compressed: {:.2} MB", bytes_written as f64 / 1_048_576.0);
    println!("  Ratio: {:.2}×", compression_ratio);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Build Metrics Report
    // ═══════════════════════════════════════════════════════════════════════

    let test_duration = test_start.elapsed().as_secs_f64();
    let test_passed = fidelity_failures == 0 && reload_failures == 0;

    let metrics = MetricsReportBuilder::new("persistence_roundtrip_worldtest")
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
            unique_biomes: 0, // Not measured in this test
            seam_validation: None,
        })
        .persistence(PersistenceMetrics {
            chunks_saved: chunks_generated,
            chunks_loaded: chunks_generated,
            avg_save_time_us,
            avg_load_time_us,
            bytes_written,
            bytes_read: bytes_written, // Approximate
            compression_ratio,
        })
        .execution(TestExecutionMetrics {
            duration_seconds: test_duration,
            peak_memory_mb: None,
            assertions_checked: Some(total_voxels_checked),
            validations_passed: Some(total_voxels_checked - fidelity_failures),
        })
        .build();

    // Write metrics
    let metrics_path = std::env::current_dir()
        .unwrap()
        .join("target/metrics/persistence_roundtrip_worldtest.json");

    let sink = MetricsSink::create(&metrics_path).expect("Failed to create metrics sink");
    sink.write(&metrics).expect("Failed to write metrics");

    // ═══════════════════════════════════════════════════════════════════════
    // Cleanup
    // ═══════════════════════════════════════════════════════════════════════

    drop(store2);
    std::fs::remove_dir_all(&temp_dir).ok();

    // ═══════════════════════════════════════════════════════════════════════
    // Final Results
    // ═══════════════════════════════════════════════════════════════════════

    println!("=== Final Results ===");
    println!("Test result: {:?}", metrics.result);
    println!("Total duration: {:.2}s", test_duration);
    println!(
        "Chunks: {} generated, {} saved, {} loaded",
        chunks_generated, chunks_generated, chunks_generated
    );
    println!("Save performance: {:.2}ms/chunk", avg_save_time_us / 1000.0);
    println!("Load performance: {:.2}ms/chunk", avg_load_time_us / 1000.0);
    println!("Compression: {:.2}×", compression_ratio);
    println!("Data fidelity: {:.6}%", fidelity_rate);
    println!("Metrics: {:?}", metrics_path);
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Assertions
    // ═══════════════════════════════════════════════════════════════════════

    assert_eq!(
        fidelity_failures, 0,
        "All voxel data must match after round-trip"
    );
    assert_eq!(reload_failures, 0, "All chunks must reload successfully");
    // Note: Save/load times are high because we save each chunk individually,
    // which causes region file rewrites. In production, chunks are batched.
    // These limits reflect the current test pattern, not optimal performance.
    assert!(
        avg_save_time_us < 1_000_000.0,
        "Save time must be under 1000ms/chunk"
    );
    assert!(
        avg_load_time_us < 1_000_000.0,
        "Load time must be under 1000ms/chunk"
    );
    assert!(compression_ratio > 3.0, "Compression ratio must be > 3.0×");
}
