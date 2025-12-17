//! Region-based chunk persistence with zstd compression.
//!
//! Implements .rg region files that group 32x32 chunks for efficient storage.
//! Each region file uses zstd compression and CRC32 validation.

use crate::chunk::{Chunk, ChunkPos, Voxel, CHUNK_VOLUME};
use crate::{SimTime, WeatherToggle};
use anyhow::{Context, Result};
use crc32fast::Hasher;
use mdminecraft_core::DimensionId;
use mdminecraft_core::SimTick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};

/// Magic number for region file identification ("MDRG" = mdminecraft region).
const REGION_MAGIC: u32 = 0x4D445247;

/// Current region file format version.
const REGION_VERSION: u16 = 1;

/// Magic number for the world meta file ("MDWM" = mdminecraft world meta).
const WORLD_META_MAGIC: u32 = 0x4D44574D;

/// Current world meta file format version.
const WORLD_META_VERSION: u16 = 1;

/// Magic number for the world state file ("MDWS" = mdminecraft world state).
const WORLD_STATE_MAGIC: u32 = 0x4D445753;

/// Current world state file format version.
const WORLD_STATE_VERSION: u16 = 1;

/// Region size in chunks (32x32 chunks per region).
const REGION_SIZE: i32 = 32;

/// Region file header structure.
#[derive(Debug, Clone)]
struct RegionHeader {
    magic: u32,
    version: u16,
    crc32: u32,
    payload_len: u32,
}

impl RegionHeader {
    fn new(crc32: u32, payload_len: u32) -> Self {
        Self {
            magic: REGION_MAGIC,
            version: REGION_VERSION,
            crc32,
            payload_len,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(14);
        bytes.extend_from_slice(&self.magic.to_le_bytes());
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&self.crc32.to_le_bytes());
        bytes.extend_from_slice(&self.payload_len.to_le_bytes());
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 14 {
            anyhow::bail!("Region header too short");
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != REGION_MAGIC {
            anyhow::bail!(
                "Invalid region magic: expected 0x{:08X}, got 0x{:08X}",
                REGION_MAGIC,
                magic
            );
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        let crc32 = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        let payload_len = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);

        Ok(Self {
            magic,
            version,
            crc32,
            payload_len,
        })
    }
}

/// World meta stored alongside region data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldMeta {
    /// World seed used for deterministic world generation.
    pub world_seed: u64,
}

/// Global world state that must survive save/load cycles.
///
/// Chunk voxel data is stored separately in region files; this captures
/// cross-chunk/global simulation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    /// Current simulation tick.
    pub tick: SimTick,
    /// Simulation day/night cycle state.
    pub sim_time: SimTime,
    /// Weather toggle state.
    pub weather: WeatherToggle,
    /// Elapsed time since the last weather transition (seconds).
    pub weather_timer_seconds: f32,
    /// Next weather transition scheduled after this many seconds.
    pub next_weather_change_seconds: f32,
}

/// Header for small world save blobs (meta/state).
#[derive(Debug, Clone)]
struct WorldBlobHeader {
    magic: u32,
    version: u16,
    crc32: u32,
    payload_len: u32,
}

impl WorldBlobHeader {
    fn new(magic: u32, version: u16, crc32: u32, payload_len: u32) -> Self {
        Self {
            magic,
            version,
            crc32,
            payload_len,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(14);
        bytes.extend_from_slice(&self.magic.to_le_bytes());
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&self.crc32.to_le_bytes());
        bytes.extend_from_slice(&self.payload_len.to_le_bytes());
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 14 {
            anyhow::bail!("World blob header too short");
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        let crc32 = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        let payload_len = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);

        Ok(Self {
            magic,
            version,
            crc32,
            payload_len,
        })
    }
}

/// Converts chunk position to region coordinates.
fn chunk_to_region(chunk_pos: ChunkPos) -> (i32, i32) {
    (
        chunk_pos.x.div_euclid(REGION_SIZE),
        chunk_pos.z.div_euclid(REGION_SIZE),
    )
}

