#![warn(missing_docs)]
//! Camera system for first-person 3D rendering.

use glam::{Mat4, Vec3};

/// First-person camera with position, orientation, and projection.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera position in world space.
    pub position: Vec3,
    /// Horizontal rotation in radians (around Y axis).
    pub yaw: f32,
    /// Vertical rotation in radians (around local X axis).
    pub pitch: f32,

    /// Field of view in radians.
    pub fov: f32,
    /// Aspect ratio (width / height).
    pub aspect: f32,
    /// Near clipping plane distance.
    pub near: f32,
    /// Far clipping plane distance.
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 100.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            fov: std::f32::consts::FRAC_PI_3, // 60 degrees
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    /// Create a new camera with the given position.
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Get the forward direction vector (where camera is looking).
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    /// Get the right direction vector (camera's local X axis).
    pub fn right(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin(),
            0.0,
            -self.yaw.cos(),
        )
        .normalize()
    }

    /// Get the up direction vector (camera's local Y axis).
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize()
    }

    /// Compute the view matrix (world space -> camera space).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.position + self.forward(), Vec3::Y)
    }

    /// Compute the projection matrix (camera space -> clip space).
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    /// Compute the combined view-projection matrix.
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Move the camera forward by the given distance.
    pub fn move_forward(&mut self, distance: f32) {
        let forward = self.forward();
        self.position += forward * distance;
    }

    /// Move the camera backward by the given distance.
    pub fn move_backward(&mut self, distance: f32) {
        self.move_forward(-distance);
    }

    /// Move the camera right by the given distance.
    pub fn move_right(&mut self, distance: f32) {
        let right = self.right();
        self.position += right * distance;
    }

    /// Move the camera left by the given distance.
    pub fn move_left(&mut self, distance: f32) {
        self.move_right(-distance);
    }

    /// Move the camera up by the given distance (world Y axis).
    pub fn move_up(&mut self, distance: f32) {
        self.position.y += distance;
    }

    /// Move the camera down by the given distance (world Y axis).
    pub fn move_down(&mut self, distance: f32) {
        self.move_up(-distance);
    }

    /// Rotate the camera (add to yaw and pitch).
    ///
    /// # Arguments
    /// * `delta_yaw` - Horizontal rotation delta in radians
    /// * `delta_pitch` - Vertical rotation delta in radians
    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch += delta_pitch;

        // Clamp pitch to avoid gimbal lock
        const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);

        // Normalize yaw to [0, 2π]
        self.yaw = self.yaw.rem_euclid(std::f32::consts::TAU);
    }

    /// Update the aspect ratio (call when window resizes).
    pub fn set_aspect(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    /// Get a frustum for culling (placeholder for now).
    pub fn frustum(&self) -> Frustum {
        Frustum {
            view_projection: self.view_projection_matrix(),
        }
    }
}

/// View frustum for visibility culling.
#[derive(Debug, Clone)]
pub struct Frustum {
    view_projection: Mat4,
}

impl Frustum {
    /// Test if a point is visible (placeholder - always returns true for now).
    pub fn contains_point(&self, _point: Vec3) -> bool {
        // TODO: Implement proper frustum culling
        true
    }

    /// Test if an AABB is visible (placeholder - always returns true for now).
    pub fn contains_aabb(&self, _min: Vec3, _max: Vec3) -> bool {
        // TODO: Implement proper frustum-AABB intersection
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_default_initialization() {
        let camera = Camera::default();
        assert_eq!(camera.position, Vec3::new(0.0, 100.0, 0.0));
        assert_eq!(camera.yaw, 0.0);
        assert_eq!(camera.pitch, 0.0);
    }

    #[test]
    fn camera_forward_direction() {
        let camera = Camera::default();
        let forward = camera.forward();
        // Default yaw=0, pitch=0 should point in +X direction
        assert!((forward.x - 1.0).abs() < 0.01);
        assert!(forward.y.abs() < 0.01);
        assert!(forward.z.abs() < 0.01);
    }

    #[test]
    fn camera_movement() {
        let mut camera = Camera::default();
        let initial_pos = camera.position;

        camera.move_forward(10.0);
        assert_ne!(camera.position, initial_pos);

        camera.move_right(5.0);
        assert_ne!(camera.position, initial_pos);
    }

    #[test]
    fn camera_rotation_clamps_pitch() {
        let mut camera = Camera::default();

        // Try to rotate way beyond vertical limit
        camera.rotate(0.0, 10.0);

        // Pitch should be clamped below 90 degrees
        assert!(camera.pitch < std::f32::consts::FRAC_PI_2);
        assert!(camera.pitch > -std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn camera_matrices_are_valid() {
        let camera = Camera::default();

        let view = camera.view_matrix();
        let proj = camera.projection_matrix();
        let view_proj = camera.view_projection_matrix();

        // Basic sanity check - matrices should not be zero or NaN
        assert!(!view.to_cols_array().iter().all(|&x| x == 0.0));
        assert!(!proj.to_cols_array().iter().all(|&x| x == 0.0));
        assert!(!view_proj.to_cols_array().iter().all(|&x| x == 0.0));

        assert!(view.to_cols_array().iter().all(|x| x.is_finite()));
        assert!(proj.to_cols_array().iter().all(|x| x.is_finite()));
        assert!(view_proj.to_cols_array().iter().all(|x| x.is_finite()));
    }
}
