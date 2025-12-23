//! Farming system for growing crops.
//!
//! Implements farmland, crops, and growth mechanics.

use crate::chunk::{BlockId, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::terrain::blocks;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{BTreeSet, HashMap};

/// Block IDs for farming system
pub mod farming_blocks {
    use crate::chunk::BlockId;

    pub const FARMLAND: BlockId = 47;
    pub const FARMLAND_WET: BlockId = 48;
    pub const WHEAT_0: BlockId = 49;
    pub const WHEAT_1: BlockId = 50;
    pub const WHEAT_2: BlockId = 51;
    pub const WHEAT_3: BlockId = 52;
    pub const WHEAT_4: BlockId = 53;
    pub const WHEAT_5: BlockId = 54;
    pub const WHEAT_6: BlockId = 55;
    pub const WHEAT_7: BlockId = 56;
    pub const CARROTS_0: BlockId = 57;
    pub const CARROTS_1: BlockId = 58;
    pub const CARROTS_2: BlockId = 59;
    pub const CARROTS_3: BlockId = 60;
    pub const POTATOES_0: BlockId = 61;
    pub const POTATOES_1: BlockId = 62;
    pub const POTATOES_2: BlockId = 63;
    pub const POTATOES_3: BlockId = 64;
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
    /// Positions of crops that need updates (BTreeSet for deterministic iteration)
    crop_positions: BTreeSet<CropPosition>,
    /// Dirty chunks that need mesh rebuilding (BTreeSet for deterministic iteration)
    dirty_chunks: BTreeSet<ChunkPos>,
}

/// Position of a crop in the world
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
            crop_positions: BTreeSet::new(),
            dirty_chunks: BTreeSet::new(),
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

    /// Unregister all crops in a chunk (e.g. when unloading it).
    pub fn unregister_chunk(&mut self, chunk: ChunkPos) {
        let start = CropPosition {
            chunk,
            x: 0,
            y: 0,
            z: 0,
        };
        let end = CropPosition {
            chunk,
            x: u8::MAX,
            y: u8::MAX,
            z: u8::MAX,
        };

        let to_remove: Vec<CropPosition> =
            self.crop_positions.range(start..=end).copied().collect();
        for pos in to_remove {
            self.crop_positions.remove(&pos);
        }
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

        // Skip work when there are no registered crops.
        //
        // This keeps farming "pay for play" and avoids scanning every loaded chunk in worlds
        // where the player hasn't started farming yet.
        if self.crop_positions.is_empty() {
            return;
        }

        // Only scan chunks that contain crops (farmland hydration matters for crop growth).
        // `crop_positions` is a `BTreeSet`, so iteration order is deterministic.
        let crop_chunks: Vec<ChunkPos> = {
            let mut chunks = Vec::new();
            let mut last_chunk: Option<ChunkPos> = None;
            for crop in &self.crop_positions {
                if Some(crop.chunk) == last_chunk {
                    continue;
                }
                last_chunk = Some(crop.chunk);
                chunks.push(crop.chunk);
            }
            chunks
        };

        for chunk_pos in crop_chunks {
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
                        if matches!(voxel.id, blocks::WATER | crate::BLOCK_WATER_FLOWING)
                            || (crate::block_supports_waterlogging(voxel.id)
                                && crate::is_waterlogged(voxel.state))
                        {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Take the set of dirty chunks (clears internal state)
    pub fn take_dirty_chunks(&mut self) -> BTreeSet<ChunkPos> {
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

    #[test]
    fn test_crop_type_base_block_ids() {
        assert_eq!(CropType::Wheat.base_block_id(), farming_blocks::WHEAT_0);
        assert_eq!(CropType::Carrots.base_block_id(), farming_blocks::CARROTS_0);
        assert_eq!(
            CropType::Potatoes.base_block_id(),
            farming_blocks::POTATOES_0
        );
    }

    #[test]
    fn test_potato_stages() {
        for stage in 0..=3 {
            let id = CropType::Potatoes.block_id_at_stage(stage);
            let (crop_type, parsed_stage) = CropType::from_block_id(id).unwrap();
            assert_eq!(crop_type, CropType::Potatoes);
            assert_eq!(parsed_stage, stage);
        }
    }

    #[test]
    fn test_crop_stage_clamping() {
        // Stage higher than max should be clamped
        let wheat_max = CropType::Wheat.block_id_at_stage(100);
        assert_eq!(wheat_max, farming_blocks::WHEAT_7);

        let carrot_max = CropType::Carrots.block_id_at_stage(100);
        assert_eq!(carrot_max, farming_blocks::CARROTS_3);
    }

    #[test]
    fn test_from_block_id_invalid() {
        assert!(CropType::from_block_id(blocks::STONE).is_none());
        assert!(CropType::from_block_id(blocks::AIR).is_none());
        assert!(CropType::from_block_id(blocks::DIRT).is_none());
    }

    /// Helper to create a test chunk
    fn create_test_chunk() -> Chunk {
        Chunk::new(ChunkPos::new(0, 0))
    }

    #[test]
    fn test_crop_growth_system_new() {
        let system = CropGrowthSystem::new(12345);
        assert_eq!(system.crop_count(), 0);
    }

    #[test]
    fn test_register_and_unregister_crop() {
        let mut system = CropGrowthSystem::new(12345);
        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };

        system.register_crop(pos);
        assert_eq!(system.crop_count(), 1);

        system.unregister_crop(pos);
        assert_eq!(system.crop_count(), 0);
    }

    #[test]
    fn test_register_duplicate_crop() {
        let mut system = CropGrowthSystem::new(12345);
        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };

        system.register_crop(pos);
        system.register_crop(pos);
        assert_eq!(system.crop_count(), 1); // BTreeSet prevents duplicates
    }

    #[test]
    fn test_take_dirty_chunks() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Setup: farmland with wheat
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: farming_blocks::FARMLAND_WET,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };
        system.register_crop(pos);

        // Run many ticks to ensure some growth happens
        for tick in 0..1000 {
            system.tick(tick, &mut chunks);
        }

        // Check if we got dirty chunks (may or may not depending on RNG)
        let dirty = system.take_dirty_chunks();
        // Second call should be empty
        let dirty2 = system.take_dirty_chunks();
        assert!(dirty2.is_empty());

        // dirty may or may not contain chunks depending on RNG
        let _ = dirty;
    }

    #[test]
    fn test_crop_growth_requires_farmland() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Setup: wheat on stone (no farmland)
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 15,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };
        system.register_crop(pos);

        // Run ticks - growth should not happen
        for tick in 0..1000 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let crop = chunk.voxel(5, 64, 5);
        // Crop should remain at stage 0
        assert_eq!(crop.id, farming_blocks::WHEAT_0);
    }

    #[test]
    fn test_crop_growth_requires_light() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Setup: wheat with farmland but no light
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: farming_blocks::FARMLAND_WET,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 0, // No light
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };
        system.register_crop(pos);

        // Run ticks - growth should not happen due to low light
        for tick in 0..1000 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let crop = chunk.voxel(5, 64, 5);
        // Crop should remain at stage 0 (light level < 9)
        assert_eq!(crop.id, farming_blocks::WHEAT_0);
    }

    #[test]
    fn test_crop_unregistered_when_fully_grown() {
        let mut system = CropGrowthSystem::new(42); // Specific seed for reproducibility
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Setup: wheat at max stage
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: farming_blocks::FARMLAND_WET,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::WHEAT_7, // Already fully grown
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };
        system.register_crop(pos);
        assert_eq!(system.crop_count(), 1);

        // After ticking, fully grown crops should be unregistered
        for tick in 0..1000 {
            system.tick(tick, &mut chunks);
        }

        // Fully grown crop should have been unregistered
        assert_eq!(system.crop_count(), 0);
    }

    #[test]
    fn test_crop_unregistered_when_replaced() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Setup: wheat on farmland
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: farming_blocks::FARMLAND_WET,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };
        system.register_crop(pos);

        // Replace crop with stone
        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(
                5,
                64,
                5,
                Voxel {
                    id: blocks::STONE,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );
        }

        // After ticking, non-crop block should cause unregister
        for tick in 0..1000 {
            system.tick(tick, &mut chunks);
        }

        // Crop should have been unregistered
        assert_eq!(system.crop_count(), 0);
    }

    #[test]
    fn test_crop_position_ordering() {
        // Test that CropPosition ordering is deterministic
        let pos1 = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 0,
            y: 64,
            z: 0,
        };
        let pos2 = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 1,
            y: 64,
            z: 0,
        };
        let pos3 = CropPosition {
            chunk: ChunkPos::new(1, 0),
            x: 0,
            y: 64,
            z: 0,
        };

        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
    }

    #[test]
    fn test_crop_at_y_zero() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place crop at y=0 (no farmland possible below)
        chunk.set_voxel(
            5,
            0,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 0,
            z: 5,
        };
        system.register_crop(pos);

        // Should not crash when checking growth conditions
        for tick in 0..100 {
            system.tick(tick, &mut chunks);
        }

        // Crop should still be at stage 0 (no farmland below)
        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(5, 0, 5).id, farming_blocks::WHEAT_0);
    }

    #[test]
    fn test_farmland_hydration_with_water() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place dry farmland with water nearby
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::FARMLAND,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        // Place a crop above so the hydration system is exercised.
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            7,
            64,
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        system.register_crop(CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 65,
            z: 5,
        });

        // Run hydration update (every 20 ticks)
        for tick in 0..21 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let farmland = chunk.voxel(5, 64, 5);

        // Farmland should be wet now (water within 4 blocks)
        assert_eq!(farmland.id, farming_blocks::FARMLAND_WET);
    }

    #[test]
    fn test_farmland_hydration_with_flowing_water() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::FARMLAND,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            7,
            64,
            5,
            Voxel {
                id: crate::BLOCK_WATER_FLOWING,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        system.register_crop(CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 65,
            z: 5,
        });

        for tick in 0..21 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let farmland = chunk.voxel(5, 64, 5);
        assert_eq!(farmland.id, farming_blocks::FARMLAND_WET);
    }

    #[test]
    fn test_farmland_hydration_with_waterlogged_block() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::FARMLAND,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            7,
            64,
            5,
            Voxel {
                id: crate::interactive_blocks::STONE_SLAB,
                state: crate::set_waterlogged(0, true),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        system.register_crop(CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 65,
            z: 5,
        });

        for tick in 0..21 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let farmland = chunk.voxel(5, 64, 5);
        assert_eq!(farmland.id, farming_blocks::FARMLAND_WET);
    }

    #[test]
    fn test_farmland_dries_without_water() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place wet farmland with no water nearby
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::FARMLAND_WET,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        // Place a crop above so the hydration system is exercised.
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        system.register_crop(CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 65,
            z: 5,
        });

        // Run hydration update
        for tick in 0..21 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let farmland = chunk.voxel(5, 64, 5);

        // Farmland should dry without water
        assert_eq!(farmland.id, farming_blocks::FARMLAND);
    }

    #[test]
    fn test_default_implementation() {
        let system = CropGrowthSystem::default();
        assert_eq!(system.crop_count(), 0);
    }

    #[test]
    fn test_missing_chunk_handling() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();

        let pos = CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 64,
            z: 5,
        };
        system.register_crop(pos);

        // Tick without chunk in map - should not crash
        system.tick(0, &mut chunks);

        // Crop should remain registered (waiting for chunk)
        assert_eq!(system.crop_count(), 1);
    }

    #[test]
    fn test_multiple_crops_growth() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Setup multiple crops
        for x in 0..3 {
            chunk.set_voxel(
                x,
                63,
                5,
                Voxel {
                    id: farming_blocks::FARMLAND_WET,
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                },
            );
            chunk.set_voxel(
                x,
                64,
                5,
                Voxel {
                    id: farming_blocks::WHEAT_0,
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                },
            );
            system.register_crop(CropPosition {
                chunk: ChunkPos::new(0, 0),
                x: x as u8,
                y: 64,
                z: 5,
            });
        }
        chunks.insert(ChunkPos::new(0, 0), chunk);

        assert_eq!(system.crop_count(), 3);

        // Run many ticks
        for tick in 0..10000 {
            system.tick(tick, &mut chunks);
        }

        // At least some crops should have grown or been unregistered
        // (can't guarantee specific outcome due to RNG)
    }

    #[test]
    fn test_hydration_check_water_one_below() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place dry farmland with water one block below
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::FARMLAND,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        // Place a crop above so the hydration system is exercised.
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        system.register_crop(CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 65,
            z: 5,
        });

        // Run hydration update
        for tick in 0..21 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let farmland = chunk.voxel(5, 64, 5);

        // Farmland should be wet (water at same level or one below)
        assert_eq!(farmland.id, farming_blocks::FARMLAND_WET);
    }

    #[test]
    fn test_hydration_water_too_far() {
        let mut system = CropGrowthSystem::new(12345);
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place dry farmland with water 5 blocks away (too far)
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: farming_blocks::FARMLAND,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        // Place a crop above so the hydration system is exercised.
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: farming_blocks::WHEAT_0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            10,
            64,
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        system.register_crop(CropPosition {
            chunk: ChunkPos::new(0, 0),
            x: 5,
            y: 65,
            z: 5,
        });

        // Run hydration update
        for tick in 0..21 {
            system.tick(tick, &mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let farmland = chunk.voxel(5, 64, 5);

        // Farmland should stay dry (water too far - manhattan distance 5 > 4)
        assert_eq!(farmland.id, farming_blocks::FARMLAND);
    }
}
