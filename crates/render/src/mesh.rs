use blake3::Hasher;
use mdminecraft_assets::{BlockFace, BlockRegistry, TextureAtlasMetadata};
use mdminecraft_core::RegistryKey;
use mdminecraft_world::{
    block_supports_waterlogging, get_fluid_level, get_fluid_type, interactive_blocks, is_falling,
    is_fluid, is_waterlogged, local_y_to_world_y, world_y_to_local_y, BlockId, Chunk, FluidType,
    Voxel, BLOCK_AIR, BLOCK_SNOW, BLOCK_WATER, BLOCK_WATER_FLOWING, CHUNK_SIZE_X, CHUNK_SIZE_Y,
    CHUNK_SIZE_Z, FLUID_LEVEL_SOURCE,
};

const AXIS_SIZE: [usize; 3] = [CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z];
const EXTRA_ALPHA_BIT: u8 = 0x80;
const EXTRA_TINT_SHIFT: u8 = 4;
const EXTRA_TINT_MASK: u8 = 0x70;
const EXTRA_TINT_GRASS: u8 = 1 << EXTRA_TINT_SHIFT;
const EXTRA_TINT_FOLIAGE: u8 = 2 << EXTRA_TINT_SHIFT;
const EXTRA_TINT_GRASS_SIDE: u8 = 3 << EXTRA_TINT_SHIFT;
const EXTRA_TINT_FOLIAGE_BIRCH: u8 = 4 << EXTRA_TINT_SHIFT;
const EXTRA_TINT_FOLIAGE_SPRUCE: u8 = 5 << EXTRA_TINT_SHIFT;
const EXTRA_ALPHA_KIND_WATER: u8 = 1 << EXTRA_TINT_SHIFT;
const EXTRA_ALPHA_KIND_LAVA: u8 = 2 << EXTRA_TINT_SHIFT;
const EXTRA_ALPHA_KIND_GLASS: u8 = 3 << EXTRA_TINT_SHIFT;
const EXTRA_ALPHA_KIND_NETHER_PORTAL: u8 = 4 << EXTRA_TINT_SHIFT;
const EXTRA_ALPHA_KIND_END_PORTAL: u8 = 5 << EXTRA_TINT_SHIFT;
const EXTRA_ALPHA_KIND_FIRE: u8 = 6 << EXTRA_TINT_SHIFT;

/// Hash of the combined vertex/index buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshHash(pub [u8; 32]);

/// Packed vertex layout produced by the mesher.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    /// Position in chunk-local coordinates.
    pub position: [f32; 3],
    /// Face normal (unit length).
    pub normal: [f32; 3],
    /// Texture coordinates for atlas sampling.
    pub uv: [f32; 2],
    /// Block identifier baked into the face for material lookup.
    pub block_id: u16,
    /// Packed light + ambient occlusion.
    ///
    /// - Low 4 bits: combined light level (max of skylight and blocklight), range 0-15.
    /// - High 4 bits: ambient occlusion level, range 0-3 (3 = fully lit).
    pub light: u8,
    /// Extra per-vertex metadata (packed into the high 8 bits of the `packed_data` attribute).
    pub extra: u8,
}

/// Output mesh buffers per chunk.
#[derive(Debug, Clone)]
pub struct MeshBuffers {
    /// Vertex buffer used for draw submission.
    pub vertices: Vec<MeshVertex>,
    /// Index buffer (triangle list) for the opaque + cutout pass.
    pub indices_opaque: Vec<u32>,
    /// Index buffer (triangle list) for the alpha-blended pass.
    pub indices_alpha: Vec<u32>,
    /// Stable hash of the vertex + index buffers for cache comparisons.
    pub hash: MeshHash,
}

impl MeshBuffers {
    /// Construct an empty mesh (useful for initialization).
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices_opaque: Vec::new(),
            indices_alpha: Vec::new(),
            hash: MeshHash([0; 32]),
        }
    }
}

/// Generate greedy-meshed buffers for the given chunk.
pub fn mesh_chunk(
    chunk: &Chunk,
    registry: &BlockRegistry,
    atlas: Option<&TextureAtlasMetadata>,
) -> MeshBuffers {
    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    mesh_chunk_with_voxel_at(chunk, registry, atlas, |world_x, world_y, world_z| {
        let local_y = world_y_to_local_y(world_y)?;

        let local_x = world_x - origin_x;
        let local_z = world_z - origin_z;
        if !(0..CHUNK_SIZE_X as i32).contains(&local_x)
            || !(0..CHUNK_SIZE_Z as i32).contains(&local_z)
        {
            return None;
        }

        Some(chunk.voxel(local_x as usize, local_y, local_z as usize))
    })
}

/// Generate buffers for the given chunk, with access to a world-voxel sampler for neighbor-aware blocks.
pub fn mesh_chunk_with_voxel_at<F>(
    chunk: &Chunk,
    registry: &BlockRegistry,
    atlas: Option<&TextureAtlasMetadata>,
    voxel_at_world: F,
) -> MeshBuffers
where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let mut builder = MeshBuilder::new(registry, atlas);
    GreedyMesher::mesh(chunk, &mut builder);
    mesh_fluids(chunk, &mut builder, registry, &voxel_at_world);
    mesh_waterlogged_fluids(chunk, &mut builder, registry, &voxel_at_world);
    mesh_glass_panes(chunk, &mut builder, registry, &voxel_at_world);
    mesh_oak_fences(chunk, &mut builder, registry, &voxel_at_world);
    mesh_cobblestone_walls(chunk, &mut builder, registry, &voxel_at_world);
    mesh_oak_fence_gates(chunk, &mut builder);
    mesh_stairs(chunk, &mut builder, &voxel_at_world);
    mesh_slabs(chunk, &mut builder);
    mesh_snow_layers(chunk, &mut builder);
    mesh_trapdoors(chunk, &mut builder);
    mesh_doors(chunk, &mut builder);
    mesh_ladders(chunk, &mut builder);
    mesh_torches(chunk, &mut builder);
    mesh_redstone_wires(chunk, &mut builder, &voxel_at_world);
    mesh_redstone_repeaters(chunk, &mut builder);
    mesh_redstone_comparators(chunk, &mut builder);
    mesh_pressure_plates(chunk, &mut builder);
    mesh_buttons(chunk, &mut builder);
    mesh_levers(chunk, &mut builder);
    mesh_crops(chunk, &mut builder);
    mesh_cave_decorations(chunk, &mut builder);
    mesh_beds(chunk, &mut builder);
    mesh_chests(chunk, &mut builder);
    mesh_enchanting_tables(chunk, &mut builder);
    mesh_brewing_stands(chunk, &mut builder);
    mesh_farmland(chunk, &mut builder);
    mesh_nether_portals(chunk, &mut builder);
    mesh_end_portals(chunk, &mut builder);
    builder.finish()
}

fn mesh_fluids<F>(
    chunk: &Chunk,
    builder: &mut MeshBuilder,
    registry: &BlockRegistry,
    voxel_at_world: &F,
) where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    const HEIGHT_EPS: f32 = 0.001;

    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    let cache_layer_size = (CHUNK_SIZE_X + 2) * (CHUNK_SIZE_Z + 2);
    let mut cached_water_heights = vec![f32::NAN; cache_layer_size * CHUNK_SIZE_Y];
    let mut cached_lava_heights = vec![f32::NAN; cache_layer_size * CHUNK_SIZE_Y];

    let cache_index = |world_x: i32, local_y: usize, world_z: i32| -> Option<usize> {
        let local_x = world_x - origin_x + 1;
        let local_z = world_z - origin_z + 1;

        if !(0..(CHUNK_SIZE_X as i32 + 2)).contains(&local_x) {
            return None;
        }
        if !(0..(CHUNK_SIZE_Z as i32 + 2)).contains(&local_z) {
            return None;
        }

        let local_x = local_x as usize;
        let local_z = local_z as usize;
        Some(local_y * cache_layer_size + local_z * (CHUNK_SIZE_X + 2) + local_x)
    };

    let voxel_contains_fluid = |voxel: Voxel, fluid_type: FluidType| -> bool {
        get_fluid_type(voxel.id) == Some(fluid_type)
            || (fluid_type == FluidType::Water
                && block_supports_waterlogging(voxel.id)
                && is_waterlogged(voxel.state))
    };

    let mut base_fluid_height_at =
        |world_x: i32, local_y: usize, world_z: i32, fluid_type: FluidType| -> f32 {
            let cache = match fluid_type {
                FluidType::Water => &mut cached_water_heights,
                FluidType::Lava => &mut cached_lava_heights,
            };

            if let Some(idx) = cache_index(world_x, local_y, world_z) {
                if !cache[idx].is_nan() {
                    return cache[idx];
                }

                let world_y = local_y_to_world_y(local_y);
                let Some(voxel) = voxel_at_world(world_x, world_y, world_z) else {
                    cache[idx] = 0.0;
                    return 0.0;
                };

                let is_waterlogged_cell = fluid_type == FluidType::Water
                    && block_supports_waterlogging(voxel.id)
                    && is_waterlogged(voxel.state);
                if !is_waterlogged_cell && get_fluid_type(voxel.id) != Some(fluid_type) {
                    cache[idx] = 0.0;
                    return 0.0;
                }

                let above = voxel_at_world(world_x, world_y + 1, world_z);
                if above.is_some_and(|v| voxel_contains_fluid(v, fluid_type)) {
                    cache[idx] = 1.0;
                    return 1.0;
                }

                if is_waterlogged_cell || mdminecraft_world::is_source_fluid(voxel.id) {
                    cache[idx] = 1.0;
                    return 1.0;
                }

                if is_falling(voxel.state) {
                    cache[idx] = 1.0;
                    return 1.0;
                }

                let level = get_fluid_level(voxel.state) as f32;
                let shift =
                    FLUID_LEVEL_SOURCE.saturating_sub(fluid_type.max_flow_distance() + 1) as f32;
                let height = ((level + shift) / (FLUID_LEVEL_SOURCE as f32)).clamp(0.0, 1.0);

                cache[idx] = height;
                return height;
            }

            let world_y = local_y_to_world_y(local_y);
            let Some(voxel) = voxel_at_world(world_x, world_y, world_z) else {
                return 0.0;
            };

            let is_waterlogged_cell = fluid_type == FluidType::Water
                && block_supports_waterlogging(voxel.id)
                && is_waterlogged(voxel.state);
            if !is_waterlogged_cell && get_fluid_type(voxel.id) != Some(fluid_type) {
                return 0.0;
            }

            let above = voxel_at_world(world_x, world_y + 1, world_z);
            if above.is_some_and(|v| voxel_contains_fluid(v, fluid_type)) {
                return 1.0;
            }

            if is_waterlogged_cell || mdminecraft_world::is_source_fluid(voxel.id) {
                return 1.0;
            }

            if is_falling(voxel.state) {
                return 1.0;
            }

            let level = get_fluid_level(voxel.state) as f32;
            let shift =
                FLUID_LEVEL_SOURCE.saturating_sub(fluid_type.max_flow_distance() + 1) as f32;
            ((level + shift) / (FLUID_LEVEL_SOURCE as f32)).clamp(0.0, 1.0)
        };

    let encode_flow_dir = |dx: i8, dz: i8| -> u8 {
        match (dx, dz) {
            (0, 0) => 0,
            (0, -1) => 1,
            (0, 1) => 2,
            (-1, 0) => 3,
            (1, 0) => 4,
            (-1, -1) => 5,
            (1, -1) => 6,
            (-1, 1) => 7,
            (1, 1) => 8,
            _ => 0,
        }
    };

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                let Some(fluid_type) = get_fluid_type(voxel.id) else {
                    continue;
                };

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let light = voxel.light_sky.max(voxel.light_block);
                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let above_is_fluid = voxel_at_world(world_x, world_y + 1, world_z)
                    .is_some_and(|v| voxel_contains_fluid(v, fluid_type));
                let below = voxel_at_world(world_x, world_y - 1, world_z);
                let below_is_fluid = below.is_some_and(|v| voxel_contains_fluid(v, fluid_type));

                let corner_nw = fluid_corner_height(
                    &mut base_fluid_height_at,
                    world_x,
                    y,
                    world_z,
                    -1,
                    -1,
                    fluid_type,
                );
                let corner_sw = fluid_corner_height(
                    &mut base_fluid_height_at,
                    world_x,
                    y,
                    world_z,
                    -1,
                    1,
                    fluid_type,
                );
                let corner_se = fluid_corner_height(
                    &mut base_fluid_height_at,
                    world_x,
                    y,
                    world_z,
                    1,
                    1,
                    fluid_type,
                );
                let corner_ne = fluid_corner_height(
                    &mut base_fluid_height_at,
                    world_x,
                    y,
                    world_z,
                    1,
                    -1,
                    fluid_type,
                );

                let max_height = corner_nw.max(corner_sw).max(corner_se).max(corner_ne);
                if max_height <= HEIGHT_EPS {
                    continue;
                }

                let current_base = base_fluid_height_at(world_x, y, world_z, fluid_type);
                let neighbor_w = base_fluid_height_at(world_x - 1, y, world_z, fluid_type);
                let neighbor_e = base_fluid_height_at(world_x + 1, y, world_z, fluid_type);
                let neighbor_n = base_fluid_height_at(world_x, y, world_z - 1, fluid_type);
                let neighbor_s = base_fluid_height_at(world_x, y, world_z + 1, fluid_type);

                let mut flow_x = 0.0f32;
                let mut flow_z = 0.0f32;

                if neighbor_w + HEIGHT_EPS < current_base {
                    flow_x -= current_base - neighbor_w;
                }
                if neighbor_e + HEIGHT_EPS < current_base {
                    flow_x += current_base - neighbor_e;
                }
                if neighbor_n + HEIGHT_EPS < current_base {
                    flow_z -= current_base - neighbor_n;
                }
                if neighbor_s + HEIGHT_EPS < current_base {
                    flow_z += current_base - neighbor_s;
                }

                let abs_x = flow_x.abs();
                let abs_z = flow_z.abs();
                let mut dir_x = 0i8;
                let mut dir_z = 0i8;

                if abs_x > HEIGHT_EPS || abs_z > HEIGHT_EPS {
                    dir_x = if flow_x > 0.0 { 1 } else { -1 };
                    dir_z = if flow_z > 0.0 { 1 } else { -1 };

                    let dominant_ratio = 2.0;
                    if abs_x > abs_z * dominant_ratio {
                        dir_z = 0;
                    } else if abs_z > abs_x * dominant_ratio {
                        dir_x = 0;
                    }
                }

                let flow_code = encode_flow_dir(dir_x, dir_z);

                if !above_is_fluid {
                    builder.push_quad_with_extra(
                        voxel.id,
                        BlockFace::Up,
                        [0.0, 1.0, 0.0],
                        [
                            [base_x, base_y + corner_nw, base_z],
                            [base_x, base_y + corner_sw, base_z + 1.0],
                            [base_x + 1.0, base_y + corner_se, base_z + 1.0],
                            [base_x + 1.0, base_y + corner_ne, base_z],
                        ],
                        true,
                        light,
                        flow_code,
                    );
                }

                if !below_is_fluid {
                    let show_bottom = match below {
                        Some(v) => !is_solid(v) || !is_opaque(v, registry),
                        None => true,
                    };
                    if show_bottom {
                        builder.push_quad_with_extra(
                            voxel.id,
                            BlockFace::Down,
                            [0.0, -1.0, 0.0],
                            [
                                [base_x, base_y, base_z],
                                [base_x, base_y, base_z + 1.0],
                                [base_x + 1.0, base_y, base_z + 1.0],
                                [base_x + 1.0, base_y, base_z],
                            ],
                            false,
                            light,
                            flow_code,
                        );
                    }
                }

                let mut maybe_emit_side =
                    |dx: i32,
                     dz: i32,
                     neighbor_corner_a: f32,
                     neighbor_corner_b: f32,
                     face: BlockFace,
                     normal: [f32; 3],
                     corners: [[f32; 3]; 4],
                     normal_positive: bool| {
                        let neighbor = voxel_at_world(world_x + dx, world_y, world_z + dz);
                        if neighbor.is_some_and(|v| voxel_contains_fluid(v, fluid_type)) {
                            return;
                        }
                        if neighbor.is_some_and(|v| is_solid(v) && is_opaque(v, registry)) {
                            return;
                        }

                        let top_a = neighbor_corner_a;
                        let top_b = neighbor_corner_b;
                        if top_a <= HEIGHT_EPS && top_b <= HEIGHT_EPS {
                            return;
                        }

                        builder.push_quad_with_extra(
                            voxel.id,
                            face,
                            normal,
                            corners,
                            normal_positive,
                            light,
                            flow_code,
                        );
                    };

                // North (-Z).
                maybe_emit_side(
                    0,
                    -1,
                    corner_nw,
                    corner_ne,
                    BlockFace::North,
                    [0.0, 0.0, -1.0],
                    [
                        [base_x, base_y, base_z],
                        [base_x + 1.0, base_y, base_z],
                        [base_x + 1.0, base_y + corner_ne, base_z],
                        [base_x, base_y + corner_nw, base_z],
                    ],
                    false,
                );

                // South (+Z).
                maybe_emit_side(
                    0,
                    1,
                    corner_sw,
                    corner_se,
                    BlockFace::South,
                    [0.0, 0.0, 1.0],
                    [
                        [base_x, base_y, base_z + 1.0],
                        [base_x + 1.0, base_y, base_z + 1.0],
                        [base_x + 1.0, base_y + corner_se, base_z + 1.0],
                        [base_x, base_y + corner_sw, base_z + 1.0],
                    ],
                    true,
                );

                // West (-X).
                maybe_emit_side(
                    -1,
                    0,
                    corner_nw,
                    corner_sw,
                    BlockFace::West,
                    [-1.0, 0.0, 0.0],
                    [
                        [base_x, base_y, base_z],
                        [base_x, base_y + corner_nw, base_z],
                        [base_x, base_y + corner_sw, base_z + 1.0],
                        [base_x, base_y, base_z + 1.0],
                    ],
                    false,
                );

                // East (+X).
                maybe_emit_side(
                    1,
                    0,
                    corner_ne,
                    corner_se,
                    BlockFace::East,
                    [1.0, 0.0, 0.0],
                    [
                        [base_x + 1.0, base_y, base_z],
                        [base_x + 1.0, base_y + corner_ne, base_z],
                        [base_x + 1.0, base_y + corner_se, base_z + 1.0],
                        [base_x + 1.0, base_y, base_z + 1.0],
                    ],
                    true,
                );
            }
        }
    }
}

