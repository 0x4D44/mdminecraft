//! Texture atlas system for efficient GPU texture management.
//!
//! This module provides:
//! - `TextureAtlas`: A deterministic in-memory texture atlas that packs
//!   multiple block textures into a single GPU texture.
//! - `RuntimeAtlas`: Disk-based atlas loading from authored assets.

use std::collections::HashMap;
use std::{env, path::PathBuf};

use image::ImageReader;
use mdminecraft_assets::{AtlasError, TextureAtlasMetadata};
use thiserror::Error;
use tracing::warn;

// ---------------------------------------------------------------------------
// RuntimeAtlas (disk-based, authored assets)
// ---------------------------------------------------------------------------

/// Runtime error when attempting to load the authored texture atlas.
#[derive(Debug, Error)]
pub enum RuntimeAtlasError {
    /// Metadata loading/validation failed.
    #[error(transparent)]
    Metadata(#[from] AtlasError),
    /// Image decoding failed.
    #[error("failed to decode atlas image: {0}")]
    Image(#[from] image::ImageError),
    /// Image did not match metadata-provided dimensions.
    #[error("atlas image dimensions {found_width}x{found_height} do not match metadata {expected_width}x{expected_height}")]
    DimensionMismatch {
        /// Width reported by metadata.
        expected_width: u32,
        /// Height reported by metadata.
        expected_height: u32,
        /// Width read from the PNG.
        found_width: u32,
        /// Height read from the PNG.
        found_height: u32,
    },
    /// Generic IO failure.
    #[error("failed to load atlas assets: {0}")]
    Io(#[from] std::io::Error),
}

/// Loaded atlas image + metadata ready for GPU upload.
pub struct RuntimeAtlas {
    /// Metadata describing tile layout/UVs.
    pub metadata: TextureAtlasMetadata,
    /// RGBA pixels (width * height * 4).
    pub pixels: Vec<u8>,
}

fn bleed_atlas_tile_padding(metadata: &TextureAtlasMetadata, pixels: &mut [u8]) {
    let pad = metadata.padding as i32;
    if pad <= 0 {
        return;
    }

    let width = metadata.atlas_width as i32;
    let height = metadata.atlas_height as i32;
    let tile = metadata.tile_size as i32;
    if width <= 0 || height <= 0 || tile <= 0 {
        return;
    }

    let stride = (width as usize) * 4;
    if pixels.len() < stride * height as usize {
        return;
    }

    let get = |x: i32, y: i32, pixels: &[u8]| -> [u8; 4] {
        let idx = (y as usize * stride) + (x as usize * 4);
        [
            pixels[idx],
            pixels[idx + 1],
            pixels[idx + 2],
            pixels[idx + 3],
        ]
    };

    let set = |x: i32, y: i32, color: [u8; 4], pixels: &mut [u8]| {
        if x < 0 || y < 0 || x >= width || y >= height {
            return;
        }
        let idx = (y as usize * stride) + (x as usize * 4);
        pixels[idx] = color[0];
        pixels[idx + 1] = color[1];
        pixels[idx + 2] = color[2];
        pixels[idx + 3] = color[3];
    };

    for entry in &metadata.entries {
        let x0 = entry.x as i32;
        let y0 = entry.y as i32;
        let x1 = x0 + tile - 1;
        let y1 = y0 + tile - 1;

        if x0 < 0 || y0 < 0 || x1 >= width || y1 >= height {
            continue;
        }

        for dy in 0..tile {
            let y = y0 + dy;
            let left = get(x0, y, pixels);
            for dx in 1..=pad {
                set(x0 - dx, y, left, pixels);
            }

            let right = get(x1, y, pixels);
            for dx in 1..=pad {
                set(x1 + dx, y, right, pixels);
            }
        }

        for dx in 0..tile {
            let x = x0 + dx;
            let top = get(x, y0, pixels);
            for dy in 1..=pad {
                set(x, y0 - dy, top, pixels);
            }

            let bottom = get(x, y1, pixels);
            for dy in 1..=pad {
                set(x, y1 + dy, bottom, pixels);
            }
        }

        let top_left = get(x0, y0, pixels);
        for dy in 1..=pad {
            for dx in 1..=pad {
                set(x0 - dx, y0 - dy, top_left, pixels);
            }
        }

        let top_right = get(x1, y0, pixels);
        for dy in 1..=pad {
            for dx in 1..=pad {
                set(x1 + dx, y0 - dy, top_right, pixels);
            }
        }

        let bottom_left = get(x0, y1, pixels);
        for dy in 1..=pad {
            for dx in 1..=pad {
                set(x0 - dx, y1 + dy, bottom_left, pixels);
            }
        }

        let bottom_right = get(x1, y1, pixels);
        for dy in 1..=pad {
            for dx in 1..=pad {
                set(x1 + dx, y1 + dy, bottom_right, pixels);
            }
        }
    }
}

impl RuntimeAtlas {
    /// Attempt to load the atlas assets from disk, falling back to defaults on failure.
    pub fn load_from_disk() -> Result<Self, RuntimeAtlasError> {
        let (meta_path, image_path) = atlas_paths();
        if !meta_path.exists() || !image_path.exists() {
            return Err(RuntimeAtlasError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "atlas files not found (metadata: {}, image: {})",
                    meta_path.display(),
                    image_path.display()
                ),
            )));
        }

        let metadata = TextureAtlasMetadata::load_file(&meta_path)?;
        let image = ImageReader::open(&image_path)?.decode()?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        if width != metadata.atlas_width || height != metadata.atlas_height {
            return Err(RuntimeAtlasError::DimensionMismatch {
                expected_width: metadata.atlas_width,
                expected_height: metadata.atlas_height,
                found_width: width,
                found_height: height,
            });
        }

        let mut pixels = rgba.into_raw();
        bleed_atlas_tile_padding(&metadata, &mut pixels);

        Ok(RuntimeAtlas { metadata, pixels })
    }
}

