use crate::chunk::{Chunk, Voxel, CHUNK_SIZE_Y};
use crate::interaction::interactive_blocks;
use crate::structures::{region_coords_for_chunk, region_seed, region_world_bounds};
use crate::structures::{set_world_voxel_if_in_chunk, StructureBounds};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const MINESHAFT_SEED_SALT: u64 = 0x4D_49_4E_45_u64; // "MINE"

const CORRIDOR_HEIGHT: usize = 3;
const CORRIDOR_HALF_WIDTH: isize = 1;
const SUPPORT_SPACING: usize = 4;

/// Minimal deterministic mineshaft-lite generator.
///
/// Region-based placement that can cross chunk boundaries. This is intentionally
/// simple: a single corridor with wooden supports and a chest.
#[derive(Debug, Clone, Copy)]
pub struct MineshaftGenerator {
    world_seed: u64,
}

impl MineshaftGenerator {
    pub const fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    /// Attempt to generate a short mineshaft corridor in this chunk.
    ///
    /// Returns `true` if any part of the corridor intersects this chunk.
    pub fn try_generate_mineshaft(&self, chunk: &mut Chunk) -> bool {
        let chunk_pos = chunk.position();

        let (region_x, region_z) = region_coords_for_chunk(chunk_pos);
        let Some(bounds) = mineshaft_bounds_for_region(self.world_seed, region_x, region_z) else {
            return false;
        };

        if !bounds.intersects_chunk(chunk_pos) {
            return false;
        }

        let (region_min_x, region_max_x, region_min_z, region_max_z) =
            region_world_bounds(region_x, region_z);

        let seed = region_seed(self.world_seed, region_x, region_z, MINESHAFT_SEED_SALT);
        let mut rng = StdRng::seed_from_u64(seed);

        let floor_y = rng.gen_range(16..=48) as i32;
        if floor_y < 0 || floor_y + CORRIDOR_HEIGHT as i32 >= CHUNK_SIZE_Y as i32 {
            return false;
        }

        let along_x = rng.gen_bool(0.5);
        let length = rng.gen_range(24..=80) as i32;

        if along_x {
            let max_start_x = (region_max_x - region_min_x + 1) - length;
            if max_start_x <= 0 {
                return false;
            }

            let start_x = region_min_x + rng.gen_range(0..=max_start_x);
            let center_z = rng.gen_range((region_min_z + 1)..=(region_max_z - 1));
            carve_x_corridor(chunk, &mut rng, floor_y, start_x, length, center_z);
        } else {
            let max_start_z = (region_max_z - region_min_z + 1) - length;
            if max_start_z <= 0 {
                return false;
            }

            let start_z = region_min_z + rng.gen_range(0..=max_start_z);
            let center_x = rng.gen_range((region_min_x + 1)..=(region_max_x - 1));
            carve_z_corridor(chunk, &mut rng, floor_y, center_x, start_z, length);
        }

        true
    }
}

pub(crate) fn mineshaft_bounds_for_region(
    world_seed: u64,
    region_x: i32,
    region_z: i32,
) -> Option<StructureBounds> {
    let seed = region_seed(world_seed, region_x, region_z, MINESHAFT_SEED_SALT);
    let mut rng = StdRng::seed_from_u64(seed);

    let floor_y = rng.gen_range(16..=48) as i32;
    if floor_y < 0 || floor_y + CORRIDOR_HEIGHT as i32 >= CHUNK_SIZE_Y as i32 {
        return None;
    }

    let (region_min_x, region_max_x, region_min_z, region_max_z) =
        region_world_bounds(region_x, region_z);

    let along_x = rng.gen_bool(0.5);
    let length = rng.gen_range(24..=80) as i32;

    if along_x {
        let max_start_x = (region_max_x - region_min_x + 1) - length;
        if max_start_x <= 0 {
            return None;
        }
        let start_x = region_min_x + rng.gen_range(0..=max_start_x);
        let center_z = rng.gen_range((region_min_z + 1)..=(region_max_z - 1));

        Some(StructureBounds {
            min_x: start_x,
            max_x: start_x + length - 1,
            min_y: floor_y,
            max_y: floor_y + CORRIDOR_HEIGHT as i32,
            min_z: center_z - 1,
            max_z: center_z + 1,
        })
    } else {
        let max_start_z = (region_max_z - region_min_z + 1) - length;
        if max_start_z <= 0 {
            return None;
        }
        let start_z = region_min_z + rng.gen_range(0..=max_start_z);
        let center_x = rng.gen_range((region_min_x + 1)..=(region_max_x - 1));

        Some(StructureBounds {
            min_x: center_x - 1,
            max_x: center_x + 1,
            min_y: floor_y,
            max_y: floor_y + CORRIDOR_HEIGHT as i32,
            min_z: start_z,
            max_z: start_z + length - 1,
        })
    }
}

fn carve_x_corridor(
    chunk: &mut Chunk,
    rng: &mut StdRng,
    floor_y: i32,
    start_x: i32,
    length: i32,
    center_z: i32,
) {
    // Floor + air space.
    for offset_x in 0..length {
        let world_x = start_x + offset_x;
        for dz in -CORRIDOR_HALF_WIDTH..=CORRIDOR_HALF_WIDTH {
            let world_z = center_z + dz as i32;

            // Floor is planks.
            set_world_voxel_if_in_chunk(
                chunk,
                world_x,
                floor_y,
                world_z,
                Voxel {
                    id: crate::BLOCK_OAK_PLANKS,
                    ..Default::default()
                },
            );

            // Clear headroom.
            for dy in 1..=CORRIDOR_HEIGHT {
                set_world_voxel_if_in_chunk(
                    chunk,
                    world_x,
                    floor_y + dy as i32,
                    world_z,
                    Voxel::default(),
                );
            }
        }
    }

    // Supports.
    for offset_x in (0..(length as usize)).step_by(SUPPORT_SPACING) {
        let support_x = start_x + offset_x as i32;
        place_support_along_x(chunk, rng, support_x, floor_y, center_z);
    }

    // Chest (loot filled at the game layer).
    if length >= 3 {
        let chest_x = start_x + rng.gen_range(1..(length - 1));
        set_world_voxel_if_in_chunk(
            chunk,
            chest_x,
            floor_y + 1,
            center_z,
            Voxel {
                id: interactive_blocks::CHEST,
                ..Default::default()
            },
        );
    }
}