fn fluid_corner_height<F>(
    base_fluid_height_at: &mut F,
    world_x: i32,
    local_y: usize,
    world_z: i32,
    dx: i32,
    dz: i32,
    fluid_type: FluidType,
) -> f32
where
    F: FnMut(i32, usize, i32, FluidType) -> f32,
{
    let h00 = base_fluid_height_at(world_x, local_y, world_z, fluid_type);
    let h10 = base_fluid_height_at(world_x + dx, local_y, world_z, fluid_type);
    let h01 = base_fluid_height_at(world_x, local_y, world_z + dz, fluid_type);
    let h11 = base_fluid_height_at(world_x + dx, local_y, world_z + dz, fluid_type);
    h00.max(h10).max(h01).max(h11)
}

fn mesh_waterlogged_fluids<F>(
    chunk: &Chunk,
    builder: &mut MeshBuilder,
    registry: &BlockRegistry,
    voxel_at_world: &F,
) where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    let voxel_has_water = |voxel: Voxel| -> bool {
        voxel.id == BLOCK_WATER
            || voxel.id == BLOCK_WATER_FLOWING
            || (block_supports_waterlogging(voxel.id) && is_waterlogged(voxel.state))
    };

    let water_face_visible = |neighbor: Option<Voxel>| -> bool {
        let Some(neighbor) = neighbor else {
            return true;
        };
        if voxel_has_water(neighbor) {
            return false;
        }

        let neighbor_solid = is_solid(neighbor);
        if !neighbor_solid {
            return true;
        }

        let neighbor_opaque = is_opaque(neighbor, registry);
        !neighbor_opaque && neighbor.id != BLOCK_WATER && neighbor.id != BLOCK_WATER_FLOWING
    };

    let mut emit_water_box = |world_x: i32,
                              world_y: i32,
                              world_z: i32,
                              base_x: f32,
                              base_y: f32,
                              base_z: f32,
                              min: [f32; 3],
                              max: [f32; 3],
                              light: u8| {
        let mut faces = 0u8;

        if min[0] == base_x && water_face_visible(voxel_at_world(world_x - 1, world_y, world_z)) {
            faces |= FACE_WEST;
        }
        if max[0] == base_x + 1.0
            && water_face_visible(voxel_at_world(world_x + 1, world_y, world_z))
        {
            faces |= FACE_EAST;
        }

        if min[2] == base_z && water_face_visible(voxel_at_world(world_x, world_y, world_z - 1)) {
            faces |= FACE_NORTH;
        }
        if max[2] == base_z + 1.0
            && water_face_visible(voxel_at_world(world_x, world_y, world_z + 1))
        {
            faces |= FACE_SOUTH;
        }

        if min[1] == base_y && water_face_visible(voxel_at_world(world_x, world_y - 1, world_z)) {
            faces |= FACE_DOWN;
        }
        if max[1] == base_y + 1.0
            && water_face_visible(voxel_at_world(world_x, world_y + 1, world_z))
        {
            faces |= FACE_UP;
        }

        if faces != 0 {
            emit_box_masked(builder, BLOCK_WATER, min, max, light, faces);
        }
    };

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !block_supports_waterlogging(voxel.id) || !is_waterlogged(voxel.state) {
                    continue;
                }

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;
                let light = voxel.light_sky.max(voxel.light_block);

                if mdminecraft_world::is_slab(voxel.id) {
                    let top = matches!(
                        mdminecraft_world::SlabPosition::from_state(voxel.state),
                        mdminecraft_world::SlabPosition::Top
                    );
                    let (min_y, max_y) = if top { (0.0, 0.5) } else { (0.5, 1.0) };
                    emit_water_box(
                        world_x,
                        world_y,
                        world_z,
                        base_x,
                        base_y,
                        base_z,
                        [base_x, base_y + min_y, base_z],
                        [base_x + 1.0, base_y + max_y, base_z + 1.0],
                        light,
                    );
                } else if mdminecraft_world::is_stairs(voxel.id) {
                    let half = 0.5;
                    let facing = mdminecraft_world::Facing::from_state(voxel.state);
                    let top = (voxel.state & 0x04) != 0;
                    let shape = mdminecraft_world::stairs_shape_at(
                        world_x,
                        world_y,
                        world_z,
                        voxel,
                        voxel_at_world,
                    );
                    let (footprints, footprint_count) =
                        mdminecraft_world::stairs_step_footprints(facing, shape);

                    let (min_y, max_y) = if top { (0.0, half) } else { (half, 1.0) };

                    let quadrants = [
                        (0.0, half, 0.0, half),
                        (half, 1.0, 0.0, half),
                        (0.0, half, half, 1.0),
                        (half, 1.0, half, 1.0),
                    ];

                    for (q_min_x, q_max_x, q_min_z, q_max_z) in quadrants {
                        let mut occupied = false;
                        for footprint in footprints.iter().take(footprint_count) {
                            if footprint.min_x <= q_min_x
                                && footprint.max_x >= q_max_x
                                && footprint.min_z <= q_min_z
                                && footprint.max_z >= q_max_z
                            {
                                occupied = true;
                                break;
                            }
                        }

                        if occupied {
                            continue;
                        }

                        emit_water_box(
                            world_x,
                            world_y,
                            world_z,
                            base_x,
                            base_y,
                            base_z,
                            [base_x + q_min_x, base_y + min_y, base_z + q_min_z],
                            [base_x + q_max_x, base_y + max_y, base_z + q_max_z],
                            light,
                        );
                    }
                } else if mdminecraft_world::is_trapdoor(voxel.id) {
                    let thickness = 3.0 / 16.0;
                    let facing = mdminecraft_world::Facing::from_state(voxel.state);
                    let open = mdminecraft_world::is_trapdoor_open(voxel.state);
                    let top = mdminecraft_world::is_trapdoor_top(voxel.state);

                    let (min, max) = if open {
                        match facing {
                            mdminecraft_world::Facing::North => (
                                [base_x, base_y, base_z + thickness],
                                [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                            ),
                            mdminecraft_world::Facing::South => (
                                [base_x, base_y, base_z],
                                [base_x + 1.0, base_y + 1.0, base_z + 1.0 - thickness],
                            ),
                            mdminecraft_world::Facing::East => (
                                [base_x, base_y, base_z],
                                [base_x + 1.0 - thickness, base_y + 1.0, base_z + 1.0],
                            ),
                            mdminecraft_world::Facing::West => (
                                [base_x + thickness, base_y, base_z],
                                [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                            ),
                        }
                    } else if top {
                        (
                            [base_x, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0 - thickness, base_z + 1.0],
                        )
                    } else {
                        (
                            [base_x, base_y + thickness, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        )
                    };

                    emit_water_box(
                        world_x, world_y, world_z, base_x, base_y, base_z, min, max, light,
                    );
                } else {
                    emit_water_box(
                        world_x,
                        world_y,
                        world_z,
                        base_x,
                        base_y,
                        base_z,
                        [base_x, base_y, base_z],
                        [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        light,
                    );
                }
            }
        }
    }
}

fn render_translucent_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/translucent").expect("valid render translucent tag")
    })
}

fn render_alpha_glass_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/alpha/glass").expect("valid render alpha glass tag")
    })
}

fn render_alpha_portal_nether_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/alpha/portal/nether")
            .expect("valid render alpha nether portal tag")
    })
}

fn render_alpha_portal_end_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/alpha/portal/end")
            .expect("valid render alpha end portal tag")
    })
}

fn render_alpha_fire_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/alpha/fire").expect("valid render alpha fire tag")
    })
}

fn render_tint_grass_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/tint/grass").expect("valid render grass tint tag")
    })
}

fn render_tint_foliage_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/tint/foliage").expect("valid render foliage tint tag")
    })
}

fn render_tint_foliage_birch_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/tint/foliage/birch")
            .expect("valid render birch foliage tint tag")
    })
}

fn render_tint_foliage_spruce_tag() -> &'static RegistryKey {
    use std::sync::OnceLock;

    static TAG: OnceLock<RegistryKey> = OnceLock::new();
    TAG.get_or_init(|| {
        RegistryKey::parse("mdm:render/tint/foliage/spruce")
            .expect("valid render spruce foliage tint tag")
    })
}

fn is_alpha_blended(block_id: BlockId, registry: &BlockRegistry) -> bool {
    is_fluid(block_id) || registry.has_tag(block_id, render_translucent_tag())
}

fn alpha_kind_extra(block_id: BlockId, registry: &BlockRegistry) -> u8 {
    match get_fluid_type(block_id) {
        Some(FluidType::Water) => return EXTRA_ALPHA_KIND_WATER,
        Some(FluidType::Lava) => return EXTRA_ALPHA_KIND_LAVA,
        None => {}
    }

    if registry.has_tag(block_id, render_alpha_glass_tag()) {
        EXTRA_ALPHA_KIND_GLASS
    } else if registry.has_tag(block_id, render_alpha_fire_tag()) {
        EXTRA_ALPHA_KIND_FIRE
    } else if registry.has_tag(block_id, render_alpha_portal_nether_tag()) {
        EXTRA_ALPHA_KIND_NETHER_PORTAL
    } else if registry.has_tag(block_id, render_alpha_portal_end_tag()) {
        EXTRA_ALPHA_KIND_END_PORTAL
    } else {
        0
    }
}

/// Internal helper for building mesh data.
struct MeshBuilder<'a> {
    vertices: Vec<MeshVertex>,
    indices_opaque: Vec<u32>,
    indices_alpha: Vec<u32>,
    registry: &'a BlockRegistry,
    atlas: Option<&'a TextureAtlasMetadata>,
}

impl<'a> MeshBuilder<'a> {
    fn pack_light_and_ao(light: u8, ao: u8) -> u8 {
        let light = light.min(15) & 0x0F;
        let ao = ao.min(15) & 0x0F;
        (ao << 4) | light
    }

    fn new(registry: &'a BlockRegistry, atlas: Option<&'a TextureAtlasMetadata>) -> Self {
        Self {
            vertices: Vec::with_capacity(1024), // Pre-allocate to reduce reallocations
            indices_opaque: Vec::with_capacity(1024 * 6 / 4), // Indices are 1.5x vertices for quads
            indices_alpha: Vec::with_capacity(256),
            registry,
            atlas,
        }
    }

