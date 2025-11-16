//! 4D mathematics for hypercubic voxel worlds.
//!
//! This module provides 4D vector types, chunk positions, and utilities
//! for working with 4-dimensional voxel worlds that are rendered as
//! 3D slices (cross-sections through the 4th dimension).

use glam::{Vec3, Vec4 as GlamVec4};

/// A 4D floating-point vector with x, y, z, w components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    /// Create a new 4D vector.
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Zero vector (0, 0, 0, 0).
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// Unit vector along X axis (1, 0, 0, 0).
    pub const X: Self = Self::new(1.0, 0.0, 0.0, 0.0);

    /// Unit vector along Y axis (0, 1, 0, 0).
    pub const Y: Self = Self::new(0.0, 1.0, 0.0, 0.0);

    /// Unit vector along Z axis (0, 0, 1, 0).
    pub const Z: Self = Self::new(0.0, 0.0, 1.0, 0.0);

    /// Unit vector along W axis (0, 0, 0, 1).
    pub const W: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    /// Calculate dot product with another 4D vector.
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    /// Calculate length (magnitude) of the vector.
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Normalize the vector to unit length.
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Self::new(self.x / len, self.y / len, self.z / len, self.w / len)
        } else {
            Self::ZERO
        }
    }

    /// Extract 3D slice at a specific W coordinate (for rendering).
    ///
    /// Returns the xyz components, treating w as a "slice level".
    pub fn to_vec3_slice(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    /// Convert to array [x, y, z, w].
    pub fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    /// Convert from glam Vec4.
    pub fn from_glam(v: GlamVec4) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }

    /// Convert to glam Vec4.
    pub fn to_glam(self) -> GlamVec4 {
        GlamVec4::new(self.x, self.y, self.z, self.w)
    }
}

impl std::ops::Add for Vec4 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(
            self.x + other.x,
            self.y + other.y,
            self.z + other.z,
            self.w + other.w,
        )
    }
}

impl std::ops::Sub for Vec4 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(
            self.x - other.x,
            self.y - other.y,
            self.z - other.z,
            self.w - other.w,
        )
    }
}

impl std::ops::Mul<f32> for Vec4 {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Self::new(
            self.x * scalar,
            self.y * scalar,
            self.z * scalar,
            self.w * scalar,
        )
    }
}

/// A 4D chunk position with integer coordinates (cx, cy, cz, cw).
///
/// In a 4D voxel world, chunks are 16×16×16×16 hypercubes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos4D {
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
    pub cw: i32,
}

impl ChunkPos4D {
    /// Create a new 4D chunk position.
    pub const fn new(cx: i32, cy: i32, cz: i32, cw: i32) -> Self {
        Self { cx, cy, cz, cw }
    }

    /// Origin chunk (0, 0, 0, 0).
    pub const ZERO: Self = Self::new(0, 0, 0, 0);

    /// Convert world position to chunk position.
    ///
    /// Assumes chunks are 16×16×16×16 blocks.
    pub fn from_world_pos(x: i32, y: i32, z: i32, w: i32) -> Self {
        Self::new(
            x.div_euclid(16),
            y.div_euclid(16),
            z.div_euclid(16),
            w.div_euclid(16),
        )
    }

    /// Get world position of chunk origin (minimum corner).
    pub fn to_world_pos(self) -> (i32, i32, i32, i32) {
        (self.cx * 16, self.cy * 16, self.cz * 16, self.cw * 16)
    }

    /// Extract 3D chunk position at a specific W slice.
    ///
    /// Returns (cx, cy, cz) for rendering the world at chunk layer cw.
    pub fn to_3d_slice(self) -> (i32, i32, i32) {
        (self.cx, self.cy, self.cz)
    }

    /// Create from 3D chunk position and W coordinate.
    pub fn from_3d_slice(cx: i32, cy: i32, cz: i32, cw: i32) -> Self {
        Self::new(cx, cy, cz, cw)
    }

    /// Calculate Manhattan distance to another chunk.
    pub fn manhattan_distance(self, other: Self) -> i32 {
        (self.cx - other.cx).abs()
            + (self.cy - other.cy).abs()
            + (self.cz - other.cz).abs()
            + (self.cw - other.cw).abs()
    }
}

/// Utilities for working with 4D to 3D slice extraction.
pub mod slice {
    use super::*;

    /// Extract a 3D slice from a 4D position at a given W coordinate.
    ///
    /// This is used to render a 3D cross-section of the 4D world.
    /// The W coordinate determines which "layer" of the 4D world is visible.
    pub fn extract_3d_slice(pos_4d: Vec4, _w_slice: f32) -> Vec3 {
        // For now, simply project out the W coordinate
        // In the future, this could interpolate between adjacent W slices
        pos_4d.to_vec3_slice()
    }

    /// Check if a 4D chunk is visible in the current W slice.
    ///
    /// Returns true if the chunk's W coordinate matches the slice level.
    pub fn is_chunk_in_slice(chunk_4d: ChunkPos4D, w_slice: i32) -> bool {
        chunk_4d.cw == w_slice
    }

    /// Get all chunks in a 3D slice at a specific W coordinate,
    /// within a given XZ radius and Y range.
    pub fn chunks_in_slice(
        w_slice: i32,
        center_x: i32,
        center_z: i32,
        xz_radius: i32,
        y_min: i32,
        y_max: i32,
    ) -> Vec<ChunkPos4D> {
        let mut chunks = Vec::new();

        for cx in (center_x - xz_radius)..=(center_x + xz_radius) {
            for cz in (center_z - xz_radius)..=(center_z + xz_radius) {
                for cy in y_min..=y_max {
                    chunks.push(ChunkPos4D::new(cx, cy, cz, w_slice));
                }
            }
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec4_basic_operations() {
        let v1 = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let v2 = Vec4::new(5.0, 6.0, 7.0, 8.0);

        let sum = v1 + v2;
        assert_eq!(sum, Vec4::new(6.0, 8.0, 10.0, 12.0));

        let diff = v2 - v1;
        assert_eq!(diff, Vec4::new(4.0, 4.0, 4.0, 4.0));

        let scaled = v1 * 2.0;
        assert_eq!(scaled, Vec4::new(2.0, 4.0, 6.0, 8.0));
    }

    #[test]
    fn vec4_dot_product() {
        let v1 = Vec4::new(1.0, 0.0, 0.0, 0.0);
        let v2 = Vec4::new(0.0, 1.0, 0.0, 0.0);
        assert_eq!(v1.dot(v2), 0.0);

        let v3 = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let v4 = Vec4::new(4.0, 3.0, 2.0, 1.0);
        assert_eq!(v3.dot(v4), 20.0); // 1*4 + 2*3 + 3*2 + 4*1
    }

    #[test]
    fn chunk_pos_conversion() {
        let chunk = ChunkPos4D::new(2, 3, 4, 5);
        let (x, y, z, w) = chunk.to_world_pos();
        assert_eq!((x, y, z, w), (32, 48, 64, 80));

        let chunk2 = ChunkPos4D::from_world_pos(35, 50, 67, 82);
        assert_eq!(chunk2, ChunkPos4D::new(2, 3, 4, 5));
    }

    #[test]
    fn chunk_3d_slice() {
        let chunk_4d = ChunkPos4D::new(1, 2, 3, 5);
        let (cx, cy, cz) = chunk_4d.to_3d_slice();
        assert_eq!((cx, cy, cz), (1, 2, 3));

        assert!(slice::is_chunk_in_slice(chunk_4d, 5));
        assert!(!slice::is_chunk_in_slice(chunk_4d, 4));
    }
}
