//! Dual-channel BFS lighting propagation.
//!
//! Implements skylight (0-15) and block light (0-15) using breadth-first search
//! queues for deterministic propagation. Light updates track changes for cross-
//! chunk border handling and event logging.

use crate::chunk::{Chunk, ChunkPos, LocalPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use std::collections::VecDeque;

/// Maximum light level (0-15 range).
pub const MAX_LIGHT_LEVEL: u8 = 15;

/// Minimum light level (complete darkness).
pub const MIN_LIGHT_LEVEL: u8 = 0;

/// Position within world space (chunk-relative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub chunk: ChunkPos,
    pub local: LocalPos,
}

impl BlockPos {
    pub fn new(chunk: ChunkPos, local: LocalPos) -> Self {
        Self { chunk, local }
    }
}

/// Light propagation queue entry.
#[derive(Debug, Clone, Copy)]
struct LightNode {
    pos: LocalPos,
    level: u8,
}

/// Describes a light update event for instrumentation/testkit.
#[derive(Debug, Clone)]
pub struct LightUpdate {
    pub chunk_pos: ChunkPos,
    pub light_type: LightType,
    pub nodes_processed: usize,
}

/// Type of light being updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    Skylight,
    BlockLight,
}

/// BFS queue for light propagation within a single chunk.
pub struct LightQueue {
    queue: VecDeque<LightNode>,
}

impl LightQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(256),
        }
    }

    /// Add a light source to the propagation queue.
    pub fn enqueue(&mut self, pos: LocalPos, level: u8) {
        self.queue.push_back(LightNode { pos, level });
    }

    /// Process all queued light updates for skylight.
    pub fn propagate_skylight(
        &mut self,
        chunk: &mut Chunk,
        registry: &dyn BlockOpacityProvider,
    ) -> usize {
        let mut nodes_processed = 0;

        while let Some(node) = self.queue.pop_front() {
            nodes_processed += 1;
            let current_light = chunk.voxel(node.pos.x, node.pos.y, node.pos.z).light_sky;

            // Skip if current light is already higher (another path got here first).
            if current_light >= node.level {
                continue;
            }

            // Update voxel light value.
            let mut voxel = chunk.voxel(node.pos.x, node.pos.y, node.pos.z);
            voxel.light_sky = node.level;
            chunk.set_voxel(node.pos.x, node.pos.y, node.pos.z, voxel);

            // Propagate to neighbors if light can continue.
            if node.level > 0 {
                self.propagate_to_neighbors(node.pos, node.level, chunk, registry, true);
            }
        }

        nodes_processed
    }

    /// Process all queued light updates for block light.
    pub fn propagate_blocklight(
        &mut self,
        chunk: &mut Chunk,
        registry: &dyn BlockOpacityProvider,
    ) -> usize {
        let mut nodes_processed = 0;

        while let Some(node) = self.queue.pop_front() {
            nodes_processed += 1;
            let current_light = chunk.voxel(node.pos.x, node.pos.y, node.pos.z).light_block;

            // Skip if current light is already higher.
            if current_light >= node.level {
                continue;
            }

            // Update voxel light value.
            let mut voxel = chunk.voxel(node.pos.x, node.pos.y, node.pos.z);
            voxel.light_block = node.level;
            chunk.set_voxel(node.pos.x, node.pos.y, node.pos.z, voxel);

            // Propagate to neighbors if light can continue.
            if node.level > 0 {
                self.propagate_to_neighbors(node.pos, node.level, chunk, registry, false);
            }
        }

        nodes_processed
    }

    /// Propagate light to all 6 cardinal neighbors.
    fn propagate_to_neighbors(
        &mut self,
        pos: LocalPos,
        level: u8,
        chunk: &Chunk,
        registry: &dyn BlockOpacityProvider,
        is_skylight: bool,
    ) {
        let directions = [
            (0, 1, 0),  // up
            (0, -1, 0), // down
            (1, 0, 0),  // east
            (-1, 0, 0), // west
            (0, 0, 1),  // south
            (0, 0, -1), // north
        ];

        for (dx, dy, dz) in directions {
            let nx = pos.x as i32 + dx;
            let ny = pos.y as i32 + dy;
            let nz = pos.z as i32 + dz;

            // Check bounds (stay within chunk).
            if nx < 0
                || nx >= CHUNK_SIZE_X as i32
                || ny < 0
                || ny >= CHUNK_SIZE_Y as i32
                || nz < 0
                || nz >= CHUNK_SIZE_Z as i32
            {
                // TODO: Queue cross-chunk light updates for border handling.
                continue;
            }

            let neighbor_pos = LocalPos {
                x: nx as usize,
                y: ny as usize,
                z: nz as usize,
            };

            let neighbor_voxel = chunk.voxel(neighbor_pos.x, neighbor_pos.y, neighbor_pos.z);

            // Check if neighbor blocks light.
            if registry.is_opaque(neighbor_voxel.id) {
                continue;
            }

            // Calculate new light level (decay by 1, special case for skylight downward).
            let new_level = if is_skylight && dy == -1 {
                // Skylight propagates downward without decay.
                level
            } else {
                level.saturating_sub(1)
            };

            if new_level > 0 {
                self.enqueue(neighbor_pos, new_level);
            }
        }
    }
}

