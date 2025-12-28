//! Fluid physics simulation for water and lava.
//!
//! Implements cellular automata-based fluid flow mechanics with deterministic updates.

use crate::chunk::{
    world_y_to_local_y, BlockId, BlockState, Chunk, ChunkPos, Voxel, BLOCK_FIRE, CHUNK_SIZE_X,
    CHUNK_SIZE_Z,
};
use crate::terrain::blocks;
use crate::{block_supports_waterlogging, is_waterlogged};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Fluid type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FluidType {
    Water,
    Lava,
}

impl FluidType {
    /// Get the block ID for a source block of this fluid type
    pub fn source_block_id(self) -> BlockId {
        match self {
            FluidType::Water => blocks::WATER,
            FluidType::Lava => BLOCK_LAVA,
        }
    }

    /// Get the maximum flow distance from a source
    pub fn max_flow_distance(self) -> u8 {
        match self {
            FluidType::Water => 7,
            FluidType::Lava => 3,
        }
    }

    /// Get the flow speed (ticks between updates)
    pub fn flow_speed(self) -> u32 {
        match self {
            FluidType::Water => 1,
            FluidType::Lava => 4,
        }
    }

    /// Get the light level emitted by this fluid (0 for water)
    pub fn light_level(self) -> u8 {
        match self {
            FluidType::Water => 0,
            FluidType::Lava => 15,
        }
    }

    /// Check if this fluid sets blocks on fire
    pub fn causes_fire(self) -> bool {
        matches!(self, FluidType::Lava)
    }

    /// Get damage per tick when player is in this fluid
    pub fn damage_per_tick(self) -> f32 {
        match self {
            FluidType::Water => 0.0,
            FluidType::Lava => 4.0,
        }
    }
}

/// Block ID for lava (added to registry)
pub const BLOCK_LAVA: BlockId = 20;

/// Legacy alias for the lava source block.
///
/// Historically `config/blocks.json` contained a second lava-like entry at id 81 that could be
/// referenced by name via the block registry. Keep it behaving like lava for backward
/// compatibility with existing saves/scripts.
pub const BLOCK_LAVA_LEGACY: BlockId = 81;

/// Block ID for flowing water (level encoded in state)
pub const BLOCK_WATER_FLOWING: BlockId = 21;

/// Block ID for flowing lava (level encoded in state)
pub const BLOCK_LAVA_FLOWING: BlockId = 22;

/// Maximum fluid level (source = 8, flowing = 1-7)
pub const FLUID_LEVEL_SOURCE: u8 = 8;

/// Fluid level stored in the low 4 bits of block state
pub fn get_fluid_level(state: BlockState) -> u8 {
    (state & 0x0F) as u8
}

/// Set fluid level in block state (preserves upper bits)
pub fn set_fluid_level(state: BlockState, level: u8) -> BlockState {
    (state & 0xFFF0) | (level as BlockState & 0x0F)
}

/// Check if the falling flag is set (fluid falling down)
pub fn is_falling(state: BlockState) -> bool {
    (state & 0x10) != 0
}

/// Set the falling flag in block state
pub fn set_falling(state: BlockState, falling: bool) -> BlockState {
    if falling {
        state | 0x10
    } else {
        state & !0x10
    }
}

/// Get fluid type from block ID
pub fn get_fluid_type(block_id: BlockId) -> Option<FluidType> {
    match block_id {
        blocks::WATER | BLOCK_WATER_FLOWING => Some(FluidType::Water),
        BLOCK_LAVA | BLOCK_LAVA_LEGACY | BLOCK_LAVA_FLOWING => Some(FluidType::Lava),
        _ => None,
    }
}

/// Check if a block ID is a fluid
pub fn is_fluid(block_id: BlockId) -> bool {
    get_fluid_type(block_id).is_some()
}

/// Check if a block ID is a source fluid
pub fn is_source_fluid(block_id: BlockId) -> bool {
    matches!(block_id, blocks::WATER | BLOCK_LAVA | BLOCK_LAVA_LEGACY)
}

/// Check if a block ID is flowing fluid
pub fn is_flowing_fluid(block_id: BlockId) -> bool {
    matches!(block_id, BLOCK_WATER_FLOWING | BLOCK_LAVA_FLOWING)
}

/// Check if a block can be replaced by fluid (air, flowers, etc.)
pub fn can_fluid_replace(block_id: BlockId) -> bool {
    block_id == blocks::AIR || block_id == BLOCK_FIRE || is_fluid(block_id)
}

fn voxel_is_waterlogged(voxel: Voxel) -> bool {
    block_supports_waterlogging(voxel.id) && is_waterlogged(voxel.state)
}

