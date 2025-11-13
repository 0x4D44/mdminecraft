#![warn(missing_docs)]
//! Rendering facade built on top of wgpu + chunk meshing.

mod cache;
mod driver;
mod mesh;

pub use cache::ChunkMeshCache;
pub use driver::{ChunkMeshDriver, ChunkMeshStat};
pub use mesh::{mesh_chunk, MeshBuffers, MeshHash, MeshVertex};

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

/// Placeholder renderer that will later own the wgpu device/queue.
pub struct Renderer {
    _config: RendererConfig,
}

impl Renderer {
    /// Construct a renderer with the supplied config (no GPU touches yet).
    pub fn new(config: RendererConfig) -> Self {
        tracing::info!(?config, "renderer placeholder initialized");
        Self { _config: config }
    }
}
