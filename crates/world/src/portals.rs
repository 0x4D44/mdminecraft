use crate::chunk::{
    world_y_to_local_y, Chunk, ChunkPos, Voxel, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Z,
};
use crate::{BlockId, BLOCK_END_PORTAL};
use std::collections::HashMap;

/// Place a simple 3×3 End exit portal centered at the given world position.
///
/// This is a vanilla-inspired convenience for the End boss loop. The portal is
/// purely functional (non-solid) and is safe to call repeatedly.
///
/// Returns the list of changed world positions.
pub fn place_end_exit_portal(
    chunks: &mut HashMap<ChunkPos, Chunk>,
    center_x: i32,
    center_y: i32,
    center_z: i32,
) -> Option<Vec<(i32, i32, i32)>> {
    let _center_local_y = world_y_to_local_y(center_y)?;

    let chunk_and_local = |x: i32, y: i32, z: i32| -> Option<(ChunkPos, usize, usize, usize)> {
        let local_y = world_y_to_local_y(y)?;
        let chunk_pos = ChunkPos::new(
            x.div_euclid(CHUNK_SIZE_X as i32),
            z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let local_x = x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        Some((chunk_pos, local_x, local_y, local_z))
    };

    let voxel_id_at = |x: i32, y: i32, z: i32| -> Option<BlockId> {
        let (chunk_pos, local_x, local_y, local_z) = chunk_and_local(x, y, z)?;
        Some(chunks.get(&chunk_pos)?.voxel(local_x, local_y, local_z).id)
    };

    // Only place into air (or re-place existing portal blocks).
    for dx in -1..=1 {
        for dz in -1..=1 {
            let x = center_x + dx;
            let z = center_z + dz;
            match voxel_id_at(x, center_y, z)? {
                BLOCK_AIR | BLOCK_END_PORTAL => {}
                _ => return None,
            }
        }
    }

    let mut changed = Vec::with_capacity(9);
    for dx in -1..=1 {
        for dz in -1..=1 {
            let x = center_x + dx;
            let z = center_z + dz;
            let (chunk_pos, local_x, local_y, local_z) = chunk_and_local(x, center_y, z)?;
            let chunk = chunks.get_mut(&chunk_pos)?;
            chunk.set_voxel(
                local_x,
                local_y,
                local_z,
                Voxel {
                    id: BLOCK_END_PORTAL,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                },
            );
            changed.push((x, center_y, z));
        }
    }

    Some(changed)
}
