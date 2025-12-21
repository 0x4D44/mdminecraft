use crate::chunk::{Chunk, ChunkPos, Voxel, CHUNK_SIZE_Y};
use crate::structures::{region_coords_for_chunk, region_seed, region_world_bounds};
use crate::structures::{set_world_voxel_if_in_chunk, world_to_chunk_local, StructureBounds};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const FORTRESS_SEED_SALT: u64 = 0x46_4F_52_54_52_45_53_53_u64; // "FORTRESS"

const CORRIDOR_WIDTH: usize = 5;
const CORRIDOR_HEIGHT: usize = 4;
const CORRIDOR_LENGTH: usize = 48;

/// Minimal deterministic Nether fortress generator.
///
/// This is a simplified "fortress-lite" that creates a single enclosed corridor
/// inside a region. It is designed to be deterministic and cross-chunk safe.
#[derive(Debug, Clone, Copy)]
pub struct FortressGenerator {
    world_seed: u64,
}

impl FortressGenerator {
    pub const fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    /// Attempt to generate the region's fortress corridor into this chunk.
    ///
    /// Returns `true` if any part of the fortress intersects this chunk.
    pub fn try_generate_fortress(&self, chunk: &mut Chunk) -> bool {
        let chunk_pos = chunk.position();
        let (region_x, region_z) = region_coords_for_chunk(chunk_pos);

        let Some(plan) = plan_fortress_for_region(self.world_seed, region_x, region_z) else {
            return false;
        };

        if !plan.bounds.intersects_chunk(chunk_pos) {
            return false;
        }

        let corridor_block = crate::BLOCK_STONE_BRICKS;

        for i in 0..plan.length {
            for w in 0..plan.width {
                for h in 0..plan.height {
                    let (world_x, world_z) = match plan.axis {
                        FortressAxis::X => (plan.start_x + i as i32, plan.start_z + w as i32),
                        FortressAxis::Z => (plan.start_x + w as i32, plan.start_z + i as i32),
                    };
                    let world_y = plan.base_y + h as i32;

                    let is_floor = h == 0;
                    let is_ceiling = h + 1 == plan.height;
                    let is_wall = w == 0 || w + 1 == plan.width;

                    let id = if is_floor || is_ceiling || is_wall {
                        corridor_block
                    } else {
                        crate::BLOCK_AIR
                    };

                    set_world_voxel_if_in_chunk(
                        chunk,
                        world_x,
                        world_y,
                        world_z,
                        Voxel {
                            id,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // Place a chest near the center of the corridor (loot is filled at the game layer).
        if plan.length >= 7 && plan.width >= 3 && plan.height >= 3 {
            let i = plan.length / 2;
            let w = plan.width / 2;
            let (world_x, world_z) = match plan.axis {
                FortressAxis::X => (plan.start_x + i as i32, plan.start_z + w as i32),
                FortressAxis::Z => (plan.start_x + w as i32, plan.start_z + i as i32),
            };

            set_world_voxel_if_in_chunk(
                chunk,
                world_x,
                plan.base_y + 1,
                world_z,
                Voxel {
                    id: crate::interactive_blocks::CHEST,
                    ..Default::default()
                },
            );
        }

        true
    }
}

#[derive(Debug, Clone, Copy)]
enum FortressAxis {
    X,
    Z,
}

#[derive(Debug, Clone, Copy)]
struct FortressPlan {
    axis: FortressAxis,
    start_x: i32,
    start_z: i32,
    base_y: i32,
    length: usize,
    width: usize,
    height: usize,
    bounds: StructureBounds,
}

fn plan_fortress_for_region(world_seed: u64, region_x: i32, region_z: i32) -> Option<FortressPlan> {
    let seed = region_seed(world_seed, region_x, region_z, FORTRESS_SEED_SALT);
    let mut rng = StdRng::seed_from_u64(seed);

    // Keep fortresses fairly rare (about 1 in 4 regions).
    if !rng.gen_ratio(1, 4) {
        return None;
    }

    let (region_min_x, region_max_x, region_min_z, region_max_z) =
        region_world_bounds(region_x, region_z);

    let axis = if rng.gen_bool(0.5) {
        FortressAxis::X
    } else {
        FortressAxis::Z
    };

    let length = CORRIDOR_LENGTH;
    let width = CORRIDOR_WIDTH;
    let height = CORRIDOR_HEIGHT;

    let max_base_y = (CHUNK_SIZE_Y as i32 - 2 - height as i32).min(200);
    if max_base_y <= 40 {
        return None;
    }
    let base_y = rng.gen_range(40..=max_base_y);

    let (size_x, size_z) = match axis {
        FortressAxis::X => (length as i32, width as i32),
        FortressAxis::Z => (width as i32, length as i32),
    };

    let max_start_x = (region_max_x - region_min_x + 1) - size_x;
    let max_start_z = (region_max_z - region_min_z + 1) - size_z;
    if max_start_x <= 0 || max_start_z <= 0 {
        return None;
    }

    let start_x = region_min_x + rng.gen_range(0..=max_start_x);
    let start_z = region_min_z + rng.gen_range(0..=max_start_z);

    let bounds = StructureBounds {
        min_x: start_x,
        max_x: start_x + size_x - 1,
        min_y: base_y,
        max_y: base_y + height as i32 - 1,
        min_z: start_z,
        max_z: start_z + size_z - 1,
    };

    Some(FortressPlan {
        axis,
        start_x,
        start_z,
        base_y,
        length,
        width,
        height,
        bounds,
    })
}

/// Deterministic blaze spawns associated with fortress regions.
pub fn fortress_blaze_spawns_for_chunk(world_seed: u64, chunk_pos: ChunkPos) -> Vec<crate::Mob> {
    let (region_x, region_z) = region_coords_for_chunk(chunk_pos);
    let Some(plan) = plan_fortress_for_region(world_seed, region_x, region_z) else {
        return Vec::new();
    };
    if !plan.bounds.intersects_chunk(chunk_pos) {
        return Vec::new();
    }

    let mid_w = (plan.width / 2).max(1);
    let base_y = plan.base_y + 1;
    let mut spawns = Vec::new();

    let candidates = [8_usize, plan.length / 2, plan.length.saturating_sub(9)];

    for i in candidates {
        if plan.length < 3 {
            continue;
        }
        let i = i.clamp(1, plan.length - 2);
        let (world_x, world_z) = match plan.axis {
            FortressAxis::X => (plan.start_x + i as i32, plan.start_z + mid_w as i32),
            FortressAxis::Z => (plan.start_x + mid_w as i32, plan.start_z + i as i32),
        };

        if world_to_chunk_local(chunk_pos, world_x, world_z).is_none() {
            continue;
        }

        spawns.push(crate::Mob::new(
            world_x as f64 + 0.5,
            base_y as f64,
            world_z as f64 + 0.5,
            crate::MobType::Blaze,
        ));
    }

    spawns
}

/// Returns true if the world coordinate lies inside a generated Nether fortress.
pub fn nether_fortress_contains(world_seed: u64, world_x: i32, world_y: i32, world_z: i32) -> bool {
    let chunk_pos = ChunkPos::new(
        world_x.div_euclid(crate::chunk::CHUNK_SIZE_X as i32),
        world_z.div_euclid(crate::chunk::CHUNK_SIZE_Z as i32),
    );
    let (region_x, region_z) = region_coords_for_chunk(chunk_pos);
    let Some(plan) = plan_fortress_for_region(world_seed, region_x, region_z) else {
        return false;
    };

    (plan.bounds.min_x..=plan.bounds.max_x).contains(&world_x)
        && (plan.bounds.min_y..=plan.bounds.max_y).contains(&world_y)
        && (plan.bounds.min_z..=plan.bounds.max_z).contains(&world_z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{CHUNK_SIZE_X, CHUNK_SIZE_Z};

    #[test]
    fn fortress_generation_is_deterministic_for_chunk() {
        let generator = FortressGenerator::new(FORTRESS_SEED_SALT);

        let mut bounds = None;
        for region_x in -6..=6 {
            for region_z in -6..=6 {
                if let Some(found) =
                    plan_fortress_for_region(FORTRESS_SEED_SALT, region_x, region_z)
                        .map(|plan| plan.bounds)
                {
                    bounds = Some(found);
                    break;
                }
            }
            if bounds.is_some() {
                break;
            }
        }

        let bounds = bounds.expect("expected at least one fortress in search window");
        let chunk_pos = ChunkPos::new(
            bounds.min_x.div_euclid(CHUNK_SIZE_X as i32),
            bounds.min_z.div_euclid(CHUNK_SIZE_Z as i32),
        );

        let mut chunk_a = Chunk::new(chunk_pos);
        let mut chunk_b = Chunk::new(chunk_pos);

        assert!(generator.try_generate_fortress(&mut chunk_a));
        assert!(generator.try_generate_fortress(&mut chunk_b));

        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    assert_eq!(
                        chunk_a.voxel(x, y, z),
                        chunk_b.voxel(x, y, z),
                        "voxel mismatch at ({x}, {y}, {z})"
                    );
                }
            }
        }
    }
}
