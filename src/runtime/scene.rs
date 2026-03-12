use std::rc::Rc;
use std::cell::RefCell;

use crate::engine::math::{camera::Camera, transform::Transform};
use crate::engine::render::mesh::Mesh;
use crate::engine::render::material::Material;

pub struct PointLight {
    pub color     : [f32; 3],
    pub intensity : f32,
    pub is_light  : f32, // 1.0 for disc mesh, 0.0 for pure light
}

pub struct RenderObject {
    pub mesh            : Rc<Mesh>,
    pub transform       : Rc<RefCell<Transform>>,
    pub material        : Material,
    
    pub base_color_tex   : Option<Rc<wgpu::TextureView>>,
    pub metallic_rough_tex: Option<Rc<wgpu::TextureView>>,
    pub normal_tex       : Option<Rc<wgpu::TextureView>>,
    pub emissive_tex      : Option<Rc<wgpu::TextureView>>,
    pub occlusion_tex     : Option<Rc<wgpu::TextureView>>,
    pub sampler           : Option<Rc<wgpu::Sampler>>,

    pub bounds_min      : glam::Vec3,
    pub bounds_max      : glam::Vec3,
}

pub struct Scene {
    pub camera      : Rc<RefCell<Camera>>,
    pub objects     : Vec<RenderObject>,
    pub point_lights: Vec<(Rc<RefCell<Transform>>, PointLight)>,
}

impl Scene {
    pub fn new(camera: Rc<RefCell<Camera>>) -> Self {
        Self { camera, objects: Vec::new(), point_lights: Vec::new() }
    }

    pub fn add_object(
        &mut self,
        mesh            : Rc<Mesh>,
        transform       : Rc<RefCell<Transform>>,
        material        : Material,
        base_color_tex : Option<Rc<wgpu::TextureView>>,
        mr_tex         : Option<Rc<wgpu::TextureView>>,
        normal_tex     : Option<Rc<wgpu::TextureView>>,
        emissive_tex   : Option<Rc<wgpu::TextureView>>,
        occlusion_tex  : Option<Rc<wgpu::TextureView>>,
        sampler        : Option<Rc<wgpu::Sampler>>,
    ) {
        self.objects.push(RenderObject { 
            mesh, 
            transform, 
            material,
            base_color_tex,
            metallic_rough_tex: mr_tex,
            normal_tex,
            emissive_tex,
            occlusion_tex,
            sampler,
            bounds_min: glam::Vec3::splat(-1.0), 
            bounds_max: glam::Vec3::splat(1.0),
        });
    }

    pub fn add_point_light(
        &mut self,
        transform : Rc<RefCell<Transform>>,
        light     : PointLight,
    ) {
        self.point_lights.push((transform, light));
    }
}
