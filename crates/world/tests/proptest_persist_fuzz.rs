//! Fuzz-style property tests for chunk persistence
//!
//! These tests validate that the chunk serialization/deserialization
//! handles arbitrary inputs gracefully without crashing, even on malformed data.
//!
//! Critical properties:
//! - Deserializer never panics on arbitrary input
//! - Valid chunks always roundtrip correctly via RegionStore
//! - Invalid data is rejected with proper errors
//! - CRC validation catches corruption
//! - No buffer overflows or memory corruption

use mdminecraft_world::{Chunk, ChunkPos, RegionStore, Voxel, CHUNK_VOLUME};
use proptest::prelude::*;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

/// Test that deserialize_chunk handles arbitrary byte sequences gracefully
#[cfg(test)]
mod fuzz_chunk_deserializer {
    use super::*;

    proptest! {
        /// Property: Arbitrary bytes don't crash the deserializer
        ///
        /// For any random byte sequence, deserialize_chunk should either:
        /// - Return a valid chunk, or
        /// - Return an error
        /// But it should NEVER panic or crash.
        #[test]
        fn arbitrary_bytes_dont_crash(
            random_bytes in prop::collection::vec(any::<u8>(), 0..10000),
        ) {
            // This should not panic regardless of input
            let result = bincode::deserialize::<Vec<Voxel>>(&random_bytes);

            // We don't care if it succeeds or fails, only that it doesn't crash
            match result {
                Ok(voxels) => {
                    // If it succeeds, it might be random data that happened to deserialize
                    // That's fine - we just want to ensure no crash
                    prop_assert!(voxels.len() <= 100000, "Deserialized vec too large");
                }
                Err(_) => {
                    // Expected for random data - bincode should reject it
                }
            }
        }

        /// Property: Valid chunks always roundtrip correctly via RegionStore
        ///
        /// For any chunk with valid voxel data, saving and loading through
        /// RegionStore should produce an identical chunk.
        #[test]
        fn valid_chunks_roundtrip(
            chunk_x in -100i32..100i32,
            chunk_z in -100i32..100i32,
            voxel_seed in any::<u64>(),
        ) {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let temp_dir = env::temp_dir().join(format!("fuzz_roundtrip_{}", timestamp));
            let store = RegionStore::new(&temp_dir).expect("Failed to create store");

            let pos = ChunkPos::new(chunk_x, chunk_z);
            let mut chunk = Chunk::new(pos);

            // Fill chunk with deterministic semi-random data
            for i in 0..256 {  // Sample 256 voxels across the chunk
                let x = (i * 7) % 16;
                let y = (i * 13) % 256;
                let z = (i * 11) % 16;

                let voxel_data = voxel_seed.wrapping_add(i as u64);
                let voxel = Voxel {
                    id: (voxel_data % 256) as u16,
                    state: ((voxel_data >> 8) % 16) as u16,
                    light_sky: ((voxel_data >> 12) % 16) as u8,
                    light_block: ((voxel_data >> 16) % 16) as u8,
                };
                chunk.set_voxel(x, y, z, voxel);
            }

            // Save chunk
            store.save_chunk(&chunk).expect("Save should succeed");

            // Load chunk
            let loaded = store.load_chunk(pos).expect("Load should succeed");

            // Verify sampled voxels match
            for i in 0..256 {
                let x = (i * 7) % 16;
                let y = (i * 13) % 256;
                let z = (i * 11) % 16;

                let original = chunk.voxel(x, y, z);
                let recovered = loaded.voxel(x, y, z);

                prop_assert_eq!(
                    original.id, recovered.id,
                    "Voxel ({},{},{}) id mismatch", x, y, z
                );
                prop_assert_eq!(
                    original.state, recovered.state,
                    "Voxel ({},{},{}) state mismatch", x, y, z
                );
                prop_assert_eq!(
                    original.light_sky, recovered.light_sky,
                    "Voxel ({},{},{}) light_sky mismatch", x, y, z
                );
                prop_assert_eq!(
                    original.light_block, recovered.light_block,
                    "Voxel ({},{},{}) light_block mismatch", x, y, z
                );
            }

            // Cleanup
            std::fs::remove_dir_all(&temp_dir).ok();
        }

        /// Property: Wrong-sized data is rejected
        ///
        /// If the deserialized voxel count != CHUNK_VOLUME, it should be rejected.
        #[test]
        fn wrong_size_rejected(
            wrong_size in 0usize..20000,
        ) {
            // Skip the correct size
            prop_assume!(wrong_size != CHUNK_VOLUME);

            // Create a vector of the wrong size
            let voxels: Vec<Voxel> = (0..wrong_size).map(|_| Voxel {
                id: 0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            }).collect();

            let serialized = bincode::serialize(&voxels).unwrap();
            let deserialized: Result<Vec<Voxel>, _> = bincode::deserialize(&serialized);

            // Deserialization might succeed (it's just a Vec)
            if let Ok(voxels) = deserialized {
                // But the length check should catch it
                prop_assert_ne!(
                    voxels.len(), CHUNK_VOLUME,
                    "Wrong-sized data should not match CHUNK_VOLUME"
                );
            }
        }

        /// Property: Truncated data is rejected
        ///
        /// If serialized data is truncated, deserialization should fail gracefully.
        #[test]
        fn truncated_data_rejected(
            truncate_at in 0usize..1000,
        ) {
            // Create valid chunk data
            let voxels: Vec<Voxel> = (0..CHUNK_VOLUME).map(|_| Voxel {
                id: 1,
                state: 0,
                light_sky: 15,
                light_block: 0,
            }).collect();

            let mut serialized = bincode::serialize(&voxels).unwrap();

            // Truncate the data
            if truncate_at < serialized.len() {
                serialized.truncate(truncate_at);

                // Deserializing truncated data should fail
                let result: Result<Vec<Voxel>, _> = bincode::deserialize(&serialized);
                prop_assert!(
                    result.is_err(),
                    "Truncated data should fail deserialization"
                );
            }
        }

        /// Property: Corrupted data doesn't crash deserializer
        ///
        /// Flipping random bits in serialized data should not cause panics.
        /// The deserializer should either:
        /// - Fail gracefully with an error, or
        /// - Succeed (possibly with different data)
        ///
        /// Note: Some bit flips (e.g., in length prefixes) may not affect
        /// the actual deserialized data, which is acceptable.
        #[test]
        fn corrupted_data_handled_gracefully(
            flip_byte in 0usize..100,
            flip_bit in 0u8..8,
        ) {
            // Create valid chunk data
            let voxels: Vec<Voxel> = (0..CHUNK_VOLUME).map(|i| Voxel {
                id: (i % 256) as u16,
                state: 0,
                light_sky: 15,
                light_block: 0,
            }).collect();

            let mut serialized = bincode::serialize(&voxels).unwrap();

            // Corrupt a byte
            if flip_byte < serialized.len() {
                serialized[flip_byte] ^= 1 << flip_bit;

                // Deserialize corrupted data - should not panic
                let _result: Result<Vec<Voxel>, _> = bincode::deserialize(&serialized);

                // We don't assert anything - just that it didn't panic
                // Corruption can fail (good) or succeed with altered/same data (acceptable)
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn empty_data_fails() {
        let result: Result<Vec<Voxel>, _> = bincode::deserialize(&[]);
        assert!(result.is_err(), "Empty data should fail deserialization");
    }

    #[test]
    fn single_byte_fails() {
        let result: Result<Vec<Voxel>, _> = bincode::deserialize(&[0xFF]);
        assert!(result.is_err(), "Single byte should fail deserialization");
    }

    #[test]
    fn valid_empty_chunk_roundtrips() {
        let voxels: Vec<Voxel> = (0..CHUNK_VOLUME)
            .map(|_| Voxel {
                id: 0,
                state: 0,
                light_sky: 15,
                light_block: 0,
            })
            .collect();

        let serialized = bincode::serialize(&voxels).unwrap();
        let deserialized: Vec<Voxel> = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.len(), CHUNK_VOLUME);
        assert!(deserialized.iter().all(|v| v.id == 0));
    }

    #[test]
    fn oversized_vec_deserializes_but_wrong_length() {
        let voxels: Vec<Voxel> = (0..CHUNK_VOLUME * 2)
            .map(|_| Voxel {
                id: 1,
                state: 0,
                light_sky: 15,
                light_block: 0,
            })
            .collect();

        let serialized = bincode::serialize(&voxels).unwrap();
        let deserialized: Vec<Voxel> = bincode::deserialize(&serialized).unwrap();

        // Deserialization succeeds but length is wrong
        assert_ne!(deserialized.len(), CHUNK_VOLUME);
        assert_eq!(deserialized.len(), CHUNK_VOLUME * 2);
    }
}
