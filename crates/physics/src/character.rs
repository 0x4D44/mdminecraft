//! Character controller with physics integration.

use crate::collision::{resolve_capsule_aabb, Capsule};
use crate::constants::{
    FIXED_TIMESTEP, FRICTION, GRAVITY, MAX_SPEED, PLAYER_HEIGHT, PLAYER_RADIUS, STEP_HEIGHT,
};
use crate::Aabb;
use mdminecraft_world::{ChunkStorage, BLOCK_AIR, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z};

/// Character controller managing player physics state.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterController {
    /// Current position (base of capsule, at player's feet).
    pub position: [f32; 3],
    /// Current velocity in m/s (blocks/second).
    pub velocity: [f32; 3],
    /// Whether the character is standing on solid ground.
    pub grounded: bool,
    /// Whether the character was grounded in the previous frame (for step detection).
    pub was_grounded: bool,
}

impl CharacterController {
    /// Create a new character controller at the given position.
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            position,
            velocity: [0.0, 0.0, 0.0],
            grounded: false,
            was_grounded: false,
        }
    }

    /// Apply gravity acceleration to the character's velocity.
    pub fn apply_gravity(&mut self, dt: f32) {
        if !self.grounded {
            self.velocity[1] -= GRAVITY * dt;
        }
    }

    /// Apply movement input to the character's horizontal velocity.
    /// Input direction is expected to be normalized or zero.
    pub fn apply_movement(&mut self, direction: [f32; 2], speed: f32) {
        // Apply input to horizontal velocity
        self.velocity[0] = direction[0] * speed;
        self.velocity[2] = direction[1] * speed;

        // Clamp to max speed
        let horizontal_speed =
            (self.velocity[0] * self.velocity[0] + self.velocity[2] * self.velocity[2]).sqrt();
        if horizontal_speed > MAX_SPEED {
            let scale = MAX_SPEED / horizontal_speed;
            self.velocity[0] *= scale;
            self.velocity[2] *= scale;
        }
    }

    /// Apply friction to horizontal velocity when grounded.
    pub fn apply_friction(&mut self) {
        if self.grounded {
            self.velocity[0] *= FRICTION;
            self.velocity[2] *= FRICTION;

            // Zero out very small velocities to prevent micro-drift
            const MIN_VELOCITY: f32 = 0.001;
            if self.velocity[0].abs() < MIN_VELOCITY {
                self.velocity[0] = 0.0;
            }
            if self.velocity[2].abs() < MIN_VELOCITY {
                self.velocity[2] = 0.0;
            }
        }
    }

    /// Integrate velocity into position using semi-implicit Euler (velocity-Verlet).
    /// Updates velocity first, then position.
    /// Does NOT apply collision resolution - call resolve_collisions separately.
    pub fn integrate(&mut self, dt: f32) {
        // Semi-implicit Euler: v(t+dt) is already updated by apply_gravity and apply_movement
        // Now update position: p(t+dt) = p(t) + v(t+dt) * dt
        self.position[0] += self.velocity[0] * dt;
        self.position[1] += self.velocity[1] * dt;
        self.position[2] += self.velocity[2] * dt;
    }

    /// Resolve collisions with the voxel world.
    /// This should be called after integrate() to handle collision response.
    pub fn resolve_collisions(&mut self, storage: &ChunkStorage) {
        self.was_grounded = self.grounded;
        self.grounded = false;

        // Create capsule collider for the character
        let mut capsule = Capsule::new(self.position, PLAYER_HEIGHT, PLAYER_RADIUS);

        // Get nearby solid block AABBs
        let nearby_blocks = get_nearby_solid_blocks(storage, &capsule);

        // Resolve collisions with each block
        for block_aabb in nearby_blocks {
            let result = resolve_capsule_aabb(&capsule, self.velocity, &block_aabb);

            // Update position and velocity from collision result
            self.position = result.position;
            self.velocity = result.velocity;

            if result.grounded {
                self.grounded = true;
            }

            // Update capsule for next iteration
            capsule = Capsule::new(self.position, PLAYER_HEIGHT, PLAYER_RADIUS);

            // Handle step-up if we were grounded and hit a wall
            if self.was_grounded && !self.grounded {
                if let Some(stepped_pos) = try_step_up(storage, &capsule, &block_aabb) {
                    self.position = stepped_pos;
                    self.grounded = true;
                }
            }
        }
    }

    /// Perform a full physics step with the given delta time.
    /// This combines all physics operations in the correct order.
    pub fn step(&mut self, storage: &ChunkStorage, input_direction: [f32; 2], move_speed: f32) {
        let dt = FIXED_TIMESTEP;

        // Apply forces
        self.apply_gravity(dt);
        self.apply_movement(input_direction, move_speed);
        self.apply_friction();

        // Integrate velocity into position
        self.integrate(dt);

        // Resolve collisions
        self.resolve_collisions(storage);
    }
}

