mod engine;
mod runtime;
mod features;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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
    angular_velocity : f32,       // rotation around Y axis
    base_y           : f32,       // target hover height
    hover_phase      : f32,       // accumulated time for sine bob
    
    // Config
    accel      : f32,
    turn_accel : f32,
    friction   : f32,     // exponential dampening
    ang_frict  : f32,
}

impl VehiclePhysics {
    fn new(base_y: f32) -> Self {
        Self {
            velocity: glam::Vec3::ZERO,
            angular_velocity: 0.0,
            base_y,
            hover_phase: 0.0,
            
            accel: 25.0,        // thrust power
            turn_accel: 15.0,   // turning torque
            friction: 4.0,      // slide friction 
            ang_frict: 8.0,     // rotational friction
        }
    }

    fn update(&mut self, transform: &mut Transform, keys: &KeyboardState, dt: f32) {
        // --- 1. Input Forces ---
        // Forward vector comes from the transform's current rotation
        let forward = transform.rotation.mul_vec3(glam::Vec3::Z);
        
        let mut thrust = 0.0;
        if keys.w() { thrust += 1.0; }
        if keys.s() { thrust -= 1.0; }
        
        let mut turn = 0.0;
        if keys.a() { turn += 1.0; } // Turn left
        if keys.d() { turn -= 1.0; } // Turn right

        // Apply forces to velocities
        self.velocity += forward * thrust * self.accel * dt;
        self.angular_velocity += turn * self.turn_accel * dt;

        // --- 2. Damping (Friction) ---
        // Exponential decay: v = v0 * exp(-friction * dt)
        let lin_damp = (-self.friction * dt).exp();
        let ang_damp = (-self.ang_frict * dt).exp();
        self.velocity *= lin_damp;
        self.angular_velocity *= ang_damp;

        // --- 3. Integration ---
        transform.translation += self.velocity * dt;
        transform.rotation *= glam::Quat::from_rotation_y(self.angular_velocity * dt);

        // --- 4. Procedural Hover & Ground Constraint ---
        // Base translation Y is 0.0, calculate hover over it
        self.hover_phase += dt * 2.5; // hover frequency
        
        // Complex mechanical bob: main sine + subtle high-freq sine + velocity dip
        let bob = (self.hover_phase.sin() * 0.06) 
                + ((self.hover_phase * 2.1).sin() * 0.02);
        
        // Dip the nose down slightly based on forward velocity
        let speed = self.velocity.dot(forward);
        let pitch_dip = speed * -0.015; 
        
        // Dip sideways (roll) based on angular velocity (drifting)
        let roll_dip = self.angular_velocity * -0.05;

        // Apply final procedural height and tilts
        transform.translation.y = self.base_y + bob;
        
        // Reconstruct rotation with pitch/roll applied *on top* of the base Y yaw
        let (yaw, _, _) = transform.rotation.to_euler(glam::EulerRot::YXZ);
        transform.rotation = glam::Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch_dip, roll_dip);
    }
}