/// Check if a block is flammable (can be set on fire by lava)
pub fn is_flammable(block_id: BlockId) -> bool {
    use crate::trees::tree_blocks;

    matches!(
        block_id,
        // Logs, planks, and wood-like stations.
        crate::BLOCK_OAK_LOG
            | crate::BLOCK_OAK_PLANKS
            | crate::BLOCK_CRAFTING_TABLE
            | crate::BLOCK_BOOKSHELF
            // Leaves.
            | tree_blocks::LEAVES
            | tree_blocks::BIRCH_LEAVES
            | tree_blocks::PINE_LEAVES
            // Other logs.
            | tree_blocks::BIRCH_LOG
            | tree_blocks::PINE_LOG
            // Common wood derivatives.
            | crate::interactive_blocks::OAK_DOOR_LOWER
            | crate::interactive_blocks::OAK_DOOR_UPPER
            | crate::interactive_blocks::TRAPDOOR
            | crate::interactive_blocks::LADDER
            | crate::interactive_blocks::OAK_FENCE
            | crate::interactive_blocks::OAK_FENCE_GATE
            | crate::interactive_blocks::OAK_SLAB
            | crate::interactive_blocks::OAK_STAIRS
            | crate::interactive_blocks::BED_HEAD
            | crate::interactive_blocks::BED_FOOT
            | crate::interactive_blocks::CHEST
            | crate::redstone_blocks::OAK_BUTTON
            | crate::redstone_blocks::OAK_PRESSURE_PLATE
    )
}

/// World position for fluid updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FluidPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl FluidPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Get neighbors in flow order: down, north, south, east, west
    pub fn neighbors(&self) -> [FluidPos; 5] {
        [
            FluidPos::new(self.x, self.y - 1, self.z), // Down (priority)
            FluidPos::new(self.x, self.y, self.z - 1), // North
            FluidPos::new(self.x, self.y, self.z + 1), // South
            FluidPos::new(self.x + 1, self.y, self.z), // East
            FluidPos::new(self.x - 1, self.y, self.z), // West
        ]
    }

    /// Get only horizontal neighbors
    pub fn horizontal_neighbors(&self) -> [FluidPos; 4] {
        [
            FluidPos::new(self.x, self.y, self.z - 1), // North
            FluidPos::new(self.x, self.y, self.z + 1), // South
            FluidPos::new(self.x + 1, self.y, self.z), // East
            FluidPos::new(self.x - 1, self.y, self.z), // West
        ]
    }

    /// Convert to chunk position and local position
    pub fn to_chunk_local(&self) -> (ChunkPos, usize, usize, usize) {
        let chunk_x = self.x.div_euclid(CHUNK_SIZE_X as i32);
        let chunk_z = self.z.div_euclid(CHUNK_SIZE_Z as i32);
        let local_x = self.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_y = world_y_to_local_y(self.y).expect("y in world bounds");
        let local_z = self.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        (ChunkPos::new(chunk_x, chunk_z), local_x, local_y, local_z)
    }
}

/// Fluid simulator using cellular automata approach
pub struct FluidSimulator {
    /// Pending fluid updates (position -> scheduled tick)
    pending_updates: BTreeMap<FluidPos, u64>,
    /// Current simulation tick
    current_tick: u64,
    /// Dirty chunks that need mesh rebuilding
    dirty_chunks: HashSet<ChunkPos>,
    /// Dirty chunks that need block-light recomputation
    dirty_light_chunks: HashSet<ChunkPos>,
}

impl FluidSimulator {
    /// Create a new fluid simulator
    pub fn new() -> Self {
        Self {
            pending_updates: BTreeMap::new(),
            current_tick: 0,
            dirty_chunks: HashSet::new(),
            dirty_light_chunks: HashSet::new(),
        }
    }

    /// Schedule a fluid update at a position
    pub fn schedule_update(&mut self, pos: FluidPos, delay: u32) {
        let tick = self.current_tick + delay as u64;
        // Only schedule if not already scheduled for earlier
        if let Some(&existing) = self.pending_updates.get(&pos) {
            if existing <= tick {
                return;
            }
        }
        self.pending_updates.insert(pos, tick);
    }

    /// Tick the fluid simulation
    pub fn tick(&mut self, chunks: &mut HashMap<ChunkPos, Chunk>) {
        self.current_tick += 1;

        // Collect updates that are due
        let due_updates: Vec<FluidPos> = self
            .pending_updates
            .iter()
            .filter(|(_, &tick)| tick <= self.current_tick)
            .map(|(pos, _)| *pos)
            .collect();

        // Remove due updates from pending
        for pos in &due_updates {
            self.pending_updates.remove(pos);
        }

        // Process each update
        for pos in due_updates {
            self.process_update(pos, chunks);
        }
    }

