use std::time::{Duration, Instant};

use anyhow::Result;
use bevy_ecs::{
    schedule::Schedule,
    system::{IntoSystemConfigs, Query},
    world::World,
};
use clap::Parser;
use hecs::World as HecsWorld;
use rand::{rngs::StdRng, Rng, SeedableRng};

#[derive(Parser, Debug)]
#[command(author, version, about = "Compare ECS runtimes for mdminecraft workloads", long_about = None)]
struct Args {
    /// Number of entities to spawn in each benchmark
    #[arg(long, default_value_t = 50_000)]
    entities: usize,
    /// Number of simulation ticks to run per benchmark
    #[arg(long, default_value_t = 200)]
    ticks: usize,
    /// Random seed for reproducibility
    #[arg(long, default_value_t = 1337)]
    seed: u64,
}

#[derive(Clone, Copy)]
struct Position {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy)]
struct Velocity {
    x: f32,
    y: f32,
    z: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!(
        "Running ECS benchmarks with {} entities, {} ticks",
        args.entities, args.ticks
    );

    let bevy_duration = benchmark_bevy(args.entities, args.ticks, args.seed);
    let hecs_duration = benchmark_hecs(args.entities, args.ticks, args.seed);

    println!("\nResults:");
    println!(
        "  bevy_ecs: {:.2?} (avg {:.4?}/tick)",
        bevy_duration,
        bevy_duration / args.ticks as u32
    );
    println!(
        "  hecs    : {:.2?} (avg {:.4?}/tick)",
        hecs_duration,
        hecs_duration / args.ticks as u32
    );

    Ok(())
}

fn benchmark_bevy(entities: usize, ticks: usize, seed: u64) -> Duration {
    let mut world = World::new();
    spawn_entities_bevy(&mut world, entities, seed);

    fn integrate(mut query: Query<(&mut Position, &Velocity)>) {
        for (mut pos, vel) in &mut query {
            pos.x += vel.x;
            pos.y += vel.y;
            pos.z += vel.z;
        }
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(integrate);

    let start = Instant::now();
    for _ in 0..ticks {
        schedule.run(&mut world);
    }
    start.elapsed()
}

fn spawn_entities_bevy(world: &mut World, entities: usize, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    for _ in 0..entities {
        let vel = Velocity {
            x: rng.gen_range(-1.0..1.0),
            y: rng.gen_range(-1.0..1.0),
            z: rng.gen_range(-1.0..1.0),
        };
        let pos = Position {
            x: rng.gen_range(-512.0..512.0),
            y: rng.gen_range(0.0..256.0),
            z: rng.gen_range(-512.0..512.0),
        };
        world.spawn((pos, vel));
    }
}

fn benchmark_hecs(entities: usize, ticks: usize, seed: u64) -> Duration {
    let mut world = HecsWorld::new();
    spawn_entities_hecs(&mut world, entities, seed);

    let start = Instant::now();
    for _ in 0..ticks {
        let mut query = world.query::<(&mut Position, &Velocity)>();
        for (mut pos, vel) in query.iter() {
            pos.x += vel.x;
            pos.y += vel.y;
            pos.z += vel.z;
        }
    }
    start.elapsed()
}

fn spawn_entities_hecs(world: &mut HecsWorld, entities: usize, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    for _ in 0..entities {
        let vel = Velocity {
            x: rng.gen_range(-1.0..1.0),
            y: rng.gen_range(-1.0..1.0),
            z: rng.gen_range(-1.0..1.0),
        };
        let pos = Position {
            x: rng.gen_range(-512.0..512.0),
            y: rng.gen_range(0.0..256.0),
            z: rng.gen_range(-512.0..512.0),
        };
        world.spawn((pos, vel));
    }
}
