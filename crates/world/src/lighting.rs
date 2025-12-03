//! Dual-channel BFS lighting propagation.
//!
//! Implements skylight (0-15) and block light (0-15) using breadth-first search
//! queues for deterministic propagation. Light updates track changes for cross-
//! chunk border handling and event logging.

use crate::chunk::{
    Chunk, ChunkPos, LocalPos, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use std::collections::HashMap;
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

/// Represents a pending cross-chunk light update.
/// Used to queue updates that need to be processed in neighboring chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossChunkLightUpdate {
    /// The neighboring chunk that needs updating.
    pub target_chunk: ChunkPos,
    /// Local position within the target chunk.
    pub target_pos: LocalPos,
    /// Light level to propagate.
    pub level: u8,
    /// Type of light (skylight or block light).
    pub light_type: LightType,
}

/// Result of light propagation, including any cross-chunk updates needed.
#[derive(Debug, Clone, Default)]
pub struct LightPropagationResult {
    /// Number of nodes processed within the chunk.
    pub nodes_processed: usize,
    /// Pending updates for neighboring chunks.
    pub cross_chunk_updates: Vec<CrossChunkLightUpdate>,
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

impl LightType {
    fn voxel_level(voxel: &crate::chunk::Voxel, light_type: LightType) -> u8 {
        match light_type {
            LightType::Skylight => voxel.light_sky,
            LightType::BlockLight => voxel.light_block,
        }
    }

    fn set_voxel_level(voxel: &mut crate::chunk::Voxel, light_type: LightType, level: u8) {
        match light_type {
            LightType::Skylight => voxel.light_sky = level,
            LightType::BlockLight => voxel.light_block = level,
        }
    }
}

/// BFS queue for light propagation within a single chunk.
pub struct LightQueue {
    queue: VecDeque<LightNode>,
    /// Chunk position for calculating cross-chunk updates.
    chunk_pos: ChunkPos,
    /// Pending cross-chunk updates collected during propagation.
    cross_chunk_updates: Vec<CrossChunkLightUpdate>,
}

impl LightQueue {
    /// Create a new light queue for the given chunk.
    pub fn new_for_chunk(chunk_pos: ChunkPos) -> Self {
        Self {
            queue: VecDeque::with_capacity(256),
            chunk_pos,
            cross_chunk_updates: Vec::new(),
        }
    }

    /// Create a new light queue (uses default chunk position).
    /// Use `new_for_chunk` when cross-chunk updates need to be tracked.
    pub fn new() -> Self {
        Self::new_for_chunk(ChunkPos::new(0, 0))
    }

    /// Add a light source to the propagation queue.
    pub fn enqueue(&mut self, pos: LocalPos, level: u8) {
        self.queue.push_back(LightNode { pos, level });
    }

    /// Take the collected cross-chunk updates, clearing the internal buffer.
    pub fn take_cross_chunk_updates(&mut self) -> Vec<CrossChunkLightUpdate> {
        std::mem::take(&mut self.cross_chunk_updates)
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

            // Calculate new light level (decay by 1, special case for skylight downward).
            let new_level = if is_skylight && dy == -1 {
                // Skylight propagates downward without decay.
                level
            } else {
                level.saturating_sub(1)
            };

            if new_level == 0 {
                continue; // No point propagating zero light
            }

            // Check vertical bounds (no chunks above/below).
            if ny < 0 || ny >= CHUNK_SIZE_Y as i32 {
                continue;
            }

            // Check horizontal bounds and queue cross-chunk updates if needed.
            let mut target_chunk = self.chunk_pos;
            let mut local_x = nx;
            let mut local_z = nz;

            // Handle X boundary crossing
            if nx < 0 {
                target_chunk.x -= 1;
                local_x = CHUNK_SIZE_X as i32 - 1;
            } else if nx >= CHUNK_SIZE_X as i32 {
                target_chunk.x += 1;
                local_x = 0;
            }

            // Handle Z boundary crossing
            if nz < 0 {
                target_chunk.z -= 1;
                local_z = CHUNK_SIZE_Z as i32 - 1;
            } else if nz >= CHUNK_SIZE_Z as i32 {
                target_chunk.z += 1;
                local_z = 0;
            }

            // If we crossed a chunk boundary, queue the cross-chunk update
            if target_chunk != self.chunk_pos {
                let light_type = if is_skylight {
                    LightType::Skylight
                } else {
                    LightType::BlockLight
                };

                self.cross_chunk_updates.push(CrossChunkLightUpdate {
                    target_chunk,
                    target_pos: LocalPos {
                        x: local_x as usize,
                        y: ny as usize,
                        z: local_z as usize,
                    },
                    level: new_level,
                    light_type,
                });
                continue;
            }

            // Within chunk bounds - check opacity and enqueue
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

            self.enqueue(neighbor_pos, new_level);
        }
    }
}