    /// Process a single fluid update
    fn process_update(&mut self, pos: FluidPos, chunks: &mut HashMap<ChunkPos, Chunk>) {
        // Skip if position is out of world bounds
        if world_y_to_local_y(pos.y).is_none() {
            return;
        }

        // Get the voxel at this position
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        // Determine fluid type.
        //
        // We treat waterlogged blocks as in-place water sources (foundation support).
        let (fluid_type, is_source, current_level) = match get_fluid_type(voxel.id) {
            Some(ft) => {
                let is_source = is_source_fluid(voxel.id);
                let current_level = if is_source {
                    FLUID_LEVEL_SOURCE
                } else {
                    get_fluid_level(voxel.state)
                };
                (ft, is_source, current_level)
            }
            None if voxel_is_waterlogged(voxel) => (FluidType::Water, true, FLUID_LEVEL_SOURCE),
            None => return,
        };

        // Try to flow down first
        let down_pos = FluidPos::new(pos.x, pos.y - 1, pos.z);
        let down_voxel = self.get_voxel(down_pos, chunks);
        if let Some(down_voxel) = down_voxel {
            if is_fluid(down_voxel.id) {
                let down_type = get_fluid_type(down_voxel.id);
                if down_type != Some(fluid_type) {
                    if let Some(interaction) = self.check_fluid_interaction(
                        down_pos,
                        fluid_type,
                        is_source,
                        chunks,
                    ) {
                        self.set_voxel(down_pos, interaction, chunks);
                    }
                } else {
                    // Flow down into same fluid type.
                    let new_level = if is_source {
                        FLUID_LEVEL_SOURCE
                    } else {
                        current_level
                    };
                    let flowing_id = match fluid_type {
                        FluidType::Water => BLOCK_WATER_FLOWING,
                        FluidType::Lava => BLOCK_LAVA_FLOWING,
                    };

                    let new_state = set_falling(set_fluid_level(0, new_level), true);
                    self.set_voxel(
                        down_pos,
                        Voxel {
                            id: flowing_id,
                            state: new_state,
                            light_sky: 0,
                            light_block: fluid_type.light_level(),
                        },
                        chunks,
                    );
                    self.schedule_update(down_pos, fluid_type.flow_speed());
                }
            } else if can_fluid_replace(down_voxel.id) {
                // Flow down into replaceable blocks.
                let new_level = if is_source {
                    FLUID_LEVEL_SOURCE
                } else {
                    current_level
                };
                let flowing_id = match fluid_type {
                    FluidType::Water => BLOCK_WATER_FLOWING,
                    FluidType::Lava => BLOCK_LAVA_FLOWING,
                };

                let new_state = set_falling(set_fluid_level(0, new_level), true);
                self.set_voxel(
                    down_pos,
                    Voxel {
                        id: flowing_id,
                        state: new_state,
                        light_sky: 0,
                        light_block: fluid_type.light_level(),
                    },
                    chunks,
                );
                self.schedule_update(down_pos, fluid_type.flow_speed());
            }
        }

        if fluid_type == FluidType::Water && !is_source && !is_falling(voxel.state) {
            if self.check_infinite_water(pos, chunks) {
                self.set_voxel(
                    pos,
                    Voxel {
                        id: blocks::WATER,
                        state: 0,
                        light_sky: 0,
                        light_block: 0,
                    },
                    chunks,
                );
                self.schedule_update(pos, fluid_type.flow_speed());
                return;
            }
        }

        // Spread horizontally if we have remaining level
        if current_level > 1 || is_source {
            let new_level = if is_source {
                fluid_type.max_flow_distance()
            } else {
                current_level.saturating_sub(1)
            };

            if new_level > 0 {
                for neighbor in pos.horizontal_neighbors() {
                    if let Some(neighbor_voxel) = self.get_voxel(neighbor, chunks) {
                        // Check if we can flow to this neighbor
                        let should_flow = if is_fluid(neighbor_voxel.id) {
                            let neighbor_type = get_fluid_type(neighbor_voxel.id);
                            if neighbor_type != Some(fluid_type) {
                                true
                            } else {
                                // Only flow if we have higher level
                                let neighbor_level = if is_source_fluid(neighbor_voxel.id) {
                                    FLUID_LEVEL_SOURCE
                                } else {
                                    get_fluid_level(neighbor_voxel.state)
                                };
                                new_level > neighbor_level
                            }
                        } else {
                            can_fluid_replace(neighbor_voxel.id)
                        };

                        if should_flow {
                            // Check for water + lava interaction
                            if let Some(interaction) = self.check_fluid_interaction(
                                neighbor,
                                fluid_type,
                                is_source,
                                chunks,
                            )
                            {
                                self.set_voxel(neighbor, interaction, chunks);
                            } else if !is_fluid(neighbor_voxel.id)
                                || get_fluid_type(neighbor_voxel.id) == Some(fluid_type)
                            {
                                let flowing_id = match fluid_type {
                                    FluidType::Water => BLOCK_WATER_FLOWING,
                                    FluidType::Lava => BLOCK_LAVA_FLOWING,
                                };
                                let new_state = set_fluid_level(0, new_level);
                                self.set_voxel(
                                    neighbor,
                                    Voxel {
                                        id: flowing_id,
                                        state: new_state,
                                        light_sky: 0,
                                        light_block: fluid_type.light_level(),
                                    },
                                    chunks,
                                );
                                self.schedule_update(neighbor, fluid_type.flow_speed());
                            }
                        }
                    }
                }
            }
        }

        // Handle lava starting fires near flammable blocks (vanilla-ish; simplified).
        if fluid_type == FluidType::Lava {
            for neighbor in pos.horizontal_neighbors() {
                let Some(here) = self.get_voxel(neighbor, chunks) else {
                    continue;
                };
                if here.id != blocks::AIR {
                    continue;
                }

                let below = FluidPos::new(neighbor.x, neighbor.y - 1, neighbor.z);
                let Some(below_voxel) = self.get_voxel(below, chunks) else {
                    continue;
                };
                if !is_flammable(below_voxel.id) {
                    continue;
                }

                self.set_voxel(
                    neighbor,
                    Voxel {
                        id: BLOCK_FIRE,
                        state: 0,
                        light_sky: 0,
                        light_block: 0,
                    },
                    chunks,
                );
            }
        }
    }

