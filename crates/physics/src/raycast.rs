//! Voxel raycasting using DDA (Digital Differential Analyzer) algorithm.

use mdminecraft_world::{
    BlockId, ChunkStorage, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};

/// Result of a raycast hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    /// The position of the voxel that was hit (block coordinates).
    pub position: [i32; 3],
    /// The face normal of the hit (-1, 0, or 1 for each axis).
    pub normal: [i32; 3],
    /// The distance from the ray origin to the hit point.
    pub distance: f32,
    /// The block ID that was hit.
    pub block_id: BlockId,
}

/// Cast a ray through the voxel grid using DDA algorithm.
///
/// # Arguments
/// * `storage` - The chunk storage to query blocks from
/// * `origin` - The starting point of the ray in world coordinates
/// * `direction` - The direction vector (does not need to be normalized)
/// * `max_distance` - Maximum distance to cast the ray (in blocks)
///
/// # Returns
/// Some(RaycastHit) if a solid block was hit, None otherwise
pub fn ray_cast_voxel(
    storage: &ChunkStorage,
    origin: [f32; 3],
    direction: [f32; 3],
    max_distance: f32,
) -> Option<RaycastHit> {
    // Normalize direction
    let len =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    if len < f32::EPSILON {
        return None;
    }

    let dir = [direction[0] / len, direction[1] / len, direction[2] / len];

    // Starting voxel coordinates
    let mut x = origin[0].floor() as i32;
    let mut y = origin[1].floor() as i32;
    let mut z = origin[2].floor() as i32;

    // Step direction for each axis (-1 or 1)
    let step_x = if dir[0] >= 0.0 { 1 } else { -1 };
    let step_y = if dir[1] >= 0.0 { 1 } else { -1 };
    let step_z = if dir[2] >= 0.0 { 1 } else { -1 };

    // Distance to next voxel boundary along each axis
    let t_delta_x = if dir[0].abs() > f32::EPSILON {
        (1.0 / dir[0]).abs()
    } else {
        f32::MAX
    };
    let t_delta_y = if dir[1].abs() > f32::EPSILON {
        (1.0 / dir[1]).abs()
    } else {
        f32::MAX
    };
    let t_delta_z = if dir[2].abs() > f32::EPSILON {
        (1.0 / dir[2]).abs()
    } else {
        f32::MAX
    };

    // Calculate initial t_max values (distance to first voxel boundary)
    let mut t_max_x = if dir[0].abs() > f32::EPSILON {
        let next_boundary = if dir[0] > 0.0 {
            (x + 1) as f32
        } else {
            x as f32
        };
        (next_boundary - origin[0]) / dir[0]
    } else {
        f32::MAX
    };

    let mut t_max_y = if dir[1].abs() > f32::EPSILON {
        let next_boundary = if dir[1] > 0.0 {
            (y + 1) as f32
        } else {
            y as f32
        };
        (next_boundary - origin[1]) / dir[1]
    } else {
        f32::MAX
    };

    let mut t_max_z = if dir[2].abs() > f32::EPSILON {
        let next_boundary = if dir[2] > 0.0 {
            (z + 1) as f32
        } else {
            z as f32
        };
        (next_boundary - origin[2]) / dir[2]
    } else {
        f32::MAX
    };

    // Track which face was hit
    let mut normal = [0, 0, 0];
    let mut distance = 0.0;

    // DDA traversal
    loop {
        // Check if we've exceeded max distance
        if distance > max_distance {
            return None;
        }

        // Check current voxel
        if let Some(block_id) = get_block_at(storage, x, y, z) {
            if block_id != BLOCK_AIR {
                return Some(RaycastHit {
                    position: [x, y, z],
                    normal,
                    distance,
                    block_id,
                });
            }
        }

        // Step to next voxel
        if t_max_x < t_max_y && t_max_x < t_max_z {
            x += step_x;
            distance = t_max_x;
            t_max_x += t_delta_x;
            normal = [-step_x, 0, 0];
        } else if t_max_y < t_max_z {
            y += step_y;
            distance = t_max_y;
            t_max_y += t_delta_y;
            normal = [0, -step_y, 0];
        } else {
            z += step_z;
            distance = t_max_z;
            t_max_z += t_delta_z;
            normal = [0, 0, -step_z];
        }
    }
}

