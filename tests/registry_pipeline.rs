use mdminecraft_assets::registry_from_str;
use mdminecraft_render::{ChunkMeshCache, ChunkMeshDriver};
use mdminecraft_world::{ChunkPos, ChunkStorage, Voxel};

const PACK: &str = r#"
[
  { "name": "air", "opaque": false },
  { "name": "stone", "opaque": true }
]
"#;

#[test]
fn registry_mesh_pipeline_from_json() {
    let registry = registry_from_str(PACK).expect("valid pack");
    let mut storage = ChunkStorage::new(1);
    let pos = ChunkPos::new(0, 0);
    let chunk = storage.ensure_chunk(pos);
    chunk.set_voxel(
        0,
        0,
        0,
        Voxel {
            id: registry.id_by_name("stone").unwrap(),
            state: 0,
            light_sky: 0,
            light_block: 0,
        },
    );
    let mut cache = ChunkMeshCache::new();
    let mut driver = ChunkMeshDriver::new(&mut storage, &mut cache, &registry);
    let stats = driver.process();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].position, pos);
    assert!(stats[0].triangles > 0);
}

