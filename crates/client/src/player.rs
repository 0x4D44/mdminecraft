//! Player controller for physics and movement.
//!
//! Integrates InputState with CharacterController from the physics crate
//! to provide player movement with WASD controls, jumping, and collision.

use crate::camera::CameraController;
use crate::input::{InputEvent, InputState};
use mdminecraft_physics::CharacterController;
use mdminecraft_world::ChunkStorage;

/// Walk speed in blocks per second.
const WALK_SPEED: f32 = 4.3;
/// Sprint speed multiplier.
const SPRINT_MULTIPLIER: f32 = 1.3;
/// Jump velocity in blocks per second.
const JUMP_VELOCITY: f32 = 8.0;

/// Player controller integrating input and physics.
#[derive(Debug, Clone)]
pub struct PlayerController {
    /// Character physics controller.
    character: CharacterController,
    /// Current input state.
    input_state: InputState,
}

impl PlayerController {
    /// Create a new player controller.
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            character: CharacterController::new(position),
            input_state: InputState::new(),
        }
    }

    /// Get player position.
    pub fn position(&self) -> [f32; 3] {
        self.character.position
    }

    /// Get player velocity.
    pub fn velocity(&self) -> [f32; 3] {
        self.character.velocity
    }

    /// Check if player is on ground.
    pub fn is_grounded(&self) -> bool {
        self.character.grounded
    }

    /// Process an input event.
    pub fn process_input(&mut self, event: InputEvent) {
        self.input_state.apply_event(event);
    }

    /// Update player physics for one frame.
    ///
    /// # Arguments
    /// * `storage` - Chunk storage for collision detection
    /// * `camera` - Camera to synchronize with player position
    /// * `delta_time` - Time since last update (unused, uses fixed timestep internally)
    pub fn update(
        &mut self,
        storage: &ChunkStorage,
        camera: &mut CameraController,
        _delta_time: f32,
    ) {
        // Get movement direction from input in camera space
        let (forward_input, right_input) = self.input_state.movement_direction();

        // Convert camera-relative input to world-space movement
        let camera_forward = camera.forward();
        let camera_right = camera.right();

        // Project onto horizontal plane
        let forward_xz = [camera_forward[0], camera_forward[2]];
        let right_xz = [camera_right[0], camera_right[2]];

        // Normalize projected vectors
        let forward_len = (forward_xz[0] * forward_xz[0] + forward_xz[1] * forward_xz[1]).sqrt();
        let right_len = (right_xz[0] * right_xz[0] + right_xz[1] * right_xz[1]).sqrt();

        let forward_norm = if forward_len > 0.0 {
            [forward_xz[0] / forward_len, forward_xz[1] / forward_len]
        } else {
            [0.0, 0.0]
        };

        let right_norm = if right_len > 0.0 {
            [right_xz[0] / right_len, right_xz[1] / right_len]
        } else {
            [0.0, 0.0]
        };

        // Combine input directions
        let move_x = forward_norm[0] * forward_input + right_norm[0] * right_input;
        let move_z = forward_norm[1] * forward_input + right_norm[1] * right_input;

        // Normalize combined direction
        let move_len = (move_x * move_x + move_z * move_z).sqrt();
        let direction = if move_len > 0.0 {
            [move_x / move_len, move_z / move_len]
        } else {
            [0.0, 0.0]
        };

        // Calculate speed
        let mut speed = WALK_SPEED;
        if self.input_state.sprint {
            speed *= SPRINT_MULTIPLIER;
        }

        // Apply jump
        if self.input_state.jump && self.character.grounded {
            self.character.velocity[1] = JUMP_VELOCITY;
        }

        // Update physics
        self.character.step(storage, direction, speed);

        // Synchronize camera position with player
        camera.position = self.character.position;
        camera.position[1] += 1.62; // Eye height
    }
}

