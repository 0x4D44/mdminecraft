//! Deterministic simulation time and day/night cycle.
//!
//! Provides SimTime resource for tracking in-game time progression and computing
//! sun elevation for skylight scaling. All time advancement is tick-based to ensure
//! deterministic replay.

use mdminecraft_core::SimTick;
use serde::{Deserialize, Serialize};

/// Simulation time state tracking day/night cycles.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SimTime {
    /// Current simulation tick.
    pub tick: SimTick,
    /// Ticks per in-game day (default: 24000 = 20 minutes at 20 TPS).
    pub ticks_per_day: u64,
}

impl SimTime {
    /// Create a new SimTime starting at tick 0.
    pub fn new(ticks_per_day: u64) -> Self {
        Self {
            tick: SimTick::ZERO,
            ticks_per_day,
        }
    }

    /// Default configuration: 24000 ticks per day (20 minutes real-time at 20 TPS).
    pub fn default() -> Self {
        Self::new(24000)
    }

    /// Advance time by one tick.
    pub fn advance(&mut self) {
        self.tick = self.tick.advance(1);
    }

    /// Get the current time of day as a fraction (0.0 = midnight, 0.5 = noon, 1.0 = next midnight).
    pub fn time_of_day(&self) -> f64 {
        let tick_in_day = self.tick.0 % self.ticks_per_day;
        tick_in_day as f64 / self.ticks_per_day as f64
    }

    /// Compute sun elevation angle in radians (-π/2 to π/2).
    /// Returns 0.0 at sunrise/sunset, π/2 at noon, -π/2 at midnight.
    pub fn sun_elevation(&self) -> f64 {
        let time_of_day = self.time_of_day();
        // Map time_of_day (0.0-1.0) to angle:
        // 0.0 (midnight) -> -π/2
        // 0.25 (sunrise) -> 0
        // 0.5 (noon) -> π/2
        // 0.75 (sunset) -> 0
        // 1.0 (next midnight) -> -π/2
        let angle = (time_of_day - 0.25) * 2.0 * std::f64::consts::PI;
        (angle.sin() * std::f64::consts::PI / 2.0)
            .clamp(-std::f64::consts::PI / 2.0, std::f64::consts::PI / 2.0)
    }

    /// Compute skylight scalar based on sun elevation (0.0 = night, 1.0 = full daylight).
    /// This scalar multiplies the base skylight level (15) for ambient lighting.
    pub fn skylight_scalar(&self) -> f32 {
        let elevation = self.sun_elevation();
        // Map elevation from [-π/2, π/2] to [0.2, 1.0] (minimum ambient light at night)
        let normalized = (elevation + std::f64::consts::PI / 2.0) / std::f64::consts::PI;
        (0.2 + 0.8 * normalized) as f32
    }

    /// Get effective skylight level (0-15) based on current time of day.
    pub fn effective_skylight(&self) -> u8 {
        (15.0 * self.skylight_scalar()).round() as u8
    }
}

impl Default for SimTime {
    fn default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_of_day_wraps_at_day_boundary() {
        let mut time = SimTime::new(100);
        assert_eq!(time.time_of_day(), 0.0);

        // Advance halfway through day.
        for _ in 0..50 {
            time.advance();
        }
        assert!((time.time_of_day() - 0.5).abs() < 0.01);

        // Advance to end of day.
        for _ in 0..50 {
            time.advance();
        }
        assert!((time.time_of_day() - 0.0).abs() < 0.01);
    }

    #[test]
    fn sun_elevation_peaks_at_noon() {
        let mut time = SimTime::new(24000);

        // Advance to noon (tick 6000 = 0.25 of day).
        for _ in 0..12000 {
            time.advance();
        }

        let elevation = time.sun_elevation();
        // At noon (time_of_day = 0.5), elevation should be near π/2.
        assert!((elevation - std::f64::consts::PI / 2.0).abs() < 0.1);
    }

    #[test]
    fn skylight_scalar_has_minimum_at_midnight() {
        let time = SimTime::new(24000);
        let scalar = time.skylight_scalar();

        // At midnight (tick 0), scalar should be near minimum (0.2).
        assert!((scalar - 0.2).abs() < 0.05);
    }

    #[test]
    fn skylight_scalar_maximizes_at_noon() {
        let mut time = SimTime::new(24000);

        // Advance to noon.
        for _ in 0..12000 {
            time.advance();
        }

        let scalar = time.skylight_scalar();
        // At noon, scalar should be near 1.0.
        assert!((scalar - 1.0).abs() < 0.05);
    }

    #[test]
    fn effective_skylight_varies_with_time() {
        let mut time = SimTime::new(24000);

        // Midnight: minimum light.
        let midnight_light = time.effective_skylight();
        assert!(midnight_light >= 3 && midnight_light <= 4); // ~20% of 15

        // Noon: maximum light.
        for _ in 0..12000 {
            time.advance();
        }
        let noon_light = time.effective_skylight();
        assert_eq!(noon_light, 15);
    }
}