fn carve_z_corridor(
    chunk: &mut Chunk,
    rng: &mut StdRng,
    floor_y: i32,
    center_x: i32,
    start_z: i32,
    length: i32,
) {
    // Floor + air space.
    for offset_z in 0..length {
        let world_z = start_z + offset_z;
        for dx in -CORRIDOR_HALF_WIDTH..=CORRIDOR_HALF_WIDTH {
            let world_x = center_x + dx as i32;

            set_world_voxel_if_in_chunk(
                chunk,
                world_x,
                floor_y,
                world_z,
                Voxel {
                    id: crate::BLOCK_OAK_PLANKS,
                    ..Default::default()
                },
            );

            for dy in 1..=CORRIDOR_HEIGHT {
                set_world_voxel_if_in_chunk(
                    chunk,
                    world_x,
                    floor_y + dy as i32,
                    world_z,
                    Voxel::default(),
                );
            }
        }
    }

    // Supports.
    for offset_z in (0..(length as usize)).step_by(SUPPORT_SPACING) {
        let support_z = start_z + offset_z as i32;
        place_support_along_z(chunk, rng, center_x, floor_y, support_z);
    }

    // Chest.
    if length >= 3 {
        let chest_z = start_z + rng.gen_range(1..(length - 1));
        set_world_voxel_if_in_chunk(
            chunk,
            center_x,
            floor_y + 1,
            chest_z,
            Voxel {
                id: interactive_blocks::CHEST,
                ..Default::default()
            },
        );
    }
}

fn place_support_along_x(
    chunk: &mut Chunk,
    rng: &mut StdRng,
    world_x: i32,
    floor_y: i32,
    center_z: i32,
) {
    // Randomly omit some supports for variation.
    if rng.gen_ratio(1, 6) {
        return;
    }

    // Vertical posts (logs) on either side + a beam across the top.
    let z0 = center_z - 1;
    let z1 = center_z + 1;
    for dy in 1..=2 {
        set_world_voxel_if_in_chunk(
            chunk,
            world_x,
            floor_y + dy,
            z0,
            Voxel {
                id: crate::BLOCK_OAK_LOG,
                ..Default::default()
            },
        );
        set_world_voxel_if_in_chunk(
            chunk,
            world_x,
            floor_y + dy,
            z1,
            Voxel {
                id: crate::BLOCK_OAK_LOG,
                ..Default::default()
            },
        );
    }

    let beam_y = floor_y + 2;
    for dz in -CORRIDOR_HALF_WIDTH..=CORRIDOR_HALF_WIDTH {
        let world_z = center_z + dz as i32;
        set_world_voxel_if_in_chunk(
            chunk,
            world_x,
            beam_y,
            world_z,
            Voxel {
                id: crate::BLOCK_OAK_PLANKS,
                ..Default::default()
            },
        );
    }
}

fn place_support_along_z(
    chunk: &mut Chunk,
    rng: &mut StdRng,
    center_x: i32,
    floor_y: i32,
    world_z: i32,
) {
    if rng.gen_ratio(1, 6) {
        return;
    }

    let x0 = center_x - 1;
    let x1 = center_x + 1;
    for dy in 1..=2 {
        set_world_voxel_if_in_chunk(
            chunk,
            x0,
            floor_y + dy,
            world_z,
            Voxel {
                id: crate::BLOCK_OAK_LOG,
                ..Default::default()
            },
        );
        set_world_voxel_if_in_chunk(
            chunk,
            x1,
            floor_y + dy,
            world_z,
            Voxel {
                id: crate::BLOCK_OAK_LOG,
                ..Default::default()
            },
        );
    }

    let beam_y = floor_y + 2;
    for dx in -CORRIDOR_HALF_WIDTH..=CORRIDOR_HALF_WIDTH {
        let world_x = center_x + dx as i32;
        set_world_voxel_if_in_chunk(
            chunk,
            world_x,
            beam_y,
            world_z,
            Voxel {
                id: crate::BLOCK_OAK_PLANKS,
                ..Default::default()
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Z};
    use crate::structures::world_to_chunk_local;

    #[test]
    fn mineshaft_generation_is_deterministic_for_chunk() {
        let generator = MineshaftGenerator::new(MINESHAFT_SEED_SALT);
        let bounds =
            mineshaft_bounds_for_region(MINESHAFT_SEED_SALT, 0, 0).expect("missing mineshaft");
        let chunk_pos = ChunkPos::new(
            bounds.min_x.div_euclid(CHUNK_SIZE_X as i32),
            bounds.min_z.div_euclid(CHUNK_SIZE_Z as i32),
        );

        assert!(
            world_to_chunk_local(chunk_pos, bounds.min_x, bounds.min_z).is_some(),
            "expected bounds corner to be inside chosen chunk"
        );

        let mut chunk_a = Chunk::new(chunk_pos);
        let mut chunk_b = Chunk::new(chunk_pos);

        assert!(generator.try_generate_mineshaft(&mut chunk_a));
        assert!(generator.try_generate_mineshaft(&mut chunk_b));

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
