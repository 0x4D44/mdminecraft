//! UI Raycasting - Detect mouse interaction with 3D UI elements

use glam::{Mat4, Vec3};

/// Result of a UI raycast
#[derive(Debug, Clone, Copy)]
pub struct UIRaycastHit {
    /// Position where ray hit the UI element
    pub position: Vec3,
    /// Distance from ray origin to hit point
    pub distance: f32,
    /// UV coordinates on the UI quad (0-1 range)
    pub uv: (f32, f32),
}

/// Axis-aligned bounding box in 3D space
#[derive(Debug, Clone, Copy)]
pub struct UIAABB {
    /// Minimum corner of the box
    pub min: Vec3,
    /// Maximum corner of the box
    pub max: Vec3,
}

impl UIAABB {
    /// Create a new AABB from min and max corners
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from center position and size
    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half_size = size * 0.5;
        Self {
            min: center - half_size,
            max: center + half_size,
        }
    }

    /// Test if a ray intersects this AABB
    /// Returns distance to intersection point if hit
    pub fn ray_intersection(&self, ray_origin: Vec3, ray_dir: Vec3) -> Option<f32> {
        let inv_dir = Vec3::new(1.0 / ray_dir.x, 1.0 / ray_dir.y, 1.0 / ray_dir.z);

        let t1 = (self.min.x - ray_origin.x) * inv_dir.x;
        let t2 = (self.max.x - ray_origin.x) * inv_dir.x;
        let t3 = (self.min.y - ray_origin.y) * inv_dir.y;
        let t4 = (self.max.y - ray_origin.y) * inv_dir.y;
        let t5 = (self.min.z - ray_origin.z) * inv_dir.z;
        let t6 = (self.max.z - ray_origin.z) * inv_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        // If tmax < 0, ray is intersecting AABB but entire AABB is behind us
        if tmax < 0.0 {
            return None;
        }

        // If tmin > tmax, ray doesn't intersect AABB
        if tmin > tmax {
            return None;
        }

        // If tmin < 0, we're inside the AABB
        let distance = if tmin < 0.0 { tmax } else { tmin };

        Some(distance)
    }
}

/// Convert screen coordinates to a 3D ray in world space
pub fn screen_to_ray(
    screen_pos: (f32, f32),
    screen_size: (u32, u32),
    view_matrix: &Mat4,
    projection_matrix: &Mat4,
) -> (Vec3, Vec3) {
    // Convert screen coordinates to normalized device coordinates (-1 to 1)
    let x = (2.0 * screen_pos.0) / screen_size.0 as f32 - 1.0;
    let y = 1.0 - (2.0 * screen_pos.1) / screen_size.1 as f32; // Flip Y

    // Compute ray in clip space
    let ray_clip = Vec3::new(x, y, -1.0);

    // Convert to view space
    let inv_proj = projection_matrix.inverse();
    let ray_eye = inv_proj.project_point3(ray_clip);
    let ray_eye = Vec3::new(ray_eye.x, ray_eye.y, -1.0);

    // Convert to world space
    let inv_view = view_matrix.inverse();
    let ray_world = inv_view.transform_vector3(ray_eye).normalize();

    // Ray origin is camera position (from view matrix)
    let ray_origin = inv_view.transform_point3(Vec3::ZERO);

    (ray_origin, ray_world)
}

/// Raycast against a billboard quad (camera-facing)
pub fn raycast_billboard_quad(
    ray_origin: Vec3,
    ray_dir: Vec3,
    quad_center: Vec3,
    quad_size: (f32, f32), // width, height
    camera_pos: Vec3,
) -> Option<UIRaycastHit> {
    // For billboards, we need to compute the quad's orientation based on camera
    // The quad always faces the camera

    // Direction from quad to camera
    let to_camera = (camera_pos - quad_center).normalize();

    // Compute billboard right and up vectors
    let world_up = Vec3::Y;
    let right = world_up.cross(to_camera).normalize();
    let up = to_camera.cross(right);

    // Define quad corners in world space
    let half_width = quad_size.0 * 0.5;
    let half_height = quad_size.1 * 0.5;

    let corners = [
        quad_center - right * half_width - up * half_height, // bottom-left
        quad_center + right * half_width - up * half_height, // bottom-right
        quad_center + right * half_width + up * half_height, // top-right
        quad_center - right * half_width + up * half_height, // top-left
    ];

    // Ray-plane intersection
    let plane_normal = to_camera;
    let denom = ray_dir.dot(plane_normal);

    // Ray parallel to plane
    if denom.abs() < 0.0001 {
        return None;
    }

    let t = (quad_center - ray_origin).dot(plane_normal) / denom;

    // Intersection behind ray origin
    if t < 0.0 {
        return None;
    }

    let hit_pos = ray_origin + ray_dir * t;

    // Check if hit point is inside quad
    let to_hit = hit_pos - quad_center;
    let u = to_hit.dot(right);
    let v = to_hit.dot(up);

    if u.abs() <= half_width && v.abs() <= half_height {
        // Convert to UV coordinates (0-1)
        let uv = (
            (u + half_width) / quad_size.0,
            (v + half_height) / quad_size.1,
        );

        Some(UIRaycastHit {
            position: hit_pos,
            distance: t,
            uv,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_ray_intersection() {
        let aabb = UIAABB::from_center_size(Vec3::ZERO, Vec3::ONE);

        // Ray pointing at center from positive Z
        let hit = aabb.ray_intersection(Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 0.0, -1.0));
        assert!(hit.is_some());
        assert!((hit.unwrap() - 1.5).abs() < 0.001);

        // Ray missing the box
        let miss = aabb.ray_intersection(Vec3::new(2.0, 0.0, 2.0), Vec3::new(0.0, 0.0, -1.0));
        assert!(miss.is_none());
    }

    #[test]
    fn test_billboard_raycast() {
        let quad_center = Vec3::new(0.0, 0.0, -5.0);
        let camera_pos = Vec3::ZERO;

        // Ray pointing directly at quad center
        let ray_origin = Vec3::ZERO;
        let ray_dir = Vec3::new(0.0, 0.0, -1.0);

        let hit = raycast_billboard_quad(
            ray_origin,
            ray_dir,
            quad_center,
            (2.0, 1.0), // 2 units wide, 1 unit tall
            camera_pos,
        );

        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert!((hit.distance - 5.0).abs() < 0.001);
        assert!((hit.uv.0 - 0.5).abs() < 0.001); // Center U
        assert!((hit.uv.1 - 0.5).abs() < 0.001); // Center V
    }
}
