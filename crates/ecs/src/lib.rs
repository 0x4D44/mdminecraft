#![warn(missing_docs)]
//! ECS schedule helpers wrapping `bevy_ecs` for deterministic staging.

use bevy_ecs::schedule::{Schedule, ScheduleLabel, Schedules};
use bevy_ecs::world::World;
use mdminecraft_core::SimTick;

/// Labels for the default deterministic schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel)]
pub struct DefaultSimSchedule;

/// Build the baseline deterministic simulation schedule.
pub fn build_default_schedule() -> Schedules {
    let mut schedules = Schedules::default();
    let mut schedule = Schedule::new(DefaultSimSchedule);
    schedule.set_apply_final_deferred(true);
    schedules.insert(schedule);
    schedules
}

/// Run the default schedule for a given tick.
pub fn run_tick(world: &mut World, schedules: &mut Schedules, tick: SimTick) {
    tracing::debug!(tick = tick.0, "running deterministic schedule");
    if let Some(schedule) = schedules.get_mut(DefaultSimSchedule) {
        schedule.run(world);
    }
}
