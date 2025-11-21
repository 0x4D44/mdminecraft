//! Particle system utilities for block break effects and weather streaks.

use wgpu::util::DeviceExt;

/// GPU vertex for a single particle billboard.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleVertex {
    /// World-space center of the particle.
    pub position: [f32; 3],
    /// RGBA color/intensity.
    pub color: [f32; 4],
    /// Remaining lifetime in seconds.
    pub lifetime: f32,
    /// Billboard scale in world units.
    pub scale: f32,
}

/// CPU-side emitter used to build particle batches each frame.
#[derive(Debug, Default)]
pub struct ParticleEmitter {
    /// Temporary vertex buffer for GPU upload.
    pub vertices: Vec<ParticleVertex>,
}

impl ParticleEmitter {
    /// Create an empty emitter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a new particle.
    pub fn spawn(&mut self, vertex: ParticleVertex) {
        self.vertices.push(vertex);
    }

    /// Clear accumulated particles.
    pub fn clear(&mut self) {
        self.vertices.clear();
    }
}

/// GPU upload containing the current frame’s particles.
pub struct ParticleSystem {
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl ParticleSystem {
    /// Create a GPU buffer from the emitter contents.
    pub fn from_emitter(device: &wgpu::Device, emitter: &ParticleEmitter) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Vertex Buffer"),
            contents: bytemuck::cast_slice(&emitter.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertex_buffer,
            vertex_count: emitter.vertices.len() as u32,
        }
    }

    /// Draw the particles as point sprites.
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.vertex_count == 0 {
            return;
        }
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}
