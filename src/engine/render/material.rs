#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Material {
    pub base_color_factor : [f32; 4],
    pub emissive_factor   : [f32; 3],
    pub roughness_factor  : f32,
    pub metallic_factor   : f32,
    pub occlusion_factor  : f32,
    pub is_light          : f32,
    pub _pad              : [f32; 1],
}

impl Material {
    pub fn default_blue() -> Self {
        Self {
            base_color_factor : [0.2, 0.65, 1.0, 1.0],
            emissive_factor   : [0.0, 0.0, 0.0],
            roughness_factor  : 0.35,
            metallic_factor   : 0.05,
            occlusion_factor  : 1.0,
            is_light          : 0.0,
            _pad              : [0.0; 1],
        }
    }

    pub fn metal() -> Self {
        Self {
            base_color_factor : [0.9, 0.8, 0.5, 1.0],
            emissive_factor   : [0.0, 0.0, 0.0],
            roughness_factor  : 0.15,
            metallic_factor   : 0.95,
            occlusion_factor  : 1.0,
            is_light          : 0.0,
            _pad              : [0.0; 1],
        }
    }

    pub fn ground() -> Self {
        Self {
            base_color_factor : [0.18, 0.20, 0.14, 1.0],
            emissive_factor   : [0.0, 0.0, 0.0],
            roughness_factor  : 0.92,
            metallic_factor   : 0.0,
            occlusion_factor  : 1.0,
            is_light          : 0.0,
            _pad              : [0.0; 1],
        }
    }

    pub fn light_source() -> Self {
        Self {
            base_color_factor : [1.0, 0.95, 0.6, 1.0],
            emissive_factor   : [12.0, 12.0, 12.0],
            roughness_factor  : 1.0,
            metallic_factor   : 0.0,
            occlusion_factor  : 0.0,
            is_light          : 1.0,
            _pad              : [0.0; 1],
        }
    }
}