/// Region file manager for saving/loading chunks.
pub struct RegionStore {
    world_dir: PathBuf,
}

impl RegionStore {
    /// Create a new region store rooted at the given world directory.
    pub fn new<P: AsRef<Path>>(world_dir: P) -> Result<Self> {
        let world_dir = world_dir.as_ref().to_path_buf();
        fs::create_dir_all(&world_dir).context("Failed to create world directory")?;
        Ok(Self { world_dir })
    }

    fn world_meta_path(&self) -> PathBuf {
        self.world_dir.join("world.meta")
    }

    fn world_state_path(&self) -> PathBuf {
        self.world_dir.join("world.state")
    }

    /// Get the path to a region file for the given dimension and region coordinates.
    fn region_path(&self, dimension: DimensionId, region_x: i32, region_z: i32) -> PathBuf {
        match dimension {
            DimensionId::Overworld => self
                .world_dir
                .join(format!("r.{}.{}.rg", region_x, region_z)),
            other => self
                .world_dir
                .join("dimensions")
                .join(other.as_str())
                .join(format!("r.{}.{}.rg", region_x, region_z)),
        }
    }

    /// Check if a world meta blob exists on disk.
    pub fn world_meta_exists(&self) -> bool {
        self.world_meta_path().exists()
    }

    /// Save world meta.
    pub fn save_world_meta(&self, meta: &WorldMeta) -> Result<()> {
        let path = self.world_meta_path();
        self.write_world_blob(&path, WORLD_META_MAGIC, WORLD_META_VERSION, meta)
            .with_context(|| format!("Failed to save world meta to {}", path.display()))
    }

    /// Load world meta.
    pub fn load_world_meta(&self) -> Result<WorldMeta> {
        let path = self.world_meta_path();
        self.read_world_blob(&path, WORLD_META_MAGIC, WORLD_META_VERSION)
            .with_context(|| format!("Failed to load world meta from {}", path.display()))
    }

    /// Check if a world state blob exists on disk.
    pub fn world_state_exists(&self) -> bool {
        self.world_state_path().exists()
    }

    /// Save world state.
    pub fn save_world_state(&self, state: &WorldState) -> Result<()> {
        let path = self.world_state_path();
        self.write_world_blob(&path, WORLD_STATE_MAGIC, WORLD_STATE_VERSION, state)
            .with_context(|| format!("Failed to save world state to {}", path.display()))
    }

    /// Load world state.
    pub fn load_world_state(&self) -> Result<WorldState> {
        let path = self.world_state_path();
        self.read_world_blob(&path, WORLD_STATE_MAGIC, WORLD_STATE_VERSION)
            .with_context(|| format!("Failed to load world state from {}", path.display()))
    }

    /// Save a chunk to its region file.
    #[instrument(skip(self, chunk), fields(chunk_pos = ?chunk.position()))]
    pub fn save_chunk(&self, chunk: &Chunk) -> Result<()> {
        self.save_chunk_in_dimension(DimensionId::DEFAULT, chunk)
    }

    /// Save a chunk to its region file in the specified dimension.
    #[instrument(skip(self, chunk), fields(dimension = %dimension.as_str(), chunk_pos = ?chunk.position()))]
    pub fn save_chunk_in_dimension(&self, dimension: DimensionId, chunk: &Chunk) -> Result<()> {
        let (region_x, region_z) = chunk_to_region(chunk.position());
        debug!(region_x, region_z, "Saving chunk to region");

        // Load existing region or create new one.
        let mut region_data = self
            .load_region(dimension, region_x, region_z)
            .unwrap_or_default();

        // Serialize chunk data.
        let chunk_data = serialize_chunk(chunk)?;
        debug!(chunk_data_size = chunk_data.len(), "Serialized chunk data");

        // Store in region map.
        region_data.insert(chunk.position(), chunk_data);

        // Write region file.
        self.write_region(dimension, region_x, region_z, &region_data)?;

        debug!(chunk_count = region_data.len(), "Chunk saved successfully");
        Ok(())
    }

