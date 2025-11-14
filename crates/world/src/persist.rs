//! Region-based chunk persistence with zstd compression.
//!
//! Implements .rg region files that group 32x32 chunks for efficient storage.
//! Each region file uses zstd compression and CRC32 validation.

use crate::chunk::{Chunk, ChunkPos, Voxel, CHUNK_VOLUME};
use anyhow::{Context, Result};
use crc32fast::Hasher;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Magic number for region file identification ("MDRG" = mdminecraft region).
const REGION_MAGIC: u32 = 0x4D445247;

/// Current region file format version.
const REGION_VERSION: u16 = 1;

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

    /// Get the path to a region file for the given region coordinates.
    fn region_path(&self, region_x: i32, region_z: i32) -> PathBuf {
        self.world_dir
            .join(format!("r.{}.{}.rg", region_x, region_z))
    }

    /// Save a chunk to its region file.
    pub fn save_chunk(&self, chunk: &Chunk) -> Result<()> {
        let (region_x, region_z) = chunk_to_region(chunk.position());

        // Load existing region or create new one.
        let mut region_data = self.load_region(region_x, region_z).unwrap_or_default();

        // Serialize chunk data.
        let chunk_data = serialize_chunk(chunk)?;

        // Store in region map.
        region_data.insert(chunk.position(), chunk_data);

        // Write region file.
        self.write_region(region_x, region_z, &region_data)?;

        Ok(())
    }

    /// Load a chunk from its region file.
    pub fn load_chunk(&self, pos: ChunkPos) -> Result<Chunk> {
        let (region_x, region_z) = chunk_to_region(pos);
        let region_data = self.load_region(region_x, region_z)?;

        let chunk_data = region_data.get(&pos).context("Chunk not found in region")?;

        deserialize_chunk(pos, chunk_data)
    }

    /// Load an entire region file into memory.
    fn load_region(&self, region_x: i32, region_z: i32) -> Result<HashMap<ChunkPos, Vec<u8>>> {
        let region_path = self.region_path(region_x, region_z);

        if !region_path.exists() {
            anyhow::bail!("Region file does not exist");
        }

        let mut file = File::open(&region_path).context("Failed to open region file")?;

        // Read header.
        let mut header_bytes = [0u8; 14];
        file.read_exact(&mut header_bytes)
            .context("Failed to read region header")?;
        let header = RegionHeader::from_bytes(&header_bytes)?;

        // Read compressed payload.
        let mut compressed = vec![0u8; header.payload_len as usize];
        file.read_exact(&mut compressed)
            .context("Failed to read region payload")?;

        // Verify CRC32.
        let mut hasher = Hasher::new();
        hasher.update(&compressed);
        let computed_crc = hasher.finalize();

        if computed_crc != header.crc32 {
            anyhow::bail!(
                "CRC32 mismatch: expected {:08X}, got {:08X}",
                header.crc32,
                computed_crc
            );
        }

        // Decompress payload.
        let decompressed =
            zstd::decode_all(&compressed[..]).context("Failed to decompress region")?;

        // Deserialize region data.
        let region_data: HashMap<ChunkPos, Vec<u8>> =
            bincode::deserialize(&decompressed).context("Failed to deserialize region")?;

        Ok(region_data)
    }

    /// Write an entire region file to disk.
    fn write_region(
        &self,
        region_x: i32,
        region_z: i32,
        data: &HashMap<ChunkPos, Vec<u8>>,
    ) -> Result<()> {
        let region_path = self.region_path(region_x, region_z);

        // Serialize region data.
        let serialized = bincode::serialize(data).context("Failed to serialize region")?;

        // Compress with zstd (level 3 for balanced speed/compression).
        let compressed =
            zstd::encode_all(&serialized[..], 3).context("Failed to compress region")?;

        // Compute CRC32.
        let mut hasher = Hasher::new();
        hasher.update(&compressed);
        let crc32 = hasher.finalize();

        // Create header.
        let header = RegionHeader::new(crc32, compressed.len() as u32);

        // Write to file.
        let mut file = File::create(&region_path).context("Failed to create region file")?;
        file.write_all(&header.to_bytes())
            .context("Failed to write header")?;
        file.write_all(&compressed)
            .context("Failed to write payload")?;

        Ok(())
    }

    /// Check if a chunk exists in storage.
    pub fn chunk_exists(&self, pos: ChunkPos) -> bool {
        let (region_x, region_z) = chunk_to_region(pos);
        let region_path = self.region_path(region_x, region_z);

        // Check if region file exists (conservative check - doesn't verify chunk is in region).
        region_path.exists()
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
}
