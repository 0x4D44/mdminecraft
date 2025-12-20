use crate::biome::{BiomeAssigner, BiomeData};
use crate::chunk::{Chunk, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};
use crate::heightmap::Heightmap;
use crate::interaction::interactive_blocks;
use crate::structures::{region_coords_for_chunk, region_seed, region_world_bounds};
use crate::structures::{set_world_voxel_if_in_chunk, world_to_chunk_local, StructureBounds};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const VILLAGE_SEED_SALT: u64 = 0x56_49_4C_4C_41_47_45_u64; // "VILLAGE"
const VILLAGE_REGION_MODULUS: u64 = 5;

const HOUSE_SIZE: i32 = 7;
const HOUSE_HEIGHT: i32 = 5;
const WELL_SIZE: i32 = 5;
const FARM_SIZE_X: i32 = 7;
const FARM_SIZE_Z: i32 = 5;

/// Minimal deterministic village v0 generator.
///
/// Region-based placement that can cross chunk boundaries. This is intentionally
/// simple (static houses + well) and will evolve into a proper structure
/// pipeline later in Stage 6.
#[derive(Debug, Clone, Copy)]
pub struct VillageGenerator {
    world_seed: u64,
}

impl VillageGenerator {
    pub const fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    /// Attempt to generate the region's village into this chunk.
    ///
    /// Returns `true` if any part of the village intersects this chunk.
    pub fn try_generate_village(&self, chunk: &mut Chunk, biome_assigner: &BiomeAssigner) -> bool {
        let chunk_pos = chunk.position();
        let (region_x, region_z) = region_coords_for_chunk(chunk_pos);

        let Some(plan) =
            plan_village_for_region(self.world_seed, region_x, region_z, biome_assigner)
        else {
            return false;
        };

        if !plan.bounds.intersects_chunk(chunk_pos) {
            return false;
        }

        render_village(chunk, &plan);
        true
    }
}

#[derive(Debug, Clone)]
struct VillagePlan {
    bounds: StructureBounds,
    base_y: i32,
    well_origin_x: i32,
    well_origin_z: i32,
    farm_origin_x: i32,
    farm_origin_z: i32,
    house_a_origin_x: i32,
    house_a_origin_z: i32,
    house_b_origin_x: i32,
    house_b_origin_z: i32,
    rng_seed: u64,
}

pub(crate) fn village_bounds_for_region(
    world_seed: u64,
    region_x: i32,
    region_z: i32,
    biome_assigner: &BiomeAssigner,
) -> Option<StructureBounds> {
    plan_village_for_region(world_seed, region_x, region_z, biome_assigner).map(|plan| plan.bounds)
}