/// Helper function to get a block at world coordinates from ChunkStorage.
fn get_block_at(storage: &ChunkStorage, x: i32, y: i32, z: i32) -> Option<BlockId> {
    // Check Y bounds
    if y < 0 || y >= CHUNK_SIZE_Y as i32 {
        return None;
    }

    // Convert world coordinates to chunk coordinates
    let chunk_x = x.div_euclid(CHUNK_SIZE_X as i32);
    let chunk_z = z.div_euclid(CHUNK_SIZE_Z as i32);

    // Local coordinates within the chunk
    let local_x = x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let local_y = y as usize;
    let local_z = z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

    // Get the chunk
    let chunk_pos = mdminecraft_world::ChunkPos::new(chunk_x, chunk_z);
    let chunk = storage.get(chunk_pos)?;

    Some(chunk.voxel(local_x, local_y, local_z).id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_world::{ChunkPos, Voxel};

    fn create_test_storage_with_block(x: i32, y: i32, z: i32, block_id: BlockId) -> ChunkStorage {
        let mut storage = ChunkStorage::new(10);

        let chunk_x = x.div_euclid(CHUNK_SIZE_X as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE_Z as i32);
        let local_x = x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

        let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
        let chunk = storage.ensure_chunk(chunk_pos);

        let voxel = Voxel {
            id: block_id,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(local_x, local_y, local_z, voxel);

        storage
    }

    #[test]
    fn raycast_hits_block_directly_ahead() {
        let storage = create_test_storage_with_block(5, 5, 5, 1);

        let origin = [0.5, 5.5, 5.5];
        let direction = [1.0, 0.0, 0.0];

        let hit = ray_cast_voxel(&storage, origin, direction, 10.0).expect("should hit block");

        assert_eq!(hit.position, [5, 5, 5]);
        assert_eq!(hit.block_id, 1);
        assert_eq!(hit.normal, [-1, 0, 0]); // Hit from -X side
    }

    #[test]
    fn raycast_returns_none_for_empty_space() {
        let storage = ChunkStorage::new(10);

        let origin = [0.5, 5.5, 5.5];
        let direction = [1.0, 0.0, 0.0];

        let hit = ray_cast_voxel(&storage, origin, direction, 10.0);
        assert!(hit.is_none());
    }

    #[test]
    fn raycast_respects_max_distance() {
        let storage = create_test_storage_with_block(10, 5, 5, 1);

        let origin = [0.5, 5.5, 5.5];
        let direction = [1.0, 0.0, 0.0];

        // Max distance too short to reach block
        let hit = ray_cast_voxel(&storage, origin, direction, 5.0);
        assert!(hit.is_none());

        // Max distance long enough to reach block
        let hit = ray_cast_voxel(&storage, origin, direction, 15.0);
        assert!(hit.is_some());
    }

    #[test]
    fn raycast_detects_correct_face_normal_x() {
        let storage = create_test_storage_with_block(5, 5, 5, 1);

        // Hit from -X side
        let hit =
            ray_cast_voxel(&storage, [0.5, 5.5, 5.5], [1.0, 0.0, 0.0], 10.0).expect("should hit");
        assert_eq!(hit.normal, [-1, 0, 0]);

        // Hit from +X side
        let hit =
            ray_cast_voxel(&storage, [10.5, 5.5, 5.5], [-1.0, 0.0, 0.0], 10.0).expect("should hit");
        assert_eq!(hit.normal, [1, 0, 0]);
    }

    #[test]
    fn raycast_detects_correct_face_normal_y() {
        let storage = create_test_storage_with_block(5, 5, 5, 1);

        // Hit from below
        let hit =
            ray_cast_voxel(&storage, [5.5, 0.5, 5.5], [0.0, 1.0, 0.0], 10.0).expect("should hit");
        assert_eq!(hit.normal, [0, -1, 0]);

        // Hit from above
        let hit =
            ray_cast_voxel(&storage, [5.5, 10.5, 5.5], [0.0, -1.0, 0.0], 10.0).expect("should hit");
        assert_eq!(hit.normal, [0, 1, 0]);
    }

    #[test]
    fn raycast_detects_correct_face_normal_z() {
        let storage = create_test_storage_with_block(5, 5, 5, 1);

        // Hit from -Z side
        let hit =
            ray_cast_voxel(&storage, [5.5, 5.5, 0.5], [0.0, 0.0, 1.0], 10.0).expect("should hit");
        assert_eq!(hit.normal, [0, 0, -1]);

        // Hit from +Z side
        let hit =
            ray_cast_voxel(&storage, [5.5, 5.5, 10.5], [0.0, 0.0, -1.0], 10.0).expect("should hit");
        assert_eq!(hit.normal, [0, 0, 1]);
    }

    #[test]
    fn raycast_works_with_diagonal_ray() {
        let storage = create_test_storage_with_block(5, 5, 5, 1);

        let origin = [0.5, 0.5, 0.5];
        let direction = [1.0, 1.0, 1.0]; // 45-degree diagonal

        let hit = ray_cast_voxel(&storage, origin, direction, 15.0).expect("should hit block");

        assert_eq!(hit.position, [5, 5, 5]);
        assert_eq!(hit.block_id, 1);
    }

    #[test]
    fn raycast_distance_is_reasonable() {
        let storage = create_test_storage_with_block(5, 5, 5, 1);

        let origin = [0.5, 5.5, 5.5];
        let direction = [1.0, 0.0, 0.0];

        let hit = ray_cast_voxel(&storage, origin, direction, 10.0).expect("should hit");

        // Distance should be approximately 4.5 (from 0.5 to 5.0)
        assert!((hit.distance - 4.5).abs() < 0.1);
    }

    #[test]
    fn get_block_at_returns_none_for_out_of_bounds_y() {
        let storage = ChunkStorage::new(10);

        assert!(get_block_at(&storage, 0, -1, 0).is_none());
        assert!(get_block_at(&storage, 0, CHUNK_SIZE_Y as i32, 0).is_none());
        assert!(get_block_at(&storage, 0, 1000, 0).is_none());
    }

    #[test]
    fn get_block_at_returns_none_for_missing_chunk() {
        let storage = ChunkStorage::new(10);
        assert!(get_block_at(&storage, 100, 50, 100).is_none());
    }

    #[test]
    fn get_block_at_handles_negative_coordinates() {
        let mut storage = ChunkStorage::new(10);

        // Set a block in a chunk with negative coordinates
        let chunk_pos = ChunkPos::new(-1, -1);
        let chunk = storage.ensure_chunk(chunk_pos);
        let voxel = Voxel {
            id: 42,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(0, 5, 0, voxel);

        // Block at world position (-16, 5, -16) should be in chunk (-1, -1) at local (0, 5, 0)
        let block_id = get_block_at(&storage, -16, 5, -16);
        assert_eq!(block_id, Some(42));
    }
}