    fn push_quad(
        &mut self,
        block_id: BlockId,
        face: BlockFace,
        normal: [f32; 3],
        corners: [[f32; 3]; 4],
        normal_positive: bool,
        light: u8,
    ) {
        self.push_quad_with_extra(block_id, face, normal, corners, normal_positive, light, 0);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_quad_with_extra(
        &mut self,
        block_id: BlockId,
        face: BlockFace,
        normal: [f32; 3],
        corners: [[f32; 3]; 4],
        normal_positive: bool,
        light: u8,
        extra: u8,
    ) {
        self.push_quad_with_ao_and_extra(
            block_id,
            face,
            normal,
            corners,
            normal_positive,
            [light; 4],
            [3; 4],
            extra,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn push_quad_with_ao_and_extra(
        &mut self,
        block_id: BlockId,
        face: BlockFace,
        normal: [f32; 3],
        corners: [[f32; 3]; 4],
        normal_positive: bool,
        light: [u8; 4],
        ao: [u8; 4],
        extra: u8,
    ) {
        let base = self.vertices.len() as u32;

        let uvs = self.resolve_uvs(block_id, face);
        let alpha_blended = is_alpha_blended(block_id, self.registry);
        let tint_extra = if self.registry.has_tag(block_id, render_tint_grass_tag()) {
            match face {
                BlockFace::Up => EXTRA_TINT_GRASS,
                BlockFace::Down => 0,
                _ => EXTRA_TINT_GRASS_SIDE,
            }
        } else if self
            .registry
            .has_tag(block_id, render_tint_foliage_birch_tag())
        {
            EXTRA_TINT_FOLIAGE_BIRCH
        } else if self
            .registry
            .has_tag(block_id, render_tint_foliage_spruce_tag())
        {
            EXTRA_TINT_FOLIAGE_SPRUCE
        } else if self.registry.has_tag(block_id, render_tint_foliage_tag()) {
            EXTRA_TINT_FOLIAGE
        } else {
            0
        };
        let extra = if alpha_blended {
            (extra & !EXTRA_TINT_MASK) | alpha_kind_extra(block_id, self.registry)
        } else {
            (extra & !EXTRA_TINT_MASK) | tint_extra
        };
        let extra = if alpha_blended {
            extra | EXTRA_ALPHA_BIT
        } else {
            extra & !EXTRA_ALPHA_BIT
        };

        for (i, &corner) in corners.iter().enumerate() {
            self.vertices.push(MeshVertex {
                position: corner,
                normal,
                uv: uvs[i],
                block_id,
                light: Self::pack_light_and_ao(light[i], ao[i]),
                extra,
            });
        }
        let indices = if normal_positive {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        let indices_out = if alpha_blended {
            &mut self.indices_alpha
        } else {
            &mut self.indices_opaque
        };
        for idx in indices {
            indices_out.push(base + idx);
        }
    }

    fn finish(self) -> MeshBuffers {
        let MeshBuilder {
            vertices,
            indices_opaque,
            indices_alpha,
            ..
        } = self;
        let mut hasher = Hasher::new();
        hasher.update(bytemuck::cast_slice(&vertices));
        hasher.update(bytemuck::cast_slice(&indices_opaque));
        hasher.update(bytemuck::cast_slice(&indices_alpha));
        MeshBuffers {
            vertices,
            indices_opaque,
            indices_alpha,
            hash: MeshHash(*hasher.finalize().as_bytes()),
        }
    }

    fn resolve_uvs(&self, block_id: BlockId, face: BlockFace) -> [[f32; 2]; 4] {
        if let Some(atlas) = self.atlas {
            if let Some(desc) = self.registry.descriptor(block_id) {
                if let Some(entry) = atlas.entry(desc.texture_for(face)) {
                    return [
                        [entry.u0, entry.v0],
                        [entry.u1, entry.v0],
                        [entry.u1, entry.v1],
                        [entry.u0, entry.v1],
                    ];
                }
            }
        }

        let atlas_size = 16.0;
        let atlas_x = (block_id % 16) as f32;
        let atlas_y = (block_id / 16) as f32;
        [
            [atlas_x / atlas_size, atlas_y / atlas_size],
            [(atlas_x + 1.0) / atlas_size, atlas_y / atlas_size],
            [(atlas_x + 1.0) / atlas_size, (atlas_y + 1.0) / atlas_size],
            [atlas_x / atlas_size, (atlas_y + 1.0) / atlas_size],
        ]
    }
}

struct GreedyMesher;

impl GreedyMesher {
    pub fn mesh(chunk: &Chunk, builder: &mut MeshBuilder) {
        for axis in 0..3 {
            Self::mesh_axis(chunk, builder, axis);
        }
    }

    fn mesh_axis(chunk: &Chunk, builder: &mut MeshBuilder, axis: usize) {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let width = AXIS_SIZE[u_axis];
        let height = AXIS_SIZE[v_axis];
        let mut mask: Vec<Option<FaceDesc>> = vec![None; width * height];

        for slice in 0..=AXIS_SIZE[axis] {
            for j in 0..height {
                for i in 0..width {
                    let idx = j * width + i;
                    mask[idx] = Self::sample_face(chunk, builder.registry, axis, slice, i, j);
                }
            }

            let mut j = 0;
            while j < height {
                let mut i = 0;
                while i < width {
                    let idx = j * width + i;
                    if let Some(cell) = mask[idx] {
                        let mut quad_width = 1;
                        while i + quad_width < width
                            && mask[j * width + i + quad_width] == Some(cell)
                        {
                            quad_width += 1;
                        }

                        let mut quad_height = 1;
                        'scan: while j + quad_height < height {
                            for k in 0..quad_width {
                                if mask[(j + quad_height) * width + i + k] != Some(cell) {
                                    break 'scan;
                                }
                            }
                            quad_height += 1;
                        }

                        Self::emit_quad(
                            chunk,
                            builder,
                            axis,
                            slice,
                            (i, j),
                            (quad_width, quad_height),
                            cell,
                        );

                        for dy in 0..quad_height {
                            for dx in 0..quad_width {
                                mask[(j + dy) * width + i + dx] = None;
                            }
                        }
                        i += quad_width;
                    } else {
                        i += 1;
                    }
                }
                j += 1;
            }
        }
    }

    fn sample_face(
        chunk: &Chunk,
        registry: &BlockRegistry,
        axis: usize,
        slice: usize,
        u: usize,
        v: usize,
    ) -> Option<FaceDesc> {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;

        let front = if slice < AXIS_SIZE[axis] {
            let mut pos = [0usize; 3];
            pos[axis] = slice;
            pos[u_axis] = u;
            pos[v_axis] = v;
            Some(chunk.voxel(pos[0], pos[1], pos[2]))
        } else {
            None
        };

        let back = if slice == 0 {
            None
        } else {
            let mut pos = [0usize; 3];
            pos[axis] = slice - 1;
            pos[u_axis] = u;
            pos[v_axis] = v;
            Some(chunk.voxel(pos[0], pos[1], pos[2]))
        };

        // Determine which faces to render based on block types:
        // - Opaque blocks: render face when adjacent to non-opaque (air or transparent)
        // - Transparent blocks: render face when adjacent to air or different block type
        // - Air: never render
        match (front, back) {
            (Some(a), Some(b)) => {
                let a_opaque = is_opaque(a, registry);
                let b_opaque = is_opaque(b, registry);
                let a_solid = is_solid(a);
                let b_solid = is_solid(b);

                if a_opaque && !b_opaque {
                    // Opaque block 'a' facing non-opaque 'b' (air or transparent)
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, false, light))
                } else if b_opaque && !a_opaque {
                    // Opaque block 'b' facing non-opaque 'a' (air or transparent)
                    let light = b.light_sky.max(b.light_block);
                    Some(FaceDesc::new(b.id, axis, true, light))
                } else if a_solid && !a_opaque && !b_solid {
                    // Transparent block 'a' facing air
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, false, light))
                } else if b_solid && !b_opaque && !a_solid {
                    // Transparent block 'b' facing air
                    let light = b.light_sky.max(b.light_block);
                    Some(FaceDesc::new(b.id, axis, true, light))
                } else if a_solid && !a_opaque && b_solid && !b_opaque && a.id != b.id {
                    // Two different transparent blocks - render face for 'a'
                    let light = a.light_sky.max(a.light_block);
                    Some(FaceDesc::new(a.id, axis, false, light))
                } else {
                    None
                }
            }
            (Some(a), None) if is_solid(a) => {
                // Block at chunk edge facing outside
                let light = a.light_sky.max(a.light_block);
                Some(FaceDesc::new(a.id, axis, false, light))
            }
            (None, Some(b)) if is_solid(b) => {
                // Block at chunk edge facing outside
                let light = b.light_sky.max(b.light_block);
                Some(FaceDesc::new(b.id, axis, true, light))
            }
            _ => None,
        }
    }

    fn emit_quad(
        chunk: &Chunk,
        builder: &mut MeshBuilder,
        axis: usize,
        slice: usize,
        origin_uv: (usize, usize),
        quad_size: (usize, usize),
        cell: FaceDesc,
    ) {
        fn voxel_at(chunk: &Chunk, x: i32, y: i32, z: i32) -> Option<Voxel> {
            if x < 0
                || y < 0
                || z < 0
                || x >= CHUNK_SIZE_X as i32
                || y >= CHUNK_SIZE_Y as i32
                || z >= CHUNK_SIZE_Z as i32
            {
                return None;
            }
            Some(chunk.voxel(x as usize, y as usize, z as usize))
        }

        fn is_ao_occluder(chunk: &Chunk, registry: &BlockRegistry, x: i32, y: i32, z: i32) -> bool {
            voxel_at(chunk, x, y, z).is_some_and(|v| is_solid(v) && is_opaque(v, registry))
        }

        fn sample_light(chunk: &Chunk, x: i32, y: i32, z: i32, fallback: u8) -> u8 {
            voxel_at(chunk, x, y, z)
                .map(|v| v.light_sky.max(v.light_block))
                .unwrap_or(fallback)
        }

        #[allow(clippy::too_many_arguments)]
        fn ao_corner(
            chunk: &Chunk,
            registry: &BlockRegistry,
            pos: [i32; 3],
            axis: usize,
            normal: i32,
            u_axis: usize,
            u_sign: i32,
            v_axis: usize,
            v_sign: i32,
        ) -> u8 {
            let mut side_u = pos;
            side_u[axis] += normal;
            side_u[u_axis] += u_sign;
            let mut side_v = pos;
            side_v[axis] += normal;
            side_v[v_axis] += v_sign;
            let mut corner = pos;
            corner[axis] += normal;
            corner[u_axis] += u_sign;
            corner[v_axis] += v_sign;

            let side_u_occ = is_ao_occluder(chunk, registry, side_u[0], side_u[1], side_u[2]);
            let side_v_occ = is_ao_occluder(chunk, registry, side_v[0], side_v[1], side_v[2]);
            let corner_occ = is_ao_occluder(chunk, registry, corner[0], corner[1], corner[2]);

            if side_u_occ && side_v_occ {
                return 0;
            }

            let occ = side_u_occ as u8 + side_v_occ as u8 + corner_occ as u8;
            3u8.saturating_sub(occ)
        }

        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let (u, v) = origin_uv;
        let (quad_width, quad_height) = quad_size;

        let mut origin = [0f32; 3];
        origin[u_axis] = u as f32;
        origin[v_axis] = v as f32;
        origin[axis] = slice as f32;

        let mut du = [0f32; 3];
        du[u_axis] = quad_width as f32;
        let mut dv = [0f32; 3];
        dv[v_axis] = quad_height as f32;

        let v0 = origin;
        let v1 = add(origin, du);
        let v2 = add(add(origin, du), dv);
        let v3 = add(origin, dv);

        let normal = [
            cell.normal[0] as f32,
            cell.normal[1] as f32,
            cell.normal[2] as f32,
        ];

        let apply_ao = builder
            .registry
            .descriptor(cell.block_id)
            .is_some_and(|d| d.opaque)
            && !is_alpha_blended(cell.block_id, builder.registry);
        let (ao, light) = if apply_ao {
            let face_positive = cell.normal[axis] > 0;
            let normal_dir = if face_positive { 1 } else { -1 };
            let axis_pos = if face_positive {
                slice.saturating_sub(1) as i32
            } else {
                slice as i32
            };

            let corner_block_uv = [
                (u as i32, v as i32),
                ((u + quad_width - 1) as i32, v as i32),
                ((u + quad_width - 1) as i32, (v + quad_height - 1) as i32),
                (u as i32, (v + quad_height - 1) as i32),
            ];
            let corner_signs = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

            let mut ao_out = [3u8; 4];
            let mut light_out = [cell.light; 4];
            for (idx, ((cu, cv), (su, sv))) in
                corner_block_uv.into_iter().zip(corner_signs).enumerate()
            {
                let mut pos = [0i32; 3];
                pos[axis] = axis_pos;
                pos[u_axis] = cu;
                pos[v_axis] = cv;

                let mut adjacent = pos;
                adjacent[axis] += normal_dir;
                let mut side_u = adjacent;
                side_u[u_axis] += su;
                let mut side_v = adjacent;
                side_v[v_axis] += sv;
                let mut corner = adjacent;
                corner[u_axis] += su;
                corner[v_axis] += sv;

                ao_out[idx] = ao_corner(
                    chunk,
                    builder.registry,
                    pos,
                    axis,
                    normal_dir,
                    u_axis,
                    su,
                    v_axis,
                    sv,
                );

                let fallback = cell.light;
                let l0 = sample_light(chunk, adjacent[0], adjacent[1], adjacent[2], fallback);
                let l1 = sample_light(chunk, side_u[0], side_u[1], side_u[2], fallback);
                let l2 = sample_light(chunk, side_v[0], side_v[1], side_v[2], fallback);
                let l3 = sample_light(chunk, corner[0], corner[1], corner[2], fallback);
                let sum = l0 as u16 + l1 as u16 + l2 as u16 + l3 as u16;
                light_out[idx] = ((sum + 2) / 4) as u8;
            }
            (ao_out, light_out)
        } else {
            ([3u8; 4], [cell.light; 4])
        };

        builder.push_quad_with_ao_and_extra(
            cell.block_id,
            cell.face(),
            normal,
            [v0, v1, v2, v3],
            cell.normal[axis] > 0,
            light,
            ao,
            0,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FaceDesc {
    block_id: BlockId,
    normal: [i8; 3],
    light: u8,
}

impl FaceDesc {
    fn new(block_id: BlockId, axis: usize, positive: bool, light: u8) -> Self {
        let mut normal = [0i8; 3];
        normal[axis] = if positive { 1 } else { -1 };
        Self {
            block_id,
            normal,
            light,
        }
    }

    fn face(&self) -> BlockFace {
        match self.normal {
            [0, 1, 0] => BlockFace::Up,
            [0, -1, 0] => BlockFace::Down,
            [0, 0, 1] => BlockFace::South,
            [0, 0, -1] => BlockFace::North,
            [1, 0, 0] => BlockFace::East,
            [-1, 0, 0] => BlockFace::West,
            _ => BlockFace::Up,
        }
    }
}

fn mesh_glass_panes<F>(
    chunk: &Chunk,
    builder: &mut MeshBuilder,
    _registry: &BlockRegistry,
    voxel_at_world: &F,
) where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let thickness = 2.0 / 16.0;
    let half = thickness * 0.5;

    let post_min = 0.5 - half;
    let post_max = 0.5 + half;

    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != interactive_blocks::GLASS_PANE
                    && voxel.id != interactive_blocks::IRON_BARS
                {
                    continue;
                }

                let connects_to = |neighbor: Voxel| -> bool {
                    matches!(
                        neighbor.id,
                        interactive_blocks::GLASS_PANE | interactive_blocks::IRON_BARS
                    ) || mdminecraft_world::is_full_cube_block(neighbor.id)
                };

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let connect_west =
                    voxel_at_world(world_x - 1, world_y, world_z).is_some_and(connects_to);
                let connect_east =
                    voxel_at_world(world_x + 1, world_y, world_z).is_some_and(connects_to);
                let connect_north =
                    voxel_at_world(world_x, world_y, world_z - 1).is_some_and(connects_to);
                let connect_south =
                    voxel_at_world(world_x, world_y, world_z + 1).is_some_and(connects_to);

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                // Center post.
                emit_box(
                    builder,
                    voxel.id,
                    [base_x + post_min, base_y, base_z + post_min],
                    [base_x + post_max, base_y + 1.0, base_z + post_max],
                    light,
                );

                if connect_west {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x, base_y, base_z + post_min],
                        [base_x + 0.5, base_y + 1.0, base_z + post_max],
                        light,
                    );
                }
                if connect_east {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + 0.5, base_y, base_z + post_min],
                        [base_x + 1.0, base_y + 1.0, base_z + post_max],
                        light,
                    );
                }
                if connect_north {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + post_min, base_y, base_z],
                        [base_x + post_max, base_y + 1.0, base_z + 0.5],
                        light,
                    );
                }
                if connect_south {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + post_min, base_y, base_z + 0.5],
                        [base_x + post_max, base_y + 1.0, base_z + 1.0],
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_oak_fences<F>(
    chunk: &Chunk,
    builder: &mut MeshBuilder,
    _registry: &BlockRegistry,
    voxel_at_world: &F,
) where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let post_min = 6.0 / 16.0;
    let post_max = 10.0 / 16.0;

    let rail_thickness = 2.0 / 16.0;
    let rail_half = rail_thickness * 0.5;
    let rail_min_x = 0.5 - rail_half;
    let rail_max_x = 0.5 + rail_half;
    let rail_min_z = 0.5 - rail_half;
    let rail_max_z = 0.5 + rail_half;

    let lower_rail_min_y = 6.0 / 16.0;
    let lower_rail_max_y = 9.0 / 16.0;
    let upper_rail_min_y = 12.0 / 16.0;
    let upper_rail_max_y = 15.0 / 16.0;

    let connects_to = |voxel: Voxel| -> bool {
        voxel.id == interactive_blocks::OAK_FENCE
            || voxel.id == interactive_blocks::OAK_FENCE_GATE
            || mdminecraft_world::is_full_cube_block(voxel.id)
    };

    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != interactive_blocks::OAK_FENCE {
                    continue;
                }

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let connect_west =
                    voxel_at_world(world_x - 1, world_y, world_z).is_some_and(connects_to);
                let connect_east =
                    voxel_at_world(world_x + 1, world_y, world_z).is_some_and(connects_to);
                let connect_north =
                    voxel_at_world(world_x, world_y, world_z - 1).is_some_and(connects_to);
                let connect_south =
                    voxel_at_world(world_x, world_y, world_z + 1).is_some_and(connects_to);

                let light = voxel.light_sky.max(voxel.light_block);
                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                // Post (tall).
                emit_box(
                    builder,
                    voxel.id,
                    [base_x + post_min, base_y, base_z + post_min],
                    [base_x + post_max, base_y + 1.5, base_z + post_max],
                    light,
                );

                let rails = [
                    (lower_rail_min_y, lower_rail_max_y),
                    (upper_rail_min_y, upper_rail_max_y),
                ];

                for (rail_y0, rail_y1) in rails {
                    if connect_west {
                        emit_box(
                            builder,
                            voxel.id,
                            [base_x, base_y + rail_y0, base_z + rail_min_z],
                            [base_x + 0.5, base_y + rail_y1, base_z + rail_max_z],
                            light,
                        );
                    }
                    if connect_east {
                        emit_box(
                            builder,
                            voxel.id,
                            [base_x + 0.5, base_y + rail_y0, base_z + rail_min_z],
                            [base_x + 1.0, base_y + rail_y1, base_z + rail_max_z],
                            light,
                        );
                    }
                    if connect_north {
                        emit_box(
                            builder,
                            voxel.id,
                            [base_x + rail_min_x, base_y + rail_y0, base_z],
                            [base_x + rail_max_x, base_y + rail_y1, base_z + 0.5],
                            light,
                        );
                    }
                    if connect_south {
                        emit_box(
                            builder,
                            voxel.id,
                            [base_x + rail_min_x, base_y + rail_y0, base_z + 0.5],
                            [base_x + rail_max_x, base_y + rail_y1, base_z + 1.0],
                            light,
                        );
                    }
                }
            }
        }
    }
}

