use mdminecraft_core::DimensionId;
use mdminecraft_world::{
    fortress_blaze_spawns_for_chunk, ChunkPos, MobType, TerrainGenerator, BLOCK_AIR,
    world_y_to_local_y, BLOCK_STONE_BRICKS, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};

#[test]
fn nether_fortress_blaze_spawns_align_with_generated_corridor() {
    let world_seed = 0x2E2C_AE3D_7B53_D2C1_u64;

    let mut found: Option<(ChunkPos, Vec<mdminecraft_world::Mob>)> = None;
    'search: for chunk_z in -48..=48 {
        for chunk_x in -48..=48 {
            let chunk_pos = ChunkPos::new(chunk_x, chunk_z);
            let spawns_a = fortress_blaze_spawns_for_chunk(world_seed, chunk_pos);
            if spawns_a.is_empty() {
                continue;
            }

            let spawns_b = fortress_blaze_spawns_for_chunk(world_seed, chunk_pos);
            assert_eq!(spawns_a.len(), spawns_b.len());
            for (a, b) in spawns_a.iter().zip(spawns_b.iter()) {
                assert_eq!(a.mob_type, MobType::Blaze);
                assert_eq!(a.mob_type, b.mob_type);
                assert_eq!(a.x, b.x);
                assert_eq!(a.y, b.y);
                assert_eq!(a.z, b.z);
            }

            found = Some((chunk_pos, spawns_a));
            break 'search;
        }
    }

    let (chunk_pos, spawns) = found.expect("expected fortress blaze spawns in search window");

    let terrain_gen = TerrainGenerator::new(world_seed);
    let chunk = terrain_gen.generate_chunk_in_dimension(DimensionId::Nether, chunk_pos);

    let mut found_stone_bricks = false;
    'scan: for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                if chunk.voxel(x, y, z).id == BLOCK_STONE_BRICKS {
                    found_stone_bricks = true;
                    break 'scan;
                }
            }
        }
    }
    assert!(
        found_stone_bricks,
        "expected fortress corridor stone bricks"
    );

    for blaze in spawns {
        let world_x = blaze.x.floor() as i32;
        let world_y = blaze.y.floor() as i32;
        let world_z = blaze.z.floor() as i32;

        let local_x = world_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_y = world_y_to_local_y(world_y).expect("blaze spawn y within chunk bounds");
        let local_z = world_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

        let voxel = chunk.voxel(local_x, local_y, local_z);
        assert_eq!(
            voxel.id, BLOCK_AIR,
            "blaze spawn must be inside corridor air"
        );

        let below = chunk.voxel(local_x, local_y.saturating_sub(1), local_z);
        assert_eq!(
            below.id, BLOCK_STONE_BRICKS,
            "blaze spawn must have corridor floor"
        );
    }
}
