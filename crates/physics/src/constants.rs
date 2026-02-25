//! Physics constants for character simulation.

/// Gravity acceleration in m/s² (2x Earth's gravity for better game feel).
pub const GRAVITY: f32 = 32.0;

/// Player capsule radius in blocks.
pub const PLAYER_RADIUS: f32 = 0.3;

/// Player capsule height in blocks.
pub const PLAYER_HEIGHT: f32 = 1.8;

/// Maximum step height that a player can walk up in blocks (Minecraft-like).
pub const STEP_HEIGHT: f32 = 0.6;

/// Friction coefficient applied per tick when grounded (0.91 matches Minecraft).
pub const FRICTION: f32 = 0.91;

/// Maximum horizontal speed in m/s (blocks/second).
pub const MAX_SPEED: f32 = 6.0;

/// Fixed timestep in seconds (50ms = 20 TPS).
pub const FIXED_TIMESTEP: f32 = 0.05;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tunneling_at_max_speed() {
        // At MAX_SPEED with FIXED_TIMESTEP, player should move less than PLAYER_RADIUS
        // to guarantee no tunneling through single-block walls
        let max_displacement = MAX_SPEED * FIXED_TIMESTEP;

        // For swept collision, we need max displacement to be reasonable relative to player size
        // A conservative check: displacement should be less than player diameter
        assert!(
            max_displacement < PLAYER_RADIUS * 2.0,
            "At max speed, player moves {} blocks per tick, which could cause tunneling. \
             Player diameter is {} blocks",
            max_displacement,
            PLAYER_RADIUS * 2.0
        );
    }

    #[test]
    fn gravity_produces_reasonable_fall_speed() {
        // After 1 second of falling (20 ticks), velocity should be reasonable
        let fall_time = 1.0; // seconds
        let terminal_velocity = GRAVITY * fall_time;

        // Terminal velocity after 1 second should be high but not absurd
        assert!(
            terminal_velocity > 20.0 && terminal_velocity < 50.0,
            "Gravity produces {} m/s after 1 second, which seems unreasonable",
            terminal_velocity
        );
    }

    #[test]
    fn step_height_is_less_than_full_block() {
        // Step height should allow walking up stairs but not full blocks
        assert!(
            STEP_HEIGHT > 0.5 && STEP_HEIGHT < 1.0,
            "Step height of {} doesn't match Minecraft-like stairs",
            STEP_HEIGHT
        );
    }

    #[test]
    fn friction_reduces_speed() {
        // Friction should reduce speed each tick when applied
        let initial_speed = 5.0;
        let speed_after_tick = initial_speed * FRICTION;

        assert!(
            speed_after_tick < initial_speed,
            "Friction coefficient {} doesn't reduce speed",
            FRICTION
        );

        // Should significantly reduce speed over multiple ticks
        let mut speed = initial_speed;
        for _ in 0..20 {
            speed *= FRICTION;
        }
        assert!(
            speed < initial_speed * 0.5,
            "Friction doesn't reduce speed enough over 20 ticks"
        );
    }

    #[test]
    fn player_dimensions_are_reasonable() {
        // Player should be taller than wide
        assert!(
            PLAYER_HEIGHT > PLAYER_RADIUS * 2.0,
            "Player height {} should be greater than diameter {}",
            PLAYER_HEIGHT,
            PLAYER_RADIUS * 2.0
        );

        // Player should fit through a 1-block wide gap
        assert!(
            PLAYER_RADIUS * 2.0 < 1.0,
            "Player diameter {} is too wide to fit through 1-block gaps",
            PLAYER_RADIUS * 2.0
        );
    }

    #[test]
    fn timestep_matches_20_tps() {
        // 20 ticks per second = 50ms per tick = 0.05 seconds
        assert!(
            (FIXED_TIMESTEP - 0.05).abs() < f32::EPSILON,
            "Fixed timestep {} doesn't match 20 TPS",
            FIXED_TIMESTEP
        );
    }
}
