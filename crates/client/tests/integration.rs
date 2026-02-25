//! Integration tests for client systems.

use mdminecraft_client::{
    camera::CameraController,
    game_loop::{GameState, PHYSICS_TPS, TARGET_FPS},
    input::{InputEvent, InputState},
    player::PlayerController,
};

#[derive(Default)]
struct TestPhysics {
    update_count: u32,
}

#[derive(Default)]
struct TestRenderer {
    render_count: u32,
}

#[test]
fn camera_controller_initialization() {
    let camera = CameraController::default();
    assert_eq!(camera.position, [0.0, 20.0, 30.0]);
    assert_eq!(camera.yaw, 0.0);
    assert_eq!(camera.pitch, 0.0);
}

#[test]
fn player_controller_initialization() {
    let player = PlayerController::default();
    // Player controller is initialized with default position
    assert_eq!(player.position(), [0.0, 20.0, 0.0]);
    assert_eq!(player.velocity(), [0.0, 0.0, 0.0]);
    assert!(!player.is_grounded());
}

#[test]
fn input_state_movement() {
    let mut input = InputState::new();

    // Apply forward movement
    input.apply_event(InputEvent::MoveForward(true));
    let (forward, right) = input.movement_direction();
    assert_eq!(forward, 1.0);
    assert_eq!(right, 0.0);

    // Apply diagonal movement
    input.apply_event(InputEvent::MoveRight(true));
    let (forward, right) = input.movement_direction();

    // Check diagonal movement is normalized
    let length = (forward * forward + right * right).sqrt();
    assert!((length - 1.0).abs() < 0.001);
}

#[test]
fn game_state_initialization() {
    let physics = TestPhysics::default();
    let renderer = TestRenderer::default();
    let state = GameState::new(physics, renderer);

    assert_eq!(state.frame_count(), 0);
    assert_eq!(state.tick_count(), 0);
}

#[test]
fn game_state_update_cycles() {
    let physics = TestPhysics::default();
    let renderer = TestRenderer::default();
    let mut state = GameState::new(physics, renderer);

    // Simulate one update
    let result = state.update(
        |p, _dt| {
            p.update_count += 1;
            Ok(())
        },
        |r, _p| {
            r.render_count += 1;
            Ok(())
        },
    );

    assert!(result.is_ok());
    assert_eq!(state.frame_count(), 1);
}

#[test]
fn constants_are_correct() {
    assert_eq!(PHYSICS_TPS, 20);
    assert_eq!(TARGET_FPS, 60);
}

#[test]
fn camera_mouse_look() {
    let mut camera = CameraController::default();
    let initial_yaw = camera.yaw;
    let initial_pitch = camera.pitch;

    // Move mouse right and down
    camera.process_mouse(100.0, 100.0);

    // Yaw should increase (looking right)
    assert!(camera.yaw > initial_yaw);

    // Pitch should decrease (looking down, due to inverted Y)
    assert!(camera.pitch < initial_pitch);
}

#[test]
fn camera_direction_vectors() {
    let camera = CameraController::new([0.0, 0.0, 0.0], 0.0, 0.0, 0.002);

    // Forward should be along +Z
    let forward = camera.forward();
    assert!((forward[0] - 0.0).abs() < 0.01);
    assert!((forward[1] - 0.0).abs() < 0.01);
    assert!((forward[2] - 1.0).abs() < 0.01);

    // Right should be along +X
    let right = camera.right();
    assert!((right[0] - 1.0).abs() < 0.01);
    assert_eq!(right[1], 0.0);
    assert!((right[2] - 0.0).abs() < 0.01);
}