fn plan_village_for_region(
    world_seed: u64,
    region_x: i32,
    region_z: i32,
    biome_assigner: &BiomeAssigner,
) -> Option<VillagePlan> {
    let seed = region_seed(world_seed, region_x, region_z, VILLAGE_SEED_SALT);

    // Villages should be relatively uncommon.
    if !seed.is_multiple_of(VILLAGE_REGION_MODULUS) {
        return None;
    }

    let mut rng = StdRng::seed_from_u64(seed);

    let (region_min_x, region_max_x, region_min_z, region_max_z) =
        region_world_bounds(region_x, region_z);

    // Keep the village away from region edges so the small layout fits.
    let margin = 24;
    if region_max_x - region_min_x <= margin * 2 || region_max_z - region_min_z <= margin * 2 {
        return None;
    }

    let center_x = rng.gen_range((region_min_x + margin)..=(region_max_x - margin));
    let center_z = rng.gen_range((region_min_z + margin)..=(region_max_z - margin));

    let base_y = village_base_y(world_seed, center_x, center_z, biome_assigner);

    let well_origin_x = center_x - WELL_SIZE / 2;
    let well_origin_z = center_z - WELL_SIZE / 2;

    // Small farm patch north of the well.
    let farm_origin_x = center_x - FARM_SIZE_X / 2;
    let farm_origin_z = well_origin_z - FARM_SIZE_Z - 3;

    let house_spacing = 12;
    let house_a_origin_x = center_x - (HOUSE_SIZE / 2) - house_spacing;
    let house_a_origin_z = center_z - HOUSE_SIZE / 2;

    let house_b_origin_x = center_x + (HOUSE_SIZE / 2) + (house_spacing - HOUSE_SIZE);
    let house_b_origin_z = center_z - HOUSE_SIZE / 2;

    // Paths extend 1 block beyond house doors and farm south edge.
    let path_south_z = house_a_origin_z + HOUSE_SIZE;
    let farm_path_south_z = farm_origin_z + FARM_SIZE_Z;

    let min_x = house_a_origin_x
        .min(house_b_origin_x)
        .min(well_origin_x)
        .min(farm_origin_x);
    let min_z = house_a_origin_z
        .min(house_b_origin_z)
        .min(well_origin_z)
        .min(farm_origin_z);
    let max_x = (house_a_origin_x + HOUSE_SIZE - 1)
        .max(house_b_origin_x + HOUSE_SIZE - 1)
        .max(well_origin_x + WELL_SIZE - 1)
        .max(farm_origin_x + FARM_SIZE_X - 1);
    let max_z = (house_a_origin_z + HOUSE_SIZE - 1)
        .max(house_b_origin_z + HOUSE_SIZE - 1)
        .max(well_origin_z + WELL_SIZE - 1)
        .max(farm_origin_z + FARM_SIZE_Z - 1)
        .max(path_south_z)
        .max(farm_path_south_z);

    let max_y = base_y + HOUSE_HEIGHT + 2;
    if base_y < 1 || max_y >= CHUNK_SIZE_Y as i32 {
        return None;
    }

    Some(VillagePlan {
        bounds: StructureBounds {
            min_x,
            max_x,
            min_y: base_y - 2,
            max_y,
            min_z,
            max_z,
        },
        base_y,
        well_origin_x,
        well_origin_z,
        farm_origin_x,
        farm_origin_z,
        house_a_origin_x,
        house_a_origin_z,
        house_b_origin_x,
        house_b_origin_z,
        rng_seed: seed ^ 0xA11C_E0DE_1234_5678,
    })
}

fn village_base_y(
    world_seed: u64,
    world_x: i32,
    world_z: i32,
    biome_assigner: &BiomeAssigner,
) -> i32 {
    let biome = biome_assigner.get_biome(world_x, world_z);
    let biome_data = BiomeData::get(biome);

    let chunk_x = world_x.div_euclid(CHUNK_SIZE_X as i32);
    let chunk_z = world_z.div_euclid(CHUNK_SIZE_Z as i32);
    let local_x = world_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let local_z = world_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

    let heightmap = Heightmap::generate(world_seed, chunk_x, chunk_z);
    let base_height = heightmap.get(local_x, local_z);
    let target_height = (base_height as f32 + biome_data.height_modifier * 20.0) as i32;

    // Keep villages near-ish to typical surface levels in this build.
    let surface_y = target_height.clamp(8, 96);
    (surface_y + 1).clamp(8, CHUNK_SIZE_Y as i32 - (HOUSE_HEIGHT + 6))
}

fn render_village(chunk: &mut Chunk, plan: &VillagePlan) {
    let mut rng = StdRng::seed_from_u64(plan.rng_seed);

    render_well(chunk, plan.base_y, plan.well_origin_x, plan.well_origin_z);
    render_farm(
        chunk,
        &mut rng,
        plan.base_y,
        plan.farm_origin_x,
        plan.farm_origin_z,
    );

    render_house(
        chunk,
        &mut rng,
        plan.base_y,
        plan.house_a_origin_x,
        plan.house_a_origin_z,
    );
    render_house(
        chunk,
        &mut rng,
        plan.base_y,
        plan.house_b_origin_x,
        plan.house_b_origin_z,
    );

    // Gravel paths between the houses, well, and farm.
    let path_y = plan.base_y - 1;
    let house_a_center_x = plan.house_a_origin_x + HOUSE_SIZE / 2;
    let house_b_center_x = plan.house_b_origin_x + HOUSE_SIZE / 2;
    let house_path_z = plan.house_a_origin_z + HOUSE_SIZE;

    for x in house_a_center_x.min(house_b_center_x)..=house_a_center_x.max(house_b_center_x) {
        place_path_block(chunk, x, path_y, house_path_z);
    }

    let well_center_x = plan.well_origin_x + WELL_SIZE / 2;
    let well_south_z = plan.well_origin_z + WELL_SIZE;
    for z in well_south_z.min(house_path_z)..=well_south_z.max(house_path_z) {
        place_path_block(chunk, well_center_x, path_y, z);
    }

    let well_north_z = plan.well_origin_z - 1;
    let farm_south_z = plan.farm_origin_z + FARM_SIZE_Z;
    for z in farm_south_z.min(well_north_z)..=farm_south_z.max(well_north_z) {
        place_path_block(chunk, well_center_x, path_y, z);
    }
}

