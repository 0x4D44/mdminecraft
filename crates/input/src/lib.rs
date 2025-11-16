#![warn(missing_docs)]
//! Input handling for keyboard, mouse, and gamepad.

use std::collections::HashSet;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Input state tracking for a single frame.
#[derive(Debug, Default)]
pub struct InputState {
    /// Keys currently pressed.
    keys_pressed: HashSet<KeyCode>,
    /// Keys pressed this frame (edge-triggered).
    keys_just_pressed: HashSet<KeyCode>,
    /// Keys released this frame (edge-triggered).
    keys_just_released: HashSet<KeyCode>,

    /// Mouse buttons currently pressed.
    mouse_buttons: HashSet<MouseButton>,
    /// Mouse buttons pressed this frame.
    mouse_just_pressed: HashSet<MouseButton>,
    /// Mouse buttons released this frame.
    mouse_just_released: HashSet<MouseButton>,

    /// Mouse delta since last frame (for camera rotation).
    pub mouse_delta: (f64, f64),

    /// Mouse wheel delta.
    pub mouse_wheel_delta: f32,

    /// Whether the cursor is locked (for first-person camera).
    pub cursor_locked: bool,
}

impl InputState {
    /// Create a new input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a window event to update input state.
    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(keycode),
                        state,
                        ..
                    },
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        if self.keys_pressed.insert(*keycode) {
                            self.keys_just_pressed.insert(*keycode);
                        }
                    }
                    ElementState::Released => {
                        self.keys_pressed.remove(keycode);
                        self.keys_just_released.insert(*keycode);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    ElementState::Pressed => {
                        if self.mouse_buttons.insert(*button) {
                            self.mouse_just_pressed.insert(*button);
                        }
                    }
                    ElementState::Released => {
                        self.mouse_buttons.remove(button);
                        self.mouse_just_released.insert(*button);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                self.mouse_wheel_delta += match delta {
                    MouseScrollDelta::LineDelta(_x, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };
            }
            _ => {}
        }
    }

    /// Handle device event (for mouse movement).
    pub fn handle_device_event(&mut self, event: &winit::event::DeviceEvent) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            self.mouse_delta.0 += delta.0;
            self.mouse_delta.1 += delta.1;
        }
    }

    /// Reset per-frame state (call at the start of each frame).
    pub fn begin_frame(&mut self) {
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.mouse_just_pressed.clear();
        self.mouse_just_released.clear();
        self.mouse_delta = (0.0, 0.0);
        self.mouse_wheel_delta = 0.0;
    }

    /// Check if a key is currently pressed.
    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Check if a key was just pressed this frame.
    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    /// Check if a key was just released this frame.
    pub fn key_just_released(&self, key: KeyCode) -> bool {
        self.keys_just_released.contains(&key)
    }

    /// Check if a mouse button is currently pressed.
    pub fn mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    /// Check if a mouse button was just pressed this frame.
    pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_just_pressed.contains(&button)
    }

    /// Get movement input as a normalized direction vector.
    /// Returns (forward, right) where each component is -1, 0, or 1.
    pub fn movement_input(&self) -> (f32, f32) {
        let mut forward = 0.0;
        let mut right = 0.0;

        if self.key_pressed(KeyCode::KeyW) {
            forward += 1.0;
        }
        if self.key_pressed(KeyCode::KeyS) {
            forward -= 1.0;
        }
        if self.key_pressed(KeyCode::KeyD) {
            right += 1.0;
        }
        if self.key_pressed(KeyCode::KeyA) {
            right -= 1.0;
        }

        (forward, right)
    }

    /// Get vertical movement input (up/down).
    pub fn vertical_input(&self) -> f32 {
        let mut vertical = 0.0;

        if self.key_pressed(KeyCode::Space) {
            vertical += 1.0;
        }
        if self.key_pressed(KeyCode::ShiftLeft) || self.key_pressed(KeyCode::ShiftRight) {
            vertical -= 1.0;
        }

        vertical
    }

    /// Toggle cursor lock (for first-person camera).
    pub fn toggle_cursor_lock(&mut self) {
        self.cursor_locked = !self.cursor_locked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_state_creation() {
        let input = InputState::new();
        assert_eq!(input.mouse_delta, (0.0, 0.0));
        assert_eq!(input.mouse_wheel_delta, 0.0);
        assert!(!input.cursor_locked);
    }

    #[test]
    fn movement_input_default() {
        let input = InputState::new();
        let (forward, right) = input.movement_input();
        assert_eq!(forward, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn vertical_input_default() {
        let input = InputState::new();
        assert_eq!(input.vertical_input(), 0.0);
    }

    #[test]
    fn cursor_lock_toggle() {
        let mut input = InputState::new();
        assert!(!input.cursor_locked);

        input.toggle_cursor_lock();
        assert!(input.cursor_locked);

        input.toggle_cursor_lock();
        assert!(!input.cursor_locked);
    }

    #[test]
    fn begin_frame_resets_deltas() {
        let mut input = InputState::new();
        input.mouse_delta = (10.0, 20.0);
        input.mouse_wheel_delta = 5.0;

        input.begin_frame();

        assert_eq!(input.mouse_delta, (0.0, 0.0));
        assert_eq!(input.mouse_wheel_delta, 0.0);
    }
}
