//! Terrain generation integrating heightmap and biome systems.
//!
//! Generates chunk terrain by placing blocks based on height and biome.

use crate::biome::{BiomeAssigner, BiomeData, BiomeId};
use crate::caves::CaveGenerator;
use crate::chunk::{Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::heightmap::Heightmap;
use crate::trees::{generate_tree_positions, Tree, TreeType};
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
}

/// Terrain generator that fills chunks with blocks.
pub struct TerrainGenerator {
    world_seed: u64,
    biome_assigner: BiomeAssigner,
    cave_generator: CaveGenerator,
}

impl TerrainGenerator {
    /// Create a new terrain generator from world seed.
    pub fn new(world_seed: u64) -> Self {
        Self {
            world_seed,
            biome_assigner: BiomeAssigner::new(world_seed),
            cave_generator: CaveGenerator::new(world_seed),
        }
    }

    /// Generate terrain for a chunk at the given position.
    ///
    /// Returns a fully populated chunk with blocks placed based on heightmap and biome.
    #[instrument(skip(self), fields(chunk_pos = ?chunk_pos, world_seed = self.world_seed))]
    pub fn generate_chunk(&self, chunk_pos: ChunkPos) -> Chunk {
        debug!("Starting terrain generation");
        let mut chunk = Chunk::new(chunk_pos);

        // Generate heightmap for this chunk
        let heightmap = Heightmap::generate(self.world_seed, chunk_pos.x, chunk_pos.z);

        // Calculate world-space origin of this chunk
        let chunk_origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
        let chunk_origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

        // Fill each column based on heightmap and biome
        for local_z in 0..CHUNK_SIZE_Z {
            for local_x in 0..CHUNK_SIZE_X {
                let world_x = chunk_origin_x + local_x as i32;
                let world_z = chunk_origin_z + local_z as i32;

                // Get height and biome for this column
                let height = heightmap.get(local_x, local_z);
                let biome = self.biome_assigner.get_biome(world_x, world_z);
                let biome_data = BiomeData::get(biome);

                // Apply biome height modifier
                let modified_height = (height as f32 + biome_data.height_modifier * 20.0) as i32;
                let final_height = modified_height.clamp(0, CHUNK_SIZE_Y as i32 - 1);

                // Generate column
                self.generate_column(
                    &mut chunk,
                    local_x,
                    local_z,
                    final_height as usize,
                    biome,
                    &biome_data,
                );
            }
        }

        // Ore generation pass: Replace some stone with ores
        self.generate_ores(&mut chunk, chunk_origin_x, chunk_origin_z);

        // Cave pass: Carve caves through the terrain
        self.carve_caves(&mut chunk, chunk_origin_x, chunk_origin_z);

        // Population pass: Add trees
        self.populate_trees(&mut chunk, &heightmap, chunk_origin_x, chunk_origin_z);

        debug!("Terrain generation complete");
        chunk
    }

