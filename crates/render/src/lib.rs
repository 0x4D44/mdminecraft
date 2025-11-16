#![warn(missing_docs)]
//! Rendering facade built on top of wgpu + chunk meshing.

use std::cell::RefCell;

mod cache;
mod camera;
mod chunk_manager;
mod driver;
mod mesh;
mod pipeline;
mod ui;
mod window;

pub use cache::ChunkMeshCache;
pub use camera::{Camera, CameraUniform};
pub use chunk_manager::{ChunkManager, ChunkRenderData, Frustum};
pub use driver::{ChunkMeshDriver, ChunkMeshStat};
pub use mesh::{mesh_chunk, MeshBuffers, MeshHash, MeshVertex};
pub use pipeline::{ChunkMeshBuffer, RenderContext, VoxelPipeline};
pub use ui::{DebugHud, UiManager};
pub use window::{InputState, WindowConfig, WindowManager};

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
            camera,
            ui: None,
        }
    }

    /// Initialize GPU resources with a window (async).
    pub async fn initialize_gpu(&mut self, window: std::sync::Arc<winit::window::Window>) -> anyhow::Result<()> {
        let context = RenderContext::new(window.clone()).await?;
        let pipeline = VoxelPipeline::new(&context)?;

        // Initialize UI (wrapped in RefCell for interior mutability)
        let ui = UiManager::new(&context.device, context.config.format, &window);

        self.camera.set_aspect(context.aspect_ratio());

        self.context = Some(context);
        self.pipeline = Some(pipeline);
        self.ui = Some(RefCell::new(ui));

        Ok(())
    }

    /// Get mutable reference to UI manager via RefCell.
    pub fn ui_mut(&self) -> Option<std::cell::RefMut<UiManager>> {
        self.ui.as_ref().map(|cell| cell.borrow_mut())
    }

    /// Get reference to UI manager via RefCell.
    pub fn ui(&self) -> Option<std::cell::Ref<UiManager>> {
        self.ui.as_ref().map(|cell| cell.borrow())
    }

    /// Get mutable reference to the camera.
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Get reference to the camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Resize the renderer.
    pub fn resize(&mut self, new_size: (u32, u32)) {
        if let Some(context) = &mut self.context {
            context.resize(new_size);
            self.camera.set_aspect(context.aspect_ratio());

            if let Some(pipeline) = &mut self.pipeline {
                pipeline.resize(&context.device, new_size);
            }
        }
    }

    /// Begin a new frame and return the render context.
    pub fn begin_frame(&mut self) -> Option<FrameContext> {
        let context = self.context.as_ref()?;
        let pipeline = self.pipeline.as_ref()?;

        let output = context.surface.get_current_texture().ok()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Update camera uniform
        pipeline.update_camera(&context.queue, &self.camera);

        Some(FrameContext {
            output,
            view,
        })
    }

    /// Get render resources for drawing.
    pub fn render_resources(&self) -> Option<RenderResources> {
        let context = self.context.as_ref()?;
        let pipeline = self.pipeline.as_ref()?;

        Some(RenderResources {
            device: &context.device,
            queue: &context.queue,
            pipeline,
        })
    }
}

/// Frame rendering context.
pub struct FrameContext {
    output: wgpu::SurfaceTexture,
    /// The texture view for this frame.
    pub view: wgpu::TextureView,
}

impl FrameContext {
    /// Finish the frame and present.
    pub fn present(self) {
        self.output.present();
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
}
