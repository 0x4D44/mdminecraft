use crate::biome::BiomeAssigner;
use crate::chunk::{
    world_y_to_local_y, Chunk, ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Z, WORLD_MAX_Y,
    WORLD_MIN_Y,
};

pub(crate) const STRUCTURE_REGION_SIZE_CHUNKS: i32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldgenStructureKind {
    Dungeon,
    Ruin,
    Mineshaft,
    Village,
}

pub fn worldgen_structure_kind_at(
    world_seed: u64,
    world_x: i32,
    world_y: i32,
    world_z: i32,
) -> Option<WorldgenStructureKind> {
    let biome_assigner = BiomeAssigner::new(world_seed);
    worldgen_structure_kind_at_with_biome_assigner(
        world_seed,
        world_x,
        world_y,
        world_z,
        &biome_assigner,
    )
}

pub(crate) fn worldgen_structure_kind_at_with_biome_assigner(
    world_seed: u64,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    biome_assigner: &BiomeAssigner,
) -> Option<WorldgenStructureKind> {
    let chunk_pos = ChunkPos::new(
        world_x.div_euclid(CHUNK_SIZE_X as i32),
        world_z.div_euclid(CHUNK_SIZE_Z as i32),
    );
    let (region_x, region_z) = region_coords_for_chunk(chunk_pos);

    if let Some(bounds) =
        crate::village::village_bounds_for_region(world_seed, region_x, region_z, biome_assigner)
    {
        if bounds_contains(bounds, world_x, world_y, world_z) {
            return Some(WorldgenStructureKind::Village);
        }
    }

    if let Some(bounds) = crate::dungeon::dungeon_bounds_for_region(world_seed, region_x, region_z)
    {
        if bounds_contains(bounds, world_x, world_y, world_z) {
            return Some(WorldgenStructureKind::Dungeon);
        }
    }

    if let Some(bounds) =
        crate::mineshaft::mineshaft_bounds_for_region(world_seed, region_x, region_z)
    {
        if bounds_contains(bounds, world_x, world_y, world_z) {
            return Some(WorldgenStructureKind::Mineshaft);
        }
    }

    if let Some(bounds) =
        crate::ruin::ruin_bounds_for_region(world_seed, region_x, region_z, biome_assigner)
    {
        if bounds_contains(bounds, world_x, world_y, world_z) {
            return Some(WorldgenStructureKind::Ruin);
        }
    }

    None
}

pub(crate) fn region_coords_for_chunk(chunk_pos: ChunkPos) -> (i32, i32) {
    (
        div_floor_i32(chunk_pos.x, STRUCTURE_REGION_SIZE_CHUNKS),
        div_floor_i32(chunk_pos.z, STRUCTURE_REGION_SIZE_CHUNKS),
    )
}

pub(crate) fn region_world_bounds(region_x: i32, region_z: i32) -> (i32, i32, i32, i32) {
    let origin_chunk_x = region_x * STRUCTURE_REGION_SIZE_CHUNKS;
    let origin_chunk_z = region_z * STRUCTURE_REGION_SIZE_CHUNKS;

    let min_x = origin_chunk_x * CHUNK_SIZE_X as i32;
    let min_z = origin_chunk_z * CHUNK_SIZE_Z as i32;
    let max_x = min_x + (STRUCTURE_REGION_SIZE_CHUNKS * CHUNK_SIZE_X as i32) - 1;
    let max_z = min_z + (STRUCTURE_REGION_SIZE_CHUNKS * CHUNK_SIZE_Z as i32) - 1;

    (min_x, max_x, min_z, max_z)
}

pub(crate) fn chunk_world_bounds(chunk_pos: ChunkPos) -> (i32, i32, i32, i32) {
    let min_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let min_z = chunk_pos.z * CHUNK_SIZE_Z as i32;
    let max_x = min_x + CHUNK_SIZE_X as i32 - 1;
    let max_z = min_z + CHUNK_SIZE_Z as i32 - 1;

    (min_x, max_x, min_z, max_z)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StructureBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub min_z: i32,
    pub max_z: i32,
}

impl StructureBounds {
    pub(crate) fn intersects_chunk(self, chunk_pos: ChunkPos) -> bool {
        let (chunk_min_x, chunk_max_x, chunk_min_z, chunk_max_z) = chunk_world_bounds(chunk_pos);

        ranges_intersect(self.min_x, self.max_x, chunk_min_x, chunk_max_x)
            && ranges_intersect(self.min_z, self.max_z, chunk_min_z, chunk_max_z)
            && ranges_intersect(self.min_y, self.max_y, WORLD_MIN_Y, WORLD_MAX_Y)
    }

    pub(crate) fn intersects_bounds(self, other: StructureBounds) -> bool {
        ranges_intersect(self.min_x, self.max_x, other.min_x, other.max_x)
            && ranges_intersect(self.min_z, self.max_z, other.min_z, other.max_z)
            && ranges_intersect(self.min_y, self.max_y, other.min_y, other.max_y)
    }
}

pub(crate) fn region_seed(world_seed: u64, region_x: i32, region_z: i32, salt: u64) -> u64 {
    world_seed
        ^ (region_x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (region_z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ salt
}

pub(crate) fn set_world_voxel_if_in_chunk(
    chunk: &mut Chunk,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    voxel: Voxel,
) {
    let Some(local_y) = world_y_to_local_y(world_y) else {
        return;
    };

    let chunk_pos = chunk.position();
    let Some((local_x, local_z)) = world_to_chunk_local(chunk_pos, world_x, world_z) else {
        return;
    };

    chunk.set_voxel(local_x, local_y, local_z, voxel);
}

pub(crate) fn world_to_chunk_local(
    chunk_pos: ChunkPos,
    world_x: i32,
    world_z: i32,
) -> Option<(usize, usize)> {
    let (min_x, max_x, min_z, max_z) = chunk_world_bounds(chunk_pos);
    if world_x < min_x || world_x > max_x || world_z < min_z || world_z > max_z {
        return None;
    }

    let local_x = (world_x - min_x) as usize;
    let local_z = (world_z - min_z) as usize;

    if local_x >= CHUNK_SIZE_X || local_z >= CHUNK_SIZE_Z {
        return None;
    }

    Some((local_x, local_z))
}

fn div_floor_i32(value: i32, divisor: i32) -> i32 {
    debug_assert!(divisor > 0);
    let quot = value / divisor;
    let rem = value % divisor;
    if rem < 0 {
        quot - 1
    } else {
        quot
    }
}

fn ranges_intersect(a_min: i32, a_max: i32, b_min: i32, b_max: i32) -> bool {
    a_min <= b_max && b_min <= a_max
}

fn bounds_contains(bounds: StructureBounds, world_x: i32, world_y: i32, world_z: i32) -> bool {
    (bounds.min_x..=bounds.max_x).contains(&world_x)
        && (bounds.min_y..=bounds.max_y).contains(&world_y)
        && (bounds.min_z..=bounds.max_z).contains(&world_z)
}
