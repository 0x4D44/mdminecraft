//! Collision primitives and detection for player physics.

use crate::Aabb;

/// A capsule collider represented by a line segment and radius.
/// Used for player collision detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    /// Bottom center point of the capsule.
    pub base: [f32; 3],
    /// Height of the capsule's central axis.
    pub height: f32,
    /// Radius of the capsule.
    pub radius: f32,
}

impl Capsule {
    /// Create a new capsule collider.
    pub fn new(base: [f32; 3], height: f32, radius: f32) -> Self {
        Self {
            base,
            height,
            radius,
        }
    }

    /// Get the top center point of the capsule.
    pub fn top(&self) -> [f32; 3] {
        [self.base[0], self.base[1] + self.height, self.base[2]]
    }

    /// Convert the capsule to a bounding AABB.
    pub fn to_aabb(&self) -> Aabb {
        let min = [
            self.base[0] - self.radius,
            self.base[1],
            self.base[2] - self.radius,
        ];
        let max = [
            self.base[0] + self.radius,
            self.base[1] + self.height + self.radius,
            self.base[2] + self.radius,
        ];
        Aabb::new(min, max)
    }
}

/// Extended AABB methods for collision detection.
impl Aabb {
    /// Get the center point of the AABB.
    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    /// Get the half-extents (half-size) of the AABB.
    pub fn half_extents(&self) -> [f32; 3] {
        [
            (self.max[0] - self.min[0]) * 0.5,
            (self.max[1] - self.min[1]) * 0.5,
            (self.max[2] - self.min[2]) * 0.5,
        ]
    }

    /// Expand the AABB by a given amount in all directions.
    pub fn expand(&self, amount: f32) -> Self {
        Self::new(
            [
                self.min[0] - amount,
                self.min[1] - amount,
                self.min[2] - amount,
            ],
            [
                self.max[0] + amount,
                self.max[1] + amount,
                self.max[2] + amount,
            ],
        )
    }

    /// Translate the AABB by a given offset.
    pub fn translate(&self, offset: [f32; 3]) -> Self {
        Self::new(
            [
                self.min[0] + offset[0],
                self.min[1] + offset[1],
                self.min[2] + offset[2],
            ],
            [
                self.max[0] + offset[0],
                self.max[1] + offset[1],
                self.max[2] + offset[2],
            ],
        )
    }

    /// Create an AABB for a voxel at the given position.
    pub fn from_voxel(x: i32, y: i32, z: i32) -> Self {
        Self::new(
            [x as f32, y as f32, z as f32],
            [(x + 1) as f32, (y + 1) as f32, (z + 1) as f32],
        )
    }

    /// Compute the penetration depth along each axis if intersecting.
    /// Returns None if not intersecting, or Some([x, y, z]) with penetration depths.
    pub fn penetration(&self, other: &Self) -> Option<[f32; 3]> {
        if !self.intersects(other) {
            return None;
        }

        let pen_x = f32::min(self.max[0] - other.min[0], other.max[0] - self.min[0]);
        let pen_y = f32::min(self.max[1] - other.min[1], other.max[1] - self.min[1]);
        let pen_z = f32::min(self.max[2] - other.min[2], other.max[2] - self.min[2]);

        Some([pen_x, pen_y, pen_z])
    }
}

/// Result of a collision resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionResult {
    /// The corrected position after resolving the collision.
    pub position: [f32; 3],
    /// The corrected velocity after applying collision response.
    pub velocity: [f32; 3],
    /// Whether the entity is on the ground after collision resolution.
    pub grounded: bool,
}

