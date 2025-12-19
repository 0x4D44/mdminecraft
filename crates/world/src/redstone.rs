//! Redstone mechanics for power transmission and powered blocks.
//!
//! Implements levers, buttons, pressure plates, redstone wire, and powered devices.

use crate::chunk::{
    BlockId, BlockState, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use crate::Facing;
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
    // Appended to preserve stable block IDs.
    pub const REDSTONE_REPEATER: BlockId = 123;
    pub const REDSTONE_COMPARATOR: BlockId = 124;
    pub const REDSTONE_OBSERVER: BlockId = 126;
}

/// Block IDs for piston mechanics (append-only indices in `blocks.json`).
pub mod mechanical_blocks {
    use crate::chunk::BlockId;

    pub const PISTON: BlockId = 127;
    pub const PISTON_HEAD: BlockId = 128;
    pub const DISPENSER: BlockId = 129;
    pub const DROPPER: BlockId = 130;
    pub const HOPPER: BlockId = 131;
}

/// Maximum redstone power level
pub const MAX_POWER: u8 = 15;

const PISTON_PUSH_LIMIT: usize = 12;

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
    /// Redstone repeater - directional delay element
    Repeater,
    /// Redstone comparator - directional 1-tick delay element with compare/subtract modes
    Comparator,
    /// Redstone observer - emits a short pulse when its observed block changes
    Observer,
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
            redstone_blocks::REDSTONE_REPEATER => Some(RedstoneComponent::Repeater),
            redstone_blocks::REDSTONE_COMPARATOR => Some(RedstoneComponent::Comparator),
            redstone_blocks::REDSTONE_OBSERVER => Some(RedstoneComponent::Observer),
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
        matches!(
            self,
            RedstoneComponent::Wire
                | RedstoneComponent::Repeater
                | RedstoneComponent::Comparator
                | RedstoneComponent::Observer
        )
    }

    /// Check if this component can be powered
    pub fn can_be_powered(self) -> bool {
        matches!(
            self,
            RedstoneComponent::Lamp
                | RedstoneComponent::Wire
                | RedstoneComponent::Repeater
                | RedstoneComponent::Comparator
        )
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

const REPEATER_DELAY_SHIFT: u32 = 2;
const REPEATER_DELAY_MASK: BlockState = 0x03u16 << REPEATER_DELAY_SHIFT;

/// Get the repeater delay in ticks (1-4) from block state.
pub fn repeater_delay_ticks(state: BlockState) -> u8 {
    (((state & REPEATER_DELAY_MASK) >> REPEATER_DELAY_SHIFT) as u8).min(3) + 1
}

/// Set the repeater delay in ticks (1-4) in block state.
pub fn set_repeater_delay_ticks(state: BlockState, delay_ticks: u8) -> BlockState {
    let delay = delay_ticks.clamp(1, 4) - 1;
    (state & !REPEATER_DELAY_MASK) | ((delay as BlockState) << REPEATER_DELAY_SHIFT)
}

/// Get the repeater facing from block state.
pub fn repeater_facing(state: BlockState) -> Facing {
    Facing::from_state(state)
}

/// Set the repeater facing in block state.
pub fn set_repeater_facing(state: BlockState, facing: Facing) -> BlockState {
    (state & !0x03) | facing.to_state()
}

const COMPARATOR_MODE_MASK: BlockState = 0x04;
const COMPARATOR_OUTPUT_SHIFT: u32 = 5;
const COMPARATOR_OUTPUT_MASK: BlockState = 0x0Fu16 << COMPARATOR_OUTPUT_SHIFT;

/// Check if a comparator is in subtract mode.
pub fn is_comparator_subtract_mode(state: BlockState) -> bool {
    (state & COMPARATOR_MODE_MASK) != 0
}

/// Set whether a comparator is in subtract mode.
pub fn set_comparator_subtract_mode(state: BlockState, subtract: bool) -> BlockState {
    if subtract {
        state | COMPARATOR_MODE_MASK
    } else {
        state & !COMPARATOR_MODE_MASK
    }
}

/// Get the comparator output power (0-15) from block state.
pub fn comparator_output_power(state: BlockState) -> u8 {
    ((state & COMPARATOR_OUTPUT_MASK) >> COMPARATOR_OUTPUT_SHIFT) as u8
}

/// Set the comparator output power (0-15) in block state.
pub fn set_comparator_output_power(state: BlockState, power: u8) -> BlockState {
    let clamped = power.min(MAX_POWER) as BlockState;
    (state & !COMPARATOR_OUTPUT_MASK) | (clamped << COMPARATOR_OUTPUT_SHIFT)
}

/// Get the comparator facing from block state.
pub fn comparator_facing(state: BlockState) -> Facing {
    Facing::from_state(state)
}

/// Set the comparator facing in block state.
pub fn set_comparator_facing(state: BlockState, facing: Facing) -> BlockState {
    (state & !0x03) | facing.to_state()
}

const OBSERVER_HASH_SHIFT: u32 = 5;
const OBSERVER_HASH_MASK: BlockState = 0xFFu16 << OBSERVER_HASH_SHIFT;

/// Get the observer's stored "observed block" hash.
///
/// A value of 0 means "uninitialized".
pub fn observer_observed_hash(state: BlockState) -> u8 {
    ((state & OBSERVER_HASH_MASK) >> OBSERVER_HASH_SHIFT) as u8
}

/// Set the observer's stored "observed block" hash.
pub fn set_observer_observed_hash(state: BlockState, hash: u8) -> BlockState {
    (state & !OBSERVER_HASH_MASK) | ((hash as BlockState) << OBSERVER_HASH_SHIFT)
}

/// Get the observer facing from block state (output direction).
pub fn observer_facing(state: BlockState) -> Facing {
    Facing::from_state(state)
}

/// Set the observer facing in block state (output direction).
pub fn set_observer_facing(state: BlockState, facing: Facing) -> BlockState {
    (state & !0x03) | facing.to_state()
}

/// Get piston facing from block state.
pub fn piston_facing(state: BlockState) -> Facing {
    Facing::from_state(state)
}

/// Set piston facing in block state.
pub fn set_piston_facing(state: BlockState, facing: Facing) -> BlockState {
    (state & !0x03) | facing.to_state()
}

const CONTAINER_FACING_SHIFT: u32 = 5;
const CONTAINER_FACING_MASK: BlockState = 0x03u16 << CONTAINER_FACING_SHIFT;

/// Get the facing for container-style blocks (hoppers, droppers, dispensers).
pub fn container_facing(state: BlockState) -> Facing {
    Facing::from_state((state & CONTAINER_FACING_MASK) >> CONTAINER_FACING_SHIFT)
}

/// Set the facing for container-style blocks (hoppers, droppers, dispensers).
pub fn set_container_facing(state: BlockState, facing: Facing) -> BlockState {
    (state & !CONTAINER_FACING_MASK) | (facing.to_state() << CONTAINER_FACING_SHIFT)
}

pub fn hopper_facing(state: BlockState) -> Facing {
    container_facing(state)
}

pub fn set_hopper_facing(state: BlockState, facing: Facing) -> BlockState {
    set_container_facing(state, facing)
}

pub fn dispenser_facing(state: BlockState) -> Facing {
    container_facing(state)
}

pub fn set_dispenser_facing(state: BlockState, facing: Facing) -> BlockState {
    set_container_facing(state, facing)
}

pub fn dropper_facing(state: BlockState) -> Facing {
    container_facing(state)
}

pub fn set_dropper_facing(state: BlockState, facing: Facing) -> BlockState {
    set_container_facing(state, facing)
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

#[derive(Debug, Clone, Copy)]
struct RepeaterTimer {
    pos: RedstonePos,
    apply_tick: u64,
    desired_active: bool,
}

#[derive(Debug, Clone, Copy)]
struct ComparatorTimer {
    pos: RedstonePos,
    apply_tick: u64,
    desired_power: u8,
}

#[derive(Debug, Clone, Copy)]
struct ObserverTimer {
    pos: RedstonePos,
    apply_tick: u64,
    desired_active: bool,
}

#[derive(Debug, Clone, Copy)]
struct PistonTimer {
    pos: RedstonePos,
    apply_tick: u64,
    desired_extended: bool,
}

/// Redstone simulator for power propagation
pub struct RedstoneSimulator {
    /// Pending redstone updates
    pending_updates: BTreeSet<RedstonePos>,
    /// Button timers for momentary switches
    button_timers: Vec<ButtonTimer>,
    repeater_timers: Vec<RepeaterTimer>,
    comparator_timers: Vec<ComparatorTimer>,
    observer_timers: Vec<ObserverTimer>,
    piston_timers: Vec<PistonTimer>,
    /// Current simulation tick
    current_tick: u64,
    /// Dirty chunks that need mesh rebuilding
    dirty_chunks: std::collections::HashSet<ChunkPos>,
    /// Dirty chunks that need block-light recomputation
    dirty_light_chunks: std::collections::HashSet<ChunkPos>,
    /// Dirty chunks that need skylight + blocklight recomputation due to block-ID (geometry) changes.
    dirty_geometry_chunks: std::collections::HashSet<ChunkPos>,
    /// Positions where block-ID (geometry) changes occurred (used for waking other sims).
    dirty_geometry_positions: BTreeSet<RedstonePos>,
}

impl RedstoneSimulator {
    /// Create a new redstone simulator
    pub fn new() -> Self {
        Self {
            pending_updates: BTreeSet::new(),
            button_timers: Vec::new(),
            repeater_timers: Vec::new(),
            comparator_timers: Vec::new(),
            observer_timers: Vec::new(),
            piston_timers: Vec::new(),
            current_tick: 0,
            dirty_chunks: std::collections::HashSet::new(),
            dirty_light_chunks: std::collections::HashSet::new(),
            dirty_geometry_chunks: std::collections::HashSet::new(),
            dirty_geometry_positions: BTreeSet::new(),
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

        // Process repeater timers.
        let expired_repeaters: Vec<(RedstonePos, bool)> = self
            .repeater_timers
            .iter()
            .filter(|t| t.apply_tick <= self.current_tick)
            .map(|t| (t.pos, t.desired_active))
            .collect();

        self.repeater_timers
            .retain(|t| t.apply_tick > self.current_tick);

        for (pos, desired_active) in expired_repeaters {
            self.apply_repeater_output(pos, desired_active, chunks);
        }

        // Process comparator timers.
        let expired_comparators: Vec<(RedstonePos, u8)> = self
            .comparator_timers
            .iter()
            .filter(|t| t.apply_tick <= self.current_tick)
            .map(|t| (t.pos, t.desired_power))
            .collect();

        self.comparator_timers
            .retain(|t| t.apply_tick > self.current_tick);

        for (pos, desired_power) in expired_comparators {
            self.apply_comparator_output(pos, desired_power, chunks);
        }

        // Process observer timers.
        let expired_observers: Vec<(RedstonePos, bool)> = self
            .observer_timers
            .iter()
            .filter(|t| t.apply_tick <= self.current_tick)
            .map(|t| (t.pos, t.desired_active))
            .collect();

        self.observer_timers
            .retain(|t| t.apply_tick > self.current_tick);

        for (pos, desired_active) in expired_observers {
            self.apply_observer_output(pos, desired_active, chunks);
        }

        // Process piston timers.
        let expired_pistons: Vec<(RedstonePos, bool)> = self
            .piston_timers
            .iter()
            .filter(|t| t.apply_tick <= self.current_tick)
            .map(|t| (t.pos, t.desired_extended))
            .collect();

        self.piston_timers
            .retain(|t| t.apply_tick > self.current_tick);

        for (pos, desired_extended) in expired_pistons {
            self.apply_piston_state(pos, desired_extended, chunks);
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
                if crate::is_door(voxel.id) {
                    return self.update_door(pos, chunks);
                }
                if crate::is_trapdoor(voxel.id) {
                    return self.update_trapdoor(pos, chunks);
                }
                if crate::is_fence_gate(voxel.id) {
                    return self.update_fence_gate(pos, chunks);
                }
                if voxel.id == mechanical_blocks::PISTON {
                    return self.update_piston(pos, chunks);
                }
                if voxel.id == mechanical_blocks::HOPPER {
                    self.update_hopper_powered_state(pos, chunks);
                    return false;
                }

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
            RedstoneComponent::Repeater => self.update_repeater(pos, chunks),
            RedstoneComponent::Comparator => self.update_comparator(pos, chunks),
            RedstoneComponent::Observer => self.update_observer(pos, chunks),
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
                let neighbor_power = self.get_emitted_power_towards(neighbor, neighbor_voxel, pos);
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

    fn is_powered_by_neighbors(&self, pos: RedstonePos, chunks: &HashMap<ChunkPos, Chunk>) -> bool {
        for neighbor in pos.neighbors() {
            if let Some(neighbor_voxel) = self.get_voxel(neighbor, chunks) {
                if self.get_emitted_power_towards(neighbor, neighbor_voxel, pos) > 0 {
                    return true;
                }
            }
        }

        false
    }

    /// Update redstone lamp state
    fn update_lamp(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        let powered = self.is_powered_by_neighbors(pos, chunks);

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

    fn update_door(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if !crate::is_door(voxel.id) {
            return false;
        }

        let lower_pos = if crate::is_door_upper(voxel.id) {
            RedstonePos::new(pos.x, pos.y - 1, pos.z)
        } else {
            pos
        };
        let upper_pos = RedstonePos::new(lower_pos.x, lower_pos.y + 1, lower_pos.z);

        let lower_voxel = match self.get_voxel(lower_pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if !crate::is_door(lower_voxel.id) {
            return false;
        }

        let powered = self.is_powered_by_neighbors(lower_pos, chunks)
            || self.is_powered_by_neighbors(upper_pos, chunks);
        let was_powered = crate::is_redstone_powered(lower_voxel.state);
        if powered == was_powered {
            return false;
        }

        let new_state = crate::set_door_open(
            crate::set_redstone_powered(lower_voxel.state, powered),
            powered,
        );

        self.set_voxel(
            lower_pos,
            Voxel {
                state: new_state,
                ..lower_voxel
            },
            chunks,
        );

        if let Some(upper_voxel) = self.get_voxel(upper_pos, chunks) {
            if crate::is_door(upper_voxel.id) {
                self.set_voxel(
                    upper_pos,
                    Voxel {
                        state: new_state,
                        ..upper_voxel
                    },
                    chunks,
                );
            }
        }

        true
    }

    fn update_trapdoor(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if !crate::is_trapdoor(voxel.id) {
            return false;
        }

        let powered = self.is_powered_by_neighbors(pos, chunks);
        let was_powered = crate::is_redstone_powered(voxel.state);
        if powered == was_powered {
            return false;
        }

        let new_state =
            crate::set_trapdoor_open(crate::set_redstone_powered(voxel.state, powered), powered);

        self.set_voxel(
            pos,
            Voxel {
                state: new_state,
                ..voxel
            },
            chunks,
        );

        true
    }

    fn update_fence_gate(
        &mut self,
        pos: RedstonePos,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if !crate::is_fence_gate(voxel.id) {
            return false;
        }

        let powered = self.is_powered_by_neighbors(pos, chunks);
        let was_powered = crate::is_redstone_powered(voxel.state);
        if powered == was_powered {
            return false;
        }

        let new_state =
            crate::set_fence_gate_open(crate::set_redstone_powered(voxel.state, powered), powered);

        self.set_voxel(
            pos,
            Voxel {
                state: new_state,
                ..voxel
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

    fn update_repeater(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if voxel.id != redstone_blocks::REDSTONE_REPEATER {
            return false;
        }

        let facing = repeater_facing(voxel.state);
        let (dx, dz) = facing.offset();
        let input_pos = RedstonePos::new(pos.x - dx, pos.y, pos.z - dz);

        let input_power = if let Some(input_voxel) = self.get_voxel(input_pos, chunks) {
            self.get_emitted_power_towards(input_pos, input_voxel, pos)
        } else {
            0
        };
        let desired_active = input_power > 0;
        let was_active = is_active(voxel.state);

        if desired_active == was_active {
            self.repeater_timers.retain(|timer| timer.pos != pos);
            return false;
        }

        let delay_ticks = repeater_delay_ticks(voxel.state) as u64;
        let apply_tick = self.current_tick.saturating_add(delay_ticks.max(1));

        self.repeater_timers.retain(|timer| timer.pos != pos);
        self.repeater_timers.push(RepeaterTimer {
            pos,
            apply_tick,
            desired_active,
        });

        false
    }

    fn update_comparator(
        &mut self,
        pos: RedstonePos,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if voxel.id != redstone_blocks::REDSTONE_COMPARATOR {
            return false;
        }

        let facing = comparator_facing(voxel.state);
        let (dx, dz) = facing.offset();
        let rear_pos = RedstonePos::new(pos.x - dx, pos.y, pos.z - dz);

        let left_pos = RedstonePos::new(pos.x + dz, pos.y, pos.z - dx);
        let right_pos = RedstonePos::new(pos.x - dz, pos.y, pos.z + dx);

        let rear_power = if let Some(rear_voxel) = self.get_voxel(rear_pos, chunks) {
            if crate::is_chest(rear_voxel.id)
                || rear_voxel.id == mechanical_blocks::HOPPER
                || matches!(
                    rear_voxel.id,
                    mechanical_blocks::DISPENSER | mechanical_blocks::DROPPER
                )
            {
                // Vanilla-ish container read: the container's comparator output is cached in the
                // low 4 bits by the gameplay layer.
                get_power_level(rear_voxel.state)
            } else {
                self.get_emitted_power_towards(rear_pos, rear_voxel, pos)
            }
        } else {
            0
        };

        let mut side_power_max = 0u8;
        for side_pos in [left_pos, right_pos] {
            if let Some(side_voxel) = self.get_voxel(side_pos, chunks) {
                side_power_max =
                    side_power_max.max(self.get_emitted_power_towards(side_pos, side_voxel, pos));
            }
        }

        let subtract = is_comparator_subtract_mode(voxel.state);
        let desired_power = if subtract {
            rear_power.saturating_sub(side_power_max)
        } else if rear_power >= side_power_max {
            rear_power
        } else {
            0
        };

        let was_power = comparator_output_power(voxel.state);
        if desired_power == was_power {
            self.comparator_timers.retain(|timer| timer.pos != pos);
            return false;
        }

        let apply_tick = self.current_tick.saturating_add(1);
        self.comparator_timers.retain(|timer| timer.pos != pos);
        self.comparator_timers.push(ComparatorTimer {
            pos,
            apply_tick,
            desired_power,
        });

        false
    }

    fn observed_voxel_hash(voxel: Option<Voxel>) -> u8 {
        let (id, state) = match voxel {
            Some(v) => (v.id as u32, v.state as u32),
            None => (0, 0),
        };
        let mut h = id.wrapping_mul(31) ^ state.wrapping_mul(131);
        h ^= h >> 16;
        (h as u8) | 1
    }

    fn update_observer(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if voxel.id != redstone_blocks::REDSTONE_OBSERVER {
            return false;
        }

        let facing = observer_facing(voxel.state);
        let (dx, dz) = facing.offset();
        let observed_pos = RedstonePos::new(pos.x - dx, pos.y, pos.z - dz);
        let observed_voxel = self.get_voxel(observed_pos, chunks);
        let new_hash = Self::observed_voxel_hash(observed_voxel);

        let stored_hash = observer_observed_hash(voxel.state);
        if stored_hash == 0 {
            let new_state = set_observer_observed_hash(voxel.state, new_hash);
            self.set_voxel(
                pos,
                Voxel {
                    id: voxel.id,
                    state: new_state,
                    ..voxel
                },
                chunks,
            );
            return false;
        }

        if stored_hash == new_hash {
            return false;
        }

        let new_state = set_observer_observed_hash(voxel.state, new_hash);
        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        // Schedule a 2-tick pulse with a 1-tick delay (vanilla-ish).
        let on_tick = self.current_tick.saturating_add(1);
        let off_tick = on_tick.saturating_add(2);
        self.observer_timers.retain(|timer| timer.pos != pos);
        self.observer_timers.push(ObserverTimer {
            pos,
            apply_tick: on_tick,
            desired_active: true,
        });
        self.observer_timers.push(ObserverTimer {
            pos,
            apply_tick: off_tick,
            desired_active: false,
        });

        false
    }

    fn is_piston_replaceable(block_id: BlockId) -> bool {
        block_id == crate::BLOCK_AIR || crate::is_fluid(block_id)
    }

    fn is_piston_pushable(block_id: BlockId, state: BlockState) -> bool {
        if Self::is_piston_replaceable(block_id) {
            return false;
        }

        if matches!(
            block_id,
            crate::BLOCK_BEDROCK | crate::BLOCK_OBSIDIAN | mechanical_blocks::PISTON
        ) {
            return false;
        }

        if block_id == mechanical_blocks::PISTON_HEAD {
            return false;
        }

        if matches!(
            block_id,
            mechanical_blocks::DISPENSER | mechanical_blocks::DROPPER | mechanical_blocks::HOPPER
        ) {
            return false;
        }

        if matches!(block_id, crate::BLOCK_FURNACE | crate::BLOCK_FURNACE_LIT)
            || matches!(
                block_id,
                crate::BLOCK_BREWING_STAND | crate::BLOCK_ENCHANTING_TABLE
            )
            || crate::is_chest(block_id)
        {
            return false;
        }

        if crate::is_door(block_id) || crate::is_bed(block_id) {
            return false;
        }

        if crate::CropType::is_crop(block_id) || block_id == crate::BLOCK_SUGAR_CANE {
            return false;
        }

        matches!(
            crate::get_collision_type(block_id, state),
            crate::CollisionType::Full
        )
    }

    fn update_piston(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) -> bool {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return false,
        };

        if voxel.id != mechanical_blocks::PISTON {
            return false;
        }

        let desired_extended = self.is_powered_by_neighbors(pos, chunks);
        let was_extended = is_active(voxel.state);

        if desired_extended == was_extended {
            self.piston_timers.retain(|timer| timer.pos != pos);
            return false;
        }

        let apply_tick = self.current_tick.saturating_add(1);
        self.piston_timers.retain(|timer| timer.pos != pos);
        self.piston_timers.push(PistonTimer {
            pos,
            apply_tick,
            desired_extended,
        });

        false
    }

    fn update_hopper_powered_state(&mut self, pos: RedstonePos, chunks: &mut HashMap<ChunkPos, Chunk>) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if voxel.id != mechanical_blocks::HOPPER {
            return;
        }

        let powered = self.is_powered_by_neighbors(pos, chunks);
        if is_active(voxel.state) == powered {
            return;
        }

        let new_state = set_active(voxel.state, powered);
        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );
    }

    fn set_voxel_and_mark_geometry(
        &mut self,
        pos: RedstonePos,
        voxel: Voxel,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        if pos.y < 0 || pos.y >= CHUNK_SIZE_Y as i32 {
            return;
        }

        let (chunk_pos, local_x, local_y, local_z) = pos.to_chunk_local();
        let Some(chunk) = chunks.get_mut(&chunk_pos) else {
            return;
        };

        chunk.set_voxel(local_x, local_y, local_z, voxel);
        self.dirty_chunks.insert(chunk_pos);
        self.dirty_geometry_chunks.insert(chunk_pos);
        self.dirty_geometry_positions.insert(pos);
    }

    fn apply_piston_state(
        &mut self,
        pos: RedstonePos,
        desired_extended: bool,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if voxel.id != mechanical_blocks::PISTON {
            return;
        }

        let was_extended = is_active(voxel.state);
        if was_extended == desired_extended {
            return;
        }

        let facing = piston_facing(voxel.state);
        let (dx, dz) = facing.offset();
        let head_pos = RedstonePos::new(pos.x + dx, pos.y, pos.z + dz);

        let mut changed_positions: Vec<RedstonePos> = Vec::new();
        changed_positions.push(pos);
        changed_positions.push(head_pos);

        if desired_extended {
            // Scan for a space within push limit.
            let mut cursor = head_pos;
            let mut line: Vec<(RedstonePos, Voxel)> = Vec::new();

            loop {
                let Some(found) = self.get_voxel(cursor, chunks) else {
                    return;
                };

                if Self::is_piston_replaceable(found.id) {
                    break;
                }

                if line.len() >= PISTON_PUSH_LIMIT {
                    return;
                }

                if !Self::is_piston_pushable(found.id, found.state) {
                    return;
                }

                line.push((cursor, found));
                cursor = RedstonePos::new(cursor.x + dx, cursor.y, cursor.z + dz);
            }

            // Push blocks from farthest to nearest.
            for (from_pos, from_voxel) in line.iter().rev() {
                let dest_pos = RedstonePos::new(from_pos.x + dx, from_pos.y, from_pos.z + dz);
                self.set_voxel_and_mark_geometry(
                    dest_pos,
                    Voxel {
                        id: from_voxel.id,
                        state: from_voxel.state,
                        light_sky: 0,
                        light_block: 0,
                    },
                    chunks,
                );
                changed_positions.push(dest_pos);
            }

            // Place piston head.
            let mut head_state = 0;
            head_state = set_piston_facing(head_state, facing);
            self.set_voxel_and_mark_geometry(
                head_pos,
                Voxel {
                    id: mechanical_blocks::PISTON_HEAD,
                    state: head_state,
                    light_sky: 0,
                    light_block: 0,
                },
                chunks,
            );

            // Mark base as extended.
            let new_state = set_active(voxel.state, true);
            self.set_voxel(
                pos,
                Voxel {
                    id: voxel.id,
                    state: new_state,
                    ..voxel
                },
                chunks,
            );
        } else {
            // Retract: remove head if present.
            if let Some(head_voxel) = self.get_voxel(head_pos, chunks) {
                if head_voxel.id == mechanical_blocks::PISTON_HEAD {
                    self.set_voxel_and_mark_geometry(head_pos, Voxel::default(), chunks);
                }
            }

            let new_state = set_active(voxel.state, false);
            self.set_voxel(
                pos,
                Voxel {
                    id: voxel.id,
                    state: new_state,
                    ..voxel
                },
                chunks,
            );
        }

        // Schedule updates around all changed positions.
        for changed in changed_positions {
            self.schedule_update(changed);
            for neighbor in changed.neighbors() {
                self.schedule_update(neighbor);
            }
        }
    }

    fn apply_repeater_output(
        &mut self,
        pos: RedstonePos,
        desired_active: bool,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if voxel.id != redstone_blocks::REDSTONE_REPEATER {
            return;
        }

        if is_active(voxel.state) == desired_active {
            return;
        }

        let new_state = set_active(voxel.state, desired_active);
        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        // Schedule neighbor updates when output changes.
        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    fn apply_comparator_output(
        &mut self,
        pos: RedstonePos,
        desired_power: u8,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if voxel.id != redstone_blocks::REDSTONE_COMPARATOR {
            return;
        }

        if comparator_output_power(voxel.state) == desired_power {
            return;
        }

        let mut new_state = set_comparator_output_power(voxel.state, desired_power);
        new_state = set_active(new_state, desired_power > 0);
        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    fn apply_observer_output(
        &mut self,
        pos: RedstonePos,
        desired_active: bool,
        chunks: &mut HashMap<ChunkPos, Chunk>,
    ) {
        let voxel = match self.get_voxel(pos, chunks) {
            Some(v) => v,
            None => return,
        };

        if voxel.id != redstone_blocks::REDSTONE_OBSERVER {
            return;
        }

        if is_active(voxel.state) == desired_active {
            return;
        }

        let new_state = set_active(voxel.state, desired_active);
        self.set_voxel(
            pos,
            Voxel {
                id: voxel.id,
                state: new_state,
                ..voxel
            },
            chunks,
        );

        for neighbor in pos.neighbors() {
            self.schedule_update(neighbor);
        }
    }

    /// Get the power emitted by a voxel
    fn get_emitted_power_towards(&self, from: RedstonePos, voxel: Voxel, to: RedstonePos) -> u8 {
        let direction = (
            (to.x - from.x).clamp(-1, 1),
            (to.y - from.y).clamp(-1, 1),
            (to.z - from.z).clamp(-1, 1),
        );

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
            Some(RedstoneComponent::Repeater) => {
                if !is_active(voxel.state) {
                    return 0;
                }

                let facing = repeater_facing(voxel.state);
                let (dx, dz) = facing.offset();
                let front_dir = (dx, 0, dz);
                if direction == front_dir {
                    MAX_POWER
                } else {
                    0
                }
            }
            Some(RedstoneComponent::Comparator) => {
                let output_power = comparator_output_power(voxel.state);
                if output_power == 0 {
                    return 0;
                }

                let facing = comparator_facing(voxel.state);
                let (dx, dz) = facing.offset();
                let front_dir = (dx, 0, dz);
                if direction == front_dir {
                    output_power
                } else {
                    0
                }
            }
            Some(RedstoneComponent::Observer) => {
                if !is_active(voxel.state) {
                    return 0;
                }

                let facing = observer_facing(voxel.state);
                let (dx, dz) = facing.offset();
                let front_dir = (dx, 0, dz);
                if direction == front_dir {
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

    /// Take the set of chunks that need skylight + blocklight recomputation due to geometry changes.
    pub fn take_dirty_geometry_chunks(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty_geometry_chunks)
    }

    /// Take the set of positions where geometry changes occurred.
    pub fn take_dirty_geometry_positions(&mut self) -> BTreeSet<RedstonePos> {
        std::mem::take(&mut self.dirty_geometry_positions)
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
        assert_eq!(
            RedstoneComponent::from_block_id(redstone_blocks::REDSTONE_REPEATER),
            Some(RedstoneComponent::Repeater)
        );
        assert_eq!(
            RedstoneComponent::from_block_id(redstone_blocks::REDSTONE_COMPARATOR),
            Some(RedstoneComponent::Comparator)
        );
        assert_eq!(
            RedstoneComponent::from_block_id(redstone_blocks::REDSTONE_OBSERVER),
            Some(RedstoneComponent::Observer)
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
        assert!(!RedstoneComponent::Repeater.is_power_source());
        assert!(!RedstoneComponent::Comparator.is_power_source());
        assert!(!RedstoneComponent::Observer.is_power_source());
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
        assert!(RedstoneComponent::Repeater.conducts_power());
        assert!(RedstoneComponent::Comparator.conducts_power());
        assert!(RedstoneComponent::Observer.conducts_power());
        assert!(!RedstoneComponent::Lever.conducts_power());
        assert!(!RedstoneComponent::Lamp.conducts_power());

        assert!(RedstoneComponent::Lamp.can_be_powered());
        assert!(RedstoneComponent::Wire.can_be_powered());
        assert!(RedstoneComponent::Repeater.can_be_powered());
        assert!(RedstoneComponent::Comparator.can_be_powered());
        assert!(!RedstoneComponent::Observer.can_be_powered());
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
    fn test_redstone_powers_iron_door() {
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
        chunk.set_voxel(
            6,
            64,
            5,
            Voxel {
                id: crate::interactive_blocks::IRON_DOOR_LOWER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            6,
            65,
            5,
            Voxel {
                id: crate::interactive_blocks::IRON_DOOR_UPPER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let lever_pos = RedstonePos::new(5, 64, 5);

        sim.toggle_lever(lever_pos, &mut chunks);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let lower = chunk.voxel(6, 64, 5);
        let upper = chunk.voxel(6, 65, 5);
        assert!(crate::is_door_open(lower.state));
        assert!(crate::is_door_open(upper.state));
        assert!(crate::is_redstone_powered(lower.state));
        assert!(crate::is_redstone_powered(upper.state));

        sim.toggle_lever(lever_pos, &mut chunks);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let lower = chunk.voxel(6, 64, 5);
        let upper = chunk.voxel(6, 65, 5);
        assert!(!crate::is_door_open(lower.state));
        assert!(!crate::is_door_open(upper.state));
        assert!(!crate::is_redstone_powered(lower.state));
        assert!(!crate::is_redstone_powered(upper.state));
    }

    #[test]
    fn test_redstone_does_not_override_manual_door_open_when_unpowered() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        let open_unpowered = crate::set_door_open(0, true);

        chunk.set_voxel(
            5,
            64,
            5,
            Voxel {
                id: crate::interactive_blocks::OAK_DOOR_LOWER,
                state: open_unpowered,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            65,
            5,
            Voxel {
                id: crate::interactive_blocks::OAK_DOOR_UPPER,
                state: open_unpowered,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.schedule_update(RedstonePos::new(5, 64, 5));
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let lower = chunk.voxel(5, 64, 5);
        let upper = chunk.voxel(5, 65, 5);
        assert!(crate::is_door_open(lower.state));
        assert!(crate::is_door_open(upper.state));
        assert!(!crate::is_redstone_powered(lower.state));
        assert!(!crate::is_redstone_powered(upper.state));
    }

    #[test]
    fn test_redstone_powers_trapdoor_and_fence_gate() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        // Lever adjacent to both the trapdoor and the gate.
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
        chunk.set_voxel(
            6,
            64,
            5,
            Voxel {
                id: crate::interactive_blocks::TRAPDOOR,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            5,
            64,
            6,
            Voxel {
                id: crate::interactive_blocks::OAK_FENCE_GATE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunks.insert(ChunkPos::new(0, 0), chunk);

        let lever_pos = RedstonePos::new(5, 64, 5);

        sim.toggle_lever(lever_pos, &mut chunks);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let trapdoor = chunk.voxel(6, 64, 5);
        let gate = chunk.voxel(5, 64, 6);
        assert!(crate::is_trapdoor_open(trapdoor.state));
        assert!(crate::is_fence_gate_open(gate.state));
        assert!(crate::is_redstone_powered(trapdoor.state));
        assert!(crate::is_redstone_powered(gate.state));

        sim.toggle_lever(lever_pos, &mut chunks);
        sim.tick(&mut chunks);

        let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
        let trapdoor = chunk.voxel(6, 64, 5);
        let gate = chunk.voxel(5, 64, 6);
        assert!(!crate::is_trapdoor_open(trapdoor.state));
        assert!(!crate::is_fence_gate_open(gate.state));
        assert!(!crate::is_redstone_powered(trapdoor.state));
        assert!(!crate::is_redstone_powered(gate.state));
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
        let from = RedstonePos::new(0, 0, 0);
        let to = RedstonePos::new(1, 0, 0);

        // Active lever
        let lever_on = Voxel {
            id: redstone_blocks::LEVER,
            state: set_active(0, true),
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power_towards(from, lever_on, to), MAX_POWER);

        // Inactive lever
        let lever_off = Voxel {
            id: redstone_blocks::LEVER,
            state: set_active(0, false),
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power_towards(from, lever_off, to), 0);

        // Wire with power level 10
        let wire = Voxel {
            id: redstone_blocks::REDSTONE_WIRE,
            state: set_power_level(0, 10),
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power_towards(from, wire, to), 10);

        // Non-redstone block
        let stone = Voxel {
            id: 1,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(sim.get_emitted_power_towards(from, stone, to), 0);

        // Active repeater emits only out its front.
        let mut repeater_state = 0;
        repeater_state = set_repeater_facing(repeater_state, Facing::East);
        repeater_state = set_repeater_delay_ticks(repeater_state, 1);
        repeater_state = set_active(repeater_state, true);
        let repeater = Voxel {
            id: redstone_blocks::REDSTONE_REPEATER,
            state: repeater_state,
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(
            sim.get_emitted_power_towards(
                RedstonePos::new(0, 0, 0),
                repeater,
                RedstonePos::new(1, 0, 0)
            ),
            MAX_POWER
        );
        assert_eq!(
            sim.get_emitted_power_towards(
                RedstonePos::new(0, 0, 0),
                repeater,
                RedstonePos::new(-1, 0, 0)
            ),
            0
        );

        // Comparator emits only out its front, at its stored output strength.
        let mut comparator_state = 0;
        comparator_state = set_comparator_facing(comparator_state, Facing::East);
        comparator_state = set_comparator_subtract_mode(comparator_state, false);
        comparator_state = set_comparator_output_power(comparator_state, 7);
        let comparator = Voxel {
            id: redstone_blocks::REDSTONE_COMPARATOR,
            state: comparator_state,
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(
            sim.get_emitted_power_towards(
                RedstonePos::new(0, 0, 0),
                comparator,
                RedstonePos::new(1, 0, 0)
            ),
            7
        );
        assert_eq!(
            sim.get_emitted_power_towards(
                RedstonePos::new(0, 0, 0),
                comparator,
                RedstonePos::new(0, 0, 1)
            ),
            0
        );

        // Observer emits only out its front while active.
        let mut observer_state = 0;
        observer_state = set_observer_facing(observer_state, Facing::East);
        observer_state = set_active(observer_state, true);
        let observer = Voxel {
            id: redstone_blocks::REDSTONE_OBSERVER,
            state: observer_state,
            light_sky: 0,
            light_block: 0,
        };
        assert_eq!(
            sim.get_emitted_power_towards(
                RedstonePos::new(0, 0, 0),
                observer,
                RedstonePos::new(1, 0, 0)
            ),
            MAX_POWER
        );
        assert_eq!(
            sim.get_emitted_power_towards(
                RedstonePos::new(0, 0, 0),
                observer,
                RedstonePos::new(0, 0, 1)
            ),
            0
        );
    }

    #[test]
    fn repeater_delay_ticks_roundtrips() {
        let facing = Facing::North;
        for delay in 1..=4 {
            let mut state = 0;
            state = set_repeater_facing(state, facing);
            state = set_repeater_delay_ticks(state, delay);
            assert_eq!(repeater_facing(state), facing);
            assert_eq!(repeater_delay_ticks(state), delay);
        }
    }

    #[test]
    fn repeater_delays_output_and_is_directional() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        let lever_pos = RedstonePos::new(5, 64, 5);
        let repeater_pos = RedstonePos::new(6, 64, 5);
        let lamp_front_pos = RedstonePos::new(7, 64, 5);
        let lamp_side_pos = RedstonePos::new(6, 64, 6);

        chunk.set_voxel(
            lever_pos.x as usize,
            lever_pos.y as usize,
            lever_pos.z as usize,
            Voxel {
                id: redstone_blocks::LEVER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mut repeater_state = 0;
        repeater_state = set_repeater_facing(repeater_state, Facing::East);
        repeater_state = set_repeater_delay_ticks(repeater_state, 2);
        chunk.set_voxel(
            repeater_pos.x as usize,
            repeater_pos.y as usize,
            repeater_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_REPEATER,
                state: repeater_state,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            lamp_front_pos.x as usize,
            lamp_front_pos.y as usize,
            lamp_front_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            lamp_side_pos.x as usize,
            lamp_side_pos.y as usize,
            lamp_side_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        // Power the lever; this schedules neighbor updates (including the repeater).
        sim.toggle_lever(lever_pos, &mut chunks);

        // Tick 1: repeater sees input but output hasn't fired yet.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(!is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Tick 2: still waiting.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(!is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Tick 3: output applies and powers only the front lamp.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP_LIT);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }
    }

    #[test]
    fn comparator_compare_and_subtract_output_is_delayed_and_directional() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        let rear_lever_pos = RedstonePos::new(5, 64, 5);
        let side_lever_pos = RedstonePos::new(6, 64, 3);
        let side_wire_pos = RedstonePos::new(6, 64, 4);
        let comparator_pos = RedstonePos::new(6, 64, 5);
        let front_lamp_pos = RedstonePos::new(7, 64, 5);
        let side_lamp_pos = RedstonePos::new(6, 64, 6);

        chunk.set_voxel(
            rear_lever_pos.x as usize,
            rear_lever_pos.y as usize,
            rear_lever_pos.z as usize,
            Voxel {
                id: redstone_blocks::LEVER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            side_lever_pos.x as usize,
            side_lever_pos.y as usize,
            side_lever_pos.z as usize,
            Voxel {
                id: redstone_blocks::LEVER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            side_wire_pos.x as usize,
            side_wire_pos.y as usize,
            side_wire_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_WIRE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mut comparator_state = 0;
        comparator_state = set_comparator_facing(comparator_state, Facing::East);
        comparator_state = set_comparator_subtract_mode(comparator_state, false);
        comparator_state = set_comparator_output_power(comparator_state, 0);
        chunk.set_voxel(
            comparator_pos.x as usize,
            comparator_pos.y as usize,
            comparator_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_COMPARATOR,
                state: comparator_state,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            front_lamp_pos.x as usize,
            front_lamp_pos.y as usize,
            front_lamp_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            side_lamp_pos.x as usize,
            side_lamp_pos.y as usize,
            side_lamp_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        // Power rear + side levers. Side lever powers the side wire at strength 14.
        sim.toggle_lever(rear_lever_pos, &mut chunks);
        sim.toggle_lever(side_lever_pos, &mut chunks);

        // Tick 1: comparator computes desired output but hasn't applied yet.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            let comparator = chunk.voxel(6, 64, 5);
            assert_eq!(comparator_output_power(comparator.state), 0);
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Tick 2: compare-mode output applies (rear 15 >= side 14) => 15, and only powers front.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            let comparator = chunk.voxel(6, 64, 5);
            assert_eq!(comparator_output_power(comparator.state), MAX_POWER);
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP_LIT);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Toggle to subtract mode.
        {
            let chunk = chunks.get_mut(&ChunkPos::new(0, 0)).unwrap();
            let voxel = chunk.voxel(6, 64, 5);
            let new_state = set_comparator_subtract_mode(voxel.state, true);
            chunk.set_voxel(
                6,
                64,
                5,
                Voxel {
                    state: new_state,
                    ..voxel
                },
            );
        }
        sim.schedule_update(comparator_pos);

        // Tick 3: delay tick (output still 15).
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            let comparator = chunk.voxel(6, 64, 5);
            assert_eq!(comparator_output_power(comparator.state), MAX_POWER);
        }

        // Tick 4: subtract-mode output applies (15 - 14) => 1.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            let comparator = chunk.voxel(6, 64, 5);
            assert_eq!(comparator_output_power(comparator.state), 1);
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP_LIT);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }
    }

    #[test]
    fn observer_pulses_on_observed_change_with_delay_and_direction() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        let observed_pos = RedstonePos::new(5, 64, 5);
        let observer_pos = RedstonePos::new(6, 64, 5);
        let front_lamp_pos = RedstonePos::new(7, 64, 5);
        let side_lamp_pos = RedstonePos::new(6, 64, 6);

        chunk.set_voxel(
            observed_pos.x as usize,
            observed_pos.y as usize,
            observed_pos.z as usize,
            Voxel {
                id: 1,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mut observer_state = 0;
        observer_state = set_observer_facing(observer_state, Facing::East);
        chunk.set_voxel(
            observer_pos.x as usize,
            observer_pos.y as usize,
            observer_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_OBSERVER,
                state: observer_state,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            front_lamp_pos.x as usize,
            front_lamp_pos.y as usize,
            front_lamp_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunk.set_voxel(
            side_lamp_pos.x as usize,
            side_lamp_pos.y as usize,
            side_lamp_pos.z as usize,
            Voxel {
                id: redstone_blocks::REDSTONE_LAMP,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        // First update initializes the stored hash but should not pulse.
        sim.schedule_update(observer_pos);
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            let observer = chunk.voxel(6, 64, 5);
            assert_eq!(observer.id, redstone_blocks::REDSTONE_OBSERVER);
            assert!(!is_active(observer.state));
            assert_ne!(observer_observed_hash(observer.state), 0);
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Change the observed block behind the observer.
        {
            let chunk = chunks.get_mut(&ChunkPos::new(0, 0)).unwrap();
            chunk.set_voxel(
                observed_pos.x as usize,
                observed_pos.y as usize,
                observed_pos.z as usize,
                Voxel {
                    id: 24, // Cobblestone
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );
        }

        // Tick 2: observer notices the change and schedules a pulse (no output yet).
        sim.schedule_update(observer_pos);
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(!is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Tick 3: pulse turns on and powers only the front lamp.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP_LIT);
            assert_eq!(chunk.voxel(6, 64, 6).id, redstone_blocks::REDSTONE_LAMP);
        }

        // Tick 4: still on (2-tick pulse).
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP_LIT);
        }

        // Tick 5: pulse turns off.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(!is_active(chunk.voxel(6, 64, 5).state));
            assert_eq!(chunk.voxel(7, 64, 5).id, redstone_blocks::REDSTONE_LAMP);
        }
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

    #[test]
    fn piston_extends_after_one_tick_and_pushes_line() {
        let mut sim = RedstoneSimulator::new();
        let mut chunks = HashMap::new();
        let mut chunk = create_test_chunk();

        let lever_pos = RedstonePos::new(4, 64, 5);
        let piston_pos = RedstonePos::new(5, 64, 5);
        let pushed_pos = RedstonePos::new(6, 64, 5);
        let dest_pos = RedstonePos::new(7, 64, 5);

        // Lever powering piston from the rear.
        chunk.set_voxel(
            lever_pos.x as usize,
            lever_pos.y as usize,
            lever_pos.z as usize,
            Voxel {
                id: redstone_blocks::LEVER,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mut piston_state = 0;
        piston_state = set_piston_facing(piston_state, Facing::East);
        piston_state = set_active(piston_state, false);
        chunk.set_voxel(
            piston_pos.x as usize,
            piston_pos.y as usize,
            piston_pos.z as usize,
            Voxel {
                id: mechanical_blocks::PISTON,
                state: piston_state,
                light_sky: 0,
                light_block: 0,
            },
        );

        // Block to push.
        chunk.set_voxel(
            pushed_pos.x as usize,
            pushed_pos.y as usize,
            pushed_pos.z as usize,
            Voxel {
                id: 1, // Stone
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        chunks.insert(ChunkPos::new(0, 0), chunk);

        sim.toggle_lever(lever_pos, &mut chunks);

        // Tick 1: piston schedules extension but has not moved blocks yet.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(!is_active(chunk.voxel(5, 64, 5).state));
            assert_eq!(chunk.voxel(6, 64, 5).id, 1);
            assert_eq!(
                chunk.voxel(dest_pos.x as usize, dest_pos.y as usize, dest_pos.z as usize)
                    .id,
                crate::BLOCK_AIR
            );
        }

        // Tick 2: extension applies, head appears, and the block is pushed forward.
        sim.tick(&mut chunks);
        {
            let chunk = chunks.get(&ChunkPos::new(0, 0)).unwrap();
            assert!(is_active(chunk.voxel(5, 64, 5).state));
            assert_eq!(chunk.voxel(6, 64, 5).id, mechanical_blocks::PISTON_HEAD);
            assert_eq!(
                chunk.voxel(dest_pos.x as usize, dest_pos.y as usize, dest_pos.z as usize)
                    .id,
                1
            );
        }
    }
}
