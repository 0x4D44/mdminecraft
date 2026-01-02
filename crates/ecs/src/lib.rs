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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::{ResMut, Resource};

    #[derive(Resource, Default)]
    struct Counter(u32);

    #[test]
    fn default_schedule_runs_added_systems() {
        let mut world = World::default();
        world.insert_resource(Counter::default());
        let mut schedules = build_default_schedule();

        if let Some(schedule) = schedules.get_mut(DefaultSimSchedule) {
            schedule.add_systems(|mut counter: ResMut<Counter>| {
                counter.0 += 1;
            });
        }

        run_tick(&mut world, &mut schedules, SimTick::ZERO);
        assert_eq!(world.resource::<Counter>().0, 1);
    }
}
