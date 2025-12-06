//! Block interaction system for doors, ladders, beds, and other interactive blocks.
//!
//! Implements block-specific behaviors and state management.

use crate::chunk::{BlockId, BlockState, Chunk, ChunkPos, Voxel, CHUNK_SIZE_Y};
use crate::terrain::blocks;
use std::collections::{HashMap, HashSet};

/// Block IDs for interactive blocks
pub mod interactive_blocks {
    use crate::chunk::BlockId;

    pub const GLASS: BlockId = 25;
    pub const OAK_DOOR_LOWER: BlockId = 26;
    pub const OAK_DOOR_UPPER: BlockId = 27;
    pub const IRON_DOOR_LOWER: BlockId = 28;
    pub const IRON_DOOR_UPPER: BlockId = 29;
    pub const LADDER: BlockId = 30;
    pub const OAK_FENCE: BlockId = 31;
    pub const OAK_FENCE_GATE: BlockId = 32;
    pub const STONE_SLAB: BlockId = 33;
    pub const OAK_SLAB: BlockId = 34;
    pub const STONE_STAIRS: BlockId = 35;
    pub const OAK_STAIRS: BlockId = 36;
    pub const GLASS_PANE: BlockId = 37;
    pub const TRAPDOOR: BlockId = 69;
    pub const TORCH: BlockId = 70;
    pub const BED_HEAD: BlockId = 66;
    pub const BED_FOOT: BlockId = 67;
    pub const CHEST: BlockId = 68;
}

/// Facing direction for blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Facing {
    North,
    South,
    East,
    West,
}

impl Facing {
    /// Get facing from state bits (2 bits)
    pub fn from_state(state: BlockState) -> Self {
        match state & 0x03 {
            0 => Facing::North,
            1 => Facing::South,
            2 => Facing::East,
            _ => Facing::West,
        }
    }

    /// Convert to state bits
    pub fn to_state(self) -> BlockState {
        match self {
            Facing::North => 0,
            Facing::South => 1,
            Facing::East => 2,
            Facing::West => 3,
        }
    }

    /// Get facing from player yaw angle
    pub fn from_yaw(yaw: f32) -> Self {
        // Normalize yaw to 0-360
        let yaw = yaw.rem_euclid(std::f32::consts::TAU);
        let degrees = yaw.to_degrees();

        if !(45.0..315.0).contains(&degrees) {
            Facing::South
        } else if (45.0..135.0).contains(&degrees) {
            Facing::West
        } else if (135.0..225.0).contains(&degrees) {
            Facing::North
        } else {
            Facing::East
        }
    }

    /// Get the opposite facing
    pub fn opposite(self) -> Self {
        match self {
            Facing::North => Facing::South,
            Facing::South => Facing::North,
            Facing::East => Facing::West,
            Facing::West => Facing::East,
        }
    }

    /// Get the offset vector for this facing
    pub fn offset(self) -> (i32, i32) {
        match self {
            Facing::North => (0, -1),
            Facing::South => (0, 1),
            Facing::East => (1, 0),
            Facing::West => (-1, 0),
        }
    }
}

/// Slab position (top or bottom half of block)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlabPosition {
    Bottom,
    Top,
}

impl SlabPosition {
    /// Get position from state
    pub fn from_state(state: BlockState) -> Self {
        if (state & 0x04) != 0 {
            SlabPosition::Top
        } else {
            SlabPosition::Bottom
        }
    }

    /// Set position in state
    pub fn to_state(self, state: BlockState) -> BlockState {
        match self {
            SlabPosition::Bottom => state & !0x04,
            SlabPosition::Top => state | 0x04,
        }
    }
}

/// Check if block state indicates door is open
pub fn is_door_open(state: BlockState) -> bool {
    (state & 0x08) != 0
}

/// Set door open state
pub fn set_door_open(state: BlockState, open: bool) -> BlockState {
    if open {
        state | 0x08
    } else {
        state & !0x08
    }
}

/// Check if a block is a door (upper or lower)
pub fn is_door(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::OAK_DOOR_LOWER
            | interactive_blocks::OAK_DOOR_UPPER
            | interactive_blocks::IRON_DOOR_LOWER
            | interactive_blocks::IRON_DOOR_UPPER
    )
}

/// Check if a block is the lower part of a door
pub fn is_door_lower(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::OAK_DOOR_LOWER | interactive_blocks::IRON_DOOR_LOWER
    )
}

/// Check if a block is the upper part of a door
pub fn is_door_upper(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::OAK_DOOR_UPPER | interactive_blocks::IRON_DOOR_UPPER
    )
}

/// Check if a door is made of iron (requires redstone to open)
pub fn is_iron_door(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::IRON_DOOR_LOWER | interactive_blocks::IRON_DOOR_UPPER
    )
}