/// Get all solid block AABBs near the given capsule.
fn get_nearby_solid_blocks(storage: &ChunkStorage, capsule: &Capsule) -> Vec<Aabb> {
    let mut blocks = Vec::new();
    let aabb = capsule.to_aabb();

    // Expand slightly to catch edge cases
    let expanded = aabb.expand(0.1);

    // Get integer bounds
    let min_x = expanded.min[0].floor() as i32;
    let min_y = expanded.min[1].floor() as i32;
    let min_z = expanded.min[2].floor() as i32;
    let max_x = expanded.max[0].ceil() as i32;
    let max_y = expanded.max[1].ceil() as i32;
    let max_z = expanded.max[2].ceil() as i32;

    // Iterate through all blocks in the expanded AABB
    for y in min_y..=max_y {
        if y < 0 || y >= CHUNK_SIZE_Y as i32 {
            continue;
        }

        for x in min_x..=max_x {
            for z in min_z..=max_z {
                if let Some(block_id) = get_block_at(storage, x, y, z) {
                    if block_id != BLOCK_AIR {
                        blocks.push(Aabb::from_voxel(x, y, z));
                    }
                }
            }
        }
    }

    blocks
}

/// Try to step up onto a block if the step height is reasonable.
/// Returns Some(new_position) if stepping succeeded, None otherwise.
fn try_step_up(storage: &ChunkStorage, capsule: &Capsule, _obstacle: &Aabb) -> Option<[f32; 3]> {
    // Try stepping up in increments
    const STEP_INCREMENTS: usize = 3;
    let step_increment = STEP_HEIGHT / STEP_INCREMENTS as f32;

    for i in 1..=STEP_INCREMENTS {
        let step_up = step_increment * i as f32;
        let test_pos = [capsule.base[0], capsule.base[1] + step_up, capsule.base[2]];

        let test_capsule = Capsule::new(test_pos, capsule.height, capsule.radius);
        let nearby = get_nearby_solid_blocks(storage, &test_capsule);

        // Check if this position is valid (no collisions)
        let test_aabb = test_capsule.to_aabb();
        let mut valid = true;
        for block_aabb in nearby {
            if test_aabb.intersects(&block_aabb) {
                valid = false;
                break;
            }
        }

        if valid {
            return Some(test_pos);
        }
    }

    None
}