/// Trait for querying block opacity (used by lighting system).
pub trait BlockOpacityProvider {
    fn is_opaque(&self, block_id: u16) -> bool;
}

/// Skylight initialization: flood from top of chunk downward.
pub fn init_skylight(chunk: &mut Chunk, registry: &dyn BlockOpacityProvider) -> LightUpdate {
    let mut queue = LightQueue::new();

    // Start from top layer (Y = CHUNK_SIZE_Y - 1) with max light.
    for x in 0..CHUNK_SIZE_X {
        for z in 0..CHUNK_SIZE_Z {
            let pos = LocalPos {
                x,
                y: CHUNK_SIZE_Y - 1,
                z,
            };
            queue.enqueue(pos, MAX_LIGHT_LEVEL);
        }
    }

    let nodes_processed = queue.propagate_skylight(chunk, registry);
    chunk.take_dirty_flags(); // Clear dirty flags after init.

    LightUpdate {
        chunk_pos: chunk.position(),
        light_type: LightType::Skylight,
        nodes_processed,
    }
}

/// Add a block light source at the given position.
pub fn add_block_light(
    chunk: &mut Chunk,
    pos: LocalPos,
    intensity: u8,
    registry: &dyn BlockOpacityProvider,
) -> LightUpdate {
    let mut queue = LightQueue::new();
    queue.enqueue(pos, intensity.min(MAX_LIGHT_LEVEL));
    let nodes_processed = queue.propagate_blocklight(chunk, registry);

    LightUpdate {
        chunk_pos: chunk.position(),
        light_type: LightType::BlockLight,
        nodes_processed,
    }
}