const DEFAULT_META_PATH: &str = "assets/atlas/atlas.json";
const DEFAULT_IMAGE_PATH: &str = "assets/atlas/atlas.png";

fn atlas_paths() -> (PathBuf, PathBuf) {
    let meta = env::var("MDM_ATLAS_META").unwrap_or_else(|_| DEFAULT_META_PATH.to_string());
    let image = env::var("MDM_ATLAS_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE_PATH.to_string());
    (PathBuf::from(meta), PathBuf::from(image))
}

/// Utility to check whether authored atlas assets exist on disk.
pub fn atlas_exists() -> bool {
    let (meta, image) = atlas_paths();
    meta.exists() && image.exists()
}

/// Log a helpful warning if atlas loading fails.
pub fn warn_missing_atlas(err: &RuntimeAtlasError) {
    warn!("Falling back to debug texture atlas: {err}");
}

// ---------------------------------------------------------------------------
// TextureAtlas (in-memory, deterministic packing)
// ---------------------------------------------------------------------------

/// Handle to a texture in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// UV coordinates for a texture region in the atlas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    /// Minimum U coordinate (0.0 to 1.0).
    pub u_min: f32,
    /// Minimum V coordinate (0.0 to 1.0).
    pub v_min: f32,
    /// Maximum U coordinate (0.0 to 1.0).
    pub u_max: f32,
    /// Maximum V coordinate (0.0 to 1.0).
    pub v_max: f32,
}

/// Configuration for the texture atlas.
#[derive(Debug, Clone)]
pub struct AtlasConfig {
    /// Maximum atlas width/height (must be power of 2).
    pub max_size: u32,
    /// Whether to use a placeholder for missing textures.
    pub use_placeholder: bool,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        Self {
            max_size: 4096,
            use_placeholder: true,
        }
    }
}

/// Texture atlas that packs multiple textures into a single GPU texture.
#[allow(dead_code)] // Fields used in implementation
pub struct TextureAtlas {
    /// Atlas configuration.
    config: AtlasConfig,
    /// Width of the atlas in pixels.
    width: u32,
    /// Height of the atlas in pixels.
    height: u32,
    /// RGBA pixel data.
    pixels: Vec<u8>,
    /// Mapping from texture handles to UV coordinates.
    uv_map: HashMap<TextureHandle, UvRect>,
    /// Hash of the atlas for snapshot stability.
    hash: AtlasHash,
}

/// Hash of the texture atlas for snapshot stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasHash(pub [u8; 32]);

impl TextureAtlas {
    /// Create a new empty texture atlas.
    pub fn new(config: AtlasConfig) -> Self {
        Self {
            config,
            width: 0,
            height: 0,
            pixels: Vec::new(),
            uv_map: HashMap::new(),
            hash: AtlasHash([0; 32]),
        }
    }