fn mesh_cobblestone_walls<F>(
    chunk: &Chunk,
    builder: &mut MeshBuilder,
    _registry: &BlockRegistry,
    voxel_at_world: &F,
) where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let thickness = 6.0 / 16.0;
    let half = thickness * 0.5;

    let post_min = 0.5 - half;
    let post_max = 0.5 + half;

    let arm_height = 1.0;

    let connects_to = |voxel: Voxel| -> bool {
        matches!(
            voxel.id,
            interactive_blocks::COBBLESTONE_WALL | interactive_blocks::STONE_BRICK_WALL
        ) || voxel.id == interactive_blocks::OAK_FENCE_GATE
            || mdminecraft_world::is_full_cube_block(voxel.id)
    };

    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !matches!(
                    voxel.id,
                    interactive_blocks::COBBLESTONE_WALL | interactive_blocks::STONE_BRICK_WALL
                ) {
                    continue;
                }

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let connect_west =
                    voxel_at_world(world_x - 1, world_y, world_z).is_some_and(connects_to);
                let connect_east =
                    voxel_at_world(world_x + 1, world_y, world_z).is_some_and(connects_to);
                let connect_north =
                    voxel_at_world(world_x, world_y, world_z - 1).is_some_and(connects_to);
                let connect_south =
                    voxel_at_world(world_x, world_y, world_z + 1).is_some_and(connects_to);

                let post_height = if connect_west || connect_east || connect_north || connect_south
                {
                    1.5
                } else {
                    1.0
                };

                let light = voxel.light_sky.max(voxel.light_block);
                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                // Center post.
                emit_box(
                    builder,
                    voxel.id,
                    [base_x + post_min, base_y, base_z + post_min],
                    [base_x + post_max, base_y + post_height, base_z + post_max],
                    light,
                );

                if connect_west {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x, base_y, base_z + post_min],
                        [base_x + 0.5, base_y + arm_height, base_z + post_max],
                        light,
                    );
                }
                if connect_east {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + 0.5, base_y, base_z + post_min],
                        [base_x + 1.0, base_y + arm_height, base_z + post_max],
                        light,
                    );
                }
                if connect_north {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + post_min, base_y, base_z],
                        [base_x + post_max, base_y + arm_height, base_z + 0.5],
                        light,
                    );
                }
                if connect_south {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + post_min, base_y, base_z + 0.5],
                        [base_x + post_max, base_y + arm_height, base_z + 1.0],
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_oak_fence_gates(chunk: &Chunk, builder: &mut MeshBuilder) {
    let thickness = 3.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != interactive_blocks::OAK_FENCE_GATE {
                    continue;
                }

                let facing = mdminecraft_world::Facing::from_state(voxel.state);
                let open = mdminecraft_world::is_fence_gate_open(voxel.state);
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let (min, max) = if open {
                    // Simplified hinge: gates always swing "left" from their facing direction.
                    match facing {
                        mdminecraft_world::Facing::North => (
                            [base_x, base_y, base_z],
                            [base_x + thickness, base_y + 1.5, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::South => (
                            [base_x + 1.0 - thickness, base_y, base_z],
                            [base_x + 1.0, base_y + 1.5, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::East => (
                            [base_x, base_y, base_z],
                            [base_x + 1.0, base_y + 1.5, base_z + thickness],
                        ),
                        mdminecraft_world::Facing::West => (
                            [base_x, base_y, base_z + 1.0 - thickness],
                            [base_x + 1.0, base_y + 1.5, base_z + 1.0],
                        ),
                    }
                } else {
                    let half = thickness * 0.5;
                    match facing {
                        mdminecraft_world::Facing::North | mdminecraft_world::Facing::South => (
                            [base_x, base_y, base_z + 0.5 - half],
                            [base_x + 1.0, base_y + 1.5, base_z + 0.5 + half],
                        ),
                        mdminecraft_world::Facing::East | mdminecraft_world::Facing::West => (
                            [base_x + 0.5 - half, base_y, base_z],
                            [base_x + 0.5 + half, base_y + 1.5, base_z + 1.0],
                        ),
                    }
                };

                emit_box(builder, voxel.id, min, max, light);
            }
        }
    }
}

fn mesh_stairs<F>(chunk: &Chunk, builder: &mut MeshBuilder, voxel_at_world: &F)
where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let half = 0.5;

    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_stairs(voxel.id) {
                    continue;
                }

                let facing = mdminecraft_world::Facing::from_state(voxel.state);
                let top = (voxel.state & 0x04) != 0;
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let shape = mdminecraft_world::stairs_shape_at(
                    world_x,
                    world_y,
                    world_z,
                    voxel,
                    voxel_at_world,
                );
                let (footprints, footprint_count) =
                    mdminecraft_world::stairs_step_footprints(facing, shape);

                if top {
                    // Upside-down stairs: upper half is a full slab.
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x, base_y + half, base_z],
                        [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        light,
                    );

                    // Lower half-height step, shaped via neighbor-aware corner resolution.
                    for footprint in footprints.iter().take(footprint_count) {
                        emit_box_masked(
                            builder,
                            voxel.id,
                            [base_x + footprint.min_x, base_y, base_z + footprint.min_z],
                            [
                                base_x + footprint.max_x,
                                base_y + half,
                                base_z + footprint.max_z,
                            ],
                            light,
                            FACES_ALL & !FACE_UP,
                        );
                    }
                } else {
                    // Normal stairs: lower half is a full slab.
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x, base_y, base_z],
                        [base_x + 1.0, base_y + half, base_z + 1.0],
                        light,
                    );

                    // Upper half-height step, shaped via neighbor-aware corner resolution.
                    for footprint in footprints.iter().take(footprint_count) {
                        emit_box_masked(
                            builder,
                            voxel.id,
                            [
                                base_x + footprint.min_x,
                                base_y + half,
                                base_z + footprint.min_z,
                            ],
                            [
                                base_x + footprint.max_x,
                                base_y + 1.0,
                                base_z + footprint.max_z,
                            ],
                            light,
                            FACES_ALL & !FACE_DOWN,
                        );
                    }
                }
            }
        }
    }
}

fn mesh_slabs(chunk: &Chunk, builder: &mut MeshBuilder) {
    let half = 0.5;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_slab(voxel.id) {
                    continue;
                }

                let top = matches!(
                    mdminecraft_world::SlabPosition::from_state(voxel.state),
                    mdminecraft_world::SlabPosition::Top
                );
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let (min_y, max_y) = if top { (half, 1.0) } else { (0.0, half) };
                emit_box(
                    builder,
                    voxel.id,
                    [base_x, base_y + min_y, base_z],
                    [base_x + 1.0, base_y + max_y, base_z + 1.0],
                    light,
                );
            }
        }
    }
}

fn mesh_snow_layers(chunk: &Chunk, builder: &mut MeshBuilder) {
    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != BLOCK_SNOW {
                    continue;
                }

                let layers = mdminecraft_world::snow_layers(voxel.state);
                let height = layers as f32 / mdminecraft_world::SNOW_LAYERS_MAX as f32;
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x, base_y, base_z],
                    [base_x + 1.0, base_y + height, base_z + 1.0],
                    light,
                );
            }
        }
    }
}

fn mesh_trapdoors(chunk: &Chunk, builder: &mut MeshBuilder) {
    let thickness = 3.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_trapdoor(voxel.id) {
                    continue;
                }

                let facing = mdminecraft_world::Facing::from_state(voxel.state);
                let open = mdminecraft_world::is_trapdoor_open(voxel.state);
                let top = mdminecraft_world::is_trapdoor_top(voxel.state);
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let (min, max) = if open {
                    match facing {
                        mdminecraft_world::Facing::North => (
                            [base_x, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + thickness],
                        ),
                        mdminecraft_world::Facing::South => (
                            [base_x, base_y, base_z + 1.0 - thickness],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::East => (
                            [base_x + 1.0 - thickness, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::West => (
                            [base_x, base_y, base_z],
                            [base_x + thickness, base_y + 1.0, base_z + 1.0],
                        ),
                    }
                } else if top {
                    (
                        [base_x, base_y + 1.0 - thickness, base_z],
                        [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                    )
                } else {
                    (
                        [base_x, base_y, base_z],
                        [base_x + 1.0, base_y + thickness, base_z + 1.0],
                    )
                };

                emit_box(builder, voxel.id, min, max, light);
            }
        }
    }
}

fn mesh_doors(chunk: &Chunk, builder: &mut MeshBuilder) {
    let thickness = 3.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_door(voxel.id) {
                    continue;
                }

                let facing = mdminecraft_world::Facing::from_state(voxel.state);
                let open = mdminecraft_world::is_door_open(voxel.state);
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let (min, max) = if open {
                    // Simplified hinge: doors always swing "left" from their facing direction.
                    match facing {
                        mdminecraft_world::Facing::North => (
                            [base_x, base_y, base_z],
                            [base_x + thickness, base_y + 1.0, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::South => (
                            [base_x + 1.0 - thickness, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::East => (
                            [base_x, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + thickness],
                        ),
                        mdminecraft_world::Facing::West => (
                            [base_x, base_y, base_z + 1.0 - thickness],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        ),
                    }
                } else {
                    match facing {
                        mdminecraft_world::Facing::North => (
                            [base_x, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + thickness],
                        ),
                        mdminecraft_world::Facing::South => (
                            [base_x, base_y, base_z + 1.0 - thickness],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::East => (
                            [base_x + 1.0 - thickness, base_y, base_z],
                            [base_x + 1.0, base_y + 1.0, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::West => (
                            [base_x, base_y, base_z],
                            [base_x + thickness, base_y + 1.0, base_z + 1.0],
                        ),
                    }
                };

                let mut faces = FACES_ALL;
                if mdminecraft_world::is_door_lower(voxel.id) && y + 1 < CHUNK_SIZE_Y {
                    let above = chunk.voxel(x, y + 1, z);
                    if mdminecraft_world::is_door(above.id) {
                        faces &= !FACE_UP;
                    }
                }
                if mdminecraft_world::is_door_upper(voxel.id) && y > 0 {
                    let below = chunk.voxel(x, y - 1, z);
                    if mdminecraft_world::is_door(below.id) {
                        faces &= !FACE_DOWN;
                    }
                }

                emit_box_masked(builder, voxel.id, min, max, light, faces);
            }
        }
    }
}

fn mesh_ladders(chunk: &Chunk, builder: &mut MeshBuilder) {
    let thickness = 1.0 / 16.0;
    let inset = 1.0 / 512.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_ladder(voxel.id) {
                    continue;
                }

                let facing = mdminecraft_world::Facing::from_state(voxel.state);
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let (min, max) = match facing {
                    mdminecraft_world::Facing::North => (
                        [base_x, base_y, base_z + inset],
                        [base_x + 1.0, base_y + 1.0, base_z + inset + thickness],
                    ),
                    mdminecraft_world::Facing::South => (
                        [base_x, base_y, base_z + 1.0 - inset - thickness],
                        [base_x + 1.0, base_y + 1.0, base_z + 1.0 - inset],
                    ),
                    mdminecraft_world::Facing::East => (
                        [base_x + 1.0 - inset - thickness, base_y, base_z],
                        [base_x + 1.0 - inset, base_y + 1.0, base_z + 1.0],
                    ),
                    mdminecraft_world::Facing::West => (
                        [base_x + inset, base_y, base_z],
                        [base_x + inset + thickness, base_y + 1.0, base_z + 1.0],
                    ),
                };

                emit_box(builder, voxel.id, min, max, light);
            }
        }
    }
}

fn mesh_torches(chunk: &Chunk, builder: &mut MeshBuilder) {
    let half_width = 1.0 / 16.0;
    let height = 10.0 / 16.0;
    let wall_offset = 5.0 / 16.0;
    let wall_base = 4.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != interactive_blocks::TORCH
                    && voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_TORCH
                {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                if mdminecraft_world::is_torch_wall(voxel.state) {
                    let facing = mdminecraft_world::torch_facing(voxel.state);
                    let (dx, dz) = facing.offset();
                    let center_x = base_x + 0.5 - dx as f32 * wall_offset;
                    let center_z = base_z + 0.5 - dz as f32 * wall_offset;
                    let min_y = base_y + wall_base;
                    emit_box(
                        builder,
                        voxel.id,
                        [center_x - half_width, min_y, center_z - half_width],
                        [center_x + half_width, min_y + height, center_z + half_width],
                        light,
                    );
                } else {
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + 0.5 - half_width, base_y, base_z + 0.5 - half_width],
                        [
                            base_x + 0.5 + half_width,
                            base_y + height,
                            base_z + 0.5 + half_width,
                        ],
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_crops(chunk: &Chunk, builder: &mut MeshBuilder) {
    let inv_sqrt2 = 0.70710677_f32;
    let min_height = 4.0 / 16.0;
    let max_height = 1.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                let Some((crop_type, stage)) = mdminecraft_world::CropType::from_block_id(voxel.id)
                else {
                    continue;
                };

                let max_stage = crop_type.max_stage() as f32;
                let t = if max_stage > 0.0 {
                    (stage as f32 / max_stage).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let height = min_height + t * (max_height - min_height);

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let x0 = base_x;
                let x1 = base_x + 1.0;
                let y0 = base_y;
                let y1 = base_y + height;
                let z0 = base_z;
                let z1 = base_z + 1.0;

                let planes = [
                    (
                        [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]],
                        [inv_sqrt2, 0.0, -inv_sqrt2],
                    ),
                    (
                        [[x0, y0, z1], [x1, y0, z0], [x1, y1, z0], [x0, y1, z1]],
                        [inv_sqrt2, 0.0, inv_sqrt2],
                    ),
                ];

                for (corners, normal) in planes {
                    builder.push_quad(voxel.id, BlockFace::North, normal, corners, true, light);
                    builder.push_quad(
                        voxel.id,
                        BlockFace::North,
                        [-normal[0], -normal[1], -normal[2]],
                        corners,
                        false,
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_redstone_wires<F>(chunk: &Chunk, builder: &mut MeshBuilder, voxel_at_world: &F)
where
    F: Fn(i32, i32, i32) -> Option<Voxel>,
{
    let thickness = 1.0 / 16.0;
    let half_width = 1.0 / 16.0;

    let connects_to = |voxel: Voxel| -> bool {
        matches!(
            voxel.id,
            mdminecraft_world::redstone_blocks::REDSTONE_WIRE
                | mdminecraft_world::redstone_blocks::LEVER
                | mdminecraft_world::redstone_blocks::STONE_BUTTON
                | mdminecraft_world::redstone_blocks::OAK_BUTTON
                | mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
                | mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
                | mdminecraft_world::redstone_blocks::REDSTONE_TORCH
                | mdminecraft_world::redstone_blocks::REDSTONE_REPEATER
                | mdminecraft_world::redstone_blocks::REDSTONE_COMPARATOR
                | mdminecraft_world::redstone_blocks::REDSTONE_OBSERVER
                | mdminecraft_world::redstone_blocks::REDSTONE_LAMP
                | mdminecraft_world::redstone_blocks::REDSTONE_LAMP_LIT
        )
    };

    let chunk_pos = chunk.position();
    let origin_x = chunk_pos.x * CHUNK_SIZE_X as i32;
    let origin_z = chunk_pos.z * CHUNK_SIZE_Z as i32;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_WIRE {
                    continue;
                }

                let world_x = origin_x + x as i32;
                let world_y = local_y_to_world_y(y);
                let world_z = origin_z + z as i32;

                let connect_west =
                    voxel_at_world(world_x - 1, world_y, world_z).is_some_and(connects_to);
                let connect_east =
                    voxel_at_world(world_x + 1, world_y, world_z).is_some_and(connects_to);
                let connect_north =
                    voxel_at_world(world_x, world_y, world_z - 1).is_some_and(connects_to);
                let connect_south =
                    voxel_at_world(world_x, world_y, world_z + 1).is_some_and(connects_to);

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let min_y = base_y;
                let max_y = base_y + thickness;

                let center_min_x = base_x + 0.5 - half_width;
                let center_max_x = base_x + 0.5 + half_width;
                let center_min_z = base_z + 0.5 - half_width;
                let center_max_z = base_z + 0.5 + half_width;

                let any_x = connect_west || connect_east;
                let any_z = connect_north || connect_south;

                if any_x {
                    let min_x = if connect_west { base_x } else { center_min_x };
                    let max_x = if connect_east {
                        base_x + 1.0
                    } else {
                        center_max_x
                    };
                    emit_box(
                        builder,
                        voxel.id,
                        [min_x, min_y, center_min_z],
                        [max_x, max_y, center_max_z],
                        light,
                    );
                }

                if any_z {
                    let min_z = if connect_north { base_z } else { center_min_z };
                    let max_z = if connect_south {
                        base_z + 1.0
                    } else {
                        center_max_z
                    };
                    emit_box(
                        builder,
                        voxel.id,
                        [center_min_x, min_y, min_z],
                        [center_max_x, max_y, max_z],
                        light,
                    );
                }

                if !any_x && !any_z {
                    emit_box(
                        builder,
                        voxel.id,
                        [center_min_x, min_y, center_min_z],
                        [center_max_x, max_y, center_max_z],
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_redstone_repeaters(chunk: &Chunk, builder: &mut MeshBuilder) {
    let pad = 1.0 / 16.0;
    let plate_height = 2.0 / 16.0;
    let ridge_depth = 4.0 / 16.0;
    let ridge_height = 2.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_REPEATER {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x + pad, base_y, base_z + pad],
                    [
                        base_x + 1.0 - pad,
                        base_y + plate_height,
                        base_z + 1.0 - pad,
                    ],
                    light,
                );

                let facing = mdminecraft_world::repeater_facing(voxel.state);

                // Minimal direction indicator: a small ridge near the repeater's "front" edge.
                let ridge_min_y = base_y + plate_height;
                let ridge_max_y = ridge_min_y + ridge_height;

                let (min, max) = match facing {
                    mdminecraft_world::Facing::North => (
                        [base_x + 4.0 / 16.0, ridge_min_y, base_z + pad],
                        [
                            base_x + 12.0 / 16.0,
                            ridge_max_y,
                            base_z + pad + ridge_depth,
                        ],
                    ),
                    mdminecraft_world::Facing::South => (
                        [
                            base_x + 4.0 / 16.0,
                            ridge_min_y,
                            base_z + 1.0 - pad - ridge_depth,
                        ],
                        [base_x + 12.0 / 16.0, ridge_max_y, base_z + 1.0 - pad],
                    ),
                    mdminecraft_world::Facing::East => (
                        [
                            base_x + 1.0 - pad - ridge_depth,
                            ridge_min_y,
                            base_z + 4.0 / 16.0,
                        ],
                        [base_x + 1.0 - pad, ridge_max_y, base_z + 12.0 / 16.0],
                    ),
                    mdminecraft_world::Facing::West => (
                        [base_x + pad, ridge_min_y, base_z + 4.0 / 16.0],
                        [
                            base_x + pad + ridge_depth,
                            ridge_max_y,
                            base_z + 12.0 / 16.0,
                        ],
                    ),
                };

                emit_box(builder, voxel.id, min, max, light);
            }
        }
    }
}

fn mesh_redstone_comparators(chunk: &Chunk, builder: &mut MeshBuilder) {
    let pad = 1.0 / 16.0;
    let plate_height = 2.0 / 16.0;
    let post_size = 2.0 / 16.0;
    let post_height = 4.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_COMPARATOR {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x + pad, base_y, base_z + pad],
                    [
                        base_x + 1.0 - pad,
                        base_y + plate_height,
                        base_z + 1.0 - pad,
                    ],
                    light,
                );

                let facing = mdminecraft_world::comparator_facing(voxel.state);
                let subtract = mdminecraft_world::is_comparator_subtract_mode(voxel.state);

                let post_min_y = base_y + plate_height;
                let post_max_y = post_min_y + post_height;

                let (rear_a_min, rear_a_max, rear_b_min, rear_b_max, front_min, front_max) =
                    match facing {
                        mdminecraft_world::Facing::North => (
                            // Rear posts near the south edge.
                            [
                                base_x + 4.0 / 16.0,
                                post_min_y,
                                base_z + 1.0 - pad - 4.0 / 16.0,
                            ],
                            [
                                base_x + 4.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 1.0 - pad - 4.0 / 16.0 + post_size,
                            ],
                            [
                                base_x + 10.0 / 16.0,
                                post_min_y,
                                base_z + 1.0 - pad - 4.0 / 16.0,
                            ],
                            [
                                base_x + 10.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 1.0 - pad - 4.0 / 16.0 + post_size,
                            ],
                            // Front post near the north edge.
                            [base_x + 7.0 / 16.0, post_min_y, base_z + pad + 4.0 / 16.0],
                            [
                                base_x + 7.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + pad + 4.0 / 16.0 + post_size,
                            ],
                        ),
                        mdminecraft_world::Facing::South => (
                            // Rear posts near the north edge.
                            [base_x + 4.0 / 16.0, post_min_y, base_z + pad + 2.0 / 16.0],
                            [
                                base_x + 4.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + pad + 2.0 / 16.0 + post_size,
                            ],
                            [base_x + 10.0 / 16.0, post_min_y, base_z + pad + 2.0 / 16.0],
                            [
                                base_x + 10.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + pad + 2.0 / 16.0 + post_size,
                            ],
                            // Front post near the south edge.
                            [
                                base_x + 7.0 / 16.0,
                                post_min_y,
                                base_z + 1.0 - pad - 6.0 / 16.0,
                            ],
                            [
                                base_x + 7.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 1.0 - pad - 6.0 / 16.0 + post_size,
                            ],
                        ),
                        mdminecraft_world::Facing::East => (
                            // Rear posts near the west edge.
                            [base_x + pad + 2.0 / 16.0, post_min_y, base_z + 4.0 / 16.0],
                            [
                                base_x + pad + 2.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 4.0 / 16.0 + post_size,
                            ],
                            [base_x + pad + 2.0 / 16.0, post_min_y, base_z + 10.0 / 16.0],
                            [
                                base_x + pad + 2.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 10.0 / 16.0 + post_size,
                            ],
                            // Front post near the east edge.
                            [
                                base_x + 1.0 - pad - 6.0 / 16.0,
                                post_min_y,
                                base_z + 7.0 / 16.0,
                            ],
                            [
                                base_x + 1.0 - pad - 6.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 7.0 / 16.0 + post_size,
                            ],
                        ),
                        mdminecraft_world::Facing::West => (
                            // Rear posts near the east edge.
                            [
                                base_x + 1.0 - pad - 4.0 / 16.0,
                                post_min_y,
                                base_z + 4.0 / 16.0,
                            ],
                            [
                                base_x + 1.0 - pad - 4.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 4.0 / 16.0 + post_size,
                            ],
                            [
                                base_x + 1.0 - pad - 4.0 / 16.0,
                                post_min_y,
                                base_z + 10.0 / 16.0,
                            ],
                            [
                                base_x + 1.0 - pad - 4.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 10.0 / 16.0 + post_size,
                            ],
                            // Front post near the west edge.
                            [base_x + pad + 4.0 / 16.0, post_min_y, base_z + 7.0 / 16.0],
                            [
                                base_x + pad + 4.0 / 16.0 + post_size,
                                post_max_y,
                                base_z + 7.0 / 16.0 + post_size,
                            ],
                        ),
                    };

                emit_box(builder, voxel.id, rear_a_min, rear_a_max, light);
                emit_box(builder, voxel.id, rear_b_min, rear_b_max, light);
                emit_box(builder, voxel.id, front_min, front_max, light);

                if subtract {
                    // Minimal mode indicator: small center nub.
                    let nub_min_y = base_y + plate_height;
                    let nub_max_y = nub_min_y + 1.0 / 16.0;
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + 7.0 / 16.0, nub_min_y, base_z + 7.0 / 16.0],
                        [base_x + 9.0 / 16.0, nub_max_y, base_z + 9.0 / 16.0],
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_pressure_plates(chunk: &Chunk, builder: &mut MeshBuilder) {
    let pad = 1.0 / 16.0;
    let unpressed_height = 1.0 / 16.0;
    let pressed_height = 1.0 / 32.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !matches!(
                    voxel.id,
                    mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
                        | mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
                ) {
                    continue;
                }

                let pressed = mdminecraft_world::is_active(voxel.state);
                let height = if pressed {
                    pressed_height
                } else {
                    unpressed_height
                };
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x + pad, base_y, base_z + pad],
                    [base_x + 1.0 - pad, base_y + height, base_z + 1.0 - pad],
                    light,
                );
            }
        }
    }
}

fn mesh_buttons(chunk: &Chunk, builder: &mut MeshBuilder) {
    let half = 3.0 / 16.0;
    let unpressed_depth = 2.0 / 16.0;
    let pressed_depth = 1.0 / 16.0;
    let wall_min_y = 6.0 / 16.0;
    let wall_max_y = 10.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !matches!(
                    voxel.id,
                    mdminecraft_world::redstone_blocks::STONE_BUTTON
                        | mdminecraft_world::redstone_blocks::OAK_BUTTON
                ) {
                    continue;
                }

                let pressed = mdminecraft_world::is_active(voxel.state);
                let depth = if pressed {
                    pressed_depth
                } else {
                    unpressed_depth
                };
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let (min, max) = if mdminecraft_world::is_wall_mounted(voxel.state) {
                    let facing = mdminecraft_world::wall_mounted_facing(voxel.state);
                    match facing {
                        mdminecraft_world::Facing::North => (
                            [
                                base_x + 0.5 - half,
                                base_y + wall_min_y,
                                base_z + 1.0 - depth,
                            ],
                            [base_x + 0.5 + half, base_y + wall_max_y, base_z + 1.0],
                        ),
                        mdminecraft_world::Facing::South => (
                            [base_x + 0.5 - half, base_y + wall_min_y, base_z],
                            [base_x + 0.5 + half, base_y + wall_max_y, base_z + depth],
                        ),
                        mdminecraft_world::Facing::East => (
                            [base_x, base_y + wall_min_y, base_z + 0.5 - half],
                            [base_x + depth, base_y + wall_max_y, base_z + 0.5 + half],
                        ),
                        mdminecraft_world::Facing::West => (
                            [
                                base_x + 1.0 - depth,
                                base_y + wall_min_y,
                                base_z + 0.5 - half,
                            ],
                            [base_x + 1.0, base_y + wall_max_y, base_z + 0.5 + half],
                        ),
                    }
                } else if mdminecraft_world::is_ceiling_mounted(voxel.state) {
                    (
                        [
                            base_x + 0.5 - half,
                            base_y + 1.0 - depth,
                            base_z + 0.5 - half,
                        ],
                        [base_x + 0.5 + half, base_y + 1.0, base_z + 0.5 + half],
                    )
                } else {
                    (
                        [base_x + 0.5 - half, base_y, base_z + 0.5 - half],
                        [base_x + 0.5 + half, base_y + depth, base_z + 0.5 + half],
                    )
                };

                emit_box(builder, voxel.id, min, max, light);
            }
        }
    }
}