/// Remove block light at a position (reverse BFS for light removal).
pub fn remove_block_light(
    chunk: &mut Chunk,
    pos: LocalPos,
    registry: &dyn BlockOpacityProvider,
) -> LightUpdate {
    // Reverse BFS: mark all affected positions, then re-propagate from remaining sources.
    let mut removal_queue = VecDeque::new();
    let old_light = chunk.voxel(pos.x, pos.y, pos.z).light_block;

    if old_light == 0 {
        // No light to remove.
        return LightUpdate {
            chunk_pos: chunk.position(),
            light_type: LightType::BlockLight,
            nodes_processed: 0,
        };
    }

    // Clear the source light.
    let mut voxel = chunk.voxel(pos.x, pos.y, pos.z);
    voxel.light_block = 0;
    chunk.set_voxel(pos.x, pos.y, pos.z, voxel);

    removal_queue.push_back((pos, old_light));
    let mut nodes_processed = 1;
    let mut relight_queue = LightQueue::new();

    // Remove light from all affected neighbors.
    while let Some((current_pos, light_level)) = removal_queue.pop_front() {
        let directions = [
            (0, 1, 0),
            (0, -1, 0),
            (1, 0, 0),
            (-1, 0, 0),
            (0, 0, 1),
            (0, 0, -1),
        ];

        for (dx, dy, dz) in directions {
            let nx = current_pos.x as i32 + dx;
            let ny = current_pos.y as i32 + dy;
            let nz = current_pos.z as i32 + dz;

            if nx < 0
                || nx >= CHUNK_SIZE_X as i32
                || ny < 0
                || ny >= CHUNK_SIZE_Y as i32
                || nz < 0
                || nz >= CHUNK_SIZE_Z as i32
            {
                continue;
            }

            let neighbor_pos = LocalPos {
                x: nx as usize,
                y: ny as usize,
                z: nz as usize,
            };

            let neighbor_voxel = chunk.voxel(neighbor_pos.x, neighbor_pos.y, neighbor_pos.z);
            let neighbor_light = neighbor_voxel.light_block;

            if neighbor_light > 0 {
                if neighbor_light < light_level {
                    // This neighbor was lit by the removed source, clear it.
                    let mut voxel = chunk.voxel(neighbor_pos.x, neighbor_pos.y, neighbor_pos.z);
                    voxel.light_block = 0;
                    chunk.set_voxel(neighbor_pos.x, neighbor_pos.y, neighbor_pos.z, voxel);
                    removal_queue.push_back((neighbor_pos, neighbor_light));
                    nodes_processed += 1;
                } else {
                    // This neighbor has a stronger light source, re-propagate from it.
                    relight_queue.enqueue(neighbor_pos, neighbor_light);
                }
            }
        }
    }

    // Re-propagate remaining light sources.
    nodes_processed += relight_queue.propagate_blocklight(chunk, registry);

    LightUpdate {
        chunk_pos: chunk.position(),
        light_type: LightType::BlockLight,
        nodes_processed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{BlockId, BLOCK_AIR};

    /// Mock block registry for testing.
    struct MockRegistry;

    impl BlockOpacityProvider for MockRegistry {
        fn is_opaque(&self, block_id: BlockId) -> bool {
            block_id != BLOCK_AIR
        }
    }

    #[test]
    fn skylight_floods_downward() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = MockRegistry;

        let update = init_skylight(&mut chunk, &registry);
        assert!(update.nodes_processed > 0);

        // Check top voxels have max light.
        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                let voxel = chunk.voxel(x, CHUNK_SIZE_Y - 1, z);
                assert_eq!(voxel.light_sky, MAX_LIGHT_LEVEL);
            }
        }

        // Check bottom voxels have max light (no obstacles).
        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                let voxel = chunk.voxel(x, 0, z);
                assert_eq!(voxel.light_sky, MAX_LIGHT_LEVEL);
            }
        }
    }

    #[test]
    fn block_light_propagates_with_decay() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = MockRegistry;

        let torch_pos = LocalPos { x: 8, y: 64, z: 8 };
        let update = add_block_light(&mut chunk, torch_pos, 15, &registry);
        assert!(update.nodes_processed > 0);

        // Check torch position has max light.
        let voxel = chunk.voxel(torch_pos.x, torch_pos.y, torch_pos.z);
        assert_eq!(voxel.light_block, 15);

        // Check adjacent position has light - 1.
        let adjacent = chunk.voxel(torch_pos.x + 1, torch_pos.y, torch_pos.z);
        assert_eq!(adjacent.light_block, 14);
    }

    #[test]
    fn remove_block_light_clears_affected_area() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = MockRegistry;

        let torch_pos = LocalPos { x: 8, y: 64, z: 8 };
        add_block_light(&mut chunk, torch_pos, 15, &registry);

        // Verify light is present.
        let voxel_before = chunk.voxel(torch_pos.x, torch_pos.y, torch_pos.z);
        assert_eq!(voxel_before.light_block, 15);

        // Remove light.
        let update = remove_block_light(&mut chunk, torch_pos, &registry);
        assert!(update.nodes_processed > 0);

        // Check torch position is now dark.
        let voxel_after = chunk.voxel(torch_pos.x, torch_pos.y, torch_pos.z);
        assert_eq!(voxel_after.light_block, 0);

        // Check adjacent positions are also dark.
        let adjacent = chunk.voxel(torch_pos.x + 1, torch_pos.y, torch_pos.z);
        assert_eq!(adjacent.light_block, 0);
    }
}
