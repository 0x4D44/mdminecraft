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
//! ```rust,no_run
//! use mdminecraft_ui3d::{UIManager, Label3D};
//! use glam::Vec3;
//!
//! let mut ui_manager = UIManager::new();
//!
//! ui_manager.add_label(Label3D {
//!     position: Vec3::new(0.0, 75.0, 0.0),
//!     text: "Welcome!".to_string(),
//!     color: [1.0, 1.0, 0.0, 1.0],
//! });
//!
//! // In your render loop:
//! ui_manager.update(dt, camera, &input);
//! ui_manager.render(&mut encoder, &frame_view, camera);
//! ```

pub mod components;
pub mod render;
pub mod interaction;
pub mod layout;
pub mod manager;

// Re-export commonly used types
pub use components::{
    Button3D, ButtonColors, ButtonState, Label3D, Panel3D, PanelBorder, PanelVertex, Text3D,
    TextAlignment,
};
pub use render::{FontAtlas, FontAtlasBuilder, TextRenderer, TextVertex};
pub use manager::{UI3DManager, UIElementHandle};
pub use interaction::{raycast_billboard_quad, screen_to_ray, UIRaycastHit, UIAABB};

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
