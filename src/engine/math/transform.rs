use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale,
            self.rotation,
            self.translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec3, Quat};

    #[test]
    fn test_identity_transform() {
        let t = Transform::identity();
        assert_eq!(t.translation, Vec3::ZERO);
        assert_eq!(t.scale, Vec3::ONE);
        assert_eq!(t.rotation, Quat::IDENTITY);

        let m = t.matrix();
        assert_eq!(m, Mat4::IDENTITY);
    }

    #[test]
    fn test_transform_matrix() {
        let mut t = Transform::identity();
        t.translation = Vec3::new(2.0, 3.0, 4.0);
        t.scale = Vec3::new(0.5, 0.5, 0.5);
        
        let m = t.matrix();
        
        // Assert point correctly scales then translates
        let p = Vec3::new(10.0, 0.0, 0.0);
        let transformed = m.project_point3(p); // project_point3 applies (M * (p, 1)).xyz
        
        assert_eq!(transformed, Vec3::new(7.0, 3.0, 4.0)); // 10*0.5 + 2 = 7
    }
}