impl Default for PlayerController {
    fn default() -> Self {
        Self::new([0.0, 20.0, 0.0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_world::{ChunkPos, Voxel, CHUNK_SIZE_X, CHUNK_SIZE_Z};

    fn create_test_floor(storage: &mut ChunkStorage) {
        let chunk_pos = ChunkPos::new(0, 0);
        let chunk = storage.ensure_chunk(chunk_pos);

        // Create a floor at y=0
        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                let voxel = Voxel {
                    id: 1,
                    state: 0,
                    light_sky: 15,
                    light_block: 0,
                };
                chunk.set_voxel(x, 0, z, voxel);
            }
        }
    }

    #[test]
    fn player_controller_creation() {
        let player = PlayerController::new([5.0, 10.0, 15.0]);
        assert_eq!(player.position(), [5.0, 10.0, 15.0]);
        assert_eq!(player.velocity(), [0.0, 0.0, 0.0]);
        assert!(!player.is_grounded());
    }

    #[test]
    fn player_controller_default() {
        let player = PlayerController::default();
        assert_eq!(player.position(), [0.0, 20.0, 0.0]);
    }

    #[test]
    fn player_controller_process_input() {
        let mut player = PlayerController::default();
        player.process_input(InputEvent::MoveForward(true));

        // Input state should be updated
        assert!(player.input_state.forward);
    }

    #[test]
    fn player_controller_update_integrates_physics() {
        let mut storage = ChunkStorage::new(10);
        create_test_floor(&mut storage);

        let mut player = PlayerController::new([5.0, 2.0, 5.0]);
        let mut camera = CameraController::default();

        // Apply some forward input
        player.process_input(InputEvent::MoveForward(true));

        let initial_pos = player.position();

        // Update for a few frames
        for _ in 0..10 {
            player.update(&storage, &mut camera, 0.016);
        }

        // Player should have moved forward (in +Z direction initially)
        let final_pos = player.position();
        let distance_moved = ((final_pos[0] - initial_pos[0]).powi(2)
            + (final_pos[2] - initial_pos[2]).powi(2))
        .sqrt();

        assert!(distance_moved > 0.1, "Player should have moved");
    }

    #[test]
    fn player_controller_camera_follows_player() {
        let mut storage = ChunkStorage::new(10);
        create_test_floor(&mut storage);

        let mut player = PlayerController::new([5.0, 2.0, 5.0]);
        let mut camera = CameraController::new([0.0, 0.0, 0.0], 0.0, 0.0, 0.002);

        player.update(&storage, &mut camera, 0.016);

        // Camera should be at player position + eye height
        assert_eq!(camera.position[0], player.position()[0]);
        assert_eq!(camera.position[1], player.position()[1] + 1.62);
        assert_eq!(camera.position[2], player.position()[2]);
    }

    #[test]
    fn player_controller_jump_when_grounded() {
        let mut storage = ChunkStorage::new(10);
        create_test_floor(&mut storage);

        let mut player = PlayerController::new([5.0, 1.0, 5.0]);
        let mut camera = CameraController::default();

        // Let player settle on ground
        for _ in 0..10 {
            player.update(&storage, &mut camera, 0.016);
        }

        assert!(player.is_grounded());

        // Apply jump
        player.process_input(InputEvent::Jump(true));
        player.update(&storage, &mut camera, 0.016);

        // Player should have upward velocity
        assert!(player.velocity()[1] > 0.0);
    }

    #[test]
    fn player_controller_sprint_increases_speed() {
        let mut storage = ChunkStorage::new(10);
        create_test_floor(&mut storage);

        // Test walking
        let mut player_walk = PlayerController::new([5.0, 1.0, 5.0]);
        let mut camera_walk = CameraController::default();
        player_walk.process_input(InputEvent::MoveForward(true));

        let walk_start = player_walk.position();
        for _ in 0..20 {
            player_walk.update(&storage, &mut camera_walk, 0.016);
        }
        let walk_distance = ((player_walk.position()[0] - walk_start[0]).powi(2)
            + (player_walk.position()[2] - walk_start[2]).powi(2))
        .sqrt();

        // Test sprinting
        let mut player_sprint = PlayerController::new([5.0, 1.0, 5.0]);
        let mut camera_sprint = CameraController::default();
        player_sprint.process_input(InputEvent::MoveForward(true));
        player_sprint.process_input(InputEvent::Sprint(true));

        let sprint_start = player_sprint.position();
        for _ in 0..20 {
            player_sprint.update(&storage, &mut camera_sprint, 0.016);
        }
        let sprint_distance = ((player_sprint.position()[0] - sprint_start[0]).powi(2)
            + (player_sprint.position()[2] - sprint_start[2]).powi(2))
        .sqrt();

        // Sprint should be faster than walk
        assert!(sprint_distance > walk_distance * 1.2);
    }
}
