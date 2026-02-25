//! Camera controller for free-look navigation.
//!
//! This module provides a camera controller that maintains yaw/pitch state
//! and generates view matrices for rendering.

use mdminecraft_render::Camera;

/// Camera controller with yaw/pitch state for mouse-look controls.
#[derive(Debug, Clone, Copy)]
pub struct CameraController {
    /// Camera position in world space.
    pub position: [f32; 3],
    /// Yaw angle in radians (rotation around Y axis).
    pub yaw: f32,
    /// Pitch angle in radians (rotation around X axis).
    pub pitch: f32,
    /// Mouse sensitivity multiplier.
    pub sensitivity: f32,
}

impl CameraController {
    /// Create a new camera controller.
    ///
    /// # Arguments
    /// * `position` - Initial camera position
    /// * `yaw` - Initial yaw in radians (0 = looking along +Z)
    /// * `pitch` - Initial pitch in radians (0 = looking horizontally)
    /// * `sensitivity` - Mouse sensitivity (0.002 is a good default)
    pub fn new(position: [f32; 3], yaw: f32, pitch: f32, sensitivity: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            sensitivity,
        }
    }

    /// Process mouse motion to update yaw and pitch.
    ///
    /// # Arguments
    /// * `delta_x` - Mouse X delta (positive = right)
    /// * `delta_y` - Mouse Y delta (positive = down)
    pub fn process_mouse(&mut self, delta_x: f32, delta_y: f32) {
        // Update yaw and pitch with sensitivity
        self.yaw += delta_x * self.sensitivity;
        self.pitch -= delta_y * self.sensitivity;

        // Clamp pitch to prevent gimbal lock (89 degrees = ~1.553 radians)
        const MAX_PITCH: f32 = 1.553;
        self.pitch = self.pitch.clamp(-MAX_PITCH, MAX_PITCH);

        // Normalize yaw to [0, 2π]
        const TAU: f32 = std::f32::consts::PI * 2.0;
        self.yaw = self.yaw.rem_euclid(TAU);
    }

    /// Get forward direction vector from yaw/pitch.
    pub fn forward(&self) -> [f32; 3] {
        let cos_pitch = self.pitch.cos();
        [
            self.yaw.sin() * cos_pitch,
            self.pitch.sin(),
            self.yaw.cos() * cos_pitch,
        ]
    }

    /// Get right direction vector (perpendicular to forward).
    pub fn right(&self) -> [f32; 3] {
        [
            (self.yaw + std::f32::consts::FRAC_PI_2).sin(),
            0.0,
            (self.yaw + std::f32::consts::FRAC_PI_2).cos(),
        ]
    }

    /// Get view matrix for rendering using the Camera struct.
    pub fn get_view_matrix(&self) -> Camera {
        let forward = self.forward();
        let target = [
            self.position[0] + forward[0],
            self.position[1] + forward[1],
            self.position[2] + forward[2],
        ];

        Camera {
            position: self.position,
            target,
            up: [0.0, 1.0, 0.0],
        }
    }
}

