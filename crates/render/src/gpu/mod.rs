//! GPU abstraction layer for wgpu device initialization and management.
//!
//! This module provides deterministic GPU initialization for both headless
//! (CI/testing) and windowed (interactive) rendering modes.

#![allow(dead_code)] // Public API used by tests
#![allow(unused_imports)] // GPU module types will be used in future integration

mod buffer;
mod context;
mod device;
mod pipeline;
mod surface;
pub mod texture_atlas;
pub mod texture_loader;
mod uniforms;

pub use buffer::{BufferHandle, BufferManager, BufferUsage};
pub use context::{GpuContext, GpuContextConfig};
pub use pipeline::{RenderPipeline, RenderPipelineBuilder};
pub use surface::SurfaceManager;
pub use texture_atlas::{TextureAtlas, TextureId, UvCoordinates};
pub use texture_loader::{LoadedTexture, TextureLoader};
pub use uniforms::{
    create_texture_layout, create_view_projection_layout, look_at_matrix, perspective_matrix,
    CameraUniforms, TimeUniforms, UniformBuffer, ViewProjectionUniforms,
};

#[cfg(test)]
mod tests;
