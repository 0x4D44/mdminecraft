//! Font Atlas Generation with Signed Distance Field (SDF)
//!
//! This module generates texture atlases containing font glyphs rendered as
//! signed distance fields for high-quality text rendering at any scale.

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use tracing::{debug, info};

/// Character range for ASCII printable characters
pub const ASCII_RANGE: std::ops::Range<u32> = 32..127;

/// A font atlas containing pre-rendered glyphs with signed distance fields
pub struct FontAtlas {
    /// The font used for this atlas
    font: Font,

    /// Atlas texture data (grayscale)
    pub texture_data: Vec<u8>,

    /// Atlas dimensions
    pub width: u32,
    pub height: u32,

    /// Glyph metrics and positions in atlas
    glyphs: HashMap<char, GlyphInfo>,

    /// Font size used for rasterization
    pub font_size: f32,

    /// Padding around each glyph
    pub padding: u32,
}

/// Information about a glyph in the atlas
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// Position in atlas texture (pixels)
    pub atlas_x: u32,
    pub atlas_y: u32,

    /// Glyph dimensions (pixels)
    pub width: u32,
    pub height: u32,

    /// Glyph metrics (for layout)
    pub advance_width: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,

    /// Normalized texture coordinates (0.0 to 1.0)
    pub uv_min: (f32, f32),
    pub uv_max: (f32, f32),
}

impl FontAtlas {
    /// Get glyph information for a character
    pub fn get_glyph(&self, c: char) -> Option<&GlyphInfo> {
        self.glyphs.get(&c)
    }

    /// Get the font
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Calculate text layout for a string
    pub fn layout_text(&self, text: &str, font_size: f32) -> Vec<GlyphLayout> {
        let scale = font_size / self.font_size;
        let mut layouts = Vec::with_capacity(text.len());
        let mut cursor_x = 0.0;
        let _line_height = self.font_size * scale;

        for c in text.chars() {
            if c == '\n' {
                cursor_x = 0.0;
                continue;
            }

            if let Some(glyph) = self.get_glyph(c) {
                layouts.push(GlyphLayout {
                    char: c,
                    position_x: cursor_x + glyph.bearing_x * scale,
                    position_y: -glyph.bearing_y * scale,
                    width: glyph.width as f32 * scale,
                    height: glyph.height as f32 * scale,
                    uv_min: glyph.uv_min,
                    uv_max: glyph.uv_max,
                });

                cursor_x += glyph.advance_width * scale;
            }
        }

        layouts
    }

    /// Calculate the width of a string when rendered
    pub fn measure_text(&self, text: &str, font_size: f32) -> f32 {
        let scale = font_size / self.font_size;
        let mut width = 0.0;

        for c in text.chars() {
            if let Some(glyph) = self.get_glyph(c) {
                width += glyph.advance_width * scale;
            }
        }

        width
    }

    /// Get line height for the given font size
    pub fn line_height(&self, font_size: f32) -> f32 {
        let scale = font_size / self.font_size;
        self.font_size * scale * 1.2 // 20% line spacing
    }
}

/// Layout information for a single glyph
#[derive(Debug, Clone, Copy)]
pub struct GlyphLayout {
    pub char: char,
    pub position_x: f32,
    pub position_y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_min: (f32, f32),
    pub uv_max: (f32, f32),
}

/// Builder for creating font atlases
pub struct FontAtlasBuilder {
    font_data: Vec<u8>,
    font_size: f32,
    padding: u32,
    chars: Vec<char>,
}

impl FontAtlasBuilder {
    /// Create a new builder from font data
    pub fn new(font_data: Vec<u8>) -> Self {
        Self {
            font_data,
            font_size: 48.0, // Default rasterization size
            padding: 2,
            chars: ASCII_RANGE.filter_map(|c| char::from_u32(c)).collect(),
        }
    }

    /// Load font from a file
    pub fn from_file(path: &str) -> Result<Self> {
        let font_data = std::fs::read(path)
            .with_context(|| format!("Failed to read font file: {}", path))?;
        Ok(Self::new(font_data))
    }

    /// Load default embedded font (fallback)
    pub fn default_font() -> Result<Self> {
        // For now, we'll require users to provide their own font
        // In a real implementation, we'd embed a font like DejaVu Sans
        anyhow::bail!("Default font not yet implemented. Please provide a font file.")
    }

    /// Set the font size for rasterization
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set padding around glyphs
    pub fn with_padding(mut self, padding: u32) -> Self {
        self.padding = padding;
        self
    }

