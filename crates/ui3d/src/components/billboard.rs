//! Billboard Component - Camera-facing quads for sprites and UI elements

use super::{Transform3D, UIComponent};
use glam::Vec3;

/// A billboard is a quad that always faces the camera
///
/// Billboards are useful for:
/// - Particles
/// - Sprite-based UI elements
/// - Icons and markers in 3D space
#[derive(Debug, Clone)]
pub struct Billboard {
    /// World transform
    pub transform: Transform3D,

    /// Size of the billboard (width, height) in world units
    pub size: (f32, f32),

    /// Color tint (RGBA)
    pub color: [f32; 4],

    /// Texture coordinates in atlas (u_min, v_min, u_max, v_max)
    /// If None, renders solid color
    pub texture_coords: Option<(f32, f32, f32, f32)>,

    /// Billboard orientation mode
    pub orientation: BillboardOrientation,

    /// Whether the billboard is visible
    pub visible: bool,

    /// Depth testing mode
    pub depth_mode: DepthMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillboardOrientation {
    /// Full billboard - faces camera completely
    Full,
    /// Y-axis aligned - rotates around Y axis only (vertical billboards)
    YAxis,
    /// Fixed orientation - no billboarding
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthMode {
    /// Normal depth testing (billboard can be occluded)
    Normal,
    /// Always render on top
    AlwaysOnTop,
}

impl Default for Billboard {
    fn default() -> Self {
        Self {
            transform: Transform3D::default(),
            size: (1.0, 1.0),
            color: [1.0, 1.0, 1.0, 1.0],
            texture_coords: None,
            orientation: BillboardOrientation::Full,
            visible: true,
            depth_mode: DepthMode::Normal,
        }
    }
}

impl Billboard {
    /// Create a new billboard at the given position
    pub fn new(position: Vec3) -> Self {
        Self {
            transform: Transform3D::new(position),
            ..Default::default()
        }
    }

    /// Builder: Set size
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.size = (width, height);
        self
    }

    /// Builder: Set color
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    /// Builder: Set texture coordinates in atlas
    pub fn with_texture(mut self, u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> Self {
        self.texture_coords = Some((u_min, v_min, u_max, v_max));
        self
    }

    /// Builder: Set orientation mode
    pub fn with_orientation(mut self, orientation: BillboardOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Builder: Set depth mode
    pub fn with_depth_mode(mut self, mode: DepthMode) -> Self {
        self.depth_mode = mode;
        self
    }

    /// Get the width of the billboard
    pub fn width(&self) -> f32 {
        self.size.0 * self.transform.scale.x
    }

    /// Get the height of the billboard
    pub fn height(&self) -> f32 {
        self.size.1 * self.transform.scale.y
    }

    /// Calculate the billboard's orientation matrix relative to camera
    pub fn calculate_orientation(&self, camera_position: Vec3, camera_up: Vec3) -> glam::Mat4 {
        match self.orientation {
            BillboardOrientation::Full => {
                self.calculate_full_billboard(camera_position, camera_up)
            }
            BillboardOrientation::YAxis => {
                self.calculate_y_axis_billboard(camera_position)
            }
            BillboardOrientation::Fixed => {
                glam::Mat4::from_scale_rotation_translation(
                    self.transform.scale,
                    self.transform.rotation,
                    self.transform.position,
                )
            }
        }
    }

    fn calculate_full_billboard(&self, camera_position: Vec3, camera_up: Vec3) -> glam::Mat4 {
        let to_camera = (camera_position - self.transform.position).normalize();
        let right = camera_up.cross(to_camera).normalize();
        let up = to_camera.cross(right);

        let rotation = glam::Mat3::from_cols(right, up, to_camera);
        let rotation_quat = glam::Quat::from_mat3(&rotation);

        glam::Mat4::from_scale_rotation_translation(
            self.transform.scale,
            rotation_quat,
            self.transform.position,
        )
    }

    fn calculate_y_axis_billboard(&self, camera_position: Vec3) -> glam::Mat4 {
        let mut to_camera = camera_position - self.transform.position;
        to_camera.y = 0.0; // Project to XZ plane
        to_camera = to_camera.normalize();

        let right = Vec3::Y.cross(to_camera).normalize();
        let rotation = glam::Mat3::from_cols(right, Vec3::Y, to_camera);
        let rotation_quat = glam::Quat::from_mat3(&rotation);

        glam::Mat4::from_scale_rotation_translation(
            self.transform.scale,
            rotation_quat,
            self.transform.position,
        )
    }
}

impl UIComponent for Billboard {
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
    fn test_billboard_creation() {
        let billboard = Billboard::new(Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(billboard.position(), Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(billboard.size, (1.0, 1.0));
        assert!(billboard.is_visible());
    }

    #[test]
    fn test_billboard_builder() {
        let billboard = Billboard::new(Vec3::ZERO)
            .with_size(2.0, 3.0)
            .with_color([1.0, 0.0, 0.0, 0.5])
            .with_orientation(BillboardOrientation::YAxis)
            .with_depth_mode(DepthMode::AlwaysOnTop);

        assert_eq!(billboard.size, (2.0, 3.0));
        assert_eq!(billboard.color, [1.0, 0.0, 0.0, 0.5]);
        assert_eq!(billboard.orientation, BillboardOrientation::YAxis);
        assert_eq!(billboard.depth_mode, DepthMode::AlwaysOnTop);
    }

    #[test]
    fn test_billboard_dimensions() {
        let billboard = Billboard::new(Vec3::ZERO)
            .with_size(2.0, 3.0);

        assert_eq!(billboard.width(), 2.0);
        assert_eq!(billboard.height(), 3.0);
    }
}
