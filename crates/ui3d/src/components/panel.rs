//! 3D Panel Component - Background quad for UI elements

use super::{Transform3D, UIComponent};
use glam::Vec3;

/// Border style for panels
#[derive(Debug, Clone, Copy)]
pub struct PanelBorder {
    /// Border color
    pub color: [f32; 4],
    /// Border thickness (in world units)
    pub thickness: f32,
}

impl Default for PanelBorder {
    fn default() -> Self {
        Self {
            color: [0.2, 0.2, 0.2, 1.0], // Dark gray
            thickness: 0.05,
        }
    }
}

/// 3D Panel - A background quad for UI elements
#[derive(Debug, Clone)]
pub struct Panel3D {
    /// Panel transform (position, rotation, scale)
    pub transform: Transform3D,

    /// Panel size (width, height)
    pub size: (f32, f32),

    /// Background color
    pub color: [f32; 4],

    /// Optional border
    pub border: Option<PanelBorder>,

    /// Whether the panel should billboard (face camera)
    pub billboard: bool,

    /// Whether the panel is visible
    pub visible: bool,

    /// Corner radius for rounded corners (0 = sharp corners)
    pub corner_radius: f32,

    /// Padding inside the panel (affects child layout)
    pub padding: f32,
}

impl Default for Panel3D {
    fn default() -> Self {
        Self {
            transform: Transform3D::default(),
            size: (2.0, 1.0),
            color: [0.1, 0.1, 0.1, 0.8], // Dark semi-transparent
            border: Some(PanelBorder::default()),
            billboard: true,
            visible: true,
            corner_radius: 0.05,
            padding: 0.1,
        }
    }
}

impl Panel3D {
    /// Create a new 3D panel
    pub fn new(position: Vec3, width: f32, height: f32) -> Self {
        Self {
            transform: Transform3D::new(position),
            size: (width, height),
            ..Default::default()
        }
    }

    /// Builder: Set background color
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    /// Builder: Set border
    pub fn with_border(mut self, border: Option<PanelBorder>) -> Self {
        self.border = border;
        self
    }

    /// Builder: Set billboard mode
    pub fn with_billboard(mut self, billboard: bool) -> Self {
        self.billboard = billboard;
        self
    }

    /// Builder: Set corner radius
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Builder: Set padding
    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Get the content area (area inside padding)
    pub fn content_area(&self) -> (Vec3, (f32, f32)) {
        let content_width = self.size.0 - 2.0 * self.padding;
        let content_height = self.size.1 - 2.0 * self.padding;
        (self.transform.position, (content_width, content_height))
    }

    /// Get panel bounds (for raycasting)
    pub fn bounds(&self) -> (Vec3, (f32, f32)) {
        (self.transform.position, self.size)
    }

    /// Check if a 2D point (in panel-local space) is inside the panel
    /// u, v are in range [0, 1] representing position on the panel
    pub fn contains_uv(&self, u: f32, v: f32) -> bool {
        u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0
    }

    /// Generate vertices for the panel quad
    /// Returns vertices in a format suitable for GPU rendering
    /// Format: [bottom-left, bottom-right, top-right, top-left]
    pub fn generate_vertices(&self) -> [PanelVertex; 4] {
        let half_width = self.size.0 * 0.5;
        let half_height = self.size.1 * 0.5;
        let center = self.transform.position;

        // TODO: Apply rotation from transform
        // For now, assume no rotation (or rotation handled by billboarding)

        [
            // Bottom-left
            PanelVertex {
                position: [
                    center.x - half_width,
                    center.y - half_height,
                    center.z,
                ],
                uv: [0.0, 0.0],
                color: self.color,
            },
            // Bottom-right
            PanelVertex {
                position: [
                    center.x + half_width,
                    center.y - half_height,
                    center.z,
                ],
                uv: [1.0, 0.0],
                color: self.color,
            },
            // Top-right
            PanelVertex {
                position: [
                    center.x + half_width,
                    center.y + half_height,
                    center.z,
                ],
                uv: [1.0, 1.0],
                color: self.color,
            },
            // Top-left
            PanelVertex {
                position: [
                    center.x - half_width,
                    center.y + half_height,
                    center.z,
                ],
                uv: [0.0, 1.0],
                color: self.color,
            },
        ]
    }
}

impl UIComponent for Panel3D {
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

/// Vertex data for panel rendering
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PanelVertex {
    /// Vertex position in world space
    pub position: [f32; 3],
    /// UV coordinates (0-1)
    pub uv: [f32; 2],
    /// Vertex color
    pub color: [f32; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_creation() {
        let panel = Panel3D::new(Vec3::ZERO, 2.0, 1.0);
        assert_eq!(panel.size, (2.0, 1.0));
        assert!(panel.visible);
        assert!(panel.billboard);
    }

    #[test]
    fn test_panel_content_area() {
        let panel = Panel3D::new(Vec3::ZERO, 2.0, 1.0).with_padding(0.1);
        let (_, (width, height)) = panel.content_area();
        assert!((width - 1.8).abs() < 0.001); // 2.0 - 2*0.1
        assert!((height - 0.8).abs() < 0.001); // 1.0 - 2*0.1
    }

    #[test]
    fn test_panel_contains_uv() {
        let panel = Panel3D::new(Vec3::ZERO, 2.0, 1.0);
        assert!(panel.contains_uv(0.5, 0.5)); // Center
        assert!(panel.contains_uv(0.0, 0.0)); // Corner
        assert!(panel.contains_uv(1.0, 1.0)); // Opposite corner
        assert!(!panel.contains_uv(1.1, 0.5)); // Outside
        assert!(!panel.contains_uv(-0.1, 0.5)); // Outside
    }

    #[test]
    fn test_panel_builder() {
        let panel = Panel3D::new(Vec3::ZERO, 2.0, 1.0)
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_corner_radius(0.1)
            .with_padding(0.2)
            .with_billboard(false);

        assert_eq!(panel.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(panel.corner_radius, 0.1);
        assert_eq!(panel.padding, 0.2);
        assert!(!panel.billboard);
    }

    #[test]
    fn test_panel_vertices() {
        let panel = Panel3D::new(Vec3::ZERO, 2.0, 1.0);
        let vertices = panel.generate_vertices();

        // Check we have 4 vertices
        assert_eq!(vertices.len(), 4);

        // Check UV coordinates are correct
        assert_eq!(vertices[0].uv, [0.0, 0.0]); // Bottom-left
        assert_eq!(vertices[1].uv, [1.0, 0.0]); // Bottom-right
        assert_eq!(vertices[2].uv, [1.0, 1.0]); // Top-right
        assert_eq!(vertices[3].uv, [0.0, 1.0]); // Top-left
    }
}
