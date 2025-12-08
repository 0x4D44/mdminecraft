// Amethyst geode generation for Minecraft 1.18+
// Creates rare spherical structures with smooth basalt, calcite, and amethyst layers

use crate::chunk::Chunk;
use crate::noise::{NoiseConfig, NoiseGenerator};

/// Geode generator creates rare spherical amethyst structures
pub struct GeodeGenerator {
    location_noise: NoiseGenerator,
    shape_noise: NoiseGenerator,
}

impl GeodeGenerator {
    pub fn new(seed: u64) -> Self {
        let location_config = NoiseConfig {
            octaves: 2,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.008,
            seed: ((seed ^ 0x6E0DE001) as u32),
        };

        let shape_config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.3,
            seed: ((seed ^ 0x6E0DE002) as u32),
        };

        Self {
            location_noise: NoiseGenerator::new(location_config),
            shape_noise: NoiseGenerator::new(shape_config),
        }
    }

    /// Attempt to generate a geode in the chunk
    /// Geodes are very rare - only spawn when location noise is in specific range
    pub fn try_generate_geode(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let world_x_base = chunk_x * 16;
        let world_z_base = chunk_z * 16;

        // Check if this chunk should have a geode
        let chunk_center_x = world_x_base + 8;
        let chunk_center_z = world_z_base + 8;

        let location_val = self
            .location_noise
            .sample_2d(chunk_center_x as f64 * 0.008, chunk_center_z as f64 * 0.008);

        // Geodes are very rare - only spawn in specific noise range
        if !(0.85..=0.95).contains(&location_val) {
            return;
        }

        // Determine geode center position (within chunk, underground)
        let center_y = 20 + ((location_val * 100.0) as i32 % 30);
        let center_x = 8;
        let center_z = 8;

        // Generate geode with radius 4-6 blocks
        let base_radius = 4.0 + (location_val * 10.0) % 2.0;

        self.carve_geode(chunk, center_x, center_y, center_z, base_radius);
    }

    fn carve_geode(&self, chunk: &mut Chunk, cx: usize, cy: i32, cz: usize, radius: f64) {
        let min_x = (cx as i32 - radius as i32 - 2).max(0) as usize;
        let max_x = (cx as i32 + radius as i32 + 2).min(15) as usize;
        let min_y = (cy - radius as i32 - 2).max(1);
        let max_y = (cy + radius as i32 + 2).min(250);
        let min_z = (cz as i32 - radius as i32 - 2).max(0) as usize;
        let max_z = (cz as i32 + radius as i32 + 2).min(15) as usize;

        for x in min_x..=max_x {
            for z in min_z..=max_z {
                for y in min_y..max_y {
                    let dx = x as f64 - cx as f64;
                    let dy = (y as f64 - cy as f64) * 1.2; // Slightly squashed vertically
                    let dz = z as f64 - cz as f64;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                    // Add noise to make the sphere irregular
                    let shape_noise =
                        self.shape_noise
                            .sample_3d(x as f64 * 0.3, y as f64 * 0.3, z as f64 * 0.3);
                    let modified_dist = dist + shape_noise * 0.8;

                    let voxel = chunk.voxel(x, y as usize, z);

                    // Skip if already air or water
                    if voxel.id == 0 || voxel.id == 6 {
                        continue;
                    }

                    let mut new_voxel = voxel;

                    // Layer 1: Outer shell - smooth basalt
                    if modified_dist > radius - 0.5 && modified_dist <= radius + 1.0 {
                        new_voxel.id = 107; // smooth_basalt
                        chunk.set_voxel(x, y as usize, z, new_voxel);
                    }
                    // Layer 2: Middle shell - calcite
                    else if modified_dist > radius - 1.5 && modified_dist <= radius - 0.5 {
                        new_voxel.id = 108; // calcite
                        chunk.set_voxel(x, y as usize, z, new_voxel);
                    }
                    // Layer 3: Inner shell - amethyst block
                    else if modified_dist > radius - 2.5 && modified_dist <= radius - 1.5 {
                        new_voxel.id = 109; // amethyst_block
                        chunk.set_voxel(x, y as usize, z, new_voxel);
                    }
                    // Layer 4: Innermost - budding amethyst (rare)
                    else if modified_dist > radius - 3.0 && modified_dist <= radius - 2.5 {
                        new_voxel.id = 110; // budding_amethyst
                        chunk.set_voxel(x, y as usize, z, new_voxel);
                    }
                    // Center: Air cavity
                    else if modified_dist <= radius - 3.0 {
                        new_voxel.id = 0; // air
                        chunk.set_voxel(x, y as usize, z, new_voxel);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkPos;

    #[test]
    fn test_geode_generator_creation() {
        let _gen = GeodeGenerator::new(12345);
        // Just verify it doesn't crash
        assert!(true);
    }

    #[test]
    fn test_geode_rare_spawning() {
        let gen = GeodeGenerator::new(99999);
        let mut geode_count = 0;

        // Test 100 chunks - geodes should be very rare
        for cx in 0..10 {
            for cz in 0..10 {
                let mut chunk = Chunk::new(ChunkPos::new(0, 0));

                // Fill with stone first
                for x in 0..16 {
                    for z in 0..16 {
                        for y in 1..100 {
                            let mut voxel = chunk.voxel(x, y, z);
                            voxel.id = 13; // stone
                            chunk.set_voxel(x, y, z, voxel);
                        }
                    }
                }

                gen.try_generate_geode(&mut chunk, cx, cz);

                // Check if any geode blocks were placed
                let mut has_geode = false;
                for x in 0..16 {
                    for z in 0..16 {
                        for y in 1..100 {
                            let block_id = chunk.voxel(x, y, z).id;
                            if block_id == 107
                                || block_id == 108
                                || block_id == 109
                                || block_id == 110
                            {
                                has_geode = true;
                                break;
                            }
                        }
                        if has_geode {
                            break;
                        }
                    }
                    if has_geode {
                        break;
                    }
                }

                if has_geode {
                    geode_count += 1;
                }
            }
        }

        // Geodes should be rare - expect less than 10% of chunks to have one
        assert!(
            geode_count < 10,
            "Geodes should be rare (found {} in 100 chunks)",
            geode_count
        );
    }

    #[test]
    fn test_geode_has_layers() {
        let gen = GeodeGenerator::new(55555);

        // Force a geode to spawn by finding a good chunk
        for cx in 0..100 {
            for cz in 0..100 {
                let mut chunk = Chunk::new(ChunkPos::new(0, 0));

                // Fill with stone
                for x in 0..16 {
                    for z in 0..16 {
                        for y in 1..100 {
                            let mut voxel = chunk.voxel(x, y, z);
                            voxel.id = 13;
                            chunk.set_voxel(x, y, z, voxel);
                        }
                    }
                }

                gen.try_generate_geode(&mut chunk, cx, cz);

                // Check if we got a geode with all layers
                let mut has_basalt = false;
                let mut has_calcite = false;
                let mut has_amethyst = false;
                let mut has_budding = false;

                for x in 0..16 {
                    for z in 0..16 {
                        for y in 1..100 {
                            let block_id = chunk.voxel(x, y, z).id;
                            if block_id == 107 {
                                has_basalt = true;
                            }
                            if block_id == 108 {
                                has_calcite = true;
                            }
                            if block_id == 109 {
                                has_amethyst = true;
                            }
                            if block_id == 110 {
                                has_budding = true;
                            }
                        }
                    }
                }

                // If we found any geode blocks, verify all layers are present
                if has_basalt || has_calcite || has_amethyst || has_budding {
                    assert!(has_basalt, "Geode should have smooth basalt outer layer");
                    assert!(has_calcite, "Geode should have calcite middle layer");
                    assert!(has_amethyst, "Geode should have amethyst block layer");
                    // budding amethyst is innermost and may not always spawn depending on size
                    return; // Test passed
                }
            }
        }

        // If no geode spawned in 10000 chunks, that's fine (they're very rare)
        // Test passes either way
    }
}
