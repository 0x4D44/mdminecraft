//! Input configuration and key bindings.
//!
//! This module provides configuration for input sensitivity and key bindings.

use crate::input::{InputEvent, KeyCode};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input configuration with mouse sensitivity and key bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Mouse sensitivity multiplier (default: 0.002).
    pub mouse_sensitivity: f32,
    /// Key bindings map (key -> action name).
    #[serde(default = "default_key_bindings")]
    pub key_bindings: HashMap<String, String>,
}

impl InputConfig {
    /// Create a new input config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load config from TOML string.
    pub fn from_toml(toml: &str) -> Result<Self> {
        Ok(toml::from_str(toml)?)
    }

    /// Get input event for a key press.
    pub fn key_to_event(&self, key: KeyCode, pressed: bool) -> Option<InputEvent> {
        let key_name = format!("{:?}", key);
        let action = self.key_bindings.get(&key_name)?;

        match action.as_str() {
            "forward" => Some(InputEvent::MoveForward(pressed)),
            "back" => Some(InputEvent::MoveBack(pressed)),
            "left" => Some(InputEvent::MoveLeft(pressed)),
            "right" => Some(InputEvent::MoveRight(pressed)),
            "jump" => Some(InputEvent::Jump(pressed)),
            "crouch" => Some(InputEvent::Crouch(pressed)),
            "sprint" => Some(InputEvent::Sprint(pressed)),
            _ => None,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.002,
            key_bindings: default_key_bindings(),
        }
    }
}

fn default_key_bindings() -> HashMap<String, String> {
    let mut bindings = HashMap::new();
    bindings.insert("W".to_string(), "forward".to_string());
    bindings.insert("S".to_string(), "back".to_string());
    bindings.insert("A".to_string(), "left".to_string());
    bindings.insert("D".to_string(), "right".to_string());
    bindings.insert("Space".to_string(), "jump".to_string());
    bindings.insert("ShiftLeft".to_string(), "crouch".to_string());
    bindings.insert("ControlLeft".to_string(), "sprint".to_string());
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_config_creation() {
        let config = InputConfig::new();
        assert_eq!(config.mouse_sensitivity, 0.002);
        assert!(!config.key_bindings.is_empty());
    }

    #[test]
    fn input_config_default() {
        let config = InputConfig::default();
        assert_eq!(config.mouse_sensitivity, 0.002);

        // Check default bindings
        assert_eq!(config.key_bindings.get("W"), Some(&"forward".to_string()));
        assert_eq!(config.key_bindings.get("S"), Some(&"back".to_string()));
        assert_eq!(config.key_bindings.get("A"), Some(&"left".to_string()));
        assert_eq!(config.key_bindings.get("D"), Some(&"right".to_string()));
        assert_eq!(config.key_bindings.get("Space"), Some(&"jump".to_string()));
        assert_eq!(
            config.key_bindings.get("ShiftLeft"),
            Some(&"crouch".to_string())
        );
        assert_eq!(
            config.key_bindings.get("ControlLeft"),
            Some(&"sprint".to_string())
        );
    }

    #[test]
    fn input_config_from_toml() -> Result<()> {
        let toml = r#"
            mouse_sensitivity = 0.005
            [key_bindings]
            W = "forward"
            S = "back"
            A = "left"
            D = "right"
        "#;

        let config = InputConfig::from_toml(toml)?;
        assert_eq!(config.mouse_sensitivity, 0.005);
        assert_eq!(config.key_bindings.get("W"), Some(&"forward".to_string()));

        Ok(())
    }

    #[test]
    fn input_config_from_toml_partial() -> Result<()> {
        let toml = r#"
            mouse_sensitivity = 0.003
        "#;

        let config = InputConfig::from_toml(toml)?;
        assert_eq!(config.mouse_sensitivity, 0.003);

        // Should have default key bindings
        assert!(!config.key_bindings.is_empty());

        Ok(())
    }

    #[test]
    fn key_to_event_forward() {
        let config = InputConfig::default();
        let event = config.key_to_event(KeyCode::W, true);
        assert_eq!(event, Some(InputEvent::MoveForward(true)));

        let event = config.key_to_event(KeyCode::W, false);
        assert_eq!(event, Some(InputEvent::MoveForward(false)));
    }

    #[test]
    fn key_to_event_all_bindings() {
        let config = InputConfig::default();

        assert_eq!(
            config.key_to_event(KeyCode::W, true),
            Some(InputEvent::MoveForward(true))
        );
        assert_eq!(
            config.key_to_event(KeyCode::S, true),
            Some(InputEvent::MoveBack(true))
        );
        assert_eq!(
            config.key_to_event(KeyCode::A, true),
            Some(InputEvent::MoveLeft(true))
        );
        assert_eq!(
            config.key_to_event(KeyCode::D, true),
            Some(InputEvent::MoveRight(true))
        );
        assert_eq!(
            config.key_to_event(KeyCode::Space, true),
            Some(InputEvent::Jump(true))
        );
        assert_eq!(
            config.key_to_event(KeyCode::ShiftLeft, true),
            Some(InputEvent::Crouch(true))
        );
        assert_eq!(
            config.key_to_event(KeyCode::ControlLeft, true),
            Some(InputEvent::Sprint(true))
        );
    }

    #[test]
    fn key_to_event_custom_bindings() -> Result<()> {
        let toml = r#"
            mouse_sensitivity = 0.002
            [key_bindings]
            W = "jump"
        "#;

        let config = InputConfig::from_toml(toml)?;
        let event = config.key_to_event(KeyCode::W, true);
        assert_eq!(event, Some(InputEvent::Jump(true)));

        Ok(())
    }

    #[test]
    fn config_serialization() -> Result<()> {
        let config = InputConfig::default();
        let toml = toml::to_string(&config)?;

        // Should be able to deserialize back
        let deserialized = InputConfig::from_toml(&toml)?;
        assert_eq!(deserialized.mouse_sensitivity, config.mouse_sensitivity);

        Ok(())
    }
}
