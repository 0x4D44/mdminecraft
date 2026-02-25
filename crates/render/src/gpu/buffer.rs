//! GPU buffer management for vertex, index, and uniform data.
//!
//! This module provides managed GPU buffers with automatic lifecycle tracking.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use wgpu::{util::DeviceExt, Buffer, BufferUsages, Device};

use crate::MeshVertex;

/// Buffer usage flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    /// Vertex buffer.
    Vertex,
    /// Index buffer.
    Index,
    /// Uniform buffer.
    Uniform,
    /// Staging buffer for CPU-to-GPU transfers.
    Staging,
}

/// Handle to a GPU buffer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferHandle {
    id: u64,
}

/// Information about a buffer.
#[derive(Debug, Clone)]
pub struct BufferInfo {
    /// Debug label for the buffer.
    pub label: String,
    /// Buffer size in bytes.
    pub size: u64,
    /// Buffer usage.
    pub usage: BufferUsage,
}

/// Statistics for buffer manager.
#[derive(Debug, Clone, Default)]
pub struct BufferStatistics {
    /// Number of active buffers.
    pub buffer_count: usize,
    /// Total bytes allocated.
    pub total_bytes: u64,
}

struct BufferEntry {
    buffer: Buffer,
    info: BufferInfo,
}

/// Manages GPU buffers and their lifecycle.
pub struct BufferManager {
    device: *const Device,
    buffers: HashMap<u64, BufferEntry>,
    next_id: AtomicU64,
}

// SAFETY: BufferManager owns the buffers and device pointer is read-only
unsafe impl Send for BufferManager {}
unsafe impl Sync for BufferManager {}

impl BufferManager {
    /// Create a new buffer manager.
    pub fn new(device: &Device) -> Self {
        Self {
            device: device as *const Device,
            buffers: HashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Get device reference (safe because device outlives manager).
    fn device(&self) -> &Device {
        unsafe { &*self.device }
    }

    /// Create a new GPU buffer.
    pub fn create_buffer(
        &mut self,
        label: &str,
        data: &[u8],
        usage: BufferUsage,
    ) -> Result<BufferHandle> {
        let wgpu_usage = match usage {
            BufferUsage::Vertex => BufferUsages::VERTEX | BufferUsages::COPY_DST,
            BufferUsage::Index => BufferUsages::INDEX | BufferUsages::COPY_DST,
            BufferUsage::Uniform => BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            BufferUsage::Staging => BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        };

        let buffer = self
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: data,
                usage: wgpu_usage,
            });

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let info = BufferInfo {
            label: label.to_string(),
            size: data.len() as u64,
            usage,
        };

        self.buffers.insert(id, BufferEntry { buffer, info });

        Ok(BufferHandle { id })
    }

    /// Update an existing buffer with new data.
    pub fn update_buffer(&mut self, handle: &BufferHandle, data: &[u8]) -> Result<()> {
        let entry = self
            .buffers
            .get(&handle.id)
            .ok_or_else(|| anyhow::anyhow!("Buffer not found: {}", handle.id))?;

        // For now, recreate the buffer if size doesn't match
        if data.len() as u64 != entry.info.size {
            anyhow::bail!(
                "Buffer size mismatch: expected {}, got {}",
                entry.info.size,
                data.len()
            );
        }

        // Write data using queue (would need queue parameter in real implementation)
        // For now, we'll just fail gracefully
        anyhow::bail!("Buffer updates require queue - use create_buffer instead for now")
    }

    /// Update a range within a buffer.
    pub fn update_buffer_range(
        &mut self,
        handle: &BufferHandle,
        offset: u64,
        data: &[u8],
    ) -> Result<()> {
        let entry = self
            .buffers
            .get(&handle.id)
            .ok_or_else(|| anyhow::anyhow!("Buffer not found: {}", handle.id))?;

        if offset + data.len() as u64 > entry.info.size {
            anyhow::bail!("Buffer range out of bounds");
        }

        anyhow::bail!("Buffer updates require queue - use create_buffer instead for now")
    }

    /// Destroy a buffer and free its resources.
    pub fn destroy_buffer(&mut self, handle: BufferHandle) -> Result<()> {
        self.buffers
            .remove(&handle.id)
            .ok_or_else(|| anyhow::anyhow!("Buffer not found: {}", handle.id))?;
        Ok(())
    }

    /// Get information about a buffer.
    pub fn buffer_info(&self, handle: &BufferHandle) -> Result<BufferInfo> {
        self.buffers
            .get(&handle.id)
            .map(|entry| entry.info.clone())
            .ok_or_else(|| anyhow::anyhow!("Buffer not found: {}", handle.id))
    }

    /// Get statistics about buffer allocations.
    pub fn statistics(&self) -> BufferStatistics {
        let total_bytes = self.buffers.values().map(|e| e.info.size).sum();
        BufferStatistics {
            buffer_count: self.buffers.len(),
            total_bytes,
        }
    }

    /// Clear all buffers.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    /// Upload mesh data to GPU.
    pub fn upload_mesh(&self, vertices: &[MeshVertex], indices: &[u32]) -> Result<GpuMesh> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let vertex_data = bytemuck::cast_slice(vertices);
        let index_data = bytemuck::cast_slice(indices);

        let vertex_buffer = self
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("mesh_{}_vertices", id)),
                contents: vertex_data,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });

        let index_buffer = self
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("mesh_{}_indices", id)),
                contents: index_data,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });

        Ok(GpuMesh {
            vertex_buffer,
            index_buffer,
            vertex_count: vertices.len(),
            index_count: indices.len(),
            id,
        })
    }

    /// Free a mesh's GPU resources.
    pub fn free_mesh(&self, _mesh_id: u64) {
        // Buffers are automatically dropped when GpuMesh is dropped
        // This method exists for symmetry with upload_mesh
    }
}

/// GPU mesh with vertex and index buffers.
pub struct GpuMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_count: usize,
    index_count: usize,
    id: u64,
}

impl GpuMesh {
    /// Get vertex buffer.
    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    /// Get index buffer.
    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    /// Get vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Get index count.
    pub fn index_count(&self) -> usize {
        self.index_count
    }

    /// Get mesh ID.
    pub fn id(&self) -> u64 {
        self.id
    }
}