/// Helper function to get a block at world coordinates from ChunkStorage.
fn get_block_at(storage: &ChunkStorage, x: i32, y: i32, z: i32) -> Option<u16> {
    if y < 0 || y >= CHUNK_SIZE_Y as i32 {
        return None;
    }

    let chunk_x = x.div_euclid(CHUNK_SIZE_X as i32);
    let chunk_z = z.div_euclid(CHUNK_SIZE_Z as i32);
    let local_x = x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let local_y = y as usize;
    let local_z = z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

    let chunk_pos = mdminecraft_world::ChunkPos::new(chunk_x, chunk_z);
    let chunk = storage.get(chunk_pos)?;

    Some(chunk.voxel(local_x, local_y, local_z).id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_world::{ChunkPos, Voxel};

    fn create_test_storage_with_floor() -> ChunkStorage {
        let mut storage = ChunkStorage::new(10);
        let chunk_pos = ChunkPos::new(0, 0);
        let chunk = storage.ensure_chunk(chunk_pos);

        // Create a floor at y=0
        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                let voxel = Voxel {
                    id: 1,
                    state: 0,
                    light_sky: 0,
                    light_block: 0,
                };
                chunk.set_voxel(x, 0, z, voxel);
            }
        }

        storage
    }

    #[test]
    fn character_controller_starts_at_position() {
        let pos = [5.0, 10.0, 5.0];
        let controller = CharacterController::new(pos);

        assert_eq!(controller.position, pos);
        assert_eq!(controller.velocity, [0.0, 0.0, 0.0]);
        assert!(!controller.grounded);
    }

    #[test]
    fn apply_gravity_accelerates_downward() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);
        controller.grounded = false;

        let initial_velocity = controller.velocity[1];
        controller.apply_gravity(FIXED_TIMESTEP);

        assert!(controller.velocity[1] < initial_velocity);
        assert!(
            (controller.velocity[1] - (initial_velocity - GRAVITY * FIXED_TIMESTEP)).abs() < 0.001
        );
    }

    #[test]
    fn apply_gravity_does_nothing_when_grounded() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);
        controller.grounded = true;

        let initial_velocity = controller.velocity[1];
        controller.apply_gravity(FIXED_TIMESTEP);

        assert_eq!(controller.velocity[1], initial_velocity);
    }

    #[test]
    fn apply_movement_sets_horizontal_velocity() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);

        controller.apply_movement([1.0, 0.0], 4.0);

        assert!((controller.velocity[0] - 4.0).abs() < 0.001);
        assert!((controller.velocity[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn apply_movement_clamps_to_max_speed() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);

        // Try to move faster than MAX_SPEED
        controller.apply_movement([1.0, 0.0], MAX_SPEED * 2.0);

        let horizontal_speed = (controller.velocity[0] * controller.velocity[0]
            + controller.velocity[2] * controller.velocity[2])
            .sqrt();

        assert!(horizontal_speed <= MAX_SPEED + 0.001);
    }

    #[test]
    fn apply_friction_reduces_velocity_when_grounded() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);
        controller.velocity = [5.0, 0.0, 5.0];
        controller.grounded = true;

        controller.apply_friction();

        assert!(controller.velocity[0] < 5.0);
        assert!(controller.velocity[2] < 5.0);
        assert!((controller.velocity[0] - 5.0 * FRICTION).abs() < 0.001);
        assert!((controller.velocity[2] - 5.0 * FRICTION).abs() < 0.001);
    }

    #[test]
    fn apply_friction_does_nothing_when_airborne() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);
        controller.velocity = [5.0, 0.0, 5.0];
        controller.grounded = false;

        let initial_velocity = controller.velocity;
        controller.apply_friction();

        assert_eq!(controller.velocity, initial_velocity);
    }

    #[test]
    fn apply_friction_zeros_tiny_velocities() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);
        controller.velocity = [0.0001, 0.0, 0.0001];
        controller.grounded = true;

        controller.apply_friction();

        assert_eq!(controller.velocity[0], 0.0);
        assert_eq!(controller.velocity[2], 0.0);
    }

    #[test]
    fn integrate_updates_position_from_velocity() {
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);
        controller.velocity = [2.0, -1.0, 3.0];

        let expected_pos = [
            5.0 + 2.0 * FIXED_TIMESTEP,
            10.0 + -1.0 * FIXED_TIMESTEP,
            5.0 + 3.0 * FIXED_TIMESTEP,
        ];

        controller.integrate(FIXED_TIMESTEP);

        assert!((controller.position[0] - expected_pos[0]).abs() < 0.001);
        assert!((controller.position[1] - expected_pos[1]).abs() < 0.001);
        assert!((controller.position[2] - expected_pos[2]).abs() < 0.001);
    }

    #[test]
    fn resolve_collisions_stops_at_floor() {
        let storage = create_test_storage_with_floor();
        let mut controller = CharacterController::new([5.0, 0.5, 5.0]);
        controller.velocity = [0.0, -1.0, 0.0];

        controller.resolve_collisions(&storage);

        // Should be placed on top of floor (y=1.0)
        assert!(
            (controller.position[1] - 1.0).abs() < 0.1,
            "Position Y is {}, expected 1.0",
            controller.position[1]
        );
        assert_eq!(controller.velocity[1], 0.0);
        assert!(controller.grounded);
    }

    #[test]
    fn character_standing_on_block_is_stable() {
        let storage = create_test_storage_with_floor();
        let mut controller = CharacterController::new([5.0, 1.0, 5.0]);

        // Simulate standing still for multiple frames
        for _ in 0..10 {
            controller.step(&storage, [0.0, 0.0], 0.0);
        }

        // Position should remain stable (within tolerance)
        assert!((controller.position[1] - 1.0).abs() < 0.01);
        assert!(controller.grounded);
    }

    #[test]
    fn character_falls_with_gravity() {
        let storage = ChunkStorage::new(10); // Empty world
        let mut controller = CharacterController::new([5.0, 10.0, 5.0]);

        let initial_y = controller.position[1];

        // Simulate several frames of falling
        for _ in 0..10 {
            controller.step(&storage, [0.0, 0.0], 0.0);
        }

        // Should have fallen
        assert!(controller.position[1] < initial_y);
        assert!(!controller.grounded);
        assert!(controller.velocity[1] < 0.0);
    }

    #[test]
    fn get_nearby_solid_blocks_finds_floor() {
        let storage = create_test_storage_with_floor();
        let capsule = Capsule::new([5.0, 1.0, 5.0], PLAYER_HEIGHT, PLAYER_RADIUS);

        let blocks = get_nearby_solid_blocks(&storage, &capsule);

        // Should find at least the block directly below
        assert!(!blocks.is_empty());
        assert!(blocks.iter().any(|b| b.min[1] == 0.0 && b.max[1] == 1.0));
    }

    #[test]
    fn semi_implicit_euler_is_deterministic() {
        let storage = create_test_storage_with_floor();

        let mut controller1 = CharacterController::new([5.0, 10.0, 5.0]);
        let mut controller2 = CharacterController::new([5.0, 10.0, 5.0]);

        // Run same simulation twice
        for _ in 0..20 {
            controller1.step(&storage, [1.0, 0.0], 4.0);
            controller2.step(&storage, [1.0, 0.0], 4.0);
        }

        // Results should be identical
        assert_eq!(controller1.position, controller2.position);
        assert_eq!(controller1.velocity, controller2.velocity);
        assert_eq!(controller1.grounded, controller2.grounded);
    }

    #[test]
    fn no_tunneling_at_max_speed_with_fixed_timestep() {
        // This test ensures the constants are configured correctly
        let max_displacement = MAX_SPEED * FIXED_TIMESTEP;

        // Maximum displacement should be less than player radius to prevent tunneling
        assert!(
            max_displacement < PLAYER_RADIUS * 2.0,
            "Player can move {} blocks per tick, which could cause tunneling through thin walls",
            max_displacement
        );
    }
}
