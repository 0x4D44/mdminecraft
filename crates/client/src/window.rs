//! Window management using winit.
//!
//! This module provides window creation and event handling for the game client.

use anyhow::{Context, Result};
use std::sync::Arc;
use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

/// Configuration for window creation.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window title.
    pub title: String,
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
    /// Whether to start in fullscreen mode.
    pub fullscreen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "MDMinecraft".to_string(),
            width: 1280,
            height: 720,
            fullscreen: false,
        }
    }
}

/// Window manager that handles window creation and state.
pub struct WindowManager {
    window: Arc<Window>,
    cursor_grabbed: bool,
}

impl WindowManager {
    /// Create a new window with the given configuration.
    ///
    /// # Arguments
    /// * `event_loop` - The event loop to attach the window to
    /// * `config` - Window configuration
    ///
    /// # Returns
    /// A new window manager instance, or an error if window creation fails.
    pub fn new(event_loop: &EventLoop<()>, config: WindowConfig) -> Result<Self> {
        let window = WindowBuilder::new()
            .with_title(&config.title)
            .with_inner_size(PhysicalSize::new(config.width, config.height))
            .with_fullscreen(if config.fullscreen {
                Some(winit::window::Fullscreen::Borderless(None))
            } else {
                None
            })
            .build(event_loop)
            .context("Failed to create window")?;

        Ok(Self {
            window: Arc::new(window),
            cursor_grabbed: false,
        })
    }

    /// Get a reference to the underlying window (as Arc for sharing).
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    /// Get window dimensions.
    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width, size.height)
    }

    /// Check if cursor is currently grabbed.
    pub fn is_cursor_grabbed(&self) -> bool {
        self.cursor_grabbed
    }

    /// Grab the cursor for mouse look.
    ///
    /// This hides the cursor and locks it to the window.
    pub fn grab_cursor(&mut self) -> Result<()> {
        use winit::window::CursorGrabMode;

        self.window
            .set_cursor_grab(CursorGrabMode::Confined)
            .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Locked))
            .context("Failed to grab cursor")?;

        self.window.set_cursor_visible(false);
        self.cursor_grabbed = true;

        Ok(())
    }

    /// Release the cursor.
    ///
    /// This shows the cursor and unlocks it from the window.
    pub fn release_cursor(&mut self) -> Result<()> {
        use winit::window::CursorGrabMode;

        self.window
            .set_cursor_grab(CursorGrabMode::None)
            .context("Failed to release cursor")?;

        self.window.set_cursor_visible(true);
        self.cursor_grabbed = false;

        Ok(())
    }

    /// Request a redraw of the window.
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_config_default() {
        let config = WindowConfig::default();
        assert_eq!(config.title, "MDMinecraft");
        assert_eq!(config.width, 1280);
        assert_eq!(config.height, 720);
        assert!(!config.fullscreen);
    }

    #[test]
    fn window_config_custom() {
        let config = WindowConfig {
            title: "Test Window".to_string(),
            width: 800,
            height: 600,
            fullscreen: true,
        };
        assert_eq!(config.title, "Test Window");
        assert_eq!(config.width, 800);
        assert_eq!(config.height, 600);
        assert!(config.fullscreen);
    }

    // Note: Can't easily test window creation without a display server,
    // so we rely on integration tests for that.
}
