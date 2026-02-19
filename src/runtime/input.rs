use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, WheelEvent, HtmlCanvasElement};

use crate::engine::math::camera::Camera;

pub struct OrbitCamera {
    pub yaw      : f32,
    pub pitch    : f32,
    pub radius   : f32,
    pub target   : glam::Vec3,
    dragging     : bool,
    last_x       : f32,
    last_y       : f32,
}

impl OrbitCamera {
    pub fn new(radius: f32) -> Self {
        Self {
            yaw: 0.3, pitch: 0.4, radius,
            target: glam::Vec3::ZERO,
            dragging: false, last_x: 0.0, last_y: 0.0,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera) {
        let x = self.radius * self.pitch.cos() * self.yaw.sin();
        let y = self.radius * self.pitch.sin();
        let z = self.radius * self.pitch.cos() * self.yaw.cos();
        camera.position = self.target + glam::Vec3::new(x, y, z);
        camera.target   = self.target;
    }

    pub fn on_mouse_down(&mut self, x: f32, y: f32) {
        self.dragging = true;
        self.last_x = x;
        self.last_y = y;
    }

    pub fn on_mouse_up(&mut self) {
        self.dragging = false;
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if !self.dragging { return; }
        let dx = x - self.last_x;
        let dy = y - self.last_y;
        self.yaw   -= dx * 0.005;
        self.pitch  = (self.pitch + dy * 0.005).clamp(0.05, std::f32::consts::FRAC_PI_2 - 0.05);
        self.last_x = x;
        self.last_y = y;
    }

    pub fn on_scroll(&mut self, delta: f32) {
        self.radius = (self.radius + delta * 0.01).clamp(1.5, 30.0);
    }
}

/// Attach mouse/wheel event listeners to canvas.
/// Returns closures that must be kept alive (store them in your RAF state).
pub fn attach_orbit_listeners(
    canvas  : &HtmlCanvasElement,
    orbit   : Rc<RefCell<OrbitCamera>>,
) -> Vec<Closure<dyn FnMut(JsValue)>> {
    let mut closures = Vec::new();

    // mousedown
    {
        let orbit = orbit.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: MouseEvent = e.dyn_into().unwrap();
            orbit.borrow_mut().on_mouse_down(e.client_x() as f32, e.client_y() as f32);
        }) as Box<dyn FnMut(JsValue)>);
        canvas.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // mouseup
    {
        let orbit = orbit.clone();
        let cb = Closure::wrap(Box::new(move |_e: JsValue| {
            orbit.borrow_mut().on_mouse_up();
        }) as Box<dyn FnMut(JsValue)>);
        canvas.add_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // mousemove
    {
        let orbit = orbit.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: MouseEvent = e.dyn_into().unwrap();
            orbit.borrow_mut().on_mouse_move(e.client_x() as f32, e.client_y() as f32);
        }) as Box<dyn FnMut(JsValue)>);
        canvas.add_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // wheel
    {
        let orbit = orbit.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: WheelEvent = e.dyn_into().unwrap();
            orbit.borrow_mut().on_scroll(e.delta_y() as f32);
        }) as Box<dyn FnMut(JsValue)>);
        canvas.add_event_listener_with_callback("wheel", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    closures
}
