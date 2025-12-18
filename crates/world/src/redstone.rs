//! Redstone mechanics for power transmission and powered blocks.
//!
//! Implements levers, buttons, pressure plates, redstone wire, and powered devices.

use crate::chunk::{
    BlockId, BlockState, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Block IDs for redstone components
pub mod redstone_blocks {
    use crate::chunk::BlockId;

    pub const LEVER: BlockId = 38;
    pub const STONE_BUTTON: BlockId = 39;
    pub const OAK_BUTTON: BlockId = 40;
    pub const STONE_PRESSURE_PLATE: BlockId = 41;
    pub const OAK_PRESSURE_PLATE: BlockId = 42;
    pub const REDSTONE_WIRE: BlockId = 43;
    pub const REDSTONE_TORCH: BlockId = 44;
    pub const REDSTONE_LAMP: BlockId = 45;
    pub const REDSTONE_LAMP_LIT: BlockId = 46;
}

/// Maximum redstone power level
pub const MAX_POWER: u8 = 15;

/// Type of redstone component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedstoneComponent {
    /// Lever - toggles on/off, provides power
    Lever,
    /// Button - momentary power for 20 ticks
    Button,
    /// Pressure plate - powered when entity on it
    PressurePlate,
    /// Redstone wire - transmits power with decay
    Wire,
    /// Redstone torch - provides power, inverts signal
    Torch,
    /// Redstone lamp - lights up when powered
    Lamp,
}

impl RedstoneComponent {
    /// Get component type from block ID
    pub fn from_block_id(block_id: BlockId) -> Option<Self> {
        match block_id {
            redstone_blocks::LEVER => Some(RedstoneComponent::Lever),
            redstone_blocks::STONE_BUTTON | redstone_blocks::OAK_BUTTON => {
                Some(RedstoneComponent::Button)
            }
            redstone_blocks::STONE_PRESSURE_PLATE | redstone_blocks::OAK_PRESSURE_PLATE => {
                Some(RedstoneComponent::PressurePlate)
            }
            redstone_blocks::REDSTONE_WIRE => Some(RedstoneComponent::Wire),
            redstone_blocks::REDSTONE_TORCH => Some(RedstoneComponent::Torch),
            redstone_blocks::REDSTONE_LAMP | redstone_blocks::REDSTONE_LAMP_LIT => {
                Some(RedstoneComponent::Lamp)
            }
            _ => None,
        }
    }

    /// Check if this component is a power source
    pub fn is_power_source(self) -> bool {
        matches!(
            self,
            RedstoneComponent::Lever
                | RedstoneComponent::Button
                | RedstoneComponent::PressurePlate
                | RedstoneComponent::Torch
        )
    }

    /// Check if this component conducts power
    pub fn conducts_power(self) -> bool {
        matches!(self, RedstoneComponent::Wire)
    }

    /// Check if this component can be powered
    pub fn can_be_powered(self) -> bool {
        matches!(self, RedstoneComponent::Lamp | RedstoneComponent::Wire)
    }
}

/// Get power level from block state (stored in lower 4 bits)
pub fn get_power_level(state: BlockState) -> u8 {
    (state & 0x0F) as u8
}

/// Set power level in block state
pub fn set_power_level(state: BlockState, power: u8) -> BlockState {
    (state & 0xFFF0) | (power.min(MAX_POWER) as BlockState)
}

/// Check if a redstone component is active (powered/on)
pub fn is_active(state: BlockState) -> bool {
    (state & 0x10) != 0
}

/// Set the active flag in block state
pub fn set_active(state: BlockState, active: bool) -> BlockState {
    if active {
        state | 0x10
    } else {
        state & !0x10
    }
}

