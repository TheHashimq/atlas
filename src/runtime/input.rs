use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, WheelEvent, KeyboardEvent, HtmlCanvasElement, window};
use glam::{Vec2, Vec3};

const ZOOM_SPEED: f32 = 0.5;
const PAN_SPEED: f32 = 0.005;

pub struct KeyboardState {
    pub keys         : [bool; 7], 
    pub is_dragging  : bool,
    pub last_mouse   : Vec2,
    pub mouse_delta  : Vec2,
    pub scroll_delta : f32, 
}

impl KeyboardState {
    pub fn new() -> Self {
        Self {
            keys:         [false; 7],
            is_dragging:  false,
            last_mouse:   Vec2::ZERO,
            mouse_delta:  Vec2::ZERO,
            scroll_delta: 0.0,
        }
    }

    pub fn set_key(&mut self, key: &str, pressed: bool) {
        match key {
            "KeyW" | "ArrowUp"    => self.keys[0] = pressed,
            "KeyA" | "ArrowLeft"  => self.keys[1] = pressed,
            "KeyS" | "ArrowDown"  => self.keys[2] = pressed,
            "KeyD" | "ArrowRight" => self.keys[3] = pressed,
            "Space"               => self.keys[4] = pressed,
            "ShiftLeft" | "ShiftRight" => self.keys[5] = pressed,
            _ => {}
        }
    }

    pub fn set_mouse_pressed(&mut self, pressed: bool, button: i16) {
        if button == 0 { self.is_dragging = pressed; }
        else if button == 2 { self.keys[6] = pressed; }
    }

    pub fn set_mouse_pos(&mut self, x: f32, y: f32) {
        let new_pos = Vec2::new(x, y);
        if self.is_dragging || self.keys[6] {
            self.mouse_delta = new_pos - self.last_mouse;
        }
        self.last_mouse = new_pos;
    }

    pub fn set_scroll(&mut self, delta: f32) { self.scroll_delta = delta; }

    pub fn w(&self) -> bool { self.keys[0] }
    pub fn a(&self) -> bool { self.keys[1] }
    pub fn s(&self) -> bool { self.keys[2] }
    pub fn d(&self) -> bool { self.keys[3] }
    pub fn space(&self) -> bool { self.keys[4] }
    pub fn shift(&self) -> bool { self.keys[5] }
    pub fn is_panning(&self) -> bool { self.keys[6] }
}

pub struct OrbitCamera {
    pub yaw: f32, pub pitch: f32, pub radius: f32,
    pub t_yaw: f32, pub t_pitch: f32, pub t_radius: f32,
    pub target: Vec3,
    pub last_mouse_norm: Vec2, 
}

impl OrbitCamera {
    pub fn new(radius: f32) -> Self {
        Self {
            yaw: 0.3, pitch: 0.4, radius,
            t_yaw: 0.3, t_pitch: 0.4, t_radius: radius,
            target: Vec3::ZERO,
            last_mouse_norm: Vec2::ZERO,
        }
    }

    pub fn update(&mut self, dt: f32, input: &mut KeyboardState) {
        let spring = 15.0; 
        let t = 1.0 - (-(spring * dt)).exp();

        self.last_mouse_norm = Vec2::new(
            (input.last_mouse.x / 800.0) * 2.0 - 1.0,
            (input.last_mouse.y / 600.0) * 2.0 - 1.0,
        ).clamp(Vec2::splat(-1.0), Vec2::splat(1.0));

        if input.is_dragging && !input.is_panning() {
            self.t_yaw   -= input.mouse_delta.x * 0.005;
            self.t_pitch  = (self.t_pitch - input.mouse_delta.y * 0.005).clamp(-1.5, 1.5);
        }

        if input.scroll_delta.abs() > 0.001 {
            self.t_radius = (self.t_radius + input.scroll_delta * ZOOM_SPEED).clamp(1.5, 80.0);
        }

        if input.is_panning() {
            let forward = (self.target - self.get_position_raw()).normalize_or_zero();
            let right = forward.cross(Vec3::Y).normalize_or_zero();
            let up = right.cross(forward).normalize_or_zero();
            self.target += right * -input.mouse_delta.x * PAN_SPEED * self.radius;
            self.target += up * input.mouse_delta.y * PAN_SPEED * self.radius;
        }

        self.yaw    += (self.t_yaw - self.yaw) * t;
        self.pitch  += (self.t_pitch - self.pitch) * t;
        self.radius += (self.t_radius - self.radius) * t;

        input.mouse_delta = Vec2::ZERO;
        input.scroll_delta = 0.0;
    }

    pub fn get_position(&self) -> Vec3 { self.target + self.get_position_raw() }

