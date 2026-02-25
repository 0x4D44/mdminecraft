//! Surface management for windowed rendering.
//!
//! This module handles creation and configuration of wgpu surfaces for rendering to windows.

use anyhow::{Context, Result};
use wgpu::{Instance, Surface, SurfaceConfiguration, TextureFormat};

/// Surface wrapper that manages a wgpu surface and its configuration.
pub struct SurfaceManager<'window> {
    surface: Surface<'window>,
    config: SurfaceConfiguration,
}

impl<'window> SurfaceManager<'window> {
    /// Create a new surface manager from a winit window.
    ///
    /// # Arguments
    /// * `instance` - The wgpu instance
    /// * `target` - The surface target (from window)
    /// * `width` - Surface width in pixels
    /// * `height` - Surface height in pixels
    /// * `present_mode` - The present mode for the swap chain
    ///
    /// # Safety
    /// The window must remain valid for the lifetime of the surface.
    ///
    /// # Errors
    /// Returns an error if surface creation or configuration fails.
    pub unsafe fn new(
        instance: &Instance,
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<Self> {
        // Create surface from window
        let surface = instance
            .create_surface(target)
            .context("Failed to create surface from window")?;

        // Create default configuration
        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb, // Common format for windows
            width,
            height,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Ok(Self { surface, config })
    }

    /// Configure the surface with the given device.
    ///
    /// This must be called before the surface can be used for rendering.
    pub fn configure(&mut self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }

    /// Get the current surface texture to render to.
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture> {
        self.surface
            .get_current_texture()
            .context("Failed to acquire next surface texture")
    }

    /// Resize the surface to new dimensions.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(device, &self.config);
        }
    }

    /// Get the current surface configuration.
    pub fn config(&self) -> &SurfaceConfiguration {
        &self.config
    }

    /// Get the surface format.
    pub fn format(&self) -> TextureFormat {
        self.config.format
    }

    /// Get the current dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
}