    /// Check for water + lava interaction and return resulting block if any
    fn check_fluid_interaction(
        &self,
        pos: FluidPos,
        incoming_type: FluidType,
        incoming_is_source: bool,
        chunks: &HashMap<ChunkPos, Chunk>,
    ) -> Option<Voxel> {
        let existing_voxel = self.get_voxel(pos, chunks)?;
        let existing_type = get_fluid_type(existing_voxel.id)?;

        // Water meeting lava or lava meeting water
        if incoming_type != existing_type {
            // Lava source + water = obsidian
            // Flowing lava + water = cobblestone
            let lava_is_source = if incoming_type == FluidType::Lava {
                incoming_is_source
            } else {
                is_source_fluid(existing_voxel.id)
            };

            let result_id = if lava_is_source {
                BLOCK_OBSIDIAN
            } else {
                crate::BLOCK_COBBLESTONE
            };

            return Some(Voxel {
                id: result_id,
                state: 0,
                light_sky: 0,
                light_block: 0,
            });
        }

        None
    }

    /// Get voxel at a world position
    fn get_voxel(&self, pos: FluidPos, chunks: &HashMap<ChunkPos, Chunk>) -> Option<Voxel> {
        let local_y = world_y_to_local_y(pos.y)?;
        let chunk_pos = ChunkPos::new(
            pos.x.div_euclid(CHUNK_SIZE_X as i32),
            pos.z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let local_x = pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_z = pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        chunks
            .get(&chunk_pos)
            .map(|chunk| chunk.voxel(local_x, local_y, local_z))
    }

    /// Set voxel at a world position
    fn set_voxel(&mut self, pos: FluidPos, voxel: Voxel, chunks: &mut HashMap<ChunkPos, Chunk>) {
        let local_y = match world_y_to_local_y(pos.y) {
            Some(y) => y,
            None => return,
        };

        let chunk_pos = ChunkPos::new(
            pos.x.div_euclid(CHUNK_SIZE_X as i32),
            pos.z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let local_x = pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_z = pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        if let Some(chunk) = chunks.get_mut(&chunk_pos) {
            let old = chunk.voxel(local_x, local_y, local_z);
            let old_emissive = matches!(
                old.id,
                BLOCK_LAVA | BLOCK_LAVA_LEGACY | BLOCK_LAVA_FLOWING | BLOCK_FIRE
            );
            let new_emissive = matches!(
                voxel.id,
                BLOCK_LAVA | BLOCK_LAVA_LEGACY | BLOCK_LAVA_FLOWING | BLOCK_FIRE
            );
            let old_opaque = matches!(old.id, blocks::STONE | BLOCK_OBSIDIAN);
            let new_opaque = matches!(voxel.id, blocks::STONE | BLOCK_OBSIDIAN);

            chunk.set_voxel(local_x, local_y, local_z, voxel);
            self.dirty_chunks.insert(chunk_pos);
            if old_emissive != new_emissive || old_opaque != new_opaque {
                self.dirty_light_chunks.insert(chunk_pos);
            }
        }
    }

    /// Take the set of dirty chunks (clears internal state)
    pub fn take_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
    }

    /// Take the set of chunks that need block-light recomputation.
    pub fn take_dirty_light_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_light_chunks)
    }

    /// Get pending update count
    pub fn pending_count(&self) -> usize {
        self.pending_updates.len()
    }

    /// Notify the simulator that a fluid block was placed
    pub fn on_fluid_placed(&mut self, pos: FluidPos, fluid_type: FluidType) {
        self.schedule_update(pos, fluid_type.flow_speed());
    }

    /// Notify the simulator that a fluid block was removed
    pub fn on_fluid_removed(&mut self, pos: FluidPos, chunks: &HashMap<ChunkPos, Chunk>) {
        // Schedule updates for neighbors to potentially fill in
        for neighbor in pos.neighbors() {
            if let Some(voxel) = self.get_voxel(neighbor, chunks) {
                if let Some(ft) = get_fluid_type(voxel.id) {
                    self.schedule_update(neighbor, ft.flow_speed());
                } else if voxel_is_waterlogged(voxel) {
                    self.schedule_update(neighbor, FluidType::Water.flow_speed());
                }
            }
        }
    }

    /// Check for infinite water source creation
    /// Returns true if this position should become a water source
    pub fn check_infinite_water(&self, pos: FluidPos, chunks: &HashMap<ChunkPos, Chunk>) -> bool {
        // Count adjacent water sources (horizontally)
        let mut source_count = 0;
        for neighbor in pos.horizontal_neighbors() {
            if let Some(voxel) = self.get_voxel(neighbor, chunks) {
                if voxel.id == blocks::WATER || voxel_is_waterlogged(voxel) {
                    source_count += 1;
                }
            }
        }
        // Two or more adjacent sources create infinite water
        source_count >= 2
    }
}