/// Check if a block is a fence gate
pub fn is_fence_gate(block_id: BlockId) -> bool {
    block_id == interactive_blocks::OAK_FENCE_GATE
}

/// Check if fence gate is open
pub fn is_fence_gate_open(state: BlockState) -> bool {
    (state & 0x08) != 0
}

/// Set fence gate open state
pub fn set_fence_gate_open(state: BlockState, open: bool) -> BlockState {
    if open {
        state | 0x08
    } else {
        state & !0x08
    }
}

/// Check if a block is a trapdoor
pub fn is_trapdoor(block_id: BlockId) -> bool {
    block_id == interactive_blocks::TRAPDOOR
}

/// Check if trapdoor is open
pub fn is_trapdoor_open(state: BlockState) -> bool {
    (state & 0x08) != 0
}

/// Set trapdoor open state
pub fn set_trapdoor_open(state: BlockState, open: bool) -> BlockState {
    if open {
        state | 0x08
    } else {
        state & !0x08
    }
}

/// Check if a block is a ladder
pub fn is_ladder(block_id: BlockId) -> bool {
    block_id == interactive_blocks::LADDER
}

/// Check if a block is a slab
pub fn is_slab(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::STONE_SLAB | interactive_blocks::OAK_SLAB
    )
}

/// Check if a block is stairs
pub fn is_stairs(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::STONE_STAIRS | interactive_blocks::OAK_STAIRS
    )
}

/// Check if a block is a fence
pub fn is_fence(block_id: BlockId) -> bool {
    block_id == interactive_blocks::OAK_FENCE
}

/// Check if a block is part of a bed
pub fn is_bed(block_id: BlockId) -> bool {
    matches!(
        block_id,
        interactive_blocks::BED_HEAD | interactive_blocks::BED_FOOT
    )
}

/// Check if a block is a chest
pub fn is_chest(block_id: BlockId) -> bool {
    block_id == interactive_blocks::CHEST
}

/// Collision type for blocks
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollisionType {
    /// Full block collision
    Full,
    /// No collision
    None,
    /// Partial block (height 0.0-1.0)
    Partial { min_y: f32, max_y: f32 },
    /// Fence collision (1.5 blocks high for jumping)
    Fence,
    /// Door collision (depends on open state)
    Door { open: bool },
    /// Ladder collision (climbable)
    Ladder,
}

/// Get collision type for a block
pub fn get_collision_type(block_id: BlockId, state: BlockState) -> CollisionType {
    match block_id {
        blocks::AIR => CollisionType::None,
        blocks::WATER => CollisionType::None,

        id if is_door(id) => CollisionType::Door {
            open: is_door_open(state),
        },

        id if is_ladder(id) => CollisionType::Ladder,

        id if is_fence(id) => CollisionType::Fence,

        id if is_fence_gate(id) => {
            if is_fence_gate_open(state) {
                CollisionType::None
            } else {
                CollisionType::Fence
            }
        }

        id if is_trapdoor(id) => {
            if is_trapdoor_open(state) {
                CollisionType::None
            } else {
                CollisionType::Partial {
                    min_y: 0.0,
                    max_y: 0.1875,
                }
            }
        }

        id if is_slab(id) => {
            let pos = SlabPosition::from_state(state);
            match pos {
                SlabPosition::Bottom => CollisionType::Partial {
                    min_y: 0.0,
                    max_y: 0.5,
                },
                SlabPosition::Top => CollisionType::Partial {
                    min_y: 0.5,
                    max_y: 1.0,
                },
            }
        }

        id if is_stairs(id) => {
            // Simplified stairs collision
            CollisionType::Partial {
                min_y: 0.0,
                max_y: 1.0,
            }
        }

        id if is_bed(id) => CollisionType::Partial {
            min_y: 0.0,
            max_y: 0.5625,
        },

        interactive_blocks::GLASS | interactive_blocks::GLASS_PANE => CollisionType::Full,

        interactive_blocks::TORCH => CollisionType::None,

        interactive_blocks::CHEST => CollisionType::Partial {
            min_y: 0.0,
            max_y: 0.875,
        },

        _ => CollisionType::Full,
    }
}

/// Block interaction manager
pub struct InteractionManager {
    /// Dirty chunks that need mesh rebuilding
    dirty_chunks: HashSet<ChunkPos>,
}

impl InteractionManager {
    /// Create a new interaction manager
    pub fn new() -> Self {
        Self {
            dirty_chunks: HashSet::new(),
        }
    }

