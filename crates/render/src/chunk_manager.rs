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

/// Frustum culling helper.
pub struct Frustum {
    // Placeholder for now - will implement proper frustum planes later
    _dummy: (),
}

impl Frustum {
    /// Create frustum from view-projection matrix.
    pub fn from_matrix(_vp_matrix: &glam::Mat4) -> Self {
        // TODO: Extract frustum planes from matrix
        Self { _dummy: () }
    }

    /// Check if a chunk is visible (placeholder - always returns true for now).
    pub fn is_chunk_visible(&self, _chunk_pos: ChunkPos) -> bool {
        // TODO: Implement proper AABB vs frustum test
        true
    }
}
