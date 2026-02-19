# SYSTEM_SPEC.md
**Single Source of Truth**

This document is the authoritative specification for this codebase.

All code, refactors, reviews, and AI assistance MUST conform to this document.
If reality changes, this document MUST be updated first.

---

## 1. Project Identity


Name: Portfolio Engine
Purpose: Deterministic modular 3D rendering engine with strict layering and zero hidden global state.
Non-Goals:
- Not a general-purpose game engine
- No runtime plugin loading
- No dynamic reflection system

---

## 2. High-Level Architecture



Dependency direction:

features → runtime → engine

engine MUST NOT depend on runtime or features.
runtime MUST NOT depend on features.

No module may depend on a lower-level subsystem unless explicitly allowed.

---

## 3. AUTO-GENERATED: FILE STRUCTURE

<!-- AUTO:FILE_TREE:START -->
- src
  - engine
    - assets
      - gltf.rs
      - loader.rs
      - mod.rs
    - gpu
      - bind_group.rs
      - command.rs
      - device.rs
      - mod.rs
      - pipeline.rs
      - surface.rs
    - math
      - camera.rs
      - mod.rs
      - transform.rs
    - mod.rs
    - render
      - basic.wgsl
      - bloom.rs
      - bloom.wgsl
      - material.rs
      - mesh.rs
      - mod.rs
      - pass.rs
      - renderer.rs
      - shadow.rs
      - shadow_pass.wgsl
      - skybox.rs
      - skybox.wgsl
    - time
      - clock.rs
      - mod.rs
  - features
    - example_feature
      - feature.rs
      - mod.rs
      - shader.wgsl
      - system.rs
      - test.rs
    - mod.rs
  - lib.rs
  - runtime
    - input.rs
    - mod.rs
    - scene.rs
    - scene_loader.rs
    - scheduler.rs
<!-- AUTO:FILE_TREE:END -->

---

## 4. Module Responsibilities & Boundaries

