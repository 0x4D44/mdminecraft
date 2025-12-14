//! Heightmap generation for terrain.
//!
//! Converts noise layers into block heights for chunk generation.

use crate::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z};
use crate::noise::LayeredNoise;

/// Minimum height for terrain generation.
pub const MIN_HEIGHT: i32 = 0;

/// Maximum height for terrain generation.
pub const MAX_HEIGHT: i32 = 255;

/// Sea level height.
pub const SEA_LEVEL: i32 = 64;

/// Base height for terrain (ground level).
pub const BASE_HEIGHT: i32 = 64;

/// Maximum height variation above base.
pub const HEIGHT_VARIATION: i32 = 64;

/// Heightmap for a single chunk (16x16).
///
/// Each value represents the topmost solid block Y coordinate at that (x, z) position.
pub struct Heightmap {
    /// Height values for each (x, z) position in the chunk.
    /// Indexed as heights[z][x] for cache-friendly iteration.
    heights: [[i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z],
}

impl Heightmap {
    /// Generate a heightmap for the given chunk coordinates.
    ///
    /// # Arguments
    /// * `world_seed` - World seed for deterministic generation
    /// * `chunk_x` - Chunk X coordinate
    /// * `chunk_z` - Chunk Z coordinate
    ///
    /// # Returns
    /// A heightmap with heights in range [MIN_HEIGHT, MAX_HEIGHT].
    pub fn generate(world_seed: u64, chunk_x: i32, chunk_z: i32) -> Self {
        let noise = LayeredNoise::new(world_seed);
        let mut heights = [[0i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z];

        // Calculate the world-space origin of this chunk
        let chunk_origin_x = chunk_x * CHUNK_SIZE_X as i32;
        let chunk_origin_z = chunk_z * CHUNK_SIZE_Z as i32;

        // Generate height for each column in the chunk
        for (local_z, column) in heights.iter_mut().enumerate() {
            for (local_x, cell) in column.iter_mut().enumerate() {
                // Calculate world-space coordinates
                let world_x = chunk_origin_x + local_x as i32;
                let world_z = chunk_origin_z + local_z as i32;

                // Sample noise at world coordinates
                let noise_value = noise.sample_height(world_x as f64, world_z as f64);

                // Convert noise [-1.0, 1.0] to height [BASE - VAR, BASE + VAR]
                let height = BASE_HEIGHT + (noise_value * HEIGHT_VARIATION as f64) as i32;

                // Clamp to valid range
                let clamped_height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);

                *cell = clamped_height;
            }
        }

        Self { heights }
    }

    /// Get the height at a specific local (x, z) coordinate within the chunk.
    ///
    /// # Arguments
    /// * `local_x` - X coordinate within chunk [0, 15]
    /// * `local_z` - Z coordinate within chunk [0, 15]
    ///
    /// # Returns
    /// Height value in range [MIN_HEIGHT, MAX_HEIGHT].
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    pub fn get(&self, local_x: usize, local_z: usize) -> i32 {
        assert!(local_x < CHUNK_SIZE_X, "local_x out of bounds");
        assert!(local_z < CHUNK_SIZE_Z, "local_z out of bounds");
        self.heights[local_z][local_x]
    }

    /// Get a reference to the raw height array.
    ///
    /// Indexed as [z][x] for cache-friendly iteration.
    pub fn heights(&self) -> &[[i32; CHUNK_SIZE_X]; CHUNK_SIZE_Z] {
        &self.heights
    }

    /// Get the minimum height in this heightmap.
    pub fn min_height(&self) -> i32 {
        self.heights
            .iter()
            .flat_map(|row| row.iter())
            .copied()
            .min()
            .unwrap_or(MIN_HEIGHT)
    }

    /// Get the maximum height in this heightmap.
    pub fn max_height(&self) -> i32 {
        self.heights
            .iter()
            .flat_map(|row| row.iter())
            .copied()
            .max()
            .unwrap_or(MAX_HEIGHT)
    }

