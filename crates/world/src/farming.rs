//! Farming system for growing crops.
//!
//! Implements farmland, crops, and growth mechanics.

use crate::chunk::{BlockId, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::terrain::blocks;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};

/// Block IDs for farming system
pub mod farming_blocks {
    use crate::chunk::BlockId;

    pub const FARMLAND: BlockId = 48;
    pub const FARMLAND_WET: BlockId = 49;
    pub const WHEAT_0: BlockId = 50;
    pub const WHEAT_1: BlockId = 51;
    pub const WHEAT_2: BlockId = 52;
    pub const WHEAT_3: BlockId = 53;
    pub const WHEAT_4: BlockId = 54;
    pub const WHEAT_5: BlockId = 55;
    pub const WHEAT_6: BlockId = 56;
    pub const WHEAT_7: BlockId = 57;
    pub const CARROTS_0: BlockId = 58;
    pub const CARROTS_1: BlockId = 59;
    pub const CARROTS_2: BlockId = 60;
    pub const CARROTS_3: BlockId = 61;
    pub const POTATOES_0: BlockId = 62;
    pub const POTATOES_1: BlockId = 63;
    pub const POTATOES_2: BlockId = 64;
    pub const POTATOES_3: BlockId = 65;
}

/// Type of crop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CropType {
    Wheat,
    Carrots,
    Potatoes,
}

impl CropType {
    /// Get the base block ID for this crop at stage 0
    pub fn base_block_id(self) -> BlockId {
        match self {
            CropType::Wheat => farming_blocks::WHEAT_0,
            CropType::Carrots => farming_blocks::CARROTS_0,
            CropType::Potatoes => farming_blocks::POTATOES_0,
        }
    }

    /// Get the number of growth stages for this crop
    pub fn max_stage(self) -> u8 {
        match self {
            CropType::Wheat => 7,
            CropType::Carrots => 3,
            CropType::Potatoes => 3,
        }
    }

    /// Get the block ID for a specific growth stage
    pub fn block_id_at_stage(self, stage: u8) -> BlockId {
        let stage = stage.min(self.max_stage());
        self.base_block_id() + stage as BlockId
    }

    /// Get crop type from block ID
    pub fn from_block_id(block_id: BlockId) -> Option<(CropType, u8)> {
        if (farming_blocks::WHEAT_0..=farming_blocks::WHEAT_7).contains(&block_id) {
            Some((CropType::Wheat, (block_id - farming_blocks::WHEAT_0) as u8))
        } else if (farming_blocks::CARROTS_0..=farming_blocks::CARROTS_3).contains(&block_id) {
            Some((
                CropType::Carrots,
                (block_id - farming_blocks::CARROTS_0) as u8,
            ))
        } else if (farming_blocks::POTATOES_0..=farming_blocks::POTATOES_3).contains(&block_id) {
            Some((
                CropType::Potatoes,
                (block_id - farming_blocks::POTATOES_0) as u8,
            ))
        } else {
            None
        }
    }

    /// Check if a block ID is a crop
    pub fn is_crop(block_id: BlockId) -> bool {
        Self::from_block_id(block_id).is_some()
    }

    /// Check if a crop is fully grown
    pub fn is_fully_grown(block_id: BlockId) -> bool {
        if let Some((crop_type, stage)) = Self::from_block_id(block_id) {
            stage >= crop_type.max_stage()
        } else {
            false
        }
    }
}

/// Check if a block is farmland (wet or dry)
pub fn is_farmland(block_id: BlockId) -> bool {
    block_id == farming_blocks::FARMLAND || block_id == farming_blocks::FARMLAND_WET
}

/// Check if a block can be tilled into farmland
pub fn can_till(block_id: BlockId) -> bool {
    block_id == blocks::DIRT || block_id == blocks::GRASS
}

/// Crop growth system using deterministic random ticks
pub struct CropGrowthSystem {
    /// World seed for determinism
    world_seed: u64,
    /// Positions of crops that need updates
    crop_positions: HashSet<CropPosition>,
    /// Dirty chunks that need mesh rebuilding
    dirty_chunks: HashSet<ChunkPos>,
}