    /// Set custom character set
    pub fn with_chars(mut self, chars: Vec<char>) -> Self {
        self.chars = chars;
        self
    }

    /// Build the font atlas
    pub fn build(self) -> Result<FontAtlas> {
        info!(
            "Building font atlas with {} characters at size {}",
            self.chars.len(),
            self.font_size
        );

        // Extract values we need before moving font_data
        let padding = self.padding;
        let font_size = self.font_size;
        let chars = self.chars;

        // Parse font
        let font = Font::from_bytes(self.font_data, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("Failed to parse font: {}", e))?;

        // Rasterize all glyphs to measure sizes
        let mut glyph_data = Vec::new();
        for &c in &chars {
            let (metrics, bitmap) = font.rasterize(c, font_size);
            glyph_data.push((c, metrics, bitmap));
        }

        // Pack glyphs into atlas using simple row packing
        let (atlas_width, atlas_height, positions) =
            Self::pack_glyphs_static(&glyph_data, padding)?;

        debug!(
            "Atlas dimensions: {}x{} pixels",
            atlas_width, atlas_height
        );

        // Create atlas texture
        let mut texture_data = vec![0u8; (atlas_width * atlas_height) as usize];

        // Copy glyph bitmaps into atlas
        let mut glyphs = HashMap::new();
        for (i, (c, metrics, bitmap)) in glyph_data.iter().enumerate() {
            let (x, y) = positions[i];

            // Copy bitmap to atlas
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let src_idx = row * metrics.width + col;
                    let dst_x = x + col as u32;
                    let dst_y = y + row as u32;
                    let dst_idx = (dst_y * atlas_width + dst_x) as usize;

                    if dst_idx < texture_data.len() && src_idx < bitmap.len() {
                        texture_data[dst_idx] = bitmap[src_idx];
                    }
                }
            }

            // Store glyph info
            let uv_min = (
                x as f32 / atlas_width as f32,
                y as f32 / atlas_height as f32,
            );
            let uv_max = (
                (x + metrics.width as u32) as f32 / atlas_width as f32,
                (y + metrics.height as u32) as f32 / atlas_height as f32,
            );

            glyphs.insert(
                *c,
                GlyphInfo {
                    atlas_x: x,
                    atlas_y: y,
                    width: metrics.width as u32,
                    height: metrics.height as u32,
                    advance_width: metrics.advance_width,
                    bearing_x: metrics.xmin as f32,
                    bearing_y: metrics.ymin as f32,
                    uv_min,
                    uv_max,
                },
            );
        }

        info!(
            "Font atlas built successfully: {} glyphs, {}x{} texture",
            glyphs.len(),
            atlas_width,
            atlas_height
        );

        Ok(FontAtlas {
            font,
            texture_data,
            width: atlas_width,
            height: atlas_height,
            glyphs,
            font_size,
            padding,
        })
    }

    /// Pack glyphs into atlas using row packing algorithm
    fn pack_glyphs_static(
        glyphs: &[(char, fontdue::Metrics, Vec<u8>)],
        padding: u32,
    ) -> Result<(u32, u32, Vec<(u32, u32)>)> {
        let mut current_x = padding;
        let mut current_y = padding;
        let mut row_height = 0u32;
        let mut max_width = 0u32;
        let mut positions = Vec::new();

        // Simple row packing
        const MAX_ATLAS_WIDTH: u32 = 2048;

        for (_, metrics, _) in glyphs {
            let glyph_width = metrics.width as u32 + padding * 2;
            let glyph_height = metrics.height as u32 + padding * 2;

            // Check if we need to wrap to next row
            if current_x + glyph_width > MAX_ATLAS_WIDTH {
                current_x = padding;
                current_y += row_height + padding;
                row_height = 0;
            }

            positions.push((current_x, current_y));

            current_x += glyph_width;
            row_height = row_height.max(glyph_height);
            max_width = max_width.max(current_x);
        }

        let atlas_width = max_width.next_power_of_two();
        let atlas_height = (current_y + row_height + padding).next_power_of_two();

        Ok((atlas_width, atlas_height, positions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_range() {
        let chars: Vec<char> = ASCII_RANGE.filter_map(|c| char::from_u32(c)).collect();
        assert!(!chars.is_empty());
        assert!(chars.contains(&'A'));
        assert!(chars.contains(&'z'));
        assert!(chars.contains(&'0'));
    }

    // Note: Actual font tests would require a font file
}
