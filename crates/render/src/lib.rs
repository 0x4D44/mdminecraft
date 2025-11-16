#![warn(missing_docs)]
//! Rendering facade built on top of wgpu + chunk meshing.

mod cache;
mod driver;
mod mesh;
mod gpu_mesh;
mod pipeline;

pub use cache::ChunkMeshCache;
pub use driver::{ChunkMeshDriver, ChunkMeshStat};
pub use mesh::{mesh_chunk, MeshBuffers, MeshHash, MeshVertex};
pub use gpu_mesh::GpuMesh;

use std::collections::HashMap;
use mdminecraft_world::ChunkPos;
use mdminecraft_camera::Camera;

/// Renderer configuration for headless + onscreen paths.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
    /// Request a headless (off-screen) surface.
    pub headless: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            headless: false,
        }
    }
}

/// Fully-featured renderer with wgpu device/queue and rendering pipeline.
pub struct Renderer {
    config: RendererConfig,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    pipeline: pipeline::ChunkPipeline,
    chunk_meshes: HashMap<ChunkPos, GpuMesh>,
}

impl Renderer {
    /// Create a new renderer with a window surface.
    pub fn new(window: std::sync::Arc<winit::window::Window>, config: RendererConfig) -> Self {
        tracing::info!(?config, "initializing wgpu renderer");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: config.width,
            height: config.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        let pipeline = pipeline::ChunkPipeline::new(&device, surface_format);

        tracing::info!("wgpu renderer initialized successfully");

        Self {
            config,
            device,
            queue,
            surface: Some(surface),
            surface_config: Some(surface_config),
            pipeline,
            chunk_meshes: HashMap::new(),
        }
    }

    /// Create a headless renderer (no window surface).
    pub fn new_headless(config: RendererConfig) -> Self {
        tracing::info!(?config, "initializing headless wgpu renderer");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Headless Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .expect("Failed to create device");

        let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let pipeline = pipeline::ChunkPipeline::new(&device, surface_format);

        tracing::info!("headless wgpu renderer initialized successfully");

        Self {
            config,
            device,
            queue,
            surface: None,
            surface_config: None,
            pipeline,
            chunk_meshes: HashMap::new(),
        }
    }

    /// Upload a chunk mesh to the GPU.
    pub fn upload_chunk_mesh(&mut self, pos: ChunkPos, mesh: &MeshBuffers) {
        let gpu_mesh = GpuMesh::from_mesh_buffers(&self.device, mesh);
        self.chunk_meshes.insert(pos, gpu_mesh);
    }

    /// Remove a chunk mesh from the GPU.
    pub fn remove_chunk_mesh(&mut self, pos: ChunkPos) {
        self.chunk_meshes.remove(&pos);
    }

    /// Render a frame with the given camera.
    pub fn render(&mut self, camera: &Camera) -> Result<(), wgpu::SurfaceError> {
        let surface = match &self.surface {
            Some(s) => s,
            None => {
                tracing::warn!("attempted to render on headless renderer");
                return Ok(());
            }
        };

        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Update camera uniform
        self.pipeline.update_camera(&self.queue, camera);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            render_pass.set_bind_group(0, &self.pipeline.camera_bind_group, &[]);

            // Draw all chunk meshes
            for (_pos, mesh) in &self.chunk_meshes {
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Resize the renderer's surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;

        if let (Some(surface), Some(surface_config)) = (&self.surface, &mut self.surface_config) {
            surface_config.width = width;
            surface_config.height = height;
            surface.configure(&self.device, surface_config);
        }
    }
}
