use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, WheelEvent, KeyboardEvent, HtmlCanvasElement, window};

use crate::engine::math::camera::Camera;

// ================================================================
//  Keyboard tracking for WASD + Modifiers
// ================================================================

pub struct KeyboardState {
    pub keys: [bool; 256],
}

impl KeyboardState {
    pub fn new() -> Self {
        Self { keys: [false; 256] }
    }
    
    pub fn set_key(&mut self, code: &str, state: bool) {
        let idx = match code {
            "KeyW" | "ArrowUp"    => 0,
            "KeyS" | "ArrowDown"  => 1,
            "KeyA" | "ArrowLeft"  => 2,
            "KeyD" | "ArrowRight" => 3,
            "Space"               => 4,
            "ShiftLeft" | "ShiftRight" => 5,
            _ => return,
        };
        self.keys[idx] = state;
    }

    pub fn w(&self) -> bool { self.keys[0] }
    pub fn s(&self) -> bool { self.keys[1] }
    pub fn a(&self) -> bool { self.keys[2] }
    pub fn d(&self) -> bool { self.keys[3] }
    pub fn space(&self) -> bool { self.keys[4] }
    pub fn shift(&self) -> bool { self.keys[5] }
}

// ================================================================
//  Butter-smooth momentum Orbit Camera
// ================================================================

pub struct OrbitCamera {
    // Current positions
    pub yaw      : f32,
    pub pitch    : f32,
    pub radius   : f32,
    
    // Target positions (for spring interpolation)
    pub t_yaw    : f32,
    pub t_pitch  : f32,
    pub t_radius : f32,

    pub target   : glam::Vec3,
    
    // State
    dragging     : bool,
    last_x       : f32,
    last_y       : f32,
}

impl OrbitCamera {
    pub fn new(radius: f32) -> Self {
        Self {
            yaw: 0.3, pitch: 0.4, radius,
            t_yaw: 0.3, t_pitch: 0.4, t_radius: radius,
            target: glam::Vec3::ZERO,
            dragging: false, last_x: 0.0, last_y: 0.0,
        }
    }

    // Call this every frame with delta-time for smooth spring
    pub fn update(&mut self, dt: f32) {
        let spring = 15.0; // Higher = snappier, lower = slippier
        self.yaw    += (self.t_yaw - self.yaw)       * (1.0 - (-(spring * dt)).exp());
        self.pitch  += (self.t_pitch - self.pitch)   * (1.0 - (-(spring * dt)).exp());
        self.radius += (self.t_radius - self.radius) * (1.0 - (-(spring * dt)).exp());
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
        
        // Modify targets, not current values, for momentum
        self.t_yaw   -= dx * 0.005;
        self.t_pitch  = (self.t_pitch + dy * 0.005).clamp(0.05, std::f32::consts::FRAC_PI_2 - 0.05);
        
        self.last_x = x;
        self.last_y = y;
    }

    pub fn on_scroll(&mut self, delta: f32) {
        self.t_radius = (self.t_radius + delta * 0.01).clamp(1.5, 30.0);
    }
}

// ================================================================
//  Smooth Sun Dragging (Right Click)
// ================================================================

pub struct SunController {
    pub t_yaw   : f32,
    pub t_pitch : f32,
    pub yaw     : f32,
    pub pitch   : f32,
    
    dragging : bool,
    last_x   : f32,
    last_y   : f32,
}

