//! Texture atlas system for deterministic texture packing.
//!
//! This module provides a texture atlas that packs multiple block textures
//! into a single GPU texture for efficient rendering. The packing algorithm
//! is deterministic to ensure reproducible rendering across runs.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;

/// Standard block texture size in Minecraft.
const TEXTURE_SIZE: u32 = 16;
/// Maximum atlas dimension (power of 2).
const MAX_ATLAS_SIZE: u32 = 4096;

/// Unique identifier for a texture in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(u32);

/// UV coordinates for a texture in the atlas.
/// Coordinates are normalized to [0,1] range.
#[derive(Debug, Clone, Copy)]
pub struct UvCoordinates {
    /// Minimum U coordinate (left edge).
    pub min_u: f32,
    /// Minimum V coordinate (top edge).
    pub min_v: f32,
    /// Maximum U coordinate (right edge).
    pub max_u: f32,
    /// Maximum V coordinate (bottom edge).
    pub max_v: f32,
}

/// Internal texture entry.
#[derive(Clone)]
struct TextureEntry {
    name: String,
    data: Vec<u8>,
    x: u32,
    y: u32,
}

/// A texture atlas that packs multiple textures into a single GPU texture.
///
/// The atlas uses a deterministic packing algorithm to ensure that the same
/// set of textures always produces the same layout. This is critical for
/// reproducible rendering in tests and snapshots.
pub struct TextureAtlas {
    /// List of textures in insertion order.
    textures: Vec<TextureEntry>,
    /// Map from texture name to ID.
    name_to_id: HashMap<String, TextureId>,
    /// Atlas dimensions (0 until built).
    width: u32,
    height: u32,
    /// Packed atlas data.
    data: Vec<u8>,
    /// Blake3 hash of the atlas.
    hash: String,
    /// Whether the atlas has been built.
    built: bool,
}

