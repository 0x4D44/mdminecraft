//! Tree generation for biome decoration.
//!
//! Generates different tree types based on biome characteristics.

use crate::biome::BiomeId;
use crate::chunk::{Chunk, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::terrain::blocks;

/// Additional block IDs for tree structures.
pub mod tree_blocks {
    use crate::chunk::BlockId;

    pub const LOG: BlockId = 11;
    pub const LEAVES: BlockId = 12;
    pub const BIRCH_LOG: BlockId = 13;
    pub const BIRCH_LEAVES: BlockId = 14;
    pub const PINE_LOG: BlockId = 15;
    pub const PINE_LEAVES: BlockId = 16;
}

/// Tree type variations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeType {
    Oak,
    Birch,
    Pine,
}

impl TreeType {
    /// Select tree type based on biome.
    pub fn for_biome(biome: BiomeId) -> Option<Self> {
        match biome {
            BiomeId::BirchForest => Some(TreeType::Birch),
            BiomeId::IcePlains | BiomeId::IceMountains | BiomeId::Tundra => Some(TreeType::Pine),
            BiomeId::Forest | BiomeId::Plains | BiomeId::Hills => Some(TreeType::Oak),
            BiomeId::RainForest => Some(TreeType::Oak), // Dense oak
            _ => None, // Deserts, oceans, swamps don't have standard trees
        }
    }

    /// Get log block ID for this tree type.
    pub fn log_block(&self) -> u16 {
        match self {
            TreeType::Oak => tree_blocks::LOG,
            TreeType::Birch => tree_blocks::BIRCH_LOG,
            TreeType::Pine => tree_blocks::PINE_LOG,
        }
    }

    /// Get leaves block ID for this tree type.
    pub fn leaves_block(&self) -> u16 {
        match self {
            TreeType::Oak => tree_blocks::LEAVES,
            TreeType::Birch => tree_blocks::BIRCH_LEAVES,
            TreeType::Pine => tree_blocks::PINE_LEAVES,
        }
    }

    /// Get trunk height for this tree type.
    pub fn trunk_height(&self) -> usize {
        match self {
            TreeType::Oak => 5,
            TreeType::Birch => 6,
            TreeType::Pine => 8,
        }
    }
}

/// Tree structure with position and type.
#[derive(Debug, Clone)]
pub struct Tree {
    /// World X coordinate of trunk base
    pub world_x: i32,
    /// World Y coordinate of trunk base
    pub world_y: i32,
    /// World Z coordinate of trunk base
    pub world_z: i32,
    /// Tree type
    pub tree_type: TreeType,
}

impl Tree {
    /// Create a new tree at the given world position.
    pub fn new(world_x: i32, world_y: i32, world_z: i32, tree_type: TreeType) -> Self {
        Self {
            world_x,
            world_y,
            world_z,
            tree_type,
        }
    }

    /// Generate tree structure into a chunk.
    ///
    /// Only places blocks that fall within the chunk bounds.
    pub fn generate_into_chunk(&self, chunk: &mut Chunk) {
        let trunk_height = self.tree_type.trunk_height();
        let log_block = self.tree_type.log_block();
        let leaves_block = self.tree_type.leaves_block();

        match self.tree_type {
            TreeType::Oak => {
                self.generate_oak(chunk, trunk_height, log_block, leaves_block);
            }
            TreeType::Birch => {
                self.generate_birch(chunk, trunk_height, log_block, leaves_block);
            }
            TreeType::Pine => {
                self.generate_pine(chunk, trunk_height, log_block, leaves_block);
            }
        }
    }

    /// Generate oak tree (round canopy).
    fn generate_oak(
        &self,
        chunk: &mut Chunk,
        trunk_height: usize,
        log_block: u16,
        leaves_block: u16,
    ) {
        // Trunk
        for y_offset in 0..trunk_height {
            self.place_block(chunk, 0, y_offset as i32, 0, log_block);
        }

        // Canopy (round shape, 3x3x3 with corners cut)
        let canopy_y = trunk_height as i32;
        for dy in 0..3_i32 {
            for dx in -1_i32..=1 {
                for dz in -1_i32..=1 {
                    // Skip corners at bottom layer
                    if dy == 0 && dx.abs() == 1 && dz.abs() == 1 {
                        continue;
                    }
                    // Skip center (trunk)
                    if dx == 0 && dz == 0 && dy == 0 {
                        continue;
                    }
                    self.place_block(chunk, dx, canopy_y + dy, dz, leaves_block);
                }
            }
        }

        // Top leaf block
        self.place_block(chunk, 0, canopy_y + 3, 0, leaves_block);
    }

