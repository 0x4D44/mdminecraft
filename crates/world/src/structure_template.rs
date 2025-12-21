use crate::chunk::{Chunk, Voxel};
use crate::structures::set_world_voxel_if_in_chunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum YRotation {
    R0,
    R90,
    R180,
    R270,
}

impl YRotation {
    pub(crate) const fn rotated_size_xz(self, size_x: usize, size_z: usize) -> (usize, usize) {
        match self {
            Self::R0 | Self::R180 => (size_x, size_z),
            Self::R90 | Self::R270 => (size_z, size_x),
        }
    }

    pub(crate) const fn rotate_xz(
        self,
        x: usize,
        z: usize,
        size_x: usize,
        size_z: usize,
    ) -> (usize, usize) {
        match self {
            Self::R0 => (x, z),
            Self::R90 => (size_z - 1 - z, x),
            Self::R180 => (size_x - 1 - x, size_z - 1 - z),
            Self::R270 => (z, size_x - 1 - x),
        }
    }
}

/// Place an ASCII volume template into the given chunk.
///
/// `layers[y][z]` is a string of length `size_x` (bytes), and every layer must share the same
/// dimensions.
///
/// The palette closure returns `Some(voxel)` to set that voxel, or `None` to leave the world
/// untouched at that position.
pub(crate) fn place_ascii_volume_in_chunk(
    chunk: &mut Chunk,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    rotation: YRotation,
    layers: &[&[&str]],
    mut voxel_for_byte: impl FnMut(u8) -> Option<Voxel>,
) {
    let size_y = layers.len();
    if size_y == 0 {
        return;
    }

    let size_z = layers[0].len();
    if size_z == 0 {
        return;
    }

    let size_x = layers[0][0].len();
    if size_x == 0 {
        return;
    }

    debug_assert!(
        layers.iter().all(|layer| layer.len() == size_z),
        "all template layers must have consistent z dimensions"
    );
    debug_assert!(
        layers
            .iter()
            .all(|layer| layer.iter().all(|row| row.len() == size_x)),
        "all template rows must have consistent x dimensions"
    );

    let (rot_x, rot_z) = rotation.rotated_size_xz(size_x, size_z);

    for (dy, layer) in layers.iter().enumerate() {
        let world_y = origin_y + dy as i32;

        for (z, row) in layer.iter().enumerate() {
            let bytes = row.as_bytes();
            debug_assert!(
                bytes.len() == size_x,
                "template row must have consistent length"
            );

            for (x, byte) in bytes.iter().enumerate() {
                let Some(voxel) = voxel_for_byte(*byte) else {
                    continue;
                };

                let (rx, rz) = rotation.rotate_xz(x, z, size_x, size_z);
                debug_assert!(
                    rx < rot_x && rz < rot_z,
                    "rotated coordinate must be in bounds"
                );

                set_world_voxel_if_in_chunk(
                    chunk,
                    origin_x + rx as i32,
                    world_y,
                    origin_z + rz as i32,
                    voxel,
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

    fn local_y(world_y: i32) -> usize {
        crate::chunk::world_y_to_local_y(world_y).expect("world y in bounds")
    }

    const LAYER0: [&str; 2] = ["ab", "cd"];
    const VOLUME: [&[&str]; 1] = [&LAYER0];

    fn voxel_for_byte(byte: u8) -> Option<Voxel> {
        let id = match byte {
            b'a' => 1,
            b'b' => 2,
            b'c' => 3,
            b'd' => 4,
            _ => return None,
        };
        Some(Voxel {
            id,
            ..Default::default()
        })
    }

    #[test]
    fn template_rotation_maps_expected_positions() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        place_ascii_volume_in_chunk(
            &mut chunk,
            0,
            64,
            0,
            YRotation::R90,
            &VOLUME,
            voxel_for_byte,
        );

        // Original:
        // z=0: a b
        // z=1: c d
        //
        // Rot90 should produce:
        // z'=0: c a
        // z'=1: d b
        let positions = [
            (0, 64, 0, 3), // c
            (1, 64, 0, 1), // a
            (0, 64, 1, 4), // d
            (1, 64, 1, 2), // b
        ];

        for (wx, wy, wz, id) in positions {
            let Some((lx, lz)) = world_to_chunk_local(chunk.position(), wx, wz) else {
                panic!("expected ({wx},{wz}) to be in chunk");
            };
            assert_eq!(
                chunk.voxel(lx, local_y(wy), lz).id,
                id,
                "mismatch at ({wx},{wy},{wz})"
            );
        }
    }
}
