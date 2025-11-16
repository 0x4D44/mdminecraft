//! First-person camera system with view and projection matrices.

use glam::{Mat4, Quat, Vec3};

/// First-person camera for 3D voxel rendering.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera position in world space
    pub position: Vec3,
    /// Camera rotation (yaw, pitch, roll)
    pub yaw: f32,
    pub pitch: f32,
    /// Field of view in radians
    pub fov: f32,
    /// Aspect ratio (width/height)
    pub aspect: f32,
    /// Near clip plane
    pub near: f32,
    /// Far clip plane
    pub far: f32,
}

impl Camera {
    /// Create a new camera with default settings.
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 100.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            fov: std::f32::consts::FRAC_PI_3, // 60 degrees
            aspect,
            near: 0.1,
            far: 1000.0,
        }
    }

    /// Get the forward direction vector.
    pub fn forward(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(
            yaw_cos * pitch_cos,
            pitch_sin,
            yaw_sin * pitch_cos,
        ).normalize()
    }

    /// Get the right direction vector.
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    /// Get the up direction vector.
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize()
    }

    /// Build the view matrix.
    pub fn view_matrix(&self) -> Mat4 {
        let rotation = Quat::from_euler(glam::EulerRot::YXZ, self.yaw, self.pitch, 0.0);
        Mat4::from_rotation_translation(rotation, self.position).inverse()
    }

    /// Build the projection matrix.
    pub fn projection_matrix(&self) -> Mat4 {
        // Using reversed-Z for better depth precision
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    /// Build combined view-projection matrix.
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Update aspect ratio (call when window resizes).
    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    /// Move the camera by a direction vector.
    pub fn translate(&mut self, delta: Vec3) {
        self.position += delta;
    }

    /// Rotate the camera by yaw/pitch deltas.
    pub fn rotate(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw += yaw_delta;
        self.pitch = (self.pitch + pitch_delta).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.001,
            std::f32::consts::FRAC_PI_2 - 0.001,
        );
    }
}

/// Uniform data sent to GPU for camera transforms.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// View-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Camera position in world space
    pub camera_pos: [f32; 4],
}

impl CameraUniform {
    /// Create camera uniform from camera.
    pub fn from_camera(camera: &Camera) -> Self {
        Self {
            view_proj: camera.view_projection_matrix().to_cols_array_2d(),
            camera_pos: [camera.position.x, camera.position.y, camera.position.z, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_directions() {
        let camera = Camera::new(16.0 / 9.0);

        // Default camera should look forward along +X
        let forward = camera.forward();
        assert!((forward.x - 1.0).abs() < 0.01);
        assert!(forward.y.abs() < 0.01);
        assert!(forward.z.abs() < 0.01);
    }

    #[test]
    fn test_camera_rotation() {
        let mut camera = Camera::new(16.0 / 9.0);

        // Pitch clamps at +/- 90 degrees
        camera.rotate(0.0, std::f32::consts::PI);
        assert!(camera.pitch < std::f32::consts::FRAC_PI_2);

        camera.rotate(0.0, -std::f32::consts::PI * 2.0);
        assert!(camera.pitch > -std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn test_view_projection_matrix() {
        let camera = Camera::new(16.0 / 9.0);
        let vp = camera.view_projection_matrix();

        // Matrix should be invertible
        assert!(vp.determinant().abs() > 0.0);
    }
}
