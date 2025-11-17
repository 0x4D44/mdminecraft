//! Interactive 3D Button Component

use super::{Text3D, Transform3D, UIComponent};
use glam::Vec3;

/// Button state for visual feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Normal state (not interacted with)
    Normal,
    /// Mouse is hovering over button
    Hover,
    /// Button is being pressed
    Pressed,
    /// Button is disabled
    Disabled,
}

/// Color scheme for button states
#[derive(Debug, Clone, Copy)]
pub struct ButtonColors {
    /// Normal state color
    pub normal: [f32; 4],
    /// Hover state color
    pub hover: [f32; 4],
    /// Pressed state color
    pub pressed: [f32; 4],
    /// Disabled state color
    pub disabled: [f32; 4],
}

impl Default for ButtonColors {
    fn default() -> Self {
        Self {
            normal: [0.8, 0.8, 0.8, 1.0],       // Light gray
            hover: [1.0, 1.0, 0.6, 1.0],        // Yellow
            pressed: [0.6, 0.6, 1.0, 1.0],      // Blue
            disabled: [0.5, 0.5, 0.5, 0.5],     // Dark gray, semi-transparent
        }
    }
}

/// Interactive 3D button with text label
#[derive(Debug, Clone)]
pub struct Button3D {
    /// Button transform (position, rotation, scale)
    pub transform: Transform3D,

    /// Button label text
    pub text: String,

    /// Current button state
    pub state: ButtonState,

    /// Color scheme for different states
    pub colors: ButtonColors,

    /// Button size (width, height)
    pub size: (f32, f32),

    /// Font size for button text
    pub font_size: f32,

    /// Whether the button should billboard (face camera)
    pub billboard: bool,

    /// Whether the button is visible
    pub visible: bool,

    /// Optional callback ID (for application-level handling)
    pub callback_id: Option<u32>,
}

impl Default for Button3D {
    fn default() -> Self {
        Self {
            transform: Transform3D::default(),
            text: String::new(),
            state: ButtonState::Normal,
            colors: ButtonColors::default(),
            size: (2.0, 0.5), // Default button size
            font_size: 0.3,
            billboard: true,
            visible: true,
            callback_id: None,
        }
    }
}

impl Button3D {
    /// Create a new 3D button
    pub fn new(position: Vec3, text: impl Into<String>) -> Self {
        Self {
            transform: Transform3D::new(position),
            text: text.into(),
            ..Default::default()
        }
    }

    /// Builder: Set button size
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.size = (width, height);
        self
    }

    /// Builder: Set font size
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Builder: Set color scheme
    pub fn with_colors(mut self, colors: ButtonColors) -> Self {
        self.colors = colors;
        self
    }

    /// Builder: Set callback ID
    pub fn with_callback(mut self, id: u32) -> Self {
        self.callback_id = Some(id);
        self
    }

    /// Builder: Set billboard mode
    pub fn with_billboard(mut self, billboard: bool) -> Self {
        self.billboard = billboard;
        self
    }

    /// Update button state
    pub fn set_state(&mut self, state: ButtonState) {
        self.state = state;
    }

    /// Get current color based on state
    pub fn current_color(&self) -> [f32; 4] {
        match self.state {
            ButtonState::Normal => self.colors.normal,
            ButtonState::Hover => self.colors.hover,
            ButtonState::Pressed => self.colors.pressed,
            ButtonState::Disabled => self.colors.disabled,
        }
    }

    /// Check if button is interactable
    pub fn is_interactable(&self) -> bool {
        self.visible && self.state != ButtonState::Disabled
    }

    /// Convert to Text3D for rendering
    pub fn to_text3d(&self) -> Text3D {
        Text3D::new(self.transform.position, &self.text)
            .with_font_size(self.font_size)
            .with_color(self.current_color())
            .with_billboard(self.billboard)
    }

    /// Get button bounds (for raycasting)
    /// Returns (center, size) tuple
    pub fn bounds(&self) -> (Vec3, (f32, f32)) {
        (self.transform.position, self.size)
    }
}

impl UIComponent for Button3D {
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
    fn test_button_creation() {
        let button = Button3D::new(Vec3::ZERO, "Click Me!");
        assert_eq!(button.text, "Click Me!");
        assert_eq!(button.state, ButtonState::Normal);
        assert!(button.is_interactable());
    }

    #[test]
    fn test_button_states() {
        let mut button = Button3D::new(Vec3::ZERO, "Test");

        button.set_state(ButtonState::Hover);
        assert_eq!(button.state, ButtonState::Hover);
        assert!(button.is_interactable());

        button.set_state(ButtonState::Disabled);
        assert_eq!(button.state, ButtonState::Disabled);
        assert!(!button.is_interactable());
    }

    #[test]
    fn test_button_builder() {
        let button = Button3D::new(Vec3::ZERO, "Test")
            .with_size(3.0, 1.0)
            .with_font_size(0.5)
            .with_callback(42)
            .with_billboard(false);

        assert_eq!(button.size, (3.0, 1.0));
        assert_eq!(button.font_size, 0.5);
        assert_eq!(button.callback_id, Some(42));
        assert!(!button.billboard);
    }

    #[test]
    fn test_button_colors() {
        let button = Button3D::new(Vec3::ZERO, "Test");

        // Default colors should be different for each state
        let normal = button.current_color();

        let mut hover_button = button.clone();
        hover_button.set_state(ButtonState::Hover);
        let hover = hover_button.current_color();

        assert_ne!(normal, hover);
    }
}
