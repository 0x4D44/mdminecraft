use crate::biome::{BiomeAssigner, BiomeData};
use crate::chunk::{Chunk, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::heightmap::Heightmap;
use crate::interaction::interactive_blocks;
use crate::structures::{region_coords_for_chunk, region_seed, region_world_bounds};
use crate::structures::{set_world_voxel_if_in_chunk, world_to_chunk_local, StructureBounds};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const RUIN_SEED_SALT: u64 = 0x52_55_49_4E_u64; // "RUIN"

const RUIN_SIZE: usize = 5;
const RUIN_WALL_HEIGHT: usize = 3;

/// Minimal deterministic surface ruin generator.
///
/// Region-based placement that can cross chunk boundaries. This remains
/// intentionally simple, without advanced terrain adaptation.
#[derive(Debug, Clone, Copy)]
pub struct RuinGenerator {
    world_seed: u64,
}

impl RuinGenerator {
    pub const fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    /// Attempt to generate the region's ruin into this chunk.
    ///
    /// Returns `true` if any part of the ruin intersects this chunk.
    pub fn try_generate_ruin(&self, chunk: &mut Chunk, biome_assigner: &BiomeAssigner) -> bool {
        let chunk_pos = chunk.position();

        let (region_x, region_z) = region_coords_for_chunk(chunk_pos);
        let Some(bounds) =
            ruin_bounds_for_region(self.world_seed, region_x, region_z, biome_assigner)
        else {
            return false;
        };

        if !bounds.intersects_chunk(chunk_pos) {
            return false;
        }

        let (region_min_x, region_max_x, region_min_z, region_max_z) =
            region_world_bounds(region_x, region_z);
        let seed = region_seed(self.world_seed, region_x, region_z, RUIN_SEED_SALT);
        let mut rng = StdRng::seed_from_u64(seed);

        let max_start_x = (region_max_x - region_min_x + 1) - RUIN_SIZE as i32;
        let max_start_z = (region_max_z - region_min_z + 1) - RUIN_SIZE as i32;
        if max_start_x <= 0 || max_start_z <= 0 {
            return false;
        }

        let start_x = region_min_x + rng.gen_range(0..=max_start_x);
        let start_z = region_min_z + rng.gen_range(0..=max_start_z);

        let center_world_x = start_x + (RUIN_SIZE / 2) as i32;
        let center_world_z = start_z + (RUIN_SIZE / 2) as i32;

        let base_y = ruin_base_y(
            self.world_seed,
            center_world_x,
            center_world_z,
            biome_assigner,
        );

        fill_ruin_foundation(chunk, base_y, start_x, start_z);

        // Clear interior volume.
        for dy in 1..=RUIN_WALL_HEIGHT {
            let world_y = base_y + dy as i32;
            for dz in 1..(RUIN_SIZE - 1) {
                let world_z = start_z + dz as i32;
                for dx in 1..(RUIN_SIZE - 1) {
                    let world_x = start_x + dx as i32;
                    set_world_voxel_if_in_chunk(chunk, world_x, world_y, world_z, Voxel::default());
                }
            }
        }

        // Place floor.
        for dz in 0..RUIN_SIZE {
            let world_z = start_z + dz as i32;
            for dx in 0..RUIN_SIZE {
                let world_x = start_x + dx as i32;
                let id = if rng.gen_ratio(1, 5) {
                    crate::BLOCK_COBBLESTONE
                } else {
                    crate::BLOCK_STONE_BRICKS
                };
                set_world_voxel_if_in_chunk(
                    chunk,
                    world_x,
                    base_y,
                    world_z,
                    Voxel {
                        id,
                        ..Default::default()
                    },
                );
            }
        }

        // Place partial walls.
        for dy in 1..=RUIN_WALL_HEIGHT {
            let world_y = base_y + dy as i32;
            for dz in 0..RUIN_SIZE {
                let world_z = start_z + dz as i32;
                for dx in 0..RUIN_SIZE {
                    let world_x = start_x + dx as i32;
                    let is_boundary =
                        dx == 0 || dz == 0 || dx + 1 == RUIN_SIZE || dz + 1 == RUIN_SIZE;
                    if !is_boundary {
                        continue;
                    }

                    // Ruined look: randomly omit some wall blocks.
                    if rng.gen_ratio(1, 4) {
                        continue;
                    }

                    let id = if rng.gen_ratio(1, 6) {
                        crate::BLOCK_MOSS_BLOCK
                    } else {
                        crate::BLOCK_STONE_BRICKS
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

        // Place a chest inside the ruin (loot is filled at the game layer).
        let chest_x = start_x + (RUIN_SIZE / 2) as i32;
        let chest_z = start_z + (RUIN_SIZE / 2) as i32;
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

        true
    }
}

fn fill_ruin_foundation(chunk: &mut Chunk, base_y: i32, origin_x: i32, origin_z: i32) {
    if base_y <= 0 {
        return;
    }

    let chunk_pos = chunk.position();
    let scan_start_y = (base_y - 1).min(CHUNK_SIZE_Y as i32 - 1);

    for dz in 0..RUIN_SIZE {
        for dx in 0..RUIN_SIZE {
            let world_x = origin_x + dx as i32;
            let world_z = origin_z + dz as i32;

            let Some((local_x, local_z)) = world_to_chunk_local(chunk_pos, world_x, world_z) else {
                continue;
            };

            let mut ground_y = None;
            for y in (0..=scan_start_y).rev() {
                let voxel = chunk.voxel(local_x, y as usize, local_z);
                if voxel.id != crate::BLOCK_AIR && voxel.id != crate::BLOCK_WATER {
                    ground_y = Some(y);
                    break;
                }
            }

            let Some(ground_y) = ground_y else {
                continue;
            };

            for y in (ground_y + 1)..=scan_start_y {
                set_world_voxel_if_in_chunk(
                    chunk,
                    world_x,
                    y,
                    world_z,
                    Voxel {
                        id: crate::BLOCK_COBBLESTONE,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

fn ruin_base_y(world_seed: u64, world_x: i32, world_z: i32, biome_assigner: &BiomeAssigner) -> i32 {
    let biome = biome_assigner.get_biome(world_x, world_z);
    let biome_data = BiomeData::get(biome);

    let chunk_x = world_x.div_euclid(CHUNK_SIZE_X as i32);
    let chunk_z = world_z.div_euclid(CHUNK_SIZE_Z as i32);
    let local_x = world_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let local_z = world_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

    let heightmap = Heightmap::generate(world_seed, chunk_x, chunk_z);
    let base_height = heightmap.get(local_x, local_z);
    let target_height = (base_height as f32 + biome_data.height_modifier * 20.0) as i32;

    let max_surface_y = CHUNK_SIZE_Y as i32 - RUIN_WALL_HEIGHT as i32 - 2;
    let surface_y = target_height.clamp(1, max_surface_y.max(1));
    surface_y + 1
}

pub(crate) fn ruin_bounds_for_region(
    world_seed: u64,
    region_x: i32,
    region_z: i32,
    biome_assigner: &BiomeAssigner,
) -> Option<StructureBounds> {
    let seed = region_seed(world_seed, region_x, region_z, RUIN_SEED_SALT);
    let mut rng = StdRng::seed_from_u64(seed);

    let (region_min_x, region_max_x, region_min_z, region_max_z) =
        region_world_bounds(region_x, region_z);

    let max_start_x = (region_max_x - region_min_x + 1) - RUIN_SIZE as i32;
    let max_start_z = (region_max_z - region_min_z + 1) - RUIN_SIZE as i32;
    if max_start_x <= 0 || max_start_z <= 0 {
        return None;
    }

    let start_x = region_min_x + rng.gen_range(0..=max_start_x);
    let start_z = region_min_z + rng.gen_range(0..=max_start_z);

    let center_world_x = start_x + (RUIN_SIZE / 2) as i32;
    let center_world_z = start_z + (RUIN_SIZE / 2) as i32;
    let base_y = ruin_base_y(world_seed, center_world_x, center_world_z, biome_assigner);

    let bounds = StructureBounds {
        min_x: start_x,
        max_x: start_x + RUIN_SIZE as i32 - 1,
        min_y: base_y,
        max_y: base_y + RUIN_WALL_HEIGHT as i32,
        min_z: start_z,
        max_z: start_z + RUIN_SIZE as i32 - 1,
    };

    // Avoid surface ruins overlapping villages; villages are higher priority structures.
    if let Some(village_bounds) =
        crate::village::village_bounds_for_region(world_seed, region_x, region_z, biome_assigner)
    {
        if bounds.intersects_bounds(village_bounds) {
            return None;
        }
    }

    Some(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkPos;
    use crate::structures::world_to_chunk_local;

    fn solid_chunk(pos: ChunkPos) -> Chunk {
        let mut chunk = Chunk::new(pos);
        for y in 0..64 {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    chunk.set_voxel(
                        x,
                        y,
                        z,
                        Voxel {
                            id: crate::BLOCK_STONE,
                            ..Default::default()
                        },
                    );
                }
            }
        }
        chunk
    }

    #[test]
    fn ruin_generation_is_deterministic_for_chunk() {
        let biome_assigner = BiomeAssigner::new(RUIN_SEED_SALT);
        let generator = RuinGenerator::new(RUIN_SEED_SALT);
        let bounds =
            ruin_bounds_for_region(RUIN_SEED_SALT, 0, 0, &biome_assigner).expect("missing ruin");

        let chunk_pos = ChunkPos::new(
            bounds.min_x.div_euclid(CHUNK_SIZE_X as i32),
            bounds.min_z.div_euclid(CHUNK_SIZE_Z as i32),
        );

        assert!(
            world_to_chunk_local(chunk_pos, bounds.min_x, bounds.min_z).is_some(),
            "expected bounds corner to be inside chosen chunk"
        );

        let mut chunk_a = solid_chunk(chunk_pos);
        let mut chunk_b = solid_chunk(chunk_pos);

        assert!(generator.try_generate_ruin(&mut chunk_a, &biome_assigner));
        assert!(generator.try_generate_ruin(&mut chunk_b, &biome_assigner));

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
