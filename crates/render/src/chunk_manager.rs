//! Chunk mesh buffer management for multi-chunk rendering.

use std::collections::HashMap;

use crate::mesh::{MeshBuffers, MeshVertex};
use mdminecraft_world::{ChunkPos, CHUNK_SIZE_Y, WORLD_MIN_Y};

/// GPU buffer for a chunk mesh with position.
pub struct ChunkRenderData {
    /// Vertex buffer
    pub vertex_buffer: wgpu::Buffer,
    /// Opaque+cutout index buffer (triangle list).
    pub opaque_index_buffer: Option<wgpu::Buffer>,
    /// Number of opaque indices to draw.
    pub opaque_index_count: u32,
    /// Alpha-blended index buffer (triangle list).
    pub alpha_index_buffer: Option<wgpu::Buffer>,
    /// Number of alpha indices to draw.
    pub alpha_index_count: u32,
    /// Chunk position in world
    pub chunk_pos: ChunkPos,
    /// Bind group for chunk uniforms
    pub chunk_bind_group: wgpu::BindGroup,
}

struct BufferPool {
    vertex_buffers: Vec<wgpu::Buffer>,
    index_buffers: Vec<wgpu::Buffer>,
}

impl BufferPool {
    fn new() -> Self {
        Self {
            vertex_buffers: Vec::new(),
            index_buffers: Vec::new(),
        }
    }

    fn acquire(
        &mut self,
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
        label: &str,
    ) -> wgpu::Buffer {
        // Find best fit buffer (first one large enough)
        let list = if usage.contains(wgpu::BufferUsages::VERTEX) {
            &mut self.vertex_buffers
        } else {
            &mut self.index_buffers
        };

        if let Some(idx) = list.iter().position(|b| b.size() >= size) {
            return list.swap_remove(idx);
        }

        // No suitable buffer found, create new one
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST, // Ensure we can write to it
            mapped_at_creation: false,
        })
    }

    fn release(&mut self, buffer: wgpu::Buffer) {
        if buffer.usage().contains(wgpu::BufferUsages::VERTEX) {
            self.vertex_buffers.push(buffer);
        } else {
            self.index_buffers.push(buffer);
        }
    }
}

/// Manages rendering data for multiple chunks.
pub struct ChunkManager {
    chunks: HashMap<ChunkPos, ChunkRenderData>,
    pool: BufferPool,
}