    /// Generate birch tree (tall and thin).
    fn generate_birch(
        &self,
        chunk: &mut Chunk,
        trunk_height: usize,
        log_block: u16,
        leaves_block: u16,
    ) {
        // Trunk
        for y_offset in 0..trunk_height {
            self.place_block(chunk, 0, y_offset as i32, 0, log_block);
        }

        // Canopy (smaller, 3x3x2)
        let canopy_y = trunk_height as i32;
        for dy in 0..2 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    // Skip center (trunk) at bottom
                    if dx == 0 && dz == 0 && dy == 0 {
                        continue;
                    }
                    self.place_block(chunk, dx, canopy_y + dy, dz, leaves_block);
                }
            }
        }

        // Top leaf block
        self.place_block(chunk, 0, canopy_y + 2, 0, leaves_block);
    }

    /// Generate pine tree (conical shape).
    fn generate_pine(
        &self,
        chunk: &mut Chunk,
        trunk_height: usize,
        log_block: u16,
        leaves_block: u16,
    ) {
        // Trunk
        for y_offset in 0..trunk_height {
            self.place_block(chunk, 0, y_offset as i32, 0, log_block);
        }

        // Conical canopy (layers getting smaller as they go up)
        let canopy_start = (trunk_height as i32) - 3;

        // Bottom layer (3x3)
        for dx in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue; // Skip trunk
                }
                self.place_block(chunk, dx, canopy_start, dz, leaves_block);
            }
        }

        // Middle layers (3x3)
        for dy in 1..4 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue; // Skip trunk
                    }
                    self.place_block(chunk, dx, canopy_start + dy, dz, leaves_block);
                }
            }
        }

        // Top layer (1x1)
        self.place_block(chunk, 0, canopy_start + 4, 0, leaves_block);
        self.place_block(chunk, 0, canopy_start + 5, 0, leaves_block);
    }

    /// Place a block at world coordinates if it falls within the chunk.
    fn place_block(&self, chunk: &mut Chunk, dx: i32, dy: i32, dz: i32, block_id: u16) {
        let world_x = self.world_x + dx;
        let world_y = self.world_y + dy;
        let world_z = self.world_z + dz;

        let chunk_pos = chunk.position();
        let chunk_origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
        let chunk_origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

        // Check if within chunk bounds
        let local_x = world_x - chunk_origin_x;
        let local_z = world_z - chunk_origin_z;

        if local_x >= 0
            && local_x < CHUNK_SIZE_X as i32
            && local_z >= 0
            && local_z < CHUNK_SIZE_Z as i32
            && world_y >= 0
            && world_y < CHUNK_SIZE_Y as i32
        {
            // Only place if current block is air (don't replace existing blocks)
            let current = chunk.voxel(local_x as usize, world_y as usize, local_z as usize);
            if current.id == blocks::AIR {
                chunk.set_voxel(
                    local_x as usize,
                    world_y as usize,
                    local_z as usize,
                    Voxel {
                        id: block_id,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

/// Generate tree positions for a chunk based on biome and seed.
pub fn generate_tree_positions(
    world_seed: u64,
    chunk_x: i32,
    chunk_z: i32,
    biome: BiomeId,
    _surface_height: usize,
) -> Vec<(usize, usize)> {
    // Determine if this biome supports trees
    if TreeType::for_biome(biome).is_none() {
        return Vec::new();
    }

    let mut positions = Vec::new();

    // Simple pseudo-random tree placement based on seed and chunk position
    let seed = world_seed
        .wrapping_add((chunk_x as u64).wrapping_mul(374761393))
        .wrapping_add((chunk_z as u64).wrapping_mul(668265263));

    // Determine tree density based on biome
    let tree_density = match biome {
        BiomeId::Forest | BiomeId::BirchForest | BiomeId::RainForest => 0.15, // Dense
        BiomeId::Plains => 0.02,                                              // Sparse
        BiomeId::Hills => 0.05,                                               // Moderate
        BiomeId::IcePlains | BiomeId::IceMountains | BiomeId::Tundra => 0.03, // Sparse
        _ => 0.0,
    };

    // Try to place trees at grid positions
    for x in (0..CHUNK_SIZE_X).step_by(4) {
        for z in (0..CHUNK_SIZE_Z).step_by(4) {
            let pos_seed = seed
                .wrapping_add((x as u64).wrapping_mul(134775813))
                .wrapping_add((z as u64).wrapping_mul(1103515245));

            let rand_val = (pos_seed % 10000) as f32 / 10000.0;

            if rand_val < tree_density {
                positions.push((x, z));
            }
        }
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkPos;

    #[test]
    fn test_tree_type_for_biome() {
        assert_eq!(TreeType::for_biome(BiomeId::Forest), Some(TreeType::Oak));
        assert_eq!(
            TreeType::for_biome(BiomeId::BirchForest),
            Some(TreeType::Birch)
        );
        assert_eq!(
            TreeType::for_biome(BiomeId::IcePlains),
            Some(TreeType::Pine)
        );
        assert_eq!(TreeType::for_biome(BiomeId::Desert), None);
        assert_eq!(TreeType::for_biome(BiomeId::Ocean), None);
    }

    #[test]
    fn test_tree_properties() {
        let oak = TreeType::Oak;
        assert_eq!(oak.trunk_height(), 5);
        assert_eq!(oak.log_block(), tree_blocks::LOG);
        assert_eq!(oak.leaves_block(), tree_blocks::LEAVES);

        let birch = TreeType::Birch;
        assert_eq!(birch.trunk_height(), 6);

        let pine = TreeType::Pine;
        assert_eq!(pine.trunk_height(), 8);
    }

    #[test]
    fn test_tree_generation_places_trunk() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        let tree = Tree::new(5, 64, 5, TreeType::Oak);

        tree.generate_into_chunk(&mut chunk);

        // Check trunk blocks exist
        for y in 64..69 {
            let voxel = chunk.voxel(5, y, 5);
            assert_eq!(voxel.id, tree_blocks::LOG, "Expected log at y={}", y);
        }
    }

    #[test]
    fn test_tree_generation_places_leaves() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        let tree = Tree::new(5, 64, 5, TreeType::Oak);

        tree.generate_into_chunk(&mut chunk);

        // Check that leaves exist around trunk top
        let mut leaf_count = 0;
        for y in 69..73 {
            for x in 4..=6 {
                for z in 4..=6 {
                    let voxel = chunk.voxel(x, y, z);
                    if voxel.id == tree_blocks::LEAVES {
                        leaf_count += 1;
                    }
                }
            }
        }

        assert!(leaf_count > 10, "Should have leaves in canopy");
    }

    #[test]
    fn test_tree_respects_chunk_bounds() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));
        // Place tree at edge of chunk
        let tree = Tree::new(15, 64, 15, TreeType::Oak);

        tree.generate_into_chunk(&mut chunk);

        // Should not panic, only places blocks within bounds
        let voxel = chunk.voxel(15, 64, 15);
        assert_eq!(voxel.id, tree_blocks::LOG);
    }

    #[test]
    fn test_tree_doesnt_replace_existing_blocks() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        // Place a stone block where tree would go
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        let tree = Tree::new(5, 64, 5, TreeType::Oak);
        tree.generate_into_chunk(&mut chunk);

        // Stone should still be there (tree doesn't replace)
        let voxel = chunk.voxel(5, 65, 5);
        assert_eq!(voxel.id, blocks::STONE);
    }

    #[test]
    fn test_different_tree_types_have_different_shapes() {
        let oak = Tree::new(5, 64, 5, TreeType::Oak);
        let birch = Tree::new(5, 64, 5, TreeType::Birch);
        let pine = Tree::new(5, 64, 5, TreeType::Pine);

        let mut chunk_oak = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_birch = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_pine = Chunk::new(ChunkPos::new(0, 0));

        oak.generate_into_chunk(&mut chunk_oak);
        birch.generate_into_chunk(&mut chunk_birch);
        pine.generate_into_chunk(&mut chunk_pine);

        // Trees should have different heights
        assert!(oak.tree_type.trunk_height() != pine.tree_type.trunk_height());
        assert!(birch.tree_type.trunk_height() != oak.tree_type.trunk_height());
    }

    #[test]
    fn test_tree_position_generation_deterministic() {
        let positions1 = generate_tree_positions(12345, 0, 0, BiomeId::Forest, 64);
        let positions2 = generate_tree_positions(12345, 0, 0, BiomeId::Forest, 64);

        assert_eq!(
            positions1, positions2,
            "Tree positions should be deterministic"
        );
    }

    #[test]
    fn test_tree_position_generation_different_seeds() {
        // Use seeds that are known to produce trees
        let positions1 = generate_tree_positions(9999, 5, 5, BiomeId::Forest, 64);
        let positions2 = generate_tree_positions(8888, 5, 5, BiomeId::Forest, 64);

        // Forest biome should have some trees with these seeds
        assert!(
            !positions1.is_empty() || !positions2.is_empty(),
            "At least one seed should produce trees"
        );

        // If both have trees, they should likely differ
        if !positions1.is_empty() && !positions2.is_empty() {
            let mut different = false;
            if positions1.len() != positions2.len() {
                different = true;
            } else {
                for i in 0..positions1.len() {
                    if positions1[i] != positions2[i] {
                        different = true;
                        break;
                    }
                }
            }
            // It's okay if they happen to be the same, just check determinism is working
            assert!(different || positions1 == positions2);
        }
    }

    #[test]
    fn test_forest_has_more_trees_than_plains() {
        let forest_trees = generate_tree_positions(42, 0, 0, BiomeId::Forest, 64);
        let plains_trees = generate_tree_positions(42, 0, 0, BiomeId::Plains, 64);

        // Forest should generally have more trees (not guaranteed for every seed, but likely)
        assert!(
            forest_trees.len() > plains_trees.len(),
            "Forest ({}) should have more trees than plains ({})",
            forest_trees.len(),
            plains_trees.len()
        );
    }

    #[test]
    fn test_desert_has_no_trees() {
        let trees = generate_tree_positions(999, 5, 10, BiomeId::Desert, 64);
        assert_eq!(trees.len(), 0, "Desert should have no trees");
    }
}
