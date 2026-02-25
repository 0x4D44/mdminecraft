//! Texture loading from PNG files.
//!
//! This module handles loading block textures from asset packs,
//! validating dimensions, and generating placeholder textures.

use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

/// Standard block texture size.
const TEXTURE_SIZE: u32 = 16;

/// A loaded texture with pixel data and dimensions.
#[derive(Debug, Clone)]
pub struct LoadedTexture {
    /// RGBA8 pixel data.
    pub data: Vec<u8>,
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
}

/// Loads textures from files and generates placeholders.
pub struct TextureLoader;

impl TextureLoader {
    /// Create a new texture loader.
    pub fn new() -> Self {
        Self
    }

    /// Load a texture from a PNG file.
    ///
    /// # Arguments
    /// * `path` - Path to the PNG file
    ///
    /// # Returns
    /// A LoadedTexture if successful, or an error if the file is invalid.
    /// Non-16x16 textures will be resized to 16x16.
    pub fn load_from_file(&self, path: &str) -> Result<LoadedTexture> {
        let img =
            image::open(path).with_context(|| format!("Failed to load texture from {}", path))?;

        let (width, height) = img.dimensions();

        // Resize if not 16x16
        let img = if width != TEXTURE_SIZE || height != TEXTURE_SIZE {
            tracing::warn!(
                "Texture {} is {}x{}, resizing to {}x{}",
                path,
                width,
                height,
                TEXTURE_SIZE,
                TEXTURE_SIZE
            );
            img.resize_exact(
                TEXTURE_SIZE,
                TEXTURE_SIZE,
                image::imageops::FilterType::Nearest,
            )
        } else {
            img
        };

        // Convert to RGBA8
        let rgba_img = img.to_rgba8();
        let data = rgba_img.into_raw();

        Ok(LoadedTexture {
            data,
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
        })
    }

    /// Generate a placeholder texture (magenta/black checkerboard).
    ///
    /// This is used when a texture is missing from the asset pack.
    pub fn generate_placeholder(&self) -> LoadedTexture {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(TEXTURE_SIZE, TEXTURE_SIZE);

        // Create magenta/black checkerboard pattern
        for y in 0..TEXTURE_SIZE {
            for x in 0..TEXTURE_SIZE {
                let checker_size = 4; // 4x4 checkerboard squares
                let is_magenta = ((x / checker_size) + (y / checker_size)) % 2 == 0;

                let color = if is_magenta {
                    Rgba([255u8, 0u8, 255u8, 255u8]) // Magenta
                } else {
                    Rgba([0u8, 0u8, 0u8, 255u8]) // Black
                };

                img.put_pixel(x, y, color);
            }
        }

        LoadedTexture {
            data: img.into_raw(),
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
        }
    }
}

impl Default for TextureLoader {
    fn default() -> Self {
        Self::new()
    }
}