    /// Get the average height in this heightmap.
    pub fn avg_height(&self) -> f32 {
        let sum: i32 = self.heights.iter().flat_map(|row| row.iter()).sum();
        sum as f32 / (CHUNK_SIZE_X * CHUNK_SIZE_Z) as f32
    }
}

/// Helper function to check if adjacent chunks have reasonable height transitions (no seams).
///
/// This is a test utility to verify heightmap generation continuity.
/// Returns true if height differences at boundaries are reasonable (no sudden jumps).
pub fn check_seam_continuity(world_seed: u64, chunk1: (i32, i32), chunk2: (i32, i32)) -> bool {
    let hm1 = Heightmap::generate(world_seed, chunk1.0, chunk1.1);
    let hm2 = Heightmap::generate(world_seed, chunk2.0, chunk2.1);

    // Maximum allowed height difference at boundary (prevents visible seams)
    const MAX_BOUNDARY_DIFF: i32 = 20;

    // Determine which edge to compare based on chunk positions
    if chunk2.0 == chunk1.0 + 1 && chunk2.1 == chunk1.1 {
        // chunk2 is to the right (+X) of chunk1
        // Compare chunk1's right edge (x=15) with chunk2's left edge (x=0)
        for z in 0..CHUNK_SIZE_Z {
            let diff = (hm1.get(CHUNK_SIZE_X - 1, z) - hm2.get(0, z)).abs();
            if diff > MAX_BOUNDARY_DIFF {
                return false;
            }
        }
        true
    } else if chunk2.0 == chunk1.0 && chunk2.1 == chunk1.1 + 1 {
        // chunk2 is below (+Z) chunk1
        // Compare chunk1's bottom edge (z=15) with chunk2's top edge (z=0)
        for x in 0..CHUNK_SIZE_X {
            let diff = (hm1.get(x, CHUNK_SIZE_Z - 1) - hm2.get(x, 0)).abs();
            if diff > MAX_BOUNDARY_DIFF {
                return false;
            }
        }
        true
    } else {
        // Chunks are not adjacent
        panic!("Chunks are not adjacent");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heightmap_determinism() {
        let seed = 12345;
        let chunk_x = 10;
        let chunk_z = 20;

        let hm1 = Heightmap::generate(seed, chunk_x, chunk_z);
        let hm2 = Heightmap::generate(seed, chunk_x, chunk_z);

        // Same seed and coordinates should produce identical heightmaps
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                assert_eq!(
                    hm1.get(x, z),
                    hm2.get(x, z),
                    "Heightmap not deterministic at ({}, {})",
                    x,
                    z
                );
            }
        }
    }

