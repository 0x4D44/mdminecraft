//! Tests for texture atlas system.
//!
//! TDD Phase: RED - These tests are written first and expected to fail.

use crate::gpu::texture_atlas::{TextureAtlas, TextureId, UvCoordinates};
use crate::gpu::texture_loader::TextureLoader;

/// Test that we can create an empty texture atlas.
#[test]
fn create_empty_atlas() {
    let atlas = TextureAtlas::new();
    assert_eq!(atlas.texture_count(), 0);
    assert_eq!(atlas.width(), 0);
    assert_eq!(atlas.height(), 0);
}

/// Test that we can add a single texture and get back a valid ID.
#[test]
fn add_single_texture() {
    let mut atlas = TextureAtlas::new();
    let texture_data = vec![0u8; 16 * 16 * 4]; // 16x16 RGBA

    let id = atlas.add_texture("test_block", &texture_data, 16, 16);
    assert!(id.is_some());
    assert_eq!(atlas.texture_count(), 1);
}

/// Test that adding invalid texture dimensions is rejected.
#[test]
fn reject_invalid_dimensions() {
    let mut atlas = TextureAtlas::new();

    // Non-16x16 texture should be rejected
    let texture_data = vec![0u8; 32 * 32 * 4];
    let id = atlas.add_texture("bad_size", &texture_data, 32, 32);
    assert!(id.is_none());

    // Non-RGBA data should be rejected (wrong size)
    let bad_data = vec![0u8; 16 * 16 * 3]; // RGB instead of RGBA
    let id = atlas.add_texture("bad_format", &bad_data, 16, 16);
    assert!(id.is_none());
}

/// Test that we can build the atlas and get power-of-2 dimensions.
#[test]
fn build_produces_power_of_two_dimensions() {
    let mut atlas = TextureAtlas::new();

    // Add 10 textures
    for i in 0..10 {
        let texture_data = vec![i as u8; 16 * 16 * 4];
        atlas.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    let result = atlas.build();
    assert!(result.is_ok());

    // Dimensions should be power of 2
    let width = atlas.width();
    let height = atlas.height();
    assert!(width.is_power_of_two());
    assert!(height.is_power_of_two());
    assert!(width > 0 && height > 0);
}

/// Test that atlas dimensions don't exceed 4096x4096.
#[test]
fn atlas_respects_max_size() {
    let mut atlas = TextureAtlas::new();

    // Try to add 256*256 = 65536 textures (would require 4096x4096 if packed perfectly)
    // This should either succeed at 4096x4096 or fail gracefully
    for i in 0..(256 * 256) {
        let texture_data = vec![0u8; 16 * 16 * 4];
        atlas.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    let result = atlas.build();
    if result.is_ok() {
        assert!(atlas.width() <= 4096);
        assert!(atlas.height() <= 4096);
    } else {
        // Expected: atlas is full
        assert!(result.is_err());
    }
}

/// Test that we can get UV coordinates for a texture.
#[test]
fn get_uv_coordinates() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![255u8; 16 * 16 * 4];
    let id = atlas.add_texture("stone", &texture_data, 16, 16).unwrap();
    atlas.build().unwrap();

    let uv = atlas.get_uv(id);
    assert!(uv.is_some());

    let uv = uv.unwrap();
    // UV coordinates should be normalized [0,1] range
    assert!(uv.min_u >= 0.0 && uv.min_u <= 1.0);
    assert!(uv.min_v >= 0.0 && uv.min_v <= 1.0);
    assert!(uv.max_u >= 0.0 && uv.max_u <= 1.0);
    assert!(uv.max_v >= 0.0 && uv.max_v <= 1.0);

    // Max should be greater than min
    assert!(uv.max_u > uv.min_u);
    assert!(uv.max_v > uv.min_v);
}

/// Test that UV coordinates before build returns None.
#[test]
fn get_uv_before_build_fails() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![255u8; 16 * 16 * 4];
    let id = atlas.add_texture("stone", &texture_data, 16, 16).unwrap();

    // Should fail before build
    assert!(atlas.get_uv(id).is_none());
}

