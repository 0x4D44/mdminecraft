#![warn(missing_docs)]
//! Physics primitives (AABB, collisions, etc.).

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_intersects_when_overlapping() {
        let a = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = Aabb::new([0.5, 0.5, 0.5], [1.5, 1.5, 1.5]);
        assert!(a.intersects(&b));
    }

    #[test]
    fn aabb_intersects_on_touching_faces() {
        let a = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = Aabb::new([1.0, 0.25, 0.25], [2.0, 0.75, 0.75]);
        assert!(a.intersects(&b));
    }

    #[test]
    fn aabb_does_not_intersect_when_separated() {
        let a = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = Aabb::new([1.1, 0.0, 0.0], [2.0, 1.0, 1.0]);
        assert!(!a.intersects(&b));
    }
}
