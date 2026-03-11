//! Camera Math
//!
//! Pure view/projection matrix calculations.

use glam::{Mat4, Vec3};

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn projection(&self) -> Mat4 {
        // WebGPU uses [0, 1] clip space depth
        Mat4::perspective_rh(
            self.fov_y,
            self.aspect,
            self.near,
            self.far,
        )
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(
            self.position,
            self.target,
            self.up,
        )
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection() * self.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_camera_projection_clip_space() {
        let cam = Camera {
            position: Vec3::ZERO,
            target: Vec3::NEG_Z,
            up: Vec3::Y,
            aspect: 16.0 / 9.0,
            fov_y: PI / 2.0,
            near: 1.0,
            far: 100.0,
        };

        let proj = cam.projection();
        
        // Test near plane projection logic 
        let pt_near = Vec3::new(0.0, 0.0, -1.0);
        let clip_near = proj.project_point3(pt_near);
        
        // In WebGPU, depth (Z) maps [near, far] to [0, 1]
        // This confirms the perspective_rh mapping
        assert!((clip_near.z - 0.0).abs() < 1e-4, "Near plane should map to exactly Z=0 in WebGPU clip space");

        // Test far plane projection
        let pt_far = Vec3::new(0.0, 0.0, -100.0);
        let clip_far = proj.project_point3(pt_far);
        assert!((clip_far.z - 1.0).abs() < 1e-4, "Far plane should map to exactly Z=1 in WebGPU clip space");
    }

    #[test]
    fn test_camera_view_matrix() {
        let cam = Camera {
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            aspect: 1.0,
            fov_y: PI / 4.0,
            near: 0.1,
            far: 10.0,
        };

        let view = cam.view();
        
        // A point at the origin should be translated to -5 on Z axis relative to camera
        let pt = Vec3::ZERO;
        let p_view = view.project_point3(pt);
        assert_eq!(p_view, Vec3::new(0.0, 0.0, -5.0));
    }
}