    /// Build an atlas from a list of textures.
    pub fn build(textures: Vec<RawTexture>, config: AtlasConfig) -> anyhow::Result<Self> {
        // Handle empty texture list
        if textures.is_empty() {
            return Ok(Self::new(config));
        }

        // Use row-based packing for determinism
        let max_texture_width = textures.iter().map(|t| t.width).max().unwrap();
        let textures_per_row = (config.max_size / max_texture_width).max(1);

        // Calculate required dimensions
        let rows_needed = (textures.len() as u32).div_ceil(textures_per_row);
        let max_texture_height = textures.iter().map(|t| t.height).max().unwrap();

        // Calculate power-of-2 dimensions
        let width = (textures_per_row * max_texture_width)
            .next_power_of_two()
            .min(config.max_size);
        let height = (rows_needed * max_texture_height)
            .next_power_of_two()
            .min(config.max_size);

        // Create pixel buffer
        let pixel_count = (width * height * 4) as usize;
        let mut pixels = vec![0u8; pixel_count];

        // Pack textures and build UV map
        let mut uv_map = HashMap::new();

        for (idx, texture) in textures.iter().enumerate() {
            let col = (idx as u32) % textures_per_row;
            let row = (idx as u32) / textures_per_row;

            let x = col * max_texture_width;
            let y = row * max_texture_height;

            // Copy texture data into atlas
            Self::copy_texture(
                &mut pixels,
                width,
                &texture.pixels,
                texture.width,
                texture.height,
                x,
                y,
            );

            // Calculate UV coordinates
            let u_min = x as f32 / width as f32;
            let v_min = y as f32 / height as f32;
            let u_max = (x + texture.width) as f32 / width as f32;
            let v_max = (y + texture.height) as f32 / height as f32;

            uv_map.insert(
                TextureHandle(idx as u32),
                UvRect {
                    u_min,
                    v_min,
                    u_max,
                    v_max,
                },
            );
        }

        // Generate blake3 hash
        let hash_bytes = blake3::hash(&pixels);
        let hash = AtlasHash(*hash_bytes.as_bytes());

        Ok(Self {
            config,
            width,
            height,
            pixels,
            uv_map,
            hash,
        })
    }

    /// Copy texture data into atlas at specified position.
    fn copy_texture(
        atlas_pixels: &mut [u8],
        atlas_width: u32,
        texture_pixels: &[u8],
        texture_width: u32,
        texture_height: u32,
        x: u32,
        y: u32,
    ) {
        for row in 0..texture_height {
            let src_offset = (row * texture_width * 4) as usize;
            let dst_offset = (((y + row) * atlas_width + x) * 4) as usize;
            let row_size = (texture_width * 4) as usize;

            if src_offset + row_size <= texture_pixels.len()
                && dst_offset + row_size <= atlas_pixels.len()
            {
                atlas_pixels[dst_offset..dst_offset + row_size]
                    .copy_from_slice(&texture_pixels[src_offset..src_offset + row_size]);
            }
        }
    }

    /// Get the UV coordinates for a texture handle.
    pub fn uv_coords(&self, handle: TextureHandle) -> Option<UvRect> {
        if self.config.use_placeholder && !self.uv_map.contains_key(&handle) {
            // Return placeholder UV coords (first texture or a default)
            if let Some(uv) = self.uv_map.get(&TextureHandle(0)).copied() {
                Some(uv)
            } else {
                // If atlas is empty, return a default UV rect
                Some(UvRect {
                    u_min: 0.0,
                    v_min: 0.0,
                    u_max: 1.0,
                    v_max: 1.0,
                })
            }
        } else {
            self.uv_map.get(&handle).copied()
        }
    }

    /// Get the atlas dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the atlas pixel data.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Get the atlas hash.
    pub fn hash(&self) -> AtlasHash {
        self.hash
    }

    /// Rebuild the atlas with new textures.
    pub fn rebuild(&mut self, textures: Vec<RawTexture>) -> anyhow::Result<()> {
        let new_atlas = Self::build(textures, self.config.clone())?;
        self.width = new_atlas.width;
        self.height = new_atlas.height;
        self.pixels = new_atlas.pixels;
        self.uv_map = new_atlas.uv_map;
        self.hash = new_atlas.hash;
        Ok(())
    }
}