impl ChunkManager {
    /// Create a new empty chunk manager.
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            pool: BufferPool::new(),
        }
    }

    /// Add or update a chunk's mesh.
    pub fn add_chunk(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh_buffers: &MeshBuffers,
        chunk_pos: ChunkPos,
        chunk_bind_group: wgpu::BindGroup,
    ) {
        // Reuse old buffers if replacing
        if let Some(old) = self.chunks.remove(&chunk_pos) {
            self.pool.release(old.vertex_buffer);
            if let Some(buffer) = old.opaque_index_buffer {
                self.pool.release(buffer);
            }
            if let Some(buffer) = old.alpha_index_buffer {
                self.pool.release(buffer);
            }
        }

        let vertex_size = (mesh_buffers.vertices.len() * std::mem::size_of::<MeshVertex>()) as u64;
        let opaque_index_size =
            (mesh_buffers.indices_opaque.len() * std::mem::size_of::<u32>()) as u64;
        let alpha_index_size =
            (mesh_buffers.indices_alpha.len() * std::mem::size_of::<u32>()) as u64;

        if vertex_size == 0 || (opaque_index_size == 0 && alpha_index_size == 0) {
            return; // Don't add empty meshes
        }

        let vertex_buffer = self.pool.acquire(
            device,
            vertex_size,
            wgpu::BufferUsages::VERTEX,
            "Chunk Vertex Buffer",
        );
        queue.write_buffer(
            &vertex_buffer,
            0,
            bytemuck::cast_slice(&mesh_buffers.vertices),
        );

        let opaque_index_buffer = if opaque_index_size != 0 {
            let buffer = self.pool.acquire(
                device,
                opaque_index_size,
                wgpu::BufferUsages::INDEX,
                "Chunk Opaque Index Buffer",
            );
            queue.write_buffer(
                &buffer,
                0,
                bytemuck::cast_slice(&mesh_buffers.indices_opaque),
            );
            Some(buffer)
        } else {
            None
        };

        let alpha_index_buffer = if alpha_index_size != 0 {
            let buffer = self.pool.acquire(
                device,
                alpha_index_size,
                wgpu::BufferUsages::INDEX,
                "Chunk Alpha Index Buffer",
            );
            queue.write_buffer(
                &buffer,
                0,
                bytemuck::cast_slice(&mesh_buffers.indices_alpha),
            );
            Some(buffer)
        } else {
            None
        };

        let render_data = ChunkRenderData {
            vertex_buffer,
            opaque_index_buffer,
            opaque_index_count: mesh_buffers.indices_opaque.len() as u32,
            alpha_index_buffer,
            alpha_index_count: mesh_buffers.indices_alpha.len() as u32,
            chunk_pos,
            chunk_bind_group,
        };

        self.chunks.insert(chunk_pos, render_data);
    }

    /// Remove a chunk.
    pub fn remove_chunk(&mut self, chunk_pos: &ChunkPos) -> bool {
        if let Some(data) = self.chunks.remove(chunk_pos) {
            self.pool.release(data.vertex_buffer);
            if let Some(buffer) = data.opaque_index_buffer {
                self.pool.release(buffer);
            }
            if let Some(buffer) = data.alpha_index_buffer {
                self.pool.release(buffer);
            }
            true
        } else {
            false
        }
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
        for (_, data) in self.chunks.drain() {
            self.pool.release(data.vertex_buffer);
            if let Some(buffer) = data.opaque_index_buffer {
                self.pool.release(buffer);
            }
            if let Some(buffer) = data.alpha_index_buffer {
                self.pool.release(buffer);
            }
        }
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
        // Chunks are 16×CHUNK_SIZE_Y×16 blocks in world coordinates
        const CHUNK_SIZE_XZ: f32 = 16.0;
        const CHUNK_SIZE_Y_F32: f32 = CHUNK_SIZE_Y as f32;

        let min = glam::Vec3::new(
            (chunk_pos.x * 16) as f32,
            WORLD_MIN_Y as f32,
            (chunk_pos.z * 16) as f32,
        );

        let max = glam::Vec3::new(
            min.x + CHUNK_SIZE_XZ,
            min.y + CHUNK_SIZE_Y_F32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{MeshBuffers, MeshHash, MeshVertex};

    fn test_device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))
        .expect("adapter");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device")
    }

    fn test_bind_group(device: &wgpu::Device) -> wgpu::BindGroup {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Chunk Test Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chunk Test Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Test Bind Group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    #[test]
    fn chunk_manager_adds_and_removes_chunks() {
        let (device, queue) = test_device();
        let mut manager = ChunkManager::new();

        let vertex = MeshVertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            block_id: 0,
            light: 0,
            extra: 0,
        };
        let mesh = MeshBuffers {
            vertices: vec![vertex],
            indices_opaque: vec![0],
            indices_alpha: Vec::new(),
            hash: MeshHash([0; 32]),
        };

        let bind_group = test_bind_group(&device);
        let pos = ChunkPos::new(0, 0);
        manager.add_chunk(&device, &queue, &mesh, pos, bind_group);
        assert_eq!(manager.chunk_count(), 1);

        assert!(manager.remove_chunk(&pos));
        assert_eq!(manager.chunk_count(), 0);
        assert!(!manager.remove_chunk(&pos));
    }

    #[test]
    fn chunk_manager_clear_releases_all() {
        let (device, queue) = test_device();
        let mut manager = ChunkManager::new();
        let vertex = MeshVertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            block_id: 0,
            light: 0,
            extra: 0,
        };
        let mesh = MeshBuffers {
            vertices: vec![vertex],
            indices_opaque: vec![0],
            indices_alpha: Vec::new(),
            hash: MeshHash([0; 32]),
        };

        let bind_group = test_bind_group(&device);
        manager.add_chunk(&device, &queue, &mesh, ChunkPos::new(0, 0), bind_group);
        assert_eq!(manager.chunk_count(), 1);
        manager.clear();
        assert_eq!(manager.chunk_count(), 0);
    }

    #[test]
    fn frustum_includes_nearby_chunk() {
        let vp = glam::Mat4::orthographic_rh(-100.0, 100.0, -100.0, 100.0, -100.0, 100.0);
        let frustum = Frustum::from_matrix(&vp);
        assert!(frustum.is_chunk_visible(ChunkPos::new(0, 0)));
        assert!(!frustum.is_chunk_visible(ChunkPos::new(100, 100)));
    }
}
