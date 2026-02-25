//! Demonstrates the camera controller and input system.
//!
//! This example shows how to:
//! - Create a camera controller
//! - Process input events
//! - Serialize events to JSONL for replay
//! - Track input state

use mdminecraft_client::{
    camera::CameraController,
    config::InputConfig,
    input::{InputEvent, InputState, KeyCode},
};

fn main() -> anyhow::Result<()> {
    println!("=== Camera Controller and Input System Demo ===\n");

    // 1. Create camera controller
    let mut camera = CameraController::default();
    println!("Initial camera position: {:?}", camera.position);
    println!(
        "Initial yaw: {:.2}, pitch: {:.2}\n",
        camera.yaw, camera.pitch
    );

    // 2. Simulate mouse movement
    println!("Simulating mouse look:");
    camera.process_mouse(100.0, -50.0);
    println!("  After mouse delta (100, -50):");
    println!("    Yaw: {:.4}, Pitch: {:.4}", camera.yaw, camera.pitch);

    let forward = camera.forward();
    println!(
        "    Forward vector: [{:.3}, {:.3}, {:.3}]",
        forward[0], forward[1], forward[2]
    );

    let view_camera = camera.get_view_matrix();
    println!("    View target: {:?}\n", view_camera.target);

    // 3. Create input config and state
    let config = InputConfig::default();
    let mut input_state = InputState::new();

    println!("Default key bindings:");
    for (key, action) in &config.key_bindings {
        println!("  {} -> {}", key, action);
    }
    println!();

    // 4. Simulate keyboard input
    println!("Simulating keyboard input:");
    let events = vec![
        (KeyCode::W, true, "Press W (forward)"),
        (KeyCode::D, true, "Press D (strafe right)"),
        (KeyCode::Space, true, "Press Space (jump)"),
        (KeyCode::ShiftLeft, true, "Press Shift (crouch)"),
    ];

    let mut event_log = Vec::new();

    for (key, pressed, description) in events {
        if let Some(event) = config.key_to_event(key, pressed) {
            println!("  {}: {:?}", description, event);
            input_state.apply_event(event);
            event_log.push(event);
        }
    }
    println!();

    // 5. Check movement direction
    let (forward_movement, right_movement) = input_state.movement_direction();
    println!("Current movement direction:");
    println!("  Forward: {:.3}", forward_movement);
    println!("  Right: {:.3}", right_movement);
    println!("  Jump: {}", input_state.jump);
    println!("  Crouch: {}\n", input_state.crouch);

    // 6. Serialize events to JSONL
    println!("Serialized events (JSONL format for replay):");
    for event in &event_log {
        let json = serde_json::to_string(event)?;
        println!("  {}", json);
    }
    println!();

    // 7. Add a look delta event
    let look_event = InputEvent::LookDelta {
        yaw: 0.1,
        pitch: -0.05,
    };
    event_log.push(look_event);
    println!("Look delta event: {}", serde_json::to_string(&look_event)?);
    println!();

    // 8. Test gimbal lock prevention
    println!("Testing gimbal lock prevention:");
    let mut test_camera = CameraController::default();
    test_camera.process_mouse(0.0, -100000.0); // Try to look straight up
    println!("  After extreme upward look:");
    println!(
        "    Pitch clamped to: {:.4} radians (~{:.1} degrees)",
        test_camera.pitch,
        test_camera.pitch.to_degrees()
    );
    println!();

    // 9. Load custom config from TOML
    let toml_config = r#"
        mouse_sensitivity = 0.005
        [key_bindings]
        W = "jump"
        S = "crouch"
    "#;

    let custom_config = InputConfig::from_toml(toml_config)?;
    println!("Custom config loaded:");
    println!("  Mouse sensitivity: {}", custom_config.mouse_sensitivity);

    if let Some(event) = custom_config.key_to_event(KeyCode::W, true) {
        println!("  W now maps to: {:?}", event);
    }

    println!("\n=== Demo Complete ===");

    Ok(())
}
