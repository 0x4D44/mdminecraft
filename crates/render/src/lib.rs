#![warn(missing_docs)]
//! Rendering facade built on top of wgpu + chunk meshing.

use std::cell::RefCell;

mod cache;
mod camera;
mod chunk_manager;
mod driver;
mod mesh;
mod particles;
mod pipeline;
mod raycast;
mod screenshot;
mod texture_atlas;
mod time;
mod ui;
mod window;

pub use cache::ChunkMeshCache;
pub use camera::{Camera, CameraUniform};
pub use chunk_manager::{ChunkManager, ChunkRenderData, Frustum};
pub use driver::{ChunkMeshDriver, ChunkMeshStat};
use mdminecraft_assets::TextureAtlasMetadata;
pub use mesh::{mesh_chunk, mesh_chunk_with_voxel_at, MeshBuffers, MeshHash, MeshVertex};
pub use particles::{ParticleEmitter, ParticleSystem, ParticleVertex};
pub use pipeline::{
    ChunkMeshBuffer, ChunkUniform, HighlightUniform, ParticlePipeline, RenderContext,
    SkyboxPipeline, VoxelPipeline, WireframePipeline,
};
pub use raycast::{raycast, RaycastHit};
pub use screenshot::{record_texture_readback, write_png, TextureReadback};
pub use texture_atlas::{atlas_exists, warn_missing_atlas};
pub use time::{TimeOfDay, TimeUniform};
pub use ui::{ControlMode, DebugHud, UiManager, UiRenderContext};
pub use window::{InputContext, InputSnapshot, InputState, WindowConfig, WindowManager};

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

/// Main renderer owning GPU resources.
pub struct Renderer {
    config: RendererConfig,
    context: Option<RenderContext>,
    pipeline: Option<VoxelPipeline>,
    skybox_pipeline: Option<SkyboxPipeline>,
    wireframe_pipeline: Option<WireframePipeline>,
    particle_pipeline: Option<ParticlePipeline>,
    camera: Camera,
    ui: Option<RefCell<UiManager>>,
}

impl Renderer {
    /// Construct a renderer with the supplied config.
    pub fn new(config: RendererConfig) -> Self {
        let camera = Camera::new(config.width as f32 / config.height as f32);
        tracing::info!(?config, "renderer initialized");

        Self {
            config,
            context: None,
            pipeline: None,
            skybox_pipeline: None,
            wireframe_pipeline: None,
            particle_pipeline: None,
            camera,
            ui: None,
        }
    }

    /// Initialize GPU resources with a window (async).
    pub async fn initialize_gpu(
        &mut self,
        window: std::sync::Arc<winit::window::Window>,
    ) -> anyhow::Result<()> {
        let context = RenderContext::new(window.clone()).await?;
        let pipeline = VoxelPipeline::new(&context)?;
        let skybox_pipeline = SkyboxPipeline::new(&context)?;
        let wireframe_pipeline =
            WireframePipeline::new(&context, pipeline.camera_bind_group_layout())?;
        let particle_pipeline =
            ParticlePipeline::new(&context, pipeline.camera_bind_group_layout())?;

        // Initialize UI (wrapped in RefCell for interior mutability)
        let ui = UiManager::new(&context.device, context.config.format, &window);

        self.camera.set_aspect(context.aspect_ratio());

        self.context = Some(context);
        self.pipeline = Some(pipeline);
        self.skybox_pipeline = Some(skybox_pipeline);
        self.wireframe_pipeline = Some(wireframe_pipeline);
        self.particle_pipeline = Some(particle_pipeline);
        self.ui = Some(RefCell::new(ui));

        Ok(())
    }

    /// Initialize GPU resources for headless/offscreen rendering (async).
    pub async fn initialize_gpu_headless(&mut self) -> anyhow::Result<()> {
        let context = RenderContext::new_headless(
            (self.config.width, self.config.height),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
        .await?;

        let pipeline = VoxelPipeline::new(&context)?;
        let skybox_pipeline = SkyboxPipeline::new(&context)?;
        let wireframe_pipeline =
            WireframePipeline::new(&context, pipeline.camera_bind_group_layout())?;
        let particle_pipeline =
            ParticlePipeline::new(&context, pipeline.camera_bind_group_layout())?;

        self.camera.set_aspect(context.aspect_ratio());

        self.context = Some(context);
        self.pipeline = Some(pipeline);
        self.skybox_pipeline = Some(skybox_pipeline);
        self.wireframe_pipeline = Some(wireframe_pipeline);
        self.particle_pipeline = Some(particle_pipeline);
        self.ui = None;

        Ok(())
    }

    /// Get mutable reference to UI manager via RefCell.
    pub fn ui_mut(&self) -> Option<std::cell::RefMut<'_, UiManager>> {
        self.ui.as_ref().map(|cell| cell.borrow_mut())
    }

