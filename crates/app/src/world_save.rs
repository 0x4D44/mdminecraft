//! World save/load system.

use anyhow::{Context, Result};
use mdminecraft_world::{Chunk, ChunkPos};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// World metadata stored in the save file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMetadata {
    /// World name.
    pub name: String,

    /// World seed for generation.
    pub seed: u64,

    /// Last played timestamp (seconds since epoch).
    pub last_played: u64,

    /// Player spawn position.
    pub spawn_position: [f32; 3],

    /// Current game time (ticks).
    pub game_time: u64,
}

impl Default for WorldMetadata {
    fn default() -> Self {
        Self {
            name: "New World".to_string(),
            seed: rand::random(),
            last_played: current_timestamp(),
            spawn_position: [0.0, 72.0, 0.0],
            game_time: 0,
        }
    }
}

/// Saved world data including metadata and chunks.
#[derive(Debug, Serialize, Deserialize)]
struct WorldSaveData {
    metadata: WorldMetadata,
    chunks: Vec<Chunk>,
}

/// Get the saves directory path.
pub fn saves_dir() -> PathBuf {
    let mut path = PathBuf::from(".");
    path.push("saves");
    path
}

/// Get the path for a specific world save.
pub fn world_save_path(world_name: &str) -> PathBuf {
    let mut path = saves_dir();
    path.push(world_name);
    path
}

/// Get current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Save a world to disk.
pub fn save_world(
    world_name: &str,
    metadata: &WorldMetadata,
    chunks: &HashMap<ChunkPos, Chunk>,
) -> Result<()> {
    let world_path = world_save_path(world_name);

    // Create saves directory if it doesn't exist
    fs::create_dir_all(&world_path).context("Failed to create world directory")?;

    // Update last played timestamp
    let mut meta = metadata.clone();
    meta.last_played = current_timestamp();

    // Convert chunks HashMap to Vec for serialization
    let chunk_vec: Vec<Chunk> = chunks.values().map(|c| c.clone()).collect();

    let save_data = WorldSaveData {
        metadata: meta.clone(),
        chunks: chunk_vec,
    };

    // Save metadata separately for quick loading
    let meta_path = world_path.join("metadata.json");
    let meta_json = serde_json::to_string_pretty(&meta).context("Failed to serialize metadata")?;
    fs::write(&meta_path, meta_json).context("Failed to write metadata file")?;

    // Save full world data (compressed)
    let data_path = world_path.join("world.dat");
    let data_json = serde_json::to_vec(&save_data).context("Failed to serialize world data")?;

    // Optionally compress the data
    let compressed = compress_data(&data_json);
    fs::write(&data_path, compressed).context("Failed to write world data file")?;

    tracing::info!("saved world '{}' with {} chunks", world_name, chunks.len());
    Ok(())
}

/// Load a world from disk.
pub fn load_world(world_name: &str) -> Result<(WorldMetadata, HashMap<ChunkPos, Chunk>)> {
    let world_path = world_save_path(world_name);

    if !world_path.exists() {
        anyhow::bail!("World '{}' does not exist", world_name);
    }

    // Load world data
    let data_path = world_path.join("world.dat");
    let compressed = fs::read(&data_path).context("Failed to read world data file")?;
    let data_json = decompress_data(&compressed);
    let save_data: WorldSaveData =
        serde_json::from_slice(&data_json).context("Failed to deserialize world data")?;

    // Convert chunks Vec back to HashMap
    let mut chunks = HashMap::new();
    for chunk in save_data.chunks {
        chunks.insert(chunk.position(), chunk);
    }

    tracing::info!("loaded world '{}' with {} chunks", world_name, chunks.len());
    Ok((save_data.metadata, chunks))
}

/// List all saved worlds.
pub fn list_worlds() -> Result<Vec<WorldMetadata>> {
    let saves_path = saves_dir();

    if !saves_path.exists() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();

    for entry in fs::read_dir(&saves_path).context("Failed to read saves directory")? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let meta_path = path.join("metadata.json");
            if meta_path.exists() {
                let meta_json = fs::read_to_string(&meta_path)?;
                let metadata: WorldMetadata = serde_json::from_str(&meta_json)?;
                worlds.push(metadata);
            }
        }
    }

    // Sort by last played (most recent first)
    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));

    Ok(worlds)
}

/// Simple compression using flate2 (gzip).
fn compress_data(data: &[u8]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// Simple decompression using flate2 (gzip).
fn decompress_data(compressed: &[u8]) -> Vec<u8> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();
    decompressed
}

/// Delete a world from disk.
pub fn delete_world(world_name: &str) -> Result<()> {
    let world_path = world_save_path(world_name);
    if world_path.exists() {
        fs::remove_dir_all(&world_path).context("Failed to delete world directory")?;
        tracing::info!("deleted world '{}'", world_name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress() {
        let original = b"Hello, world! This is test data for compression.";
        let compressed = compress_data(original);
        let decompressed = decompress_data(&compressed);

        assert_eq!(original.as_slice(), decompressed.as_slice());
        assert!(compressed.len() < original.len()); // Should be smaller
    }
}