fn place_path_block(chunk: &mut Chunk, world_x: i32, world_y: i32, world_z: i32) {
    // Add support blocks under the path so it doesn't float over small depressions.
    fill_foundation_rect(
        chunk,
        world_y - 1,
        world_x,
        world_z,
        1,
        1,
        crate::BLOCK_DIRT,
    );

    set_world_voxel_if_in_chunk(
        chunk,
        world_x,
        world_y,
        world_z,
        Voxel {
            id: crate::BLOCK_GRAVEL,
            ..Default::default()
        },
    );

    // Clear 2 blocks above so path isn't buried by hills.
    for dy in 1..=2 {
        set_world_voxel_if_in_chunk(chunk, world_x, world_y + dy, world_z, Voxel::default());
    }
}

fn render_farm(chunk: &mut Chunk, rng: &mut StdRng, base_y: i32, origin_x: i32, origin_z: i32) {
    let farmland_y = base_y - 1;

    // Fill dirt foundations below the farmland layer.
    fill_foundation_rect(
        chunk,
        farmland_y - 1,
        origin_x,
        origin_z,
        FARM_SIZE_X,
        FARM_SIZE_Z,
        crate::BLOCK_DIRT,
    );

    // Clear the crop volume (2 blocks tall) so hills don't bury farms.
    for dy in 1..=2 {
        let world_y = farmland_y + dy;
        for dz in 0..FARM_SIZE_Z {
            for dx in 0..FARM_SIZE_X {
                set_world_voxel_if_in_chunk(
                    chunk,
                    origin_x + dx,
                    world_y,
                    origin_z + dz,
                    Voxel::default(),
                );
            }
        }
    }

    let water_x = origin_x + FARM_SIZE_X / 2;
    let water_z = origin_z + FARM_SIZE_Z / 2;

    for dz in 0..FARM_SIZE_Z {
        for dx in 0..FARM_SIZE_X {
            let world_x = origin_x + dx;
            let world_z = origin_z + dz;

            if world_x == water_x && world_z == water_z {
                set_world_voxel_if_in_chunk(
                    chunk,
                    world_x,
                    farmland_y,
                    world_z,
                    Voxel {
                        id: crate::BLOCK_WATER,
                        ..Default::default()
                    },
                );
                continue;
            }

            set_world_voxel_if_in_chunk(
                chunk,
                world_x,
                farmland_y,
                world_z,
                Voxel {
                    id: crate::farming_blocks::FARMLAND_WET,
                    ..Default::default()
                },
            );

            let crop = match rng.gen_range(0..3) {
                0 => crate::CropType::Wheat,
                1 => crate::CropType::Carrots,
                _ => crate::CropType::Potatoes,
            };
            let stage = rng.gen_range(0..=crop.max_stage());

            set_world_voxel_if_in_chunk(
                chunk,
                world_x,
                farmland_y + 1,
                world_z,
                Voxel {
                    id: crop.block_id_at_stage(stage),
                    ..Default::default()
                },
            );
        }
    }
}

