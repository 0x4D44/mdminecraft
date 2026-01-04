/// Time-of-day system for dynamic lighting and sky colors.
///
/// Time progresses from 0.0 (midnight) to 1.0 (next midnight).
/// - 0.00-0.25: Night → Dawn
/// - 0.25-0.50: Dawn → Noon
/// - 0.50-0.75: Noon → Dusk
/// - 0.75-1.00: Dusk → Night
///
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
            time: 0.3,  // Start at morning
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
    /// - 0.0 (midnight): Below horizon
    /// - 0.25 (dawn): Rising (east)
    /// - 0.5 (noon): Overhead
    /// - 0.75 (dusk): Setting (west)
    pub fn sun_direction(&self) -> [f32; 3] {
        let angle = self.time * std::f32::consts::TAU;

        // Use a circular orbit so the sun is above the horizon (y>0) during day
        // and below the horizon (y<0) at night.
        let x = angle.sin(); // East-West movement
        let y = -angle.cos(); // Height
        let z = 0.3; // Slight north offset

        // Normalize (the z offset makes the vector non-unit).
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
    /// Current time plus padding for std140 alignment
    pub time: [f32; 4],
    /// Sun direction (normalized)
    pub sun_dir: [f32; 4],
    /// Fog color for current time (rgb) and optional thunder strength (a).
    pub fog_color: [f32; 4],
    /// Fog parameters (start, end, night_vision, precipitation)
    pub fog_params: [f32; 4],
    /// Base sky color tint (rgb) and optional lightning flash (a).
    pub sky_color: [f32; 4],
}

impl TimeUniform {
    /// Create from TimeOfDay state.
    pub fn from_time_of_day(time: &TimeOfDay, weather_intensity: f32, night_vision: f32) -> Self {
        let dir = time.sun_direction();
        let fog_color = fog_color_for_time(time.time());
        let precipitation_tint = [0.55, 0.58, 0.64];
        let fog_color = mix_color(
            fog_color,
            precipitation_tint,
            (weather_intensity * 0.85).clamp(0.0, 1.0),
        );
        let fog_start = mix_scalar(48.0, 24.0, weather_intensity);
        let fog_end = mix_scalar(120.0, 70.0, weather_intensity);
        let night_vision = night_vision.clamp(0.0, 1.0);
        Self {
            time: [time.time(), 0.0, 0.0, 0.0],
            sun_dir: [dir[0], dir[1], dir[2], 0.0],
            fog_color: [fog_color[0], fog_color[1], fog_color[2], 0.0],
            fog_params: [fog_start, fog_end, night_vision, weather_intensity],
            sky_color: [fog_color[0], fog_color[1], fog_color[2], 0.0],
        }
    }
}

fn fog_color_for_time(t: f32) -> [f32; 3] {
    let night = [0.05, 0.07, 0.12];
    let dawn = [0.5, 0.4, 0.35];
    let day = [0.7, 0.8, 0.9];
    let dusk = [0.45, 0.35, 0.4];

    if t < 0.2 {
        mix_color(night, dawn, smoothstep(0.15, 0.2, t))
    } else if t < 0.3 {
        mix_color(dawn, day, smoothstep(0.2, 0.3, t))
    } else if t < 0.7 {
        day
    } else if t < 0.8 {
        mix_color(day, dusk, smoothstep(0.7, 0.8, t))
    } else {
        mix_color(dusk, night, smoothstep(0.8, 0.9, t))
    }
}

fn mix_color(a: [f32; 3], b: [f32; 3], factor: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * factor,
        a[1] + (b[1] - a[1]) * factor,
        a[2] + (b[2] - a[2]) * factor,
    ]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix_scalar(a: f32, b: f32, factor: f32) -> f32 {
    a + (b - a) * factor.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_updates_and_wraps() {
        let mut time = TimeOfDay::new();
        time.set_time(0.95);
        time.update(60.0);
        assert!(time.time() < 0.1, "expected wrap, got {}", time.time());
    }

    #[test]
    fn time_pause_stops_progress() {
        let mut time = TimeOfDay::new();
        time.set_time(0.4);
        time.toggle_pause();
        time.update(10.0);
        assert!((time.time() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn speed_clamps_to_bounds() {
        let mut time = TimeOfDay::new();
        for _ in 0..20 {
            time.increase_speed();
        }
        assert!(time.speed <= 10.0);

        for _ in 0..40 {
            time.decrease_speed();
        }
        assert!(time.speed >= 0.01);
    }

    #[test]
    fn sun_direction_is_normalized() {
        let time = TimeOfDay::new();
        let dir = time.sun_direction();
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4);
    }

    #[test]
    fn period_name_and_daytime_match_ranges() {
        let mut time = TimeOfDay::new();
        time.set_time(0.1);
        assert_eq!(time.period_name(), "Night");
        assert!(!time.is_daytime());

        time.set_time(0.5);
        assert_eq!(time.period_name(), "Day");
        assert!(time.is_daytime());
    }
}
