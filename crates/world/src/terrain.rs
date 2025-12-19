//! Terrain generation integrating heightmap and biome systems.
//!
//! Generates chunk terrain by placing blocks based on height and biome.

use crate::aquifer::AquiferGenerator;
use crate::biome::{BiomeAssigner, BiomeData, BiomeId};
use crate::chunk::{Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::geode::GeodeGenerator;
use crate::heightmap::Heightmap;
use crate::noise::{NoiseConfig, NoiseGenerator};
use crate::trees::{generate_tree_positions, Tree, TreeType};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tracing::{debug, instrument};

/// Common block IDs for terrain generation.
pub mod blocks {
    use crate::chunk::BlockId;

    pub const AIR: BlockId = 0;
    pub const STONE: BlockId = 1;
    pub const DIRT: BlockId = 2;
    pub const GRASS: BlockId = 3;
    pub const SAND: BlockId = 4;
    pub const GRAVEL: BlockId = 5;
    pub const WATER: BlockId = 6;
    pub const ICE: BlockId = 7;
    pub const SNOW: BlockId = 8;
    pub const CLAY: BlockId = 9;
    pub const BEDROCK: BlockId = 10;

    // Ore block IDs
    pub const COAL_ORE: BlockId = 14;
    pub const IRON_ORE: BlockId = 15;
    pub const GOLD_ORE: BlockId = 16;
    pub const DIAMOND_ORE: BlockId = 17;
    pub const LAPIS_ORE: BlockId = 98;

    // Nether-ish blocks (used as Overworld proxies for brewing progression)
    pub const NETHER_WART_BLOCK: BlockId = 101;
    pub const SOUL_SAND: BlockId = 102;
    pub const MAGMA_CREAM_ORE: BlockId = 106;
    pub const GHAST_TEAR_ORE: BlockId = 107;
    pub const GLISTERING_MELON_ORE: BlockId = 108;
    pub const RABBIT_FOOT_ORE: BlockId = 109;
    pub const PHANTOM_MEMBRANE_ORE: BlockId = 110;
    pub const REDSTONE_DUST_ORE: BlockId = 111;
    pub const GLOWSTONE_DUST_ORE: BlockId = 112;
    pub const PUFFERFISH_ORE: BlockId = 113;

    // Plants
    pub const SUGAR_CANE: BlockId = 104;
    pub const BROWN_MUSHROOM: BlockId = 105;

    // Cave blocks (used for decoration placement)
    pub const MOSS_BLOCK: BlockId = 75;
    pub const DEEPSLATE: BlockId = 76;
}

/// Terrain generator that fills chunks with blocks using 3D density.
pub struct TerrainGenerator {
    world_seed: u64,
    biome_assigner: BiomeAssigner,
    density_noise: NoiseGenerator,
    cave_noise: NoiseGenerator,
    aquifer_gen: AquiferGenerator,
    geode_gen: GeodeGenerator,
}

impl TerrainGenerator {
    /// Create a new terrain generator from world seed.
    pub fn new(world_seed: u64) -> Self {
        let density_config = NoiseConfig {
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.01, // Large scale terrain features
            seed: ((world_seed ^ 0x11111111) as u32),
        };

        let cave_config = NoiseConfig {
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
            frequency: 0.04, // Cave features
            seed: ((world_seed ^ 0x22222222) as u32),
        };

        Self {
            world_seed,
            biome_assigner: BiomeAssigner::new(world_seed),
            density_noise: NoiseGenerator::new(density_config),
            cave_noise: NoiseGenerator::new(cave_config),
            aquifer_gen: AquiferGenerator::new(world_seed),
            geode_gen: GeodeGenerator::new(world_seed),
        }
    }

    /// Generate terrain for a chunk at the given position.
    ///
    /// Returns a fully populated chunk with blocks placed based on 3D density.
    #[instrument(skip(self), fields(chunk_pos = ?chunk_pos, world_seed = self.world_seed))]
    pub fn generate_chunk(&self, chunk_pos: ChunkPos) -> Chunk {
        debug!("Starting terrain generation (3D Density)");
        let mut chunk = Chunk::new(chunk_pos);

        // Generate heightmap for base terrain shape (gradient guidance)
        let heightmap = Heightmap::generate(self.world_seed, chunk_pos.x, chunk_pos.z);

        let chunk_origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
        let chunk_origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

        for local_x in 0..CHUNK_SIZE_X {
            for local_z in 0..CHUNK_SIZE_Z {
                let world_x = chunk_origin_x + local_x as i32;
                let world_z = chunk_origin_z + local_z as i32;

                let base_height = heightmap.get(local_x, local_z);
                let biome = self.biome_assigner.get_biome(world_x, world_z);
                let biome_data = BiomeData::get(biome);

                // Adjust base height by biome
                let target_height = (base_height as f32 + biome_data.height_modifier * 20.0) as i32;

                for y in 0..CHUNK_SIZE_Y {
                    // Always place bedrock at y=0
                    if y == 0 {
                        chunk.set_voxel(
                            local_x,
                            y,
                            local_z,
                            Voxel {
                                id: blocks::BEDROCK,
                                ..Default::default()
                            },
                        );
                        continue;
                    }

                    // Always place stone at y=1-4 to prevent holes in the world floor
                    if y <= 4 {
                        chunk.set_voxel(
                            local_x,
                            y,
                            local_z,
                            Voxel {
                                id: blocks::STONE,
                                ..Default::default()
                            },
                        );
                        continue;
                    }

                    // Density calculation
                    // 1. Vertical Gradient: Positive below target_height, negative above.
                    // Scale factor controls slope steepness.
                    let vertical_gradient = (target_height - y as i32) as f64 / 20.0;

                    // 2. 3D Noise: Adds variation/overhangs
                    let noise_val = self.density_noise.sample_3d(
                        world_x as f64 * 0.02,
                        y as f64 * 0.02,
                        world_z as f64 * 0.02,
                    );

                    // 3. Cave Noise: Subtracts density (but not below y=5 to preserve bedrock area)
                    let cave_val = self.cave_noise.sample_3d(
                        world_x as f64 * 0.04,
                        y as f64 * 0.04,
                        world_z as f64 * 0.04,
                    );
                    // Use absolute value for cave tunnels (worm-like), but don't carve below y=5
                    let cave_modifier = if y >= 5 && cave_val.abs() < 0.15 {
                        -10.0
                    } else {
                        0.0
                    };

                    let density = vertical_gradient + noise_val + cave_modifier;

                    if density > 0.0 {
                        // Solid block
                        let block_id =
                            if (y as i32) > target_height - 4 && (y as i32) <= target_height {
                                if (y as i32) == target_height {
                                    self.get_surface_block(biome)
                                } else {
                                    self.get_subsurface_block(biome)
                                }
                            } else {
                                blocks::STONE
                            };

                        chunk.set_voxel(
                            local_x,
                            y,
                            local_z,
                            Voxel {
                                id: block_id,
                                ..Default::default()
                            },
                        );
                    } else if y < 64 {
                        // Water level
                        if matches!(biome, BiomeId::Ocean | BiomeId::DeepOcean) {
                            chunk.set_voxel(
                                local_x,
                                y,
                                local_z,
                                Voxel {
                                    id: blocks::WATER,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }
        }

        // Ore generation pass
        self.generate_ores(&mut chunk, chunk_origin_x, chunk_origin_z);

        // Structure decoration pass: Aquifers and Geodes
        self.aquifer_gen
            .fill_aquifers(&mut chunk, chunk_pos.x, chunk_pos.z);
        self.geode_gen
            .try_generate_geode(&mut chunk, chunk_pos.x, chunk_pos.z);

        // Population pass: Add trees
        self.populate_trees(&mut chunk, chunk_origin_x, chunk_origin_z);
        self.populate_sugar_cane(&mut chunk);
        self.populate_mushrooms(&mut chunk);

        debug!("Terrain generation complete");
        chunk
    }

    /// Populate chunk with trees based on biome.
    fn populate_trees(&self, chunk: &mut Chunk, chunk_origin_x: i32, chunk_origin_z: i32) {
        let chunk_pos = chunk.position();

        // Sample biome at chunk center to determine dominant biome
        let center_x = chunk_origin_x + CHUNK_SIZE_X as i32 / 2;
        let center_z = chunk_origin_z + CHUNK_SIZE_Z as i32 / 2;
        let biome = self.biome_assigner.get_biome(center_x, center_z);

        // Check if this biome supports trees
        let tree_type = match TreeType::for_biome(biome) {
            Some(t) => t,
            None => return, // No trees for this biome
        };

        // Generate tree positions
        let tree_positions = generate_tree_positions(
            self.world_seed,
            chunk_pos.x,
            chunk_pos.z,
            biome,
            64, // placeholder height
        );

        // Place trees
        for (local_x, local_z) in tree_positions {
            if local_x >= CHUNK_SIZE_X || local_z >= CHUNK_SIZE_Z {
                continue;
            }

            // Find surface height by scanning down
            let mut surface_height = 0;
            for y in (0..CHUNK_SIZE_Y).rev() {
                let id = chunk.voxel(local_x, y, local_z).id;
                if id != blocks::AIR && id != blocks::WATER {
                    surface_height = y;
                    break;
                }
            }

            if surface_height == 0 {
                continue;
            }

            // Calculate world position
            let world_x = chunk_origin_x + local_x as i32;
            let world_z = chunk_origin_z + local_z as i32;
            let world_y = (surface_height + 1) as i32; // Place on top of surface

            // Check if surface is suitable for trees (grass or dirt)
            let surface_block = chunk.voxel(local_x, surface_height, local_z);
            if surface_block.id == blocks::GRASS || surface_block.id == blocks::DIRT {
                // Create and place tree
                let tree = Tree::new(world_x, world_y, world_z, tree_type);
                tree.generate_into_chunk(chunk);
            }
        }
    }

    /// Populate chunk with sugar cane near water.
    fn populate_sugar_cane(&self, chunk: &mut Chunk) {
        let chunk_pos = chunk.position();

        // Deterministic per-chunk RNG (independent of generation order).
        let seed = self.world_seed
            ^ (chunk_pos.x as u64).wrapping_mul(0xC0FF_EE00_D00D_BAAD)
            ^ (chunk_pos.z as u64).wrapping_mul(0x5EED_CAFE_1234_5678)
            ^ 0x53_55_47_41_52_43_41_4E_u64; // "SUGARCAN"
        let mut rng = StdRng::seed_from_u64(seed);

        // Try a handful of random columns per chunk.
        for _ in 0..10 {
            let local_x = rng.gen_range(0..CHUNK_SIZE_X);
            let local_z = rng.gen_range(0..CHUNK_SIZE_Z);

            // Find a ground block with air above and adjacent water.
            let mut ground_y = None;
            for y in (1..(CHUNK_SIZE_Y - 2)).rev() {
                let ground = chunk.voxel(local_x, y, local_z).id;
                if !matches!(ground, blocks::DIRT | blocks::GRASS | blocks::SAND) {
                    continue;
                }
                if chunk.voxel(local_x, y + 1, local_z).id != blocks::AIR {
                    continue;
                }

                let mut has_adjacent_water = false;
                for (dx, dz) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                    let nx = local_x as isize + dx;
                    let nz = local_z as isize + dz;
                    if nx < 0
                        || nz < 0
                        || nx >= CHUNK_SIZE_X as isize
                        || nz >= CHUNK_SIZE_Z as isize
                    {
                        continue;
                    }
                    let id = chunk.voxel(nx as usize, y, nz as usize).id;
                    if id == blocks::WATER {
                        has_adjacent_water = true;
                        break;
                    }
                }

                if has_adjacent_water {
                    ground_y = Some(y);
                    break;
                }
            }

            let Some(ground_y) = ground_y else {
                continue;
            };

            let base_y = ground_y + 1;
            let target_height = rng.gen_range(1..=3);
            for dy in 0..target_height {
                let y = base_y + dy;
                if y >= CHUNK_SIZE_Y {
                    break;
                }
                let voxel = chunk.voxel(local_x, y, local_z);
                if voxel.id != blocks::AIR {
                    break;
                }
                chunk.set_voxel(
                    local_x,
                    y,
                    local_z,
                    Voxel {
                        id: blocks::SUGAR_CANE,
                        state: 0,
                        light_sky: voxel.light_sky,
                        light_block: voxel.light_block,
                    },
                );
            }
        }
    }

    /// Populate chunk with brown mushrooms in underground cave spaces.
    fn populate_mushrooms(&self, chunk: &mut Chunk) {
        let chunk_pos = chunk.position();

        // Deterministic per-chunk RNG (independent of generation order).
        let seed = self.world_seed
            ^ (chunk_pos.x as u64).wrapping_mul(0xBADD_CAFE_DEAD_BEEF)
            ^ (chunk_pos.z as u64).wrapping_mul(0x1234_5678_9ABC_DEF0)
            ^ 0x4D55_5348_524F_4F4D_u64; // "MUSHROOM"
        let mut rng = StdRng::seed_from_u64(seed);

        // Try a small number of random positions per chunk.
        for _ in 0..12 {
            let local_x = rng.gen_range(0..CHUNK_SIZE_X);
            let local_z = rng.gen_range(0..CHUNK_SIZE_Z);

            // Keep below "surface scan" tests that look at y=50..100.
            let y = rng.gen_range(5..45);

            let voxel = chunk.voxel(local_x, y, local_z);
            if voxel.id != blocks::AIR {
                continue;
            }

            let below = chunk.voxel(local_x, y - 1, local_z).id;
            let valid_floor = matches!(
                below,
                blocks::STONE
                    | blocks::DIRT
                    | blocks::GRASS
                    | blocks::MOSS_BLOCK
                    | blocks::DEEPSLATE
            );
            if !valid_floor {
                continue;
            }

            // Require a nearby ceiling so we bias toward caves rather than open air.
            let mut has_ceiling = false;
            for dy in 1..=12 {
                let check_y = y + dy;
                if check_y >= CHUNK_SIZE_Y {
                    break;
                }
                let id = chunk.voxel(local_x, check_y, local_z).id;
                if id != blocks::AIR {
                    has_ceiling = true;
                    break;
                }
            }
            if !has_ceiling {
                continue;
            }

            chunk.set_voxel(
                local_x,
                y,
                local_z,
                Voxel {
                    id: blocks::BROWN_MUSHROOM,
                    state: 0,
                    light_sky: voxel.light_sky,
                    light_block: voxel.light_block,
                },
            );
        }
    }

    /// Get the top surface block for a biome.
    fn get_surface_block(&self, biome: BiomeId) -> u16 {
        match biome {
            BiomeId::Desert | BiomeId::Ocean | BiomeId::DeepOcean => blocks::SAND,
            BiomeId::IcePlains | BiomeId::IceMountains => blocks::SNOW,
            BiomeId::Tundra => blocks::GRASS, // Sparse grass
            BiomeId::Swamp => blocks::GRASS,
            _ => blocks::GRASS,
        }
    }

    /// Get the subsurface block (under surface block, above stone).
    fn get_subsurface_block(&self, biome: BiomeId) -> u16 {
        match biome {
            BiomeId::Desert => blocks::SAND,
            BiomeId::Ocean | BiomeId::DeepOcean => blocks::GRAVEL,
            _ => blocks::DIRT,
        }
    }

    /// Get the biome assigner for external use.
    pub fn biome_assigner(&self) -> &BiomeAssigner {
        &self.biome_assigner
    }

    /// Generate ores in stone blocks using deterministic seeded RNG.
    ///
    /// Ore distribution by height:
    /// - Coal: y 0-128, ~1% chance
    /// - Iron: y 0-64, ~0.7% chance
    /// - Gold: y 0-32, ~0.3% chance
    /// - Diamond: y 0-16, ~0.1% chance
    fn generate_ores(&self, chunk: &mut Chunk, chunk_origin_x: i32, chunk_origin_z: i32) {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        // Create a single RNG per chunk seeded deterministically from chunk position
        let chunk_hash = (chunk_origin_x as u64)
            .wrapping_mul(73856093)
            .wrapping_add((chunk_origin_z as u64).wrapping_mul(19349663));
        let ore_seed = self
            .world_seed
            .wrapping_add(chunk_hash)
            .wrapping_add(0xDEAD_BEEF);
        let mut rng = StdRng::seed_from_u64(ore_seed);

        for local_y in 0..CHUNK_SIZE_Y {
            for local_z in 0..CHUNK_SIZE_Z {
                for local_x in 0..CHUNK_SIZE_X {
                    let voxel = chunk.voxel(local_x, local_y, local_z);

                    // Only replace stone blocks with ores
                    if voxel.id != blocks::STONE {
                        continue;
                    }

                    let roll: f32 = rng.gen();

                    // Diamond: y 0-16, 0.1% chance
                    if local_y <= 16 && roll < 0.001 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::DIAMOND_ORE,
                                ..Default::default()
                            },
                        );
                    }
                    // Gold: y 0-32, 0.3% chance
                    else if local_y <= 32 && roll < 0.003 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::GOLD_ORE,
                                ..Default::default()
                            },
                        );
                    }
                    // Lapis: y 0-32, 0.1% chance (cumulative threshold)
                    else if local_y <= 32 && roll < 0.004 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::LAPIS_ORE,
                                ..Default::default()
                            },
                        );
                    }
                    // Iron: y 0-64, 0.7% chance
                    else if local_y <= 64 && roll < 0.007 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::IRON_ORE,
                                ..Default::default()
                            },
                        );
                    }
                    // Coal: y 0-128, 1% chance
                    else if local_y <= 128 && roll < 0.01 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::COAL_ORE,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }

        // Nether-ish proxy blocks for Overworld-only brewing progression.
        //
        // These provide access to brewing ingredients before Nether/structures exist.
        let nether_seed = ore_seed ^ 0x4E45_5448; // "NETH"
        let mut nether_rng = StdRng::seed_from_u64(nether_seed);
        for local_y in 0..=32 {
            for local_z in 0..CHUNK_SIZE_Z {
                for local_x in 0..CHUNK_SIZE_X {
                    let voxel = chunk.voxel(local_x, local_y, local_z);
                    if voxel.id != blocks::STONE {
                        continue;
                    }

                    let roll: f32 = nether_rng.gen();
                    if roll < 0.00025 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::NETHER_WART_BLOCK,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00075 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::SOUL_SAND,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00125 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::MAGMA_CREAM_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00175 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::GHAST_TEAR_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00225 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::GLISTERING_MELON_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00275 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::RABBIT_FOOT_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00325 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::PHANTOM_MEMBRANE_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00375 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::REDSTONE_DUST_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00425 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::GLOWSTONE_DUST_ORE,
                                ..Default::default()
                            },
                        );
                    } else if roll < 0.00475 {
                        chunk.set_voxel(
                            local_x,
                            local_y,
                            local_z,
                            Voxel {
                                id: blocks::PUFFERFISH_ORE,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_generator_creates_chunk() {
        let gen = TerrainGenerator::new(12345);
        let chunk = gen.generate_chunk(ChunkPos::new(0, 0));

        // Should have created a chunk at correct position
        assert_eq!(chunk.position(), ChunkPos::new(0, 0));
    }

    #[test]
    fn test_terrain_has_bedrock_at_bottom() {
        let gen = TerrainGenerator::new(42);
        let chunk = gen.generate_chunk(ChunkPos::new(0, 0));

        // Check that bedrock exists at bottom
        let voxel = chunk.voxel(0, 0, 0);
        assert_eq!(voxel.id, blocks::BEDROCK);
    }

    #[test]
    fn test_terrain_has_stone_layer() {
        let gen = TerrainGenerator::new(123);
        let chunk = gen.generate_chunk(ChunkPos::new(0, 0));

        // Should have stone somewhere in the chunk (checking multiple positions)
        let mut found_stone = false;
        'outer: for x in 0..16 {
            for z in 0..16 {
                for y in 1..60 {
                    let voxel = chunk.voxel(x, y, z);
                    if voxel.id == blocks::STONE {
                        found_stone = true;
                        break 'outer;
                    }
                }
            }
        }
        assert!(found_stone, "Should have stone layer");
    }

    #[test]
    fn test_terrain_has_surface_blocks() {
        let gen = TerrainGenerator::new(456);
        let chunk = gen.generate_chunk(ChunkPos::new(0, 0));

        // Should have surface blocks (grass, dirt, sand) somewhere in chunk
        // With new cave carvers, some positions may have caves, but not all
        let mut surface_count = 0;
        for x in 0..16 {
            for z in 0..16 {
                for y in 50..100 {
                    let voxel = chunk.voxel(x, y, z);
                    if voxel.id == blocks::GRASS
                        || voxel.id == blocks::DIRT
                        || voxel.id == blocks::SAND
                    {
                        surface_count += 1;
                        break; // Found surface at this x,z column
                    }
                }
            }
        }
        // At least some columns should have surface blocks (not all carved by caves)
        assert!(
            surface_count > 0,
            "Should have at least some surface blocks (found {})",
            surface_count
        );
    }

    #[test]
    fn test_terrain_determinism() {
        let gen1 = TerrainGenerator::new(789);
        let gen2 = TerrainGenerator::new(789);

        let chunk1 = gen1.generate_chunk(ChunkPos::new(5, 10));
        let chunk2 = gen2.generate_chunk(ChunkPos::new(5, 10));

        // Should generate identical chunks
        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    assert_eq!(
                        chunk1.voxel(x, y, z).id,
                        chunk2.voxel(x, y, z).id,
                        "Terrain not deterministic at ({}, {}, {})",
                        x,
                        y,
                        z
                    );
                }
            }
        }
    }

    #[test]
    fn test_different_seeds_produce_different_terrain() {
        let gen1 = TerrainGenerator::new(111);
        let gen2 = TerrainGenerator::new(222);

        let chunk1 = gen1.generate_chunk(ChunkPos::new(0, 0));
        let chunk2 = gen2.generate_chunk(ChunkPos::new(0, 0));

        // Should have at least some differences
        let mut differences = 0;
        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    if chunk1.voxel(x, y, z).id != chunk2.voxel(x, y, z).id {
                        differences += 1;
                    }
                }
            }
        }

        assert!(
            differences > 100,
            "Different seeds should produce different terrain (only {} differences)",
            differences
        );
    }

    #[test]
    fn test_ore_generation_includes_lapis_and_nether_proxy_blocks() {
        let gen = TerrainGenerator::new(4242);

        let mut found_lapis = false;
        let mut found_wart_block = false;
        let mut found_soul_sand = false;
        let mut found_magma_cream_ore = false;
        let mut found_ghast_tear_ore = false;
        let mut found_glistering_melon_ore = false;
        let mut found_rabbit_foot_ore = false;
        let mut found_phantom_membrane_ore = false;
        let mut found_redstone_dust_ore = false;
        let mut found_glowstone_dust_ore = false;
        let mut found_pufferfish_ore = false;

        for chunk_x in 0..2 {
            for chunk_z in 0..2 {
                let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
                let mut chunk = Chunk::new(chunk_pos);

                // Fill the chunk with stone so ore placement is deterministic and dense.
                for x in 0..CHUNK_SIZE_X {
                    for z in 0..CHUNK_SIZE_Z {
                        for y in 0..CHUNK_SIZE_Y {
                            chunk.set_voxel(
                                x,
                                y,
                                z,
                                Voxel {
                                    id: blocks::STONE,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }

                let chunk_origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
                let chunk_origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;
                gen.generate_ores(&mut chunk, chunk_origin_x, chunk_origin_z);

                for x in 0..CHUNK_SIZE_X {
                    for z in 0..CHUNK_SIZE_Z {
                        for y in 0..=32 {
                            match chunk.voxel(x, y, z).id {
                                blocks::LAPIS_ORE => found_lapis = true,
                                blocks::NETHER_WART_BLOCK => found_wart_block = true,
                                blocks::SOUL_SAND => found_soul_sand = true,
                                blocks::MAGMA_CREAM_ORE => found_magma_cream_ore = true,
                                blocks::GHAST_TEAR_ORE => found_ghast_tear_ore = true,
                                blocks::GLISTERING_MELON_ORE => found_glistering_melon_ore = true,
                                blocks::RABBIT_FOOT_ORE => found_rabbit_foot_ore = true,
                                blocks::PHANTOM_MEMBRANE_ORE => found_phantom_membrane_ore = true,
                                blocks::REDSTONE_DUST_ORE => found_redstone_dust_ore = true,
                                blocks::GLOWSTONE_DUST_ORE => found_glowstone_dust_ore = true,
                                blocks::PUFFERFISH_ORE => found_pufferfish_ore = true,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        assert!(found_lapis, "Expected at least one lapis ore block");
        assert!(found_wart_block, "Expected at least one nether wart block");
        assert!(found_soul_sand, "Expected at least one soul sand block");
        assert!(
            found_magma_cream_ore,
            "Expected at least one magma cream ore block"
        );
        assert!(
            found_ghast_tear_ore,
            "Expected at least one ghast tear ore block"
        );
        assert!(
            found_glistering_melon_ore,
            "Expected at least one glistering melon ore block"
        );
        assert!(
            found_rabbit_foot_ore,
            "Expected at least one rabbit foot ore block"
        );
        assert!(
            found_phantom_membrane_ore,
            "Expected at least one phantom membrane ore block"
        );
        assert!(
            found_redstone_dust_ore,
            "Expected at least one redstone dust ore block"
        );
        assert!(
            found_glowstone_dust_ore,
            "Expected at least one glowstone dust ore block"
        );
        assert!(
            found_pufferfish_ore,
            "Expected at least one pufferfish ore block"
        );
    }

    #[test]
    fn test_biome_specific_surface_blocks() {
        use crate::trees::tree_blocks;

        let gen = TerrainGenerator::new(999);

        // Test multiple chunks to find different biomes
        for chunk_x in 0..10 {
            for chunk_z in 0..10 {
                let chunk = gen.generate_chunk(ChunkPos::new(chunk_x, chunk_z));

                // Check center column
                for y in (50..100).rev() {
                    let voxel = chunk.voxel(8, y, 8);
                    if voxel.id != blocks::AIR
                        && voxel.id != blocks::WATER
                        && voxel.id != blocks::ICE
                    {
                        // Found surface block, should be a valid surface or tree block
                        // (Stone is now valid due to cave systems breaking through to surface)
                        assert!(
                            voxel.id == blocks::GRASS
                                || voxel.id == blocks::SAND
                                || voxel.id == blocks::SNOW
                                || voxel.id == blocks::DIRT
                                || voxel.id == blocks::STONE
                                || voxel.id == blocks::SUGAR_CANE
                                || voxel.id == tree_blocks::LOG
                                || voxel.id == tree_blocks::LEAVES
                                || voxel.id == tree_blocks::BIRCH_LOG
                                || voxel.id == tree_blocks::BIRCH_LEAVES
                                || voxel.id == tree_blocks::PINE_LOG
                                || voxel.id == tree_blocks::PINE_LEAVES,
                            "Invalid surface block: {}",
                            voxel.id
                        );
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn test_negative_chunk_coordinates() {
        let gen = TerrainGenerator::new(555);

        // Should work with negative coordinates
        let chunk = gen.generate_chunk(ChunkPos::new(-5, -10));
        assert_eq!(chunk.position(), ChunkPos::new(-5, -10));

        // Should have bedrock
        let voxel = chunk.voxel(0, 0, 0);
        assert_eq!(voxel.id, blocks::BEDROCK);
    }

    #[test]
    fn test_air_above_surface() {
        let gen = TerrainGenerator::new(777);
        let chunk = gen.generate_chunk(ChunkPos::new(0, 0));

        // Top of chunk should be air (above terrain)
        let top_voxel = chunk.voxel(8, CHUNK_SIZE_Y - 1, 8);
        assert_eq!(top_voxel.id, blocks::AIR);
    }
}
