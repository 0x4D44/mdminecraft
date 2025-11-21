use std::{env, path::PathBuf};

use image::ImageReader;
use mdminecraft_assets::{AtlasError, TextureAtlasMetadata};
use thiserror::Error;
use tracing::warn;

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
    /// RGBA pixels (width × height × 4).
    pub pixels: Vec<u8>,
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

        Ok(RuntimeAtlas {
            metadata,
            pixels: rgba.into_raw(),
        })
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
