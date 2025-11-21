use std::{collections::HashSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur while loading or validating atlas metadata.
#[derive(Debug, Error)]
pub enum AtlasError {
    /// Wrap IO failures when reading metadata files.
    #[error("failed to read atlas metadata: {0}")]
    Io(#[from] std::io::Error),
    /// Wrap JSON parsing issues.
    #[error("failed to parse atlas metadata: {0}")]
    Parse(#[from] serde_json::Error),
    /// Validation errors describing why metadata is inconsistent.
    #[error("invalid atlas metadata: {0}")]
    Invalid(String),
}

/// Serialized entry describing the UV/cell for a single texture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtlasEntry {
    /// Logical identifier (e.g., "blocks/stone").
    pub name: String,
    /// X offset in pixels within the atlas (top-left origin).
    pub x: u32,
    /// Y offset in pixels within the atlas (top-left origin).
    pub y: u32,
    /// Width in pixels (typically `tile_size`).
    pub width: u32,
    /// Height in pixels (typically `tile_size`).
    pub height: u32,
    /// Normalized left U coordinate.
    pub u0: f32,
    /// Normalized top V coordinate.
    pub v0: f32,
    /// Normalized right U coordinate.
    pub u1: f32,
    /// Normalized bottom V coordinate.
    pub v1: f32,
}

/// Metadata emitted by `atlas_packer` describing the full atlas layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextureAtlasMetadata {
    /// Tile size in pixels.
    pub tile_size: u32,
    /// Padding around each tile in pixels.
    pub padding: u32,
    /// Number of columns in the atlas grid.
    pub columns: u32,
    /// Number of rows in the atlas grid.
    pub rows: u32,
    /// Atlas width in pixels.
    pub atlas_width: u32,
    /// Atlas height in pixels.
    pub atlas_height: u32,
    /// Entries for each texture.
    pub entries: Vec<AtlasEntry>,
}

impl TextureAtlasMetadata {
    /// Parse metadata from a JSON string and validate contents.
    pub fn parse_str(input: &str) -> Result<Self, AtlasError> {
        let metadata: TextureAtlasMetadata = serde_json::from_str(input)?;
        metadata.validate()?;
        Ok(metadata)
    }

    /// Load metadata from a file on disk.
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self, AtlasError> {
        let data = fs::read_to_string(path)?;
        Self::parse_str(&data)
    }

    /// Validate the structure of the metadata and return `Ok(())` if consistent.
    pub fn validate(&self) -> Result<(), AtlasError> {
        if self.tile_size == 0 {
            return Err(AtlasError::Invalid("tile_size must be > 0".into()));
        }
        if self.columns == 0 || self.rows == 0 {
            return Err(AtlasError::Invalid(
                "columns and rows must be greater than zero".into(),
            ));
        }
        let stride = self.tile_size + self.padding * 2;
        let expected_width = self.columns * stride;
        let expected_height = self.rows * stride;
        if self.atlas_width != expected_width {
            return Err(AtlasError::Invalid(format!(
                "atlas_width mismatch (got {}, expected {})",
                self.atlas_width, expected_width
            )));
        }
        if self.atlas_height != expected_height {
            return Err(AtlasError::Invalid(format!(
                "atlas_height mismatch (got {}, expected {})",
                self.atlas_height, expected_height
            )));
        }
        let mut seen = HashSet::new();
        for entry in &self.entries {
            if entry.width != self.tile_size || entry.height != self.tile_size {
                return Err(AtlasError::Invalid(format!(
                    "entry {} has unexpected dimensions {}x{}",
                    entry.name, entry.width, entry.height
                )));
            }
            if entry.x + entry.width > self.atlas_width
                || entry.y + entry.height > self.atlas_height
            {
                return Err(AtlasError::Invalid(format!(
                    "entry {} exceeds atlas bounds",
                    entry.name
                )));
            }
            if !seen.insert(&entry.name) {
                return Err(AtlasError::Invalid(format!(
                    "duplicate atlas entry '{}'",
                    entry.name
                )));
            }
        }
        Ok(())
    }

    /// Lookup a texture entry by name.
    pub fn entry(&self, name: &str) -> Option<&AtlasEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_basic_metadata() {
        let json = r#"{
            "tile_size": 16,
            "padding": 2,
            "columns": 2,
            "rows": 1,
            "atlas_width": 40,
            "atlas_height": 20,
            "entries": [
                {"name":"a","x":2,"y":2,"width":16,"height":16,"u0":0.05,"v0":0.1,"u1":0.45,"v1":0.9},
                {"name":"b","x":22,"y":2,"width":16,"height":16,"u0":0.55,"v0":0.1,"u1":0.95,"v1":0.9}
            ]
        }"#;
        let atlas = TextureAtlasMetadata::parse_str(json).unwrap();
        assert_eq!(atlas.tile_size, 16);
        assert!(atlas.entry("a").is_some());
    }

    #[test]
    fn detects_out_of_bounds() {
        let json = r#"{
            "tile_size": 16,
            "padding": 1,
            "columns": 1,
            "rows": 1,
            "atlas_width": 18,
            "atlas_height": 18,
            "entries": [
                {"name":"a","x":10,"y":10,"width":16,"height":16,"u0":0.0,"v0":0.0,"u1":1.0,"v1":1.0}
            ]
        }"#;
        let err = TextureAtlasMetadata::parse_str(json).unwrap_err();
        assert!(matches!(err, AtlasError::Invalid(_)));
    }
}
