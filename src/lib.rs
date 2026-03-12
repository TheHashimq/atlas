mod engine;
mod runtime;
mod features;

use std::cell::RefCell;
use std::rc::Rc;
use glam::{Vec3, Mat4, EulerRot, Quat};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlCanvasElement};

use engine::gpu::{device::GpuDevice, surface::GpuSurface};
use engine::math::{camera::Camera, transform::Transform};
use engine::render::{renderer::{Renderer, QualityTier}};
use engine::render::material::Material;
use engine::render::mesh::Mesh;

use runtime::scene::{Scene, PointLight};
use runtime::input::{OrbitCamera, SunController, KeyboardState, attach_input_listeners};

// ================================================================
//  Helper for fetching bytes over HTTP
// ================================================================

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)?;
    let window = web_sys::window().unwrap();
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: web_sys::Response = resp_value.dyn_into().unwrap();

    if !resp.ok() {
        return Err(JsValue::from_str(&format!("HTTP Error: {}", resp.status())));
    }

    let buffer_value = wasm_bindgen_futures::JsFuture::from(resp.array_buffer()?).await?;
    let buffer = js_sys::Uint8Array::new(&buffer_value);
    let mut bytes = vec![0; buffer.length() as usize];
    buffer.copy_to(&mut bytes);

    Ok(bytes)
}

// ================================================================
//  Vehicle Physics State
// ================================================================

struct VehiclePhysics {
    velocity         : glam::Vec3,
    angular_velocity : f32,       
    base_y           : f32,       
    hover_phase      : f32,       
    accel      : f32,
    turn_accel : f32,
    friction   : f32,     
    ang_frict  : f32,
}

impl VehiclePhysics {
    fn new(base_y: f32) -> Self {
        Self {
            velocity: glam::Vec3::ZERO,
            angular_velocity: 0.0,
            base_y,
            hover_phase: 0.0,
            accel: 25.0,
            turn_accel: 15.0,
            friction: 4.0,
            ang_frict: 8.0,
        }
    }

    fn update(&mut self, transform: &mut Transform, keys: &KeyboardState, dt: f32) {
        let forward = transform.rotation.mul_vec3(glam::Vec3::Z);
        let mut thrust = 0.0;
        if keys.w() { thrust += 1.0; }
        if keys.s() { thrust -= 1.0; }
        let mut turn = 0.0;
        if keys.a() { turn += 1.0; }
        if keys.d() { turn -= 1.0; }

        self.velocity += forward * thrust * self.accel * dt;
        self.angular_velocity += turn * self.turn_accel * dt;

        let lin_damp = (-self.friction * dt).exp();
        let ang_damp = (-self.ang_frict * dt).exp();
        self.velocity *= lin_damp;
        self.angular_velocity *= ang_damp;

        transform.translation += self.velocity * dt;
        transform.rotation *= glam::Quat::from_rotation_y(self.angular_velocity * dt);

        self.hover_phase += dt * 2.5;
        let bob = (self.hover_phase.sin() * 0.06) + ((self.hover_phase * 2.1).sin() * 0.02);
        let speed = self.velocity.dot(forward);
        let pitch_dip = speed * -0.015; 
        let roll_dip = self.angular_velocity * -0.05;

        transform.translation.y = self.base_y + bob;
        let (yaw, _, _) = transform.rotation.to_euler(glam::EulerRot::YXZ);
        transform.rotation = glam::Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch_dip, roll_dip);
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("logger");

