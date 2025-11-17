/// Time-of-day system for dynamic lighting and sky colors.
///
/// Time progresses from 0.0 (midnight) to 1.0 (next midnight).
/// - 0.00-0.25: Night → Dawn
/// - 0.25-0.50: Dawn → Noon
/// - 0.50-0.75: Noon → Dusk
/// - 0.75-1.00: Dusk → Night

/// Time-of-day state.
#[derive(Debug, Clone)]
pub struct TimeOfDay {
    /// Current time (0.0 to 1.0, wraps around)
    time: f32,
    /// Time progression speed (default: 1.0 = 1 minute per cycle)
    speed: f32,
    /// Whether time is paused
    paused: bool,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self {
            time: 0.3, // Start at morning
            speed: 0.1, // Slower progression for testing (60 seconds = 1 cycle)
            paused: false,
        }
    }
}

impl TimeOfDay {
    /// Create a new time-of-day system.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update time based on delta time.
    pub fn update(&mut self, dt: f32) {
        if !self.paused {
            // Progress time (cycle_duration seconds per full cycle)
            let cycle_duration = 60.0 / self.speed; // seconds per cycle
            self.time += dt / cycle_duration;

            // Wrap around at 1.0
            if self.time >= 1.0 {
                self.time -= 1.0;
            }
        }
    }

    /// Get current time (0.0 to 1.0).
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Set time directly.
    pub fn set_time(&mut self, time: f32) {
        self.time = time.clamp(0.0, 1.0);
    }

    /// Toggle pause.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Increase speed.
    pub fn increase_speed(&mut self) {
        self.speed = (self.speed * 1.5).min(10.0);
    }

    /// Decrease speed.
    pub fn decrease_speed(&mut self) {
        self.speed = (self.speed / 1.5).max(0.01);
    }

    /// Get sun direction based on time.
    ///
    /// Returns a normalized direction vector pointing toward the sun.
    /// - 0.0 (midnight): Below horizon (west)
    /// - 0.25 (dawn): Rising (east)
    /// - 0.5 (noon): Overhead
    /// - 0.75 (dusk): Setting (west)
    pub fn sun_direction(&self) -> [f32; 3] {
        // Sun rotates around Y axis
        // At noon (0.5), sun is overhead
        // At midnight (0.0/1.0), sun is below horizon

        let angle = self.time * std::f32::consts::PI * 2.0;

        // Sun position on unit sphere
        let x = angle.sin() * 0.5;           // East-West movement
        let y = (-angle.cos() * 0.5) + 0.5;  // Height (0 at horizon, 1 at zenith)
        let z = 0.3;                          // Slight north offset

        // Normalize
        let length = (x * x + y * y + z * z).sqrt();
        [x / length, y / length, z / length]
    }

    /// Get time period name for debugging.
    pub fn period_name(&self) -> &'static str {
        match self.time {
            t if t < 0.2 => "Night",
            t if t < 0.3 => "Dawn",
            t if t < 0.7 => "Day",
            t if t < 0.8 => "Dusk",
            _ => "Night",
        }
    }

    /// Check if it's currently daytime.
    pub fn is_daytime(&self) -> bool {
        self.time >= 0.25 && self.time < 0.75
    }
}

/// GPU uniform for time-of-day.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TimeUniform {
    /// Current time (0.0 to 1.0)
    pub time: f32,
    /// Sun direction (normalized)
    pub sun_dir: [f32; 3],
}

impl TimeUniform {
    /// Create from TimeOfDay state.
    pub fn from_time_of_day(time: &TimeOfDay) -> Self {
        Self {
            time: time.time(),
            sun_dir: time.sun_direction(),
        }
    }
}