    #[test]
    fn test_heightmap_range() {
        let seed = 54321;
        let hm = Heightmap::generate(seed, 0, 0);

        // All heights should be within valid range
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let height = hm.get(x, z);
                assert!(
                    height >= MIN_HEIGHT && height <= MAX_HEIGHT,
                    "Height {} at ({}, {}) out of range",
                    height,
                    x,
                    z
                );
            }
        }
    }

    #[test]
    fn test_different_seeds_produce_different_heightmaps() {
        let hm1 = Heightmap::generate(111, 0, 0);
        let hm2 = Heightmap::generate(222, 0, 0);

        // At least one height should be different
        let mut any_different = false;
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if hm1.get(x, z) != hm2.get(x, z) {
                    any_different = true;
                    break;
                }
            }
        }

        assert!(
            any_different,
            "Different seeds should produce different heightmaps"
        );
    }

    #[test]
    fn test_different_chunks_produce_different_heightmaps() {
        let seed = 999;
        let hm1 = Heightmap::generate(seed, 0, 0);
        let hm2 = Heightmap::generate(seed, 1, 0);

        // At least one height should be different
        let mut any_different = false;
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if hm1.get(x, z) != hm2.get(x, z) {
                    any_different = true;
                    break;
                }
            }
        }

        assert!(
            any_different,
            "Different chunk coordinates should produce different heightmaps"
        );
    }

    #[test]
    fn test_no_seams_between_adjacent_chunks_x() {
        let seed = 42;

        // Test horizontal adjacency (X direction)
        assert!(
            check_seam_continuity(seed, (0, 0), (1, 0)),
            "Seam detected between chunks (0,0) and (1,0)"
        );
        assert!(
            check_seam_continuity(seed, (5, 10), (6, 10)),
            "Seam detected between chunks (5,10) and (6,10)"
        );
    }

    #[test]
    fn test_no_seams_between_adjacent_chunks_z() {
        let seed = 42;

        // Test vertical adjacency (Z direction)
        assert!(
            check_seam_continuity(seed, (0, 0), (0, 1)),
            "Seam detected between chunks (0,0) and (0,1)"
        );
        assert!(
            check_seam_continuity(seed, (10, 5), (10, 6)),
            "Seam detected between chunks (10,5) and (10,6)"
        );
    }

    #[test]
    fn test_heightmap_stats() {
        let seed = 777;
        let hm = Heightmap::generate(seed, 0, 0);

        let min = hm.min_height();
        let max = hm.max_height();
        let avg = hm.avg_height();

        // Basic sanity checks
        assert!(min >= MIN_HEIGHT);
        assert!(max <= MAX_HEIGHT);
        assert!(min <= max);
        assert!(avg >= min as f32 && avg <= max as f32);
    }

    #[test]
    fn test_negative_chunk_coordinates() {
        let seed = 888;

        // Should work with negative chunk coordinates
        let hm = Heightmap::generate(seed, -10, -20);

        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let height = hm.get(x, z);
                assert!(
                    height >= MIN_HEIGHT && height <= MAX_HEIGHT,
                    "Height out of range with negative coordinates"
                );
            }
        }
    }

    #[test]
    fn test_seam_continuity_negative_coords() {
        let seed = 123;

        // Test seams with negative coordinates
        assert!(
            check_seam_continuity(seed, (-1, 0), (0, 0)),
            "Seam detected crossing chunk boundary at X=0"
        );
        assert!(
            check_seam_continuity(seed, (0, -1), (0, 0)),
            "Seam detected crossing chunk boundary at Z=0"
        );
    }

    #[test]
    fn test_heights_raw_array() {
        let seed = 999;
        let hm = Heightmap::generate(seed, 5, 5);

        // Get raw heights array
        let heights = hm.heights();

        // Verify it matches get() method
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                assert_eq!(heights[z][x], hm.get(x, z));
            }
        }
    }

    #[test]
    #[should_panic(expected = "Chunks are not adjacent")]
    fn test_seam_continuity_non_adjacent_panics() {
        let seed = 42;
        // These chunks are not adjacent - should panic
        check_seam_continuity(seed, (0, 0), (5, 5));
    }

    #[test]
    fn test_min_max_height_specific() {
        let seed = 54321;
        let hm = Heightmap::generate(seed, 0, 0);

        let min = hm.min_height();
        let max = hm.max_height();

        // Verify min/max by scanning all values
        let mut found_min = MAX_HEIGHT;
        let mut found_max = MIN_HEIGHT;
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let h = hm.get(x, z);
                if h < found_min {
                    found_min = h;
                }
                if h > found_max {
                    found_max = h;
                }
            }
        }

        assert_eq!(min, found_min);
        assert_eq!(max, found_max);
    }

    #[test]
    fn test_avg_height_calculation() {
        let seed = 11111;
        let hm = Heightmap::generate(seed, 3, 3);

        let avg = hm.avg_height();

        // Calculate average manually
        let mut sum: i64 = 0;
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                sum += hm.get(x, z) as i64;
            }
        }
        let expected_avg = sum as f32 / (CHUNK_SIZE_X * CHUNK_SIZE_Z) as f32;

        assert!((avg - expected_avg).abs() < 0.001);
    }
}
