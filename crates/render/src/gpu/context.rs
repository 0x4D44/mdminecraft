//! Main GPU context that integrates device, surface, and configuration.
//!
//! This module provides the high-level GPU context API used by the renderer.

use super::device::DeviceManager;
use anyhow::{bail, Context as AnyhowContext, Result};
use wgpu::{Device, Queue};

/// Configuration for GPU context creation.
#[derive(Debug, Clone)]
pub struct GpuContextConfig {
    /// Render target width in pixels.
    pub width: u32,
    /// Render target height in pixels.
    pub height: u32,
    /// Whether to use headless (offscreen) rendering.
    pub headless: bool,
    /// GPU power preference.
    pub power_preference: wgpu::PowerPreference,
    /// Present mode for swap chain (windowed mode only).
    pub present_mode: wgpu::PresentMode,
}

impl GpuContextConfig {
    /// Create a default headless configuration for testing.
    pub fn default_headless() -> Self {
        Self {
            width: 800,
            height: 600,
            headless: true,
            power_preference: wgpu::PowerPreference::LowPower,
            present_mode: wgpu::PresentMode::Fifo,
        }
    }
}

impl Default for GpuContextConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            headless: false,
            power_preference: wgpu::PowerPreference::HighPerformance,
            present_mode: wgpu::PresentMode::Mailbox,
        }
    }
}

/// Main GPU context integrating device, queue, and configuration.
///
/// This is the primary interface for GPU operations in the renderer.
pub struct GpuContext {
    device_manager: DeviceManager,
    config: GpuContextConfig,
}

impl GpuContext {
    /// Create a new GPU context with the specified configuration.
    ///
    /// # Arguments
    /// * `config` - Configuration for the GPU context
    ///
    /// # Returns
    /// A new GpuContext, or an error if GPU initialization fails.
    ///
    /// # Errors
    /// - If width or height are zero
    /// - If no compatible GPU adapter is found
    /// - If device creation fails
    pub fn new(config: GpuContextConfig) -> Result<Self> {
        // Validate configuration
        if config.width == 0 || config.height == 0 {
            bail!(
                "Invalid dimensions: {}x{}. Width and height must be non-zero.",
                config.width,
                config.height
            );
        }

        // Create device manager asynchronously
        let device_manager =
            pollster::block_on(DeviceManager::new(config.power_preference, config.headless))
                .context("Failed to create GPU device manager")?;

        Ok(Self {
            device_manager,
            config,
        })
    }

    /// Create a new headless GPU context for testing/CI.
    ///
    /// This is a convenience method that uses default headless settings.
    pub fn new_headless() -> Result<Self> {
        Self::new(GpuContextConfig::default_headless())
    }

    /// Get a reference to the wgpu device.
    #[inline]
    pub fn device(&self) -> &Device {
        self.device_manager.device()
    }

    /// Get a reference to the wgpu queue.
    #[inline]
    pub fn queue(&self) -> &Queue {
        self.device_manager.queue()
    }

    /// Get the render target width.
    #[inline]
    pub fn width(&self) -> u32 {
        self.config.width
    }

    /// Get the render target height.
    #[inline]
    pub fn height(&self) -> u32 {
        self.config.height
    }

    /// Check if this context is in headless mode.
    #[inline]
    pub fn is_headless(&self) -> bool {
        self.config.headless
    }

    /// Check if this context has a surface (windowed mode).
    ///
    /// In headless mode, there is no surface.
    #[inline]
    pub fn has_surface(&self) -> bool {
        !self.config.headless
    }

    /// Get a reference to the wgpu instance (needed for surface creation).
    #[inline]
    pub fn instance(&self) -> &wgpu::Instance {
        self.device_manager.instance()
    }

    /// Get adapter information for diagnostics.
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.device_manager.adapter_info()
    }

    /// Check if the device is still valid.
    pub fn is_device_valid(&self) -> bool {
        self.device_manager.is_device_valid()
    }

    /// Wait for all GPU operations to complete.
    ///
    /// This is primarily useful for cleanup and testing.
    pub fn wait_for_idle(&self) -> Result<()> {
        self.device_manager.wait_for_idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_creation_with_valid_config() {
        let config = GpuContextConfig {
            width: 800,
            height: 600,
            headless: true,
            power_preference: wgpu::PowerPreference::LowPower,
            present_mode: wgpu::PresentMode::Fifo,
        };

        let context = GpuContext::new(config).expect("Context creation failed");
        assert_eq!(context.width(), 800);
        assert_eq!(context.height(), 600);
        assert!(context.is_headless());
    }

    #[test]
    fn headless_context_creation() {
        let context = GpuContext::new_headless().expect("Headless context failed");
        assert!(context.is_headless());
        assert!(!context.has_surface());
    }

    #[test]
    fn zero_dimensions_rejected() {
        let config = GpuContextConfig {
            width: 0,
            height: 0,
            headless: true,
            power_preference: wgpu::PowerPreference::LowPower,
            present_mode: wgpu::PresentMode::Fifo,
        };

        let result = GpuContext::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn device_and_queue_accessible() {
        let context = GpuContext::new_headless().expect("Context creation failed");

        let device = context.device();
        let _queue = context.queue();

        // Basic sanity checks
        assert!(device.limits().max_texture_dimension_2d > 0);
    }
}
