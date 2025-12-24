//! Deterministic weather system for environmental simulation.
//!
//! Provides weather state management and event emission for replay/testing.
//! Weather changes are deterministic and logged for CI reproducibility.

use serde::{Deserialize, Serialize};

/// Weather state affecting ambient lighting and gameplay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeatherState {
    /// Clear skies, no precipitation.
    #[default]
    Clear,
    /// Active precipitation (rain in warm biomes, snow in cold biomes).
    Precipitation,
    /// Active precipitation plus thunder/lightning (Overworld-only visuals/gameplay).
    Thunderstorm,
}

/// Component attached to world singleton for weather management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherToggle {
    /// Current weather state.
    pub state: WeatherState,
}

impl WeatherToggle {
    /// Create a new weather toggle starting in clear state.
    pub fn new() -> Self {
        Self {
            state: WeatherState::Clear,
        }
    }

    /// Set the weather state (emits WeatherChanged event in ECS context).
    pub fn set_state(&mut self, state: WeatherState) {
        self.state = state;
    }

    /// Toggle between clear and precipitation.
    pub fn toggle(&mut self) {
        self.state = match self.state {
            WeatherState::Clear => WeatherState::Precipitation,
            WeatherState::Precipitation | WeatherState::Thunderstorm => WeatherState::Clear,
        };
    }

    /// Check if currently raining/snowing.
    pub fn is_precipitating(&self) -> bool {
        matches!(
            self.state,
            WeatherState::Precipitation | WeatherState::Thunderstorm
        )
    }

    /// Check if currently thundering.
    pub fn is_thundering(&self) -> bool {
        self.state == WeatherState::Thunderstorm
    }

    /// Get weather-modified skylight scalar (optional: reduce light during storms).
    pub fn skylight_modifier(&self) -> f32 {
        match self.state {
            WeatherState::Clear => 1.0,
            WeatherState::Precipitation => 0.85, // 15% light reduction during storms
            WeatherState::Thunderstorm => 0.78,  // darker and more dramatic
        }
    }
}

impl Default for WeatherToggle {
    fn default() -> Self {
        Self::new()
    }
}

/// Event emitted when weather changes (for testkit logging and replay).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherChanged {
    /// Previous weather state.
    pub from: WeatherState,
    /// New weather state.
    pub to: WeatherState,
}

impl WeatherChanged {
    /// Create a weather change event.
    pub fn new(from: WeatherState, to: WeatherState) -> Self {
        Self { from, to }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_toggle_starts_clear() {
        let weather = WeatherToggle::new();
        assert_eq!(weather.state, WeatherState::Clear);
        assert!(!weather.is_precipitating());
        assert!(!weather.is_thundering());
    }

    #[test]
    fn toggle_switches_between_states() {
        let mut weather = WeatherToggle::new();
        weather.toggle();
        assert_eq!(weather.state, WeatherState::Precipitation);
        assert!(weather.is_precipitating());

        weather.toggle();
        assert_eq!(weather.state, WeatherState::Clear);
        assert!(!weather.is_precipitating());
    }

    #[test]
    fn precipitation_reduces_skylight() {
        let clear = WeatherToggle::new();
        assert_eq!(clear.skylight_modifier(), 1.0);

        let mut rainy = WeatherToggle::new();
        rainy.set_state(WeatherState::Precipitation);
        assert!(rainy.skylight_modifier() < 1.0);
        assert!((rainy.skylight_modifier() - 0.85).abs() < 0.01);

        rainy.set_state(WeatherState::Thunderstorm);
        assert!(rainy.is_thundering());
        assert!(rainy.skylight_modifier() < 0.85);
    }

    #[test]
    fn weather_changed_event_tracks_transition() {
        let event = WeatherChanged::new(WeatherState::Clear, WeatherState::Precipitation);
        assert_eq!(event.from, WeatherState::Clear);
        assert_eq!(event.to, WeatherState::Precipitation);
    }
}
