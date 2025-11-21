//! Chunk mesh buffer management for multi-chunk rendering.

use std::collections::HashMap;
use wgpu::util::DeviceExt;

use crate::mesh::MeshBuffers;
use mdminecraft_world::ChunkPos;

/// GPU buffer for a chunk mesh with position.
pub struct ChunkRenderData {
    /// Vertex buffer
    pub vertex_buffer: wgpu::Buffer,
    /// Index buffer
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw
    pub index_count: u32,
    /// Chunk position in world
    pub chunk_pos: ChunkPos,
    /// Bind group for chunk uniforms
    pub chunk_bind_group: wgpu::BindGroup,
}

impl ChunkRenderData {
    /// Create chunk render data from mesh buffers.
    pub fn new(
        device: &wgpu::Device,
        mesh: &MeshBuffers,
        chunk_pos: ChunkPos,
        chunk_bind_group: wgpu::BindGroup,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            chunk_pos,
            chunk_bind_group,
        }
    }
}

/// Manages rendering data for multiple chunks.
pub struct ChunkManager {
    chunks: HashMap<ChunkPos, ChunkRenderData>,
}

impl ChunkManager {
    /// Create a new empty chunk manager.
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    /// Add or update a chunk's mesh.
    pub fn add_chunk(
        &mut self,
        device: &wgpu::Device,
        mesh: &MeshBuffers,
        chunk_pos: ChunkPos,
        chunk_bind_group: wgpu::BindGroup,
    ) {
        let render_data = ChunkRenderData::new(device, mesh, chunk_pos, chunk_bind_group);
        self.chunks.insert(chunk_pos, render_data);
    }

    /// Remove a chunk.
    pub fn remove_chunk(&mut self, chunk_pos: &ChunkPos) -> bool {
        self.chunks.remove(chunk_pos).is_some()
    }

    /// Get all chunks for rendering.
    pub fn chunks(&self) -> impl Iterator<Item = &ChunkRenderData> {
        self.chunks.values()
    }

    /// Get number of loaded chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Clear all chunks.
    pub fn clear(&mut self) {
        self.chunks.clear();
    }
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Frustum culling helper using 6 planes extracted from view-projection matrix.
pub struct Frustum {
    /// Six frustum planes: [left, right, bottom, top, near, far]
    /// Each plane is a Vec4 where xyz is the normal and w is the distance
    planes: [glam::Vec4; 6],
}

impl Frustum {
    /// Create frustum from view-projection matrix.
    ///
    /// Extracts the 6 frustum planes using the Gribb-Hartmann method.
    pub fn from_matrix(vp_matrix: &glam::Mat4) -> Self {
        let m = vp_matrix.to_cols_array();

        // Extract planes from matrix rows
        // Each plane equation is: Ax + By + Cz + D = 0
        // Represented as Vec4(A, B, C, D) where (A,B,C) is normal

        let left =
            glam::Vec4::new(m[3] + m[0], m[7] + m[4], m[11] + m[8], m[15] + m[12]).normalize();

        let right =
            glam::Vec4::new(m[3] - m[0], m[7] - m[4], m[11] - m[8], m[15] - m[12]).normalize();

        let bottom =
            glam::Vec4::new(m[3] + m[1], m[7] + m[5], m[11] + m[9], m[15] + m[13]).normalize();

        let top =
            glam::Vec4::new(m[3] - m[1], m[7] - m[5], m[11] - m[9], m[15] - m[13]).normalize();

        let near =
            glam::Vec4::new(m[3] + m[2], m[7] + m[6], m[11] + m[10], m[15] + m[14]).normalize();

        let far =
            glam::Vec4::new(m[3] - m[2], m[7] - m[6], m[11] - m[10], m[15] - m[14]).normalize();

        Self {
            planes: [left, right, bottom, top, near, far],
        }
    }

    /// Check if a chunk is visible using AABB vs frustum test.
    ///
    /// Returns true if the chunk's bounding box intersects or is inside the frustum.
    pub fn is_chunk_visible(&self, chunk_pos: ChunkPos) -> bool {
        // Define chunk AABB (axis-aligned bounding box)
        // Chunks are 16x256x16 blocks in world coordinates
        const CHUNK_SIZE_XZ: f32 = 16.0;
        const CHUNK_SIZE_Y: f32 = 256.0;

        let min = glam::Vec3::new((chunk_pos.x * 16) as f32, 0.0, (chunk_pos.z * 16) as f32);

        let max = glam::Vec3::new(
            min.x + CHUNK_SIZE_XZ,
            min.y + CHUNK_SIZE_Y,
            min.z + CHUNK_SIZE_XZ,
        );

        // Test AABB against each frustum plane
        for plane in &self.planes {
            let normal = plane.truncate();
            let d = plane.w;

            // Find the positive vertex (p-vertex) - the vertex of the AABB
            // most aligned with the plane normal
            let p = glam::Vec3::new(
                if normal.x >= 0.0 { max.x } else { min.x },
                if normal.y >= 0.0 { max.y } else { min.y },
                if normal.z >= 0.0 { max.z } else { min.z },
            );

            // If the p-vertex is outside this plane, the entire AABB is outside
            if normal.dot(p) + d < 0.0 {
                return false;
            }
        }

        // AABB intersects or is inside frustum
        true
    }
}
