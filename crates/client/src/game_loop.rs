//! Game loop with fixed timestep physics and variable framerate rendering.
//!
//! This module provides the main game loop that integrates all game systems.

use anyhow::Result;
use std::time::{Duration, Instant};

/// Fixed physics tick rate (20 ticks per second, matching Minecraft).
pub const PHYSICS_TPS: u32 = 20;

/// Target frame rate for rendering (60 FPS).
pub const TARGET_FPS: u32 = 60;

/// Physics timestep in seconds.
const PHYSICS_TIMESTEP: f32 = 1.0 / PHYSICS_TPS as f32;

/// Target frame duration.
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS as u64);

/// Game state holding all game systems.
///
/// This is generic to allow testing with different implementations.
pub struct GameState<P, R> {
    /// Physics/gameplay state.
    pub physics: P,
    /// Renderer.
    pub renderer: R,
    /// Accumulated time for fixed timestep.
    accumulator: f32,
    /// Last update time.
    last_update: Instant,
    /// Frame count for diagnostics.
    frame_count: u64,
    /// Tick count for diagnostics.
    tick_count: u64,
}

impl<P, R> GameState<P, R> {
    /// Create a new game state.
    ///
    /// # Arguments
    /// * `physics` - The physics/gameplay state
    /// * `renderer` - The renderer
    pub fn new(physics: P, renderer: R) -> Self {
        Self {
            physics,
            renderer,
            accumulator: 0.0,
            last_update: Instant::now(),
            frame_count: 0,
            tick_count: 0,
        }
    }

    /// Get the current frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Update the game state with fixed timestep physics.
    ///
    /// This uses the accumulator pattern to run physics at a fixed rate
    /// while allowing variable framerate rendering.
    ///
    /// # Arguments
    /// * `physics_update` - Function to update physics state
    /// * `render` - Function to render a frame
    pub fn update<PF, RF>(&mut self, mut physics_update: PF, mut render: RF) -> Result<()>
    where
        PF: FnMut(&mut P, f32) -> Result<()>,
        RF: FnMut(&mut R, &P) -> Result<()>,
    {
        let now = Instant::now();
        let delta = now.duration_since(self.last_update);
        self.last_update = now;

        // Add frame time to accumulator
        self.accumulator += delta.as_secs_f32();

        // Clamp accumulator to prevent spiral of death
        if self.accumulator > PHYSICS_TIMESTEP * 5.0 {
            tracing::warn!(
                "Accumulator overflow: {:.3}s, clamping to prevent spiral of death",
                self.accumulator
            );
            self.accumulator = PHYSICS_TIMESTEP * 5.0;
        }

        // Run physics updates at fixed timestep
        while self.accumulator >= PHYSICS_TIMESTEP {
            physics_update(&mut self.physics, PHYSICS_TIMESTEP)?;
            self.accumulator -= PHYSICS_TIMESTEP;
            self.tick_count += 1;
        }

        // Render after physics updates
        render(&mut self.renderer, &self.physics)?;
        self.frame_count += 1;

        Ok(())
    }

    /// Sleep to maintain target frame rate.
    ///
    /// This should be called after `update()` to limit the frame rate.
    pub fn sleep_for_frame_pacing(&self) {
        let elapsed = self.last_update.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestPhysics {
        position: f32,
        velocity: f32,
        update_count: u32,
    }

    #[derive(Default)]
    struct TestRenderer {
        render_count: u32,
    }

    #[test]
    fn game_state_creation() {
        let physics = TestPhysics::default();
        let renderer = TestRenderer::default();

        let state = GameState::new(physics, renderer);

        assert_eq!(state.frame_count(), 0);
        assert_eq!(state.tick_count(), 0);
    }

    #[test]
    fn game_state_update_runs_physics() {
        let physics = TestPhysics::default();
        let renderer = TestRenderer::default();

        let mut state = GameState::new(physics, renderer);

        // Manually set accumulator to trigger physics update
        state.accumulator = PHYSICS_TIMESTEP * 2.0;

        let result = state.update(
            |p, dt| {
                p.position += p.velocity * dt;
                p.update_count += 1;
                Ok(())
            },
            |r, _p| {
                r.render_count += 1;
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(state.physics.update_count, 2); // Two physics steps
        assert_eq!(state.renderer.render_count, 1); // One render
        assert_eq!(state.tick_count(), 2);
        assert_eq!(state.frame_count(), 1);
    }

    #[test]
    fn game_state_update_renders_every_frame() {
        let physics = TestPhysics::default();
        let renderer = TestRenderer::default();

        let mut state = GameState::new(physics, renderer);

        // Set accumulator to less than one physics step
        state.accumulator = PHYSICS_TIMESTEP * 0.5;

        let result = state.update(
            |p, _dt| {
                p.update_count += 1;
                Ok(())
            },
            |r, _p| {
                r.render_count += 1;
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(state.physics.update_count, 0); // No physics step
        assert_eq!(state.renderer.render_count, 1); // Still renders
        assert_eq!(state.tick_count(), 0);
        assert_eq!(state.frame_count(), 1);
    }

    #[test]
    fn game_state_accumulator_clamps() {
        let physics = TestPhysics::default();
        let renderer = TestRenderer::default();

        let mut state = GameState::new(physics, renderer);

        // Set accumulator way too high
        state.accumulator = PHYSICS_TIMESTEP * 100.0;

        let result = state.update(
            |p, _dt| {
                p.update_count += 1;
                Ok(())
            },
            |r, _p| {
                r.render_count += 1;
                Ok(())
            },
        );

        assert!(result.is_ok());
        // Should only run 5 physics steps due to clamping
        assert_eq!(state.physics.update_count, 5);
        assert_eq!(state.tick_count(), 5);
    }

    #[test]
    fn physics_timestep_constant() {
        assert_eq!(PHYSICS_TIMESTEP, 0.05); // 20 TPS = 0.05s per tick
    }

    #[test]
    fn target_fps_constant() {
        assert_eq!(TARGET_FPS, 60);
    }

    #[test]
    fn physics_tps_constant() {
        assert_eq!(PHYSICS_TPS, 20);
    }
}
