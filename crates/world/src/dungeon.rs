use crate::chunk::{Chunk, Voxel, CHUNK_SIZE_Y};
use crate::interaction::interactive_blocks;
use crate::structures::{region_coords_for_chunk, region_seed, region_world_bounds};
use crate::structures::{set_world_voxel_if_in_chunk, StructureBounds};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const DUNGEON_SEED_SALT: u64 = 0x44_55_4E_47_45_4F_4E_u64; // "DUNGEON"

const ROOM_MIN_SIZE: usize = 5;
const ROOM_MAX_SIZE: usize = 7;
const ROOM_HEIGHT: usize = 5;

/// Minimal deterministic dungeon generator.
///
/// Region-based placement that can cross chunk boundaries. This is still a
/// simplified first pass toward Stage 6's full structure pipeline.
#[derive(Debug, Clone, Copy)]
pub struct DungeonGenerator {
    world_seed: u64,
}

impl DungeonGenerator {
    pub const fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    /// Attempt to generate the region's dungeon room into this chunk.
    ///
    /// Returns `true` if any part of the dungeon intersects this chunk.
    pub fn try_generate_dungeon(&self, chunk: &mut Chunk) -> bool {
        let chunk_pos = chunk.position();

        let (region_x, region_z) = region_coords_for_chunk(chunk_pos);
        let Some(bounds) = dungeon_bounds_for_region(self.world_seed, region_x, region_z) else {
            return false;
        };

        if !bounds.intersects_chunk(chunk_pos) {
            return false;
        }

        let (region_min_x, region_max_x, region_min_z, region_max_z) =
            region_world_bounds(region_x, region_z);

        let seed = region_seed(self.world_seed, region_x, region_z, DUNGEON_SEED_SALT);
        let mut rng = StdRng::seed_from_u64(seed);

        let room_size_x = rng.gen_range(ROOM_MIN_SIZE..=ROOM_MAX_SIZE);
        let room_size_z = rng.gen_range(ROOM_MIN_SIZE..=ROOM_MAX_SIZE);

        let max_start_x = (region_max_x - region_min_x + 1) - room_size_x as i32;
        let max_start_z = (region_max_z - region_min_z + 1) - room_size_z as i32;
        if max_start_x <= 0 || max_start_z <= 0 {
            return false;
        }

        let start_x = region_min_x + rng.gen_range(0..=max_start_x);
        let start_z = region_min_z + rng.gen_range(0..=max_start_z);

        // Keep dungeons reasonably underground (surface is typically ~64 in this build).
        let max_base_y = (CHUNK_SIZE_Y as i32 - 1 - ROOM_HEIGHT as i32).min(60);
        if max_base_y <= 12 {
            return false;
        }
        let base_y = rng.gen_range(12..=max_base_y);

        // Carve out an enclosed room. This intentionally overwrites existing terrain.
        for dy in 0..ROOM_HEIGHT {
            let world_y = base_y + dy as i32;
            for dz in 0..room_size_z {
                let world_z = start_z + dz as i32;
                for dx in 0..room_size_x {
                    let world_x = start_x + dx as i32;

                    let is_boundary = dx == 0
                        || dz == 0
                        || dy == 0
                        || dx + 1 == room_size_x
                        || dz + 1 == room_size_z
                        || dy + 1 == ROOM_HEIGHT;

                    let id = if is_boundary {
                        // Sprinkle mossy patches on walls/floor for texture variety.
                        let mossy = rng.gen_ratio(1, 6);
                        if mossy {
                            crate::BLOCK_MOSS_BLOCK
                        } else {
                            crate::BLOCK_COBBLESTONE
                        }
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

        // Place a chest inside the room (loot is filled at the game layer).
        if room_size_x >= 3 && room_size_z >= 3 {
            let chest_x = start_x + rng.gen_range(1..(room_size_x - 1)) as i32;
            let chest_z = start_z + rng.gen_range(1..(room_size_z - 1)) as i32;
            let chest_y = base_y + 1;
            set_world_voxel_if_in_chunk(
                chunk,
                chest_x,
                chest_y,
                chest_z,
                Voxel {
                    id: interactive_blocks::CHEST,
                    ..Default::default()
                },
            );
        }

        true
    }
}

pub(crate) fn dungeon_bounds_for_region(
    world_seed: u64,
    region_x: i32,
    region_z: i32,
) -> Option<StructureBounds> {
    let seed = region_seed(world_seed, region_x, region_z, DUNGEON_SEED_SALT);
    let mut rng = StdRng::seed_from_u64(seed);

    let room_size_x = rng.gen_range(ROOM_MIN_SIZE..=ROOM_MAX_SIZE);
    let room_size_z = rng.gen_range(ROOM_MIN_SIZE..=ROOM_MAX_SIZE);

    let (region_min_x, region_max_x, region_min_z, region_max_z) =
        region_world_bounds(region_x, region_z);

    let max_start_x = (region_max_x - region_min_x + 1) - room_size_x as i32;
    let max_start_z = (region_max_z - region_min_z + 1) - room_size_z as i32;
    if max_start_x <= 0 || max_start_z <= 0 {
        return None;
    }

    let start_x = region_min_x + rng.gen_range(0..=max_start_x);
    let start_z = region_min_z + rng.gen_range(0..=max_start_z);

    let max_base_y = (CHUNK_SIZE_Y as i32 - 1 - ROOM_HEIGHT as i32).min(60);
    if max_base_y <= 12 {
        return None;
    }
    let base_y = rng.gen_range(12..=max_base_y);

    Some(StructureBounds {
        min_x: start_x,
        max_x: start_x + room_size_x as i32 - 1,
        min_y: base_y,
        max_y: base_y + ROOM_HEIGHT as i32 - 1,
        min_z: start_z,
        max_z: start_z + room_size_z as i32 - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Z};
    use crate::structures::world_to_chunk_local;

    #[test]
    fn dungeon_generation_is_deterministic_for_chunk() {
        let generator = DungeonGenerator::new(DUNGEON_SEED_SALT);
        let bounds = dungeon_bounds_for_region(DUNGEON_SEED_SALT, 0, 0).expect("missing dungeon");
        let chunk_pos = ChunkPos::new(
            bounds.min_x.div_euclid(CHUNK_SIZE_X as i32),
            bounds.min_z.div_euclid(CHUNK_SIZE_Z as i32),
        );

        // Sanity: the chosen chunk should contain at least one corner of the bbox.
        assert!(
            world_to_chunk_local(chunk_pos, bounds.min_x, bounds.min_z).is_some(),
            "expected bounds corner to be inside chosen chunk"
        );

        let mut chunk_a = Chunk::new(chunk_pos);
        let mut chunk_b = Chunk::new(chunk_pos);

        assert!(generator.try_generate_dungeon(&mut chunk_a));
        assert!(generator.try_generate_dungeon(&mut chunk_b));

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
