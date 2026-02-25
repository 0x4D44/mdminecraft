use mdminecraft_core::{SimTick, SimTime};
use serde::{Deserialize, Serialize};

/// Configuration driving day/night and weather transitions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Number of simulation ticks per full day/night cycle.
    pub day_ticks: u64,
    /// Number of ticks to interpolate sunrise/sunset lighting ramps.
    pub twilight_ticks: u64,
    /// Number of ticks required to fully transition between weather states.
    pub weather_transition_ticks: u64,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            day_ticks: SimTime::DEFAULT_DAY_TICKS,
            twilight_ticks: 1_000,
            weather_transition_ticks: 2_000,
        }
    }
}

/// High-level weather state replicated to clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherState {
    /// Clear skies; no precipitation.
    Clear,
    /// Precipitation is active (rain or snow depending on biome).
    Precipitation,
}

/// Snapshot of environment values used by lighting and gameplay.
#[derive(Debug, Clone, Copy)]
pub struct EnvironmentSnapshot {
    pub tick: SimTick,
    pub time_of_day: f32,
    pub sun_scalar: f32,
    pub weather: WeatherState,
}

/// Authoritative environment controller.
#[derive(Debug, Clone)]
pub struct EnvironmentState {
    sim_time: SimTime,
    weather: WeatherState,
    target_weather: WeatherState,
    transition_ticks_remaining: u64,
}

impl EnvironmentState {
    /// Create a new environment controller at tick zero.
    pub fn new(config: &EnvironmentConfig) -> Self {
        Self {
            sim_time: SimTime::new(config.day_ticks),
            weather: WeatherState::Clear,
            target_weather: WeatherState::Clear,
            transition_ticks_remaining: 0,
        }
    }

    /// Force the weather to a specific state (used by admin commands/tests).
    pub fn set_weather(&mut self, weather: WeatherState) {
        self.weather = weather;
        self.target_weather = weather;
        self.transition_ticks_remaining = 0;
    }

    /// Begin transitioning toward the supplied weather state.
    pub fn schedule_weather_transition(
        &mut self,
        weather: WeatherState,
        config: &EnvironmentConfig,
    ) {
        if weather != self.weather {
            self.target_weather = weather;
            self.transition_ticks_remaining = config.weather_transition_ticks;
        }
    }

    /// Advance the environment by `delta_ticks` and return a snapshot.
    pub fn tick(&mut self, delta_ticks: u64, config: &EnvironmentConfig) -> EnvironmentSnapshot {
        self.sim_time.set_day_ticks(config.day_ticks);
        self.sim_time.advance(delta_ticks);
        self.update_weather(delta_ticks);

        let time_of_day = self.sim_time.time_of_day();
        EnvironmentSnapshot {
            tick: self.sim_time.tick(),
            time_of_day,
            sun_scalar: sun_scalar(
                time_of_day,
                config.twilight_ticks as f32 / config.day_ticks as f32,
            ),
            weather: self.weather,
        }
    }

    fn update_weather(&mut self, delta_ticks: u64) {
        if self.transition_ticks_remaining == 0 || self.weather == self.target_weather {
            return;
        }
        self.transition_ticks_remaining =
            self.transition_ticks_remaining.saturating_sub(delta_ticks);
        if self.transition_ticks_remaining == 0 {
            self.weather = self.target_weather;
        }
    }

    /// Current simulation time for this environment controller.
    pub fn sim_time(&self) -> SimTime {
        self.sim_time
    }
}

/// Compute a normalized sunlight scalar given the time-of-day.
pub fn sun_scalar(time_of_day: f32, twilight_fraction: f32) -> f32 {
    let twilight = twilight_fraction.clamp(0.0, 0.25);
    let day_angle = (time_of_day * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2;
    let mut scalar = day_angle.cos() * 0.5 + 0.5;
    // Clamp twilight to avoid total darkness during sunrise/sunset.
    if twilight > 0.0 && time_of_day < twilight {
        scalar *= (time_of_day / twilight).clamp(0.0, 1.0);
    } else if twilight > 0.0 && time_of_day > (1.0 - twilight) {
        scalar *= ((1.0 - time_of_day) / twilight).clamp(0.0, 1.0);
    }
    scalar.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_scalar_at_high_noon() {
        let scalar = sun_scalar(0.5, 0.05);
        assert!(scalar > 0.45);
    }

    #[test]
    fn sun_scalar_at_midnight_is_low() {
        let scalar = sun_scalar(0.0, 0.05);
        assert!(scalar < 0.05);
    }

    #[test]
    fn environment_snapshot_advances_time() {
        let config = EnvironmentConfig::default();
        let mut env = EnvironmentState::new(&config);
        let snap = env.tick(1_000, &config);
        assert!(snap.time_of_day > 0.0);
    }

    #[test]
    fn weather_transition_applies() {
        let config = EnvironmentConfig::default();
        let mut env = EnvironmentState::new(&config);
        env.schedule_weather_transition(WeatherState::Precipitation, &config);
        let _ = env.tick(config.weather_transition_ticks, &config);
        assert_eq!(env.weather, WeatherState::Precipitation);
    }
}