impl Default for FluidSimulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Block ID for obsidian
pub const BLOCK_OBSIDIAN: BlockId = 23;

/// Swimming physics modifier
#[derive(Debug, Clone, Copy)]
pub struct SwimmingState {
    /// Whether the player is in water
    pub in_water: bool,
    /// Whether the player is in lava
    pub in_lava: bool,
    /// Water depth (0.0 = not in water, 1.0 = fully submerged)
    pub water_depth: f32,
}

impl Default for SwimmingState {
    fn default() -> Self {
        Self {
            in_water: false,
            in_lava: false,
            water_depth: 0.0,
        }
    }
}

impl SwimmingState {
    /// Movement speed multiplier when in fluid
    pub fn movement_multiplier(&self) -> f32 {
        if self.in_lava {
            0.3 // Very slow in lava
        } else if self.in_water {
            0.6 // Slower in water
        } else {
            1.0
        }
    }

    /// Vertical drag when in fluid
    pub fn vertical_drag(&self) -> f32 {
        if self.in_lava {
            0.5
        } else if self.in_water {
            0.8
        } else {
            1.0
        }
    }

    /// Whether fall damage should be negated
    pub fn negates_fall_damage(&self) -> bool {
        self.in_water
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::interactive_blocks;
    use crate::set_waterlogged;

    fn local_y(world_y: i32) -> usize {
        crate::chunk::world_y_to_local_y(world_y).expect("world y in bounds")
    }

    #[test]
    fn test_fluid_level_encoding() {
        let state = set_fluid_level(0, 5);
        assert_eq!(get_fluid_level(state), 5);

        let state = set_fluid_level(state, 3);
        assert_eq!(get_fluid_level(state), 3);
    }

    #[test]
    fn test_falling_flag() {
        let state = set_falling(0, true);
        assert!(is_falling(state));

        let state = set_falling(state, false);
        assert!(!is_falling(state));

        // Test combination with level
        let state = set_falling(set_fluid_level(0, 7), true);
        assert!(is_falling(state));
        assert_eq!(get_fluid_level(state), 7);
    }

    #[test]
    fn test_fluid_type_detection() {
        assert_eq!(get_fluid_type(blocks::WATER), Some(FluidType::Water));
        assert_eq!(get_fluid_type(BLOCK_LAVA), Some(FluidType::Lava));
        assert_eq!(get_fluid_type(BLOCK_WATER_FLOWING), Some(FluidType::Water));
        assert_eq!(get_fluid_type(BLOCK_LAVA_FLOWING), Some(FluidType::Lava));
        assert_eq!(get_fluid_type(blocks::STONE), None);
    }

    #[test]
    fn test_fluid_pos_to_chunk_local() {
        let pos = FluidPos::new(17, 64, -5);
        let (chunk_pos, lx, ly, lz) = pos.to_chunk_local();
        assert_eq!(chunk_pos, ChunkPos::new(1, -1));
        assert_eq!(lx, 1);
        assert_eq!(ly, local_y(64));
        assert_eq!(lz, 11);
    }

    #[test]
    fn test_swimming_state() {
        let mut state = SwimmingState::default();
        assert_eq!(state.movement_multiplier(), 1.0);

        state.in_water = true;
        assert!(state.movement_multiplier() < 1.0);
        assert!(state.negates_fall_damage());

        state.in_water = false;
        state.in_lava = true;
        assert!(state.movement_multiplier() < 0.5);
        assert!(!state.negates_fall_damage());
    }

    #[test]
    fn test_flow_distances() {
        assert_eq!(FluidType::Water.max_flow_distance(), 7);
        assert_eq!(FluidType::Lava.max_flow_distance(), 3);
    }

    #[test]
    fn test_fluid_type_properties() {
        // Water properties
        assert_eq!(FluidType::Water.source_block_id(), blocks::WATER);
        assert_eq!(FluidType::Water.flow_speed(), 1);
        assert_eq!(FluidType::Water.light_level(), 0);
        assert!(!FluidType::Water.causes_fire());
        assert_eq!(FluidType::Water.damage_per_tick(), 0.0);

        // Lava properties
        assert_eq!(FluidType::Lava.source_block_id(), BLOCK_LAVA);
        assert_eq!(FluidType::Lava.flow_speed(), 4);
        assert_eq!(FluidType::Lava.light_level(), 15);
        assert!(FluidType::Lava.causes_fire());
        assert_eq!(FluidType::Lava.damage_per_tick(), 4.0);
    }

    #[test]
    fn test_is_fluid_functions() {
        assert!(is_fluid(blocks::WATER));
        assert!(is_fluid(BLOCK_LAVA));
        assert!(is_fluid(BLOCK_WATER_FLOWING));
        assert!(is_fluid(BLOCK_LAVA_FLOWING));
        assert!(!is_fluid(blocks::STONE));
        assert!(!is_fluid(blocks::AIR));
    }

    #[test]
    fn test_is_source_fluid() {
        assert!(is_source_fluid(blocks::WATER));
        assert!(is_source_fluid(BLOCK_LAVA));
        assert!(!is_source_fluid(BLOCK_WATER_FLOWING));
        assert!(!is_source_fluid(BLOCK_LAVA_FLOWING));
        assert!(!is_source_fluid(blocks::STONE));
    }

    #[test]
    fn test_is_flowing_fluid() {
        assert!(!is_flowing_fluid(blocks::WATER));
        assert!(!is_flowing_fluid(BLOCK_LAVA));
        assert!(is_flowing_fluid(BLOCK_WATER_FLOWING));
        assert!(is_flowing_fluid(BLOCK_LAVA_FLOWING));
        assert!(!is_flowing_fluid(blocks::STONE));
    }

    #[test]
    fn test_can_fluid_replace() {
        assert!(can_fluid_replace(blocks::AIR));
        assert!(can_fluid_replace(BLOCK_FIRE));
        assert!(can_fluid_replace(blocks::WATER));
        assert!(can_fluid_replace(BLOCK_LAVA));
        assert!(can_fluid_replace(BLOCK_WATER_FLOWING));
        assert!(!can_fluid_replace(blocks::STONE));
        assert!(!can_fluid_replace(blocks::DIRT));
    }

    #[test]
    fn test_is_flammable() {
        assert!(is_flammable(crate::BLOCK_OAK_LOG));
        assert!(is_flammable(crate::BLOCK_OAK_PLANKS));
        assert!(is_flammable(crate::BLOCK_CRAFTING_TABLE));
        assert!(is_flammable(crate::interactive_blocks::CHEST));
        assert!(is_flammable(crate::trees::tree_blocks::LEAVES));
        assert!(!is_flammable(blocks::STONE));
        assert!(!is_flammable(blocks::DIRT));
    }

    #[test]
    fn test_fluid_pos_neighbors() {
        let pos = FluidPos::new(5, 64, 5);
        let neighbors = pos.neighbors();

        // Should return 5 neighbors (down first, then 4 horizontal)
        assert_eq!(neighbors.len(), 5);

        // First should be below
        assert_eq!(neighbors[0], FluidPos::new(5, 63, 5));

        // Check horizontal neighbors
        let horizontal = pos.horizontal_neighbors();
        assert_eq!(horizontal.len(), 4);
    }

    /// Helper to create a test chunk
    fn create_test_chunk() -> Chunk {
        Chunk::new(ChunkPos::new(0, 0))
    }

    #[test]
    fn test_simulator_new() {
        let sim = FluidSimulator::new();
        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_schedule_update() {
        let mut sim = FluidSimulator::new();
        let pos = FluidPos::new(5, 64, 5);

        sim.schedule_update(pos, 1);
        assert_eq!(sim.pending_count(), 1);

        // Scheduling same position with later tick keeps earlier
        sim.schedule_update(pos, 10);
        assert_eq!(sim.pending_count(), 1);

        // Different position adds to pending
        sim.schedule_update(FluidPos::new(6, 64, 5), 1);
        assert_eq!(sim.pending_count(), 2);
    }

    #[test]
    fn test_schedule_update_earlier_overrides() {
        let mut sim = FluidSimulator::new();
        let pos = FluidPos::new(5, 64, 5);

        // Schedule for tick 10
        sim.schedule_update(pos, 10);
        // Then schedule for tick 5 (earlier) - should override
        sim.schedule_update(pos, 5);

        // Tick 6 times - the earlier update should have processed
        for _ in 0..6 {
            let mut chunks = HashMap::new();
            sim.tick(&mut chunks);
        }

        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_on_fluid_placed() {
        let mut sim = FluidSimulator::new();
        let pos = FluidPos::new(5, 64, 5);

        sim.on_fluid_placed(pos, FluidType::Water);
        assert_eq!(sim.pending_count(), 1);

        // Lava schedules with longer delay
        sim.on_fluid_placed(FluidPos::new(6, 64, 5), FluidType::Lava);
        assert_eq!(sim.pending_count(), 2);
    }

    #[test]
    fn test_on_fluid_removed() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place water around a position
        chunk.set_voxel(
            4,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = FluidPos::new(5, 64, 5);
        sim.on_fluid_removed(pos, &chunks);

        // Should schedule updates for water neighbors
        assert!(sim.pending_count() >= 2);
    }

    #[test]
    fn test_check_infinite_water() {
        let sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place one water source
        chunk.set_voxel(
            4,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = FluidPos::new(5, 64, 5);

        // One source = not infinite
        assert!(!sim.check_infinite_water(pos, &chunks));

        // Add second source
        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0)) {
            chunk.set_voxel(
                6,
                local_y(64),
                5,
                Voxel {
                    id: blocks::WATER,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );
        }

        // Two sources = infinite
        assert!(sim.check_infinite_water(pos, &chunks));
    }

    #[test]
    fn test_take_dirty_chunks() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place water source
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        // Schedule and process update
        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let dirty = sim.take_dirty_chunks();
        assert!(dirty.contains(&ChunkPos::new(0, 0)));

        // Second call returns empty
        let dirty2 = sim.take_dirty_chunks();
        assert!(dirty2.is_empty());
    }

    #[test]
    fn test_water_flows_down() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place water source with air below
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::AIR,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let below = chunk.voxel(5, local_y(63), 5);

        // Water should have flowed down
        assert_eq!(below.id, BLOCK_WATER_FLOWING);
        assert!(is_falling(below.state));
    }

    #[test]
    fn test_water_spreads_horizontally() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place water source on solid ground
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::AIR,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let adjacent = chunk.voxel(6, local_y(64), 5);

        // Water should have spread horizontally
        assert_eq!(adjacent.id, BLOCK_WATER_FLOWING);
        assert_eq!(get_fluid_level(adjacent.state), 7); // max_flow_distance for water
    }

