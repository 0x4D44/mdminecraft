//! Sugar cane growth and placement helpers.
//!
//! Sugar cane is a vertical plant that grows next to water.
//! We implement deterministic growth with a per-tick RNG stream similar to crops.

use crate::chunk::{
    Chunk, ChunkPos, Voxel, BLOCK_AIR, BLOCK_DIRT, BLOCK_GRASS, BLOCK_SAND, BLOCK_SUGAR_CANE,
    BLOCK_WATER, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use crate::fluid::BLOCK_WATER_FLOWING;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{BTreeSet, HashMap};

/// A chunk-local sugar cane base position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SugarCanePosition {
    pub chunk: ChunkPos,
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

/// Deterministic sugar cane growth system.
pub struct SugarCaneGrowthSystem {
    world_seed: u64,
    base_positions: BTreeSet<SugarCanePosition>,
    dirty_chunks: BTreeSet<ChunkPos>,
}

impl SugarCaneGrowthSystem {
    /// Create a new sugar cane growth system.
    pub fn new(world_seed: u64) -> Self {
        Self {
            world_seed,
            base_positions: BTreeSet::new(),
            dirty_chunks: BTreeSet::new(),
        }
    }

    /// Register a base sugar cane block for growth updates.
    pub fn register_base(&mut self, pos: SugarCanePosition) {
        self.base_positions.insert(pos);
    }

    /// Unregister all sugar cane bases in a chunk (e.g. when unloading it).
    pub fn unregister_chunk(&mut self, chunk: ChunkPos) {
        let start = SugarCanePosition {
            chunk,
            x: 0,
            y: 0,
            z: 0,
        };
        let end = SugarCanePosition {
            chunk,
            x: u8::MAX,
            y: u8::MAX,
            z: u8::MAX,
        };

        let to_remove: Vec<SugarCanePosition> =
            self.base_positions.range(start..=end).copied().collect();
        for pos in to_remove {
            self.base_positions.remove(&pos);
        }
    }

    /// Tick sugar cane growth (called each game tick).
    pub fn tick(&mut self, tick: u64, chunks: &mut HashMap<ChunkPos, Chunk>) {
        if self.base_positions.is_empty() {
            return;
        }

        // Create a seeded RNG for this tick.
        let tick_seed = self
            .world_seed
            .wrapping_add(tick.wrapping_mul(0x53_55_47_41_52_43_41_4E_u64)); // "SUGARCAN"
        let mut rng = StdRng::seed_from_u64(tick_seed);

        // Collect bases to update (avoid borrow issues).
        let bases_to_check: Vec<SugarCanePosition> = self.base_positions.iter().copied().collect();

        for pos in bases_to_check {
            // Random tick chance (1 in 200 per tick).
            if rng.gen_ratio(1, 200) {
                self.try_grow(pos, chunks, &mut rng);
            }
        }
    }

    fn try_grow(
        &mut self,
        pos: SugarCanePosition,
        chunks: &mut HashMap<ChunkPos, Chunk>,
        rng: &mut StdRng,
    ) {
        let chunk = match chunks.get(&pos.chunk) {
            Some(c) => c,
            None => return,
        };

        let x = pos.x as usize;
        let y = pos.y as usize;
        let z = pos.z as usize;
        if y >= CHUNK_SIZE_Y {
            self.base_positions.remove(&pos);
            return;
        }

        let base_voxel = chunk.voxel(x, y, z);
        if base_voxel.id != BLOCK_SUGAR_CANE {
            self.base_positions.remove(&pos);
            return;
        }

        // If this isn't actually a base block (stacking/scan mistake), re-register the true base.
        if y > 0 {
            let below = chunk.voxel(x, y - 1, z);
            if below.id == BLOCK_SUGAR_CANE {
                let mut base_y = y;
                while base_y > 0 && chunk.voxel(x, base_y - 1, z).id == BLOCK_SUGAR_CANE {
                    base_y -= 1;
                }
                let corrected = SugarCanePosition {
                    chunk: pos.chunk,
                    x: pos.x,
                    y: base_y as u8,
                    z: pos.z,
                };
                self.base_positions.remove(&pos);
                self.base_positions.insert(corrected);
                return;
            }
        }

        let random_value = rng.gen::<f32>();
        if !base_can_grow(pos, chunks, base_voxel, random_value) {
            return;
        }

        // Count current column height (allow player-built taller columns; growth stops at 3).
        let mut height = 0usize;
        while y + height < CHUNK_SIZE_Y && chunk.voxel(x, y + height, z).id == BLOCK_SUGAR_CANE {
            height += 1;
        }
        if height >= 3 {
            return;
        }

        let place_y = y + height;
        if place_y >= CHUNK_SIZE_Y {
            return;
        }

        let above = chunk.voxel(x, place_y, z);
        if above.id != BLOCK_AIR {
            return;
        }

        if let Some(chunk) = chunks.get_mut(&pos.chunk) {
            chunk.set_voxel(
                x,
                place_y,
                z,
                Voxel {
                    id: BLOCK_SUGAR_CANE,
                    state: 0,
                    light_sky: above.light_sky,
                    light_block: above.light_block,
                },
            );
            self.dirty_chunks.insert(pos.chunk);
        }
    }

    /// Take the set of dirty chunks (clears internal state).
    pub fn take_dirty_chunks(&mut self) -> BTreeSet<ChunkPos> {
        std::mem::take(&mut self.dirty_chunks)
    }
}

fn base_can_grow(
    pos: SugarCanePosition,
    chunks: &HashMap<ChunkPos, Chunk>,
    base_voxel: Voxel,
    random_value: f32,
) -> bool {
    let y = pos.y as i32;
    if y <= 0 {
        return false;
    }

    // Support block must be dirt/grass/sand.
    let support_pos = SugarCanePosition {
        y: (pos.y - 1),
        ..pos
    };
    let support_id = {
        let chunk = match chunks.get(&support_pos.chunk) {
            Some(c) => c,
            None => return false,
        };
        chunk
            .voxel(
                support_pos.x as usize,
                support_pos.y as usize,
                support_pos.z as usize,
            )
            .id
    };
    if !matches!(support_id, BLOCK_DIRT | BLOCK_GRASS | BLOCK_SAND) {
        return false;
    }

    // Need light level 9+.
    let light_level = base_voxel.light_sky.max(base_voxel.light_block);
    if light_level < 9 {
        return false;
    }

    // Need adjacent water around the support block.
    let world_x = pos.chunk.x * CHUNK_SIZE_X as i32 + pos.x as i32;
    let world_z = pos.chunk.z * CHUNK_SIZE_Z as i32 + pos.z as i32;
    let support_y = y - 1;

    let mut has_water = false;
    for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
        let check_x = world_x + dx;
        let check_z = world_z + dz;
        let check_chunk_x = check_x.div_euclid(CHUNK_SIZE_X as i32);
        let check_chunk_z = check_z.div_euclid(CHUNK_SIZE_Z as i32);
        let local_x = check_x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let local_z = check_z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        let check_chunk = ChunkPos::new(check_chunk_x, check_chunk_z);

        let Some(chunk) = chunks.get(&check_chunk) else {
            continue;
        };
        if support_y < 0 || support_y >= CHUNK_SIZE_Y as i32 {
            continue;
        }
        let voxel = chunk.voxel(local_x, support_y as usize, local_z);
        if matches!(voxel.id, BLOCK_WATER | BLOCK_WATER_FLOWING) {
            has_water = true;
            break;
        }
    }
    if !has_water {
        return false;
    }

    // Growth chance based on light (small bonus for brighter spots).
    let growth_chance = (light_level as f32 / 15.0) * 0.75;
    random_value < growth_chance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_unregister_chunk_is_deterministic() {
        let mut system = SugarCaneGrowthSystem::new(123);
        let chunk = ChunkPos::new(1, 2);
        system.register_base(SugarCanePosition {
            chunk,
            x: 1,
            y: 10,
            z: 1,
        });
        system.register_base(SugarCanePosition {
            chunk,
            x: 2,
            y: 11,
            z: 2,
        });
        assert_eq!(system.base_positions.len(), 2);
        system.unregister_chunk(chunk);
        assert!(system.base_positions.is_empty());
    }

    #[test]
    fn base_can_grow_requires_water_and_light() {
        let mut chunks = HashMap::new();
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);

        // Support: dirt at y=60, sugar cane at y=61, water adjacent at y=60.
        chunk.set_voxel(
            5,
            60,
            5,
            Voxel {
                id: BLOCK_DIRT,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            5,
            61,
            5,
            Voxel {
                id: BLOCK_SUGAR_CANE,
                light_sky: 15,
                ..Default::default()
            },
        );
        chunk.set_voxel(
            6,
            60,
            5,
            Voxel {
                id: BLOCK_WATER,
                ..Default::default()
            },
        );
        chunks.insert(pos, chunk);

        let base = SugarCanePosition {
            chunk: pos,
            x: 5,
            y: 61,
            z: 5,
        };

        let base_voxel = chunks[&pos].voxel(5, 61, 5);
        assert!(base_can_grow(base, &chunks, base_voxel, 0.0));

        // Remove water: should no longer grow.
        chunks
            .get_mut(&pos)
            .unwrap()
            .set_voxel(6, 60, 5, Voxel::default());
        let base_voxel = chunks[&pos].voxel(5, 61, 5);
        assert!(!base_can_grow(base, &chunks, base_voxel, 0.0));
    }
}