    /// Generate a single vertical column of blocks.
    fn generate_column(
        &self,
        chunk: &mut Chunk,
        x: usize,
        z: usize,
        height: usize,
        biome: BiomeId,
        biome_data: &BiomeData,
    ) {
        // Bedrock layer (bottom 1-5 blocks)
        let bedrock_height = 1 + ((x + z) % 5);
        for y in 0..bedrock_height {
            chunk.set_voxel(
                x,
                y,
                z,
                Voxel {
                    id: blocks::BEDROCK,
                    ..Default::default()
                },
            );
        }

        // Stone layer (bedrock to height - surface depth)
        let surface_depth = self.get_surface_depth(biome);
        let stone_top = if height > surface_depth {
            height - surface_depth
        } else {
            bedrock_height
        };

        for y in bedrock_height..stone_top {
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

        // Surface layers
        let surface_block = self.get_surface_block(biome);
        let subsurface_block = self.get_subsurface_block(biome);

        for y in stone_top..height {
            let depth_from_surface = height - y - 1;
            let block_id = if depth_from_surface == 0 {
                surface_block
            } else {
                subsurface_block
            };

            chunk.set_voxel(
                x,
                y,
                z,
                Voxel {
                    id: block_id,
                    ..Default::default()
                },
            );
        }

        // Top surface block
        chunk.set_voxel(
            x,
            height,
            z,
            Voxel {
                id: surface_block,
                ..Default::default()
            },
        );

        // Water/ice filling for ocean biomes
        if matches!(biome, BiomeId::Ocean | BiomeId::DeepOcean) {
            let sea_level = 64;
            if height < sea_level {
                let water_block = if biome_data.temperature < 0.2 {
                    blocks::ICE
                } else {
                    blocks::WATER
                };

                for y in (height + 1)..=sea_level {
                    chunk.set_voxel(
                        x,
                        y,
                        z,
                        Voxel {
                            id: water_block,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // Snow layer for cold biomes at high elevations
        if biome_data.temperature < 0.3 && height > 90 && height + 1 < CHUNK_SIZE_Y {
            chunk.set_voxel(
                x,
                height + 1,
                z,
                Voxel {
                    id: blocks::SNOW,
                    ..Default::default()
                },
            );
        }
    }

    /// Populate chunk with trees based on biome.
    fn populate_trees(
        &self,
        chunk: &mut Chunk,
        heightmap: &Heightmap,
        chunk_origin_x: i32,
        chunk_origin_z: i32,
    ) {
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

            // Get surface height at this position
            let surface_height = heightmap.get(local_x, local_z);

            // Calculate world position
            let world_x = chunk_origin_x + local_x as i32;
            let world_z = chunk_origin_z + local_z as i32;
            let world_y = surface_height + 1; // Place on top of surface

            // Check if surface is suitable for trees (grass or dirt)
            let surface_block = chunk.voxel(local_x, surface_height as usize, local_z);
            if surface_block.id == blocks::GRASS || surface_block.id == blocks::DIRT {
                // Create and place tree
                let tree = Tree::new(world_x, world_y, world_z, tree_type);
                tree.generate_into_chunk(chunk);
            }
        }
    }

    /// Carve caves through generated terrain
    fn carve_caves(&self, chunk: &mut Chunk, chunk_origin_x: i32, chunk_origin_z: i32) {
        // Iterate through all blocks in chunk
        for local_y in 0..CHUNK_SIZE_Y {
            for local_z in 0..CHUNK_SIZE_Z {
                for local_x in 0..CHUNK_SIZE_X {
                    // Calculate world coordinates
                    let world_x = chunk_origin_x + local_x as i32;
                    let world_y = local_y as i32;
                    let world_z = chunk_origin_z + local_z as i32;

                    // Check if this position should be carved as cave
                    if self.cave_generator.is_cave(world_x, world_y, world_z) {
                        let current_voxel = chunk.voxel(local_x, local_y, local_z);

                        // Only carve through solid blocks (don't carve air or water)
                        if current_voxel.id != blocks::AIR && current_voxel.id != blocks::WATER {
                            // Check if we should fill with water (underground lakes)
                            if self.cave_generator.should_have_water(world_y) {
                                chunk.set_voxel(
                                    local_x,
                                    local_y,
                                    local_z,
                                    Voxel {
                                        id: blocks::WATER,
                                        ..Default::default()
                                    },
                                );
                            } else {
                                // Carve as air
                                chunk.set_voxel(
                                    local_x,
                                    local_y,
                                    local_z,
                                    Voxel::default(), // Air
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get surface depth (number of non-stone blocks at top).
    fn get_surface_depth(&self, biome: BiomeId) -> usize {
        match biome {
            BiomeId::Desert => 5,    // Thick sand layer
            BiomeId::Ocean => 3,     // Sand/gravel
            BiomeId::DeepOcean => 4, // Thicker ocean floor
            BiomeId::Swamp => 2,     // Shallow dirt
            _ => 3,                  // Standard grass/dirt depth
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

        // Should have stone somewhere in middle
        let mut found_stone = false;
        for y in 5..60 {
            let voxel = chunk.voxel(8, y, 8);
            if voxel.id == blocks::STONE {
                found_stone = true;
                break;
            }
        }
        assert!(found_stone, "Should have stone layer");
    }

    #[test]
    fn test_terrain_has_surface_blocks() {
        let gen = TerrainGenerator::new(456);
        let chunk = gen.generate_chunk(ChunkPos::new(0, 0));

        // Should have surface blocks (grass, dirt, sand) near top
        let mut found_surface = false;
        for y in 50..100 {
            let voxel = chunk.voxel(8, y, 8);
            if voxel.id == blocks::GRASS || voxel.id == blocks::DIRT || voxel.id == blocks::SAND {
                found_surface = true;
                break;
            }
        }
        assert!(found_surface, "Should have surface blocks");
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
                        assert!(
                            voxel.id == blocks::GRASS
                                || voxel.id == blocks::SAND
                                || voxel.id == blocks::SNOW
                                || voxel.id == blocks::DIRT
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
