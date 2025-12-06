//! Fluid physics simulation for water and lava.
//!
//! Implements cellular automata-based fluid flow mechanics with deterministic updates.

use crate::chunk::{
    BlockId, BlockState, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use crate::terrain::blocks;
use std::collections::{HashMap, HashSet};

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
        BLOCK_LAVA | BLOCK_LAVA_FLOWING => Some(FluidType::Lava),
        _ => None,
    }
}

/// Check if a block ID is a fluid
pub fn is_fluid(block_id: BlockId) -> bool {
    get_fluid_type(block_id).is_some()
}

/// Check if a block ID is a source fluid
pub fn is_source_fluid(block_id: BlockId) -> bool {
    matches!(block_id, blocks::WATER | BLOCK_LAVA)
}

/// Check if a block ID is flowing fluid
pub fn is_flowing_fluid(block_id: BlockId) -> bool {
    matches!(block_id, BLOCK_WATER_FLOWING | BLOCK_LAVA_FLOWING)
}

/// Check if a block can be replaced by fluid (air, flowers, etc.)
pub fn can_fluid_replace(block_id: BlockId) -> bool {
    block_id == blocks::AIR || is_fluid(block_id)
}

/// Check if a block is flammable (can be set on fire by lava)
pub fn is_flammable(block_id: BlockId) -> bool {
    // Wood-like blocks
    matches!(block_id, 11 | 12) // oak_log, oak_planks
}

/// World position for fluid updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        let local_y = self.y as usize;
        let local_z = self.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        (ChunkPos::new(chunk_x, chunk_z), local_x, local_y, local_z)
    }
}

/// Fluid simulator using cellular automata approach
pub struct FluidSimulator {
    /// Pending fluid updates (position -> scheduled tick)
    pending_updates: HashMap<FluidPos, u64>,
    /// Current simulation tick
    current_tick: u64,
    /// Dirty chunks that need mesh rebuilding
    dirty_chunks: HashSet<ChunkPos>,
}

impl FluidSimulator {
    /// Create a new fluid simulator
    pub fn new() -> Self {
        Self {
            pending_updates: HashMap::new(),
            current_tick: 0,
            dirty_chunks: HashSet::new(),
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
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return;
        }

        // Get the voxel at this position
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        // Skip if not a fluid
        let fluid_type = match get_fluid_type(voxel.id) {
            Some(ft) => ft,
            None => return,
        };

        let is_source = is_source_fluid(voxel.id);
        let current_level = if is_source {
            FLUID_LEVEL_SOURCE
        } else {
            get_fluid_level(voxel.state)
        };

        // Try to flow down first
        let down_pos = FluidPos::new(pos.x, pos.y - 1, pos.z);
        if pos.y > 0 {
            if let Some(down_voxel) = self.get_voxel(down_pos, chunks) {
                if can_fluid_replace(down_voxel.id)
                    || get_fluid_type(down_voxel.id) == Some(fluid_type)
                {
                    // Flow down
                    let new_level = if is_source {
                        FLUID_LEVEL_SOURCE
                    } else {
                        current_level
                    };
                    let flowing_id = match fluid_type {
                        FluidType::Water => BLOCK_WATER_FLOWING,
                        FluidType::Lava => BLOCK_LAVA_FLOWING,
                    };

                    // Check for water + lava interaction
                    if let Some(interaction) =
                        self.check_fluid_interaction(down_pos, fluid_type, chunks)
                    {
                        self.set_voxel(down_pos, interaction, chunks);
                    } else {
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
                            // Only flow if we have higher level
                            let neighbor_level = if is_source_fluid(neighbor_voxel.id) {
                                FLUID_LEVEL_SOURCE
                            } else {
                                get_fluid_level(neighbor_voxel.state)
                            };
                            new_level > neighbor_level
                                && get_fluid_type(neighbor_voxel.id) == Some(fluid_type)
                        } else {
                            can_fluid_replace(neighbor_voxel.id)
                        };

                        if should_flow {
                            // Check for water + lava interaction
                            if let Some(interaction) =
                                self.check_fluid_interaction(neighbor, fluid_type, chunks)
                            {
                                self.set_voxel(neighbor, interaction, chunks);
                            } else {
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

        // Handle lava setting things on fire
        if fluid_type == FluidType::Lava {
            for neighbor in pos.horizontal_neighbors() {
                if let Some(neighbor_voxel) = self.get_voxel(neighbor, chunks) {
                    if is_flammable(neighbor_voxel.id) {
                        // Replace with fire (or just remove for now)
                        self.set_voxel(
                            neighbor,
                            Voxel {
                                id: blocks::AIR,
                                state: 0,
                                light_sky: 0,
                                light_block: 0,
                            },
                            chunks,
                        );
                    }
                }
            }
        }
    }

    /// Check for water + lava interaction and return resulting block if any
    fn check_fluid_interaction(
        &self,
        pos: FluidPos,
        incoming_type: FluidType,
        chunks: &HashMap<ChunkPos, Chunk>,
    ) -> Option<Voxel> {
        let existing_voxel = self.get_voxel(pos, chunks)?;
        let existing_type = get_fluid_type(existing_voxel.id)?;

        // Water meeting lava or lava meeting water
        if incoming_type != existing_type {
            // Lava source + water = obsidian
            // Flowing lava + water = cobblestone
            // Water + lava source = stone (lava becomes obsidian)
            // Water + flowing lava = stone (lava becomes cobblestone)

            let result_id = if is_source_fluid(existing_voxel.id) {
                // Existing is source
                BLOCK_OBSIDIAN
            } else {
                // Existing is flowing
                blocks::STONE
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
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        let (chunk_pos, local_x, local_y, local_z) = pos.to_chunk_local();
        chunks
            .get(&chunk_pos)
            .map(|chunk| chunk.voxel(local_x, local_y, local_z))
    }

    /// Set voxel at a world position
    fn set_voxel(&mut self, pos: FluidPos, voxel: Voxel, chunks: &mut HashMap<ChunkPos, Chunk>) {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return;
        }

        let (chunk_pos, local_x, local_y, local_z) = pos.to_chunk_local();
        if let Some(chunk) = chunks.get_mut(&chunk_pos) {
            chunk.set_voxel(local_x, local_y, local_z, voxel);
            self.dirty_chunks.insert(chunk_pos);
        }
    }

    /// Take the set of dirty chunks (clears internal state)
    pub fn take_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
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
                if voxel.id == blocks::WATER {
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
        assert_eq!(ly, 64);
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
}
