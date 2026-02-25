//! Headless rendering for snapshot testing and CI.
//!
//! This module provides deterministic off-screen rendering capabilities for testing.
//! It can capture rendered frames to CPU memory, hash them with blake3, and save them as PNG files.

use anyhow::{Context as AnyhowContext, Result};
use blake3::Hasher;
use std::path::Path;
use wgpu::{Texture, TextureView};

use crate::{Camera, Renderer};

/// Hash of a rendered frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHash(pub [u8; 32]);

impl FrameHash {
    /// Convert hash to hex string for display.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

/// Headless renderer for snapshot testing.
pub struct HeadlessRenderer {
    renderer: Renderer,
    offscreen_texture: Texture,
    offscreen_view: TextureView,
    width: u32,
    height: u32,
}

impl HeadlessRenderer {
    /// Create a new headless renderer.
    ///
    /// # Arguments
    /// * `renderer` - The base renderer (must be in headless mode)
    ///
    /// # Returns
    /// A headless renderer with an off-screen render target.
    pub fn new(renderer: Renderer) -> Result<Self> {
        if !renderer.gpu().is_headless() {
            anyhow::bail!("Renderer must be in headless mode for HeadlessRenderer");
        }

        let width = renderer.gpu().width();
        let height = renderer.gpu().height();

        // Create off-screen texture for rendering
        // Use Bgra8UnormSrgb to match the pipeline format
        let offscreen_texture = renderer
            .gpu()
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("offscreen_texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

        let offscreen_view = offscreen_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self {
            renderer,
            offscreen_texture,
            offscreen_view,
            width,
            height,
        })
    }

    /// Get a reference to the underlying renderer.
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// Get a mutable reference to the underlying renderer.
    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    /// Render a frame with the given camera.
    ///
    /// # Arguments
    /// * `camera` - Camera position and orientation
    ///
    /// # Returns
    /// Ok(()) if rendering succeeds.
    pub fn render(&mut self, camera: Camera) -> Result<()> {
        self.renderer.update_camera(camera);
        self.renderer.render_frame(&self.offscreen_view)
    }

    /// Capture the current frame to CPU memory and compute its hash.
    ///
    /// # Returns
    /// The blake3 hash of the rendered frame data.
    pub fn capture_frame_hash(&self) -> Result<FrameHash> {
        let frame_data = self.capture_frame_data()?;
        let mut hasher = Hasher::new();
        hasher.update(&frame_data);
        Ok(FrameHash(*hasher.finalize().as_bytes()))
    }

    /// Capture the current frame to CPU memory.
    ///
    /// # Returns
    /// Raw RGBA8 pixel data (row-major order).
    fn capture_frame_data(&self) -> Result<Vec<u8>> {
        let device = self.renderer.gpu().device();
        let queue = self.renderer.gpu().queue();

        // Calculate buffer size and padding
        let bytes_per_pixel = 4; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        // Create staging buffer
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture_encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.offscreen_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });

        device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .context("Failed to receive buffer mapping result")?
            .context("Failed to map staging buffer")?;

        let padded_data = buffer_slice.get_mapped_range();

        // Remove padding if necessary
        let mut frame_data = Vec::with_capacity((self.width * self.height * bytes_per_pixel) as usize);
        for row in 0..self.height {
            let offset = (row * padded_bytes_per_row) as usize;
            let row_data = &padded_data[offset..offset + unpadded_bytes_per_row as usize];
            frame_data.extend_from_slice(row_data);
        }

        drop(padded_data);
        staging_buffer.unmap();

        Ok(frame_data)
    }

    /// Save the current frame as a PNG file.
    ///
    /// # Arguments
    /// * `path` - Path where the PNG file should be saved
    ///
    /// # Returns
    /// Ok(()) if the file was saved successfully.
    pub fn save_frame_png<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut frame_data = self.capture_frame_data()?;

        // Convert BGRA to RGBA by swapping B and R channels
        for chunk in frame_data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // Swap B and R
        }

        image::save_buffer(
            path.as_ref(),
            &frame_data,
            self.width,
            self.height,
            image::ColorType::Rgba8,
        )
        .context("Failed to save PNG file")?;

        Ok(())
    }

    /// Render with a deterministic camera path and return the hash.
    ///
    /// This is useful for regression testing where you want to ensure
    /// rendering output hasn't changed.
    ///
    /// # Arguments
    /// * `camera` - Fixed camera position for deterministic rendering
    ///
    /// # Returns
    /// The blake3 hash of the rendered frame.
    pub fn render_and_hash(&mut self, camera: Camera) -> Result<FrameHash> {
        self.render(camera)?;
        self.capture_frame_hash()
    }

    /// Get the render target dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RendererConfig, Camera};
    use mdminecraft_assets::{BlockDescriptor, BlockRegistry};

    fn test_registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor {
                name: "air".into(),
                opaque: false,
            },
            BlockDescriptor {
                name: "stone".into(),
                opaque: true,
            },
        ])
    }

    #[test]
    fn headless_renderer_creation() -> Result<()> {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let renderer = Renderer::new(config, &registry)?;
        let _headless = HeadlessRenderer::new(renderer)?;

        Ok(())
    }

    #[test]
    fn headless_rejects_non_headless_renderer() {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: false,
            ..Default::default()
        };
        let registry = test_registry();

        // This will fail because we can't create a windowed renderer in CI,
        // but if we could, the HeadlessRenderer should reject it
        if let Ok(renderer) = Renderer::new(config, &registry) {
            let result = HeadlessRenderer::new(renderer);
            assert!(result.is_err());
        }
    }

    #[test]
    fn headless_render_and_hash() -> Result<()> {
        let config = RendererConfig {
            width: 800,
            height: 600,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let renderer = Renderer::new(config, &registry)?;
        let mut headless = HeadlessRenderer::new(renderer)?;

        let camera = Camera {
            position: [10.0, 10.0, 10.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        };

        let hash = headless.render_and_hash(camera)?;

        // Hash should be deterministic - render again and verify
        let hash2 = headless.render_and_hash(camera)?;
        assert_eq!(hash, hash2);

        Ok(())
    }

    #[test]
    fn headless_dimensions() -> Result<()> {
        let config = RendererConfig {
            width: 640,
            height: 480,
            headless: true,
            ..Default::default()
        };
        let registry = test_registry();

        let renderer = Renderer::new(config, &registry)?;
        let headless = HeadlessRenderer::new(renderer)?;

        let (w, h) = headless.dimensions();
        assert_eq!(w, 640);
        assert_eq!(h, 480);

        Ok(())
    }
}