fn mesh_levers(chunk: &Chunk, builder: &mut MeshBuilder) {
    let base_half = 3.0 / 16.0;
    let base_height = 1.0 / 16.0;
    let handle_half = 1.0 / 16.0;
    let handle_height = 10.0 / 16.0;
    let handle_shift = 2.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::redstone_blocks::LEVER {
                    continue;
                }

                let active = mdminecraft_world::is_active(voxel.state);
                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                let shift = if active { handle_shift } else { -handle_shift };

                if mdminecraft_world::is_wall_mounted(voxel.state) {
                    let facing = mdminecraft_world::wall_mounted_facing(voxel.state);

                    // Base plate.
                    let (base_min, base_max) = match facing {
                        mdminecraft_world::Facing::North => (
                            [
                                base_x + 0.5 - base_half,
                                base_y + 0.5 - base_half,
                                base_z + 1.0 - base_height,
                            ],
                            [
                                base_x + 0.5 + base_half,
                                base_y + 0.5 + base_half,
                                base_z + 1.0,
                            ],
                        ),
                        mdminecraft_world::Facing::South => (
                            [base_x + 0.5 - base_half, base_y + 0.5 - base_half, base_z],
                            [
                                base_x + 0.5 + base_half,
                                base_y + 0.5 + base_half,
                                base_z + base_height,
                            ],
                        ),
                        mdminecraft_world::Facing::East => (
                            [base_x, base_y + 0.5 - base_half, base_z + 0.5 - base_half],
                            [
                                base_x + base_height,
                                base_y + 0.5 + base_half,
                                base_z + 0.5 + base_half,
                            ],
                        ),
                        mdminecraft_world::Facing::West => (
                            [
                                base_x + 1.0 - base_height,
                                base_y + 0.5 - base_half,
                                base_z + 0.5 - base_half,
                            ],
                            [
                                base_x + 1.0,
                                base_y + 0.5 + base_half,
                                base_z + 0.5 + base_half,
                            ],
                        ),
                    };
                    emit_box(builder, voxel.id, base_min, base_max, light);

                    // Handle (simplified: protrudes from the wall; y-shift indicates active state).
                    let (handle_min, handle_max) = match facing {
                        mdminecraft_world::Facing::North => (
                            [
                                base_x + 0.5 - handle_half,
                                base_y + 0.5 - handle_half + shift,
                                base_z + 1.0 - base_height - handle_height,
                            ],
                            [
                                base_x + 0.5 + handle_half,
                                base_y + 0.5 + handle_half + shift,
                                base_z + 1.0 - base_height,
                            ],
                        ),
                        mdminecraft_world::Facing::South => (
                            [
                                base_x + 0.5 - handle_half,
                                base_y + 0.5 - handle_half + shift,
                                base_z + base_height,
                            ],
                            [
                                base_x + 0.5 + handle_half,
                                base_y + 0.5 + handle_half + shift,
                                base_z + base_height + handle_height,
                            ],
                        ),
                        mdminecraft_world::Facing::East => (
                            [
                                base_x + base_height,
                                base_y + 0.5 - handle_half + shift,
                                base_z + 0.5 - handle_half,
                            ],
                            [
                                base_x + base_height + handle_height,
                                base_y + 0.5 + handle_half + shift,
                                base_z + 0.5 + handle_half,
                            ],
                        ),
                        mdminecraft_world::Facing::West => (
                            [
                                base_x + 1.0 - base_height - handle_height,
                                base_y + 0.5 - handle_half + shift,
                                base_z + 0.5 - handle_half,
                            ],
                            [
                                base_x + 1.0 - base_height,
                                base_y + 0.5 + handle_half + shift,
                                base_z + 0.5 + handle_half,
                            ],
                        ),
                    };
                    emit_box(builder, voxel.id, handle_min, handle_max, light);
                } else if mdminecraft_world::is_ceiling_mounted(voxel.state) {
                    // Base plate.
                    emit_box(
                        builder,
                        voxel.id,
                        [
                            base_x + 0.5 - base_half,
                            base_y + 1.0 - base_height,
                            base_z + 0.5 - base_half,
                        ],
                        [
                            base_x + 0.5 + base_half,
                            base_y + 1.0,
                            base_z + 0.5 + base_half,
                        ],
                        light,
                    );

                    // Handle (simplified: offset to indicate active state).
                    emit_box(
                        builder,
                        voxel.id,
                        [
                            base_x + 0.5 - handle_half + shift,
                            base_y + 1.0 - base_height - handle_height,
                            base_z + 0.5 - handle_half,
                        ],
                        [
                            base_x + 0.5 + handle_half + shift,
                            base_y + 1.0 - base_height,
                            base_z + 0.5 + handle_half,
                        ],
                        light,
                    );
                } else {
                    // Floor-mounted lever (default).

                    // Base plate.
                    emit_box(
                        builder,
                        voxel.id,
                        [base_x + 0.5 - base_half, base_y, base_z + 0.5 - base_half],
                        [
                            base_x + 0.5 + base_half,
                            base_y + base_height,
                            base_z + 0.5 + base_half,
                        ],
                        light,
                    );

                    // Handle (simplified: offset to indicate active state).
                    emit_box(
                        builder,
                        voxel.id,
                        [
                            base_x + 0.5 - handle_half + shift,
                            base_y + base_height,
                            base_z + 0.5 - handle_half,
                        ],
                        [
                            base_x + 0.5 + handle_half + shift,
                            base_y + base_height + handle_height,
                            base_z + 0.5 + handle_half,
                        ],
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_cave_decorations(chunk: &Chunk, builder: &mut MeshBuilder) {
    let cross_inset = 0.1;
    let dripstone_half_width = 2.0 / 16.0;
    let carpet_height = 1.0 / 16.0;

    let inv_sqrt2 = 0.70710677_f32;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);

                let light = voxel.light_sky.max(voxel.light_block);
                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                match voxel.id {
                    mdminecraft_world::BLOCK_MOSS_CARPET => {
                        emit_box(
                            builder,
                            voxel.id,
                            [base_x, base_y, base_z],
                            [base_x + 1.0, base_y + carpet_height, base_z + 1.0],
                            light,
                        );
                    }
                    mdminecraft_world::BLOCK_POINTED_DRIPSTONE => {
                        emit_box(
                            builder,
                            voxel.id,
                            [
                                base_x + 0.5 - dripstone_half_width,
                                base_y,
                                base_z + 0.5 - dripstone_half_width,
                            ],
                            [
                                base_x + 0.5 + dripstone_half_width,
                                base_y + 1.0,
                                base_z + 0.5 + dripstone_half_width,
                            ],
                            light,
                        );
                    }
                    mdminecraft_world::BLOCK_GLOW_LICHEN
                    | mdminecraft_world::BLOCK_CAVE_VINES
                    | mdminecraft_world::BLOCK_SPORE_BLOSSOM
                    | mdminecraft_world::BLOCK_HANGING_ROOTS
                    | mdminecraft_world::BLOCK_SCULK_VEIN => {
                        let x0 = base_x + cross_inset;
                        let x1 = base_x + 1.0 - cross_inset;
                        let y0 = base_y;
                        let y1 = base_y + 1.0;
                        let z0 = base_z + cross_inset;
                        let z1 = base_z + 1.0 - cross_inset;

                        let planes = [
                            (
                                [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]],
                                [inv_sqrt2, 0.0, -inv_sqrt2],
                            ),
                            (
                                [[x0, y0, z1], [x1, y0, z0], [x1, y1, z0], [x0, y1, z1]],
                                [inv_sqrt2, 0.0, inv_sqrt2],
                            ),
                        ];

                        for (corners, normal) in planes {
                            builder.push_quad(
                                voxel.id,
                                BlockFace::North,
                                normal,
                                corners,
                                true,
                                light,
                            );
                            builder.push_quad(
                                voxel.id,
                                BlockFace::North,
                                [-normal[0], -normal[1], -normal[2]],
                                corners,
                                false,
                                light,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn mesh_beds(chunk: &Chunk, builder: &mut MeshBuilder) {
    let height = 0.5625;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_bed(voxel.id) {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x, base_y, base_z],
                    [base_x + 1.0, base_y + height, base_z + 1.0],
                    light,
                );
            }
        }
    }
}

fn mesh_chests(chunk: &Chunk, builder: &mut MeshBuilder) {
    let height = 0.875;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_chest(voxel.id) {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x, base_y, base_z],
                    [base_x + 1.0, base_y + height, base_z + 1.0],
                    light,
                );
            }
        }
    }
}

