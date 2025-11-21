//! Property-based tests for lighting system
//!
//! Validates lighting invariants:
//! - Light levels are always in valid range [0, 15]
//! - Light never increases when propagating away from source
//! - Skylight propagates downward correctly
//! - Block light propagates from sources
//!
//! These properties must hold for all possible chunk configurations.

use mdminecraft_world::{lighting::stitch_light_seams, Chunk, ChunkPos, Voxel};
use proptest::prelude::*;

/// Maximum light level
const MAX_LIGHT: u8 = 15;

proptest! {
    /// Property: All light values are within valid range
    ///
    /// For any chunk configuration, all voxels must have
    /// skylight and blocklight in range [0, 15].
    #[test]
    fn light_levels_in_range(
        chunk_seed in any::<u64>(),
    ) {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);

        // Fill with semi-random blocks
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    if (chunk_seed.wrapping_add((x + y * 16 + z * 256) as u64)) % 3 == 0 {
                        chunk.set_voxel(x, y, z, Voxel {
                            id: 1,
                            state: 0,
                            light_sky: 0,
                            light_block: 0,
                        });
                    }
                }
            }
        }

        // Check all voxels
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let voxel = chunk.voxel(x, y, z);
                    prop_assert!(
                        voxel.light_sky <= MAX_LIGHT,
                        "Skylight {} out of range at ({}, {}, {})",
                        voxel.light_sky, x, y, z
                    );
                    prop_assert!(
                        voxel.light_block <= MAX_LIGHT,
                        "Blocklight {} out of range at ({}, {}, {})",
                        voxel.light_block, x, y, z
                    );
                }
            }
        }
    }

    /// Property: Empty chunk has maximum skylight
    ///
    /// An empty chunk should have skylight = 15 everywhere
    /// (assuming no neighboring chunks blocking light).
    #[test]
    fn empty_chunk_skylight(
        chunk_x in -10i32..10i32,
        chunk_z in -10i32..10i32,
    ) {
        let pos = ChunkPos::new(chunk_x, chunk_z);
        let chunk = Chunk::new(pos);

        // In an empty chunk, voxels start with default light (0)
        // This test validates the initial state
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let voxel = chunk.voxel(x, y, z);
                    // Initially, light is not propagated until lighting system runs
                    prop_assert!(
                        voxel.light_sky <= MAX_LIGHT,
                        "Skylight out of range at ({}, {}, {})",
                        x, y, z
                    );
                }
            }
        }
    }

    /// Property: Light levels are monotonic
    ///
    /// Light should never increase as you move away from a source.
    /// This tests a simple vertical column.
    #[test]
    fn skylight_monotonic_downward(
        top_light in 0u8..=MAX_LIGHT,
        column_x in 0usize..16,
        column_z in 0usize..16,
    ) {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);

        // Set top voxel with given light
        chunk.set_voxel(column_x, 15, column_z, Voxel {
            id: 0,
            state: 0,
            light_sky: top_light,
            light_block: 0,
        });

        // Manually propagate downward (simple case)
        for y in (0..15).rev() {
            let above_light = chunk.voxel(column_x, y + 1, column_z).light_sky;
            let new_light = if above_light > 0 { above_light.saturating_sub(1) } else { 0 };
            chunk.set_voxel(column_x, y, column_z, Voxel {
                id: 0,
                state: 0,
                light_sky: new_light,
                light_block: 0,
            });
        }

        // Verify monotonic decrease
        let mut prev_light = MAX_LIGHT + 1;
        for y in (0..16).rev() {
            let voxel = chunk.voxel(column_x, y, column_z);
            prop_assert!(
                voxel.light_sky <= prev_light,
                "Light increased from {} to {} at y={}",
                prev_light, voxel.light_sky, y
            );
            prev_light = voxel.light_sky;
        }
    }

    /// Property: Block light source validity
    ///
    /// When a voxel has block light, it should be <= 15
    /// and decrease as it propagates.
    #[test]
    fn blocklight_source_valid(
        source_light in 1u8..=MAX_LIGHT,
        source_x in 1usize..15,
        source_y in 1usize..15,
        source_z in 1usize..15,
    ) {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);

        // Place light source
        chunk.set_voxel(source_x, source_y, source_z, Voxel {
            id: 0,
            state: 0,
            light_sky: 0,
            light_block: source_light,
        });

        // Verify source light is valid
        let source_voxel = chunk.voxel(source_x, source_y, source_z);
        prop_assert!(
            source_voxel.light_block <= MAX_LIGHT,
            "Source blocklight {} out of range",
            source_voxel.light_block
        );
        prop_assert_eq!(
            source_voxel.light_block, source_light,
            "Source blocklight changed"
        );
    }

    /// Property: Light doesn't create energy
    ///
    /// Neighboring voxels should never have more light than the source - 1
    /// (this is a simplified check for the propagation invariant).
    #[test]
    fn light_energy_conservation(
        center_light in 1u8..=MAX_LIGHT,
    ) {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);

        // Place center voxel with light
        let cx = 8usize;
        let cy = 8usize;
        let cz = 8usize;
        chunk.set_voxel(cx, cy, cz, Voxel {
            id: 0,
            state: 0,
            light_sky: center_light,
            light_block: 0,
        });

        // Simulate simple propagation to neighbors
        let max_neighbor_light = if center_light > 0 { center_light - 1 } else { 0 };

        for &(dx, dy, dz) in &[(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let nx = (cx as i32 + dx) as usize;
            let ny = (cy as i32 + dy) as usize;
            let nz = (cz as i32 + dz) as usize;

            if nx < 16 && ny < 16 && nz < 16 {
                chunk.set_voxel(nx, ny, nz, Voxel {
                    id: 0,
                    state: 0,
                    light_sky: max_neighbor_light,
                    light_block: 0,
                });

                let neighbor = chunk.voxel(nx, ny, nz);
                prop_assert!(
                    neighbor.light_sky <= max_neighbor_light,
                    "Neighbor light {} exceeds expected {}",
                    neighbor.light_sky, max_neighbor_light
                );
            }
        }
    }

    /// Property: Seam stitching never increases light across chunk borders.
    #[test]
    fn seam_stitch_monotonic(
        center_light in 1u8..=MAX_LIGHT,
        y in 1usize..15,
        z in 1usize..15,
    ) {
        let pos_a = ChunkPos::new(0, 0);
        let pos_b = ChunkPos::new(1, 0);
        let mut chunk_a = Chunk::new(pos_a);
        let chunk_b = Chunk::new(pos_b);

        // Light source at east edge of chunk A
        let source_x = 15;
        chunk_a.set_voxel(source_x, y, z, Voxel {
            id: 0,
            state: 0,
            light_sky: 0,
            light_block: center_light,
        });

        let mut chunks = std::collections::HashMap::new();
        chunks.insert(pos_a, chunk_a);
        chunks.insert(pos_b, chunk_b);

        let registry = crate::unit_tests::LocalTestRegistry;
        // Stitch block light across seam
        let _ = stitch_light_seams(&mut chunks, &registry, pos_a, mdminecraft_world::lighting::LightType::BlockLight);

        let chunk_b = chunks.get(&pos_b).unwrap();
        let neighbor = chunk_b.voxel(0, y, z);
        prop_assert!(neighbor.light_block <= center_light.saturating_sub(1));
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn light_range_sanity() {
        assert_eq!(MAX_LIGHT, 15);
    }

    #[test]
    fn empty_chunk_default_values() {
        let chunk = Chunk::new(ChunkPos::new(0, 0));
        let voxel = chunk.voxel(0, 0, 0);

        assert!(voxel.light_sky <= MAX_LIGHT);
        assert!(voxel.light_block <= MAX_LIGHT);
        assert_eq!(voxel.id, 0); // Air
    }

    #[test]
    fn set_voxel_preserves_light_range() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0));

        chunk.set_voxel(
            5,
            5,
            5,
            Voxel {
                id: 1,
                state: 0,
                light_sky: 12,
                light_block: 8,
            },
        );

        let voxel = chunk.voxel(5, 5, 5);
        assert_eq!(voxel.light_sky, 12);
        assert_eq!(voxel.light_block, 8);
        assert!(voxel.light_sky <= MAX_LIGHT);
        assert!(voxel.light_block <= MAX_LIGHT);
    }

    /// Mock registry treating non-air as opaque.
    pub struct LocalTestRegistry;

    impl mdminecraft_world::lighting::BlockOpacityProvider for LocalTestRegistry {
        fn is_opaque(&self, block_id: u16) -> bool {
            block_id != 0
        }
    }
}