/// Propagate light across chunk seams using existing light values as seeds.
/// Seeds boundary voxels of `chunk_pos` into neighbors, respecting opacity and decay rules.
pub fn stitch_light_seams(
    chunks: &mut HashMap<ChunkPos, Chunk>,
    registry: &dyn BlockOpacityProvider,
    chunk_pos: ChunkPos,
    light_type: LightType,
) -> usize {
    let mut queue: VecDeque<(BlockPos, u8)> = VecDeque::new();

    // Seed boundary voxels of the source chunk.
    if let Some(chunk) = chunks.get(&chunk_pos) {
        for y in 0..CHUNK_SIZE_Y {
            for x in 0..CHUNK_SIZE_X {
                let north = chunk.voxel(x, y, 0);
                let south = chunk.voxel(x, y, CHUNK_SIZE_Z - 1);
                enqueue_seed(&mut queue, chunk_pos, x, y, 0, north, light_type);
                enqueue_seed(
                    &mut queue,
                    chunk_pos,
                    x,
                    y,
                    CHUNK_SIZE_Z - 1,
                    south,
                    light_type,
                );
            }
            for z in 0..CHUNK_SIZE_Z {
                let west = chunk.voxel(0, y, z);
                let east = chunk.voxel(CHUNK_SIZE_X - 1, y, z);
                enqueue_seed(&mut queue, chunk_pos, 0, y, z, west, light_type);
                enqueue_seed(
                    &mut queue,
                    chunk_pos,
                    CHUNK_SIZE_X - 1,
                    y,
                    z,
                    east,
                    light_type,
                );
            }
        }
    }

    let mut processed = 0usize;

    while let Some((pos, level)) = queue.pop_front() {
        processed += 1;

        // Visit each neighbor in 6 directions.
        for (dx, dy, dz) in [
            (0, 1, 0),
            (0, -1, 0),
            (1, 0, 0),
            (-1, 0, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            if let Some((neighbor_chunk, neighbor_local)) = neighbor_block(pos, dx, dy, dz) {
                let Some(chunk) = chunks.get_mut(&neighbor_chunk) else {
                    continue;
                };

                let voxel = chunk.voxel(neighbor_local.x, neighbor_local.y, neighbor_local.z);
                if registry.is_opaque(voxel.id) {
                    continue;
                }

                let new_level = if light_type == LightType::Skylight && dy == -1 {
                    level
                } else {
                    level.saturating_sub(1)
                };

                if new_level == 0 {
                    continue;
                }

                let current = LightType::voxel_level(&voxel, light_type);
                if current < new_level {
                    let mut updated = voxel;
                    LightType::set_voxel_level(&mut updated, light_type, new_level);
                    chunk.set_voxel(
                        neighbor_local.x,
                        neighbor_local.y,
                        neighbor_local.z,
                        updated,
                    );
                    queue.push_back((BlockPos::new(neighbor_chunk, neighbor_local), new_level));
                }
            }
        }
    }

    processed
}

fn enqueue_seed(
    queue: &mut VecDeque<(BlockPos, u8)>,
    chunk: ChunkPos,
    x: usize,
    y: usize,
    z: usize,
    voxel: crate::chunk::Voxel,
    light_type: LightType,
) {
    if voxel.id != BLOCK_AIR {
        return; // don't propagate from opaque seeds
    }
    let level = LightType::voxel_level(&voxel, light_type);
    if level > 0 {
        queue.push_back((BlockPos::new(chunk, LocalPos { x, y, z }), level));
    }
}

fn neighbor_block(pos: BlockPos, dx: i32, dy: i32, dz: i32) -> Option<(ChunkPos, LocalPos)> {
    let mut cx = pos.chunk.x;
    let mut cz = pos.chunk.z;
    let mut lx = pos.local.x as i32 + dx;
    let ly = pos.local.y as i32 + dy;
    let mut lz = pos.local.z as i32 + dz;

    if ly < 0 || ly >= CHUNK_SIZE_Y as i32 {
        return None;
    }

    if lx < 0 {
        cx -= 1;
        lx += CHUNK_SIZE_X as i32;
    } else if lx >= CHUNK_SIZE_X as i32 {
        cx += 1;
        lx -= CHUNK_SIZE_X as i32;
    }

    if lz < 0 {
        cz -= 1;
        lz += CHUNK_SIZE_Z as i32;
    } else if lz >= CHUNK_SIZE_Z as i32 {
        cz += 1;
        lz -= CHUNK_SIZE_Z as i32;
    }

    let local = LocalPos {
        x: lx as usize,
        y: ly as usize,
        z: lz as usize,
    };
    Some((ChunkPos::new(cx, cz), local))
}

impl Default for LightQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for querying block opacity (used by lighting system).
pub trait BlockOpacityProvider {
    fn is_opaque(&self, block_id: u16) -> bool;
}

/// Skylight initialization: flood from top of chunk downward.
/// Returns a LightUpdate for metrics and any pending cross-chunk updates.
pub fn init_skylight(chunk: &mut Chunk, registry: &dyn BlockOpacityProvider) -> LightUpdate {
    let chunk_pos = chunk.position();
    let mut queue = LightQueue::new_for_chunk(chunk_pos);

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
        chunk_pos,
        light_type: LightType::Skylight,
        nodes_processed,
    }
}