fn mesh_enchanting_tables(chunk: &Chunk, builder: &mut MeshBuilder) {
    let height = 12.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::BLOCK_ENCHANTING_TABLE {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x, base_y, base_z],
                    [base_x + 1.0, base_y + height, base_z + 1.0],
                    light,
                );
            }
        }
    }
}

fn mesh_brewing_stands(chunk: &Chunk, builder: &mut MeshBuilder) {
    let height = 14.0 / 16.0;
    let pad = 4.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::BLOCK_BREWING_STAND {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x + pad, base_y, base_z + pad],
                    [base_x + 1.0 - pad, base_y + height, base_z + 1.0 - pad],
                    light,
                );
            }
        }
    }
}

fn mesh_farmland(chunk: &Chunk, builder: &mut MeshBuilder) {
    let height = 15.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if !mdminecraft_world::is_farmland(voxel.id) {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let base_x = x as f32;
                let base_y = y as f32;
                let base_z = z as f32;

                emit_box(
                    builder,
                    voxel.id,
                    [base_x, base_y, base_z],
                    [base_x + 1.0, base_y + height, base_z + 1.0],
                    light,
                );
            }
        }
    }
}

fn mesh_nether_portals(chunk: &Chunk, builder: &mut MeshBuilder) {
    const THICKNESS: f32 = 2.0 / 16.0;
    let half_thickness = THICKNESS * 0.5;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::BLOCK_NETHER_PORTAL {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);
                let align_x = (voxel.state & 0x01) == 0;

                let x0 = x as f32;
                let y0 = y as f32;
                let z0 = z as f32;
                let x1 = x0 + 1.0;
                let y1 = y0 + 1.0;
                let z1 = z0 + 1.0;

                if align_x {
                    let z_center = z0 + 0.5;
                    let z_north = z_center - half_thickness;
                    let z_south = z_center + half_thickness;

                    builder.push_quad(
                        voxel.id,
                        BlockFace::North,
                        [0.0, 0.0, -1.0],
                        [
                            [x0, y0, z_north],
                            [x0, y1, z_north],
                            [x1, y1, z_north],
                            [x1, y0, z_north],
                        ],
                        false,
                        light,
                    );
                    builder.push_quad(
                        voxel.id,
                        BlockFace::South,
                        [0.0, 0.0, 1.0],
                        [
                            [x0, y0, z_south],
                            [x0, y1, z_south],
                            [x1, y1, z_south],
                            [x1, y0, z_south],
                        ],
                        true,
                        light,
                    );
                } else {
                    let x_center = x0 + 0.5;
                    let x_west = x_center - half_thickness;
                    let x_east = x_center + half_thickness;

                    builder.push_quad(
                        voxel.id,
                        BlockFace::West,
                        [-1.0, 0.0, 0.0],
                        [
                            [x_west, y0, z0],
                            [x_west, y1, z0],
                            [x_west, y1, z1],
                            [x_west, y0, z1],
                        ],
                        false,
                        light,
                    );
                    builder.push_quad(
                        voxel.id,
                        BlockFace::East,
                        [1.0, 0.0, 0.0],
                        [
                            [x_east, y0, z0],
                            [x_east, y1, z0],
                            [x_east, y1, z1],
                            [x_east, y0, z1],
                        ],
                        true,
                        light,
                    );
                }
            }
        }
    }
}

fn mesh_end_portals(chunk: &Chunk, builder: &mut MeshBuilder) {
    const SURFACE_HEIGHT: f32 = 12.0 / 16.0;

    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let voxel = chunk.voxel(x, y, z);
                if voxel.id != mdminecraft_world::BLOCK_END_PORTAL {
                    continue;
                }

                let light = voxel.light_sky.max(voxel.light_block);

                let x0 = x as f32;
                let y0 = y as f32 + SURFACE_HEIGHT;
                let z0 = z as f32;
                let x1 = x0 + 1.0;
                let z1 = z0 + 1.0;

                builder.push_quad(
                    voxel.id,
                    BlockFace::Up,
                    [0.0, 1.0, 0.0],
                    [[x0, y0, z0], [x0, y0, z1], [x1, y0, z1], [x1, y0, z0]],
                    true,
                    light,
                );
            }
        }
    }
}

const FACE_UP: u8 = 1 << 0;
const FACE_DOWN: u8 = 1 << 1;
const FACE_NORTH: u8 = 1 << 2;
const FACE_SOUTH: u8 = 1 << 3;
const FACE_EAST: u8 = 1 << 4;
const FACE_WEST: u8 = 1 << 5;
const FACES_ALL: u8 = FACE_UP | FACE_DOWN | FACE_NORTH | FACE_SOUTH | FACE_EAST | FACE_WEST;

fn emit_box(builder: &mut MeshBuilder, block_id: BlockId, min: [f32; 3], max: [f32; 3], light: u8) {
    emit_box_masked(builder, block_id, min, max, light, FACES_ALL);
}

