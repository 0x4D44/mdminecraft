//! Voxel raycasting using DDA (Digital Differential Analyzer) algorithm.

use glam::{IVec3, Vec3};

/// Result of a raycast against the voxel world.
#[derive(Debug, Clone, Copy)]
pub struct RaycastHit {
    /// The position of the block that was hit (in block coordinates).
    pub block_pos: IVec3,
    /// The normal of the face that was hit.
    pub face_normal: IVec3,
    /// The distance from the ray origin to the hit point.
    pub distance: f32,
    /// World-space position of the hit point.
    pub hit_pos: Vec3,
}

/// Performs a DDA raycast through the voxel world.
///
/// # Arguments
/// * `origin` - Ray origin in world coordinates
/// * `direction` - Ray direction (should be normalized)
/// * `max_distance` - Maximum distance to cast the ray
/// * `is_solid` - Function that returns true if a block at the given position is solid
///
/// # Returns
/// Some(RaycastHit) if a solid block was hit, None otherwise
pub fn raycast<F>(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut is_solid: F,
) -> Option<RaycastHit>
where
    F: FnMut(IVec3) -> bool,
{
    // Current voxel position
    let mut voxel = IVec3::new(
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );

    // Direction to step in each axis (-1, 0, or 1)
    let step = IVec3::new(
        if direction.x > 0.0 { 1 } else { -1 },
        if direction.y > 0.0 { 1 } else { -1 },
        if direction.z > 0.0 { 1 } else { -1 },
    );

    // Distance along ray to cross one voxel boundary in each axis
    let delta = Vec3::new(
        if direction.x != 0.0 {
            (1.0 / direction.x).abs()
        } else {
            f32::MAX
        },
        if direction.y != 0.0 {
            (1.0 / direction.y).abs()
        } else {
            f32::MAX
        },
        if direction.z != 0.0 {
            (1.0 / direction.z).abs()
        } else {
            f32::MAX
        },
    );

    // Distance from origin to next voxel boundary in each axis
    let mut t_max = Vec3::new(
        if direction.x != 0.0 {
            if direction.x > 0.0 {
                ((voxel.x + 1) as f32 - origin.x) / direction.x
            } else {
                (voxel.x as f32 - origin.x) / direction.x
            }
        } else {
            f32::MAX
        },
        if direction.y != 0.0 {
            if direction.y > 0.0 {
                ((voxel.y + 1) as f32 - origin.y) / direction.y
            } else {
                (voxel.y as f32 - origin.y) / direction.y
            }
        } else {
            f32::MAX
        },
        if direction.z != 0.0 {
            if direction.z > 0.0 {
                ((voxel.z + 1) as f32 - origin.z) / direction.z
            } else {
                (voxel.z as f32 - origin.z) / direction.z
            }
        } else {
            f32::MAX
        },
    );

    // Current face normal (which face we entered the voxel from)
    let mut face_normal = IVec3::ZERO;

    // Traverse voxels using DDA
    let max_steps = (max_distance * 2.0) as i32; // Rough upper bound
    for _ in 0..max_steps {
        // Check if current voxel is solid
        if is_solid(voxel) {
            let distance = t_max.min_element() - delta.min_element();
            let hit_pos = origin + direction * distance;
            return Some(RaycastHit {
                block_pos: voxel,
                face_normal,
                distance,
                hit_pos,
            });
        }

        // Step to next voxel
        if t_max.x < t_max.y && t_max.x < t_max.z {
            // Step in X
            voxel.x += step.x;
            t_max.x += delta.x;
            face_normal = IVec3::new(-step.x, 0, 0);
        } else if t_max.y < t_max.z {
            // Step in Y
            voxel.y += step.y;
            t_max.y += delta.y;
            face_normal = IVec3::new(0, -step.y, 0);
        } else {
            // Step in Z
            voxel.z += step.z;
            t_max.z += delta.z;
            face_normal = IVec3::new(0, 0, -step.z);
        }

        // Check if we've exceeded max distance
        if t_max.min_element() > max_distance {
            break;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raycast_hit() {
        // Simple test: ray from (0,0,0) towards (1,0,0), block at (5,0,0)
        let origin = Vec3::new(0.5, 0.5, 0.5);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        let max_distance = 10.0;

        let is_solid = |pos: IVec3| pos == IVec3::new(5, 0, 0);

        let hit = raycast(origin, direction, max_distance, is_solid);
        assert!(hit.is_some());

        let hit = hit.unwrap();
        assert_eq!(hit.block_pos, IVec3::new(5, 0, 0));
        assert_eq!(hit.face_normal, IVec3::new(-1, 0, 0)); // Hit from -X side
    }

    #[test]
    fn test_raycast_miss() {
        // Ray that doesn't hit anything
        let origin = Vec3::new(0.5, 0.5, 0.5);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        let max_distance = 10.0;

        let is_solid = |_: IVec3| false; // No solid blocks

        let hit = raycast(origin, direction, max_distance, is_solid);
        assert!(hit.is_none());
    }

    #[test]
    fn test_raycast_max_distance() {
        // Block beyond max distance
        let origin = Vec3::new(0.5, 0.5, 0.5);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        let max_distance = 3.0;

        let is_solid = |pos: IVec3| pos == IVec3::new(5, 0, 0);

        let hit = raycast(origin, direction, max_distance, is_solid);
        assert!(hit.is_none()); // Block at x=5 is beyond max_distance=3
    }
}