fn render_well(chunk: &mut Chunk, base_y: i32, origin_x: i32, origin_z: i32) {
    fill_foundation_rect(
        chunk,
        base_y - 1,
        origin_x,
        origin_z,
        WELL_SIZE,
        WELL_SIZE,
        crate::BLOCK_COBBLESTONE,
    );

    // Clear interior volume above the water so terrain doesn't intersect the well.
    for dy in 1..=3 {
        let world_y = base_y + dy;
        for dz in 1..(WELL_SIZE - 1) {
            for dx in 1..(WELL_SIZE - 1) {
                set_world_voxel_if_in_chunk(
                    chunk,
                    origin_x + dx,
                    world_y,
                    origin_z + dz,
                    Voxel::default(),
                );
            }
        }
    }

    // Floor + walls.
    for dz in 0..WELL_SIZE {
        for dx in 0..WELL_SIZE {
            let x = origin_x + dx;
            let z = origin_z + dz;

            set_world_voxel_if_in_chunk(
                chunk,
                x,
                base_y - 1,
                z,
                Voxel {
                    id: crate::BLOCK_COBBLESTONE,
                    ..Default::default()
                },
            );

            let is_wall = dx == 0 || dz == 0 || dx + 1 == WELL_SIZE || dz + 1 == WELL_SIZE;
            if is_wall {
                set_world_voxel_if_in_chunk(
                    chunk,
                    x,
                    base_y,
                    z,
                    Voxel {
                        id: crate::BLOCK_COBBLESTONE,
                        ..Default::default()
                    },
                );
            } else {
                set_world_voxel_if_in_chunk(
                    chunk,
                    x,
                    base_y,
                    z,
                    Voxel {
                        id: crate::BLOCK_WATER,
                        ..Default::default()
                    },
                );
            }
        }
    }

    // Corner posts + simple roof.
    for (dx, dz) in [
        (0, 0),
        (0, WELL_SIZE - 1),
        (WELL_SIZE - 1, 0),
        (WELL_SIZE - 1, WELL_SIZE - 1),
    ] {
        let x = origin_x + dx;
        let z = origin_z + dz;
        for dy in 1..=3 {
            set_world_voxel_if_in_chunk(
                chunk,
                x,
                base_y + dy,
                z,
                Voxel {
                    id: crate::BLOCK_OAK_LOG,
                    ..Default::default()
                },
            );
        }
    }

    for dz in -1..=(WELL_SIZE) {
        for dx in -1..=(WELL_SIZE) {
            let x = origin_x + dx;
            let z = origin_z + dz;
            set_world_voxel_if_in_chunk(
                chunk,
                x,
                base_y + 4,
                z,
                Voxel {
                    id: crate::BLOCK_OAK_PLANKS,
                    ..Default::default()
                },
            );
        }
    }
}

