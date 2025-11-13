use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use mdminecraft_assets::{registry_from_file, BlockDescriptor, BlockRegistry};
use mdminecraft_client::Client;
use mdminecraft_render::{ChunkMeshCache, ChunkMeshDriver};
use mdminecraft_world::{ChunkPos, ChunkStorage, Voxel};
use tracing::Level;
use tracing_subscriber::fmt;

fn main() -> Result<()> {
    let _ = fmt().with_max_level(Level::INFO).try_init();
    tracing::info!("booting deterministic voxel sandbox placeholder");
    let config = config_from_args()?;
    let mut client = Client::singleplayer();
    for _ in 0..3 {
        client.frame()?;
    }
    demo_mesher(&config.registry, &config.mesh_metrics)?;
    Ok(())
}

struct CliConfig {
    registry: BlockRegistry,
    mesh_metrics: PathBuf,
}

fn config_from_args() -> Result<CliConfig> {
    let mut args = env::args().skip(1);
    let mut block_path: Option<PathBuf> = None;
    let mut metrics_path: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--blocks" => block_path = args.next().map(PathBuf::from),
            "--mesh-metrics" => metrics_path = args.next().map(PathBuf::from),
            _ => {}
        }
    }
    let registry = if let Some(path) = block_path {
        registry_from_file(&path)
            .with_context(|| format!("failed to load block pack from {}", path.display()))?
    } else {
        default_registry()
    };
    let metrics = metrics_path.unwrap_or_else(|| PathBuf::from("target/mesh_metrics.json"));
    Ok(CliConfig {
        registry,
        mesh_metrics: metrics,
    })
}

fn default_registry() -> BlockRegistry {
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

fn demo_mesher(registry: &BlockRegistry, metrics_path: &Path) -> Result<()> {
    let pos = ChunkPos::new(0, 0);
    let mut storage = ChunkStorage::new(4);
    let chunk = storage.ensure_chunk(pos);
    let stone_id = registry.id_by_name("stone").unwrap_or(1);
    chunk.set_voxel(
        1,
        1,
        1,
        Voxel {
            id: stone_id,
            state: 0,
            light_sky: 0,
            light_block: 0,
        },
    );
    let mut cache = ChunkMeshCache::new();
    let mut driver = ChunkMeshDriver::new(&mut storage, &mut cache, registry);
    let stats = driver.process();
    tracing::info!(chunks = stats.len(), "processed dirty chunks");
    for stat in &stats {
        tracing::info!(
            ?stat.position,
            hash = format_args!("{:x?}", stat.hash.0),
            triangles = stat.triangles,
            "chunk meshed"
        );
    }
    if !stats.is_empty() {
        ChunkMeshDriver::write_metrics_to_file(&stats, metrics_path)?;
        tracing::info!(path = %metrics_path.display(), "wrote mesh metrics");
    }
    Ok(())
}