    fn get_position_raw(&self) -> Vec3 {
        let x = self.radius * self.pitch.cos() * self.yaw.sin();
        let y = self.radius * self.pitch.sin();
        let z = self.radius * self.pitch.cos() * self.yaw.cos();
        Vec3::new(x, y, z)
    }
}

pub struct SunController {
    pub t_yaw: f32, pub t_pitch: f32, pub yaw: f32, pub pitch: f32,
    dragging: bool, last_x: f32, last_y: f32,
}

impl SunController {
    pub fn new() -> Self {
        Self {
            yaw: 1.0, pitch: 0.6,
            t_yaw: 1.0, t_pitch: 0.6,
            dragging: false, last_x: 0.0, last_y: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let spring = 12.0;
        let t = 1.0 - (-(spring * dt)).exp();
        self.yaw   += (self.t_yaw - self.yaw)     * t;
        self.pitch += (self.t_pitch - self.pitch) * t;
    }

    pub fn get_position(&self, distance: f32) -> Vec3 {
        let x = distance * self.pitch.cos() * self.yaw.sin();
        let y = distance * self.pitch.sin();
        let z = distance * self.pitch.cos() * self.yaw.cos();
        Vec3::new(x, y, z)
    }

    pub fn on_mouse_down(&mut self, x: f32, y: f32) {
        self.dragging = true;
        self.last_x = x;
        self.last_y = y;
    }

    pub fn on_mouse_up(&mut self) { self.dragging = false; }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if !self.dragging { return; }
        self.t_yaw -= (x - self.last_x) * 0.005;
        self.t_pitch = (self.t_pitch + (y - self.last_y) * 0.005).clamp(-0.2, 1.5);
        self.last_x = x;
        self.last_y = y;
    }
}

pub fn attach_input_listeners(
    canvas  : &HtmlCanvasElement,
    _orbit   : Rc<RefCell<OrbitCamera>>,
    sun     : Rc<RefCell<SunController>>,
    keys    : Rc<RefCell<KeyboardState>>,
) -> Vec<Closure<dyn FnMut(JsValue)>> {
    let mut closures = Vec::new();

    let cb = Closure::wrap(Box::new(move |e: JsValue| {
        let e: MouseEvent = e.dyn_into().unwrap();
        e.prevent_default();
    }) as Box<dyn FnMut(JsValue)>);
    canvas.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref()).unwrap();
    closures.push(cb);

    let k = keys.clone();
    let s = sun.clone();
    let cb = Closure::wrap(Box::new(move |e: JsValue| {
        let e: MouseEvent = e.dyn_into().unwrap();
        let btn = e.button();
        k.borrow_mut().set_mouse_pressed(true, btn);
        if btn == 2 || btn == 1 {
            s.borrow_mut().on_mouse_down(e.client_x() as f32, e.client_y() as f32);
        }
    }) as Box<dyn FnMut(JsValue)>);
    canvas.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref()).unwrap();
    closures.push(cb);

    let k = keys.clone();
    let s = sun.clone();
    let cb = Closure::wrap(Box::new(move |e: JsValue| {
        let e: MouseEvent = e.dyn_into().unwrap();
        k.borrow_mut().set_mouse_pressed(false, e.button());
        s.borrow_mut().on_mouse_up();
    }) as Box<dyn FnMut(JsValue)>);
    window().unwrap().add_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref()).unwrap();
    closures.push(cb);

    let k = keys.clone();
    let s = sun.clone();
    let cb = Closure::wrap(Box::new(move |e: JsValue| {
        let e: MouseEvent = e.dyn_into().unwrap();
        let x = e.client_x() as f32;
        let y = e.client_y() as f32;
        k.borrow_mut().set_mouse_pos(x, y);
        s.borrow_mut().on_mouse_move(x, y);
    }) as Box<dyn FnMut(JsValue)>);
    window().unwrap().add_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref()).unwrap();
    closures.push(cb);

    let k = keys.clone();
    let cb = Closure::wrap(Box::new(move |e: JsValue| {
        let e: WheelEvent = e.dyn_into().unwrap();
        k.borrow_mut().set_scroll(e.delta_y() as f32 * 0.05);
    }) as Box<dyn FnMut(JsValue)>);
    canvas.add_event_listener_with_callback("wheel", cb.as_ref().unchecked_ref()).unwrap();
    closures.push(cb);

    for (name, pressed) in [("keydown", true), ("keyup", false)] {
        let k = keys.clone();
        let cb = Closure::wrap(Box::new(move |e: JsValue| {
            let e: KeyboardEvent = e.dyn_into().unwrap();
            k.borrow_mut().set_key(&e.code(), pressed);
        }) as Box<dyn FnMut(JsValue)>);
        window().unwrap().add_event_listener_with_callback(name, cb.as_ref().unchecked_ref()).unwrap();
        closures.push(cb);
    }

    closures
}