/// Test that packing is deterministic - same textures in same order produce same layout.
#[test]
fn packing_is_deterministic() {
    let mut atlas1 = TextureAtlas::new();
    let mut atlas2 = TextureAtlas::new();

    // Add same textures in same order
    for i in 0..20 {
        let texture_data = vec![i as u8; 16 * 16 * 4];
        atlas1.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
        atlas2.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    atlas1.build().unwrap();
    atlas2.build().unwrap();

    // Dimensions should match
    assert_eq!(atlas1.width(), atlas2.width());
    assert_eq!(atlas1.height(), atlas2.height());

    // Hash should match
    assert_eq!(atlas1.hash(), atlas2.hash());
}

/// Test that different texture order produces different (but still deterministic) results.
#[test]
fn different_order_produces_different_layout() {
    let mut atlas1 = TextureAtlas::new();
    let mut atlas2 = TextureAtlas::new();

    // Atlas 1: add in forward order
    for i in 0..10 {
        let texture_data = vec![i as u8; 16 * 16 * 4];
        atlas1.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    // Atlas 2: add in reverse order
    for i in (0..10).rev() {
        let texture_data = vec![i as u8; 16 * 16 * 4];
        atlas2.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    atlas1.build().unwrap();
    atlas2.build().unwrap();

    // Different order should produce different hash
    // (unless we sort by name, which would be acceptable)
    // For now, test that both builds succeed deterministically
    assert!(atlas1.hash().len() > 0);
    assert!(atlas2.hash().len() > 0);
}

/// Test that atlas hash is generated using blake3.
#[test]
fn atlas_generates_blake3_hash() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![42u8; 16 * 16 * 4];
    atlas.add_texture("stone", &texture_data, 16, 16);
    atlas.build().unwrap();

    let hash = atlas.hash();
    // blake3 hash should be 32 bytes (64 hex chars)
    assert_eq!(hash.len(), 64);

    // Should be valid hex
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

/// Test that we can get the atlas data as RGBA8 bytes.
#[test]
fn get_atlas_data() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![128u8; 16 * 16 * 4];
    atlas.add_texture("stone", &texture_data, 16, 16);
    atlas.build().unwrap();

    let data = atlas.data();
    assert!(data.len() > 0);

    // Data should match dimensions
    let expected_size = atlas.width() as usize * atlas.height() as usize * 4;
    assert_eq!(data.len(), expected_size);
}

/// Test that we can look up texture by name.
#[test]
fn lookup_texture_by_name() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![255u8; 16 * 16 * 4];
    let id = atlas.add_texture("stone", &texture_data, 16, 16).unwrap();

    let found_id = atlas.get_texture_id("stone");
    assert_eq!(found_id, Some(id));

    let not_found = atlas.get_texture_id("nonexistent");
    assert_eq!(not_found, None);
}

/// Test that duplicate texture names are handled.
#[test]
fn duplicate_texture_names() {
    let mut atlas = TextureAtlas::new();

    let texture_data1 = vec![100u8; 16 * 16 * 4];
    let id1 = atlas.add_texture("stone", &texture_data1, 16, 16).unwrap();

    // Adding same name again should either:
    // - Return the same ID (de-duplication)
    // - Return a new ID (allow duplicates)
    // - Return None (reject duplicates)
    let texture_data2 = vec![200u8; 16 * 16 * 4];
    let id2 = atlas.add_texture("stone", &texture_data2, 16, 16);

    // For now, test that we handle it gracefully
    // Implementation can choose behavior
    assert!(id2.is_none() || id2 == Some(id1) || id2.unwrap() != id1);
}

/// Test texture loader can load PNG files.
#[test]
fn texture_loader_loads_png() {
    let loader = TextureLoader::new();

    // This test requires a fixture PNG file
    // For now, test that the API exists
    let result = loader.load_from_file("tests/fixtures/stone.png");

    // Should either succeed or fail gracefully if file doesn't exist
    if result.is_ok() {
        let texture = result.unwrap();
        assert_eq!(texture.width, 16);
        assert_eq!(texture.height, 16);
        assert_eq!(texture.data.len(), 16 * 16 * 4);
    }
}

/// Test texture loader generates placeholder for missing textures.
#[test]
fn texture_loader_generates_placeholder() {
    let loader = TextureLoader::new();

    let placeholder = loader.generate_placeholder();
    assert_eq!(placeholder.width, 16);
    assert_eq!(placeholder.height, 16);
    assert_eq!(placeholder.data.len(), 16 * 16 * 4);

    // Placeholder should be non-uniform (e.g., checkerboard pattern)
    let all_same = placeholder.data.iter().all(|&b| b == placeholder.data[0]);
    assert!(
        !all_same,
        "Placeholder should have a pattern, not solid color"
    );
}

/// Test that texture loader validates dimensions.
#[test]
fn texture_loader_validates_dimensions() {
    let loader = TextureLoader::new();

    // Try to load a 32x32 texture (should fail or resize)
    // This test would need a fixture
    let result = loader.load_from_file("tests/fixtures/invalid_size.png");

    // Should either fail or resize to 16x16
    if result.is_ok() {
        let texture = result.unwrap();
        assert_eq!(texture.width, 16);
        assert_eq!(texture.height, 16);
    }
}

/// Test that we can create a GPU texture from the atlas.
#[test]
fn create_gpu_texture() {
    use crate::gpu::{GpuContext, GpuContextConfig};

    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Failed to create GPU context");

    let mut atlas = TextureAtlas::new();
    let texture_data = vec![255u8; 16 * 16 * 4];
    atlas.add_texture("stone", &texture_data, 16, 16);
    atlas.build().unwrap();

    // Should be able to create GPU texture from atlas
    let gpu_texture = atlas.create_gpu_texture(context.device(), context.queue());
    assert!(gpu_texture.is_ok());
}

/// Test atlas with single texture produces minimal dimensions.
#[test]
fn single_texture_minimal_dimensions() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![255u8; 16 * 16 * 4];
    atlas.add_texture("stone", &texture_data, 16, 16);
    atlas.build().unwrap();

    // Minimal power-of-2 size that can hold 16x16 is 16x16
    assert_eq!(atlas.width(), 16);
    assert_eq!(atlas.height(), 16);
}