    /// Load a chunk from its region file.
    #[instrument(skip(self), fields(chunk_pos = ?pos))]
    pub fn load_chunk(&self, pos: ChunkPos) -> Result<Chunk> {
        self.load_chunk_in_dimension(DimensionId::DEFAULT, pos)
    }

    /// Load a chunk from its region file in the specified dimension.
    #[instrument(skip(self), fields(dimension = %dimension.as_str(), chunk_pos = ?pos))]
    pub fn load_chunk_in_dimension(&self, dimension: DimensionId, pos: ChunkPos) -> Result<Chunk> {
        let (region_x, region_z) = chunk_to_region(pos);
        debug!(region_x, region_z, "Loading chunk from region");

        let region_data = self.load_region(dimension, region_x, region_z)?;

        let chunk_data = region_data.get(&pos).context("Chunk not found in region")?;
        debug!(
            chunk_data_size = chunk_data.len(),
            "Found chunk data in region"
        );

        let chunk = deserialize_chunk(pos, chunk_data)?;
        debug!("Chunk loaded successfully");
        Ok(chunk)
    }

    /// Load an entire region file into memory.
    #[instrument(skip(self), fields(region_x, region_z))]
    fn load_region(
        &self,
        dimension: DimensionId,
        region_x: i32,
        region_z: i32,
    ) -> Result<HashMap<ChunkPos, Vec<u8>>> {
        let region_path = self.region_path(dimension, region_x, region_z);
        debug!(path = %region_path.display(), "Loading region file");

        if !region_path.exists() {
            anyhow::bail!("Region file does not exist: {}", region_path.display());
        }

        let mut file = File::open(&region_path).context("Failed to open region file")?;

        // Read header.
        let mut header_bytes = [0u8; 14];
        file.read_exact(&mut header_bytes)
            .context("Failed to read region header")?;
        let header = RegionHeader::from_bytes(&header_bytes)?;
        if header.version != REGION_VERSION {
            anyhow::bail!(
                "Unsupported region version {} (expected {}). World upgrade required.",
                header.version,
                REGION_VERSION
            );
        }
        debug!(
            version = header.version,
            payload_len = header.payload_len,
            "Read region header"
        );

        // Read compressed payload.
        let mut compressed = vec![0u8; header.payload_len as usize];
        file.read_exact(&mut compressed)
            .context("Failed to read region payload")?;

        // Verify CRC32.
        let mut hasher = Hasher::new();
        hasher.update(&compressed);
        let computed_crc = hasher.finalize();

        if computed_crc != header.crc32 {
            warn!(
                expected_crc = format!("{:08X}", header.crc32),
                computed_crc = format!("{:08X}", computed_crc),
                "CRC32 mismatch in region file"
            );
            anyhow::bail!(
                "CRC32 mismatch: expected {:08X}, got {:08X}",
                header.crc32,
                computed_crc
            );
        }
        debug!("CRC32 validation passed");

        // Decompress payload.
        let decompressed =
            zstd::decode_all(&compressed[..]).context("Failed to decompress region")?;
        let compression_ratio = decompressed.len() as f64 / compressed.len() as f64;
        debug!(
            compressed_size = compressed.len(),
            decompressed_size = decompressed.len(),
            compression_ratio = format!("{:.2}x", compression_ratio),
            "Region decompressed"
        );

        // Deserialize region data.
        let region_data: HashMap<ChunkPos, Vec<u8>> =
            bincode::deserialize(&decompressed).context("Failed to deserialize region")?;

        info!(
            chunk_count = region_data.len(),
            "Region file loaded successfully"
        );
        Ok(region_data)
    }

