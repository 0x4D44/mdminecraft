//! Chunk data encoding with palette and RLE compression.
//!
//! Provides efficient compression for chunk data transmission over the network.

use crate::protocol::{BlockId, ChunkDataMessage};
use anyhow::{Context, Result};
use mdminecraft_core::DimensionId;
use std::collections::HashMap;

/// Encode chunk data with palette and RLE compression.
///
/// Process:
/// 1. Build palette of unique block IDs in the chunk
/// 2. Replace block IDs with palette indices
/// 3. Run-length encode the palette indices
/// 4. Calculate CRC32 for validation
///
/// Typical compression: 80-95% for natural terrain.
pub fn encode_chunk_data(
    dimension: DimensionId,
    chunk_x: i32,
    chunk_z: i32,
    block_data: &[BlockId],
) -> Result<ChunkDataMessage> {
    if block_data.len() != 65536 {
        return Err(anyhow::anyhow!(
            "Invalid chunk data size: expected 65536, got {}",
            block_data.len()
        ));
    }

    // Step 1: Build palette
    let (palette, indices) = build_palette(block_data);

    // Step 2: RLE compress the indices
    let compressed_data = rle_compress(&indices);

    // Step 3: Calculate CRC32
    let crc32 = calculate_crc32(&palette, &compressed_data);

    Ok(ChunkDataMessage {
        dimension,
        chunk_x,
        chunk_z,
        palette,
        compressed_data,
        crc32,
    })
}

/// Decode chunk data from palette and RLE compressed format.
pub fn decode_chunk_data(msg: &ChunkDataMessage) -> Result<Vec<BlockId>> {
    // Validate CRC32
    let expected_crc = calculate_crc32(&msg.palette, &msg.compressed_data);
    if msg.crc32 != expected_crc {
        return Err(anyhow::anyhow!(
            "CRC32 mismatch: expected {:08x}, got {:08x}",
            expected_crc,
            msg.crc32
        ));
    }

    // Decompress RLE data
    let indices = rle_decompress(&msg.compressed_data).context("Failed to decompress RLE data")?;

    if indices.len() != 65536 {
        return Err(anyhow::anyhow!(
            "Invalid decompressed size: expected 65536, got {}",
            indices.len()
        ));
    }

    // Map indices back to block IDs using palette
    let mut block_data = Vec::with_capacity(65536);
    for &index in &indices {
        if (index as usize) >= msg.palette.len() {
            return Err(anyhow::anyhow!(
                "Invalid palette index: {} (palette size: {})",
                index,
                msg.palette.len()
            ));
        }
        block_data.push(msg.palette[index as usize]);
    }

    Ok(block_data)
}

/// Build palette and convert block IDs to palette indices.
fn build_palette(block_data: &[BlockId]) -> (Vec<BlockId>, Vec<u8>) {
    let mut palette = Vec::new();
    let mut palette_map: HashMap<BlockId, u8> = HashMap::new();
    let mut indices = Vec::with_capacity(block_data.len());

    for &block_id in block_data {
        let index = if let Some(&idx) = palette_map.get(&block_id) {
            idx
        } else {
            let idx = palette.len() as u8;
            if idx == 255 {
                // Palette full, use last slot for overflow
                // This is a simplification; real implementation might use larger indices
                255
            } else {
                palette.push(block_id);
                palette_map.insert(block_id, idx);
                idx
            }
        };
        indices.push(index);
    }

    (palette, indices)
}

/// Run-length encode a sequence of bytes.
///
/// Format: [count: u8][value: u8]...
/// If count >= 128, it's a run. Otherwise, it's literal bytes.
fn rle_compress(data: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let current = data[i];
        let mut run_length = 1;

        // Count consecutive identical values
        while i + run_length < data.len() && data[i + run_length] == current && run_length < 127 {
            run_length += 1;
        }

        if run_length >= 3 {
            // Encode as run: [128 + length][value]
            compressed.push(128 + run_length as u8);
            compressed.push(current);
            i += run_length;
        } else {
            // Encode as literal sequence
            let mut literal_length = 1;
            while i + literal_length < data.len() && literal_length < 127 {
                // Check if we're about to hit a long run
                if i + literal_length + 2 < data.len()
                    && data[i + literal_length] == data[i + literal_length + 1]
                    && data[i + literal_length] == data[i + literal_length + 2]
                {
                    break;
                }
                literal_length += 1;
            }

            // Encode literal: [length][bytes...]
            compressed.push(literal_length as u8);
            compressed.extend_from_slice(&data[i..i + literal_length]);
            i += literal_length;
        }
    }

    compressed
}

