use anyhow::{Context, Result};
use std::path::Path;
use std::sync::mpsc;

/// Pending GPU readback of a texture.
pub struct TextureReadback {
    buffer: wgpu::Buffer,
    padded_bytes_per_row: u32,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl TextureReadback {
    /// Read back the texture contents as tightly packed RGBA8 bytes.
    pub fn read_rgba8(self, device: &wgpu::Device) -> Result<Vec<u8>> {
        let slice = self.buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });

        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .context("texture readback channel closed")?
            .context("texture readback failed")?;

        let mapped = slice.get_mapped_range();
        let unpadded_bytes_per_row = self.width * 4;
        let mut rgba = vec![0u8; (unpadded_bytes_per_row * self.height) as usize];

        for row in 0..self.height {
            let src_offset = (row * self.padded_bytes_per_row) as usize;
            let dst_offset = (row * unpadded_bytes_per_row) as usize;
            let src = &mapped[src_offset..src_offset + unpadded_bytes_per_row as usize];
            let dst = &mut rgba[dst_offset..dst_offset + unpadded_bytes_per_row as usize];
            dst.copy_from_slice(src);
        }

        drop(mapped);
        self.buffer.unmap();

        match self.format {
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {}
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            other => {
                return Err(anyhow::anyhow!(
                    "unsupported texture format for screenshot readback: {other:?}"
                ));
            }
        }

        Ok(rgba)
    }

    /// Pixel dimensions of the readback texture.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Record commands to copy `texture` into a mappable buffer.
///
/// Call [`TextureReadback::read_rgba8`] after submitting the encoder.
pub fn record_texture_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    size: (u32, u32),
) -> TextureReadback {
    let (width, height) = size;
    let bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = bytes_per_row.div_ceil(align) * align;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Screenshot Readback Buffer"),
        size: padded_bytes_per_row as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    TextureReadback {
        buffer,
        padded_bytes_per_row,
        width,
        height,
        format,
    }
}

/// Write an RGBA8 image to disk as a PNG.
pub fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<()> {
    let (width, height) = size;
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .context("failed to build screenshot image")?;
    image::DynamicImage::ImageRgba8(image)
        .save_with_format(path, image::ImageFormat::Png)
        .context("failed to write screenshot png")?;
    Ok(())
}