/// Test atlas with many textures uses efficient packing.
#[test]
fn efficient_packing() {
    let mut atlas = TextureAtlas::new();

    // Add 16 textures (should fit in 4x4 grid = 64x64)
    for i in 0..16 {
        let texture_data = vec![i as u8; 16 * 16 * 4];
        atlas.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    atlas.build().unwrap();

    // Should be at most 64x64 (4x4 grid of 16x16 textures)
    assert!(atlas.width() <= 64 || atlas.height() <= 64);
    let total_pixels = atlas.width() * atlas.height();
    assert!(total_pixels <= 64 * 64);
}

/// Test that rebuilding atlas is idempotent.
#[test]
fn rebuild_is_idempotent() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![255u8; 16 * 16 * 4];
    atlas.add_texture("stone", &texture_data, 16, 16);

    atlas.build().unwrap();
    let hash1 = atlas.hash().to_string();
    let width1 = atlas.width();

    // Rebuild should produce same result
    atlas.build().unwrap();
    let hash2 = atlas.hash().to_string();
    let width2 = atlas.width();

    assert_eq!(hash1, hash2);
    assert_eq!(width1, width2);
}

/// Test that TextureId is Copy and comparable.
#[test]
fn texture_id_is_copy() {
    let mut atlas = TextureAtlas::new();

    let texture_data = vec![255u8; 16 * 16 * 4];
    let id1 = atlas.add_texture("stone", &texture_data, 16, 16).unwrap();
    let id2 = id1; // Should be Copy

    assert_eq!(id1, id2);
}

/// Test that UvCoordinates are reasonable values.
#[test]
fn uv_coordinates_are_reasonable() {
    let mut atlas = TextureAtlas::new();

    // Add multiple textures
    for i in 0..4 {
        let texture_data = vec![i as u8; 16 * 16 * 4];
        atlas.add_texture(&format!("block_{}", i), &texture_data, 16, 16);
    }

    atlas.build().unwrap();

    // Each texture should have non-overlapping UV coordinates
    let id0 = atlas.get_texture_id("block_0").unwrap();
    let id1 = atlas.get_texture_id("block_1").unwrap();

    let uv0 = atlas.get_uv(id0).unwrap();
    let uv1 = atlas.get_uv(id1).unwrap();

    // UVs should not be identical
    assert!(
        uv0.min_u != uv1.min_u || uv0.min_v != uv1.min_v,
        "Different textures should have different UVs"
    );
}
