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
        // WebGPU uses OpenGL-style clip space
        Mat4::perspective_rh_gl(
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