    /// Write an entire region file to disk.
    #[instrument(skip(self, data), fields(region_x, region_z, chunk_count = data.len()))]
    fn write_region(
        &self,
        dimension: DimensionId,
        region_x: i32,
        region_z: i32,
        data: &HashMap<ChunkPos, Vec<u8>>,
    ) -> Result<()> {
        let region_path = self.region_path(dimension, region_x, region_z);
        debug!(path = %region_path.display(), "Writing region file");

        // Serialize region data.
        let serialized = bincode::serialize(data).context("Failed to serialize region")?;
        debug!(serialized_size = serialized.len(), "Region data serialized");

        // Compress with zstd (level 3 for balanced speed/compression).
        let compressed =
            zstd::encode_all(&serialized[..], 3).context("Failed to compress region")?;
        let compression_ratio = serialized.len() as f64 / compressed.len() as f64;
        debug!(
            compressed_size = compressed.len(),
            compression_ratio = format!("{:.2}x", compression_ratio),
            "Region data compressed"
        );

        // Compute CRC32.
        let mut hasher = Hasher::new();
        hasher.update(&compressed);
        let crc32 = hasher.finalize();
        debug!(crc32 = format!("{:08X}", crc32), "CRC32 computed");

        // Create header.
        let header = RegionHeader::new(crc32, compressed.len() as u32);

        // Write to file.
        if let Some(parent) = region_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create region directory {}", parent.display()))?;
        }
        let mut file = File::create(&region_path).context("Failed to create region file")?;
        file.write_all(&header.to_bytes())
            .context("Failed to write header")?;
        file.write_all(&compressed)
            .context("Failed to write payload")?;

        info!(
            path = %region_path.display(),
            "Region file written successfully"
        );
        Ok(())
    }

    /// Check if a chunk exists in storage.
    pub fn chunk_exists(&self, pos: ChunkPos) -> bool {
        self.chunk_exists_in_dimension(DimensionId::DEFAULT, pos)
    }

    /// Check if a chunk exists in storage in the specified dimension.
    pub fn chunk_exists_in_dimension(&self, dimension: DimensionId, pos: ChunkPos) -> bool {
        let (region_x, region_z) = chunk_to_region(pos);
        let region_path = self.region_path(dimension, region_x, region_z);

        if !region_path.exists() {
            return false;
        }

        // Try to load region map and verify the chunk key is present. Any parse error -> false.
        match self.load_region(dimension, region_x, region_z) {
            Ok(map) => map.contains_key(&pos),
            Err(err) => {
                tracing::warn!(
                    "Failed to inspect region {}: {}; returning chunk_exists=false",
                    region_path.display(),
                    err
                );
                false
            }
        }
    }

    fn write_world_blob<T: Serialize>(
        &self,
        path: &Path,
        magic: u32,
        version: u16,
        value: &T,
    ) -> Result<()> {
        let serialized = bincode::serialize(value).context("Failed to serialize world blob")?;
        let compressed =
            zstd::encode_all(&serialized[..], 3).context("Failed to compress world blob")?;

        let mut hasher = Hasher::new();
        hasher.update(&compressed);
        let crc32 = hasher.finalize();

        let header = WorldBlobHeader::new(magic, version, crc32, compressed.len() as u32);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create save directory {}", parent.display()))?;
        }

        let mut file = File::create(path).context("Failed to create world blob file")?;
        file.write_all(&header.to_bytes())
            .context("Failed to write world blob header")?;
        file.write_all(&compressed)
            .context("Failed to write world blob payload")?;
        Ok(())
    }

    fn read_world_blob<T: for<'de> Deserialize<'de>>(
        &self,
        path: &Path,
        expected_magic: u32,
        expected_version: u16,
    ) -> Result<T> {
        let mut file = File::open(path).context("Failed to open world blob file")?;

        let mut header_bytes = [0u8; 14];
        file.read_exact(&mut header_bytes)
            .context("Failed to read world blob header")?;
        let header = WorldBlobHeader::from_bytes(&header_bytes)?;

        if header.magic != expected_magic {
            anyhow::bail!(
                "Invalid world blob magic: expected 0x{:08X}, got 0x{:08X}",
                expected_magic,
                header.magic
            );
        }

        if header.version != expected_version {
            anyhow::bail!(
                "Unsupported world blob version {} (expected {}). World upgrade required.",
                header.version,
                expected_version
            );
        }

        let mut compressed = vec![0u8; header.payload_len as usize];
        file.read_exact(&mut compressed)
            .context("Failed to read world blob payload")?;

        let mut hasher = Hasher::new();
        hasher.update(&compressed);
        let computed_crc = hasher.finalize();

        if computed_crc != header.crc32 {
            anyhow::bail!(
                "World blob CRC32 mismatch: expected {:08X}, got {:08X}",
                header.crc32,
                computed_crc
            );
        }

        let decompressed =
            zstd::decode_all(&compressed[..]).context("Failed to decompress world blob")?;
        let decoded = bincode::deserialize(&decompressed).context("Failed to decode world blob")?;
        Ok(decoded)
    }
}

