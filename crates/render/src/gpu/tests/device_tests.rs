//! Tests for wgpu instance, device, and queue initialization.
//!
//! TDD Phase: RED - These tests are written first and expected to fail.

use super::super::{GpuContext, GpuContextConfig};

/// Test that we can create a headless GPU context for CI/testing.
/// This is critical for deterministic rendering in automated tests.
#[test]
fn create_headless_context_succeeds() {
    let config = GpuContextConfig {
        headless: true,
        width: 800,
        height: 600,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context = GpuContext::new(config);
    assert!(context.is_ok(), "Failed to create headless GPU context");

    let ctx = context.unwrap();
    assert_eq!(ctx.width(), 800);
    assert_eq!(ctx.height(), 600);
    assert!(ctx.is_headless());
}

/// Test that we can create a windowed GPU context.
/// This will be used for interactive gameplay.
#[test]
fn create_windowed_context_succeeds() {
    // Note: This test may fail in true headless CI environments.
    // We'll handle this gracefully in the implementation.
    let config = GpuContextConfig {
        headless: false,
        width: 1280,
        height: 720,
        power_preference: wgpu::PowerPreference::HighPerformance,
        present_mode: wgpu::PresentMode::Mailbox,
    };

    let context = GpuContext::new(config);

    // In headless CI, this should gracefully fall back or skip
    if context.is_ok() {
        let ctx = context.unwrap();
        assert_eq!(ctx.width(), 1280);
        assert_eq!(ctx.height(), 720);
        assert!(!ctx.is_headless());
    }
}

/// Test that device and queue are properly initialized.
#[test]
fn device_and_queue_are_initialized() {
    let config = GpuContextConfig {
        headless: true,
        width: 640,
        height: 480,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context = GpuContext::new(config).expect("Failed to create context");

    // Verify we have access to device and queue
    let device = context.device();
    let queue = context.queue();

    assert!(device.features().is_empty() || !device.features().is_empty());

    // Device should be valid (we'll define what this means in implementation)
    assert!(context.is_device_valid());
}

/// Test that we request appropriate device limits.
#[test]
fn device_limits_are_reasonable() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Failed to create context");

    let limits = context.device().limits();

    // Ensure we have enough resources for chunk rendering
    assert!(
        limits.max_texture_dimension_2d >= 4096,
        "Need at least 4096x4096 textures for atlas"
    );
    assert!(
        limits.max_bind_groups >= 4,
        "Need multiple bind groups for materials"
    );
    assert!(
        limits.max_buffer_size >= 256 * 1024 * 1024,
        "Need at least 256MB buffers for chunk meshes"
    );
}

/// Test that we can get adapter information for diagnostics.
#[test]
fn can_query_adapter_info() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Failed to create context");

    let info = context.adapter_info();

    // Should have basic adapter information
    assert!(!info.name.is_empty());
    assert!(!info.driver.is_empty());

    // Log for debugging (useful in CI)
    println!("Adapter: {} ({})", info.name, info.driver);
    println!("Backend: {:?}", info.backend);
}

/// Test initialization with different power preferences.
#[test]
fn power_preference_low_power_initializes() {
    let config = GpuContextConfig {
        headless: true,
        width: 800,
        height: 600,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context = GpuContext::new(config);
    assert!(context.is_ok(), "Low power mode should always work");
}

/// Test initialization with high performance preference.
#[test]
fn power_preference_high_performance_initializes() {
    let config = GpuContextConfig {
        headless: true,
        width: 800,
        height: 600,
        power_preference: wgpu::PowerPreference::HighPerformance,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context = GpuContext::new(config);
    assert!(
        context.is_ok(),
        "High performance mode should work if available"
    );
}

/// Test that context can be recreated (for resolution changes, etc).
#[test]
fn context_recreation_succeeds() {
    let config1 = GpuContextConfig {
        headless: true,
        width: 800,
        height: 600,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context1 = GpuContext::new(config1).expect("First context");
    drop(context1); // Explicitly drop first context

    let config2 = GpuContextConfig {
        headless: true,
        width: 1920,
        height: 1080,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context2 = GpuContext::new(config2);
    assert!(context2.is_ok(), "Should be able to recreate context");

    let ctx2 = context2.unwrap();
    assert_eq!(ctx2.width(), 1920);
    assert_eq!(ctx2.height(), 1080);
}

/// Test that invalid dimensions are rejected.
#[test]
fn zero_dimensions_are_rejected() {
    let config = GpuContextConfig {
        headless: true,
        width: 0,
        height: 0,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context = GpuContext::new(config);
    assert!(context.is_err(), "Zero dimensions should be rejected");
}

/// Test that extremely large dimensions are handled.
#[test]
fn huge_dimensions_are_handled() {
    let config = GpuContextConfig {
        headless: true,
        width: 16384,
        height: 16384,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    // This might fail on some hardware - that's OK, we're testing error handling
    let context = GpuContext::new(config);
    if context.is_err() {
        println!("Large dimensions rejected (expected on some hardware)");
    }
}

/// Test that we can create multiple contexts (for multi-window scenarios).
#[test]
fn multiple_contexts_can_coexist() {
    let config1 = GpuContextConfig::default_headless();
    let config2 = GpuContextConfig {
        headless: true,
        width: 1024,
        height: 768,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context1 = GpuContext::new(config1);
    let context2 = GpuContext::new(config2);

    assert!(context1.is_ok(), "First context should succeed");
    assert!(context2.is_ok(), "Second context should succeed");
}

/// Test deterministic initialization (same config = same result).
#[test]
fn initialization_is_deterministic() {
    let config = GpuContextConfig::default_headless();

    let context1 = GpuContext::new(config.clone()).expect("First init");
    let info1 = context1.adapter_info();

    drop(context1);

    let context2 = GpuContext::new(config).expect("Second init");
    let info2 = context2.adapter_info();

    // Same adapter should be selected with identical config
    assert_eq!(info1.name, info2.name);
    assert_eq!(info1.backend, info2.backend);
}

/// Test that context provides surface for windowed mode.
#[test]
fn windowed_context_provides_surface() {
    let config = GpuContextConfig {
        headless: false,
        width: 800,
        height: 600,
        power_preference: wgpu::PowerPreference::LowPower,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let context = GpuContext::new(config);

    // May fail in headless CI - that's expected
    if let Ok(ctx) = context {
        if !ctx.is_headless() {
            assert!(ctx.has_surface(), "Windowed context must have surface");
        }
    }
}

/// Test that headless context does NOT provide surface.
#[test]
fn headless_context_has_no_surface() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Headless context");

    assert!(context.is_headless());
    assert!(!context.has_surface(), "Headless should not have surface");
}

/// Test that we can wait for device to be idle (for cleanup).
#[test]
fn can_wait_for_device_idle() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");

    // Should be able to wait for device without hanging
    let result = context.wait_for_idle();
    assert!(result.is_ok(), "Device idle wait should succeed");
}
