#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Material {
    pub albedo    : [f32; 4],
    pub roughness : f32,
    pub metallic  : f32,
    pub emissive  : f32,
    pub is_light  : f32,
}

impl Material {
    pub fn default_blue() -> Self {
        Self {
            albedo    : [0.2, 0.65, 1.0, 1.0],
            roughness : 0.35,
            metallic  : 0.05,
            emissive  : 0.0,
            is_light  : 0.0,
        }
    }

    pub fn metal() -> Self {
        Self {
            albedo    : [0.9, 0.8, 0.5, 1.0],
            roughness : 0.15,
            metallic  : 0.95,
            emissive  : 0.0,
            is_light  : 0.0,
        }
    }

    pub fn ground() -> Self {
        Self {
            albedo    : [0.18, 0.20, 0.14, 1.0],
            roughness : 0.92,
            metallic  : 0.0,
            emissive  : 0.0,
            is_light  : 0.0,
        }
    }

    pub fn light_source() -> Self {
        Self {
            albedo    : [1.0, 0.95, 0.6, 1.0],
            roughness : 1.0,
            metallic  : 0.0,
            emissive  : 12.0,
            is_light  : 1.0,
        }
    }
}