    /// Get reference to UI manager via RefCell.
    pub fn ui(&self) -> Option<std::cell::Ref<'_, UiManager>> {
        self.ui.as_ref().map(|cell| cell.borrow())
    }

    /// Access the renderer configuration provided at construction time.
    pub fn config(&self) -> &RendererConfig {
        &self.config
    }

    /// Get mutable reference to the camera.
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Get reference to the camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Access texture atlas metadata if available.
    pub fn atlas_metadata(&self) -> Option<&TextureAtlasMetadata> {
        self.pipeline
            .as_ref()
            .and_then(|pipeline| pipeline.atlas_metadata())
    }

    /// Resize the renderer.
    pub fn resize(&mut self, new_size: (u32, u32)) {
        self.config.width = new_size.0;
        self.config.height = new_size.1;
        if let Some(context) = &mut self.context {
            context.resize(new_size);
            self.camera.set_aspect(context.aspect_ratio());

            if let Some(pipeline) = &mut self.pipeline {
                pipeline.resize(&context.device, new_size);
            }
            if let Some(particle_pipeline) = self.particle_pipeline.as_ref() {
                particle_pipeline.update_viewport(&context.queue, new_size);
            }
        }
    }

    /// Begin a new frame and return the render context.
    pub fn begin_frame(&mut self) -> Option<FrameContext> {
        let context = self.context.as_ref()?;
        let pipeline = self.pipeline.as_ref()?;
        let skybox_pipeline = self.skybox_pipeline.as_ref()?;

        let (output, view) = if let Some(surface) = context.surface.as_ref() {
            let output = surface.get_current_texture().ok()?;
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            (Some(output), view)
        } else {
            let headless = context.headless.as_ref()?;
            let view = headless
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            (None, view)
        };

        // Update camera uniform
        pipeline.update_camera(&context.queue, &self.camera);
        skybox_pipeline.update_camera(&context.queue, &self.camera);

        Some(FrameContext { output, view })
    }

    /// Get render resources for drawing.
    pub fn render_resources(&self) -> Option<RenderResources<'_>> {
        let context = self.context.as_ref()?;
        let pipeline = self.pipeline.as_ref()?;
        let skybox_pipeline = self.skybox_pipeline.as_ref()?;
        let wireframe_pipeline = self.wireframe_pipeline.as_ref()?;
        let particle_pipeline = self.particle_pipeline.as_ref()?;

        Some(RenderResources {
            device: &context.device,
            queue: &context.queue,
            pipeline,
            skybox_pipeline,
            wireframe_pipeline,
            particle_pipeline,
        })
    }

    /// Surface format used by the swapchain (if initialized).
    pub fn surface_format(&self) -> Option<wgpu::TextureFormat> {
        self.context.as_ref().map(|ctx| ctx.config.format)
    }

    /// Access the underlying `wgpu::Device` if the renderer has been initialized.
    pub fn device(&self) -> Option<&wgpu::Device> {
        self.context.as_ref().map(|ctx| &ctx.device)
    }
}

/// Frame rendering context.
pub struct FrameContext {
    output: Option<wgpu::SurfaceTexture>,
    /// The texture view for this frame.
    pub view: wgpu::TextureView,
}

impl FrameContext {
    /// Access the underlying surface texture for this frame (windowed only).
    pub fn surface_texture(&self) -> Option<&wgpu::Texture> {
        self.output.as_ref().map(|output| &output.texture)
    }

    /// Finish the frame and present.
    pub fn present(self) {
        if let Some(output) = self.output {
            output.present();
        }
    }
}

/// Resources needed for rendering.
pub struct RenderResources<'a> {
    /// GPU device.
    pub device: &'a wgpu::Device,
    /// Command queue.
    pub queue: &'a wgpu::Queue,
    /// Voxel rendering pipeline.
    pub pipeline: &'a VoxelPipeline,
    /// Skybox rendering pipeline.
    pub skybox_pipeline: &'a SkyboxPipeline,
    /// Wireframe rendering pipeline.
    pub wireframe_pipeline: &'a WireframePipeline,
    /// Particle rendering pipeline.
    pub particle_pipeline: &'a ParticlePipeline,
}
