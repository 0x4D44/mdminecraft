use mdminecraft_core::DimensionId;
use mdminecraft_testkit::{run_micro_worldtest, MicroWorldtestConfig};
use mdminecraft_world::{
    place_end_exit_portal, world_y_to_local_y, Chunk, ChunkPos, Mob, MobState, MobType,
    ProjectileManager, BLOCK_END_PORTAL,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name)
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct ScriptFile {
    steps: Vec<ScriptStepDef>,
}

#[derive(Debug, Deserialize)]
struct ScriptStepDef {
    tick: u64,
    action: ScriptAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScriptAction {
    DamageBoss { amount: f32 },
}

#[derive(Debug, Clone)]
struct ScriptStep {
    tick: u64,
    action: ScriptAction,
}

fn load_script() -> VecDeque<ScriptStep> {
    let path = fixture_path("end_boss_replay_script.json");
    let contents = std::fs::read_to_string(&path).expect("fixture should be readable");
    let file: ScriptFile = serde_json::from_str(&contents).expect("fixture should parse");
    assert!(!file.steps.is_empty(), "script must contain steps");

    let mut last_tick: Option<u64> = None;
    let mut pending = VecDeque::with_capacity(file.steps.len());
    for step in file.steps {
        if let Some(prev) = last_tick {
            assert!(
                step.tick >= prev,
                "script steps must be sorted by tick (got {} after {})",
                step.tick,
                prev
            );
        }
        last_tick = Some(step.tick);
        pending.push_back(ScriptStep {
            tick: step.tick,
            action: step.action,
        });
    }
    pending
}

#[derive(Debug, Clone)]
struct Player {
    x: f64,
    y: f64,
    z: f64,
    health: f32,
}

#[test]
fn micro_end_boss_replay_script_snapshot() {
    struct State {
        chunks: HashMap<ChunkPos, Chunk>,
        boss: Mob,
        projectiles: ProjectileManager,
        player: Player,
        portal_placed: bool,
        script: VecDeque<ScriptStep>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Snap {
        boss_dead: bool,
        boss_enraged: bool,
        boss_health: i32,
        boss_state: MobState,
        player_health: i32,
        fireballs_in_flight: u32,
        portal_blocks: u32,
    }

    let mut chunks = HashMap::new();
    chunks.insert(ChunkPos::new(0, 0), Chunk::new(ChunkPos::new(0, 0)));
    let mut boss = Mob::new(8.5, 80.0, 8.5, MobType::EnderDragon);
    boss.id = 0;
    boss.dimension = DimensionId::End;
    let state = State {
        chunks,
        boss,
        projectiles: ProjectileManager::new(),
        player: Player {
            x: 42.0,
            y: 80.0,
            z: 8.5,
            health: 100.0,
        },
        portal_placed: false,
        script: load_script(),
    };

    const PORTAL_Y: i32 = 80;
    const PORTAL_X: i32 = 8;
    const PORTAL_Z: i32 = 8;

    run_micro_worldtest(
        MicroWorldtestConfig {
            name: "micro_end_boss_replay_script".to_string(),
            ticks: 80,
            snapshot_path: snapshot_path("micro_end_boss_replay_script.json"),
        },
        state,
        |tick, state| {
            while let Some(step) = state.script.front() {
                if step.tick > tick.0 {
                    break;
                }
                let step = state.script.pop_front().expect("front existed");
                match step.action {
                    ScriptAction::DamageBoss { amount } => {
                        state.boss.damage(amount);
                    }
                }
            }

            if !state.boss.dead && state.player.health > 0.0 {
                let _ = state.boss.update_with_target_visibility(
                    tick.0,
                    state.player.x,
                    state.player.y,
                    state.player.z,
                    1.0,
                );

                let fireballs_in_flight = state
                    .projectiles
                    .projectiles
                    .iter()
                    .filter(|projectile| {
                        projectile.dimension == DimensionId::End
                            && !projectile.dead
                            && projectile.projectile_type
                                == mdminecraft_world::ProjectileType::DragonFireball
                    })
                    .count();

                if fireballs_in_flight < 4 {
                    if let Some(projectile) = state.boss.try_spawn_dragon_fireball(
                        tick.0,
                        state.player.x,
                        state.player.y,
                        state.player.z,
                        1.0,
                    ) {
                        state.projectiles.spawn(DimensionId::End, projectile);
                    }
                }
            }

            state.projectiles.update(DimensionId::End);

            for projectile in &mut state.projectiles.projectiles {
                if projectile.dimension != DimensionId::End || projectile.dead || projectile.stuck {
                    continue;
                }
                if projectile.projectile_type != mdminecraft_world::ProjectileType::DragonFireball {
                    continue;
                }

                if projectile.hits_point(state.player.x, state.player.y, state.player.z, 0.6) {
                    state.player.health -= projectile.damage();
                    projectile.hit();
                }
            }

            if state.boss.dead && !state.portal_placed {
                state.portal_placed =
                    place_end_exit_portal(&mut state.chunks, PORTAL_X, PORTAL_Y, PORTAL_Z)
                        .is_some();
            }
        },
        |_tick, state| {
            let boss_enraged =
                (state.boss.health as f64) <= (state.boss.mob_type.max_health() as f64) * 0.5;

            let portal_local_y = world_y_to_local_y(PORTAL_Y).expect("portal y is within bounds");
            let portal_blocks = (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dz| (dx, dz)))
                .filter(|(dx, dz)| {
                    let x = PORTAL_X + *dx;
                    let z = PORTAL_Z + *dz;
                    let local_x = x.rem_euclid(16) as usize;
                    let local_z = z.rem_euclid(16) as usize;
                    state
                        .chunks
                        .get(&ChunkPos::new(0, 0))
                        .expect("chunk exists")
                        .voxel(local_x, portal_local_y, local_z)
                        .id
                        == BLOCK_END_PORTAL
                })
                .count() as u32;

            let fireballs_in_flight = state
                .projectiles
                .projectiles
                .iter()
                .filter(|projectile| {
                    projectile.dimension == DimensionId::End
                        && !projectile.dead
                        && projectile.projectile_type
                            == mdminecraft_world::ProjectileType::DragonFireball
                })
                .count() as u32;

            Snap {
                boss_dead: state.boss.dead,
                boss_enraged,
                boss_health: state.boss.health.round() as i32,
                boss_state: state.boss.state,
                player_health: state.player.health.round() as i32,
                fireballs_in_flight,
                portal_blocks,
            }
        },
    )
    .expect("snapshot verified");
}
