use anyhow::{Context, Result};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::ColorType;
use image::ImageEncoder;
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
    let file = std::fs::File::create(path).context("failed to create screenshot png")?;
    let encoder = PngEncoder::new_with_quality(file, CompressionType::Fast, FilterType::NoFilter);
    encoder
        .write_image(rgba, width, height, ColorType::Rgba8.into())
        .context("failed to write screenshot png")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(prefix: &str, ext: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nanos}.{ext}"))
    }

    fn test_device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))
        .expect("adapter");

        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device")
    }

    fn read_back_texture(
        format: wgpu::TextureFormat,
        size: (u32, u32),
        data: &[u8],
    ) -> Vec<u8> {
        let (device, queue) = test_device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Texture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: Some(size.1),
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let readback = record_texture_readback(&device, &mut encoder, &texture, format, size);
        queue.submit(Some(encoder.finish()));

        readback.read_rgba8(&device).expect("read back")
    }

    #[test]
    fn write_png_round_trip() {
        let path = temp_path("mdm_screenshot", "png");
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        write_png(&path, (2, 2), &rgba).expect("write png");

        let image = image::open(&path).expect("open image").to_rgba8();
        assert_eq!(image.as_raw(), &rgba);
    }

    #[test]
    fn readback_rgba8_matches_source() {
        let rgba = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let result = read_back_texture(wgpu::TextureFormat::Rgba8Unorm, (2, 1), &rgba);
        assert_eq!(result, rgba);
    }

    #[test]
    fn readback_bgra8_swaps_channels() {
        let bgra = vec![3, 2, 1, 4, 7, 6, 5, 8];
        let result = read_back_texture(wgpu::TextureFormat::Bgra8Unorm, (2, 1), &bgra);
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