/// Maximum decompressed size for RLE data (chunk size = 16 * 256 * 16 = 65536).
/// Prevents decompression bombs from exhausting memory.
const MAX_DECOMPRESSED_SIZE: usize = 65536;

/// Run-length decode a compressed sequence with output size limit.
///
/// # Security
/// Limits output to MAX_DECOMPRESSED_SIZE bytes to prevent decompression bombs.
fn rle_decompress(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut decompressed = Vec::with_capacity(MAX_DECOMPRESSED_SIZE.min(compressed.len() * 127));
    let mut i = 0;

    while i < compressed.len() {
        let control = compressed[i];
        i += 1;

        if control >= 128 {
            // Run: repeat next byte (control - 128) times
            let length = (control - 128) as usize;
            if i >= compressed.len() {
                return Err(anyhow::anyhow!("Unexpected end of RLE data (run)"));
            }
            let value = compressed[i];
            i += 1;

            // Check size limit before expanding
            if decompressed.len() + length > MAX_DECOMPRESSED_SIZE {
                return Err(anyhow::anyhow!(
                    "RLE decompression would exceed max size: {} + {} > {}",
                    decompressed.len(),
                    length,
                    MAX_DECOMPRESSED_SIZE
                ));
            }
            decompressed.extend(std::iter::repeat_n(value, length));
        } else {
            // Literal: copy next (control) bytes
            let length = control as usize;
            if i + length > compressed.len() {
                return Err(anyhow::anyhow!(
                    "Unexpected end of RLE data (literal): need {} bytes, have {}",
                    length,
                    compressed.len() - i
                ));
            }

            // Check size limit before copying
            if decompressed.len() + length > MAX_DECOMPRESSED_SIZE {
                return Err(anyhow::anyhow!(
                    "RLE decompression would exceed max size: {} + {} > {}",
                    decompressed.len(),
                    length,
                    MAX_DECOMPRESSED_SIZE
                ));
            }
            decompressed.extend_from_slice(&compressed[i..i + length]);
            i += length;
        }
    }

    Ok(decompressed)
}

/// Calculate CRC32 checksum for chunk data.
fn calculate_crc32(palette: &[BlockId], compressed_data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();

    // Hash palette
    for &block_id in palette {
        hasher.update(&block_id.to_le_bytes());
    }

    // Hash compressed data
    hasher.update(compressed_data);

    hasher.finalize()
}

