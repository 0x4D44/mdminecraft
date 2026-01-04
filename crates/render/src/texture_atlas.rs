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

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_assets::AtlasEntry;
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
}
