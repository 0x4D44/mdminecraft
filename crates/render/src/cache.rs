use std::collections::HashMap;

use mdminecraft_assets::BlockRegistry;
use mdminecraft_world::{Chunk, ChunkPos, DirtyFlags};

use crate::{mesh_chunk, MeshBuffers};

/// Mesh cache keyed by chunk position.
#[derive(Default)]
pub struct ChunkMeshCache {
    entries: HashMap<ChunkPos, MeshBuffers>,
}

impl ChunkMeshCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Ensure the chunk at `pos` has an up-to-date mesh.
    pub fn update_chunk(
        &mut self,
        chunk: &Chunk,
        dirty: DirtyFlags,
        registry: &BlockRegistry,
    ) -> &MeshBuffers {
        let pos = chunk.position();
        let entry = self.entries.entry(pos).or_insert_with(MeshBuffers::empty);
        if dirty.contains(DirtyFlags::MESH) {
            *entry = mesh_chunk(chunk, registry);
        }
        entry
    }

    /// Retrieve the mesh if it's cached.
    pub fn get(&self, pos: ChunkPos) -> Option<&MeshBuffers> {
        self.entries.get(&pos)
    }
}

#[cfg(test)]
mod tests {
    use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
    use mdminecraft_world::{Chunk, ChunkPos, Voxel};

    use super::*;

    fn registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor {
                name: "air".into(),
                opaque: false,
            },
            BlockDescriptor {
                name: "stone".into(),
                opaque: true,
            },
        ])
    }

    #[test]
    fn cache_only_rebuilds_when_dirty() {
        let pos = ChunkPos::new(0, 0);
        let mut chunk = Chunk::new(pos);
        let mut cache = ChunkMeshCache::new();
        let registry = registry();

        let dirty = chunk.take_dirty_flags();
        let mesh_a = cache.update_chunk(&chunk, dirty, &registry).hash;
        let clean = chunk.take_dirty_flags();
        let mesh_b = cache.update_chunk(&chunk, clean, &registry).hash;
        assert_eq!(mesh_a, mesh_b);

        chunk.set_voxel(
            0,
            0,
            0,
            Voxel {
                id: 5,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        let dirty = chunk.take_dirty_flags();
        let mesh_c = cache.update_chunk(&chunk, dirty, &registry).hash;
        assert_ne!(mesh_b, mesh_c);
    }
}