fn emit_box_masked(
    builder: &mut MeshBuilder,
    block_id: BlockId,
    min: [f32; 3],
    max: [f32; 3],
    light: u8,
    faces: u8,
) {
    let (x0, y0, z0) = (min[0], min[1], min[2]);
    let (x1, y1, z1) = (max[0], max[1], max[2]);

    if (faces & FACE_WEST) != 0 {
        builder.push_quad(
            block_id,
            BlockFace::West,
            [-1.0, 0.0, 0.0],
            [[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]],
            false,
            light,
        );
    }
    if (faces & FACE_EAST) != 0 {
        builder.push_quad(
            block_id,
            BlockFace::East,
            [1.0, 0.0, 0.0],
            [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
            true,
            light,
        );
    }
    if (faces & FACE_NORTH) != 0 {
        builder.push_quad(
            block_id,
            BlockFace::North,
            [0.0, 0.0, -1.0],
            [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
            false,
            light,
        );
    }
    if (faces & FACE_SOUTH) != 0 {
        builder.push_quad(
            block_id,
            BlockFace::South,
            [0.0, 0.0, 1.0],
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            true,
            light,
        );
    }
    if (faces & FACE_DOWN) != 0 {
        builder.push_quad(
            block_id,
            BlockFace::Down,
            [0.0, -1.0, 0.0],
            [[x0, y0, z0], [x0, y0, z1], [x1, y0, z1], [x1, y0, z0]],
            false,
            light,
        );
    }
    if (faces & FACE_UP) != 0 {
        builder.push_quad(
            block_id,
            BlockFace::Up,
            [0.0, 1.0, 0.0],
            [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
            true,
            light,
        );
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn is_opaque(voxel: Voxel, registry: &BlockRegistry) -> bool {
    if is_alpha_blended(voxel.id, registry)
        || mdminecraft_world::is_stairs(voxel.id)
        || mdminecraft_world::is_slab(voxel.id)
        || mdminecraft_world::is_farmland(voxel.id)
        || matches!(
            voxel.id,
            mdminecraft_world::BLOCK_ENCHANTING_TABLE | mdminecraft_world::BLOCK_BREWING_STAND
        )
    {
        return false;
    }
    registry
        .descriptor(voxel.id)
        .map(|d| d.opaque)
        .unwrap_or(voxel.id != BLOCK_AIR)
}

/// Check if a voxel is a solid (non-air) block that should be rendered
fn is_solid(voxel: Voxel) -> bool {
    voxel.id != BLOCK_AIR
        && !is_fluid(voxel.id)
        && !mdminecraft_world::is_stairs(voxel.id)
        && !mdminecraft_world::is_slab(voxel.id)
        && voxel.id != BLOCK_SNOW
        && !mdminecraft_world::is_trapdoor(voxel.id)
        && !mdminecraft_world::is_door(voxel.id)
        && !mdminecraft_world::is_ladder(voxel.id)
        && !mdminecraft_world::CropType::is_crop(voxel.id)
        && !mdminecraft_world::is_bed(voxel.id)
        && !mdminecraft_world::is_chest(voxel.id)
        && !mdminecraft_world::is_farmland(voxel.id)
        && voxel.id != mdminecraft_world::BLOCK_ENCHANTING_TABLE
        && voxel.id != mdminecraft_world::BLOCK_BREWING_STAND
        && voxel.id != mdminecraft_world::BLOCK_NETHER_PORTAL
        && voxel.id != mdminecraft_world::BLOCK_END_PORTAL
        && voxel.id != mdminecraft_world::BLOCK_GLOW_LICHEN
        && voxel.id != mdminecraft_world::BLOCK_POINTED_DRIPSTONE
        && voxel.id != mdminecraft_world::BLOCK_CAVE_VINES
        && voxel.id != mdminecraft_world::BLOCK_MOSS_CARPET
        && voxel.id != mdminecraft_world::BLOCK_SPORE_BLOSSOM
        && voxel.id != mdminecraft_world::BLOCK_HANGING_ROOTS
        && voxel.id != mdminecraft_world::BLOCK_SCULK_VEIN
        && voxel.id != mdminecraft_world::redstone_blocks::LEVER
        && voxel.id != mdminecraft_world::redstone_blocks::STONE_BUTTON
        && voxel.id != mdminecraft_world::redstone_blocks::OAK_BUTTON
        && voxel.id != mdminecraft_world::redstone_blocks::STONE_PRESSURE_PLATE
        && voxel.id != mdminecraft_world::redstone_blocks::OAK_PRESSURE_PLATE
        && voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_WIRE
        && voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_REPEATER
        && voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_COMPARATOR
        && voxel.id != interactive_blocks::TORCH
        && voxel.id != mdminecraft_world::redstone_blocks::REDSTONE_TORCH
        && voxel.id != interactive_blocks::GLASS_PANE
        && voxel.id != interactive_blocks::IRON_BARS
        && voxel.id != interactive_blocks::OAK_FENCE
        && voxel.id != interactive_blocks::OAK_FENCE_GATE
        && voxel.id != interactive_blocks::COBBLESTONE_WALL
        && voxel.id != interactive_blocks::STONE_BRICK_WALL
}

#[cfg(test)]
mod tests {
    use mdminecraft_assets::{BlockDefinition, BlockDescriptor, BlockRegistry};
    use mdminecraft_world::{
        interactive_blocks, set_fluid_level, set_waterlogged, Chunk, ChunkPos, Voxel,
    };

    use super::*;

    fn registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor::simple("air", false),
            BlockDescriptor::simple("stone", true),
            BlockDescriptor::simple("leaves", false), // transparent block
        ])
    }

    fn registry_with_biome_tints() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor::simple("air", false),
            BlockDescriptor::from_definition(BlockDefinition {
                name: "grass".to_string(),
                key: None,
                tags: vec!["render/tint/grass".to_string()],
                opaque: true,
                light_opacity: None,
                light_emission: None,
                emissive: None,
                texture: None,
                textures: None,
                harvest_level: None,
            }),
            BlockDescriptor::from_definition(BlockDefinition {
                name: "leaves".to_string(),
                key: None,
                tags: vec!["render/tint/foliage".to_string()],
                opaque: false,
                light_opacity: None,
                light_emission: None,
                emissive: None,
                texture: None,
                textures: None,
                harvest_level: None,
            }),
        ])
    }

    fn registry_with_fluids() -> BlockRegistry {
        let max_id = mdminecraft_world::BLOCK_LAVA_FLOWING as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);

        for id in 0..=max_id {
            let id_u16 = id as u16;
            let opaque = !matches!(
                id_u16,
                BLOCK_AIR
                    | BLOCK_WATER
                    | BLOCK_WATER_FLOWING
                    | mdminecraft_world::BLOCK_LAVA
                    | mdminecraft_world::BLOCK_LAVA_FLOWING
            );
            let name = match id_u16 {
                BLOCK_AIR => "air".to_string(),
                BLOCK_WATER => "water".to_string(),
                BLOCK_WATER_FLOWING => "water_flowing".to_string(),
                mdminecraft_world::BLOCK_LAVA => "lava".to_string(),
                mdminecraft_world::BLOCK_LAVA_FLOWING => "lava_flowing".to_string(),
                mdminecraft_world::BLOCK_ICE => "ice".to_string(),
                _ => format!("block_{id}"),
            };

            if id_u16 == mdminecraft_world::BLOCK_ICE {
                descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
                    name,
                    key: None,
                    tags: vec!["render/translucent".to_string()],
                    opaque,
                    light_opacity: None,
                    light_emission: None,
                    emissive: None,
                    texture: None,
                    textures: None,
                    harvest_level: None,
                }));
            } else {
                descriptors.push(BlockDescriptor::simple(&name, opaque));
            }
        }

        BlockRegistry::new(descriptors)
    }

    fn registry_with_snow() -> BlockRegistry {
        let max_id = BLOCK_SNOW as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);

        for id in 0..=max_id {
            let id_u16 = id as u16;
            let opaque = id_u16 != BLOCK_AIR && id_u16 != BLOCK_SNOW;
            let name = match id_u16 {
                BLOCK_AIR => "air".to_string(),
                BLOCK_SNOW => "snow".to_string(),
                _ => format!("block_{id}"),
            };
            descriptors.push(BlockDescriptor::simple(&name, opaque));
        }

        BlockRegistry::new(descriptors)
    }

    fn registry_with_glass() -> BlockRegistry {
        let max_id = mdminecraft_world::BLOCK_GLASS as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);

        for id in 0..=max_id {
            let id_u16 = id as u16;
            let opaque = !matches!(id_u16, BLOCK_AIR | mdminecraft_world::BLOCK_GLASS);
            let name = match id_u16 {
                BLOCK_AIR => "air".to_string(),
                mdminecraft_world::BLOCK_GLASS => "glass".to_string(),
                _ => format!("block_{id}"),
            };
            if id_u16 == mdminecraft_world::BLOCK_GLASS {
                descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
                    name,
                    key: None,
                    tags: vec![
                        "render/translucent".to_string(),
                        "render/alpha/glass".to_string(),
                    ],
                    opaque,
                    light_opacity: None,
                    light_emission: None,
                    emissive: None,
                    texture: None,
                    textures: None,
                    harvest_level: None,
                }));
            } else {
                descriptors.push(BlockDescriptor::simple(&name, opaque));
            }
        }

        BlockRegistry::new(descriptors)
    }

    fn registry_with_fire() -> BlockRegistry {
        let max_id = mdminecraft_world::BLOCK_FIRE as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);

        for id in 0..=max_id {
            let id_u16 = id as u16;
            let opaque = !matches!(id_u16, BLOCK_AIR | mdminecraft_world::BLOCK_FIRE);
            let name = match id_u16 {
                BLOCK_AIR => "air".to_string(),
                mdminecraft_world::BLOCK_FIRE => "fire".to_string(),
                _ => format!("block_{id}"),
            };

            if id_u16 == mdminecraft_world::BLOCK_FIRE {
                descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
                    name,
                    key: None,
                    tags: vec![
                        "render/translucent".to_string(),
                        "render/alpha/fire".to_string(),
                    ],
                    opaque,
                    light_opacity: None,
                    light_emission: None,
                    emissive: None,
                    texture: None,
                    textures: None,
                    harvest_level: None,
                }));
            } else {
                descriptors.push(BlockDescriptor::simple(&name, opaque));
            }
        }

        BlockRegistry::new(descriptors)
    }

    fn registry_with_end_portal() -> BlockRegistry {
        let max_id = mdminecraft_world::BLOCK_END_PORTAL as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));

        for id in 1..=max_id {
            if id == mdminecraft_world::BLOCK_END_PORTAL as usize {
                descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
                    name: "end_portal".to_string(),
                    key: None,
                    tags: vec![
                        "render/translucent".to_string(),
                        "render/alpha/portal/end".to_string(),
                    ],
                    opaque: false,
                    light_opacity: None,
                    light_emission: None,
                    emissive: None,
                    texture: None,
                    textures: None,
                    harvest_level: None,
                }));
            } else {
                descriptors.push(BlockDescriptor::simple("solid", true));
            }
        }

        BlockRegistry::new(descriptors)
    }

    fn registry_with_nether_portal() -> BlockRegistry {
        let max_id = mdminecraft_world::BLOCK_NETHER_PORTAL as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));

        for id in 1..=max_id {
            if id == mdminecraft_world::BLOCK_NETHER_PORTAL as usize {
                descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
                    name: "nether_portal".to_string(),
                    key: None,
                    tags: vec![
                        "render/translucent".to_string(),
                        "render/alpha/portal/nether".to_string(),
                    ],
                    opaque: false,
                    light_opacity: None,
                    light_emission: None,
                    emissive: None,
                    texture: None,
                    textures: None,
                    harvest_level: None,
                }));
            } else {
                descriptors.push(BlockDescriptor::simple("solid", true));
            }
        }

        BlockRegistry::new(descriptors)
    }

    #[test]
    fn chunk_with_single_block_meshes() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let voxel = Voxel {
            id: 1,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(1, 1, 1, voxel);
        let registry = registry();
        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(!mesh.vertices.is_empty());
        assert_eq!(mesh.indices_opaque.len(), 36); // 6 faces * 2 tris * 3 indices
    }

    #[test]
    fn mesh_hash_changes_with_voxel_updates() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();
        let mesh = mesh_chunk(&chunk, &registry, None);
        let hash_empty = mesh.hash;

        let voxel = Voxel {
            id: 2,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };
        chunk.set_voxel(0, 0, 0, voxel);
        let mesh_updated = mesh_chunk(&chunk, &registry, None);
        assert_ne!(hash_empty, mesh_updated.hash);
    }

    #[test]
    fn transparent_block_renders_faces() {
        // Test that transparent blocks (like leaves) render faces when adjacent to air
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        // Place a transparent block (id=2 is "leaves" with opaque=false)
        let leaves = Voxel {
            id: 2,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };
        chunk.set_voxel(5, 5, 5, leaves);

        let mesh = mesh_chunk(&chunk, &registry, None);

        // Transparent block surrounded by air should have 6 faces
        assert!(
            !mesh.vertices.is_empty(),
            "Transparent block should generate mesh vertices"
        );
        assert_eq!(
            mesh.indices_opaque.len(),
            36,
            "Transparent block should have 6 faces (36 indices)"
        );
    }

    #[test]
    fn transparent_blocks_adjacent_render_correctly() {
        // Test that two adjacent transparent blocks of the same type don't render internal faces
        // and that greedy meshing combines faces properly
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        let leaves = Voxel {
            id: 2,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };
        // Place two adjacent leaves blocks along Z axis
        chunk.set_voxel(5, 5, 5, leaves);
        chunk.set_voxel(5, 5, 6, leaves);

        let mesh = mesh_chunk(&chunk, &registry, None);

        // Two adjacent same-type transparent blocks: greedy mesher merges front/back Z faces
        // So we get: 2 merged Z faces (front+back) + 4 individual side faces per block merged
        // = fewer total quads due to greedy meshing
        // The key test is that SOME mesh is generated (transparent blocks render)
        assert!(
            !mesh.vertices.is_empty(),
            "Should generate mesh for adjacent transparent blocks"
        );
        // With greedy meshing, the exact count depends on merge opportunities
        // Just verify we have less than 12 faces (would be 72 indices if no merging)
        assert!(
            !mesh.indices_opaque.is_empty() && mesh.indices_opaque.len() < 72,
            "Greedy meshing should reduce face count for adjacent blocks, got {} indices",
            mesh.indices_opaque.len()
        );
    }

    #[test]
    fn flowing_water_renders_with_partial_height() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_fluids();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: BLOCK_WATER_FLOWING,
                state: set_fluid_level(0, 7),
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        let top_vertices: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == BLOCK_WATER_FLOWING && v.normal == [0.0, 1.0, 0.0])
            .collect();

        assert_eq!(top_vertices.len(), 4);
        let expected_y = 1.0 + 7.0 / 8.0;
        for vertex in top_vertices {
            assert!(
                (vertex.position[1] - expected_y).abs() < 0.0001,
                "Expected flowing water top Y to be {expected_y}, got {}",
                vertex.position[1]
            );
        }
    }

    #[test]
    fn fluids_emit_only_alpha_indices() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_fluids();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: BLOCK_WATER,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(
            !mesh.indices_alpha.is_empty(),
            "Expected water to emit alpha indices"
        );
        assert!(
            mesh.indices_opaque.is_empty(),
            "Expected no opaque indices for a chunk containing only water"
        );
    }

    #[test]
    fn glass_emits_only_alpha_indices() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_glass();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_GLASS,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(
            !mesh.indices_alpha.is_empty(),
            "Expected glass to emit alpha indices"
        );
        assert!(
            mesh.indices_opaque.is_empty(),
            "Expected no opaque indices for a chunk containing only glass"
        );
    }

    #[test]
    fn alpha_pass_vertices_encode_alpha_kind() {
        let pos = ChunkPos::new(0, 0);

        // Water.
        {
            let registry = registry_with_fluids();
            let mut chunk = Chunk::new(pos);
            chunk.set_voxel(
                1,
                1,
                1,
                Voxel {
                    id: BLOCK_WATER,
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                },
            );

            let mesh = mesh_chunk(&chunk, &registry, None);
            let vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.block_id == BLOCK_WATER)
                .collect();
            assert!(!vertices.is_empty());
            for v in vertices {
                assert_ne!(v.extra & EXTRA_ALPHA_BIT, 0);
                assert_eq!((v.extra >> EXTRA_TINT_SHIFT) & 0x7, 1);
            }
        }

        // Lava.
        {
            let registry = registry_with_fluids();
            let mut chunk = Chunk::new(pos);
            chunk.set_voxel(
                1,
                1,
                1,
                Voxel {
                    id: mdminecraft_world::BLOCK_LAVA,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );

            let mesh = mesh_chunk(&chunk, &registry, None);
            let vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.block_id == mdminecraft_world::BLOCK_LAVA)
                .collect();
            assert!(!vertices.is_empty());
            for v in vertices {
                assert_ne!(v.extra & EXTRA_ALPHA_BIT, 0);
                assert_eq!((v.extra >> EXTRA_TINT_SHIFT) & 0x7, 2);
            }
        }

        // Glass.
        {
            let registry = registry_with_glass();
            let mut chunk = Chunk::new(pos);
            chunk.set_voxel(
                1,
                1,
                1,
                Voxel {
                    id: mdminecraft_world::BLOCK_GLASS,
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                },
            );

            let mesh = mesh_chunk(&chunk, &registry, None);
            let vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.block_id == mdminecraft_world::BLOCK_GLASS)
                .collect();
            assert!(!vertices.is_empty());
            for v in vertices {
                assert_ne!(v.extra & EXTRA_ALPHA_BIT, 0);
                assert_eq!((v.extra >> EXTRA_TINT_SHIFT) & 0x7, 3);
                assert_eq!(v.extra & 0x0F, 0);
            }
        }

        // Fire.
        {
            let registry = registry_with_fire();
            let mut chunk = Chunk::new(pos);
            chunk.set_voxel(
                1,
                1,
                1,
                Voxel {
                    id: mdminecraft_world::BLOCK_FIRE,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );

            let mesh = mesh_chunk(&chunk, &registry, None);
            let vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.block_id == mdminecraft_world::BLOCK_FIRE)
                .collect();
            assert!(!vertices.is_empty());
            for v in vertices {
                assert_ne!(v.extra & EXTRA_ALPHA_BIT, 0);
                assert_eq!((v.extra >> EXTRA_TINT_SHIFT) & 0x7, 6);
            }
        }

        // Nether portal.
        {
            let registry = registry_with_nether_portal();
            let mut chunk = Chunk::new(pos);
            chunk.set_voxel(
                1,
                1,
                1,
                Voxel {
                    id: mdminecraft_world::BLOCK_NETHER_PORTAL,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );

            let mesh = mesh_chunk(&chunk, &registry, None);
            let vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.block_id == mdminecraft_world::BLOCK_NETHER_PORTAL)
                .collect();
            assert!(!vertices.is_empty());
            for v in vertices {
                assert_ne!(v.extra & EXTRA_ALPHA_BIT, 0);
                assert_eq!((v.extra >> EXTRA_TINT_SHIFT) & 0x7, 4);
            }
        }

        // End portal.
        {
            let registry = registry_with_end_portal();
            let mut chunk = Chunk::new(pos);
            chunk.set_voxel(
                1,
                1,
                1,
                Voxel {
                    id: mdminecraft_world::BLOCK_END_PORTAL,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );

            let mesh = mesh_chunk(&chunk, &registry, None);
            let vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.block_id == mdminecraft_world::BLOCK_END_PORTAL)
                .collect();
            assert!(!vertices.is_empty());
            for v in vertices {
                assert_ne!(v.extra & EXTRA_ALPHA_BIT, 0);
                assert_eq!((v.extra >> EXTRA_TINT_SHIFT) & 0x7, 5);
            }
        }
    }

    #[test]
    fn end_portal_emits_surface_in_alpha_pass() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_end_portal();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_END_PORTAL,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(
            mesh.indices_opaque.is_empty(),
            "Expected no opaque indices for a chunk containing only end portal"
        );
        assert_eq!(mesh.indices_alpha.len(), 6);

        let top_vertices: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| {
                v.block_id == mdminecraft_world::BLOCK_END_PORTAL && v.normal == [0.0, 1.0, 0.0]
            })
            .collect();
        assert_eq!(top_vertices.len(), 4);

        let expected_y = 1.0 + 12.0 / 16.0;
        for vertex in top_vertices {
            assert!(
                (vertex.position[1] - expected_y).abs() < 0.0001,
                "Expected end portal surface Y to be {expected_y}, got {}",
                vertex.position[1]
            );
        }
    }

    #[test]
    fn nether_portal_emits_vertical_planes_in_alpha_pass() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_nether_portal();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_NETHER_PORTAL,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(
            mesh.indices_opaque.is_empty(),
            "Expected no opaque indices for a chunk containing only nether portal"
        );
        assert_eq!(mesh.indices_alpha.len(), 12);

        let half_thickness = (2.0 / 16.0) * 0.5;
        let expected_center_z = 1.0 + 0.5;
        let expected_north_z = expected_center_z - half_thickness;
        let expected_south_z = expected_center_z + half_thickness;

        let north_vertices: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| {
                v.block_id == mdminecraft_world::BLOCK_NETHER_PORTAL && v.normal == [0.0, 0.0, -1.0]
            })
            .collect();
        let south_vertices: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| {
                v.block_id == mdminecraft_world::BLOCK_NETHER_PORTAL && v.normal == [0.0, 0.0, 1.0]
            })
            .collect();

        assert_eq!(north_vertices.len(), 4);
        assert_eq!(south_vertices.len(), 4);

        for v in north_vertices {
            assert!(
                (v.position[2] - expected_north_z).abs() < 0.0001,
                "Expected north portal plane z={expected_north_z}, got {}",
                v.position[2]
            );
        }
        for v in south_vertices {
            assert!(
                (v.position[2] - expected_south_z).abs() < 0.0001,
                "Expected south portal plane z={expected_south_z}, got {}",
                v.position[2]
            );
        }
    }

    #[test]
    fn nether_portal_axis_state_is_respected_across_chunk_seams() {
        let registry = registry_with_nether_portal();

        let chunk_pos_a = ChunkPos::new(0, 0);
        let chunk_pos_b = ChunkPos::new(0, 1);
        let mut chunk_a = Chunk::new(chunk_pos_a);
        let mut chunk_b = Chunk::new(chunk_pos_b);

        chunk_a.set_voxel(
            8,
            1,
            15,
            Voxel {
                id: mdminecraft_world::BLOCK_NETHER_PORTAL,
                state: 1,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk_b.set_voxel(
            8,
            1,
            0,
            Voxel {
                id: mdminecraft_world::BLOCK_NETHER_PORTAL,
                state: 1,
                light_sky: 0,
                light_block: 0,
            },
        );

        let origin_a_x = chunk_pos_a.x * CHUNK_SIZE_X as i32;
        let origin_a_z = chunk_pos_a.z * CHUNK_SIZE_Z as i32;

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
                let local_y = world_y_to_local_y(wy)?;

                let local_x = wx - origin_a_x;
                let local_z = wz - origin_a_z;
                if !(0..CHUNK_SIZE_X as i32).contains(&local_x)
                    || !(0..CHUNK_SIZE_Z as i32).contains(&local_z)
                {
                    return None;
                }

                Some(chunk_a.voxel(local_x as usize, local_y, local_z as usize))
            });

        let origin_b_x = chunk_pos_b.x * CHUNK_SIZE_X as i32;
        let origin_b_z = chunk_pos_b.z * CHUNK_SIZE_Z as i32;

        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            let local_y = world_y_to_local_y(wy)?;

            let chunk_x = wx.div_euclid(CHUNK_SIZE_X as i32);
            let chunk_z = wz.div_euclid(CHUNK_SIZE_Z as i32);
            let chunk_pos = ChunkPos::new(chunk_x, chunk_z);

            if chunk_pos == chunk_pos_a {
                let local_x = wx - origin_a_x;
                let local_z = wz - origin_a_z;
                if !(0..CHUNK_SIZE_X as i32).contains(&local_x)
                    || !(0..CHUNK_SIZE_Z as i32).contains(&local_z)
                {
                    return None;
                }
                return Some(chunk_a.voxel(local_x as usize, local_y, local_z as usize));
            }

            if chunk_pos == chunk_pos_b {
                let local_x = wx - origin_b_x;
                let local_z = wz - origin_b_z;
                if !(0..CHUNK_SIZE_X as i32).contains(&local_x)
                    || !(0..CHUNK_SIZE_Z as i32).contains(&local_z)
                {
                    return None;
                }
                return Some(chunk_b.voxel(local_x as usize, local_y, local_z as usize));
            }

            None
        });

        assert_eq!(mesh_disconnected.indices_alpha.len(), 12);
        assert_eq!(mesh_connected.indices_alpha.len(), 12);

        let half_thickness = (2.0 / 16.0) * 0.5;
        let expected_center_x = 8.0 + 0.5;
        let expected_west_x = expected_center_x - half_thickness;
        let expected_east_x = expected_center_x + half_thickness;

        let check_west_east = |mesh: &MeshBuffers| {
            let west_vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| {
                    v.block_id == mdminecraft_world::BLOCK_NETHER_PORTAL
                        && v.normal == [-1.0, 0.0, 0.0]
                })
                .collect();
            let east_vertices: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| {
                    v.block_id == mdminecraft_world::BLOCK_NETHER_PORTAL
                        && v.normal == [1.0, 0.0, 0.0]
                })
                .collect();

            assert_eq!(west_vertices.len(), 4);
            assert_eq!(east_vertices.len(), 4);

            for v in west_vertices {
                assert!(
                    (v.position[0] - expected_west_x).abs() < 0.0001,
                    "Expected west portal plane x={expected_west_x}, got {}",
                    v.position[0]
                );
            }
            for v in east_vertices {
                assert!(
                    (v.position[0] - expected_east_x).abs() < 0.0001,
                    "Expected east portal plane x={expected_east_x}, got {}",
                    v.position[0]
                );
            }
        };

        check_west_east(&mesh_disconnected);
        check_west_east(&mesh_connected);
    }

    #[test]
    fn biome_tint_extra_bits_are_emitted_for_tagged_blocks() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_biome_tints();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: 1,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            3,
            1,
            1,
            Voxel {
                id: 2,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(!mesh.vertices.is_empty());

        let tint_mask = 0x7u8 << EXTRA_TINT_SHIFT;

        let grass_top: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == 1 && v.normal == [0.0, 1.0, 0.0])
            .collect();
        assert_eq!(grass_top.len(), 4);
        for v in grass_top {
            assert_eq!(
                (v.extra >> EXTRA_TINT_SHIFT) & 0x7,
                1,
                "Expected grass top vertices to be marked with the grass tint"
            );
        }

        let grass_bottom: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == 1 && v.normal == [0.0, -1.0, 0.0])
            .collect();
        assert_eq!(grass_bottom.len(), 4);
        for v in grass_bottom {
            assert_eq!(
                v.extra & tint_mask,
                0,
                "Expected grass bottom vertices to have no biome tint"
            );
        }

        let grass_sides: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == 1 && v.normal[1].abs() < f32::EPSILON)
            .collect();
        assert_eq!(grass_sides.len(), 16);
        for v in grass_sides {
            assert_eq!(
                (v.extra >> EXTRA_TINT_SHIFT) & 0x7,
                3,
                "Expected grass side vertices to be marked with the grass-side tint"
            );
        }

        let leaves_vertices: Vec<_> = mesh.vertices.iter().filter(|v| v.block_id == 2).collect();
        assert_eq!(leaves_vertices.len(), 24);
        for v in leaves_vertices {
            assert_eq!(
                (v.extra >> EXTRA_TINT_SHIFT) & 0x7,
                2,
                "Expected leaves vertices to be marked with the foliage tint"
            );
        }
    }

    #[test]
    fn adjacent_same_level_water_has_no_internal_face() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_fluids();

        let state = set_fluid_level(0, 7);
        let voxel = Voxel {
            id: BLOCK_WATER_FLOWING,
            state,
            light_sky: 15,
            light_block: 0,
        };

        chunk.set_voxel(1, 1, 1, voxel);
        chunk.set_voxel(2, 1, 1, voxel);

        let mesh = mesh_chunk(&chunk, &registry, None);
        let internal_face_vertices = mesh
            .vertices
            .iter()
            .filter(|v| {
                v.block_id == BLOCK_WATER_FLOWING
                    && (v.normal == [1.0, 0.0, 0.0] || v.normal == [-1.0, 0.0, 0.0])
                    && (v.position[0] - 2.0).abs() < 0.0001
            })
            .count();

        assert_eq!(
            internal_face_vertices, 0,
            "Expected no internal side faces between same-height adjacent water blocks"
        );
    }

    #[test]
    fn flowing_water_slopes_toward_higher_neighbor() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_fluids();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: BLOCK_WATER,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            2,
            1,
            1,
            Voxel {
                id: BLOCK_WATER_FLOWING,
                state: set_fluid_level(0, 7),
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        let top_vertices: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == BLOCK_WATER_FLOWING && v.normal == [0.0, 1.0, 0.0])
            .collect();

        assert_eq!(top_vertices.len(), 4);

        let expected_high_y = 2.0;
        let expected_low_y = 1.0 + 7.0 / 8.0;

        let mut saw_high = false;
        let mut saw_low = false;

        for vertex in top_vertices {
            if (vertex.position[0] - 2.0).abs() < 0.0001 {
                saw_high |= (vertex.position[1] - expected_high_y).abs() < 0.0001;
            } else if (vertex.position[0] - 3.0).abs() < 0.0001 {
                saw_low |= (vertex.position[1] - expected_low_y).abs() < 0.0001;
            }
        }

        assert!(
            saw_high && saw_low,
            "Expected sloped top face with heights {expected_high_y} (near source) and {expected_low_y} (far side)"
        );
    }

    #[test]
    fn snow_layer_mesh_height_matches_layers() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_snow();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: BLOCK_SNOW,
                state: mdminecraft_world::set_snow_layers(0, 3),
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        let top_vertices: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == BLOCK_SNOW && v.normal == [0.0, 1.0, 0.0])
            .collect();

        assert_eq!(top_vertices.len(), 4);

        let expected_y = 1.0 + 3.0 / 8.0;
        for vertex in top_vertices {
            assert!(
                (vertex.position[1] - expected_y).abs() < 0.0001,
                "Expected snow layer top at y={expected_y}, got y={}",
                vertex.position[1]
            );
        }
    }

    #[test]
    fn ice_is_treated_as_translucent_for_face_culling() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry_with_fluids();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_ICE,
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );
        chunk.set_voxel(
            1,
            1,
            2,
            Voxel {
                id: 1, // stone
                state: 0,
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        let north_face_vertices = mesh
            .vertices
            .iter()
            .filter(|v| {
                v.block_id == 1
                    && v.normal == [0.0, 0.0, -1.0]
                    && (v.position[2] - 2.0).abs() < 0.0001
            })
            .count();
        assert_eq!(
            north_face_vertices, 4,
            "Expected the stone face adjacent to ice to be rendered"
        );
    }

    #[test]
    fn waterlogged_slab_emits_water_faces() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        chunk.set_voxel(
            1,
            1,
            1,
            Voxel {
                id: interactive_blocks::STONE_SLAB,
                state: set_waterlogged(0, true),
                light_sky: 15,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        let water_vertices = mesh
            .vertices
            .iter()
            .filter(|v| v.block_id == BLOCK_WATER)
            .count();
        assert_eq!(
            water_vertices, 20,
            "Expected 5 water faces (20 vertices) for a waterlogged slab"
        );
    }

    #[test]
    fn glass_pane_connects_across_chunk_seam_with_sampler() {
        let pane_id = interactive_blocks::GLASS_PANE as usize;
        let mut descriptors = Vec::with_capacity(pane_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for _ in 1..pane_id {
            descriptors.push(BlockDescriptor::simple("solid", true));
        }
        descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
            name: "glass_pane".to_string(),
            key: None,
            tags: vec!["render/translucent".to_string()],
            opaque: false,
            light_opacity: None,
            light_emission: None,
            emissive: None,
            texture: None,
            textures: None,
            harvest_level: None,
        }));
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        chunk_a.set_voxel(
            15,
            1,
            1,
            Voxel {
                id: interactive_blocks::GLASS_PANE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk_b.set_voxel(
            0,
            1,
            1,
            Voxel {
                id: interactive_blocks::GLASS_PANE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |_wx, _wy, _wz| None);

        let pane_world_y = local_y_to_world_y(1);
        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            if wx == 16 && wy == pane_world_y && wz == 1 {
                Some(chunk_b.voxel(0, 1, 1))
            } else {
                None
            }
        });

        assert_eq!(
            mesh_disconnected.indices_alpha.len(),
            36,
            "Disconnected pane should render only the center post"
        );
        assert_eq!(
            mesh_connected.indices_alpha.len(),
            72,
            "Connected pane should render an extra arm (one more box)"
        );
    }

    #[test]
    fn iron_bars_connect_across_chunk_seam_with_sampler() {
        let bars_id = interactive_blocks::IRON_BARS as usize;
        let mut descriptors = Vec::with_capacity(bars_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for _ in 1..bars_id {
            descriptors.push(BlockDescriptor::simple("solid", true));
        }
        descriptors.push(BlockDescriptor::simple("iron_bars", false));
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        chunk_a.set_voxel(
            15,
            1,
            1,
            Voxel {
                id: interactive_blocks::IRON_BARS,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk_b.set_voxel(
            0,
            1,
            1,
            Voxel {
                id: interactive_blocks::IRON_BARS,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |_wx, _wy, _wz| None);

        let bars_world_y = local_y_to_world_y(1);
        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            if wx == 16 && wy == bars_world_y && wz == 1 {
                Some(chunk_b.voxel(0, 1, 1))
            } else {
                None
            }
        });

        assert_eq!(
            mesh_disconnected.indices_opaque.len(),
            36,
            "Disconnected bars should render only the center post"
        );
        assert_eq!(
            mesh_connected.indices_opaque.len(),
            72,
            "Connected bars should render an extra arm (one more box)"
        );
    }

    #[test]
    fn glass_pane_connects_to_iron_bars_across_chunk_seam_with_sampler() {
        let max_id = interactive_blocks::IRON_BARS as usize;
        let mut descriptors = Vec::with_capacity(max_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for id in 1..=max_id {
            if id == interactive_blocks::GLASS_PANE as usize {
                descriptors.push(BlockDescriptor::from_definition(BlockDefinition {
                    name: "glass_pane".to_string(),
                    key: None,
                    tags: vec!["render/translucent".to_string()],
                    opaque: false,
                    light_opacity: None,
                    light_emission: None,
                    emissive: None,
                    texture: None,
                    textures: None,
                    harvest_level: None,
                }));
            } else if id == interactive_blocks::IRON_BARS as usize {
                descriptors.push(BlockDescriptor::simple("iron_bars", false));
            } else {
                descriptors.push(BlockDescriptor::simple("solid", true));
            }
        }
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        chunk_a.set_voxel(
            15,
            1,
            1,
            Voxel {
                id: interactive_blocks::GLASS_PANE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        chunk_b.set_voxel(
            0,
            1,
            1,
            Voxel {
                id: interactive_blocks::IRON_BARS,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |_wx, _wy, _wz| None);

        let pane_world_y = local_y_to_world_y(1);
        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            if wx == 16 && wy == pane_world_y && wz == 1 {
                Some(chunk_b.voxel(0, 1, 1))
            } else {
                None
            }
        });

        assert_eq!(
            mesh_disconnected.indices_alpha.len(),
            36,
            "Disconnected pane should render only the center post"
        );
        assert_eq!(
            mesh_connected.indices_alpha.len(),
            72,
            "Pane should connect to adjacent bars and render one extra arm"
        );
    }

    #[test]
    fn oak_fence_connects_across_chunk_seam_with_sampler() {
        let fence_id = interactive_blocks::OAK_FENCE as usize;
        let mut descriptors = Vec::with_capacity(fence_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for _ in 1..fence_id {
            descriptors.push(BlockDescriptor::simple("solid", true));
        }
        descriptors.push(BlockDescriptor::simple("oak_fence", false));
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        let fence_voxel = Voxel {
            id: interactive_blocks::OAK_FENCE,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };

        chunk_a.set_voxel(15, 1, 1, fence_voxel);
        chunk_b.set_voxel(0, 1, 1, fence_voxel);

        let origin_a_x = chunk_a.position().x * CHUNK_SIZE_X as i32;
        let origin_a_z = chunk_a.position().z * CHUNK_SIZE_Z as i32;
        let origin_b_x = chunk_b.position().x * CHUNK_SIZE_X as i32;
        let origin_b_z = chunk_b.position().z * CHUNK_SIZE_Z as i32;

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
                let local_y = world_y_to_local_y(wy)?;

                let ax = wx - origin_a_x;
                let az = wz - origin_a_z;
                if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az)
                {
                    return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
                }

                None
            });

        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            let local_y = world_y_to_local_y(wy)?;

            let ax = wx - origin_a_x;
            let az = wz - origin_a_z;
            if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az) {
                return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
            }

            let bx = wx - origin_b_x;
            let bz = wz - origin_b_z;
            if (0..CHUNK_SIZE_X as i32).contains(&bx) && (0..CHUNK_SIZE_Z as i32).contains(&bz) {
                return Some(chunk_b.voxel(bx as usize, local_y, bz as usize));
            }

            None
        });

        assert_eq!(
            mesh_disconnected.indices_opaque.len(),
            36,
            "Disconnected fence should render only the center post"
        );
        assert_eq!(
            mesh_connected.indices_opaque.len(),
            108,
            "Connected fence should render a post plus two rail boxes"
        );
    }

    #[test]
    fn cobblestone_wall_connects_across_chunk_seam_with_sampler() {
        let wall_id = interactive_blocks::COBBLESTONE_WALL as usize;
        let mut descriptors = Vec::with_capacity(wall_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for _ in 1..wall_id {
            descriptors.push(BlockDescriptor::simple("solid", true));
        }
        descriptors.push(BlockDescriptor::simple("cobblestone_wall", false));
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        let wall_voxel = Voxel {
            id: interactive_blocks::COBBLESTONE_WALL,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };

        chunk_a.set_voxel(15, 1, 1, wall_voxel);
        chunk_b.set_voxel(0, 1, 1, wall_voxel);

        let origin_a_x = chunk_a.position().x * CHUNK_SIZE_X as i32;
        let origin_a_z = chunk_a.position().z * CHUNK_SIZE_Z as i32;
        let origin_b_x = chunk_b.position().x * CHUNK_SIZE_X as i32;
        let origin_b_z = chunk_b.position().z * CHUNK_SIZE_Z as i32;

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
                let local_y = world_y_to_local_y(wy)?;

                let ax = wx - origin_a_x;
                let az = wz - origin_a_z;
                if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az)
                {
                    return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
                }

                None
            });

        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            let local_y = world_y_to_local_y(wy)?;

            let ax = wx - origin_a_x;
            let az = wz - origin_a_z;
            if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az) {
                return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
            }

            let bx = wx - origin_b_x;
            let bz = wz - origin_b_z;
            if (0..CHUNK_SIZE_X as i32).contains(&bx) && (0..CHUNK_SIZE_Z as i32).contains(&bz) {
                return Some(chunk_b.voxel(bx as usize, local_y, bz as usize));
            }

            None
        });

        assert_eq!(
            mesh_disconnected.indices_opaque.len(),
            36,
            "Disconnected wall should render only the center post"
        );
        assert_eq!(
            mesh_connected.indices_opaque.len(),
            72,
            "Connected wall should render a post plus one arm"
        );
    }

    #[test]
    fn stone_brick_wall_connects_across_chunk_seam_with_sampler() {
        let wall_id = interactive_blocks::STONE_BRICK_WALL as usize;
        let mut descriptors = Vec::with_capacity(wall_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for _ in 1..wall_id {
            descriptors.push(BlockDescriptor::simple("solid", true));
        }
        descriptors.push(BlockDescriptor::simple("stone_brick_wall", false));
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        let wall_voxel = Voxel {
            id: interactive_blocks::STONE_BRICK_WALL,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };

        chunk_a.set_voxel(15, 1, 1, wall_voxel);
        chunk_b.set_voxel(0, 1, 1, wall_voxel);

        let origin_a_x = chunk_a.position().x * CHUNK_SIZE_X as i32;
        let origin_a_z = chunk_a.position().z * CHUNK_SIZE_Z as i32;
        let origin_b_x = chunk_b.position().x * CHUNK_SIZE_X as i32;
        let origin_b_z = chunk_b.position().z * CHUNK_SIZE_Z as i32;

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
                let local_y = world_y_to_local_y(wy)?;

                let ax = wx - origin_a_x;
                let az = wz - origin_a_z;
                if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az)
                {
                    return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
                }

                None
            });

        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            let local_y = world_y_to_local_y(wy)?;

            let ax = wx - origin_a_x;
            let az = wz - origin_a_z;
            if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az) {
                return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
            }

            let bx = wx - origin_b_x;
            let bz = wz - origin_b_z;
            if (0..CHUNK_SIZE_X as i32).contains(&bx) && (0..CHUNK_SIZE_Z as i32).contains(&bz) {
                return Some(chunk_b.voxel(bx as usize, local_y, bz as usize));
            }

            None
        });

        assert_eq!(
            mesh_disconnected.indices_opaque.len(),
            36,
            "Disconnected wall should render only the center post"
        );
        assert_eq!(
            mesh_connected.indices_opaque.len(),
            72,
            "Connected wall should render a post plus one arm"
        );
    }

    #[test]
    fn redstone_wire_connects_across_chunk_seam_with_sampler() {
        let wire_id = mdminecraft_world::redstone_blocks::REDSTONE_WIRE as usize;
        let mut descriptors = Vec::with_capacity(wire_id + 1);
        descriptors.push(BlockDescriptor::simple("air", false));
        for _ in 1..wire_id {
            descriptors.push(BlockDescriptor::simple("solid", true));
        }
        descriptors.push(BlockDescriptor::simple("redstone_wire", false));
        let registry = BlockRegistry::new(descriptors);

        let mut chunk_a = Chunk::new(ChunkPos::new(0, 0));
        let mut chunk_b = Chunk::new(ChunkPos::new(1, 0));

        let wire_voxel = Voxel {
            id: mdminecraft_world::redstone_blocks::REDSTONE_WIRE,
            state: 0,
            light_sky: 0,
            light_block: 0,
        };

        // Wire at x=15 has an internal +Z neighbor; the seam +X neighbor is only visible to the sampler.
        chunk_a.set_voxel(15, 1, 1, wire_voxel);
        chunk_a.set_voxel(15, 1, 2, wire_voxel);
        chunk_b.set_voxel(0, 1, 1, wire_voxel);

        let origin_a_x = chunk_a.position().x * CHUNK_SIZE_X as i32;
        let origin_a_z = chunk_a.position().z * CHUNK_SIZE_Z as i32;
        let origin_b_x = chunk_b.position().x * CHUNK_SIZE_X as i32;
        let origin_b_z = chunk_b.position().z * CHUNK_SIZE_Z as i32;

        let mesh_disconnected =
            mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
                let local_y = world_y_to_local_y(wy)?;

                let ax = wx - origin_a_x;
                let az = wz - origin_a_z;
                if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az)
                {
                    return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
                }

                None
            });

        let mesh_connected = mesh_chunk_with_voxel_at(&chunk_a, &registry, None, |wx, wy, wz| {
            let local_y = world_y_to_local_y(wy)?;

            let ax = wx - origin_a_x;
            let az = wz - origin_a_z;
            if (0..CHUNK_SIZE_X as i32).contains(&ax) && (0..CHUNK_SIZE_Z as i32).contains(&az) {
                return Some(chunk_a.voxel(ax as usize, local_y, az as usize));
            }

            let bx = wx - origin_b_x;
            let bz = wz - origin_b_z;
            if (0..CHUNK_SIZE_X as i32).contains(&bx) && (0..CHUNK_SIZE_Z as i32).contains(&bz) {
                return Some(chunk_b.voxel(bx as usize, local_y, bz as usize));
            }

            None
        });

        assert_eq!(
            mesh_disconnected.indices_opaque.len(),
            72,
            "Disconnected wires should render two segments (one per voxel)"
        );
        assert_eq!(
            mesh_connected.indices_opaque.len(),
            108,
            "Connected seam adds an extra segment on the edge wire"
        );
    }

    #[test]
    fn enchanting_table_renders_as_partial_height() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        chunk.set_voxel(
            1,
            0,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_ENCHANTING_TABLE,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(
            !mesh.vertices.is_empty(),
            "Enchanting table should generate vertices"
        );

        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_y, 12.0 / 16.0);
    }

    #[test]
    fn brewing_stand_renders_as_partial_height() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let registry = registry();

        chunk.set_voxel(
            1,
            0,
            1,
            Voxel {
                id: mdminecraft_world::BLOCK_BREWING_STAND,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );

        let mesh = mesh_chunk(&chunk, &registry, None);
        assert!(
            !mesh.vertices.is_empty(),
            "Brewing stand should generate vertices"
        );

        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max_y, 14.0 / 16.0);
    }
}