impl Default for CameraController {
    fn default() -> Self {
        Self::new([0.0, 20.0, 30.0], 0.0, 0.0, 0.002)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_controller_creation() {
        let camera = CameraController::new([0.0, 10.0, 0.0], 0.0, 0.0, 0.002);
        assert_eq!(camera.position, [0.0, 10.0, 0.0]);
        assert_eq!(camera.yaw, 0.0);
        assert_eq!(camera.pitch, 0.0);
        assert_eq!(camera.sensitivity, 0.002);
    }

    #[test]
    fn camera_controller_default() {
        let camera = CameraController::default();
        assert_eq!(camera.position, [0.0, 20.0, 30.0]);
        assert_eq!(camera.yaw, 0.0);
        assert_eq!(camera.pitch, 0.0);
        assert_eq!(camera.sensitivity, 0.002);
    }

    #[test]
    fn process_mouse_updates_yaw_pitch() {
        let mut camera = CameraController::default();

        // Move right (positive X)
        camera.process_mouse(100.0, 0.0);
        assert!(camera.yaw > 0.0);
        assert_eq!(camera.pitch, 0.0);

        // Reset
        let mut camera = CameraController::default();

        // Move down (positive Y)
        camera.process_mouse(0.0, 100.0);
        assert_eq!(camera.yaw, 0.0);
        assert!(camera.pitch < 0.0);
    }

    #[test]
    fn gimbal_lock_prevention() {
        let mut camera = CameraController::default();

        // Try to look straight up beyond limit
        camera.process_mouse(0.0, -10000.0);

        // Should be clamped to ~89 degrees
        assert!(camera.pitch <= 1.553);
        assert!(camera.pitch >= 1.55);

        // Try to look straight down beyond limit
        camera.process_mouse(0.0, 20000.0);

        // Should be clamped to ~-89 degrees
        assert!(camera.pitch >= -1.553);
        assert!(camera.pitch <= -1.55);
    }

    #[test]
    fn yaw_normalization() {
        let mut camera = CameraController::default();

        // Rotate multiple full circles
        let two_pi = std::f32::consts::PI * 2.0;
        camera.process_mouse(100000.0, 0.0);

        // Should be normalized to [0, 2π]
        assert!(camera.yaw >= 0.0);
        assert!(camera.yaw < two_pi);
    }

    #[test]
    fn forward_direction_calculation() {
        // Looking along +Z (yaw=0, pitch=0)
        let camera = CameraController::new([0.0, 0.0, 0.0], 0.0, 0.0, 0.002);
        let forward = camera.forward();

        // Should be approximately [0, 0, 1]
        assert!((forward[0] - 0.0).abs() < 0.01);
        assert!((forward[1] - 0.0).abs() < 0.01);
        assert!((forward[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn forward_direction_with_yaw() {
        // Looking along +X (yaw=90 degrees)
        let camera =
            CameraController::new([0.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2, 0.0, 0.002);
        let forward = camera.forward();

        // Should be approximately [1, 0, 0]
        assert!((forward[0] - 1.0).abs() < 0.01);
        assert!((forward[1] - 0.0).abs() < 0.01);
        assert!((forward[2] - 0.0).abs() < 0.01);
    }

    #[test]
    fn forward_direction_with_pitch() {
        // Looking up 45 degrees
        let camera =
            CameraController::new([0.0, 0.0, 0.0], 0.0, std::f32::consts::FRAC_PI_4, 0.002);
        let forward = camera.forward();

        // Y component should be positive (looking up)
        assert!(forward[1] > 0.5);
    }

    #[test]
    fn right_direction_calculation() {
        // Looking along +Z, right should be +X
        let camera = CameraController::new([0.0, 0.0, 0.0], 0.0, 0.0, 0.002);
        let right = camera.right();

        // Should be approximately [1, 0, 0]
        assert!((right[0] - 1.0).abs() < 0.01);
        assert_eq!(right[1], 0.0);
        assert!((right[2] - 0.0).abs() < 0.01);
    }

    #[test]
    fn get_view_matrix_returns_camera() {
        let controller = CameraController::new([5.0, 10.0, 15.0], 0.0, 0.0, 0.002);
        let camera = controller.get_view_matrix();

        // Position should match
        assert_eq!(camera.position, [5.0, 10.0, 15.0]);

        // Target should be in front of position
        assert!(camera.target[2] > camera.position[2]);

        // Up should be Y-up
        assert_eq!(camera.up, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn sensitivity_affects_rotation_speed() {
        let mut camera_slow = CameraController::new([0.0, 0.0, 0.0], 0.0, 0.0, 0.001);
        let mut camera_fast = CameraController::new([0.0, 0.0, 0.0], 0.0, 0.0, 0.004);

        camera_slow.process_mouse(100.0, 0.0);
        camera_fast.process_mouse(100.0, 0.0);

        // Fast camera should rotate more
        assert!(camera_fast.yaw > camera_slow.yaw * 3.5);
    }
}
