//! Interaction system for placing and breaking blocks.
//!
//! Uses raycast to determine which block the player is looking at,
//! then handles left-click (break) and right-click (place) actions.

use crate::camera::CameraController;
use crate::events::{EventSink, GameEvent};
use crate::ui::Hotbar;
use mdminecraft_physics::ray_cast_voxel;
use mdminecraft_world::{ChunkStorage, Voxel, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Z};

/// Maximum reach distance for block interaction (in blocks).
pub const MAX_REACH_DISTANCE: f32 = 5.0;

/// Controller for player-world interactions (placing and breaking blocks).
#[derive(Debug)]
pub struct InteractionController {
    /// Range limit for interactions.
    pub max_reach: f32,
}

impl InteractionController {
    /// Create a new interaction controller.
    pub fn new() -> Self {
        Self {
            max_reach: MAX_REACH_DISTANCE,
        }
    }

    /// Handle a left mouse click (break block).
    ///
    /// # Arguments
    /// * `storage` - Chunk storage to modify
    /// * `camera` - Camera to determine look direction
    /// * `events` - Event sink to emit BlockBroken events
    ///
    /// # Returns
    /// true if a block was broken, false otherwise
    pub fn handle_break_block(
        &self,
        storage: &mut ChunkStorage,
        camera: &CameraController,
        events: &mut EventSink,
    ) -> bool {
        // Cast ray from camera
        let origin = camera.position;
        let direction = camera.forward();

        // Find target block
        if let Some(hit) = ray_cast_voxel(storage, origin, direction, self.max_reach) {
            let [x, y, z] = hit.position;

            // Break the block (set to air)
            if let Some(chunk) = get_chunk_mut(storage, x, y, z) {
                let (local_x, local_y, local_z) = world_to_local(x, y, z);
                let old_voxel = chunk.voxel(local_x, local_y, local_z);

                if old_voxel.id != BLOCK_AIR {
                    // Set to air
                    let air_voxel = Voxel {
                        id: BLOCK_AIR,
                        state: 0,
                        light_sky: old_voxel.light_sky,
                        light_block: old_voxel.light_block,
                    };
                    chunk.set_voxel(local_x, local_y, local_z, air_voxel);

                    // Emit event
                    events.emit(GameEvent::BlockBroken {
                        position: [x, y, z],
                        block_id: old_voxel.id,
                    });

                    return true;
                }
            }
        }

        false
    }

    /// Handle a right mouse click (place block).
    ///
    /// # Arguments
    /// * `storage` - Chunk storage to modify
    /// * `camera` - Camera to determine look direction
    /// * `hotbar` - Hotbar to get selected block
    /// * `events` - Event sink to emit BlockPlaced events
    ///
    /// # Returns
    /// true if a block was placed, false otherwise
    pub fn handle_place_block(
        &self,
        storage: &mut ChunkStorage,
        camera: &CameraController,
        hotbar: &Hotbar,
        events: &mut EventSink,
    ) -> bool {
        // Get selected block from hotbar
        let block_id = hotbar.get_selected();
        if block_id == BLOCK_AIR {
            return false;
        }

        // Cast ray from camera
        let origin = camera.position;
        let direction = camera.forward();

        // Find target block
        if let Some(hit) = ray_cast_voxel(storage, origin, direction, self.max_reach) {
            // Calculate placement position (adjacent to hit block, on the hit face)
            let [bx, by, bz] = hit.position;
            let [nx, ny, nz] = hit.normal;
            let place_x = bx + nx;
            let place_y = by + ny;
            let place_z = bz + nz;

            // Check if position is valid (not inside player, not out of bounds)
            if !(0..256).contains(&place_y) {
                return false;
            }

            // Place the block
            if let Some(chunk) = get_chunk_mut(storage, place_x, place_y, place_z) {
                let (local_x, local_y, local_z) = world_to_local(place_x, place_y, place_z);
                let old_voxel = chunk.voxel(local_x, local_y, local_z);

                // Only place if the target location is air
                if old_voxel.id == BLOCK_AIR {
                    let new_voxel = Voxel {
                        id: block_id,
                        state: 0,
                        light_sky: old_voxel.light_sky,
                        light_block: old_voxel.light_block,
                    };
                    chunk.set_voxel(local_x, local_y, local_z, new_voxel);

                    // Emit event
                    events.emit(GameEvent::BlockPlaced {
                        position: [place_x, place_y, place_z],
                        block_id,
                    });

                    return true;
                }
            }
        }

        false
    }
}

