//! Property-based tests for chunk seam continuity
//!
//! Validates that chunk boundaries are always continuous regardless of:
//! - World seed
//! - Chunk coordinates
//! - Adjacent chunk positions
//!
//! Critical invariants:
//! - Height difference at seams <= 20 blocks
//! - Biome transitions are smooth
//! - No gaps or overlaps

use mdminecraft_world::{BiomeAssigner, Heightmap, CHUNK_SIZE_X, CHUNK_SIZE_Z};
use proptest::prelude::*;

/// Maximum allowed height difference at chunk boundaries (blocks)
const MAX_SEAM_DIFF: i32 = 20;

proptest! {
    /// Property: Adjacent chunks always have continuous heightmaps at X-axis seams
    ///
    /// For any world seed and any two adjacent chunks along the X-axis,
    /// the height difference at their shared boundary must be <= 20 blocks.
    #[test]
    fn heightmap_x_seam_continuity(
        world_seed in any::<u64>(),
        chunk_x in -100i32..100i32,
        chunk_z in -100i32..100i32,
    ) {
        let hm1 = Heightmap::generate(world_seed, chunk_x, chunk_z);
        let hm2 = Heightmap::generate(world_seed, chunk_x + 1, chunk_z);

        // Check all positions along the X-axis boundary
        for z in 0..CHUNK_SIZE_Z {
            let h1 = hm1.get(CHUNK_SIZE_X - 1, z);
            let h2 = hm2.get(0, z);
            let diff = (h1 - h2).abs();

            prop_assert!(
                diff <= MAX_SEAM_DIFF,
                "X-seam discontinuity at chunk ({}, {}) z={}: height diff = {} (max: {})",
                chunk_x, chunk_z, z, diff, MAX_SEAM_DIFF
            );
        }
    }

    /// Property: Adjacent chunks always have continuous heightmaps at Z-axis seams
    ///
    /// For any world seed and any two adjacent chunks along the Z-axis,
    /// the height difference at their shared boundary must be <= 20 blocks.
    #[test]
    fn heightmap_z_seam_continuity(
        world_seed in any::<u64>(),
        chunk_x in -100i32..100i32,
        chunk_z in -100i32..100i32,
    ) {
        let hm1 = Heightmap::generate(world_seed, chunk_x, chunk_z);
        let hm2 = Heightmap::generate(world_seed, chunk_x, chunk_z + 1);

        // Check all positions along the Z-axis boundary
        for x in 0..CHUNK_SIZE_X {
            let h1 = hm1.get(x, CHUNK_SIZE_Z - 1);
            let h2 = hm2.get(x, 0);
            let diff = (h1 - h2).abs();

            prop_assert!(
                diff <= MAX_SEAM_DIFF,
                "Z-seam discontinuity at chunk ({}, {}) x={}: height diff = {} (max: {})",
                chunk_x, chunk_z, x, diff, MAX_SEAM_DIFF
            );
        }
    }

    /// Property: Heightmap generation is deterministic
    ///
    /// For any world seed and chunk coordinates,
    /// generating the same chunk twice produces identical results.
    #[test]
    fn heightmap_determinism(
        world_seed in any::<u64>(),
        chunk_x in -100i32..100i32,
        chunk_z in -100i32..100i32,
    ) {
        let hm1 = Heightmap::generate(world_seed, chunk_x, chunk_z);
        let hm2 = Heightmap::generate(world_seed, chunk_x, chunk_z);

        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let h1 = hm1.get(x, z);
                let h2 = hm2.get(x, z);
                prop_assert_eq!(
                    h1, h2,
                    "Heightmap non-deterministic at chunk ({}, {}) pos ({}, {})",
                    chunk_x, chunk_z, x, z
                );
            }
        }
    }

    /// Property: Different seeds produce different heightmaps
    ///
    /// For any two different world seeds, the generated heightmaps
    /// should differ in at least some positions.
    #[test]
    fn heightmap_seed_variation(
        seeds in (any::<u64>(), any::<u64>()).prop_filter("Seeds must be different", |(s1, s2)| s1 != s2),
        chunk_x in -50i32..50i32,
        chunk_z in -50i32..50i32,
    ) {
        let (seed1, seed2) = seeds;
        let hm1 = Heightmap::generate(seed1, chunk_x, chunk_z);
        let hm2 = Heightmap::generate(seed2, chunk_x, chunk_z);

        // At least one position should differ
        let mut any_different = false;
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if hm1.get(x, z) != hm2.get(x, z) {
                    any_different = true;
                    break;
                }
            }
            if any_different {
                break;
            }
        }

        prop_assert!(
            any_different,
            "Different seeds produced identical heightmaps at chunk ({}, {})",
            chunk_x, chunk_z
        );
    }

    /// Property: Heights are within valid range
    ///
    /// All generated heights must be within the world's valid Y range.
    #[test]
    fn heightmap_bounds(
        world_seed in any::<u64>(),
        chunk_x in -100i32..100i32,
        chunk_z in -100i32..100i32,
    ) {
        let hm = Heightmap::generate(world_seed, chunk_x, chunk_z);

        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let h = hm.get(x, z);
                prop_assert!(
                    (0..=255).contains(&h),
                    "Height {} out of bounds [0, 255] at chunk ({}, {}) pos ({}, {})",
                    h, chunk_x, chunk_z, x, z
                );
            }
        }
    }

    /// Property: Biome assignment is deterministic
    ///
    /// For any world seed and coordinates,
    /// biome assignment is consistent across multiple calls.
    #[test]
    fn biome_determinism(
        world_seed in any::<u64>(),
        world_x in -1000i32..1000i32,
        world_z in -1000i32..1000i32,
    ) {
        let assigner = BiomeAssigner::new(world_seed);
        let biome1 = assigner.get_biome(world_x, world_z);
        let biome2 = assigner.get_biome(world_x, world_z);

        prop_assert_eq!(
            biome1, biome2,
            "Biome assignment non-deterministic at ({}, {})",
            world_x, world_z
        );
    }

    /// Property: Corner continuity
    ///
    /// At the corners where 4 chunks meet, all heights should be
    /// reasonably consistent (not perfectly equal due to sampling, but close).
    #[test]
    fn corner_continuity(
        world_seed in any::<u64>(),
        chunk_x in -50i32..50i32,
        chunk_z in -50i32..50i32,
    ) {
        // Get the 4 chunks that meet at this corner
        let hm_tl = Heightmap::generate(world_seed, chunk_x, chunk_z);
        let hm_tr = Heightmap::generate(world_seed, chunk_x + 1, chunk_z);
        let hm_bl = Heightmap::generate(world_seed, chunk_x, chunk_z + 1);
        let hm_br = Heightmap::generate(world_seed, chunk_x + 1, chunk_z + 1);

        // Get corner heights
        let h_tl = hm_tl.get(CHUNK_SIZE_X - 1, CHUNK_SIZE_Z - 1);
        let h_tr = hm_tr.get(0, CHUNK_SIZE_Z - 1);
        let h_bl = hm_bl.get(CHUNK_SIZE_X - 1, 0);
        let h_br = hm_br.get(0, 0);

        // All corners should be within MAX_SEAM_DIFF of each other
        let heights = [h_tl, h_tr, h_bl, h_br];
        let min_h = heights.iter().min().copied().unwrap_or_default();
        let max_h = heights.iter().max().copied().unwrap_or_default();
        let diff = max_h - min_h;

        prop_assert!(
            diff <= MAX_SEAM_DIFF,
            "Corner discontinuity at chunk ({}, {}): height range = {} (max: {})",
            chunk_x, chunk_z, diff, MAX_SEAM_DIFF
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn known_good_seam() {
        // Test a known good case
        let world_seed = 42;
        let hm1 = Heightmap::generate(world_seed, 0, 0);
        let hm2 = Heightmap::generate(world_seed, 1, 0);

        for z in 0..CHUNK_SIZE_Z {
            let h1 = hm1.get(CHUNK_SIZE_X - 1, z);
            let h2 = hm2.get(0, z);
            let diff = (h1 - h2).abs();
            assert!(
                diff <= MAX_SEAM_DIFF,
                "Known good seam failed at z={}: diff={}",
                z,
                diff
            );
        }
    }

    #[test]
    fn determinism_sanity_check() {
        let hm1 = Heightmap::generate(123, 5, 10);
        let hm2 = Heightmap::generate(123, 5, 10);

        assert_eq!(hm1.get(0, 0), hm2.get(0, 0));
        assert_eq!(hm1.get(8, 8), hm2.get(8, 8));
        assert_eq!(hm1.get(15, 15), hm2.get(15, 15));
    }
}