    /// Toggle a door at the given position
    pub fn toggle_door(
        &mut self,
        chunk_pos: ChunkPos,
        x: usize,
        y: usize,
        z: usize,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) -> bool {
        let chunk = match chunks.get(&chunk_pos) {
            Some(c) => c,
            None => return false,
        };

        let voxel = chunk.voxel(x, y, z);

        // Check if it's a door
        if !is_door(voxel.id) {
            return false;
        }

        // Iron doors can only be opened by redstone
        if is_iron_door(voxel.id) {
            return false;
        }

        let is_lower = is_door_lower(voxel.id);
        let new_open = !is_door_open(voxel.state);
        let new_state = set_door_open(voxel.state, new_open);

        // Update this block
        if let Some(chunk) = chunks.get_mut(&chunk_pos) {
            chunk.set_voxel(
                x,
                y,
                z,
                Voxel {
                    state: new_state,
                    ..voxel
                },
            );
            self.dirty_chunks.insert(chunk_pos);
        }

        // Update the other half of the door
        let other_y = if is_lower { y + 1 } else { y.saturating_sub(1) };
        if other_y < CHUNK_SIZE_Y {
            if let Some(chunk) = chunks.get_mut(&chunk_pos) {
                let other_voxel = chunk.voxel(x, other_y, z);
                if is_door(other_voxel.id) {
                    chunk.set_voxel(
                        x,
                        other_y,
                        z,
                        Voxel {
                            state: new_state,
                            ..other_voxel
                        },
                    );
                }
            }
        }

        true
    }

    /// Toggle a fence gate at the given position
    pub fn toggle_fence_gate(
        &mut self,
        chunk_pos: ChunkPos,
        x: usize,
        y: usize,
        z: usize,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) -> bool {
        let chunk = match chunks.get(&chunk_pos) {
            Some(c) => c,
            None => return false,
        };

        let voxel = chunk.voxel(x, y, z);

        if !is_fence_gate(voxel.id) {
            return false;
        }

        let new_open = !is_fence_gate_open(voxel.state);
        let new_state = set_fence_gate_open(voxel.state, new_open);

        if let Some(chunk) = chunks.get_mut(&chunk_pos) {
            chunk.set_voxel(
                x,
                y,
                z,
                Voxel {
                    state: new_state,
                    ..voxel
                },
            );
            self.dirty_chunks.insert(chunk_pos);
        }

        true
    }

    /// Toggle a trapdoor at the given position
    pub fn toggle_trapdoor(
        &mut self,
        chunk_pos: ChunkPos,
        x: usize,
        y: usize,
        z: usize,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) -> bool {
        let chunk = match chunks.get(&chunk_pos) {
            Some(c) => c,
            None => return false,
        };

        let voxel = chunk.voxel(x, y, z);

        if !is_trapdoor(voxel.id) {
            return false;
        }

        let new_open = !is_trapdoor_open(voxel.state);
        let new_state = set_trapdoor_open(voxel.state, new_open);

        if let Some(chunk) = chunks.get_mut(&chunk_pos) {
            chunk.set_voxel(
                x,
                y,
                z,
                Voxel {
                    state: new_state,
                    ..voxel
                },
            );
            self.dirty_chunks.insert(chunk_pos);
        }

        true
    }

    /// Interact with a block (right-click)
    pub fn interact(
        &mut self,
        chunk_pos: ChunkPos,
        x: usize,
        y: usize,
        z: usize,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) -> InteractionResult {
        let chunk = match chunks.get(&chunk_pos) {
            Some(c) => c,
            None => return InteractionResult::None,
        };

        let voxel = chunk.voxel(x, y, z);

        // Try various interactions
        if is_door(voxel.id)
            && !is_iron_door(voxel.id)
            && self.toggle_door(chunk_pos, x, y, z, chunks)
        {
            return InteractionResult::DoorToggled;
        }

        if is_fence_gate(voxel.id) && self.toggle_fence_gate(chunk_pos, x, y, z, chunks) {
            return InteractionResult::FenceGateToggled;
        }

        if is_trapdoor(voxel.id) && self.toggle_trapdoor(chunk_pos, x, y, z, chunks) {
            return InteractionResult::TrapdoorToggled;
        }

        if is_bed(voxel.id) {
            return InteractionResult::OpenBedUI;
        }

        if is_chest(voxel.id) {
            return InteractionResult::OpenChestUI;
        }

        InteractionResult::None
    }

    /// Take the set of dirty chunks (clears internal state)
    pub fn take_dirty_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
    }
}

impl Default for InteractionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a block interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionResult {
    None,
    DoorToggled,
    FenceGateToggled,
    TrapdoorToggled,
    OpenBedUI,
    OpenChestUI,
}

/// Bed sleep system
pub struct BedSystem {
    /// Player spawn point (set when sleeping)
    spawn_point: Option<(i32, i32, i32)>,
}

impl BedSystem {
    /// Create a new bed system
    pub fn new() -> Self {
        Self { spawn_point: None }
    }

