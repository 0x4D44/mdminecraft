//! Tests for GPU buffer creation and resource management.
//!
//! TDD Phase: RED - These tests are written first and expected to fail.

use super::super::{BufferHandle, BufferManager, BufferUsage, GpuContext, GpuContextConfig};

/// Test basic buffer creation for vertex data.
#[test]
fn create_vertex_buffer_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create a small vertex buffer
    let data: Vec<f32> = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let handle = manager.create_buffer(
        "test_vertex_buffer",
        bytemuck::cast_slice(&data),
        BufferUsage::Vertex,
    );

    assert!(handle.is_ok(), "Vertex buffer creation should succeed");
}

/// Test basic buffer creation for index data.
#[test]
fn create_index_buffer_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create a small index buffer
    let indices: Vec<u32> = vec![0, 1, 2, 2, 3, 0];
    let handle = manager.create_buffer(
        "test_index_buffer",
        bytemuck::cast_slice(&indices),
        BufferUsage::Index,
    );

    assert!(handle.is_ok(), "Index buffer creation should succeed");
}

/// Test uniform buffer creation.
#[test]
fn create_uniform_buffer_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create a uniform buffer for view-projection matrix
    let matrix: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let handle = manager.create_buffer(
        "view_proj_matrix",
        bytemuck::cast_slice(&matrix),
        BufferUsage::Uniform,
    );

    assert!(handle.is_ok(), "Uniform buffer creation should succeed");
}

/// Test that empty buffers are rejected.
#[test]
fn empty_buffer_is_rejected() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let empty_data: &[u8] = &[];
    let handle = manager.create_buffer("empty_buffer", empty_data, BufferUsage::Vertex);

    assert!(handle.is_err(), "Empty buffers should be rejected");
}

/// Test buffer with mesh vertex data from our MeshVertex struct.
#[test]
fn create_buffer_with_mesh_vertices() {
    use crate::MeshVertex;

    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let vertices = vec![
        MeshVertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            block_id: 1,
            _padding: 0,
        },
        MeshVertex {
            position: [1.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            block_id: 1,
            _padding: 0,
        },
        MeshVertex {
            position: [1.0, 0.0, 1.0],
            normal: [0.0, 1.0, 0.0],
            block_id: 1,
            _padding: 0,
        },
    ];

    // MeshVertex should be Pod-compatible
    let data = bytemuck::cast_slice(&vertices);
    let handle = manager.create_buffer("mesh_vertices", data, BufferUsage::Vertex);

    assert!(handle.is_ok(), "Should create buffer from MeshVertex data");
}

/// Test buffer update/write operations.
#[test]
fn update_buffer_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create initial buffer
    let initial_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let handle = manager
        .create_buffer(
            "updatable_buffer",
            bytemuck::cast_slice(&initial_data),
            BufferUsage::Uniform,
        )
        .expect("Initial buffer creation");

    // Update with new data
    let new_data: Vec<f32> = vec![5.0, 6.0, 7.0, 8.0];
    let result = manager.update_buffer(&handle, bytemuck::cast_slice(&new_data));

    assert!(result.is_ok(), "Buffer update should succeed");
}

/// Test partial buffer updates.
#[test]
fn partial_buffer_update_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create a buffer
    let data: Vec<u8> = vec![0; 256];
    let handle = manager
        .create_buffer("partial_update_buffer", &data, BufferUsage::Uniform)
        .expect("Buffer creation");

    // Update just part of it
    let partial_data: Vec<u8> = vec![1, 2, 3, 4];
    let result = manager.update_buffer_range(&handle, 64, &partial_data);

    assert!(result.is_ok(), "Partial buffer update should succeed");
}

/// Test that buffer manager tracks allocations.
#[test]
fn buffer_manager_tracks_allocations() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let data1: Vec<f32> = vec![1.0; 100];
    let data2: Vec<u32> = vec![1; 50];

    let _handle1 = manager
        .create_buffer("buf1", bytemuck::cast_slice(&data1), BufferUsage::Vertex)
        .expect("Buffer 1");
    let _handle2 = manager
        .create_buffer("buf2", bytemuck::cast_slice(&data2), BufferUsage::Index)
        .expect("Buffer 2");

    let stats = manager.statistics();
    assert_eq!(stats.buffer_count, 2);
    assert!(stats.total_bytes >= (data1.len() * 4 + data2.len() * 4) as u64);
}

/// Test that buffers can be destroyed.
#[test]
fn buffer_destruction_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let data: Vec<f32> = vec![1.0; 100];
    let handle = manager
        .create_buffer(
            "temp_buffer",
            bytemuck::cast_slice(&data),
            BufferUsage::Vertex,
        )
        .expect("Buffer creation");

    let initial_count = manager.statistics().buffer_count;

    let result = manager.destroy_buffer(handle);
    assert!(result.is_ok(), "Buffer destruction should succeed");

    let final_count = manager.statistics().buffer_count;
    assert_eq!(final_count, initial_count - 1);
}