/// Resolve collision between a capsule and an AABB, applying sliding response.
/// Returns the adjusted position and velocity.
pub fn resolve_capsule_aabb(capsule: &Capsule, velocity: [f32; 3], aabb: &Aabb) -> CollisionResult {
    let capsule_aabb = capsule.to_aabb();
    let penetration = match capsule_aabb.penetration(aabb) {
        Some(p) => p,
        None => {
            return CollisionResult {
                position: capsule.base,
                velocity,
                grounded: false,
            }
        }
    };

    // Find the axis with minimum penetration (MTV - Minimum Translation Vector)
    let min_pen = penetration[0].min(penetration[1]).min(penetration[2]);

    let mut new_pos = capsule.base;
    let mut new_vel = velocity;
    let mut grounded = false;

    const EPSILON: f32 = 0.001;

    // Resolve along the axis with minimum penetration
    if (penetration[1] - min_pen).abs() < EPSILON {
        // Y-axis collision (vertical)
        // Determine which direction to resolve based on distances
        let dist_base_to_top = (aabb.max[1] - capsule.base[1]).abs();
        let capsule_top = capsule.base[1] + capsule.height + capsule.radius;
        let dist_top_to_bottom = (capsule_top - aabb.min[1]).abs();

        if dist_base_to_top < dist_top_to_bottom {
            // Capsule base is closer to block top - standing on top
            new_pos[1] = aabb.max[1];
            grounded = true;
        } else {
            // Capsule top is closer to block bottom - hitting ceiling
            // Position capsule base so that capsule top (including radius) is just below block bottom
            new_pos[1] = aabb.min[1] - capsule.height - capsule.radius;
        }
        new_vel[1] = 0.0;
    } else if (penetration[0] - min_pen).abs() < EPSILON {
        // X-axis collision
        if capsule.base[0] < aabb.center()[0] {
            new_pos[0] = aabb.min[0] - capsule.radius;
        } else {
            new_pos[0] = aabb.max[0] + capsule.radius;
        }
        new_vel[0] = 0.0;
    } else {
        // Z-axis collision
        if capsule.base[2] < aabb.center()[2] {
            new_pos[2] = aabb.min[2] - capsule.radius;
        } else {
            new_pos[2] = aabb.max[2] + capsule.radius;
        }
        new_vel[2] = 0.0;
    }

    CollisionResult {
        position: new_pos,
        velocity: new_vel,
        grounded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_to_aabb_bounds_correctly() {
        let capsule = Capsule::new([0.0, 0.0, 0.0], 1.8, 0.3);
        let aabb = capsule.to_aabb();

        assert_eq!(aabb.min, [-0.3, 0.0, -0.3]);
        assert_eq!(aabb.max, [0.3, 2.1, 0.3]); // 1.8 height + 0.3 radius
    }

    #[test]
    fn capsule_top_calculates_correctly() {
        let capsule = Capsule::new([1.0, 2.0, 3.0], 1.8, 0.3);
        let top = capsule.top();
        assert_eq!(top, [1.0, 3.8, 3.0]);
    }

    #[test]
    fn aabb_center_calculation() {
        let aabb = Aabb::new([0.0, 0.0, 0.0], [2.0, 4.0, 6.0]);
        let center = aabb.center();
        assert_eq!(center, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn aabb_half_extents() {
        let aabb = Aabb::new([0.0, 0.0, 0.0], [2.0, 4.0, 6.0]);
        let half_ext = aabb.half_extents();
        assert_eq!(half_ext, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn aabb_expand_grows_symmetrically() {
        let aabb = Aabb::new([1.0, 1.0, 1.0], [2.0, 2.0, 2.0]);
        let expanded = aabb.expand(0.5);

        assert_eq!(expanded.min, [0.5, 0.5, 0.5]);
        assert_eq!(expanded.max, [2.5, 2.5, 2.5]);
    }

    #[test]
    fn aabb_translate_moves_correctly() {
        let aabb = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let translated = aabb.translate([5.0, 3.0, -2.0]);

        assert_eq!(translated.min, [5.0, 3.0, -2.0]);
        assert_eq!(translated.max, [6.0, 4.0, -1.0]);
    }

    #[test]
    fn aabb_from_voxel_creates_unit_cube() {
        let aabb = Aabb::from_voxel(2, 3, 4);
        assert_eq!(aabb.min, [2.0, 3.0, 4.0]);
        assert_eq!(aabb.max, [3.0, 4.0, 5.0]);
    }

    #[test]
    fn aabb_penetration_detects_overlap() {
        let a = Aabb::new([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let b = Aabb::new([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]);

        let pen = a.penetration(&b).expect("should intersect");

        // Penetration should be 1.0 on each axis
        assert!((pen[0] - 1.0).abs() < 0.001);
        assert!((pen[1] - 1.0).abs() < 0.001);
        assert!((pen[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn aabb_penetration_none_when_separated() {
        let a = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = Aabb::new([2.0, 0.0, 0.0], [3.0, 1.0, 1.0]);

        assert!(a.penetration(&b).is_none());
    }

    #[test]
    fn resolve_capsule_aabb_no_collision() {
        let capsule = Capsule::new([0.0, 0.0, 0.0], 1.8, 0.3);
        let velocity = [1.0, 0.0, 0.0];
        let aabb = Aabb::new([5.0, 0.0, 0.0], [6.0, 1.0, 1.0]);

        let result = resolve_capsule_aabb(&capsule, velocity, &aabb);

        assert_eq!(result.position, capsule.base);
        assert_eq!(result.velocity, velocity);
        assert!(!result.grounded);
    }

    #[test]
    fn resolve_capsule_aabb_ground_collision() {
        // Capsule slightly overlapping ground block
        let capsule = Capsule::new([0.0, 0.9, 0.0], 1.8, 0.3);
        let velocity = [0.0, -1.0, 0.0]; // falling
        let ground = Aabb::from_voxel(0, 0, 0); // Block at y=0 to y=1

        let result = resolve_capsule_aabb(&capsule, velocity, &ground);

        // Should be placed on top of block
        assert!((result.position[1] - 1.0).abs() < 0.1);
        // Vertical velocity should be zeroed
        assert_eq!(result.velocity[1], 0.0);
        // Should be marked as grounded
        assert!(result.grounded);
    }

    #[test]
    fn resolve_capsule_aabb_wall_collision() {
        // Capsule colliding with a wall on X axis
        let capsule = Capsule::new([0.8, 1.0, 0.0], 1.8, 0.3);
        let velocity = [1.0, 0.0, 0.0]; // moving right
        let wall = Aabb::from_voxel(1, 1, 0); // Block at x=1 to x=2

        let result = resolve_capsule_aabb(&capsule, velocity, &wall);

        // Should be pushed back
        assert!(result.position[0] < 1.0);
        // Horizontal velocity should be zeroed
        assert_eq!(result.velocity[0], 0.0);
        // Should not be grounded (wall collision, not floor)
        assert!(!result.grounded);
    }

    #[test]
    fn resolve_capsule_aabb_ceiling_collision() {
        // Capsule hitting ceiling - positioned in center of block for clear vertical collision
        let capsule = Capsule::new([0.5, 1.5, 0.5], 1.8, 0.3);
        let velocity = [0.0, 1.0, 0.0]; // moving up
        let ceiling = Aabb::from_voxel(0, 3, 0); // Block at y=3 to y=4

        let result = resolve_capsule_aabb(&capsule, velocity, &ceiling);

        // Should be pushed down
        assert!(
            result.position[1] < 1.5,
            "Position Y is {}, expected < 1.5",
            result.position[1]
        );
        // Vertical velocity should be zeroed
        assert_eq!(result.velocity[1], 0.0);
        // Should not be grounded (ceiling, not floor)
        assert!(!result.grounded);
    }

    #[test]
    fn aabb_intersects_existing_test() {
        let a = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = Aabb::new([0.5, 0.5, 0.5], [1.5, 1.5, 1.5]);
        assert!(a.intersects(&b));

        let c = Aabb::new([2.0, 0.0, 0.0], [3.0, 1.0, 1.0]);
        assert!(!a.intersects(&c));
    }
}
