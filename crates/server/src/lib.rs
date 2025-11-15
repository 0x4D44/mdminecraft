#![warn(missing_docs)]
//! Authoritative simulation host scaffolding.

pub mod multiplayer;

use anyhow::Result;
use bevy_ecs::schedule::Schedules;
use bevy_ecs::world::World;
use mdminecraft_core::SimTick;
use mdminecraft_ecs::{build_default_schedule, run_tick};

/// Minimal server harness that will be expanded with networking and persistence.
pub struct Server {
    world: World,
    schedules: Schedules,
    current_tick: SimTick,
}

impl Server {
    /// Create a new server with default schedules.
    pub fn new() -> Self {
        let world = World::default();
        let schedules = build_default_schedule();
        Self {
            world,
            schedules,
            current_tick: SimTick::ZERO,
        }
    }

    /// Run a single deterministic tick.
    pub fn tick(&mut self) -> Result<()> {
        run_tick(&mut self.world, &mut self.schedules, self.current_tick);
        self.current_tick = self.current_tick.advance(1);
        Ok(())
    }
}