/// Test that destroyed buffer handles cannot be used.
#[test]
fn destroyed_buffer_handle_is_invalid() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let data: Vec<f32> = vec![1.0; 10];
    let handle = manager
        .create_buffer(
            "temp_buffer",
            bytemuck::cast_slice(&data),
            BufferUsage::Vertex,
        )
        .expect("Buffer creation");

    manager.destroy_buffer(handle.clone()).expect("Destruction");

    // Attempting to use destroyed handle should fail
    let update_result = manager.update_buffer(&handle, bytemuck::cast_slice(&data));
    assert!(update_result.is_err(), "Destroyed handle should be invalid");
}

/// Test large buffer allocation (chunk mesh sized).
#[test]
fn large_buffer_allocation_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Simulate a large chunk mesh (10k vertices)
    let vertex_count = 10_000;
    let vertex_size = std::mem::size_of::<crate::MeshVertex>();
    let data = vec![0u8; vertex_count * vertex_size];

    let handle = manager.create_buffer("large_chunk_mesh", &data, BufferUsage::Vertex);

    assert!(handle.is_ok(), "Large buffer allocation should succeed");
}

/// Test very large buffer allocation (stress test).
#[test]
fn huge_buffer_allocation_is_handled() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Try to allocate 1GB - might fail on some hardware
    let size = 1024 * 1024 * 1024;
    let data = vec![0u8; size];

    let handle = manager.create_buffer("huge_buffer", &data, BufferUsage::Vertex);

    // Either succeeds or fails gracefully - both are OK
    if handle.is_err() {
        println!("Huge buffer allocation failed (expected on some hardware)");
    }
}

/// Test buffer alignment requirements are met.
#[test]
fn uniform_buffers_are_properly_aligned() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create uniform with awkward size (should be aligned to 256 bytes)
    let data: Vec<u8> = vec![0; 100];
    let handle = manager
        .create_buffer("uniform_align_test", &data, BufferUsage::Uniform)
        .expect("Buffer creation");

    let info = manager.buffer_info(&handle).expect("Get buffer info");

    // Uniform buffers must be aligned to device limits
    let min_alignment = context
        .device()
        .limits()
        .min_uniform_buffer_offset_alignment as u64;
    assert_eq!(
        info.size % min_alignment,
        0,
        "Uniform buffer must be aligned"
    );
}

/// Test buffer naming for debugging.
#[test]
fn buffer_names_are_preserved() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let data: Vec<f32> = vec![1.0; 10];
    let handle = manager
        .create_buffer(
            "my_named_buffer",
            bytemuck::cast_slice(&data),
            BufferUsage::Vertex,
        )
        .expect("Buffer creation");

    let info = manager.buffer_info(&handle).expect("Get buffer info");
    assert_eq!(info.label, "my_named_buffer");
}

/// Test that buffer manager can be reset/cleared.
#[test]
fn buffer_manager_can_be_cleared() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    // Create several buffers
    for i in 0..5 {
        let data: Vec<f32> = vec![i as f32; 10];
        let _handle = manager
            .create_buffer(
                &format!("buffer_{}", i),
                bytemuck::cast_slice(&data),
                BufferUsage::Vertex,
            )
            .expect("Buffer creation");
    }

    assert_eq!(manager.statistics().buffer_count, 5);

    // Clear all buffers
    manager.clear();

    assert_eq!(manager.statistics().buffer_count, 0);
    assert_eq!(manager.statistics().total_bytes, 0);
}

/// Test creating staging buffer for CPU-to-GPU transfers.
#[test]
fn staging_buffer_creation_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let handle = manager.create_buffer("staging_buffer", &data, BufferUsage::Staging);

    assert!(handle.is_ok(), "Staging buffer creation should succeed");
}

/// Test buffer usage flags are enforced.
#[test]
fn buffer_usage_is_validated() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let mut manager = BufferManager::new(context.device());

    let data: Vec<f32> = vec![1.0; 10];

    // Vertex buffer should succeed
    let vertex_handle = manager.create_buffer(
        "vertex_test",
        bytemuck::cast_slice(&data),
        BufferUsage::Vertex,
    );
    assert!(vertex_handle.is_ok());

    // Index buffer should succeed
    let index_handle = manager.create_buffer(
        "index_test",
        bytemuck::cast_slice(&data),
        BufferUsage::Index,
    );
    assert!(index_handle.is_ok());

    // Uniform buffer should succeed
    let uniform_handle = manager.create_buffer(
        "uniform_test",
        bytemuck::cast_slice(&data),
        BufferUsage::Uniform,
    );
    assert!(uniform_handle.is_ok());
}