impl SunController {
    pub fn new() -> Self {
        // Initial sun pos roughly (8, 6, 5) -> azimuth/elevation
        let initial_yaw   = 1.0; 
        let initial_pitch = 0.6;
        Self {
            yaw: initial_yaw, pitch: initial_pitch,
            t_yaw: initial_yaw, t_pitch: initial_pitch,
            dragging: false, last_x: 0.0, last_y: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let spring = 12.0;
        self.yaw   += (self.t_yaw - self.yaw)     * (1.0 - (-(spring * dt)).exp());
        self.pitch += (self.t_pitch - self.pitch) * (1.0 - (-(spring * dt)).exp());
    }

    pub fn get_position(&self, distance: f32) -> glam::Vec3 {
        let x = distance * self.pitch.cos() * self.yaw.sin();
        let y = distance * self.pitch.sin();
        let z = distance * self.pitch.cos() * self.yaw.cos();
        glam::Vec3::new(x, y, z)
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
        
        self.t_yaw   -= dx * 0.005;
        self.t_pitch  = (self.t_pitch + dy * 0.005).clamp(-0.2, std::f32::consts::FRAC_PI_2 - 0.05); // allow going slightly below horizon
        
        self.last_x = x;
        self.last_y = y;
    }
}

// ================================================================
//  Event Attachments
// ================================================================

pub fn attach_input_listeners(
    canvas  : &HtmlCanvasElement,
    orbit   : Rc<RefCell<OrbitCamera>>,
    sun     : Rc<RefCell<SunController>>,
    keys    : Rc<RefCell<KeyboardState>>,
) -> Vec<Closure<dyn FnMut(JsValue)>> {
    let mut closures = Vec::new();

    // Prevent context menu on right click
    {
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: MouseEvent = e.dyn_into().unwrap();
            e.prevent_default();
        }) as Box<dyn FnMut(JsValue)>);
        canvas.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // mousedown
    {
        let orbit = orbit.clone();
        let sun   = sun.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: MouseEvent = e.dyn_into().unwrap();
            let btn = e.button();
            let x = e.client_x() as f32;
            let y = e.client_y() as f32;
            
            if btn == 0 { // Left click = Camera
                orbit.borrow_mut().on_mouse_down(x, y);
            } else if btn == 2 || btn == 1 { // Right click or Middle click = Sun
                sun.borrow_mut().on_mouse_down(x, y);
            }
        }) as Box<dyn FnMut(JsValue)>);
        canvas.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // mouseup
    {
        let orbit = orbit.clone();
        let sun   = sun.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: MouseEvent = e.dyn_into().unwrap();
            let btn = e.button();
            if btn == 0 {
                orbit.borrow_mut().on_mouse_up();
            } else if btn == 2 || btn == 1 {
                sun.borrow_mut().on_mouse_up();
            }
        }) as Box<dyn FnMut(JsValue)>);
        
        // Listen on window so dragging outside canvas still releases
        let window = window().unwrap();
        window.add_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // mousemove
    {
        let orbit = orbit.clone();
        let sun   = sun.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: MouseEvent = e.dyn_into().unwrap();
            let x = e.client_x() as f32;
            let y = e.client_y() as f32;
            
            let mut o = orbit.borrow_mut();
            if o.dragging { o.on_mouse_move(x, y); }
            
            let mut s = sun.borrow_mut();
            if s.dragging { s.on_mouse_move(x, y); }
        }) as Box<dyn FnMut(JsValue)>);
        
        let window = window().unwrap();
        window.add_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref()).unwrap();
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

    // keydown
    {
        let keys = keys.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: KeyboardEvent = e.dyn_into().unwrap();
            keys.borrow_mut().set_key(&e.code(), true);
        }) as Box<dyn FnMut(JsValue)>);
        let window = window().unwrap();
        window.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    // keyup
    {
        let keys = keys.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: KeyboardEvent = e.dyn_into().unwrap();
            keys.borrow_mut().set_key(&e.code(), false);
        }) as Box<dyn FnMut(JsValue)>);
        let window = window().unwrap();
        window.add_event_listener_with_callback("keyup", cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    closures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orbit_camera_momentum() {
        let mut cam = OrbitCamera::new(5.0);
        
        // Simulate a mouse drag
        cam.on_mouse_down(0.0, 0.0);
        cam.on_mouse_move(10.0, 5.0);
        cam.on_mouse_up();
        
        // Target yaw should have decreased by 10 * 0.005 = 0.05
        assert!((cam.t_yaw - (0.3 - 0.05)).abs() < 1e-4);
        
        // Run physics integration for 0.1 seconds
        let old_yaw = cam.yaw;
        cam.update(0.1);
        
        // Current yaw should have moved *towards* target yaw, but not reached it instantly
        assert!(cam.yaw < old_yaw, "Yaw should decrease towards target");
        assert!(cam.yaw > cam.t_yaw, "Yaw should not instantly snap to target");
    }

    #[test]
    fn test_keyboard_tracking() {
        let mut keys = KeyboardState::new();
        
        keys.set_key("KeyW", true);
        keys.set_key("Space", true);
        
        assert!(keys.w(), "W should be pressed");
        assert!(keys.space(), "Space should be pressed");
        assert!(!keys.s(), "S should not be pressed");
        
        keys.set_key("KeyW", false);
        assert!(!keys.w(), "W should have been released");
    }
}
