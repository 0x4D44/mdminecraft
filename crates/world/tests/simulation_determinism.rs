use mdminecraft_world::{
    Chunk, ChunkPos, FluidPos, FluidSimulator, FluidType, RedstonePos, RedstoneSimulator, Voxel,
    CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z,
};
use std::collections::HashMap;

#[test]
fn test_fluid_simulation_determinism() {
    // Run two identical simulations
    let final_chunks1 = run_fluid_sim();
    let final_chunks2 = run_fluid_sim();

    // Compare results
    assert_eq!(final_chunks1.len(), final_chunks2.len());
    // Sort keys for deterministic comparison order (though HashMap iteration is random, Eq check is fine)
    // But we need to iterate to compare contents.
    for (pos, chunk1) in &final_chunks1 {
        let chunk2 = final_chunks2.get(pos).expect("Chunk missing in sim2");

        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let v1 = chunk1.voxel(x, y, z);
                    let v2 = chunk2.voxel(x, y, z);
                    assert_eq!(v1, v2, "Mismatch at ({},{},{}) in chunk {:?}", x, y, z, pos);
                }
            }
        }
    }
}

fn run_fluid_sim() -> HashMap<ChunkPos, Chunk> {
    let mut chunk = Chunk::new(ChunkPos::new(0, 0));
    // Water source
    chunk.set_voxel(
        8,
        60,
        8,
        Voxel {
            id: FluidType::Water.source_block_id(),
            state: 0,
            light_sky: 0,
            light_block: 0,
        },
    );

    let mut chunks = HashMap::new();
    chunks.insert(ChunkPos::new(0, 0), chunk);

    let mut sim = FluidSimulator::new();
    sim.schedule_update(FluidPos::new(8, 60, 8), 0);

    for _ in 0..50 {
        sim.tick(&mut chunks);
    }

    chunks
}

#[test]
fn test_redstone_simulation_determinism() {
    let final_chunks1 = run_redstone_sim();
    let final_chunks2 = run_redstone_sim();

    assert_eq!(final_chunks1.len(), final_chunks2.len());
    for (pos, chunk1) in &final_chunks1 {
        let chunk2 = final_chunks2.get(pos).expect("Chunk missing in sim2");
        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let v1 = chunk1.voxel(x, y, z);
                    let v2 = chunk2.voxel(x, y, z);
                    assert_eq!(v1, v2, "Mismatch at ({},{},{}) in chunk {:?}", x, y, z, pos);
                }
            }
        }
    }
}

fn run_redstone_sim() -> HashMap<ChunkPos, Chunk> {
    use mdminecraft_world::redstone_blocks;
    let mut chunk = Chunk::new(ChunkPos::new(0, 0));

    // Place lever and wire
    chunk.set_voxel(
        8,
        60,
        8,
        Voxel {
            id: redstone_blocks::LEVER,
            state: 0,
            ..Default::default()
        },
    );
    chunk.set_voxel(
        9,
        60,
        8,
        Voxel {
            id: redstone_blocks::REDSTONE_WIRE,
            state: 0,
            ..Default::default()
        },
    );

    let mut chunks = HashMap::new();
    chunks.insert(ChunkPos::new(0, 0), chunk);

    let mut sim = RedstoneSimulator::new();
    sim.toggle_lever(RedstonePos::new(8, 60, 8), &mut chunks);

    for _ in 0..20 {
        sim.tick(&mut chunks);
    }

    chunks
}