/// Serialize a chunk to bytes (bincode format).
fn serialize_chunk(chunk: &Chunk) -> Result<Vec<u8>> {
    // Extract voxel data from chunk.
    let voxels = chunk.voxels();

    // Serialize as raw voxel array.
    bincode::serialize(voxels).context("Failed to serialize chunk data")
}

/// Deserialize a chunk from bytes.
fn deserialize_chunk(pos: ChunkPos, data: &[u8]) -> Result<Chunk> {
    let voxels: Vec<Voxel> =
        bincode::deserialize(data).context("Failed to deserialize chunk data")?;

    if voxels.len() != CHUNK_VOLUME {
        anyhow::bail!(
            "Invalid chunk data: expected {} voxels, got {}",
            CHUNK_VOLUME,
            voxels.len()
        );
    }

    // Create chunk and populate with voxel data.
    // Voxel index formula: (y * CHUNK_SIZE_Z + z) * CHUNK_SIZE_X + x
    let mut chunk = Chunk::new(pos);
    for (idx, &voxel) in voxels.iter().enumerate() {
        let x = idx % 16;
        let z = (idx / 16) % 16;
        let y = idx / (16 * 16);
        chunk.set_voxel(x, y, z, voxel);
    }

    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::BLOCK_AIR;
    use mdminecraft_core::DimensionId;
    use std::env;

    #[test]
    fn region_header_roundtrip() {
        let header = RegionHeader::new(0xDEADBEEF, 1234);
        let bytes = header.to_bytes();
        let decoded = RegionHeader::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.magic, REGION_MAGIC);
        assert_eq!(decoded.version, REGION_VERSION);
        assert_eq!(decoded.crc32, 0xDEADBEEF);
        assert_eq!(decoded.payload_len, 1234);
    }

    #[test]
    fn chunk_to_region_coords() {
        assert_eq!(chunk_to_region(ChunkPos::new(0, 0)), (0, 0));
        assert_eq!(chunk_to_region(ChunkPos::new(31, 31)), (0, 0));
        assert_eq!(chunk_to_region(ChunkPos::new(32, 32)), (1, 1));
        assert_eq!(chunk_to_region(ChunkPos::new(-1, -1)), (-1, -1));
        assert_eq!(chunk_to_region(ChunkPos::new(-32, -32)), (-1, -1));
        assert_eq!(chunk_to_region(ChunkPos::new(-33, -33)), (-2, -2));
    }

    #[test]
    fn save_and_load_chunk() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_save_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Create a test chunk with some data.
        let pos = ChunkPos::new(5, 10);
        let mut chunk = Chunk::new(pos);

        let test_voxel = Voxel {
            id: 42,
            state: 1,
            light_sky: 15,
            light_block: 0,
        };
        chunk.set_voxel(8, 64, 8, test_voxel);

        // Save chunk.
        store.save_chunk(&chunk).expect("Failed to save chunk");

        // Load chunk.
        let loaded = store.load_chunk(pos).expect("Failed to load chunk");

        // Verify data.
        assert_eq!(loaded.position(), pos);
        let loaded_voxel = loaded.voxel(8, 64, 8);
        assert_eq!(loaded_voxel.id, 42);
        assert_eq!(loaded_voxel.state, 1);
        assert_eq!(loaded_voxel.light_sky, 15);

        // Verify air blocks are preserved.
        let air_voxel = loaded.voxel(0, 0, 0);
        assert_eq!(air_voxel.id, BLOCK_AIR);

        // Cleanup.
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn save_and_load_chunk_across_dimensions() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_dims_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        let pos = ChunkPos::new(0, 0);

        let mut overworld = Chunk::new(pos);
        overworld.set_voxel(
            1,
            64,
            1,
            Voxel {
                id: 1,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        store
            .save_chunk_in_dimension(DimensionId::Overworld, &overworld)
            .unwrap();

        let mut nether = Chunk::new(pos);
        nether.set_voxel(
            1,
            64,
            1,
            Voxel {
                id: 2,
                state: 0,
                light_sky: 0,
                light_block: 0,
            },
        );
        store
            .save_chunk_in_dimension(DimensionId::Nether, &nether)
            .unwrap();

        assert!(store.chunk_exists_in_dimension(DimensionId::Overworld, pos));
        assert!(store.chunk_exists_in_dimension(DimensionId::Nether, pos));

        let loaded_overworld = store
            .load_chunk_in_dimension(DimensionId::Overworld, pos)
            .unwrap();
        let loaded_nether = store
            .load_chunk_in_dimension(DimensionId::Nether, pos)
            .unwrap();

        assert_eq!(loaded_overworld.voxel(1, 64, 1).id, 1);
        assert_eq!(loaded_nether.voxel(1, 64, 1).id, 2);

        assert!(
            temp_dir.join("dimensions").join("nether").exists(),
            "Nether directory should be created"
        );

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn multiple_chunks_in_same_region() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_multi_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Create multiple chunks in same region (0, 0).
        let pos1 = ChunkPos::new(0, 0);
        let pos2 = ChunkPos::new(15, 20);

        let mut chunk1 = Chunk::new(pos1);
        let mut chunk2 = Chunk::new(pos2);

        let voxel1 = Voxel {
            id: 10,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };
        let voxel2 = Voxel {
            id: 20,
            state: 0,
            light_sky: 15,
            light_block: 0,
        };

        chunk1.set_voxel(0, 0, 0, voxel1);
        chunk2.set_voxel(1, 1, 1, voxel2);

        // Save both chunks.
        store.save_chunk(&chunk1).unwrap();
        store.save_chunk(&chunk2).unwrap();

        // Load and verify both chunks.
        let loaded1 = store.load_chunk(pos1).unwrap();
        let loaded2 = store.load_chunk(pos2).unwrap();

        assert_eq!(loaded1.voxel(0, 0, 0).id, 10);
        assert_eq!(loaded2.voxel(1, 1, 1).id, 20);

        // Cleanup.
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn chunk_exists_returns_false_when_missing_from_region() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_exists_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Save a single chunk in region (0,0)
        let pos_present = ChunkPos::new(0, 0);
        let chunk = Chunk::new(pos_present);
        store.save_chunk(&chunk).unwrap();

        // Different chunk in same region (0,1) should report false without load error.
        let pos_absent = ChunkPos::new(0, 1);
        assert!(!store.chunk_exists(pos_absent));

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn load_chunk_not_found_errors() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_notfound_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Save one chunk
        let pos_present = ChunkPos::new(5, 5);
        let chunk = Chunk::new(pos_present);
        store.save_chunk(&chunk).unwrap();

        // Try to load a different chunk in same region
        let pos_absent = ChunkPos::new(5, 6);
        let result = store.load_chunk(pos_absent);
        assert!(result.is_err());

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn load_chunk_region_not_found_errors() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_noregion_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Try to load chunk from non-existent region
        let pos = ChunkPos::new(100, 100);
        let result = store.load_chunk(pos);
        assert!(result.is_err());

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn chunk_exists_returns_false_for_missing_region() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_nomiss_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Check chunk in non-existent region
        let pos = ChunkPos::new(1000, 1000);
        assert!(!store.chunk_exists(pos));

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn chunk_to_region_negative_coords() {
        // Test negative chunk coords map to negative regions
        let pos = ChunkPos::new(-1, -1);
        let (rx, rz) = chunk_to_region(pos);
        assert_eq!(rx, -1);
        assert_eq!(rz, -1);

        let pos2 = ChunkPos::new(-32, -32);
        let (rx2, rz2) = chunk_to_region(pos2);
        assert_eq!(rx2, -1);
        assert_eq!(rz2, -1);
    }

    #[test]
    fn save_and_load_chunk_different_regions() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_diffregion_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        // Save chunks in different regions
        let pos1 = ChunkPos::new(0, 0);
        let pos2 = ChunkPos::new(32, 0); // Different region (1, 0)
        let pos3 = ChunkPos::new(0, 32); // Different region (0, 1)

        let mut chunk1 = Chunk::new(pos1);
        let mut chunk2 = Chunk::new(pos2);
        let mut chunk3 = Chunk::new(pos3);

        chunk1.set_voxel(0, 0, 0, Voxel {
            id: 1,
            state: 0,
            light_sky: 15,
            light_block: 0,
        });
        chunk2.set_voxel(0, 0, 0, Voxel {
            id: 2,
            state: 0,
            light_sky: 15,
            light_block: 0,
        });
        chunk3.set_voxel(0, 0, 0, Voxel {
            id: 3,
            state: 0,
            light_sky: 15,
            light_block: 0,
        });

        store.save_chunk(&chunk1).unwrap();
        store.save_chunk(&chunk2).unwrap();
        store.save_chunk(&chunk3).unwrap();

        // Load and verify
        let loaded1 = store.load_chunk(pos1).unwrap();
        let loaded2 = store.load_chunk(pos2).unwrap();
        let loaded3 = store.load_chunk(pos3).unwrap();

        assert_eq!(loaded1.voxel(0, 0, 0).id, 1);
        assert_eq!(loaded2.voxel(0, 0, 0).id, 2);
        assert_eq!(loaded3.voxel(0, 0, 0).id, 3);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn region_header_too_short_fails() {
        // Test parsing a header that's too short
        let short_bytes = vec![0u8; 10]; // Too short
        let result = RegionHeader::from_bytes(&short_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn region_header_invalid_magic_fails() {
        // Create bytes with wrong magic
        let mut bytes = vec![0u8; 14];
        // Set wrong magic (not REGION_MAGIC)
        bytes[0] = 0xFF;
        bytes[1] = 0xFF;
        bytes[2] = 0xFF;
        bytes[3] = 0xFF;

        let result = RegionHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn chunk_to_region_boundary_values() {
        // Test boundary values
        let pos = ChunkPos::new(31, 31); // Last chunk in region (0,0)
        let (rx, rz) = chunk_to_region(pos);
        assert_eq!(rx, 0);
        assert_eq!(rz, 0);

        let pos2 = ChunkPos::new(32, 32); // First chunk in region (1,1)
        let (rx2, rz2) = chunk_to_region(pos2);
        assert_eq!(rx2, 1);
        assert_eq!(rz2, 1);
    }

    #[test]
    fn world_meta_roundtrip() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_meta_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        let meta = WorldMeta { world_seed: 12345 };
        store.save_world_meta(&meta).unwrap();
        assert!(store.world_meta_exists());

        let loaded = store.load_world_meta().unwrap();
        assert_eq!(loaded, meta);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn world_state_roundtrip() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("mdminecraft_test_state_{}", timestamp));
        let store = RegionStore::new(&temp_dir).unwrap();

        let mut time = SimTime::new(24000);
        time.tick = SimTick(777);
        let mut weather = WeatherToggle::new();
        weather.toggle();

        let state = WorldState {
            tick: SimTick(777),
            sim_time: time,
            weather,
            weather_timer_seconds: 12.5,
            next_weather_change_seconds: 99.0,
        };

        store.save_world_state(&state).unwrap();
        assert!(store.world_state_exists());

        let loaded = store.load_world_state().unwrap();
        assert_eq!(loaded, state);

        fs::remove_dir_all(&temp_dir).ok();
    }
}