### Example
**renderer/**
- Responsibilities:
  - GPU initialization
  - Render pipelines
- Forbidden:
  - App state mutation

(TODO: fill for all modules)

---



## AUTO-GENERATED: FUNCTIONS (ALL)

<!-- AUTO:FUNCTIONS:START -->
### `src/lib.rs`
- `pub fn start() -> ()`

### `src/runtime/input.rs`
- `pub fn new(radius: f32) -> Self`
- `pub fn update_camera(&self, camera: &mut Camera) -> ()`
- `pub fn on_mouse_down(&mut self, x: f32, y: f32) -> ()`
- `pub fn on_mouse_up(&mut self) -> ()`
- `pub fn on_mouse_move(&mut self, x: f32, y: f32) -> ()`
- `pub fn on_scroll(&mut self, delta: f32) -> ()`
- `pub fn attach_orbit_listeners(canvas  : &HtmlCanvasElement,
    orbit   : Rc<RefCell<OrbitCamera>>,) -> Vec<Closure<dyn FnMut(JsValue)>>`

### `src/runtime/scene.rs`
- `pub fn new(camera: Rc<RefCell<Camera>>) -> Self`
- `pub fn add_object(&mut self,
        mesh      : Rc<Mesh>,
        transform : Rc<RefCell<Transform>>,
        material  : Material,) -> ()`
- `pub fn add_point_light(&mut self,
        transform : Rc<RefCell<Transform>>,
        light     : PointLight,) -> ()`

### `src/engine/math/transform.rs`
- `pub fn identity() -> Self`
- `pub fn matrix(&self) -> Mat4`

### `src/engine/math/camera.rs`
- `pub fn projection(&self) -> Mat4`
- `pub fn view(&self) -> Mat4`
- `pub fn view_projection(&self) -> Mat4`

### `src/engine/time/clock.rs`
- `pub fn new() -> Clock`
- `pub fn tick(&mut self) -> ()`
- `pub fn delta_seconds(&self) -> f32`

### `src/engine/render/skybox.rs`
- `pub fn new(device: &Device, format: TextureFormat) -> Self`

### `src/engine/render/renderer.rs`
- `private fn new(device: &Device, format: TextureFormat, width: u32, height: u32) -> Self`
- `pub fn new(device         : &Device,
        surface_format : TextureFormat,
        width          : u32,
        height         : u32,) -> Self`
- `pub fn resize(&mut self, device: &Device, width: u32, height: u32) -> ()`
- `private fn make_depth(device: &Device, width: u32, height: u32) -> (Texture, TextureView)`
- `private fn make_shadow_pipeline(device: &Device, layout: &BindGroupLayout) -> RenderPipeline`
- `private fn make_main_pipeline(device : &Device,
        layout : &BindGroupLayout,
        format : TextureFormat,) -> RenderPipeline`
- `private fn light_view_proj(scene: &Scene) -> glam::Mat4`
- `pub fn render_scene(&mut self,
        device : &Device,
        queue  : &Queue,
        view   : &TextureView,
        scene  : &Scene,) -> ()`

### `src/engine/render/pass.rs`
- `private fn execute(&self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,) -> ()`

### `src/engine/render/material.rs`
- `pub fn default_blue() -> Self`
- `pub fn metal() -> Self`
- `pub fn ground() -> Self`
- `pub fn light_source() -> Self`

### `src/engine/render/shadow.rs`
- `pub fn new(device: &Device) -> Self`

### `src/engine/render/bloom.rs`
- `pub fn new(device: &Device, format: TextureFormat, width: u32, height: u32) -> Self`
- `private fn make_bg(&self, device: &Device, view: &TextureView) -> wgpu::BindGroup`
- `pub fn execute(&self,
        device   : &wgpu::Device,
        encoder  : &mut wgpu::CommandEncoder,
        scene_view  : &TextureView,
        output_view : &TextureView,) -> ()`
- `private fn run_pass(&self,
        encoder  : &mut wgpu::CommandEncoder,
        target   : &TextureView,
        pipeline : &wgpu::RenderPipeline,
        bg       : &wgpu::BindGroup,
        label    : &str,) -> ()`
- `private fn run_pass_additive(&self,
        encoder  : &mut wgpu::CommandEncoder,
        target   : &TextureView,
        pipeline : &wgpu::RenderPipeline,
        bg       : &wgpu::BindGroup,
        label    : &str,) -> ()`

### `src/engine/render/mesh.rs`
- `pub fn layout() -> wgpu::VertexBufferLayout<'static>`
- `private fn new(position: [f32; 3], normal: [f32; 3], tangent: [f32; 3], uv: [f32; 2]) -> Self`
- `private fn upload(device: &wgpu::Device, vertices: &[Vertex], indices: &[u16], label: &str) -> Self`
- `pub fn pyramid(device: &wgpu::Device) -> Self`
- `pub fn sphere(device: &wgpu::Device) -> Self`
- `pub fn ground_plane(device: &wgpu::Device, size: f32, subdivisions: u32) -> Self`

### `src/engine/assets/loader.rs`
- `pub fn load_gltf(path: &str) -> GltfAsset`

### `src/engine/gpu/pipeline.rs`
- `pub fn create(device: &Device,
        layout: &PipelineLayout,
        shader: &ShaderModule,
        format: TextureFormat,) -> RenderPipelineHandle`

### `src/engine/gpu/surface.rs`
- `pub fn new(instance: &Instance, canvas: &'a HtmlCanvasElement) -> GpuSurface<'a>`
- `pub fn configure(&mut self,
        adapter: &Adapter,
        device: &wgpu::Device,) -> ()`
- `pub fn resize(&mut self,
        adapter: &Adapter,
        device: &wgpu::Device,
        width: u32,
        height: u32,) -> ()`

### `src/engine/gpu/bind_group.rs`
- `private fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry`
- `pub fn create_uniform_layout(device: &Device) -> BindGroupLayout`
- `pub fn create_scene_material_layout(device: &Device) -> BindGroupLayout`
- `pub fn create_main_pass_layout(device: &Device) -> BindGroupLayout`
- `pub fn create_shadow_pass_layout(device: &Device) -> BindGroupLayout`
- `pub fn create_uniform_bind_group(device : &Device,
    layout : &BindGroupLayout,
    buffer : &wgpu::Buffer,) -> BindGroup`
- `pub fn create_scene_material_bind_group(device          : &Device,
    layout          : &BindGroupLayout,
    scene_buffer    : &wgpu::Buffer,
    material_buffer : &wgpu::Buffer,) -> BindGroup`
- `pub fn create_main_pass_bind_group(device          : &Device,
    layout          : &BindGroupLayout,
    scene_buffer    : &wgpu::Buffer,
    material_buffer : &wgpu::Buffer,
    shadow_view     : &wgpu::TextureView,
    shadow_sampler  : &wgpu::Sampler,) -> BindGroup`

### `src/engine/gpu/command.rs`
- `pub fn begin(device: &Device) -> CommandEncoder`
- `pub fn submit(queue: &Queue, encoder: CommandEncoder) -> ()`

<!-- AUTO:FUNCTIONS:END -->

---

## AUTO-GENERATED: GLOBAL SYMBOLS

<!-- AUTO:GLOBALS:START -->
### `src/engine/render/renderer.rs`
- `const MAX_LIGHTS : usize` (private)

### `src/engine/render/shadow.rs`
- `const SHADOW_SIZE : u32` (pub)

<!-- AUTO:GLOBALS:END -->



## 5. AUTO-GENERATED: PUBLIC RUST APIs

<!-- AUTO:PUBLIC_API:START -->
### `src/lib.rs`

**Functions:**
- `pub fn start(…)`

<!-- AUTO:PUBLIC_API:END -->

---

## 6. Global State & Ownership

If global state exists, define it explicitly.

Example:

- **APP_STATE**
  - Type: TODO
  - Owner: TODO
  - Mutable by: TODO
  - Read by: TODO

If none exist, explicitly state:  
**This system has no global mutable state.**

---

## 7. AUTO-GENERATED: UNSAFE USAGE

This section only lists where `unsafe` exists.
Justification MUST be written manually below.

<!-- AUTO:UNSAFE:START -->
_No unsafe usage detected._
<!-- AUTO:UNSAFE:END -->

### Unsafe Justifications
(TODO: explain each unsafe block)

---

## 8. Performance Constraints (Hard Rules)

- TODO
- TODO

Violating these rules is considered a bug.

---

## 9. System Invariants

These conditions MUST always hold:

- TODO
- TODO

If violated, the system is broken.

---

## 10. Evolution Rules

Changes that REQUIRE updating this document:
- Public APIs
- Module boundaries
- Unsafe usage
- Performance assumptions

Forbidden changes:
- TODO

---

## 11. Open Questions / TODOs

- TODO