/// Calculate compression ratio as a percentage.
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f32 {
    if original_size == 0 {
        return 0.0;
    }
    ((original_size - compressed_size) as f32 / original_size as f32) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_single_block() {
        let data = vec![1u16; 65536];
        let (palette, indices) = build_palette(&data);

        assert_eq!(palette.len(), 1);
        assert_eq!(palette[0], 1);
        assert_eq!(indices.len(), 65536);
        assert!(indices.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_palette_multiple_blocks() {
        let mut data = vec![0u16; 65536];
        data[0] = 1;
        data[1] = 2;
        data[2] = 1;
        data[3] = 3;

        let (palette, _indices) = build_palette(&data);

        assert!(palette.contains(&0));
        assert!(palette.contains(&1));
        assert!(palette.contains(&2));
        assert!(palette.contains(&3));
        assert_eq!(palette.len(), 4);
    }

    #[test]
    fn test_rle_compress_simple_run() {
        let data = vec![5u8; 10];
        let compressed = rle_compress(&data);

        // Should be encoded as run: [128 + 10][5]
        assert_eq!(compressed.len(), 2);
        assert_eq!(compressed[0], 128 + 10);
        assert_eq!(compressed[1], 5);
    }

    #[test]
    fn test_rle_compress_mixed() {
        let data = vec![1, 1, 1, 2, 3, 4, 5, 5, 5, 5];
        let compressed = rle_compress(&data);

        // Should compress the runs of 1s and 5s
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_rle_roundtrip() {
        let original = vec![
            1, 1, 1, 1, 2, 3, 4, 5, 5, 5, 6, 7, 8, 8, 8, 8, 8, 9, 10, 11, 12, 12, 12,
        ];
        let compressed = rle_compress(&original);
        let decompressed = rle_decompress(&compressed).expect("Failed to decompress");

        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_encode_decode_uniform_chunk() {
        let block_data = vec![1u16; 65536];
        let encoded =
            encode_chunk_data(DimensionId::DEFAULT, 0, 0, &block_data).expect("Failed to encode");
        assert_eq!(encoded.dimension, DimensionId::DEFAULT);

        assert_eq!(encoded.palette.len(), 1);
        assert_eq!(encoded.palette[0], 1);

        // Should have high compression ratio for uniform data
        let original_size = 65536 * 2; // 2 bytes per BlockId
        let compressed_size = encoded.compressed_data.len() + encoded.palette.len() * 2;
        assert!(compressed_size < original_size / 10); // >90% compression

        let decoded = decode_chunk_data(&encoded).expect("Failed to decode");
        assert_eq!(decoded, block_data);
    }

    #[test]
    fn test_encode_decode_varied_chunk() {
        let mut block_data = vec![0u16; 65536];
        // Create some variation
        for (i, value) in block_data.iter_mut().take(1000).enumerate() {
            *value = (i % 10) as u16;
        }

        let encoded =
            encode_chunk_data(DimensionId::DEFAULT, 5, -3, &block_data).expect("Failed to encode");

        assert_eq!(encoded.dimension, DimensionId::DEFAULT);
        assert_eq!(encoded.chunk_x, 5);
        assert_eq!(encoded.chunk_z, -3);
        assert!(encoded.palette.len() <= 10);

        let decoded = decode_chunk_data(&encoded).expect("Failed to decode");
        assert_eq!(decoded, block_data);
    }

    #[test]
    fn test_crc32_validation() {
        let block_data = vec![1u16; 65536];
        let mut encoded =
            encode_chunk_data(DimensionId::DEFAULT, 0, 0, &block_data).expect("Failed to encode");

        // Corrupt the CRC
        encoded.crc32 ^= 0xFFFFFFFF;

        let result = decode_chunk_data(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC32 mismatch"));
    }

    #[test]
    fn test_compression_ratio_calculation() {
        let ratio = compression_ratio(1000, 200);
        assert_eq!(ratio, 80.0);

        let ratio = compression_ratio(1000, 500);
        assert_eq!(ratio, 50.0);

        let ratio = compression_ratio(0, 0);
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn test_invalid_chunk_size() {
        let block_data = vec![1u16; 100]; // Wrong size
        let result = encode_chunk_data(DimensionId::DEFAULT, 0, 0, &block_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_rle_decompression_bomb_prevention_run() {
        // Create malicious RLE data that tries to decompress to more than MAX_DECOMPRESSED_SIZE
        // Each run control byte can specify up to 127 bytes
        // We need > 65536 bytes output to trigger the limit

        // Create data that would decompress to 127 * 600 = 76200 bytes (> 65536)
        let mut malicious_data = Vec::new();
        for _ in 0..600 {
            malicious_data.push(255); // 128 + 127 = run of 127
            malicious_data.push(0); // value to repeat
        }

        let result = rle_decompress(&malicious_data);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceed max size"), "Error was: {}", err);
    }

    #[test]
    fn test_rle_decompression_bomb_prevention_literal() {
        // Create malicious data with many literal sequences
        let mut malicious_data = Vec::new();

        // Fill with literal sequences to exceed MAX_DECOMPRESSED_SIZE
        // We'll use max literal length (127) repeatedly
        for _ in 0..600 {
            malicious_data.push(127); // literal length
            malicious_data.extend(vec![0u8; 127]); // literal data
        }

        let result = rle_decompress(&malicious_data);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceed max size"), "Error was: {}", err);
    }

    #[test]
    fn test_rle_decompression_exactly_max_size() {
        // Create data that decompresses to exactly MAX_DECOMPRESSED_SIZE (65536)
        // Using runs of 127 bytes: 65536 / 127 = 516.0...
        // 516 * 127 = 65532, need 4 more

        let mut valid_data = Vec::new();
        for _ in 0..516 {
            valid_data.push(255); // 128 + 127 = run of 127
            valid_data.push(0); // value to repeat
        }
        // Add remaining 4 bytes as a short run
        valid_data.push(128 + 4); // run of 4
        valid_data.push(0);

        let result = rle_decompress(&valid_data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 65536);
    }

    #[test]
    fn test_rle_decompression_one_over_max() {
        // Create data that would decompress to MAX_DECOMPRESSED_SIZE + 1
        let mut malicious_data = Vec::new();
        for _ in 0..516 {
            malicious_data.push(255); // run of 127
            malicious_data.push(0);
        }
        // 516 * 127 = 65532, add 5 more to exceed limit
        malicious_data.push(128 + 5); // run of 5
        malicious_data.push(0);

        let result = rle_decompress(&malicious_data);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceed max size"), "Error was: {}", err);
    }

    #[test]
    fn test_max_decompressed_size_constant() {
        // Verify the constant matches the expected chunk size
        assert_eq!(MAX_DECOMPRESSED_SIZE, 65536);
        assert_eq!(MAX_DECOMPRESSED_SIZE, 16 * 256 * 16);
    }
}