    spawn_local(async move {
        let win      = window().expect("no window");
        let document = win.document().expect("no document");
        let canvas = document.get_element_by_id("gfx").expect("canvas not found").dyn_into::<HtmlCanvasElement>().expect("not a canvas");

        let instance = wgpu::Instance::default();
        let mut surface = GpuSurface::new(&instance, &canvas);
        let gpu = GpuDevice::new(&instance, &surface.surface).await;

        let dpr = win.device_pixel_ratio();
        let mut width  = (canvas.client_width()  as f64 * dpr) as u32;
        let mut height = (canvas.client_height() as f64 * dpr) as u32;
        if width  == 0 { width  = 1; }
        if height == 0 { height = 1; }

        canvas.set_width(width);
        canvas.set_height(height);
        surface.resize(&gpu.adapter, &gpu.device, width, height);

        let renderer = Rc::new(RefCell::new(Renderer::new_with_quality(&gpu.device, &gpu.queue, surface.config.format, width, height, QualityTier::Balanced)));
        renderer.borrow_mut().skip_effects = false;

        let orbit_cam = Rc::new(RefCell::new(OrbitCamera::new(25.0)));
        let sun_ctrl  = Rc::new(RefCell::new(SunController::new()));
        let keys      = Rc::new(RefCell::new(KeyboardState::new()));
        let listeners = attach_input_listeners(&canvas, orbit_cam.clone(), sun_ctrl.clone(), keys.clone());

        let camera = Rc::new(RefCell::new(Camera {
            position : Vec3::new(0.0, 3.0, 8.0),
            target   : Vec3::ZERO,
            up       : Vec3::Y,
            aspect   : width as f32 / height as f32,
            fov_y    : 45f32.to_radians(),
            near     : 0.1,
            far      : 500.0,
        }));

        let mut scene = Scene::new(camera.clone());

        // Model loading
        let location = win.location();
        let search = location.search().unwrap_or_default();
        let mut model_name = "hovercar.glb".to_string();
        if search.starts_with("?model=") { model_name = search[7..].to_string(); }
        let model_url = format!("/models/{}", model_name);
        
        if let Ok(model_bytes) = fetch_bytes(&model_url).await {
            log::info!("Loading model: {}", model_url);
            let model_objects = crate::runtime::scene_loader::load_gltf_from_bytes(&gpu.device, &gpu.queue, &model_bytes);
            
            if !model_objects.is_empty() {
                // Determine global bounds for all objects to center it
                let mut global_min = glam::Vec3::splat(f32::MAX);
                let mut global_max = glam::Vec3::splat(f32::MIN);
                for obj in &model_objects {
                    global_min = global_min.min(obj.bounds_min);
                    global_max = global_max.max(obj.bounds_max);
                }

                let max_dim = (global_max - global_min).max_element().max(0.001);
                let scale_factor = 10.0 / max_dim;
                let center = (global_min + global_max) * 0.5;

                for obj in model_objects {
                    {
                        let mut t = obj.transform.borrow_mut();
                        // Combine existing transform (from node hierarchy) with centering/scaling
                        let m = glam::Mat4::from_scale(Vec3::splat(scale_factor)) * 
                                glam::Mat4::from_translation(-center) * 
                                t.matrix();
                        t.set_from_matrix(m);
                    }
                    scene.objects.push(obj);
                }
            }
        }

        // Lighting System
        let sun_t = Rc::new(RefCell::new(Transform::identity()));
        scene.point_lights.push((sun_t.clone(), PointLight { color: [1.0, 0.95, 0.8], intensity: 2.0, is_light: 1.0 }));

        let fill_t = Rc::new(RefCell::new(Transform::identity()));
        { fill_t.borrow_mut().translation = Vec3::new(-30.0, 15.0, -20.0); }
        scene.point_lights.push((fill_t, PointLight { color: [0.5, 0.7, 1.0], intensity: 1.5, is_light: 1.0 }));

        let rim_t = Rc::new(RefCell::new(Transform::identity()));
        { rim_t.borrow_mut().translation = Vec3::new(0.0, -20.0, 30.0); }
        scene.point_lights.push((rim_t, PointLight { color: [0.3, 0.4, 0.8], intensity: 0.8, is_light: 1.0 }));

        // The Visible Sun Orb
        let sun_mesh = Rc::new(Mesh::sphere(&gpu.device));
        scene.add_object(
            sun_mesh, 
            sun_t.clone(), 
            Material { 
                base_color_factor: [1.0, 0.8, 0.3, 1.0], 
                roughness_factor: 1.0, 
                metallic_factor: 0.0, 
                emissive_factor: [80.0, 60.0, 20.0], 
                occlusion_factor: 1.0,
                is_light: 1.0, 
                _pad: [0.0; 1]
            },
            None, None, None, None, None, None
        );

        let scene_rc = Rc::new(RefCell::new(scene));
        let surface_rc = Rc::new(RefCell::new(surface));
        let performance = win.performance().unwrap();
        let mut last_time = performance.now();

        let raf: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let raf_clone = raf.clone();
        let raf_win = win.clone();

        *raf_clone.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            let _keep_alive = &listeners; // Capture to prevent dropping
            let now = performance.now();
            let dt = ((now - last_time) as f32 / 1000.0).clamp(0.001, 0.05);
            last_time = now;

            // Physics Update (Scoped)
            {
                let mut orbit = orbit_cam.borrow_mut();
                let mut k = keys.borrow_mut();
                orbit.update(dt, &mut k);

                let mut scene = scene_rc.borrow_mut();
                if let Some(car) = scene.objects.get_mut(0) {
                    let mut t = car.transform.borrow_mut();
                    t.rotation *= Quat::from_rotation_y(0.15 * dt); // 🚀 SLOW STATION ROTATION
                }

                let mut cam = camera.borrow_mut();
                cam.target = orbit.target;
                cam.position = orbit.get_position();
            }

            // Animation Update (Sun)
            {
                let mut sun_c = sun_ctrl.borrow_mut();
                sun_c.update(dt);
                sun_t.borrow_mut().translation = sun_c.get_position(60.0);
            }

            // Render Pass
            let surf = surface_rc.borrow();
            if let Ok(frame) = surf.surface.get_current_texture() {
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                
                let vp = {
                    let orbit = orbit_cam.borrow();
                    let cam = camera.borrow();
                    let mut m = cam.projection() * cam.view();
                    let tilt_x = orbit.last_mouse_norm.y * 0.04;
                    let tilt_y = -orbit.last_mouse_norm.x * 0.04;
                    m *= Mat4::from_euler(EulerRot::XYZ, tilt_x, tilt_y, 0.0);
                    m
                };

                renderer.borrow_mut().render_scene(&gpu.device, &gpu.queue, &view, &scene_rc.borrow(), Some(vp));
                frame.present();
            }

            raf_win.request_animation_frame(raf.borrow().as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
        }) as Box<dyn FnMut()>));

        win.request_animation_frame(raf_clone.borrow().as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
    });
}