/// Skylight initialization with cross-chunk update tracking.
/// Returns both the update metrics and pending cross-chunk updates.
pub fn init_skylight_with_neighbors(
    chunk: &mut Chunk,
    registry: &dyn BlockOpacityProvider,
) -> (LightUpdate, Vec<CrossChunkLightUpdate>) {
    let chunk_pos = chunk.position();
    let mut queue = LightQueue::new_for_chunk(chunk_pos);

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
    let cross_chunk_updates = queue.take_cross_chunk_updates();
    chunk.take_dirty_flags();

    (
        LightUpdate {
            chunk_pos,
            light_type: LightType::Skylight,
            nodes_processed,
        },
        cross_chunk_updates,
    )
}

/// Add a block light source at the given position.
pub fn add_block_light(
    chunk: &mut Chunk,
    pos: LocalPos,
    intensity: u8,
    registry: &dyn BlockOpacityProvider,
) -> LightUpdate {
    let chunk_pos = chunk.position();
    let mut queue = LightQueue::new_for_chunk(chunk_pos);
    queue.enqueue(pos, intensity.min(MAX_LIGHT_LEVEL));
    let nodes_processed = queue.propagate_blocklight(chunk, registry);

    LightUpdate {
        chunk_pos,
        light_type: LightType::BlockLight,
        nodes_processed,
    }
}

/// Add a block light source with cross-chunk update tracking.
/// Returns both the update metrics and pending cross-chunk updates.
pub fn add_block_light_with_neighbors(
    chunk: &mut Chunk,
    pos: LocalPos,
    intensity: u8,
    registry: &dyn BlockOpacityProvider,
) -> (LightUpdate, Vec<CrossChunkLightUpdate>) {
    let chunk_pos = chunk.position();
    let mut queue = LightQueue::new_for_chunk(chunk_pos);
    queue.enqueue(pos, intensity.min(MAX_LIGHT_LEVEL));
    let nodes_processed = queue.propagate_blocklight(chunk, registry);
    let cross_chunk_updates = queue.take_cross_chunk_updates();

    (
        LightUpdate {
            chunk_pos,
            light_type: LightType::BlockLight,
            nodes_processed,
        },
        cross_chunk_updates,
    )
}

