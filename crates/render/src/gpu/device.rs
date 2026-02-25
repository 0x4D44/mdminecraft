//! Low-level wgpu instance, adapter, and device management.
//!
//! This module handles the creation and lifecycle of wgpu GPU resources.
//! It supports both headless (for CI/testing) and windowed rendering modes.

use anyhow::{Context, Result};
use wgpu::{Adapter, Device, DeviceDescriptor, Features, Instance, Limits, PowerPreference, Queue};

/// Manages wgpu instance, adapter, device, and queue.
///
/// This struct encapsulates all low-level GPU initialization logic.
/// It provides deterministic adapter selection based on power preference.
pub struct DeviceManager {
    instance: Instance,
    adapter: Adapter,
    device: Device,
    queue: Queue,
}

impl DeviceManager {
    /// Create a new device manager with the specified power preference.
    ///
    /// # Arguments
    /// * `power_preference` - Preference for GPU selection (LowPower or HighPerformance)
    /// * `headless` - Whether to use headless backend for testing
    ///
    /// # Returns
    /// A DeviceManager with initialized GPU resources, or an error if initialization fails.
    pub async fn new(power_preference: PowerPreference, headless: bool) -> Result<Self> {
        // Create wgpu instance with appropriate backends
        let instance = Self::create_instance(headless);

        // Request adapter with specified power preference
        let adapter = Self::request_adapter(&instance, power_preference, headless).await?;

        // Request device and queue
        let (device, queue) = Self::request_device(&adapter).await?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Create a wgpu instance with appropriate backends for the mode.
    fn create_instance(headless: bool) -> Instance {
        let backends = if headless {
            // For headless/CI, prefer Vulkan/DX12 which support offscreen rendering
            wgpu::Backends::VULKAN | wgpu::Backends::DX12
        } else {
            // For windowed mode, use all available backends
            wgpu::Backends::all()
        };

        Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        })
    }

    /// Request an adapter matching the specified criteria.
    async fn request_adapter(
        instance: &Instance,
        power_preference: PowerPreference,
        headless: bool,
    ) -> Result<Adapter> {
        let options = wgpu::RequestAdapterOptions {
            power_preference,
            compatible_surface: None, // No surface needed for headless
            force_fallback_adapter: false,
        };

        instance
            .request_adapter(&options)
            .await
            .context(if headless {
                "Failed to find compatible GPU adapter for headless rendering. \
                 Ensure Vulkan or DX12 drivers are installed."
            } else {
                "Failed to find compatible GPU adapter. \
                 Ensure graphics drivers are up to date."
            })
    }

    /// Request device and queue from the adapter.
    async fn request_device(adapter: &Adapter) -> Result<(Device, Queue)> {
        let required_limits = Limits {
            max_texture_dimension_2d: 8192,     // For texture atlas
            max_buffer_size: 256 * 1024 * 1024, // 256MB for chunk meshes
            max_bind_groups: 4,                 // For materials, textures, uniforms
            ..Limits::downlevel_defaults()
        };

        // Request minimal features for maximum compatibility
        let required_features = Features::empty();

        let descriptor = DeviceDescriptor {
            label: Some("mdminecraft-gpu-device"),
            required_features,
            required_limits,
        };

        adapter
            .request_device(&descriptor, None)
            .await
            .context("Failed to create GPU device. Hardware may not meet minimum requirements.")
    }

    /// Get a reference to the device.
    #[inline]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get a reference to the queue.
    #[inline]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Get a reference to the adapter.
    #[inline]
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    /// Get a reference to the instance (needed for surface creation).
    #[inline]
    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    /// Get adapter information for diagnostics.
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    /// Wait for all GPU operations to complete.
    ///
    /// This is useful for cleanup and synchronization in tests.
    pub fn wait_for_idle(&self) -> Result<()> {
        self.device.poll(wgpu::Maintain::Wait).panic_on_timeout();
        Ok(())
    }

    /// Check if the device is still valid.
    pub fn is_device_valid(&self) -> bool {
        // In wgpu, a device remains valid unless explicitly lost
        // We could add device loss detection here in the future
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_manager_creation() {
        pollster::block_on(async {
            let manager = DeviceManager::new(PowerPreference::LowPower, true)
                .await
                .expect("Failed to create device manager");

            assert!(manager.is_device_valid());
            assert!(manager.device().limits().max_texture_dimension_2d >= 4096);
        });
    }

    #[test]
    fn adapter_info_is_populated() {
        pollster::block_on(async {
            let manager = DeviceManager::new(PowerPreference::LowPower, true)
                .await
                .expect("Failed to create device manager");

            let info = manager.adapter_info();
            assert!(!info.name.is_empty());
        });
    }
}
