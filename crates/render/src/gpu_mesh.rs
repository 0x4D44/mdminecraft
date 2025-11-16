use wgpu::util::DeviceExt;
use crate::mesh::{MeshBuffers, MeshVertex};

/// GPU-side representation of a chunk mesh.
pub struct GpuMesh {
    /// Vertex buffer on GPU.
    pub vertex_buffer: wgpu::Buffer,
    /// Index buffer on GPU.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw.
    pub index_count: u32,
}

impl GpuMesh {
    /// Upload a mesh to the GPU.
    pub fn from_mesh_buffers(device: &wgpu::Device, mesh: &MeshBuffers) -> Self {
        // Convert MeshVertex to bytes for GPU upload
        let vertex_data = bytemuck::cast_slice(&mesh.vertices);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Vertex Buffer"),
            contents: vertex_data,
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_data = bytemuck::cast_slice(&mesh.indices);
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Index Buffer"),
            contents: index_data,
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        }
    }
}

// Make MeshVertex compatible with bytemuck for GPU upload
unsafe impl bytemuck::Pod for MeshVertex {}
unsafe impl bytemuck::Zeroable for MeshVertex {}