/// Raw texture data before atlas packing.
#[derive(Debug, Clone)]
pub struct RawTexture {
    /// Unique identifier for this texture.
    pub name: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA pixel data.
    pub pixels: Vec<u8>,
}

impl RawTexture {
    /// Create a new raw texture.
    pub fn new(name: String, width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            name,
            width,
            height,
            pixels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::{AtlasEntry, BlockDescriptor, BlockRegistry};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nanos}"))
    }

    fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * width + x) * 4) as usize;
        [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]]
    }

    #[test]
    fn load_from_disk_reads_metadata_and_bleeds_padding() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = temp_dir("mdm_atlas");
        std::fs::create_dir_all(&dir).expect("create dir");
        let meta_path = dir.join("atlas.json");
        let image_path = dir.join("atlas.png");

        let metadata = TextureAtlasMetadata {
            tile_size: 2,
            padding: 1,
            columns: 1,
            rows: 1,
            atlas_width: 4,
            atlas_height: 4,
            entries: vec![AtlasEntry {
                name: "tile".to_string(),
                x: 1,
                y: 1,
                width: 2,
                height: 2,
                u0: 0.25,
                v0: 0.25,
                u1: 0.75,
                v1: 0.75,
            }],
        };
        let json = serde_json::to_string_pretty(&metadata).expect("serialize metadata");
        std::fs::write(&meta_path, json).expect("write metadata");

        let mut image = image::RgbaImage::new(4, 4);
        for y in 1..=2 {
            for x in 1..=2 {
                image.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        image.save(&image_path).expect("save image");

        std::env::set_var("MDM_ATLAS_META", &meta_path);
        std::env::set_var("MDM_ATLAS_IMAGE", &image_path);

        let atlas = RuntimeAtlas::load_from_disk().expect("load atlas");
        assert_eq!(atlas.metadata, metadata);
        assert_eq!(pixel_at(&atlas.pixels, 4, 0, 1), [255, 0, 0, 255]);

        std::env::remove_var("MDM_ATLAS_META");
        std::env::remove_var("MDM_ATLAS_IMAGE");
    }

    #[test]
    fn load_from_disk_detects_dimension_mismatch() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = temp_dir("mdm_atlas_mismatch");
        std::fs::create_dir_all(&dir).expect("create dir");
        let meta_path = dir.join("atlas.json");
        let image_path = dir.join("atlas.png");

        let metadata = TextureAtlasMetadata {
            tile_size: 2,
            padding: 1,
            columns: 1,
            rows: 1,
            atlas_width: 4,
            atlas_height: 4,
            entries: vec![AtlasEntry {
                name: "tile".to_string(),
                x: 1,
                y: 1,
                width: 2,
                height: 2,
                u0: 0.25,
                v0: 0.25,
                u1: 0.75,
                v1: 0.75,
            }],
        };
        let json = serde_json::to_string_pretty(&metadata).expect("serialize metadata");
        std::fs::write(&meta_path, json).expect("write metadata");

        let image = image::RgbaImage::new(2, 2);
        image.save(&image_path).expect("save image");

        std::env::set_var("MDM_ATLAS_META", &meta_path);
        std::env::set_var("MDM_ATLAS_IMAGE", &image_path);

        let err = RuntimeAtlas::load_from_disk().err().expect("expect mismatch");
        match err {
            RuntimeAtlasError::DimensionMismatch { .. } => {}
            other => panic!("expected dimension mismatch, got {other:?}"),
        }

        std::env::remove_var("MDM_ATLAS_META");
        std::env::remove_var("MDM_ATLAS_IMAGE");
    }

    #[test]
    fn atlas_paths_respects_env_and_exists() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = temp_dir("mdm_atlas_missing");
        let meta_path = dir.join("missing.json");
        let image_path = dir.join("missing.png");

        std::env::set_var("MDM_ATLAS_META", &meta_path);
        std::env::set_var("MDM_ATLAS_IMAGE", &image_path);

        let (meta, image) = atlas_paths();
        assert_eq!(meta, meta_path);
        assert_eq!(image, image_path);
        assert!(!atlas_exists());

        std::env::remove_var("MDM_ATLAS_META");
        std::env::remove_var("MDM_ATLAS_IMAGE");
    }

    // --- TextureAtlas (in-memory) tests ---

    /// Helper to create a solid color texture for testing.
    fn create_test_texture(name: &str, width: u32, height: u32, color: [u8; 4]) -> RawTexture {
        let pixel_count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            pixels.extend_from_slice(&color);
        }
        RawTexture::new(name.to_string(), width, height, pixels)
    }

    /// Helper to create a checkerboard texture for testing.
    fn create_checkerboard_texture(
        name: &str,
        width: u32,
        height: u32,
        color1: [u8; 4],
        color2: [u8; 4],
    ) -> RawTexture {
        let mut pixels = Vec::with_capacity((width * height) as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let color = if (x + y) % 2 == 0 { color1 } else { color2 };
                pixels.extend_from_slice(&color);
            }
        }
        RawTexture::new(name.to_string(), width, height, pixels)
    }

    #[test]
    fn atlas_creation_empty() {
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::new(config);
        assert_eq!(atlas.dimensions(), (0, 0));
        assert!(atlas.pixels().is_empty());
    }

    #[test]
    fn atlas_creation_with_single_texture() {
        let texture = create_test_texture("stone", 16, 16, [128, 128, 128, 255]);
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(vec![texture], config).unwrap();

        assert!(atlas.dimensions().0 >= 16);
        assert!(atlas.dimensions().1 >= 16);
        assert!(!atlas.pixels().is_empty());

        let handle = TextureHandle(0);
        let uv = atlas.uv_coords(handle).expect("texture should exist");
        assert!(uv.u_min >= 0.0 && uv.u_min <= 1.0);
        assert!(uv.v_min >= 0.0 && uv.v_min <= 1.0);
        assert!(uv.u_max >= 0.0 && uv.u_max <= 1.0);
        assert!(uv.v_max >= 0.0 && uv.v_max <= 1.0);
        assert!(uv.u_max > uv.u_min);
        assert!(uv.v_max > uv.v_min);
    }

    #[test]
    fn atlas_creation_with_multiple_textures() {
        let textures = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
            create_test_texture("grass", 16, 16, [34, 139, 34, 255]),
            create_test_texture("wood", 16, 16, [160, 82, 45, 255]),
        ];
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(textures, config).unwrap();

        // All textures should be accessible
        for i in 0..4 {
            let handle = TextureHandle(i);
            let uv = atlas.uv_coords(handle).expect("texture should exist");
            assert!(uv.u_min >= 0.0 && uv.u_min <= 1.0);
            assert!(uv.v_min >= 0.0 && uv.v_min <= 1.0);
            assert!(uv.u_max > uv.u_min);
            assert!(uv.v_max > uv.v_min);
        }
    }

    #[test]
    fn atlas_deterministic_packing_same_order() {
        let textures1 = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
            create_test_texture("grass", 16, 16, [34, 139, 34, 255]),
        ];
        let textures2 = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
            create_test_texture("grass", 16, 16, [34, 139, 34, 255]),
        ];

        let config = AtlasConfig::default();
        let atlas1 = TextureAtlas::build(textures1, config.clone()).unwrap();
        let atlas2 = TextureAtlas::build(textures2, config).unwrap();

        // Same textures in same order should produce identical atlas
        assert_eq!(atlas1.dimensions(), atlas2.dimensions());
        assert_eq!(atlas1.hash(), atlas2.hash());

        // UV coordinates should match
        for i in 0..3 {
            let handle = TextureHandle(i);
            let uv1 = atlas1.uv_coords(handle).unwrap();
            let uv2 = atlas2.uv_coords(handle).unwrap();
            assert_eq!(uv1, uv2);
        }
    }

    #[test]
    fn atlas_deterministic_packing_different_order() {
        let textures1 = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
        ];
        let textures2 = vec![
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
        ];

        let config = AtlasConfig::default();
        let atlas1 = TextureAtlas::build(textures1, config.clone()).unwrap();
        let atlas2 = TextureAtlas::build(textures2, config).unwrap();

        // Different order should produce different layout but same dimensions
        assert_eq!(atlas1.dimensions(), atlas2.dimensions());
        // Hashes should be different because content is in different positions
        assert_ne!(atlas1.hash(), atlas2.hash());
    }

    #[test]
    fn atlas_uv_coordinates_for_block_faces() {
        // Test that UV coordinates can be used for block face rendering
        let texture = create_test_texture("stone", 16, 16, [128, 128, 128, 255]);
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(vec![texture], config).unwrap();

        let handle = TextureHandle(0);
        let uv = atlas.uv_coords(handle).unwrap();

        // Verify we can compute face corners
        let corners = [
            [uv.u_min, uv.v_min],
            [uv.u_max, uv.v_min],
            [uv.u_max, uv.v_max],
            [uv.u_min, uv.v_max],
        ];

        // All corners should be valid UV coordinates
        for corner in corners {
            assert!(corner[0] >= 0.0 && corner[0] <= 1.0);
            assert!(corner[1] >= 0.0 && corner[1] <= 1.0);
        }
    }

    #[test]
    fn atlas_hash_generation() {
        let texture = create_test_texture("stone", 16, 16, [128, 128, 128, 255]);
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(vec![texture], config).unwrap();

        let hash = atlas.hash();
        // Hash should be non-zero
        assert_ne!(hash, AtlasHash([0; 32]));
    }

    #[test]
    fn atlas_hash_changes_with_content() {
        let texture1 = create_test_texture("stone", 16, 16, [128, 128, 128, 255]);
        let texture2 = create_test_texture("stone", 16, 16, [129, 128, 128, 255]); // One pixel different

        let config = AtlasConfig::default();
        let atlas1 = TextureAtlas::build(vec![texture1], config.clone()).unwrap();
        let atlas2 = TextureAtlas::build(vec![texture2], config).unwrap();

        // Different content should produce different hash
        assert_ne!(atlas1.hash(), atlas2.hash());
    }

    #[test]
    fn atlas_power_of_two_dimensions() {
        let textures = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
        ];
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(textures, config).unwrap();

        let (width, height) = atlas.dimensions();
        // Dimensions should be power of 2
        assert!(width.is_power_of_two());
        assert!(height.is_power_of_two());
    }

    #[test]
    fn atlas_respects_max_size_limit() {
        // Create many textures that would exceed max size
        let mut textures = Vec::new();
        for i in 0..100 {
            textures.push(create_test_texture(
                &format!("tex{}", i),
                64,
                64,
                [i as u8, 0, 0, 255],
            ));
        }

        let config = AtlasConfig {
            max_size: 512,
            use_placeholder: true,
        };
        let atlas = TextureAtlas::build(textures, config).unwrap();

        let (width, height) = atlas.dimensions();
        assert!(width <= 512);
        assert!(height <= 512);
    }

    #[test]
    fn atlas_maximum_size_limit_4096() {
        // Test that atlas can handle up to 4096x4096
        let config = AtlasConfig {
            max_size: 4096,
            use_placeholder: true,
        };

        // This should work without error
        let texture = create_test_texture("large", 64, 64, [255, 0, 0, 255]);
        let atlas = TextureAtlas::build(vec![texture], config).unwrap();

        let (width, height) = atlas.dimensions();
        assert!(width <= 4096);
        assert!(height <= 4096);
    }

    #[test]
    fn atlas_missing_texture_placeholder() {
        // Test that missing textures are handled with placeholder
        let config = AtlasConfig {
            max_size: 4096,
            use_placeholder: true,
        };

        let atlas = TextureAtlas::new(config);

        // Request UV for non-existent texture
        let handle = TextureHandle(999);
        let uv = atlas.uv_coords(handle);

        if atlas.config.use_placeholder {
            // Should return placeholder UV coords
            assert!(uv.is_some());
        } else {
            // Should return None
            assert!(uv.is_none());
        }
    }

    #[test]
    fn atlas_rebuild_updates_hash() {
        let initial_textures = vec![create_test_texture("stone", 16, 16, [128, 128, 128, 255])];

        let config = AtlasConfig::default();
        let mut atlas = TextureAtlas::build(initial_textures, config).unwrap();
        let initial_hash = atlas.hash();

        let new_textures = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
        ];

        atlas.rebuild(new_textures).unwrap();
        let new_hash = atlas.hash();

        // Hash should change after rebuild
        assert_ne!(initial_hash, new_hash);
    }

    #[test]
    fn atlas_rebuild_maintains_determinism() {
        let textures = vec![
            create_test_texture("stone", 16, 16, [128, 128, 128, 255]),
            create_test_texture("dirt", 16, 16, [139, 69, 19, 255]),
        ];

        let config = AtlasConfig::default();
        let mut atlas1 = TextureAtlas::build(textures.clone(), config.clone()).unwrap();
        let mut atlas2 = TextureAtlas::build(textures.clone(), config).unwrap();

        let rebuilt_textures = vec![create_test_texture("grass", 16, 16, [34, 139, 34, 255])];

        atlas1.rebuild(rebuilt_textures.clone()).unwrap();
        atlas2.rebuild(rebuilt_textures).unwrap();

        // Both should have identical state after rebuild
        assert_eq!(atlas1.hash(), atlas2.hash());
        assert_eq!(atlas1.dimensions(), atlas2.dimensions());
    }

    #[test]
    fn atlas_integration_with_block_registry() {
        // Test that atlas can be integrated with block registry
        let registry = BlockRegistry::new(vec![
            BlockDescriptor {
                name: "air".into(),
                opaque: false,
            },
            BlockDescriptor {
                name: "stone".into(),
                opaque: true,
            },
            BlockDescriptor {
                name: "dirt".into(),
                opaque: true,
            },
        ]);

        // Create textures for each block
        let mut textures = Vec::new();
        for i in 0..3 {
            let desc = registry.descriptor(i).unwrap();
            let color = match desc.name.as_str() {
                "air" => [0, 0, 0, 0],
                "stone" => [128, 128, 128, 255],
                "dirt" => [139, 69, 19, 255],
                _ => [255, 0, 255, 255],
            };
            textures.push(create_test_texture(&desc.name, 16, 16, color));
        }

        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(textures, config).unwrap();

        // Verify we can look up textures by block ID
        for i in 0..3 {
            let handle = TextureHandle(i as u32);
            let uv = atlas.uv_coords(handle).expect("texture should exist");
            assert!(uv.u_max > uv.u_min);
            assert!(uv.v_max > uv.v_min);
        }
    }

    #[test]
    fn atlas_handles_varying_texture_sizes() {
        let textures = vec![
            create_test_texture("small", 8, 8, [255, 0, 0, 255]),
            create_test_texture("medium", 16, 16, [0, 255, 0, 255]),
            create_test_texture("large", 32, 32, [0, 0, 255, 255]),
        ];

        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(textures, config).unwrap();

        // All textures should be packed
        for i in 0..3 {
            let handle = TextureHandle(i);
            assert!(atlas.uv_coords(handle).is_some());
        }
    }

    #[test]
    fn atlas_pixel_data_format() {
        let texture = create_test_texture("stone", 16, 16, [128, 128, 128, 255]);
        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(vec![texture], config).unwrap();

        let pixels = atlas.pixels();
        let (width, height) = atlas.dimensions();

        // Pixel data should be RGBA (4 bytes per pixel)
        assert_eq!(pixels.len(), (width * height * 4) as usize);
    }

    #[test]
    fn atlas_preserves_texture_data() {
        // Create a unique checkerboard pattern
        let texture =
            create_checkerboard_texture("checker", 16, 16, [255, 0, 0, 255], [0, 0, 255, 255]);

        let config = AtlasConfig::default();
        let atlas = TextureAtlas::build(vec![texture.clone()], config).unwrap();

        let handle = TextureHandle(0);
        let uv = atlas.uv_coords(handle).unwrap();

        // Calculate pixel coordinates from UV
        let (atlas_width, atlas_height) = atlas.dimensions();
        let x = (uv.u_min * atlas_width as f32) as u32;
        let y = (uv.v_min * atlas_height as f32) as u32;

        // Sample first pixel from atlas
        let atlas_pixels = atlas.pixels();
        let pixel_index = ((y * atlas_width + x) * 4) as usize;

        if pixel_index + 4 <= atlas_pixels.len() {
            let sampled_pixel = [
                atlas_pixels[pixel_index],
                atlas_pixels[pixel_index + 1],
                atlas_pixels[pixel_index + 2],
                atlas_pixels[pixel_index + 3],
            ];

            // First pixel should match (red from checkerboard)
            assert_eq!(sampled_pixel, [255, 0, 0, 255]);
        }
    }

    #[test]
    fn atlas_empty_texture_list() {
        let config = AtlasConfig::default();
        let result = TextureAtlas::build(vec![], config);

        // Empty texture list should either create empty atlas or error
        match result {
            Ok(atlas) => {
                assert_eq!(atlas.dimensions(), (0, 0));
            }
            Err(_) => {
                // Also acceptable to return an error
            }
        }
    }
}