impl Default for InteractionController {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert world coordinates to chunk and local coordinates.
fn world_to_local(x: i32, y: i32, z: i32) -> (usize, usize, usize) {
    let local_x = x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let local_y = y as usize;
    let local_z = z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
    (local_x, local_y, local_z)
}

/// Get mutable access to the chunk containing the given world coordinates.
fn get_chunk_mut(
    storage: &mut ChunkStorage,
    x: i32,
    y: i32,
    _z: i32,
) -> Option<&mut mdminecraft_world::Chunk> {
    if !(0..256).contains(&y) {
        return None;
    }

    let chunk_x = x.div_euclid(CHUNK_SIZE_X as i32);
    let chunk_z = _z.div_euclid(CHUNK_SIZE_Z as i32);
    let chunk_pos = mdminecraft_world::ChunkPos::new(chunk_x, chunk_z);

    Some(storage.ensure_chunk(chunk_pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_world::{BlockId, ChunkPos};

    fn create_test_storage_with_block(x: i32, y: i32, z: i32, block_id: BlockId) -> ChunkStorage {
        let mut storage = ChunkStorage::new(10);

        let chunk_x = x.div_euclid(CHUNK_SIZE_X as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE_Z as i32);
        let (local_x, local_y, local_z) = world_to_local(x, y, z);

        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
        let chunk = storage.ensure_chunk(chunk_pos);

        let voxel = Voxel {
            id: block_id,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };
        chunk.set_voxel(local_x, local_y, local_z, voxel);

        storage
    }

    #[test]
    fn interaction_controller_creation() {
        let controller = InteractionController::new();
        assert_eq!(controller.max_reach, MAX_REACH_DISTANCE);
    }

    #[test]
    fn break_block_removes_block_and_emits_event() {
        let mut storage = create_test_storage_with_block(5, 5, 5, 1);
        // Camera looking along +X axis (yaw = PI/2)
        let camera =
            CameraController::new([0.5, 5.5, 5.5], std::f32::consts::FRAC_PI_2, 0.0, 0.002);
        let mut events = EventSink::new();
        let controller = InteractionController::new();

        // Verify block exists
        let chunk = storage.get(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(5, 5, 5).id, 1);

        // Break the block
        let result = controller.handle_break_block(&mut storage, &camera, &mut events);

        assert!(result);

        // Verify block is now air
        let chunk = storage.get(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(5, 5, 5).id, BLOCK_AIR);

        // Verify event was emitted
        assert_eq!(events.events().len(), 1);
        match &events.events()[0] {
            GameEvent::BlockBroken { position, block_id } => {
                assert_eq!(*position, [5, 5, 5]);
                assert_eq!(*block_id, 1);
            }
            _ => panic!("Expected BlockBroken event"),
        }
    }

    #[test]
    fn break_block_returns_false_when_no_block_in_range() {
        let mut storage = ChunkStorage::new(10); // Empty world
        let camera = CameraController::new([0.5, 5.5, 5.5], 0.0, 0.0, 0.002);
        let mut events = EventSink::new();
        let controller = InteractionController::new();

        let result = controller.handle_break_block(&mut storage, &camera, &mut events);

        assert!(!result);
        assert!(events.events().is_empty());
    }

    #[test]
    fn break_block_respects_max_reach() {
        let mut storage = create_test_storage_with_block(10, 5, 5, 1);
        let camera = CameraController::new([0.5, 5.5, 5.5], 0.0, 0.0, 0.002);
        let mut events = EventSink::new();
        let mut controller = InteractionController::new();
        controller.max_reach = 5.0;

        // Block is too far away
        let result = controller.handle_break_block(&mut storage, &camera, &mut events);

        assert!(!result);
        assert!(events.events().is_empty());
    }

    #[test]
    fn place_block_adds_block_and_emits_event() {
        let mut storage = create_test_storage_with_block(5, 5, 5, 1);
        // Camera looking along +X axis (yaw = PI/2)
        let camera =
            CameraController::new([0.5, 5.5, 5.5], std::f32::consts::FRAC_PI_2, 0.0, 0.002);
        let mut events = EventSink::new();
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(0, 2); // Select block ID 2
        let controller = InteractionController::new();

        // Place a block adjacent to the existing block
        let result = controller.handle_place_block(&mut storage, &camera, &hotbar, &mut events);

        assert!(result);

        // Verify event was emitted
        assert_eq!(events.events().len(), 1);
        match &events.events()[0] {
            GameEvent::BlockPlaced { position, block_id } => {
                // Should be placed on the -X face (normal [-1, 0, 0])
                assert_eq!(*position, [4, 5, 5]);
                assert_eq!(*block_id, 2);
            }
            _ => panic!("Expected BlockPlaced event"),
        }

        // Verify block was placed
        let chunk = storage.get(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(4, 5, 5).id, 2);
    }

    #[test]
    fn place_block_returns_false_when_hotbar_is_empty() {
        let mut storage = create_test_storage_with_block(5, 5, 5, 1);
        let camera = CameraController::new([0.5, 5.5, 5.5], 0.0, 0.0, 0.002);
        let mut events = EventSink::new();
        let hotbar = Hotbar::new(); // Empty hotbar (air selected)
        let controller = InteractionController::new();

        let result = controller.handle_place_block(&mut storage, &camera, &hotbar, &mut events);

        assert!(!result);
        assert!(events.events().is_empty());
    }

    #[test]
    fn place_block_returns_false_when_no_block_in_range() {
        let mut storage = ChunkStorage::new(10); // Empty world
        let camera = CameraController::new([0.5, 5.5, 5.5], 0.0, 0.0, 0.002);
        let mut events = EventSink::new();
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(0, 2);
        let controller = InteractionController::new();

        let result = controller.handle_place_block(&mut storage, &camera, &hotbar, &mut events);

        assert!(!result);
        assert!(events.events().is_empty());
    }

    #[test]
    fn place_block_only_replaces_air() {
        let mut storage = create_test_storage_with_block(5, 5, 5, 1);
        // Also place a block at [4, 5, 5]
        let chunk = storage.ensure_chunk(ChunkPos::new(0, 0));
        chunk.set_voxel(
            4,
            5,
            5,
            Voxel {
                id: 3,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );

        let camera = CameraController::new([0.5, 5.5, 5.5], 0.0, 0.0, 0.002);
        let mut events = EventSink::new();
        let mut hotbar = Hotbar::new();
        hotbar.set_slot(0, 2);
        let controller = InteractionController::new();

        // Try to place - should fail because target is not air
        let result = controller.handle_place_block(&mut storage, &camera, &hotbar, &mut events);

        assert!(!result);
        assert!(events.events().is_empty());

        // Verify original block is unchanged
        let chunk = storage.get(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(chunk.voxel(4, 5, 5).id, 3);
    }

    #[test]
    fn world_to_local_converts_positive_coords() {
        let (x, y, z) = world_to_local(5, 10, 7);
        assert_eq!((x, y, z), (5, 10, 7));
    }

    #[test]
    fn world_to_local_converts_chunk_boundary() {
        let (x, y, z) = world_to_local(16, 10, 16);
        assert_eq!((x, y, z), (0, 10, 0));
    }

    #[test]
    fn world_to_local_converts_negative_coords() {
        let (x, y, z) = world_to_local(-1, 10, -1);
        assert_eq!((x, y, z), (15, 10, 15));
    }

    #[test]
    fn get_chunk_mut_returns_chunk_for_valid_coords() {
        let mut storage = ChunkStorage::new(10);
        let chunk = get_chunk_mut(&mut storage, 5, 10, 7);
        assert!(chunk.is_some());
    }

    #[test]
    fn get_chunk_mut_returns_none_for_invalid_y() {
        let mut storage = ChunkStorage::new(10);
        assert!(get_chunk_mut(&mut storage, 5, -1, 7).is_none());
        assert!(get_chunk_mut(&mut storage, 5, 256, 7).is_none());
    }
}