    #[test]
    fn test_water_lava_interaction_horizontal() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place water and lava sources next to each other on solid ground.
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_LAVA,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let converted = chunk.voxel(6, local_y(64), 5);
        assert_eq!(converted.id, BLOCK_OBSIDIAN);
    }

    #[test]
    fn test_water_flowing_lava_interaction_horizontal() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place water next to a flowing lava block.
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_LAVA_FLOWING,
                state: set_fluid_level(0, 3),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let converted = chunk.voxel(6, local_y(64), 5);
        assert_eq!(converted.id, crate::BLOCK_COBBLESTONE);
    }

    #[test]
    fn test_lava_flowing_water_interaction_horizontal() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place flowing lava next to a water source.
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_LAVA_FLOWING,
                state: set_fluid_level(0, 3),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(6, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let converted = chunk.voxel(5, local_y(64), 5);
        assert_eq!(converted.id, crate::BLOCK_COBBLESTONE);
    }

    #[test]
    fn test_infinite_water_creates_source() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Solid ground under the water.
        for x in 4..=6 {
            chunk.set_voxel(
                x,
                local_y(63),
                5,
                Voxel {
                    id: blocks::STONE,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );
        }

        // Two sources with a flowing water block between them.
        chunk.set_voxel(
            4,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_WATER_FLOWING,
                state: set_fluid_level(0, 1),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let mid = chunk.voxel(5, local_y(64), 5);
        assert_eq!(mid.id, blocks::WATER);
    }

    #[test]
    fn test_waterlogged_slab_acts_as_source() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: interactive_blocks::STONE_SLAB,
                state: set_waterlogged(0, true),
                ..Default::default()
            },
        );
        // Solid below so we spread horizontally deterministically.
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::AIR,
                light_sky: 15,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let source = chunk.voxel(5, local_y(64), 5);
        assert_eq!(source.id, interactive_blocks::STONE_SLAB);
        assert!(is_waterlogged(source.state));

        let adjacent = chunk.voxel(6, local_y(64), 5);
        assert_eq!(adjacent.id, BLOCK_WATER_FLOWING);
        assert_eq!(get_fluid_level(adjacent.state), 7);
    }

    #[test]
    fn test_flowing_water_does_not_auto_waterlog_slab() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place a water source on solid ground.
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        // Adjacent slab should not change unless explicitly waterlogged via block placement or
        // a bucket interaction.
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: interactive_blocks::STONE_SLAB,
                state: 0,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let adjacent = chunk.voxel(6, local_y(64), 5);
        assert_eq!(adjacent.id, interactive_blocks::STONE_SLAB);
        assert!(!is_waterlogged(adjacent.state));
    }

    #[test]
    fn test_flowing_water_does_not_auto_waterlog_stairs() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: interactive_blocks::STONE_STAIRS,
                state: 0,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let adjacent = chunk.voxel(6, local_y(64), 5);
        assert_eq!(adjacent.id, interactive_blocks::STONE_STAIRS);
        assert!(!is_waterlogged(adjacent.state));
    }

    #[test]
    fn test_flowing_water_does_not_auto_waterlog_trapdoor() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::WATER,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: interactive_blocks::TRAPDOOR,
                state: 0,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                ..Default::default()
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let adjacent = chunk.voxel(6, local_y(64), 5);
        assert_eq!(adjacent.id, interactive_blocks::TRAPDOOR);
        assert!(!is_waterlogged(adjacent.state));
    }

    #[test]
    fn test_lava_spreads_slower() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place lava source on solid ground
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_LAVA,
                state: 0,
                light_sky: 0,
                light_block: 15,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::AIR,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let adjacent = chunk.voxel(6, local_y(64), 5);

        // Lava should have spread with shorter flow distance
        assert_eq!(adjacent.id, BLOCK_LAVA_FLOWING);
        assert_eq!(get_fluid_level(adjacent.state), 3); // max_flow_distance for lava
    }

    #[test]
    fn test_out_of_bounds_handling() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), create_test_chunk());

        // Schedule updates at invalid Y coordinates
        sim.schedule_update(FluidPos::new(5, crate::chunk::WORLD_MIN_Y - 1, 5), 0);
        sim.schedule_update(FluidPos::new(5, crate::chunk::WORLD_MAX_Y + 1, 5), 0);

        // Should not crash
        sim.tick(&mut chunks);
    }

    #[test]
    fn test_missing_chunk_handling() {
        let mut sim = FluidSimulator::new();
        let chunks = HashMap::new();

        // Operations on missing chunks should not crash
        sim.on_fluid_removed(FluidPos::new(5, 64, 5), &chunks);
        assert!(!sim.check_infinite_water(FluidPos::new(5, 64, 5), &chunks));
    }

    #[test]
    fn test_default_implementation() {
        let sim = FluidSimulator::default();
        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_swimming_state_default() {
        let state = SwimmingState::default();
        assert!(!state.in_water);
        assert!(!state.in_lava);
        assert_eq!(state.water_depth, 0.0);
    }

    #[test]
    fn test_swimming_vertical_drag() {
        let mut state = SwimmingState::default();
        assert_eq!(state.vertical_drag(), 1.0);

        state.in_water = true;
        assert!(state.vertical_drag() < 1.0);

        state.in_water = false;
        state.in_lava = true;
        assert!(state.vertical_drag() < 0.8);
    }

    #[test]
    fn test_fluid_level_max() {
        // Test level boundaries
        let state = set_fluid_level(0, FLUID_LEVEL_SOURCE);
        assert_eq!(get_fluid_level(state), FLUID_LEVEL_SOURCE);

        let state = set_fluid_level(0, 0);
        assert_eq!(get_fluid_level(state), 0);

        // Test that level is masked to 4 bits
        let state = set_fluid_level(0, 15);
        assert_eq!(get_fluid_level(state), 15);
    }

    #[test]
    fn test_falling_preserves_level() {
        // Set both falling and level
        let state = set_falling(set_fluid_level(0, 5), true);
        assert!(is_falling(state));
        assert_eq!(get_fluid_level(state), 5);

        // Unset falling, level should remain
        let state = set_falling(state, false);
        assert!(!is_falling(state));
        assert_eq!(get_fluid_level(state), 5);
    }

    #[test]
    fn test_process_non_fluid_block() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place stone (not a fluid)
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        // Stone should remain unchanged
        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(5, local_y(64), 5).id, blocks::STONE);
    }

    #[test]
    fn test_flowing_water_continues_flowing() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place flowing water with level 5 on solid ground
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_WATER_FLOWING,
                state: set_fluid_level(0, 5),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::AIR,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let adjacent = chunk.voxel(6, local_y(64), 5);

        // Should spread with reduced level (5 - 1 = 4)
        assert_eq!(adjacent.id, BLOCK_WATER_FLOWING);
        assert_eq!(get_fluid_level(adjacent.state), 4);
    }

    #[test]
    fn test_lava_ignites_fire_near_flammable() {
        let mut sim = FluidSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place flowing lava with minimal level so it won't spread into neighbors, but can still
        // ignite nearby air above a flammable block (vanilla-ish; simplified).
        chunk.set_voxel(
            5,
            local_y(64),
            5,
            Voxel {
                id: BLOCK_LAVA_FLOWING,
                state: set_fluid_level(0, 1),
                light_sky: 0,
                light_block: 15,
            },
        );
        chunk.set_voxel(
            5,
            local_y(63),
            5,
            Voxel {
                id: blocks::STONE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(63),
            5,
            Voxel {
                id: 11, // oak_log (flammable)
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            local_y(64),
            5,
            Voxel {
                id: blocks::AIR,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(FluidPos::new(5, 64, 5), 0);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let fire = chunk.voxel(6, local_y(64), 5);
        assert_eq!(fire.id, BLOCK_FIRE);
        let flammable = chunk.voxel(6, local_y(63), 5);
        assert_eq!(flammable.id, 11);
    }

    #[test]
    fn test_fluid_pos_negative_coords() {
        let pos = FluidPos::new(-5, 64, -10);
        let (chunk_pos, lx, ly, lz) = pos.to_chunk_local();

        assert_eq!(chunk_pos, ChunkPos::new(-1, -1));
        assert_eq!(lx, 11);
        assert_eq!(ly, local_y(64));
        assert_eq!(lz, 6);
    }
}