// ================================================================
//  Main Startup
// ================================================================

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("logger");

    spawn_local(async move {

        let window   = window().expect("no window");
        let document = window.document().expect("no document");

        let canvas = document
            .get_element_by_id("gfx")
            .expect("canvas not found")
            .dyn_into::<HtmlCanvasElement>()
            .expect("not a canvas");

        // ---- GPU ----
        let instance = wgpu::Instance::default();
        let mut surface = GpuSurface::new(&instance, &canvas);
        let gpu = GpuDevice::new(&instance, &surface.surface).await;

        gpu.device.on_uncaptured_error(Arc::new(|error: wgpu::Error| {
            log::error!("WebGPU device error: {:?}", error);
        }));

        let dpr = window.device_pixel_ratio();
        let mut width  = (canvas.client_width()  as f64 * dpr) as u32;
        let mut height = (canvas.client_height() as f64 * dpr) as u32;
        if width  == 0 { width  = 1; }
        if height == 0 { height = 1; }

        canvas.set_width(width);
        canvas.set_height(height);
        surface.resize(&gpu.adapter, &gpu.device, width, height);

        let surface_format = surface.config.format;
        log::info!("Initialized WebGPU surface. Format: {:?}", surface_format);

        // 🌟 Enable Full Effects (Balanced Quality)
        let renderer = Rc::new(RefCell::new(Renderer::new_with_quality(
            &gpu.device, surface_format, width, height, QualityTier::Balanced
        )));
        renderer.borrow_mut().skip_effects = false;

        // ---- Input State ----
        let orbit_cam = Rc::new(RefCell::new(OrbitCamera::new(6.0)));
        let sun_ctrl  = Rc::new(RefCell::new(SunController::new()));
        let keys      = Rc::new(RefCell::new(KeyboardState::new()));
        
        let listeners = attach_input_listeners(&canvas, orbit_cam.clone(), sun_ctrl.clone(), keys.clone());
        let listeners = Rc::new(listeners); // Keep alive
        
        {
            let mut o = orbit_cam.borrow_mut();
            o.t_yaw   = 0.4;
            o.t_pitch = 0.30;
            o.yaw     = 0.4;
            o.pitch   = 0.30;
        }

        let camera = Rc::new(RefCell::new(Camera {
            position : glam::Vec3::new(0.0, 1.5, 5.0),
            target   : glam::Vec3::ZERO,
            up       : glam::Vec3::Y,
            aspect   : width as f32 / height as f32,
            fov_y    : 50f32.to_radians(),
            near     : 0.1,
            far      : 100.0,
        }));

        // ---- Scene ----
        let mut scene = Scene::new(camera.clone());

        // Dynamic Loading - Check URL for model parameter
        let location = window.location();
        let search = location.search().unwrap_or_default();
        let mut model_name = "hovercar.glb".to_string();
        if search.starts_with("?model=") {
            model_name = search[7..].to_string();
        }

        let model_url = format!("/models/{}", model_name);
        log::info!("Fetching model from: {}", model_url);
        
        let mut model_bytes = Vec::new();
        match fetch_bytes(&model_url).await {
            Ok(bytes) => { 
                log::info!("Successfully fetched {} bytes", bytes.len());
                model_bytes = bytes; 
            },
            Err(e) => { 
                let err_str = js_sys::JSON::stringify(&e)
                    .map(|s| String::from(s))
                    .unwrap_or_else(|_| "Unknown Error".to_string());
                log::error!("Failed to fetch model {}: {}", model_url, err_str); 
            }
        }

        // 1. Vehicle
        let mut vehicle_physics = VehiclePhysics::new(0.0);
        let car_transform = Rc::new(RefCell::new(Transform::identity()));
        
        if !model_bytes.is_empty() {
            if let Some(mut hovercar) = crate::runtime::scene_loader::load_gltf_from_bytes(&gpu.device, &model_bytes) {
                {
                    let mut t = hovercar.transform.borrow_mut();
                    t.scale = glam::Vec3::splat(0.5);
                    t.rotation = glam::Quat::from_rotation_y(20f32.to_radians());
                    *car_transform.borrow_mut() = *t; // Capture initial
                }
                hovercar.transform = car_transform.clone();
                scene.objects.push(hovercar);
            }
        }

        // 2. The Sun (Light + Mesh)
        let sun_t = Rc::new(RefCell::new(Transform::identity()));
        let distance = 20.0;
        sun_t.borrow_mut().translation = sun_ctrl.borrow().get_position(distance);
        sun_t.borrow_mut().scale = glam::Vec3::splat(1.2); // Big sun disc

        scene.add_point_light(
            sun_t.clone(),
            PointLight { color: [1.0, 0.95, 0.9], intensity: 45.0 },
        );

        let sun_mesh = Rc::new(Mesh::sphere(&gpu.device));
        scene.add_object(
            sun_mesh,
            sun_t.clone(),
            Material {
                albedo: [1.0, 0.8, 0.3, 1.0], // Not really used for emissive
                roughness: 1.0,
                metallic: 0.0,
                emissive: 60.0,   // Drive the HDR bloom VERY high
                is_light: 1.0,    // Triggers sun_color() in basic.wgsl
            }
        );

        let scene   = Rc::new(RefCell::new(scene));
        let surface = Rc::new(RefCell::new(surface));

        // ---- Timekeeping ----
        let performance = window.performance().unwrap();
        let mut last_time = performance.now();

        // ---- RAF Loop ----
        let raf: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let raf_clone       = raf.clone();
        let window_for_loop = window.clone();

        *raf_clone.borrow_mut() = Some(Closure::wrap(Box::new(move || {

            let _keep_alive = &listeners;

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {

                // --- 1. Timing ---
                let now = performance.now();
                let dt_ms = (now - last_time) as f32;
                last_time = now;
                
                // Clamp dt between 1ms and 50ms to prevent physics explosions and NaN propagation
                let mut dt = (dt_ms / 1000.0).clamp(0.001, 0.05);
                if dt.is_nan() { 
                    log::warn!("Delta time is NaN! Defaulting to 16ms.");
                    dt = 0.016; 
                }
                
                if dt_ms > 100.0 {
                    log::warn!("Significant lag spike detected: {}ms frame time.", dt_ms);
                }

                // --- 2. Resize check ---
                {
                    let canvas_el = window_for_loop
                        .document().unwrap()
                        .get_element_by_id("gfx").unwrap()
                        .dyn_into::<HtmlCanvasElement>().unwrap();

                    let dpr   = window_for_loop.device_pixel_ratio();
                    let new_w = (canvas_el.client_width()  as f64 * dpr) as u32;
                    let new_h = (canvas_el.client_height() as f64 * dpr) as u32;

                    let mut surf = surface.borrow_mut();
                    if new_w != surf.config.width || new_h != surf.config.height {
                        canvas_el.set_width(new_w);
                        canvas_el.set_height(new_h);
                        surf.resize(&gpu.adapter, &gpu.device, new_w, new_h);
                        renderer.borrow_mut().resize(&gpu.device, new_w, new_h);
                        camera.borrow_mut().aspect = new_w as f32 / new_h as f32;
                    }
                }

                // --- 3. Physics & Controllers ---
                {
                    // Update vehicle
                    let mut ct = car_transform.borrow_mut();
                    vehicle_physics.update(&mut ct, &keys.borrow(), dt);
                    
                    // Smooth tracking - Camera follows vehicle translation gracefully
                    let mut orbit = orbit_cam.borrow_mut();
                    let target_cam_pos = ct.translation + glam::Vec3::new(0.0, 1.0, 0.0);
                    
                    // Safe exp calculation - bounded by dt clamp but handle NaN just in case
                    let t = (1.0 - (-8.0_f32 * dt).exp()).clamp(0.0, 1.0);
                    let cam_target = orbit.target;
                    orbit.target += (target_cam_pos - cam_target) * t;
                    
                    if orbit.target.is_nan() {
                        orbit.target = glam::Vec3::ZERO;
                    }

                    // Update camera momentum
                    orbit.update(dt);
                    let mut cam = camera.borrow_mut();
                    orbit.update_camera(&mut cam);

                    // Update sun
                    let mut sun_c = sun_ctrl.borrow_mut();
                    sun_c.update(dt);
                    
                    // Sun orbits around origin
                    let mut st = sun_t.borrow_mut();
                    st.translation = sun_c.get_position(distance);
                }

                // --- 4. Render ---
                let surface_ref = surface.borrow_mut();
                let frame = match surface_ref.surface.get_current_texture() {
                    Ok(f)  => f,
                    Err(e) => {
                        log::warn!("get_current_texture: {:?}", e);
                        return;
                    },
                };

                let view = frame.texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                renderer.borrow_mut().render_scene(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &scene.borrow(),
                );

                frame.present();

            }));

            if let Err(e) = result {
                log::error!("RAF panic: {:?}", e);
            }

            window_for_loop
                .request_animation_frame(
                    raf.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
                )
                .unwrap();

        }) as Box<dyn FnMut()>));

        window
            .request_animation_frame(
                raf_clone.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            )
            .unwrap();
    });
}
