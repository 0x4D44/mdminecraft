#![warn(missing_docs)]
//! Physics primitives (AABB, collisions, raycast, etc.).

use glam::Vec3;

/// Axis-aligned bounding box used for collisions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Minimum corner (x, y, z).
    pub min: [f32; 3],
    /// Maximum corner (x, y, z).
    pub max: [f32; 3],
}

impl Aabb {
    /// Create a new AABB ensuring min <= max per axis.
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        debug_assert!(min[0] <= max[0] && min[1] <= max[1] && min[2] <= max[2]);
        Self { min, max }
    }

    /// Tests intersection with another AABB.
    pub fn intersects(&self, other: &Self) -> bool {
        self.min[0] <= other.max[0]
            && self.max[0] >= other.min[0]
            && self.min[1] <= other.max[1]
            && self.max[1] >= other.min[1]
            && self.min[2] <= other.max[2]
            && self.max[2] >= other.min[2]
    }
}

/// Result of a voxel raycast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    /// Position of the hit block (integer coordinates).
    pub block_pos: [i32; 3],
    /// Face normal of the hit (which face was hit).
    pub normal: [i32; 3],
    /// Distance from ray origin to hit point.
    pub distance: f32,
}

/// Performs a voxel raycast using DDA (Digital Differential Analyzer) algorithm.
///
/// # Arguments
/// * `origin` - Ray starting position
/// * `direction` - Ray direction (should be normalized)
/// * `max_distance` - Maximum ray travel distance
/// * `is_solid` - Function to test if a block is solid
///
/// # Returns
/// Some(RaycastHit) if a solid block was hit, None otherwise.
pub fn raycast_voxel<F>(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    is_solid: F,
) -> Option<RaycastHit>
where
    F: Fn(i32, i32, i32) -> bool,
{
    // Normalize direction
    let dir = direction.normalize();

    // Current voxel position (integer)
    let mut vx = origin.x.floor() as i32;
    let mut vy = origin.y.floor() as i32;
    let mut vz = origin.z.floor() as i32;

    // Step direction (-1, 0, or 1)
    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };

    // Distance to next voxel boundary along each axis
    let t_delta_x = if dir.x != 0.0 { (1.0 / dir.x).abs() } else { f32::MAX };
    let t_delta_y = if dir.y != 0.0 { (1.0 / dir.y).abs() } else { f32::MAX };
    let t_delta_z = if dir.z != 0.0 { (1.0 / dir.z).abs() } else { f32::MAX };

    // Calculate initial t_max (distance to next voxel boundary)
    let mut t_max_x = if dir.x > 0.0 {
        ((vx + 1) as f32 - origin.x) / dir.x
    } else if dir.x < 0.0 {
        (origin.x - vx as f32) / -dir.x
    } else {
        f32::MAX
    };

    let mut t_max_y = if dir.y > 0.0 {
        ((vy + 1) as f32 - origin.y) / dir.y
    } else if dir.y < 0.0 {
        (origin.y - vy as f32) / -dir.y
    } else {
        f32::MAX
    };

    let mut t_max_z = if dir.z > 0.0 {
        ((vz + 1) as f32 - origin.z) / dir.z
    } else if dir.z < 0.0 {
        (origin.z - vz as f32) / -dir.z
    } else {
        f32::MAX
    };

    // Track which face we hit
    let mut normal = [0, 0, 0];
    let mut distance = 0.0;

    // March through voxels
    for _ in 0..200 {  // Max iterations to prevent infinite loop
        // Check if current voxel is solid
        if is_solid(vx, vy, vz) {
            return Some(RaycastHit {
                block_pos: [vx, vy, vz],
                normal,
                distance,
            });
        }

        // Check if we've exceeded max distance
        if distance > max_distance {
            return None;
        }

        // Step to next voxel along axis with smallest t_max
        if t_max_x < t_max_y && t_max_x < t_max_z {
            vx += step_x;
            distance = t_max_x;
            t_max_x += t_delta_x;
            normal = [-step_x, 0, 0];
        } else if t_max_y < t_max_z {
            vy += step_y;
            distance = t_max_y;
            t_max_y += t_delta_y;
            normal = [0, -step_y, 0];
        } else {
            vz += step_z;
            distance = t_max_z;
            t_max_z += t_delta_z;
            normal = [0, 0, -step_z];
        }
    }

    None
}
