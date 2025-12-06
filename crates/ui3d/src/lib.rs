//! 3D UI System for mdminecraft
//!
//! This crate provides a complete 3D user interface system built on top of wgpu.
//! UI elements are rendered in world space and can be interacted with using raycasting.
//!
//! # Features
//!
//! - **Text Rendering**: SDF-based text rendering for crisp text at any scale
//! - **Billboards**: Camera-facing quads for sprites and UI elements
//! - **Interactive Components**: Buttons, panels, labels with click/hover support
//! - **3D Layout**: Constraint-based positioning in world space
//! - **Effects**: Shader-based effects (glow, fade, etc.)
//!
//! # Example
//!
//! ```rust,ignore
//! use mdminecraft_ui3d::Label3D;
//! use glam::Vec3;
//!
//! // Create a label at position with text and color
//! let label = Label3D::new(Vec3::new(0.0, 75.0, 0.0), "Welcome!")
//!     .with_color([1.0, 1.0, 0.0, 1.0]);
//! ```

pub mod components;
pub mod interaction;
pub mod layout;
pub mod render;

// Re-export commonly used types
pub use components::{Label3D, Text3D};
pub use render::{FontAtlas, TextRenderer};

use anyhow::Result;

/// Version of the UI3D crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the 3D UI system with default settings
pub fn init() -> Result<()> {
    tracing::info!("Initializing mdminecraft-ui3d v{}", VERSION);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }
}
