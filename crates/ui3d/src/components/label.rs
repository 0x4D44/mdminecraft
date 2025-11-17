//! 3D Label Component - Simplified text for nameplates and tooltips

use super::text3d::{Text3D, TextAlignment};
use super::UIComponent;
use glam::Vec3;

/// A simplified text component for floating labels, nameplates, and tooltips
///
/// Label3D is a convenience wrapper around Text3D with defaults optimized
/// for common use cases like player nameplates and item tooltips.
#[derive(Debug, Clone)]
pub struct Label3D {
    /// Underlying text component
    text: Text3D,

    /// Distance at which label starts to fade
    pub fade_start_distance: f32,

    /// Distance at which label is fully transparent
    pub fade_end_distance: f32,

    /// Whether to show background panel
    pub show_background: bool,

    /// Background color (RGBA)
    pub background_color: [f32; 4],

    /// Padding around text (in world units)
    pub padding: f32,
}

impl Default for Label3D {
    fn default() -> Self {
        Self {
            text: Text3D {
                billboard: true,
                alignment: TextAlignment::Center,
                ..Default::default()
            },
            fade_start_distance: 20.0,
            fade_end_distance: 50.0,
            show_background: true,
            background_color: [0.0, 0.0, 0.0, 0.7],
            padding: 0.1,
        }
    }
}

impl Label3D {
    /// Create a new label at the given position
    pub fn new(position: Vec3, text: impl Into<String>) -> Self {
        Self {
            text: Text3D::new(position, text)
                .with_billboard(true)
                .with_alignment(TextAlignment::Center),
            ..Default::default()
        }
    }

    /// Builder: Set color
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.text.color = color;
        self
    }

    /// Builder: Set font size
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.text.font_size = size;
        self
    }

    /// Builder: Set fade distances
    pub fn with_fade_distance(mut self, start: f32, end: f32) -> Self {
        self.fade_start_distance = start;
        self.fade_end_distance = end;
        self
    }

    /// Builder: Set background visibility
    pub fn with_background(mut self, show: bool) -> Self {
        self.show_background = show;
        self
    }

    /// Builder: Set background color
    pub fn with_background_color(mut self, color: [f32; 4]) -> Self {
        self.background_color = color;
        self
    }

    /// Get the underlying text component
    pub fn text(&self) -> &Text3D {
        &self.text
    }

    /// Get a mutable reference to the underlying text component
    pub fn text_mut(&mut self) -> &mut Text3D {
        &mut self.text
    }

    /// Calculate alpha based on distance from camera
    pub fn calculate_alpha(&self, camera_position: Vec3) -> f32 {
        let distance = self.position().distance(camera_position);

        if distance <= self.fade_start_distance {
            1.0
        } else if distance >= self.fade_end_distance {
            0.0
        } else {
            let fade_range = self.fade_end_distance - self.fade_start_distance;
            let fade_progress = (distance - self.fade_start_distance) / fade_range;
            1.0 - fade_progress
        }
    }
}

impl UIComponent for Label3D {
    fn position(&self) -> Vec3 {
        self.text.position()
    }

    fn set_position(&mut self, position: Vec3) {
        self.text.set_position(position);
    }

    fn is_visible(&self) -> bool {
        self.text.is_visible()
    }

    fn set_visible(&mut self, visible: bool) {
        self.text.set_visible(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_creation() {
        let label = Label3D::new(Vec3::new(0.0, 10.0, 0.0), "Player Name");
        assert_eq!(label.text().text(), "Player Name");
        assert!(label.show_background);
        assert!(label.text().billboard);
    }

    #[test]
    fn test_label_fade() {
        let label = Label3D::new(Vec3::ZERO, "Test")
            .with_fade_distance(10.0, 20.0);

        let camera_close = Vec3::new(5.0, 0.0, 0.0);
        let camera_mid = Vec3::new(15.0, 0.0, 0.0);
        let camera_far = Vec3::new(25.0, 0.0, 0.0);

        assert_eq!(label.calculate_alpha(camera_close), 1.0);
        assert_eq!(label.calculate_alpha(camera_mid), 0.5);
        assert_eq!(label.calculate_alpha(camera_far), 0.0);
    }

    #[test]
    fn test_label_builder() {
        let label = Label3D::new(Vec3::ZERO, "Test")
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_font_size(2.0)
            .with_background(false);

        assert_eq!(label.text().color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(label.text().font_size, 2.0);
        assert!(!label.show_background);
    }
}