/// Apply pending cross-chunk light updates to a chunk collection.
///
/// This function processes a list of cross-chunk updates, applying light values
/// to the appropriate chunks and potentially generating further updates.
///
/// # Arguments
/// * `chunks` - Mutable reference to the chunk collection
/// * `registry` - Block opacity provider for light propagation
/// * `updates` - List of pending cross-chunk updates to apply
///
/// # Returns
/// Number of voxels updated across all chunks.
pub fn apply_cross_chunk_updates(
    chunks: &mut HashMap<ChunkPos, Chunk>,
    registry: &dyn BlockOpacityProvider,
    updates: Vec<CrossChunkLightUpdate>,
) -> usize {
    let mut total_updated = 0;
    let mut pending: VecDeque<CrossChunkLightUpdate> = updates.into_iter().collect();

    while let Some(update) = pending.pop_front() {
        let Some(chunk) = chunks.get_mut(&update.target_chunk) else {
            continue; // Chunk not loaded, skip
        };

        let voxel = chunk.voxel(update.target_pos.x, update.target_pos.y, update.target_pos.z);

        // Check if this voxel blocks light
        if registry.is_opaque(voxel.id) {
            continue;
        }

        // Check current light level
        let current_level = match update.light_type {
            LightType::Skylight => voxel.light_sky,
            LightType::BlockLight => voxel.light_block,
        };

        // Only update if new level is higher
        if update.level <= current_level {
            continue;
        }

        // Update the voxel's light level
        let mut updated_voxel = voxel;
        match update.light_type {
            LightType::Skylight => updated_voxel.light_sky = update.level,
            LightType::BlockLight => updated_voxel.light_block = update.level,
        }
        chunk.set_voxel(
            update.target_pos.x,
            update.target_pos.y,
            update.target_pos.z,
            updated_voxel,
        );
        total_updated += 1;

        // Propagate to neighbors using a local queue
        let mut queue = LightQueue::new_for_chunk(update.target_chunk);
        queue.enqueue(update.target_pos, update.level);

        // Only propagate to get cross-chunk updates (don't re-process same chunk)
        // The BFS will naturally handle intra-chunk propagation
        match update.light_type {
            LightType::Skylight => {
                queue.propagate_skylight(chunk, registry);
            }
            LightType::BlockLight => {
                queue.propagate_blocklight(chunk, registry);
            }
        }

        // Add any new cross-chunk updates to the queue
        for new_update in queue.take_cross_chunk_updates() {
            pending.push_back(new_update);
        }
    }

    total_updated
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
    use std::collections::HashMap;

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

    #[test]
    fn block_light_crosses_chunk_seams() {
        let mut chunks = HashMap::new();
        let pos_a = ChunkPos::new(0, 0);
        let pos_b = ChunkPos::new(1, 0);
        let mut chunk_a = Chunk::new(pos_a);
        let chunk_b = Chunk::new(pos_b);

        // Place a torch-equivalent block light at the east edge of chunk A.
        let torch_pos = LocalPos {
            x: CHUNK_SIZE_X - 1,
            y: 64,
            z: 8,
        };
        let mut torch_voxel = chunk_a.voxel(torch_pos.x, torch_pos.y, torch_pos.z);
        torch_voxel.light_block = MAX_LIGHT_LEVEL;
        chunk_a.set_voxel(torch_pos.x, torch_pos.y, torch_pos.z, torch_voxel);

        chunks.insert(pos_a, chunk_a);
        chunks.insert(pos_b, chunk_b);

        let registry = MockRegistry;

        // Stitch block light across seam.
        let _ = stitch_light_seams(&mut chunks, &registry, pos_a, LightType::BlockLight);

        // West face of chunk B adjacent to torch should receive propagated light (level 14).
        let chunk_b = chunks.get(&pos_b).unwrap();
        let lit = chunk_b.voxel(0, torch_pos.y, torch_pos.z);
        assert_eq!(lit.light_block, MAX_LIGHT_LEVEL - 1);
    }

    #[test]
    fn add_block_light_generates_cross_chunk_updates() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = MockRegistry;

        // Place torch at the very edge of the chunk (east boundary).
        let torch_pos = LocalPos {
            x: CHUNK_SIZE_X - 1,
            y: 64,
            z: 8,
        };

        let (update, cross_chunk_updates) =
            add_block_light_with_neighbors(&mut chunk, torch_pos, MAX_LIGHT_LEVEL, &registry);

        assert!(update.nodes_processed > 0);

        // Should have generated at least one cross-chunk update for the east neighbor.
        assert!(
            !cross_chunk_updates.is_empty(),
            "Expected cross-chunk updates for edge torch"
        );

        // Verify at least one update targets the chunk to the east.
        let east_updates: Vec<_> = cross_chunk_updates
            .iter()
            .filter(|u| u.target_chunk == ChunkPos::new(1, 0))
            .collect();

        assert!(
            !east_updates.is_empty(),
            "Expected update for east neighbor chunk"
        );

        // Check the first east update has correct position and level.
        let first_east = east_updates[0];
        assert_eq!(first_east.target_pos.x, 0); // Should be west edge of east chunk
        assert_eq!(first_east.target_pos.y, torch_pos.y);
        assert_eq!(first_east.light_type, LightType::BlockLight);
        assert_eq!(first_east.level, MAX_LIGHT_LEVEL - 1); // Decayed by 1
    }

    #[test]
    fn apply_cross_chunk_updates_propagates_light() {
        let mut chunks = HashMap::new();
        let pos_a = ChunkPos::new(0, 0);
        let pos_b = ChunkPos::new(1, 0);
        let mut chunk_a = Chunk::new(pos_a);
        let chunk_b = Chunk::new(pos_b);
        let registry = MockRegistry;

        // Add torch at east edge of chunk A.
        let torch_pos = LocalPos {
            x: CHUNK_SIZE_X - 1,
            y: 64,
            z: 8,
        };

        let (_, cross_chunk_updates) =
            add_block_light_with_neighbors(&mut chunk_a, torch_pos, MAX_LIGHT_LEVEL, &registry);

        chunks.insert(pos_a, chunk_a);
        chunks.insert(pos_b, chunk_b);

        // Apply the cross-chunk updates.
        let updated = apply_cross_chunk_updates(&mut chunks, &registry, cross_chunk_updates);

        assert!(updated > 0, "Expected some voxels to be updated");

        // Verify chunk B received light at its west edge.
        let chunk_b = chunks.get(&pos_b).unwrap();
        let lit_voxel = chunk_b.voxel(0, torch_pos.y, torch_pos.z);
        assert!(
            lit_voxel.light_block > 0,
            "Expected chunk B to receive light"
        );
    }

    #[test]
    fn cross_chunk_update_respects_opacity() {
        let mut chunks = HashMap::new();
        let pos_a = ChunkPos::new(0, 0);
        let pos_b = ChunkPos::new(1, 0);
        let mut chunk_a = Chunk::new(pos_a);
        let mut chunk_b = Chunk::new(pos_b);
        let registry = MockRegistry;

        // Add torch at east edge of chunk A.
        let torch_pos = LocalPos {
            x: CHUNK_SIZE_X - 1,
            y: 64,
            z: 8,
        };

        // Place an opaque block in chunk B at the position that would receive light.
        let mut blocking_voxel = chunk_b.voxel(0, torch_pos.y, torch_pos.z);
        blocking_voxel.id = 1; // Opaque block (not air)
        chunk_b.set_voxel(0, torch_pos.y, torch_pos.z, blocking_voxel);

        let (_, cross_chunk_updates) =
            add_block_light_with_neighbors(&mut chunk_a, torch_pos, MAX_LIGHT_LEVEL, &registry);

        chunks.insert(pos_a, chunk_a);
        chunks.insert(pos_b, chunk_b);

        // Apply the cross-chunk updates.
        apply_cross_chunk_updates(&mut chunks, &registry, cross_chunk_updates);

        // Verify chunk B did NOT receive light (blocked by opaque block).
        let chunk_b = chunks.get(&pos_b).unwrap();
        let blocked_voxel = chunk_b.voxel(0, torch_pos.y, torch_pos.z);
        assert_eq!(
            blocked_voxel.light_block, 0,
            "Opaque block should not receive light"
        );
    }
}
