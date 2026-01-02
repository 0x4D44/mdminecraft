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
    config_from_iter(env::args().skip(1))
}

fn config_from_iter<I>(mut args: I) -> Result<CliConfig>
where
    I: Iterator<Item = String>,
{
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
        BlockDescriptor::simple("air", false),
        BlockDescriptor::simple("stone", true),
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
    let mut driver = ChunkMeshDriver::new(&mut storage, &mut cache, registry, None);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_use_local_registry_and_default_metrics_path() {
        let config = config_from_iter(std::iter::empty()).expect("config");
        assert!(config.registry.id_by_name("air").is_some());
        assert!(config.registry.id_by_name("stone").is_some());
        assert_eq!(
            config.mesh_metrics,
            PathBuf::from("target/mesh_metrics.json")
        );
    }

    #[test]
    fn config_accepts_metrics_override() {
        let config = config_from_iter(
            ["--mesh-metrics".to_string(), "target/custom_metrics.json".to_string()].into_iter(),
        )
        .expect("config");
        assert_eq!(
            config.mesh_metrics,
            PathBuf::from("target/custom_metrics.json")
        );
    }

    #[test]
    fn config_loads_registry_from_blocks_file() {
        let blocks_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config")
            .join("blocks.json");
        let args = [
            "--blocks".to_string(),
            blocks_path.to_string_lossy().to_string(),
        ];
        let config = config_from_iter(args.into_iter()).expect("config");
        assert!(config.registry.id_by_name("stone").is_some());
    }
}
