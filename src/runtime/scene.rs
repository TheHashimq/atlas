use std::rc::Rc;
use std::cell::RefCell;

use crate::engine::math::{camera::Camera, transform::Transform};
use crate::engine::render::mesh::Mesh;
use crate::engine::render::material::Material;

pub struct PointLight {
    pub color     : [f32; 3],
    pub intensity : f32,
}

pub struct RenderObject {
    pub mesh      : Rc<Mesh>,
    pub transform : Rc<RefCell<Transform>>,
    pub material  : Material,
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
        mesh      : Rc<Mesh>,
        transform : Rc<RefCell<Transform>>,
        material  : Material,
    ) {
        self.objects.push(RenderObject { mesh, transform, material });
    }

    pub fn add_point_light(
        &mut self,
        transform : Rc<RefCell<Transform>>,
        light     : PointLight,
    ) {
        self.point_lights.push((transform, light));
    }
}