/// Position of a crop in the world
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CropPosition {
    pub chunk: ChunkPos,
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl CropGrowthSystem {
    /// Create a new crop growth system
    pub fn new(world_seed: u64) -> Self {
        Self {
            world_seed,
            crop_positions: HashSet::new(),
            dirty_chunks: HashSet::new(),
        }
    }

    /// Register a crop for growth updates
    pub fn register_crop(&mut self, pos: CropPosition) {
        self.crop_positions.insert(pos);
    }

    /// Unregister a crop (when broken or fully grown)
    pub fn unregister_crop(&mut self, pos: CropPosition) {
        self.crop_positions.remove(&pos);
    }

    /// Tick crop growth (called each game tick)
    pub fn tick(&mut self, tick: u64, chunks: &mut HashMap<ChunkPos, Chunk>) {
        // Create a seeded RNG for this tick
        let tick_seed = self.world_seed.wrapping_add(tick.wrapping_mul(0x5EED_CAFE));
        let mut rng = StdRng::seed_from_u64(tick_seed);

        // Collect crops to update (avoid borrow issues)
        let crops_to_check: Vec<CropPosition> = self.crop_positions.iter().copied().collect();

        for pos in crops_to_check {
            // Random tick chance (1 in 100 per tick, roughly 1 growth every 5 seconds at 20 TPS)
            if rng.gen_ratio(1, 100) {
                self.try_grow_crop(pos, chunks, &mut rng);
            }
        }

        // Update farmland hydration
        self.update_farmland_hydration(tick, chunks);
    }

    /// Try to grow a crop at the given position
    fn try_grow_crop(
        &mut self,
        pos: CropPosition,
        chunks: &mut HashMap<ChunkPos, Chunk>,
        rng: &mut StdRng,
    ) {
        let chunk = match chunks.get(&pos.chunk) {
            Some(c) => c,
            None => return,
        };

        let voxel = chunk.voxel(pos.x as usize, pos.y as usize, pos.z as usize);
        let (crop_type, stage) = match CropType::from_block_id(voxel.id) {
            Some(c) => c,
            None => {
                // Not a crop anymore, unregister
                self.crop_positions.remove(&pos);
                return;
            }
        };

        // Check if already fully grown
        if stage >= crop_type.max_stage() {
            self.crop_positions.remove(&pos);
            return;
        }

        // Check growth conditions
        let can_grow = self.check_growth_conditions(pos, chunks, rng);
        if !can_grow {
            return;
        }

        // Grow the crop
        let new_stage = stage + 1;
        let new_block_id = crop_type.block_id_at_stage(new_stage);

        if let Some(chunk) = chunks.get_mut(&pos.chunk) {
            chunk.set_voxel(
                pos.x as usize,
                pos.y as usize,
                pos.z as usize,
                Voxel {
                    id: new_block_id,
                    state: 0,
                    light_sky: voxel.light_sky,
                    light_block: voxel.light_block,
                },
            );
            self.dirty_chunks.insert(pos.chunk);
        }

        // Unregister if now fully grown
        if new_stage >= crop_type.max_stage() {
            self.crop_positions.remove(&pos);
        }
    }

    /// Check if conditions are right for crop growth
    fn check_growth_conditions(
        &self,
        pos: CropPosition,
        chunks: &HashMap<ChunkPos, Chunk>,
        rng: &mut StdRng,
    ) -> bool {
        // Check for farmland below
        if pos.y == 0 {
            return false;
        }

        let chunk = match chunks.get(&pos.chunk) {
            Some(c) => c,
            None => return false,
        };

        let below = chunk.voxel(pos.x as usize, (pos.y - 1) as usize, pos.z as usize);
        if !is_farmland(below.id) {
            return false;
        }

        // Hydrated farmland gives better growth chance
        let hydration_bonus = if below.id == farming_blocks::FARMLAND_WET {
            2
        } else {
            1
        };

        // Light level affects growth (need light level 9+)
        let crop = chunk.voxel(pos.x as usize, pos.y as usize, pos.z as usize);
        let light_level = crop.light_sky.max(crop.light_block);
        if light_level < 9 {
            return false;
        }

        // Growth chance based on conditions
        // Higher hydration and light = better chance
        let growth_chance = (hydration_bonus * light_level as u32) as f32 / 30.0;
        rng.gen::<f32>() < growth_chance
    }

    /// Update farmland hydration based on nearby water
    fn update_farmland_hydration(&mut self, tick: u64, chunks: &mut HashMap<ChunkPos, Chunk>) {
        // Only check every 20 ticks (once per second)
        if !tick.is_multiple_of(20) {
            return;
        }

        let chunk_positions: Vec<ChunkPos> = chunks.keys().copied().collect();

        for chunk_pos in chunk_positions {
            self.update_chunk_farmland(chunk_pos, chunks);
        }
    }

    /// Update farmland in a single chunk
    fn update_chunk_farmland(
        &mut self,
        chunk_pos: ChunkPos,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        let mut updates: Vec<(usize, usize, usize, BlockId)> = Vec::new();

        if let Some(chunk) = chunks.get(&chunk_pos) {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    for x in 0..CHUNK_SIZE_X {
                        let voxel = chunk.voxel(x, y, z);
                        if is_farmland(voxel.id) {
                            let should_be_wet = self.check_water_nearby(chunk_pos, x, y, z, chunks);
                            let expected_id = if should_be_wet {
                                farming_blocks::FARMLAND_WET
                            } else {
                                farming_blocks::FARMLAND
                            };

                            if voxel.id != expected_id {
                                updates.push((x, y, z, expected_id));
                            }
                        }
                    }
                }
            }
        }

        if !updates.is_empty() {
            if let Some(chunk) = chunks.get_mut(&chunk_pos) {
                for (x, y, z, new_id) in updates {
                    let voxel = chunk.voxel(x, y, z);
                    chunk.set_voxel(
                        x,
                        y,
                        z,
                        Voxel {
                            id: new_id,
                            ..voxel
                        },
                    );
                }
                self.dirty_chunks.insert(chunk_pos);
            }
        }
    }

    /// Check if there's water within 4 blocks horizontally of the farmland
    fn check_water_nearby(
        &self,
        chunk_pos: ChunkPos,
        x: usize,
        y: usize,
        z: usize,
        chunks: &HashMap<ChunkPos, Chunk>,
    ) -> bool {
        let world_x = chunk_pos.x * CHUNK_SIZE_X as i32 + x as i32;
        let world_z = chunk_pos.z * CHUNK_SIZE_Z as i32 + z as i32;

        // Check 4-block radius at same level or one below
        for dy in 0..=1i32 {
            let check_y = y as i32 - dy;
            if check_y < 0 || check_y >= CHUNK_SIZE_Y as i32 {
                continue;
            }

            for dx in -4..=4i32 {
                for dz in -4..=4i32 {
                    // Manhattan distance <= 4
                    if dx.abs() + dz.abs() > 4 {
                        continue;
                    }

                    let check_x = world_x + dx;
                    let check_z = world_z + dz;

                    // Get chunk and local position
                    let check_chunk_x = check_x.div_euclid(CHUNK_SIZE_X as i32);
                    let check_chunk_z = check_z.div_euclid(CHUNK_SIZE_Z as i32);
                    let local_x = check_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
                    let local_z = check_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

                    let check_pos = ChunkPos::new(check_chunk_x, check_chunk_z);
                    if let Some(chunk) = chunks.get(&check_pos) {
                        let voxel = chunk.voxel(local_x, check_y as usize, local_z);
                        if voxel.id == blocks::WATER {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Take the set of dirty chunks (clears internal state)
    pub fn take_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
    }

    /// Get the number of registered crops
    pub fn crop_count(&self) -> usize {
        self.crop_positions.len()
    }
}

impl Default for CropGrowthSystem {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_type_stages() {
        assert_eq!(CropType::Wheat.max_stage(), 7);
        assert_eq!(CropType::Carrots.max_stage(), 3);
        assert_eq!(CropType::Potatoes.max_stage(), 3);
    }

    #[test]
    fn test_crop_block_ids() {
        // Wheat stages
        for stage in 0..=7 {
            let id = CropType::Wheat.block_id_at_stage(stage);
            let (crop_type, parsed_stage) = CropType::from_block_id(id).unwrap();
            assert_eq!(crop_type, CropType::Wheat);
            assert_eq!(parsed_stage, stage);
        }

        // Carrots stages
        for stage in 0..=3 {
            let id = CropType::Carrots.block_id_at_stage(stage);
            let (crop_type, parsed_stage) = CropType::from_block_id(id).unwrap();
            assert_eq!(crop_type, CropType::Carrots);
            assert_eq!(parsed_stage, stage);
        }
    }

    #[test]
    fn test_is_crop() {
        assert!(CropType::is_crop(farming_blocks::WHEAT_0));
        assert!(CropType::is_crop(farming_blocks::WHEAT_7));
        assert!(CropType::is_crop(farming_blocks::CARROTS_0));
        assert!(!CropType::is_crop(blocks::DIRT));
        assert!(!CropType::is_crop(blocks::STONE));
    }

    #[test]
    fn test_is_fully_grown() {
        assert!(!CropType::is_fully_grown(farming_blocks::WHEAT_0));
        assert!(!CropType::is_fully_grown(farming_blocks::WHEAT_6));
        assert!(CropType::is_fully_grown(farming_blocks::WHEAT_7));
        assert!(!CropType::is_fully_grown(farming_blocks::CARROTS_0));
        assert!(CropType::is_fully_grown(farming_blocks::CARROTS_3));
    }

    #[test]
    fn test_can_till() {
        assert!(can_till(blocks::DIRT));
        assert!(can_till(blocks::GRASS));
        assert!(!can_till(blocks::STONE));
        assert!(!can_till(blocks::SAND));
    }

    #[test]
    fn test_is_farmland() {
        assert!(is_farmland(farming_blocks::FARMLAND));
        assert!(is_farmland(farming_blocks::FARMLAND_WET));
        assert!(!is_farmland(blocks::DIRT));
        assert!(!is_farmland(blocks::GRASS));
    }
}
