use std::path::Path;

use anyhow::Result;
use mdminecraft_assets::{BlockRegistry, TextureAtlasMetadata};
use mdminecraft_testkit::{ChunkMeshMetric, MeshMetricSink};
use mdminecraft_world::{ChunkPos, ChunkStorage, DirtyFlags};

use crate::{ChunkMeshCache, MeshHash};

/// Mesh stats for a chunk update pass.
pub struct ChunkMeshStat {
    /// Chunk position this mesh belongs to.
    pub position: ChunkPos,
    /// Number of triangles generated for the chunk.
    pub triangles: usize,
    /// Mesh hash for determinism comparisons.
    pub hash: MeshHash,
}

/// Processes dirty chunks and refreshes mesh cache entries.
pub struct ChunkMeshDriver<'a> {
    storage: &'a mut ChunkStorage,
    cache: &'a mut ChunkMeshCache,
    registry: &'a BlockRegistry,
    atlas: Option<&'a TextureAtlasMetadata>,
}

impl<'a> ChunkMeshDriver<'a> {
    /// Create a new driver spanning storage/cache/registry.
    pub fn new(
        storage: &'a mut ChunkStorage,
        cache: &'a mut ChunkMeshCache,
        registry: &'a BlockRegistry,
        atlas: Option<&'a TextureAtlasMetadata>,
    ) -> Self {
        Self {
            storage,
            cache,
            registry,
            atlas,
        }
    }

    /// Mesh all dirty chunks and return stats.
    pub fn process(&mut self) -> Vec<ChunkMeshStat> {
        let positions: Vec<_> = self.storage.iter_positions().collect();
        let mut stats = Vec::new();
        for pos in positions {
            if let Some(chunk) = self.storage.get_mut(pos) {
                let dirty = chunk.take_dirty_flags();
                if dirty.contains(DirtyFlags::MESH) {
                    let mesh = self
                        .cache
                        .update_chunk(chunk, dirty, self.registry, self.atlas);
                    stats.push(ChunkMeshStat {
                        position: pos,
                        triangles: mesh.indices.len() / 3,
                        hash: mesh.hash,
                    });
                }
            }
        }
        stats
    }

    /// Convert stats into serializable metrics for CI artifacts.
    pub fn stats_to_metrics(stats: &[ChunkMeshStat]) -> Vec<ChunkMeshMetric> {
        stats
            .iter()
            .map(|stat| ChunkMeshMetric {
                chunk: [stat.position.x, stat.position.z],
                triangles: stat.triangles,
                hash: format!("{:x?}", stat.hash.0),
            })
            .collect()
    }

    /// Write metrics to disk using the testkit sink.
    pub fn write_metrics_to_file<P: AsRef<Path>>(stats: &[ChunkMeshStat], path: P) -> Result<()> {
        let metrics = Self::stats_to_metrics(stats);
        let mut sink = MeshMetricSink::create(path)?;
        sink.write(&metrics)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use mdminecraft_assets::{BlockDescriptor, BlockRegistry};
    use mdminecraft_world::{ChunkPos, ChunkStorage, Voxel};

    use super::*;
    use crate::ChunkMeshCache;

    fn registry() -> BlockRegistry {
        BlockRegistry::new(vec![
            BlockDescriptor::simple("air", false),
            BlockDescriptor::simple("stone", true),
        ])
    }

    #[test]
    fn driver_meshes_dirty_chunks() {
        let mut storage = ChunkStorage::new(2);
        let pos = ChunkPos::new(0, 0);
        let chunk = storage.ensure_chunk(pos);
        chunk.set_voxel(
            0,
            0,
            0,
            Voxel {
                id: 1,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        let mut cache = ChunkMeshCache::new();
        let registry = registry();
        let mut driver = ChunkMeshDriver::new(&mut storage, &mut cache, &registry, None);
        let stats = driver.process();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].position, pos);
        assert!(stats[0].triangles > 0);
        let metrics = ChunkMeshDriver::stats_to_metrics(&stats);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].chunk, [0, 0]);
    }

    #[test]
    fn write_metrics_to_file_outputs_json() {
        let stats = vec![ChunkMeshStat {
            position: ChunkPos::new(1, -2),
            triangles: 12,
            hash: MeshHash([0; 32]),
        }];
        let path = std::env::temp_dir().join("mesh-metrics-driver.json");
        ChunkMeshDriver::write_metrics_to_file(&stats, &path).expect("metrics write");
        let contents = fs::read_to_string(&path).expect("read metrics");
        assert!(contents.contains("\"triangles\""));
    }
}
