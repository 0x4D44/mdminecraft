//! 3D Text Component

use super::{Transform3D, UIComponent};
use glam::Vec3;

/// Text rendering in 3D world space
#[derive(Debug, Clone)]
pub struct Text3D {
    /// World position of the text
    pub transform: Transform3D,

    /// Text content
    pub text: String,

    /// Font size in world units
    pub font_size: f32,

    /// Text color (RGBA)
    pub color: [f32; 4],

    /// Whether the text should billboard (face camera)
    pub billboard: bool,

    /// Whether the text is visible
    pub visible: bool,

    /// Text alignment
    pub alignment: TextAlignment,

    /// Maximum width before wrapping (0 = no wrap)
    pub max_width: f32,

    /// Line spacing multiplier
    pub line_spacing: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl Default for Text3D {
    fn default() -> Self {
        Self {
            transform: Transform3D::default(),
            text: String::new(),
            font_size: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            billboard: true,
            visible: true,
            alignment: TextAlignment::Center,
            max_width: 0.0,
            line_spacing: 1.2,
        }
    }
}

impl Text3D {
    /// Create a new Text3D component
    pub fn new(position: Vec3, text: impl Into<String>) -> Self {
        Self {
            transform: Transform3D::new(position),
            text: text.into(),
            ..Default::default()
        }
    }

    /// Builder: Set font size
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Builder: Set color
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    /// Builder: Set billboard mode
    pub fn with_billboard(mut self, billboard: bool) -> Self {
        self.billboard = billboard;
        self
    }

    /// Builder: Set alignment
    pub fn with_alignment(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Builder: Set max width for wrapping
    pub fn with_max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    /// Update the text content
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Get a reference to the text
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl UIComponent for Text3D {
    fn position(&self) -> Vec3 {
        self.transform.position
    }

    fn set_position(&mut self, position: Vec3) {
        self.transform.position = position;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text3d_creation() {
        let text = Text3D::new(Vec3::new(0.0, 10.0, 0.0), "Hello, World!");
        assert_eq!(text.text(), "Hello, World!");
        assert_eq!(text.position(), Vec3::new(0.0, 10.0, 0.0));
        assert!(text.is_visible());
    }

    #[test]
    fn test_text3d_builder() {
        let text = Text3D::new(Vec3::ZERO, "Test")
            .with_font_size(2.0)
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_billboard(false)
            .with_alignment(TextAlignment::Left);

        assert_eq!(text.font_size, 2.0);
        assert_eq!(text.color, [1.0, 0.0, 0.0, 1.0]);
        assert!(!text.billboard);
        assert_eq!(text.alignment, TextAlignment::Left);
    }

    #[test]
    fn test_text3d_update() {
        let mut text = Text3D::new(Vec3::ZERO, "Initial");
        text.set_text("Updated");
        assert_eq!(text.text(), "Updated");

        text.set_position(Vec3::new(5.0, 5.0, 5.0));
        assert_eq!(text.position(), Vec3::new(5.0, 5.0, 5.0));
    }
}
