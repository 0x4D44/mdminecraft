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
use egui_wgpu::ScreenDescriptor;

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
    depth_texture: Option<wgpu::TextureView>,
    egui_renderer: Option<egui_wgpu::Renderer>,
}

impl Renderer {
    /// Create a depth texture for depth testing.
    fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = device.create_texture(&desc);
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }


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

        // Create depth texture
        let depth_texture = Self::create_depth_texture(&device, config.width, config.height);

        // Initialize egui renderer
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1);

        tracing::info!("wgpu renderer initialized successfully");

        Self {
            config,
            device,
            queue,
            surface: Some(surface),
            surface_config: Some(surface_config),
            pipeline,
            chunk_meshes: HashMap::new(),
            depth_texture: Some(depth_texture),
            egui_renderer: Some(egui_renderer),
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
            depth_texture: None,
            egui_renderer: None,
        }
    }

    /// Upload a chunk mesh to the GPU.
    pub fn upload_chunk_mesh(&mut self, pos: ChunkPos, mesh: &MeshBuffers) {
        // Convert ChunkPos to world coordinates
        // Chunks are 16x256x16, ChunkPos is (x, z) in chunk coordinates
        let chunk_offset = [
            (pos.x * 16) as f32,
            0.0,  // Y offset is always 0 (chunks span full height)
            (pos.z * 16) as f32,
        ];

        let gpu_mesh = GpuMesh::from_mesh_buffers(
            &self.device,
            mesh,
            chunk_offset,
            &self.pipeline.chunk_bind_group_layout,
        );
        self.chunk_meshes.insert(pos, gpu_mesh);
    }

    /// Remove a chunk mesh from the GPU.
    pub fn remove_chunk_mesh(&mut self, pos: ChunkPos) {
        self.chunk_meshes.remove(&pos);
    }

    /// Render a frame with the given camera and optional egui primitives.
    pub fn render_with_ui(
        &mut self,
        camera: &Camera,
        egui_primitives: Option<(egui::Context, egui::FullOutput)>,
    ) -> Result<(), wgpu::SurfaceError> {
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
            let depth_stencil_attachment = self.depth_texture.as_ref().map(|depth_view| {
                wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }
            });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.7,  // Sky horizon color (matches shader)
                            g: 0.85,
                            b: 0.95,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            render_pass.set_bind_group(0, &self.pipeline.camera_bind_group, &[]);

            // Draw all chunk meshes
            for (_pos, mesh) in &self.chunk_meshes {
                // Set chunk-specific bind group (group 1)
                render_pass.set_bind_group(1, &mesh.bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        // Render egui if provided
        if let (Some((ctx, full_output)), Some(egui_renderer)) = (egui_primitives, &mut self.egui_renderer) {
            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: ctx.pixels_per_point(),
            };

            // Upload egui textures
            for (id, image_delta) in &full_output.textures_delta.set {
                egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
            }

            // Record egui render commands
            let primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
            egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &primitives, &screen_descriptor);

            {
                let mut egui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                egui_renderer.render(&mut egui_pass, &primitives, &screen_descriptor);
            }

            // Cleanup egui textures
            for id in &full_output.textures_delta.free {
                egui_renderer.free_texture(id);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Render a frame with the given camera (no UI).
    pub fn render(&mut self, camera: &Camera) -> Result<(), wgpu::SurfaceError> {
        self.render_with_ui(camera, None)
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

        // Recreate depth texture with new size
        if self.depth_texture.is_some() {
            self.depth_texture = Some(Self::create_depth_texture(&self.device, width, height));
        }
    }

    /// Get render statistics (total indices, total triangles, chunk count).
    pub fn get_render_stats(&self) -> (u32, u32, usize) {
        let total_indices: u32 = self.chunk_meshes.values().map(|m| m.index_count).sum();
        let total_triangles = total_indices / 3;
        let chunk_count = self.chunk_meshes.len();
        (total_indices, total_triangles, chunk_count)
    }
}