    /// Try to sleep in a bed
    pub fn try_sleep(
        &mut self,
        bed_pos: (i32, i32, i32),
        is_night: bool,
        hostile_mobs_nearby: bool,
    ) -> SleepResult {
        if !is_night {
            return SleepResult::NotNight;
        }

        if hostile_mobs_nearby {
            return SleepResult::MonstersNearby;
        }

        // Set spawn point
        self.spawn_point = Some(bed_pos);
        SleepResult::Success
    }

    /// Get the spawn point
    pub fn spawn_point(&self) -> Option<(i32, i32, i32)> {
        self.spawn_point
    }

    /// Set spawn point manually
    pub fn set_spawn_point(&mut self, pos: (i32, i32, i32)) {
        self.spawn_point = Some(pos);
    }
}

impl Default for BedSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of attempting to sleep
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepResult {
    Success,
    NotNight,
    MonstersNearby,
    BedOccupied,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facing() {
        // Test state round-trip
        for facing in [Facing::North, Facing::South, Facing::East, Facing::West] {
            let state = facing.to_state();
            assert_eq!(Facing::from_state(state), facing);
        }
    }

    #[test]
    fn test_facing_from_yaw() {
        // Test yaw to facing conversion
        assert_eq!(Facing::from_yaw(0.0), Facing::South);
        assert_eq!(Facing::from_yaw(std::f32::consts::PI), Facing::North);
        assert_eq!(Facing::from_yaw(std::f32::consts::FRAC_PI_2), Facing::West);
        assert_eq!(
            Facing::from_yaw(3.0 * std::f32::consts::FRAC_PI_2),
            Facing::East
        );
    }

    #[test]
    fn test_door_state() {
        let state: BlockState = 0;
        assert!(!is_door_open(state));

        let open_state = set_door_open(state, true);
        assert!(is_door_open(open_state));

        let closed_state = set_door_open(open_state, false);
        assert!(!is_door_open(closed_state));
    }

    #[test]
    fn test_slab_position() {
        let state: BlockState = 0;
        assert_eq!(SlabPosition::from_state(state), SlabPosition::Bottom);

        let top_state = SlabPosition::Top.to_state(state);
        assert_eq!(SlabPosition::from_state(top_state), SlabPosition::Top);

        let bottom_state = SlabPosition::Bottom.to_state(top_state);
        assert_eq!(SlabPosition::from_state(bottom_state), SlabPosition::Bottom);
    }

    #[test]
    fn test_collision_types() {
        // Air has no collision
        assert_eq!(get_collision_type(blocks::AIR, 0), CollisionType::None);

        // Stone has full collision
        assert_eq!(get_collision_type(blocks::STONE, 0), CollisionType::Full);

        // Ladder is climbable
        assert_eq!(
            get_collision_type(interactive_blocks::LADDER, 0),
            CollisionType::Ladder
        );

        // Fence has special collision
        assert_eq!(
            get_collision_type(interactive_blocks::OAK_FENCE, 0),
            CollisionType::Fence
        );

        // Slab is partial
        let bottom_slab_collision = get_collision_type(
            interactive_blocks::STONE_SLAB,
            SlabPosition::Bottom.to_state(0),
        );
        match bottom_slab_collision {
            CollisionType::Partial { min_y, max_y } => {
                assert_eq!(min_y, 0.0);
                assert_eq!(max_y, 0.5);
            }
            _ => panic!("Expected partial collision for bottom slab"),
        }
    }

    #[test]
    fn test_is_door_functions() {
        assert!(is_door(interactive_blocks::OAK_DOOR_LOWER));
        assert!(is_door(interactive_blocks::OAK_DOOR_UPPER));
        assert!(is_door(interactive_blocks::IRON_DOOR_LOWER));
        assert!(is_door(interactive_blocks::IRON_DOOR_UPPER));
        assert!(!is_door(blocks::STONE));

        assert!(is_door_lower(interactive_blocks::OAK_DOOR_LOWER));
        assert!(!is_door_lower(interactive_blocks::OAK_DOOR_UPPER));

        assert!(is_iron_door(interactive_blocks::IRON_DOOR_LOWER));
        assert!(!is_iron_door(interactive_blocks::OAK_DOOR_LOWER));
    }

    #[test]
    fn test_bed_system() {
        let mut bed = BedSystem::new();
        assert_eq!(bed.spawn_point(), None);

        // Can't sleep during day
        let result = bed.try_sleep((10, 64, 10), false, false);
        assert_eq!(result, SleepResult::NotNight);

        // Can't sleep with monsters nearby
        let result = bed.try_sleep((10, 64, 10), true, true);
        assert_eq!(result, SleepResult::MonstersNearby);

        // Successful sleep
        let result = bed.try_sleep((10, 64, 10), true, false);
        assert_eq!(result, SleepResult::Success);
        assert_eq!(bed.spawn_point(), Some((10, 64, 10)));
    }
}