impl TextureAtlas {
    /// Create a new empty texture atlas.
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            name_to_id: HashMap::new(),
            width: 0,
            height: 0,
            data: Vec::new(),
            hash: String::new(),
            built: false,
        }
    }

    /// Add a texture to the atlas.
    ///
    /// # Arguments
    /// * `name` - Unique name for the texture (e.g., "stone", "dirt")
    /// * `data` - RGBA8 pixel data
    /// * `width` - Texture width (must be 16)
    /// * `height` - Texture height (must be 16)
    ///
    /// # Returns
    /// A TextureId if the texture was added successfully, None if invalid.
    pub fn add_texture(
        &mut self,
        name: &str,
        data: &[u8],
        width: u32,
        height: u32,
    ) -> Option<TextureId> {
        // Validate dimensions
        if width != TEXTURE_SIZE || height != TEXTURE_SIZE {
            return None;
        }

        // Validate data size (RGBA = 4 bytes per pixel)
        let expected_size = (TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize;
        if data.len() != expected_size {
            return None;
        }

        // Check for duplicate names
        if self.name_to_id.contains_key(name) {
            return None;
        }

        // Add texture
        let id = TextureId(self.textures.len() as u32);
        self.textures.push(TextureEntry {
            name: name.to_string(),
            data: data.to_vec(),
            x: 0,
            y: 0,
        });
        self.name_to_id.insert(name.to_string(), id);

        // Mark as not built since we added a texture
        self.built = false;

        Some(id)
    }

    /// Build the atlas, packing all added textures.
    ///
    /// This must be called before querying UV coordinates or creating GPU textures.
    /// The packing algorithm is deterministic and will always produce the same
    /// layout for the same set of textures in the same order.
    pub fn build(&mut self) -> Result<()> {
        if self.textures.is_empty() {
            bail!("Cannot build empty atlas");
        }

        // Calculate required dimensions using row-based packing
        let texture_count = self.textures.len();
        let textures_per_row = (MAX_ATLAS_SIZE / TEXTURE_SIZE) as usize;

        // Calculate rows needed
        let rows_needed = texture_count.div_ceil(textures_per_row);

        // Check if we exceed max size
        if rows_needed * TEXTURE_SIZE as usize > MAX_ATLAS_SIZE as usize {
            bail!(
                "Too many textures: {} textures would require {}x{} atlas (max {}x{})",
                texture_count,
                textures_per_row * TEXTURE_SIZE as usize,
                rows_needed * TEXTURE_SIZE as usize,
                MAX_ATLAS_SIZE,
                MAX_ATLAS_SIZE
            );
        }

        // Calculate power-of-2 dimensions
        let textures_in_last_row = texture_count % textures_per_row;
        let cols_needed = if textures_in_last_row == 0 {
            textures_per_row
        } else {
            textures_in_last_row.max(
                (texture_count / rows_needed)
                    + if !texture_count.is_multiple_of(rows_needed) {
                        1
                    } else {
                        0
                    },
            )
        };

        self.width = (cols_needed as u32 * TEXTURE_SIZE).next_power_of_two();
        self.height = (rows_needed as u32 * TEXTURE_SIZE).next_power_of_two();

        // Ensure minimum size
        self.width = self.width.max(TEXTURE_SIZE);
        self.height = self.height.max(TEXTURE_SIZE);

        // Create atlas data buffer
        let data_size = (self.width * self.height * 4) as usize;
        self.data = vec![0u8; data_size];

        // Pack textures in row-major order
        // First pass: calculate positions and collect data to copy
        let mut textures_to_copy = Vec::new();
        for (idx, texture) in self.textures.iter_mut().enumerate() {
            let col = idx % textures_per_row;
            let row = idx / textures_per_row;

            texture.x = col as u32 * TEXTURE_SIZE;
            texture.y = row as u32 * TEXTURE_SIZE;

            textures_to_copy.push((texture.data.clone(), texture.x, texture.y));
        }

        // Second pass: copy texture data into atlas
        for (data, x, y) in textures_to_copy {
            self.copy_texture_to_atlas(&data, x, y);
        }

        // Generate blake3 hash
        let hash_bytes = blake3::hash(&self.data);
        self.hash = hash_bytes.to_hex().to_string();

        self.built = true;
        Ok(())
    }

    /// Copy a texture's data into the atlas at the specified position.
    fn copy_texture_to_atlas(&mut self, texture_data: &[u8], x: u32, y: u32) {
        let atlas_width = self.width;

        for row in 0..TEXTURE_SIZE {
            let src_offset = (row * TEXTURE_SIZE * 4) as usize;
            let dst_offset = (((y + row) * atlas_width + x) * 4) as usize;
            let row_size = (TEXTURE_SIZE * 4) as usize;

            self.data[dst_offset..dst_offset + row_size]
                .copy_from_slice(&texture_data[src_offset..src_offset + row_size]);
        }
    }

    /// Get the number of textures in the atlas.
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Get the width of the atlas in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the height of the atlas in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get UV coordinates for a texture.
    ///
    /// Returns None if the atlas hasn't been built yet or if the ID is invalid.
    pub fn get_uv(&self, id: TextureId) -> Option<UvCoordinates> {
        if !self.built {
            return None;
        }

        let texture = self.textures.get(id.0 as usize)?;

        let min_u = texture.x as f32 / self.width as f32;
        let min_v = texture.y as f32 / self.height as f32;
        let max_u = (texture.x + TEXTURE_SIZE) as f32 / self.width as f32;
        let max_v = (texture.y + TEXTURE_SIZE) as f32 / self.height as f32;

        Some(UvCoordinates {
            min_u,
            min_v,
            max_u,
            max_v,
        })
    }

    /// Get the blake3 hash of the atlas data.
    ///
    /// This hash uniquely identifies the atlas layout and can be used for
    /// snapshot testing and cache invalidation.
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    /// Get the raw RGBA8 pixel data of the atlas.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Look up a texture ID by name.
    pub fn get_texture_id(&self, name: &str) -> Option<TextureId> {
        self.name_to_id.get(name).copied()
    }

    /// Create a GPU texture from the atlas.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue for uploading data
    pub fn create_gpu_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture> {
        if !self.built {
            bail!("Atlas must be built before creating GPU texture");
        }

        let size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture_atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload data to GPU
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            size,
        );

        Ok(texture)
    }
}

impl Default for TextureAtlas {
    fn default() -> Self {
        Self::new()
    }
}
