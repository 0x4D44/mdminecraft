//! Input event system for deterministic replay.
//!
//! This module defines input events and state tracking for player actions.
//! All events are serializable for replay logs.

use serde::{Deserialize, Serialize};

/// Input events generated from keyboard and mouse.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// Movement forward (relative to camera facing).
    MoveForward(bool),
    /// Movement backward.
    MoveBack(bool),
    /// Strafe left.
    MoveLeft(bool),
    /// Strafe right.
    MoveRight(bool),
    /// Jump action.
    Jump(bool),
    /// Crouch/sneak action.
    Crouch(bool),
    /// Sprint action.
    Sprint(bool),
    /// Camera look delta from mouse motion.
    LookDelta {
        /// Yaw delta (horizontal rotation).
        yaw: f32,
        /// Pitch delta (vertical rotation).
        pitch: f32,
    },
}

/// Tracks current state of all input keys.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Forward movement state.
    pub forward: bool,
    /// Backward movement state.
    pub back: bool,
    /// Left strafe state.
    pub left: bool,
    /// Right strafe state.
    pub right: bool,
    /// Jump state.
    pub jump: bool,
    /// Crouch state.
    pub crouch: bool,
    /// Sprint state.
    pub sprint: bool,
}

impl InputState {
    /// Create a new empty input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply an input event to update state.
    pub fn apply_event(&mut self, event: InputEvent) {
        match event {
            InputEvent::MoveForward(pressed) => self.forward = pressed,
            InputEvent::MoveBack(pressed) => self.back = pressed,
            InputEvent::MoveLeft(pressed) => self.left = pressed,
            InputEvent::MoveRight(pressed) => self.right = pressed,
            InputEvent::Jump(pressed) => self.jump = pressed,
            InputEvent::Crouch(pressed) => self.crouch = pressed,
            InputEvent::Sprint(pressed) => self.sprint = pressed,
            InputEvent::LookDelta { .. } => {} // Look delta doesn't update state
        }
    }

    /// Get movement direction as normalized vector.
    ///
    /// Returns (forward, right) components in range [-1, 1].
    pub fn movement_direction(&self) -> (f32, f32) {
        let mut forward: f32 = 0.0;
        let mut right: f32 = 0.0;

        if self.forward {
            forward += 1.0;
        }
        if self.back {
            forward -= 1.0;
        }
        if self.right {
            right += 1.0;
        }
        if self.left {
            right -= 1.0;
        }

        // Normalize diagonal movement
        let length = (forward * forward + right * right).sqrt();
        if length > 0.0 {
            forward /= length;
            right /= length;
        }

        (forward, right)
    }
}

/// Key codes for input bindings (simplified for now).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    /// W key.
    W,
    /// A key.
    A,
    /// S key.
    S,
    /// D key.
    D,
    /// Space bar.
    Space,
    /// Left Shift key.
    ShiftLeft,
    /// Left Control key.
    ControlLeft,
}

/// Mouse button codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_event_serialization() {
        let event = InputEvent::MoveForward(true);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: InputEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn input_event_look_delta_serialization() {
        let event = InputEvent::LookDelta {
            yaw: 0.5,
            pitch: -0.3,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: InputEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn input_state_creation() {
        let state = InputState::new();
        assert!(!state.forward);
        assert!(!state.back);
        assert!(!state.left);
        assert!(!state.right);
        assert!(!state.jump);
        assert!(!state.crouch);
        assert!(!state.sprint);
    }

    #[test]
    fn input_state_apply_event() {
        let mut state = InputState::new();

        state.apply_event(InputEvent::MoveForward(true));
        assert!(state.forward);

        state.apply_event(InputEvent::Jump(true));
        assert!(state.jump);
        assert!(state.forward); // Previous state preserved

        state.apply_event(InputEvent::MoveForward(false));
        assert!(!state.forward);
        assert!(state.jump); // Other state preserved
    }

    #[test]
    fn input_state_look_delta_ignored() {
        let mut state = InputState::new();
        state.apply_event(InputEvent::LookDelta {
            yaw: 1.0,
            pitch: 1.0,
        });

        // Look delta should not affect state
        let (forward, right) = state.movement_direction();
        assert_eq!(forward, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn movement_direction_forward() {
        let mut state = InputState::new();
        state.forward = true;

        let (forward, right) = state.movement_direction();
        assert_eq!(forward, 1.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn movement_direction_backward() {
        let mut state = InputState::new();
        state.back = true;

        let (forward, right) = state.movement_direction();
        assert_eq!(forward, -1.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn movement_direction_strafe() {
        let mut state = InputState::new();
        state.left = true;

        let (forward, right) = state.movement_direction();
        assert_eq!(forward, 0.0);
        assert_eq!(right, -1.0);

        state.left = false;
        state.right = true;

        let (forward, right) = state.movement_direction();
        assert_eq!(forward, 0.0);
        assert_eq!(right, 1.0);
    }

    #[test]
    fn movement_direction_diagonal_normalized() {
        let mut state = InputState::new();
        state.forward = true;
        state.right = true;

        let (forward, right) = state.movement_direction();

        // Should be normalized (length = 1)
        let length = (forward * forward + right * right).sqrt();
        assert!((length - 1.0).abs() < 0.001);

        // Each component should be ~0.707
        assert!((forward - 0.707).abs() < 0.01);
        assert!((right - 0.707).abs() < 0.01);
    }

    #[test]
    fn movement_direction_opposite_cancel() {
        let mut state = InputState::new();
        state.forward = true;
        state.back = true;

        let (forward, right) = state.movement_direction();
        assert_eq!(forward, 0.0);
        assert_eq!(right, 0.0);

        state = InputState::new();
        state.left = true;
        state.right = true;

        let (forward, right) = state.movement_direction();
        assert_eq!(forward, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn keycode_serialization() {
        let key = KeyCode::W;
        let json = serde_json::to_string(&key).unwrap();
        let deserialized: KeyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(key, deserialized);
    }

    #[test]
    fn mouse_button_serialization() {
        let button = MouseButton::Left;
        let json = serde_json::to_string(&button).unwrap();
        let deserialized: MouseButton = serde_json::from_str(&json).unwrap();
        assert_eq!(button, deserialized);
    }
}
