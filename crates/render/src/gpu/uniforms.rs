//! Uniform buffer management for shader data.
//!
//! This module provides structs and utilities for managing uniform data
//! that is passed to shaders during rendering.

use anyhow::{Context, Result};
use wgpu::{BindGroup, BindGroupLayout, Buffer, Device, Queue};

/// View and projection matrices combined for vertex transformation.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewProjectionUniforms {
    /// Combined view-projection matrix (projection * view).
    pub view_proj: [[f32; 4]; 4],
}

impl ViewProjectionUniforms {
    /// Create identity view-projection uniforms.
    pub fn identity() -> Self {
        Self {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create from separate view and projection matrices.
    pub fn from_view_projection(view: [[f32; 4]; 4], projection: [[f32; 4]; 4]) -> Self {
        Self {
            view_proj: multiply_matrices(projection, view),
        }
    }
}

/// Camera position and direction uniforms.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniforms {
    /// Camera position in world space.
    pub position: [f32; 3],
    /// Padding for alignment.
    pub _padding1: f32,
    /// Camera forward direction (normalized).
    pub direction: [f32; 3],
    /// Padding for alignment.
    pub _padding2: f32,
}

impl CameraUniforms {
    /// Create default camera uniforms.
    pub fn new(position: [f32; 3], direction: [f32; 3]) -> Self {
        Self {
            position,
            _padding1: 0.0,
            direction,
            _padding2: 0.0,
        }
    }
}

/// Time and animation uniforms.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TimeUniforms {
    /// Time in seconds since start.
    pub time: f32,
    /// Delta time since last frame.
    pub delta_time: f32,
    /// Frame counter.
    pub frame: u32,
    /// Padding for alignment.
    pub _padding: u32,
}

impl TimeUniforms {
    /// Create default time uniforms.
    pub fn new() -> Self {
        Self {
            time: 0.0,
            delta_time: 0.0,
            frame: 0,
            _padding: 0,
        }
    }
}

impl Default for TimeUniforms {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages a uniform buffer and its bind group.
pub struct UniformBuffer<T: bytemuck::Pod> {
    buffer: Buffer,
    bind_group: BindGroup,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: bytemuck::Pod> UniformBuffer<T> {
    /// Create a new uniform buffer with the given initial data.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `data` - Initial uniform data
    /// * `label` - Debug label for the buffer
    /// * `bind_group_layout` - Layout for creating the bind group
    pub fn new(
        device: &Device,
        data: &T,
        label: &str,
        bind_group_layout: &BindGroupLayout,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_buffer", label)),
            contents: bytemuck::cast_slice(&[*data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{}_bind_group", label)),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Update the uniform buffer with new data.
    pub fn update(&self, queue: &Queue, data: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[*data]));
    }

    /// Get the bind group for this uniform buffer.
    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}

/// Create a bind group layout for view-projection uniforms.
pub fn create_view_projection_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("view_projection_layout"),
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
    })
}

/// Create a bind group layout for texture atlas.
pub fn create_texture_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Helper to multiply two 4x4 matrices.
fn multiply_matrices(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}

/// Create a perspective projection matrix.
///
/// # Arguments
/// * `fov_y` - Vertical field of view in radians
/// * `aspect` - Aspect ratio (width / height)
/// * `near` - Near clipping plane
/// * `far` - Far clipping plane
pub fn perspective_matrix(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y / 2.0).tan();
    let range_inv = 1.0 / (near - far);

    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, (near + far) * range_inv, -1.0],
        [0.0, 0.0, near * far * range_inv * 2.0, 0.0],
    ]
}

/// Create a view matrix from position and target.
///
/// # Arguments
/// * `position` - Camera position
/// * `target` - Point the camera is looking at
/// * `up` - Up direction (usually [0, 1, 0])
pub fn look_at_matrix(position: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    // Calculate forward vector (from target to position)
    let forward = normalize([
        position[0] - target[0],
        position[1] - target[1],
        position[2] - target[2],
    ]);

    // Calculate right vector
    let right = normalize(cross(up, forward));

    // Recalculate up vector
    let up = cross(forward, right);

    [
        [right[0], up[0], forward[0], 0.0],
        [right[1], up[1], forward[1], 0.0],
        [right[2], up[2], forward[2], 0.0],
        [
            -dot(right, position),
            -dot(up, position),
            -dot(forward, position),
            1.0,
        ],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_projection_uniforms_identity() {
        let uniforms = ViewProjectionUniforms::identity();
        assert_eq!(uniforms.view_proj[0][0], 1.0);
        assert_eq!(uniforms.view_proj[1][1], 1.0);
        assert_eq!(uniforms.view_proj[2][2], 1.0);
        assert_eq!(uniforms.view_proj[3][3], 1.0);
    }

    #[test]
    fn camera_uniforms_creation() {
        let position = [1.0, 2.0, 3.0];
        let direction = [0.0, 0.0, -1.0];
        let uniforms = CameraUniforms::new(position, direction);
        assert_eq!(uniforms.position, position);
        assert_eq!(uniforms.direction, direction);
    }

    #[test]
    fn time_uniforms_default() {
        let uniforms = TimeUniforms::default();
        assert_eq!(uniforms.time, 0.0);
        assert_eq!(uniforms.delta_time, 0.0);
        assert_eq!(uniforms.frame, 0);
    }

    #[test]
    fn uniform_buffer_creation() -> Result<()> {
        use crate::GpuContext;

        let context = GpuContext::new_headless()?;
        let layout = create_view_projection_layout(context.device());
        let data = ViewProjectionUniforms::identity();

        let _buffer = UniformBuffer::new(context.device(), &data, "test", &layout);

        Ok(())
    }

    #[test]
    fn perspective_matrix_creation() {
        let fov = std::f32::consts::PI / 4.0; // 45 degrees
        let aspect = 16.0 / 9.0;
        let near = 0.1;
        let far = 100.0;

        let matrix = perspective_matrix(fov, aspect, near, far);

        // Check that diagonal elements are non-zero
        assert!(matrix[0][0] != 0.0);
        assert!(matrix[1][1] != 0.0);
        assert!(matrix[2][2] != 0.0);
    }

    #[test]
    fn look_at_matrix_creation() {
        let position = [0.0, 0.0, 5.0];
        let target = [0.0, 0.0, 0.0];
        let up = [0.0, 1.0, 0.0];

        let matrix = look_at_matrix(position, target, up);

        // Basic sanity checks
        assert!(matrix[3][3] == 1.0);
    }
}