/// World position for redstone updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RedstonePos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RedstonePos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Get adjacent positions (6 neighbors)
    pub fn neighbors(&self) -> [RedstonePos; 6] {
        [
            RedstonePos::new(self.x - 1, self.y, self.z),
            RedstonePos::new(self.x + 1, self.y, self.z),
            RedstonePos::new(self.x, self.y - 1, self.z),
            RedstonePos::new(self.x, self.y + 1, self.z),
            RedstonePos::new(self.x, self.y, self.z - 1),
            RedstonePos::new(self.x, self.y, self.z + 1),
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

/// Pending button deactivation
#[derive(Debug, Clone, Copy)]
struct ButtonTimer {
    pos: RedstonePos,
    deactivate_tick: u64,
}

/// Redstone simulator for power propagation
pub struct RedstoneSimulator {
    /// Pending redstone updates
    pending_updates: BTreeSet<RedstonePos>,
    /// Button timers for momentary switches
    button_timers: Vec<ButtonTimer>,
    /// Current simulation tick
    current_tick: u64,
    /// Dirty chunks that need mesh rebuilding
    dirty_chunks: std::collections::HashSet<ChunkPos>,
    /// Dirty chunks that need block-light recomputation
    dirty_light_chunks: std::collections::HashSet<ChunkPos>,
}

impl RedstoneSimulator {
    /// Create a new redstone simulator
    pub fn new() -> Self {
        Self {
            pending_updates: BTreeSet::new(),
            button_timers: Vec::new(),
            current_tick: 0,
            dirty_chunks: std::collections::HashSet::new(),
            dirty_light_chunks: std::collections::HashSet::new(),
        }
    }

    /// Schedule a redstone update at a position
    pub fn schedule_update(&mut self, pos: RedstonePos) {
        self.pending_updates.insert(pos);
    }

    /// Toggle a lever at the given position
    pub fn toggle_lever(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if voxel.id != redstone_blocks::LEVER {
            return;
        }

        let new_active = !is_active(voxel.state);
        let new_state = set_active(voxel.state, new_active);
        let new_power = if new_active { MAX_POWER } else { 0 };
        let new_state = set_power_level(new_state, new_power);

        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        // Schedule updates for neighbors
        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    /// Activate a button at the given position
    pub fn activate_button(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if !matches!(
            voxel.id,
            redstone_blocks::STONE_BUTTON | redstone_blocks::OAK_BUTTON
        ) {
            return;
        }

        // Already active?
        if is_active(voxel.state) {
            return;
        }

        let new_state = set_active(set_power_level(voxel.state, MAX_POWER), true);

        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        // Schedule deactivation in 20 ticks (1 second)
        self.button_timers.push(ButtonTimer {
            pos,
            deactivate_tick: self.current_tick + 20,
        });

        // Schedule updates for neighbors
        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    /// Update pressure plate state based on entity presence
    pub fn update_pressure_plate(
        &mut self,
        pos: RedstonePos,
        entity_present: bool,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if !matches!(
            voxel.id,
            redstone_blocks::STONE_PRESSURE_PLATE | redstone_blocks::OAK_PRESSURE_PLATE
        ) {
            return;
        }

        let was_active = is_active(voxel.state);
        if entity_present == was_active {
            return; // No change
        }

        let new_power = if entity_present { MAX_POWER } else { 0 };
        let new_state = set_active(set_power_level(voxel.state, new_power), entity_present);

        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        // Schedule updates for neighbors
        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    /// Tick the redstone simulation
    pub fn tick(&mut self, chunks: &mut HashMap<ChunkPos, Chunk>) {
        self.current_tick += 1;

        // Process button timers
        let expired_buttons: Vec<RedstonePos> = self
            .button_timers
            .iter()
            .filter(|t| t.deactivate_tick <= self.current_tick)
            .map(|t| t.pos)
            .collect();

        self.button_timers
            .retain(|t| t.deactivate_tick > self.current_tick);

        for pos in expired_buttons {
            self.deactivate_button(pos, chunks);
        }

        // Process pending updates using BFS for deterministic order
        if self.pending_updates.is_empty() {
            return;
        }

        let updates: Vec<RedstonePos> = self.pending_updates.iter().copied().collect();
        self.pending_updates.clear();
        let mut queue: VecDeque<RedstonePos> = updates.into_iter().collect();
        let mut visited: HashSet<RedstonePos> = HashSet::new();

        while let Some(pos) = queue.pop_front() {
            if visited.contains(&pos) {
                continue;
            }
            visited.insert(pos);

            if self.process_update(pos, chunks) {
                // If power changed, schedule neighbor updates
                for neighbor in pos.neighbors() {
                    if !visited.contains(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    /// Deactivate a button
    fn deactivate_button(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if !matches!(
            voxel.id,
            redstone_blocks::STONE_BUTTON | redstone_blocks::OAK_BUTTON
        ) {
            return;
        }

        let new_state = set_active(set_power_level(voxel.state, 0), false);

        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        // Schedule updates for neighbors
        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    /// Process a single redstone update, returns true if power changed
    fn process_update(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return false;
        }

        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        let component = match RedstoneComponent::from_block_id(voxel.id) {
            Some(c) => c,
            None => {
                // Check if it's a lamp that needs updating
                if voxel.id == redstone_blocks::REDSTONE_LAMP
                    || voxel.id == redstone_blocks::REDSTONE_LAMP_LIT
                {
                    return self.update_lamp(pos, chunks);
                }
                return false;
            }
        };

        match component {
            RedstoneComponent::Wire => self.update_wire(pos, chunks),
            RedstoneComponent::Lamp => self.update_lamp(pos, chunks),
            RedstoneComponent::Torch => self.update_torch(pos, chunks),
            _ => false, // Power sources don't need updating
        }
    }

    /// Update redstone wire power level
    fn update_wire(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        let old_power = get_power_level(voxel.state);

        // Find strongest power from neighbors
        let mut max_power: u8 = 0;

        for neighbor in pos.neighbors() {
            if let Some(neighbor_voxel) = self.get_voxel(neighbor, chunks) {
                let neighbor_power = self.get_emitted_power(neighbor_voxel);
                if neighbor_power > 0 {
                    // Wire loses 1 power per block
                    let received = neighbor_power.saturating_sub(1);
                    max_power = max_power.max(received);
                }
            }
        }

        if max_power == old_power {
            return false;
        }

        let new_state = set_power_level(voxel.state, max_power);
        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        true
    }

    /// Update redstone lamp state
    fn update_lamp(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        // Check if any neighbor is providing power
        let mut powered = false;
        for neighbor in pos.neighbors() {
            if let Some(neighbor_voxel) = self.get_voxel(neighbor, chunks) {
                if self.get_emitted_power(neighbor_voxel) > 0 {
                    powered = true;
                    break;
                }
            }
        }

        let is_lit = voxel.id == redstone_blocks::REDSTONE_LAMP_LIT;
        if powered == is_lit {
            return false;
        }

        let new_id = if powered {
            redstone_blocks::REDSTONE_LAMP_LIT
        } else {
            redstone_blocks::REDSTONE_LAMP
        };

        self.set_voxel(
            pos,
            Voxel {
                id: new_id,
                state: voxel.state,
                light_sky: voxel.light_sky,
                light_block: if powered { 15 } else { 0 },
            },
            chunks,
        );

        true
    }

    /// Update redstone torch (inverts power from block below)
    fn update_torch(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        // Check power from supporting block (torch inverts).
        let support_pos = if crate::is_torch_wall(voxel.state) {
            let facing = crate::torch_facing(voxel.state);
            let (dx, dz) = facing.offset();
            RedstonePos::new(pos.x - dx, pos.y, pos.z - dz)
        } else {
            RedstonePos::new(pos.x, pos.y - 1, pos.z)
        };
        let powered_from_support = if let Some(support_voxel) = self.get_voxel(support_pos, chunks)
        {
            get_power_level(support_voxel.state) > 0
        } else {
            false
        };

        // Torch is ON when NOT powered from the supporting block (inversion)
        let should_be_active = !powered_from_support;
        let was_active = is_active(voxel.state);

        if should_be_active == was_active {
            return false;
        }

        let new_power = if should_be_active { MAX_POWER } else { 0 };
        let new_state = set_active(set_power_level(voxel.state, new_power), should_be_active);

        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                light_sky: voxel.light_sky,
                light_block: if should_be_active { 7 } else { 0 },
            },
            chunks,
        );

        true
    }

    /// Get the power emitted by a voxel
    fn get_emitted_power(&self, voxel: Voxel) -> u8 {
        match RedstoneComponent::from_block_id(voxel.id) {
            Some(RedstoneComponent::Lever) => {
                if is_active(voxel.state) {
                    MAX_POWER
                } else {
                    0
                }
            }
            Some(RedstoneComponent::Button) => {
                if is_active(voxel.state) {
                    MAX_POWER
                } else {
                    0
                }
            }
            Some(RedstoneComponent::PressurePlate) => {
                if is_active(voxel.state) {
                    MAX_POWER
                } else {
                    0
                }
            }
            Some(RedstoneComponent::Wire) => get_power_level(voxel.state),
            Some(RedstoneComponent::Torch) => {
                if is_active(voxel.state) {
                    MAX_POWER
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// Get voxel at a world position
    fn get_voxel(&self, pos: RedstonePos, chunks: &HashMap<ChunkPos, Chunk>) -> Option<Voxel> {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return None;
        }

        let (chunk_pos, local_x, local_y, local_z) = pos.to_chunk_local();
        chunks
            .get(&chunk_pos)
            .map(|chunk| chunk.voxel(local_x, local_y, local_z))
    }

    /// Set voxel at a world position
    fn set_voxel(&mut self, pos: RedstonePos, voxel: Voxel, chunks: &mut HashMap<ChunkPos, Chunk>) {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return;
        }

        let (chunk_pos, local_x, local_y, local_z) = pos.to_chunk_local();
        if let Some(chunk) = chunks.get_mut(&chunk_pos) {
            let old = chunk.voxel(local_x, local_y, local_z);
            chunk.set_voxel(local_x, local_y, local_z, voxel);
            self.dirty_chunks.insert(chunk_pos);
            if old.light_block != voxel.light_block {
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
}

impl Default for RedstoneSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_level_encoding() {
        let state = set_power_level(0, 10);
        assert_eq!(get_power_level(state), 10);

        let state = set_power_level(state, 15);
        assert_eq!(get_power_level(state), 15);

        // Test max clamp
        let state = set_power_level(0, 20);
        assert_eq!(get_power_level(state), 15);
    }

    #[test]
    fn test_active_flag() {
        let state = set_active(0, true);
        assert!(is_active(state));

        let state = set_active(state, false);
        assert!(!is_active(state));

        // Test combination with power level
        let state = set_active(set_power_level(0, 12), true);
        assert!(is_active(state));
        assert_eq!(get_power_level(state), 12);
    }

    #[test]
    fn test_redstone_component_detection() {
        assert_eq!(
            RedstoneComponent::from_block_id(redstone_blocks::LEVER),
            Some(RedstoneComponent::Lever)
        );
        assert_eq!(
            RedstoneComponent::from_block_id(redstone_blocks::REDSTONE_WIRE),
            Some(RedstoneComponent::Wire)
        );
        assert_eq!(
            RedstoneComponent::from_block_id(redstone_blocks::REDSTONE_LAMP),
            Some(RedstoneComponent::Lamp)
        );
        assert_eq!(RedstoneComponent::from_block_id(0), None); // Air
    }

    #[test]
    fn test_power_source_detection() {
        assert!(RedstoneComponent::Lever.is_power_source());
        assert!(RedstoneComponent::Button.is_power_source());
        assert!(RedstoneComponent::PressurePlate.is_power_source());
        assert!(RedstoneComponent::Torch.is_power_source());
        assert!(!RedstoneComponent::Wire.is_power_source());
        assert!(!RedstoneComponent::Lamp.is_power_source());
    }

    #[test]
    fn test_redstone_pos_neighbors() {
        let pos = RedstonePos::new(5, 10, 15);
        let neighbors = pos.neighbors();
        assert_eq!(neighbors.len(), 6);

        // Check that all neighbors are adjacent
        for neighbor in &neighbors {
            let dx = (neighbor.x - pos.x).abs();
            let dy = (neighbor.y - pos.y).abs();
            let dz = (neighbor.z - pos.z).abs();
            assert_eq!(dx + dy + dz, 1);
        }
    }

    #[test]
    fn test_redstone_pos_to_chunk_local() {
        // Positive coordinates
        let pos = RedstonePos::new(17, 64, 5);
        let (chunk_pos, lx, ly, lz) = pos.to_chunk_local();
        assert_eq!(chunk_pos, ChunkPos::new(1, 0));
        assert_eq!(lx, 1);
        assert_eq!(ly, 64);
        assert_eq!(lz, 5);

        // Negative coordinates
        let pos = RedstonePos::new(-5, 32, -20);
        let (chunk_pos, lx, ly, lz) = pos.to_chunk_local();
        assert_eq!(chunk_pos, ChunkPos::new(-1, -2));
        assert_eq!(lx, 11);
        assert_eq!(ly, 32);
        assert_eq!(lz, 12);
    }

    #[test]
    fn test_conducts_and_can_be_powered() {
        assert!(RedstoneComponent::Wire.conducts_power());
        assert!(!RedstoneComponent::Lever.conducts_power());
        assert!(!RedstoneComponent::Lamp.conducts_power());

        assert!(RedstoneComponent::Lamp.can_be_powered());
        assert!(RedstoneComponent::Wire.can_be_powered());
        assert!(!RedstoneComponent::Lever.can_be_powered());
        assert!(!RedstoneComponent::Button.can_be_powered());
    }

    /// Helper to create a test chunk with air
    fn create_test_chunk() -> Chunk {
        Chunk::new(ChunkPos::new(0, 0))
    }

    #[test]
    fn test_simulator_new() {
        let sim = RedstoneSimulator::new();
        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_schedule_update() {
        let mut sim = RedstoneSimulator::new();
        let pos = RedstonePos::new(5, 64, 5);

        sim.schedule_update(pos);
        assert_eq!(sim.pending_count(), 1);

        // Scheduling same position doesn't duplicate
        sim.schedule_update(pos);
        assert_eq!(sim.pending_count(), 1);

        // Different position adds to pending
        sim.schedule_update(RedstonePos::new(6, 64, 5));
        assert_eq!(sim.pending_count(), 2);
    }

    #[test]
    fn test_toggle_lever() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place a lever
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::LEVER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);

        // Toggle lever on
        sim.toggle_lever(pos, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(is_active(voxel.state));
        assert_eq!(get_power_level(voxel.state), MAX_POWER);

        // Neighbors should be scheduled for update
        assert!(sim.pending_count() > 0);

        // Toggle lever off
        sim.toggle_lever(pos, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(!is_active(voxel.state));
        assert_eq!(get_power_level(voxel.state), 0);
    }

    #[test]
    fn test_toggle_lever_wrong_block() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place stone instead of lever
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: 1, // Stone
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);

        // Toggle should do nothing
        sim.toggle_lever(pos, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert_eq!(voxel.id, 1);
        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_activate_button() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place a stone button
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::STONE_BUTTON,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);

        // Activate button
        sim.activate_button(pos, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(is_active(voxel.state));
        assert_eq!(get_power_level(voxel.state), MAX_POWER);

        // Activating again should do nothing (already active)
        sim.activate_button(pos, &mut chunks);
        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(is_active(voxel.state));
    }

    #[test]
    fn test_button_deactivation_on_tick() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place a stone button
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::STONE_BUTTON,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);
        sim.activate_button(pos, &mut chunks);

        // Tick 21 times (button deactivates after 20 ticks)
        for _ in 0..21 {
            sim.tick(&mut chunks);
        }

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(!is_active(voxel.state));
        assert_eq!(get_power_level(voxel.state), 0);
    }

    #[test]
    fn test_update_pressure_plate() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place a stone pressure plate
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::STONE_PRESSURE_PLATE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);

        // Entity steps on plate
        sim.update_pressure_plate(pos, true, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(is_active(voxel.state));
        assert_eq!(get_power_level(voxel.state), MAX_POWER);

        // Entity steps off plate
        sim.update_pressure_plate(pos, false, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(!is_active(voxel.state));
        assert_eq!(get_power_level(voxel.state), 0);
    }

    #[test]
    fn test_redstone_wire_power_propagation() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place lever and wire next to each other
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::LEVER,
                state: set_active(set_power_level(0, MAX_POWER), true),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            64,
            5,
            Voxel {
                id: redstone_blocks::REDSTONE_WIRE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            7,
            64,
            5,
            Voxel {
                id: redstone_blocks::REDSTONE_WIRE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        // Schedule updates for wire positions
        sim.schedule_update(RedstonePos::new(6, 64, 5));
        sim.schedule_update(RedstonePos::new(7, 64, 5));

        // Process updates
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();

        // First wire should have power 14 (15 - 1)
        let wire1 = chunk.voxel(6, 64, 5);
        assert_eq!(get_power_level(wire1.state), 14);

        // Second wire should have power 13 (14 - 1)
        let wire2 = chunk.voxel(7, 64, 5);
        assert_eq!(get_power_level(wire2.state), 13);
    }

    #[test]
    fn test_redstone_lamp() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place lever and lamp next to each other
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::LEVER,
                state: set_active(set_power_level(0, MAX_POWER), true),
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            64,
            5,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        // Schedule lamp update
        sim.schedule_update(RedstonePos::new(6, 64, 5));
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let lamp = chunk.voxel(6, 64, 5);

        // Lamp should be lit
        assert_eq!(lamp.id, redstone_blocks::REDSTONE_LAMP_LIT);
        assert_eq!(lamp.light_block, 15);
    }

    #[test]
    fn test_redstone_torch_inversion() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place wire below and torch above
        chunk.set_voxel(
            5,
            63,
            5,
            Voxel {
                id: redstone_blocks::REDSTONE_WIRE,
                state: set_power_level(0, MAX_POWER), // Powered wire
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::REDSTONE_TORCH,
                state: set_active(set_power_level(0, MAX_POWER), true), // Initially on
                light_sky: 0,
                light_block: 7,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        // Schedule torch update
        sim.schedule_update(RedstonePos::new(5, 64, 5));
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let torch = chunk.voxel(5, 64, 5);

        // Torch should be off (inverted)
        assert!(!is_active(torch.state));
        assert_eq!(get_power_level(torch.state), 0);
        assert_eq!(torch.light_block, 0);
    }

    #[test]
    fn test_take_dirty_chunks() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::LEVER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.toggle_lever(RedstonePos::new(5, 64, 5), &mut chunks);

        let dirty = sim.take_dirty_chunks();
        assert!(dirty.contains(&ChunkPos::new(0, 0)));

        // Second call should return empty set
        let dirty2 = sim.take_dirty_chunks();
        assert!(dirty2.is_empty());
    }

    #[test]
    fn test_out_of_bounds_updates() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        chunks.insert(ChunkPos::new(0, 0), create_test_chunk());

        // Try to toggle lever at invalid Y
        sim.toggle_lever(RedstonePos::new(5, -1, 5), &mut chunks);
        sim.toggle_lever(RedstonePos::new(5, 256, 5), &mut chunks);

        // Should not crash, just do nothing
        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_oak_button() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place an oak button
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::OAK_BUTTON,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);
        sim.activate_button(pos, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(is_active(voxel.state));
    }

    #[test]
    fn test_oak_pressure_plate() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::OAK_PRESSURE_PLATE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let pos = RedstonePos::new(5, 64, 5);
        sim.update_pressure_plate(pos, true, &mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let voxel = chunk.voxel(5, 64, 5);
        assert!(is_active(voxel.state));
    }

    #[test]
    fn test_get_emitted_power_all_components() {
        let sim = RedstoneSimulator::new();

        // Active lever
        let lever_on = Voxel {
            id: redstone_blocks::LEVER,
            state: set_active(0, true),
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power(lever_on), MAX_POWER);

        // Inactive lever
        let lever_off = Voxel {
            id: redstone_blocks::LEVER,
            state: set_active(0, false),
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power(lever_off), 0);

        // Wire with power level 10
        let wire = Voxel {
            id: redstone_blocks::REDSTONE_WIRE,
            state: set_power_level(0, 10),
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power(wire), 10);

        // Non-redstone block
        let stone = Voxel {
            id: 1,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power(stone), 0);
    }

    #[test]
    fn test_missing_chunk_handling() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();

        // Operations on missing chunks should not panic
        sim.toggle_lever(RedstonePos::new(5, 64, 5), &mut chunks);
        sim.activate_button(RedstonePos::new(5, 64, 5), &mut chunks);
        sim.update_pressure_plate(RedstonePos::new(5, 64, 5), true, &mut chunks);

        assert_eq!(sim.pending_count(), 0);
    }

    #[test]
    fn test_lamp_turns_off_when_unpowered() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Place a lit lamp with no power source
        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP_LIT,
                state: 0,
                light_sky: 0,
                light_block: 15,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(RedstonePos::new(5, 64, 5));
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let lamp = chunk.voxel(5, 64, 5);

        // Lamp should turn off
        assert_eq!(lamp.id, redstone_blocks::REDSTONE_LAMP);
        assert_eq!(lamp.light_block, 0);
    }

    #[test]
    fn test_default_implementation() {
        let sim = RedstoneSimulator::default();
        assert_eq!(sim.pending_count(), 0);
    }
}
