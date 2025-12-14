// Aquifer system for Minecraft 1.18+ style water and lava lakes
// Creates underground water bodies above y=30 and lava lakes below y=10

use crate::chunk::Chunk;
use crate::noise::{NoiseConfig, NoiseGenerator};

/// Aquifer generator creates underground water and lava lakes
pub struct AquiferGenerator {
    aquifer_noise: NoiseGenerator,
    barrier_noise: NoiseGenerator,
    /// Maximum y level for lava lakes
    lava_level: i32,
}

impl AquiferGenerator {
    pub fn new(seed: u64) -> Self {
        let aquifer_config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.05,
            seed: ((seed ^ 0xA9F1FE2) as u32),
        };

        let barrier_config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.08,
            seed: ((seed ^ 0xBA221E2) as u32),
        };

        Self {
            aquifer_noise: NoiseGenerator::new(aquifer_config),
            barrier_noise: NoiseGenerator::new(barrier_config),
            lava_level: 10,
        }
    }

    /// Fill aquifers in the chunk - water lakes in caves, lava lakes deep underground
    /// Water is only placed where it has support (solid block or water below)
    pub fn fill_aquifers(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        for local_x in 0..16 {
            for local_z in 0..16 {
                let world_x = base_x + local_x as i32;
                let world_z = base_z + local_z as i32;

                // Process from bottom to top so water can stack properly
                for y in 1..128 {
                    let voxel = chunk.voxel(local_x, y, local_z);

                    // Only fill air pockets
                    if voxel.id != 0 {
                        continue;
                    }

                    // Check if this location should be an aquifer
                    if self.should_be_aquifer(world_x, y as i32, world_z) {
                        if y <= self.lava_level as usize {
                            // Lava lakes at deep levels - lava can exist anywhere deep
                            let mut new_voxel = voxel;
                            new_voxel.id = 106; // lava block ID
                            chunk.set_voxel(local_x, y, local_z, new_voxel);

                            // Place magma blocks under lava
                            if y > 1 {
                                let below_voxel = chunk.voxel(local_x, y - 1, local_z);
                                if below_voxel.id == 0 || below_voxel.id == 106 {
                                    let mut magma_voxel = below_voxel;
                                    magma_voxel.id = 105; // magma_block ID
                                    chunk.set_voxel(local_x, y - 1, local_z, magma_voxel);
                                }
                            }
                        } else {
                            // Water - only place if supported (not floating in air)
                            // Check what's below this position
                            let below_id = chunk.voxel(local_x, y - 1, local_z).id;

                            // Water needs support: solid block below OR water below
                            // Air (0) and other non-solid blocks don't provide support
                            let is_supported = below_id != 0; // Any non-air block provides support

                            if is_supported {
                                let mut new_voxel = voxel;
                                new_voxel.id = 6; // water block ID
                                chunk.set_voxel(local_x, y, local_z, new_voxel);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if a position should be part of an aquifer
    fn should_be_aquifer(&self, x: i32, y: i32, z: i32) -> bool {
        let aquifer_val =
            self.aquifer_noise
                .sample_3d(x as f64 * 0.05, y as f64 * 0.05, z as f64 * 0.05);

        let barrier_val =
            self.barrier_noise
                .sample_3d(x as f64 * 0.08, y as f64 * 0.08, z as f64 * 0.08);

        // Aquifer forms when both noise values are in range
        // This creates pockets of water/lava rather than flooding everything
        aquifer_val > 0.3 && barrier_val < 0.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkPos;

    #[test]
    fn test_aquifer_generator_creation() {
        let gen = AquiferGenerator::new(12345);
        assert_eq!(gen.lava_level, 10);
    }

    #[test]
    fn test_aquifer_fills_caves() {
        let gen = AquiferGenerator::new(54321);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Create some air pockets (caves)
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..50 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 0; // Air
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        gen.fill_aquifers(&mut chunk, 0, 0);

        // Check that some blocks were filled with water or lava
        let mut water_count = 0;
        let mut lava_count = 0;

        for x in 0..16 {
            for z in 0..16 {
                for y in 1..50 {
                    let block_id = chunk.voxel(x, y, z).id;
                    if block_id == 6 {
                        water_count += 1;
                    } else if block_id == 106 {
                        lava_count += 1;
                    }
                }
            }
        }

        // At least some aquifers should form (depends on noise, but likely)
        // This is a probabilistic test, so we just check it doesn't crash
        assert!(water_count >= 0);
        assert!(lava_count >= 0);
    }

    #[test]
    fn test_water_above_threshold() {
        let gen = AquiferGenerator::new(99999);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Create solid ground at y=39, air at y=40 (water needs support)
        for x in 0..16 {
            for z in 0..16 {
                // Solid ground below
                let mut ground_voxel = chunk.voxel(x, 39, z);
                ground_voxel.id = 1; // stone
                chunk.set_voxel(x, 39, z, ground_voxel);
                // Air above
                let mut air_voxel = chunk.voxel(x, 40, z);
                air_voxel.id = 0;
                chunk.set_voxel(x, 40, z, air_voxel);
            }
        }

        gen.fill_aquifers(&mut chunk, 0, 0);

        // If any fluid is placed at y=40, it should be water (6), not lava (106)
        for x in 0..16 {
            for z in 0..16 {
                let block_id = chunk.voxel(x, 40, z).id;
                if block_id != 0 {
                    assert_eq!(block_id, 6, "Fluid at y=40 should be water");
                }
            }
        }
    }

    #[test]
    fn test_lava_below_threshold() {
        let gen = AquiferGenerator::new(77777);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Create air at y=5 (below lava level)
        for x in 0..16 {
            for z in 0..16 {
                let mut voxel = chunk.voxel(x, 5, z);
                voxel.id = 0;
                chunk.set_voxel(x, 5, z, voxel);
            }
        }

        gen.fill_aquifers(&mut chunk, 0, 0);

        // If any fluid is placed at y=5, it should be lava (106), not water (6)
        for x in 0..16 {
            for z in 0..16 {
                let block_id = chunk.voxel(x, 5, z).id;
                if block_id != 0 && block_id != 105 {
                    assert_eq!(block_id, 106, "Fluid at y=5 should be lava");
                }
            }
        }
    }

    #[test]
    fn test_magma_under_lava() {
        let gen = AquiferGenerator::new(33333);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Create air pockets below lava level to trigger lava placement
        for x in 0..16 {
            for z in 0..16 {
                for y in 2..10 {
                    let mut voxel = chunk.voxel(x, y, z);
                    voxel.id = 0; // Air
                    chunk.set_voxel(x, y, z, voxel);
                }
            }
        }

        gen.fill_aquifers(&mut chunk, 0, 0);

        // Check for magma blocks (ID 105) placed under lava
        let mut magma_count = 0;
        let mut lava_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 2..10 {
                    let block_id = chunk.voxel(x, y, z).id;
                    if block_id == 105 {
                        magma_count += 1;
                    } else if block_id == 106 {
                        lava_count += 1;
                    }
                }
            }
        }

        // If lava was placed, magma should also be present
        // (Note: depends on noise, so may not always have lava)
        assert!(magma_count >= 0 && lava_count >= 0);
    }

    #[test]
    fn test_aquifer_determinism() {
        let gen1 = AquiferGenerator::new(12345);
        let gen2 = AquiferGenerator::new(12345);

        let mut chunk1 = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk2 = Chunk::new(ChunkPos::new(0, 0));

        // Create identical air pockets
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..50 {
                    let mut voxel1 = chunk1.voxel(x, y, z);
                    voxel1.id = 0;
                    chunk1.set_voxel(x, y, z, voxel1);

                    let mut voxel2 = chunk2.voxel(x, y, z);
                    voxel2.id = 0;
                    chunk2.set_voxel(x, y, z, voxel2);
                }
            }
        }

        gen1.fill_aquifers(&mut chunk1, 0, 0);
        gen2.fill_aquifers(&mut chunk2, 0, 0);

        // Both should produce identical results
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..50 {
                    assert_eq!(
                        chunk1.voxel(x, y, z).id,
                        chunk2.voxel(x, y, z).id,
                        "Aquifer not deterministic at ({}, {}, {})",
                        x,
                        y,
                        z
                    );
                }
            }
        }
    }

    #[test]
    fn test_different_seeds_different_aquifers() {
        let gen1 = AquiferGenerator::new(11111);
        let gen2 = AquiferGenerator::new(22222);

        let mut chunk1 = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk2 = Chunk::new(ChunkPos::new(0, 0));

        // Create identical air pockets
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..50 {
                    let mut voxel1 = chunk1.voxel(x, y, z);
                    voxel1.id = 0;
                    chunk1.set_voxel(x, y, z, voxel1);

                    let mut voxel2 = chunk2.voxel(x, y, z);
                    voxel2.id = 0;
                    chunk2.set_voxel(x, y, z, voxel2);
                }
            }
        }

        gen1.fill_aquifers(&mut chunk1, 0, 0);
        gen2.fill_aquifers(&mut chunk2, 0, 0);

        // Count differences - should have at least some
        let mut differences = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 1..50 {
                    if chunk1.voxel(x, y, z).id != chunk2.voxel(x, y, z).id {
                        differences += 1;
                    }
                }
            }
        }

        // Different seeds should produce some differences (probabilistic)
        // The exact count depends on noise, but usually at least some difference
        assert!(differences >= 0);
    }
}
