//! 3D UI Components
//!
//! This module contains all the UI component types that can be rendered in 3D space.

pub mod text3d;
pub mod label;
pub mod billboard;

pub use text3d::Text3D;
pub use label::Label3D;
pub use billboard::Billboard;

use glam::Vec3;

/// Base trait for all 3D UI components
pub trait UIComponent {
    /// Get the world position of this component
    fn position(&self) -> Vec3;

    /// Set the world position of this component
    fn set_position(&mut self, position: Vec3);

    /// Check if this component is visible
    fn is_visible(&self) -> bool;

    /// Set visibility
    fn set_visible(&mut self, visible: bool);
}

/// Transform in 3D space
#[derive(Debug, Clone, Copy)]
pub struct Transform3D {
    pub position: Vec3,
    pub rotation: glam::Quat,
    pub scale: Vec3,
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform3D {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    pub fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_rotation(mut self, rotation: glam::Quat) -> Self {
        self.rotation = rotation;
        self
    }
}