fn render_house(chunk: &mut Chunk, rng: &mut StdRng, base_y: i32, origin_x: i32, origin_z: i32) {
    let floor_y = base_y;
    let wall_min_y = base_y + 1;
    let wall_max_y = base_y + 3;
    let roof_y = base_y + 4;

    fill_foundation_rect(
        chunk,
        floor_y - 1,
        origin_x,
        origin_z,
        HOUSE_SIZE,
        HOUSE_SIZE,
        crate::BLOCK_COBBLESTONE,
    );

    // Foundation + floor.
    for dz in 0..HOUSE_SIZE {
        for dx in 0..HOUSE_SIZE {
            let x = origin_x + dx;
            let z = origin_z + dz;

            set_world_voxel_if_in_chunk(
                chunk,
                x,
                floor_y - 1,
                z,
                Voxel {
                    id: crate::BLOCK_COBBLESTONE,
                    ..Default::default()
                },
            );
            set_world_voxel_if_in_chunk(
                chunk,
                x,
                floor_y,
                z,
                Voxel {
                    id: crate::BLOCK_OAK_PLANKS,
                    ..Default::default()
                },
            );
        }
    }

    // Clear interior volume so terrain doesn't intersect the house.
    for y in wall_min_y..roof_y {
        for dz in 1..(HOUSE_SIZE - 1) {
            for dx in 1..(HOUSE_SIZE - 1) {
                set_world_voxel_if_in_chunk(
                    chunk,
                    origin_x + dx,
                    y,
                    origin_z + dz,
                    Voxel::default(),
                );
            }
        }
    }

    // Walls.
    for y in wall_min_y..=wall_max_y {
        for dz in 0..HOUSE_SIZE {
            for dx in 0..HOUSE_SIZE {
                let is_boundary =
                    dx == 0 || dz == 0 || dx + 1 == HOUSE_SIZE || dz + 1 == HOUSE_SIZE;
                if !is_boundary {
                    continue;
                }

                let x = origin_x + dx;
                let z = origin_z + dz;

                // Door opening on the south wall at center.
                let is_door_column = dz == HOUSE_SIZE - 1 && dx == HOUSE_SIZE / 2;
                if is_door_column && y <= wall_min_y + 1 {
                    set_world_voxel_if_in_chunk(chunk, x, y, z, Voxel::default());
                    continue;
                }

                let is_corner =
                    (dx == 0 || dx + 1 == HOUSE_SIZE) && (dz == 0 || dz + 1 == HOUSE_SIZE);
                let id = if is_corner {
                    crate::BLOCK_OAK_LOG
                } else {
                    crate::BLOCK_OAK_PLANKS
                };

                // Sparse windows.
                let is_window_col = dx == 2 || dx == HOUSE_SIZE - 3;
                let is_window_row = dz == 2 || dz == HOUSE_SIZE - 3;
                let is_window = y == wall_min_y + 1
                    && (((dz == 0 || dz == HOUSE_SIZE - 1) && is_window_col)
                        || ((dx == 0 || dx == HOUSE_SIZE - 1) && is_window_row));

                let id = if is_window && rng.gen_ratio(3, 4) {
                    crate::BLOCK_GLASS
                } else {
                    id
                };

                set_world_voxel_if_in_chunk(
                    chunk,
                    x,
                    y,
                    z,
                    Voxel {
                        id,
                        ..Default::default()
                    },
                );
            }
        }
    }

    // Roof.
    for dz in 0..HOUSE_SIZE {
        for dx in 0..HOUSE_SIZE {
            let x = origin_x + dx;
            let z = origin_z + dz;
            set_world_voxel_if_in_chunk(
                chunk,
                x,
                roof_y,
                z,
                Voxel {
                    id: crate::BLOCK_OAK_PLANKS,
                    ..Default::default()
                },
            );
        }
    }

    // Interior: crafting table + chest (loot filled at the game layer).
    set_world_voxel_if_in_chunk(
        chunk,
        origin_x + 1,
        floor_y + 1,
        origin_z + 1,
        Voxel {
            id: crate::BLOCK_CRAFTING_TABLE,
            ..Default::default()
        },
    );

    set_world_voxel_if_in_chunk(
        chunk,
        origin_x + (HOUSE_SIZE - 2),
        floor_y + 1,
        origin_z + 1,
        Voxel {
            id: interactive_blocks::CHEST,
            ..Default::default()
        },
    );

    // Door + a little light.
    let door_x = origin_x + HOUSE_SIZE / 2;
    let door_z = origin_z + HOUSE_SIZE - 1;
    let door_state = crate::Facing::South.to_state();

    set_world_voxel_if_in_chunk(
        chunk,
        door_x,
        wall_min_y,
        door_z,
        Voxel {
            id: interactive_blocks::OAK_DOOR_LOWER,
            state: door_state,
            ..Default::default()
        },
    );
    set_world_voxel_if_in_chunk(
        chunk,
        door_x,
        wall_min_y + 1,
        door_z,
        Voxel {
            id: interactive_blocks::OAK_DOOR_UPPER,
            state: door_state,
            ..Default::default()
        },
    );

    set_world_voxel_if_in_chunk(
        chunk,
        origin_x + HOUSE_SIZE / 2,
        floor_y + 1,
        origin_z + HOUSE_SIZE / 2,
        Voxel {
            id: interactive_blocks::TORCH,
            ..Default::default()
        },
    );
}

fn fill_foundation_rect(
    chunk: &mut Chunk,
    floor_y: i32,
    origin_x: i32,
    origin_z: i32,
    size_x: i32,
    size_z: i32,
    id: u16,
) {
    if floor_y <= 0 {
        return;
    }

    let chunk_pos = chunk.position();
    for dz in 0..size_z {
        for dx in 0..size_x {
            let world_x = origin_x + dx;
            let world_z = origin_z + dz;
            let Some((local_x, local_z)) = world_to_chunk_local(chunk_pos, world_x, world_z) else {
                continue;
            };

            let scan_start_y = (floor_y - 1).min(CHUNK_SIZE_Y as i32 - 1);
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

            for y in (ground_y + 1)..=floor_y {
                set_world_voxel_if_in_chunk(
                    chunk,
                    world_x,
                    y,
                    world_z,
                    Voxel {
                        id,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkPos;
    use crate::structures::world_to_chunk_local;

    #[test]
    fn village_generation_is_deterministic_for_chunk() {
        let biome_assigner = BiomeAssigner::new(VILLAGE_SEED_SALT);
        let generator = VillageGenerator::new(VILLAGE_SEED_SALT);
        let bounds = village_bounds_for_region(VILLAGE_SEED_SALT, 0, 0, &biome_assigner)
            .expect("missing village");

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

        assert!(generator.try_generate_village(&mut chunk_a, &biome_assigner));
        assert!(generator.try_generate_village(&mut chunk_b, &biome_assigner));

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
