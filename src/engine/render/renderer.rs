use std::rc::Rc;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer,
    Device, Queue, RenderPipeline, Texture, TextureFormat, TextureView,
};

use crate::{
    engine::{
        gpu::bind_group::{
            create_global_bind_group,
            create_material_bind_group,
            create_shadow_pass_layout, create_uniform_bind_group,
        },
        render::{
            bloom::BloomPass,
            material::Material,
            shadow::ShadowMap,
            skybox::Skybox,
        },
    },
    runtime::scene::Scene,
};

const MAX_LIGHTS  : usize = 4;
const MAX_OBJECTS : usize = 64;

// ================================================================
//  Performance tiers
// ================================================================

#[derive(Clone, Copy, PartialEq)]
pub enum QualityTier {
    Low,
    Balanced,
    Ultra,
}

impl QualityTier {
    pub fn bloom_divisor(&self) -> u32 {
        match self {
            QualityTier::Low      => 4,
            QualityTier::Balanced => 2,
            QualityTier::Ultra    => 2,
        }
    }

    pub fn bloom_enabled(&self) -> bool {
        !matches!(self, QualityTier::Low)
    }
}

// ================================================================
//  Uniforms
// ================================================================

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniforms {
    view_proj       : [[f32; 4]; 4],
    light_view_proj : [[f32; 4]; 4],
    camera_pos      : [f32; 4],
    time            : [f32; 4],
    light_pos       : [[f32; 4]; MAX_LIGHTS],
    light_color     : [[f32; 4]; MAX_LIGHTS],
    fog_params      : [f32; 4],
    fog_color       : [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjectUniforms {
    model    : [[f32; 4]; 4],
    material : Material,
    _pad     : [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowUniforms {
    light_view_proj : [[f32; 4]; 4],
    model           : [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyUniforms {
    view_proj  : [[f32; 4]; 4],
    camera_pos : [f32; 4],
    time       : [f32; 4],
    sun_dir    : [f32; 4],   // xyz = normalized direction toward sun
}

// ================================================================
//  HDR render target — Rgba16Float for true HDR headroom
// ================================================================

const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

struct HdrTarget {
    texture : Texture,
    view    : TextureView,
}

impl HdrTarget {
    fn new(device: &Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("HDR Target"),
            size:            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          HDR_FORMAT,
            usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                           | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:    &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}

// ================================================================
//  Persistent per-frame uniform buffers
// ================================================================

struct FrameBuffers {
    scene_buf    : Buffer,
    sky_buf      : Buffer,
    shadow_bufs  : Vec<Buffer>,
    object_bufs  : Vec<Buffer>,
    global_bg    : BindGroup,
    material_bgs : Vec<BindGroup>,
    object_count : usize,
}

impl FrameBuffers {
    pub fn new(
        device          : &Device,
        global_layout   : &BindGroupLayout,
        material_layout : &BindGroupLayout,
        shadow_map      : &ShadowMap,
        default_texture : &TextureView,
        default_mr      : &TextureView,
        default_normal  : &TextureView,
        default_occl    : &TextureView,
        default_sampler : &wgpu::Sampler,
        count           : usize,
    ) -> Self {
        let scene_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Scene UB"),
            size:               std::mem::size_of::<SceneUniforms>() as u64,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sky_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Sky UB"),
            size:               std::mem::size_of::<SkyUniforms>() as u64,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let global_bg = create_global_bind_group(
            device, global_layout, &scene_buf, &shadow_map.view, &shadow_map.sampler,
        );

        let mut shadow_bufs = Vec::with_capacity(count);
        let mut object_bufs = Vec::with_capacity(count);
        let mut material_bgs = Vec::with_capacity(count);

        for _i in 0..count {
            shadow_bufs.push(device.create_buffer(&wgpu::BufferDescriptor {
                label:              Some("Shadow UB"),
                size:               std::mem::size_of::<ShadowUniforms>() as u64,
                usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            let ob = device.create_buffer(&wgpu::BufferDescriptor {
                label:              Some("Object UB"),
                size:               std::mem::size_of::<ObjectUniforms>() as u64,
                usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mg = create_material_bind_group(
                device, material_layout, &ob,
                default_texture, default_mr, default_normal, default_texture, default_occl,
                default_sampler,
            );
            object_bufs.push(ob);
            material_bgs.push(mg);
        }

        Self {
            scene_buf, sky_buf,
            shadow_bufs, object_bufs,
            global_bg, material_bgs,
            object_count: count,
        }
    }

    fn ensure_capacity(
        &mut self,
        device          : &Device,
        global_layout   : &BindGroupLayout,
        material_layout : &BindGroupLayout,
        shadow_map      : &ShadowMap,
        default_texture : &TextureView,
        default_mr      : &TextureView,
        default_normal  : &TextureView,
        default_occl    : &TextureView,
        default_sampler : &wgpu::Sampler,
        needed          : usize,
    ) {
        if needed <= self.object_count { return; }
        *self = Self::new(device, global_layout, material_layout, shadow_map, default_texture, default_mr, default_normal, default_occl, default_sampler, needed.max(MAX_OBJECTS));
    }
}

// ================================================================
//  Renderer
// ================================================================

pub struct Renderer {
    surface_format  : TextureFormat,
    pub quality     : QualityTier,
    pub skip_effects: bool,

    shadow_map      : ShadowMap,
    shadow_pipeline : RenderPipeline,
    shadow_layout   : BindGroupLayout,

    main_pipeline   : RenderPipeline,
    global_layout   : BindGroupLayout,
    material_layout : BindGroupLayout,

    skybox          : Skybox,
    bloom           : BloomPass,

    hdr_target      : HdrTarget,
    depth_texture   : Texture,
    depth_view      : TextureView,

    frame_buffers   : FrameBuffers,

    pub default_texture : Rc<wgpu::TextureView>,
    pub default_mr      : Rc<wgpu::TextureView>,
    pub default_normal  : Rc<wgpu::TextureView>,
    pub default_occl    : Rc<wgpu::TextureView>,
    pub default_sampler : Rc<wgpu::Sampler>,

    width           : u32,
    height          : u32,
}

impl Renderer {
    pub fn new(
        device         : &Device,
        queue          : &Queue,
        surface_format : TextureFormat,
        width          : u32,
        height         : u32,
    ) -> Self {
        Self::new_with_quality(device, queue, surface_format, width, height, QualityTier::Balanced)
    }

    pub fn new_with_quality(
        device         : &Device,
        queue          : &Queue,
        surface_format : TextureFormat,
        width          : u32,
        height         : u32,
        quality        : QualityTier,
    ) -> Self {
        let shadow_layout   = create_shadow_pass_layout(device);
        let shadow_pipeline = Self::make_shadow_pipeline(device, &shadow_layout);

        let global_layout   = crate::engine::gpu::bind_group::create_global_layout(device);
        let material_layout = crate::engine::gpu::bind_group::create_material_layout(device);
        let main_pipeline   = Self::make_main_pipeline(device, &global_layout, &material_layout, HDR_FORMAT);
        let skybox          = Skybox::new(device, HDR_FORMAT);

        let bloom_div = quality.bloom_divisor();
        let bloom     = BloomPass::new_with_formats(
            device,
            HDR_FORMAT,
            surface_format,
            (width  / bloom_div).max(1),
            (height / bloom_div).max(1),
        );

        let shadow_map  = ShadowMap::new(device);
        let hdr_target  = HdrTarget::new(device, width, height);
        let (depth_texture, depth_view) = Self::make_depth(device, width, height);

        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default White Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );

        let default_view = Rc::new(default_texture.create_view(&wgpu::TextureViewDescriptor::default()));

        // Default Normal (Flat blue: 128, 128, 255)
        let default_normal_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Normal Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_normal_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[128, 128, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let default_normal_view = Rc::new(default_normal_texture.create_view(&wgpu::TextureViewDescriptor::default()));

        // Default MR (Metallic=0, Roughness=0.5 -> grey)
        let default_mr_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default MR Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_mr_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 128, 0, 255], // R=1 (unused), G=0.5 (roughness), B=0 (metallic)
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let default_mr_view = Rc::new(default_mr_texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let default_occl_view = default_view.clone();

        let default_sampler = Rc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear, // Trilinear filtering enabled
            ..Default::default()
        }));

        let frame_buffers = FrameBuffers::new(
            device, &global_layout, &material_layout, &shadow_map, &default_view, &default_mr_view, &default_normal_view, &default_occl_view, &default_sampler, MAX_OBJECTS,
        );

        Self {
            surface_format,
            quality,
            skip_effects: false,
            shadow_map, shadow_pipeline, shadow_layout,
            main_pipeline, global_layout, material_layout,
            skybox, bloom,
            hdr_target, depth_texture, depth_view,
            frame_buffers,
            default_texture: default_view,
            default_mr:      default_mr_view,
            default_normal:  default_normal_view,
            default_occl:    default_occl_view,
            default_sampler,
            width, height,
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        self.width  = width;
        self.height = height;
        let (dt, dv)       = Self::make_depth(device, width, height);
        self.depth_texture = dt;
        self.depth_view    = dv;
        self.hdr_target    = HdrTarget::new(device, width, height);
        let div            = self.quality.bloom_divisor();
        self.bloom         = BloomPass::new_with_formats(
            device,
            HDR_FORMAT,
            self.surface_format,
            (width  / div).max(1),
            (height / div).max(1),
        );
    }

    pub fn set_quality(&mut self, device: &Device, quality: QualityTier) {
        if self.quality == quality { return; }
        self.quality = quality;
        self.resize(device, self.width, self.height);
    }

    fn make_depth(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("Depth"),
            size:            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          wgpu::TextureFormat::Depth24Plus,
            usage:           wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats:    &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    fn make_shadow_pipeline(device: &Device, layout: &BindGroupLayout) -> RenderPipeline {
        use crate::engine::render::mesh::Vertex;
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow PL"), bind_group_layouts: &[layout], push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow_pass.wgsl").into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("Shadow Pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_shadow"),
                buffers: &[Vertex::layout()], compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format:              wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare:       wgpu::CompareFunction::Less,
                stencil:             Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: Default::default(), multiview: None, cache: None,
        })
    }

    fn make_main_pipeline(
        device          : &Device,
        global_layout   : &BindGroupLayout,
        material_layout : &BindGroupLayout,
        format          : TextureFormat,
    ) -> RenderPipeline {
        use crate::engine::render::mesh::Vertex;
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main PL"),
            bind_group_layouts: &[global_layout, material_layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Main Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("basic.wgsl").into()),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("Main Pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()], compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fs_main"),
                targets: &[Some(format.into())], compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format:              wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare:       wgpu::CompareFunction::Less,
                stencil:             Default::default(),
                bias:                Default::default(),
            }),
            multisample: Default::default(), multiview: None, cache: None,
        })
    }

    fn light_view_proj(scene: &Scene) -> glam::Mat4 {
        let light_pos = scene.point_lights
            .first()
            .map(|(t, _)| t.borrow().translation)
            .unwrap_or(glam::Vec3::new(0.0, 8.0, 0.0));
        let view = glam::Mat4::look_at_rh(light_pos, glam::Vec3::ZERO, glam::Vec3::Y);
        let proj = glam::Mat4::orthographic_rh(-12.0, 12.0, -12.0, 12.0, 0.5, 30.0);
        proj * view
    }

    pub fn render_scene(
        &mut self,
        device : &Device,
        queue  : &Queue,
        view   : &TextureView,
        scene  : &Scene,
        vp_override: Option<glam::Mat4>,
    ) {
        let time       = js_sys::Date::now() as f32 * 0.001;
        let light_vp   = Self::light_view_proj(scene);
        let camera_ref = scene.camera.borrow();
        let view_proj  = vp_override.unwrap_or_else(|| camera_ref.view_projection());
        let camera_pos = camera_ref.position;
        drop(camera_ref);

        let mut light_pos   = [[0.0f32; 4]; MAX_LIGHTS];
        let mut light_color = [[0.0f32; 4]; MAX_LIGHTS];
        for (i, (t, l)) in scene.point_lights.iter().take(MAX_LIGHTS).enumerate() {
            let p = t.borrow().translation;
            light_pos[i]   = [p.x, p.y, p.z, 1.0];
            light_color[i] = [l.color[0], l.color[1], l.color[2], l.intensity];
        }

        // No fog for clean model viewer
        let fog_params = [0.0f32, 0.0, 0.0, 0.0];
        let fog_color  = [0.0f32, 0.0, 0.0, 1.0];

        let scene_ub = SceneUniforms {
            view_proj:       view_proj.to_cols_array_2d(),
            light_view_proj: light_vp.to_cols_array_2d(),
            camera_pos:      [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            time:            [time, 0.0, 0.0, 0.0],
            light_pos,
            light_color,
            fog_params,
            fog_color,
        };

        // Direction toward sun (first light) for sky shader
        let sun_pos = scene.point_lights
            .first()
            .map(|(t, _)| t.borrow().translation)
            .unwrap_or(glam::Vec3::new(8.0, 6.0, 5.0));
        let sun_dir = sun_pos.normalize();

        let sky_ub = SkyUniforms {
            view_proj:  view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            time:       [time, 0.0, 0.0, 0.0],
            sun_dir:    [sun_dir.x, sun_dir.y, sun_dir.z, 0.0],
        };

        let obj_count = scene.objects.len();
        self.frame_buffers.ensure_capacity(
            device, &self.global_layout, &self.material_layout, &self.shadow_map,
            &self.default_texture, &self.default_mr, &self.default_normal, &self.default_occl, &self.default_sampler, obj_count,
        );

        queue.write_buffer(&self.frame_buffers.scene_buf, 0, bytemuck::bytes_of(&scene_ub));
        queue.write_buffer(&self.frame_buffers.sky_buf,   0, bytemuck::bytes_of(&sky_ub));

        // Group 0: Global (Scene + Shadows)
        self.frame_buffers.global_bg = create_global_bind_group(
            device, &self.global_layout, &self.frame_buffers.scene_buf,
            &self.shadow_map.view, &self.shadow_map.sampler,
        );

        for (i, obj) in scene.objects.iter().enumerate() {
            let base_color = obj.base_color_tex.as_ref().map(|v| v.as_ref()).unwrap_or(&self.default_texture);
            let mr_tex     = obj.metallic_rough_tex.as_ref().map(|v| v.as_ref()).unwrap_or(&self.default_mr);
            let normal_map = obj.normal_tex.as_ref().map(|v| v.as_ref()).unwrap_or(&self.default_normal); 
            let emissive   = obj.emissive_tex.as_ref().map(|v| v.as_ref()).unwrap_or(&self.default_texture); 
            let occlusion  = obj.occlusion_tex.as_ref().map(|v| v.as_ref()).unwrap_or(&self.default_occl);
            let sampler    = obj.sampler.as_ref().map(|s| s.as_ref()).unwrap_or(&self.default_sampler);

            let mg = crate::engine::gpu::bind_group::create_material_bind_group(
                device, &self.material_layout, &self.frame_buffers.object_bufs[i],
                base_color, mr_tex, normal_map, emissive, occlusion, sampler,
            );

            let model = obj.transform.borrow().matrix();
            let su = ShadowUniforms {
                light_view_proj: light_vp.to_cols_array_2d(),
                model:           model.to_cols_array_2d(),
            };
            queue.write_buffer(&self.frame_buffers.shadow_bufs[i], 0, bytemuck::bytes_of(&su));

            let ou = ObjectUniforms {
                model:    model.to_cols_array_2d(),
                material: obj.material,
                _pad:     [0.0; 4],
            };
            queue.write_buffer(&self.frame_buffers.object_bufs[i], 0, bytemuck::bytes_of(&ou));

            self.frame_buffers.material_bgs[i] = mg;
        }

        let sky_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("Sky BG"),
            layout:  &self.skybox.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0, resource: self.frame_buffers.sky_buf.as_entire_binding(),
            }],
        });

        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("ATLAS Encoder") },
        );

        // ============================================================
        // Pass 1: Shadow (skipped when skip_effects is true)
        // ============================================================
        if !self.skip_effects {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            for (i, obj) in scene.objects.iter().enumerate() {
                if obj.material.is_light > 0.5 { continue; }
                let shadow_bg = create_uniform_bind_group(device, &self.shadow_layout, &self.frame_buffers.shadow_bufs[i]);
                pass.set_bind_group(0, &shadow_bg, &[]);
                pass.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }

        // ============================================================
        // Pass 2: Skybox → HDR target (skipped when skip_effects is true)
        // ============================================================
        {
            // #0a0a0f in linear = approx (0.0015, 0.0015, 0.0024)
            let clear_r = if self.skip_effects { 0.0015 } else { 0.003 };
            let clear_g = if self.skip_effects { 0.0015 } else { 0.004 };
            let clear_b = if self.skip_effects { 0.0024 } else { 0.010 };

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sky Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           &self.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(wgpu::Color { r: clear_r, g: clear_g, b: clear_b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
            });
            if !self.skip_effects {
                pass.set_pipeline(&self.skybox.pipeline);
                pass.set_bind_group(0, &sky_bg, &[]);
                pass.draw(0..3, 0..1);
            }
        }

        // ============================================================
        // Pass 3: Geometry → HDR target
        // ============================================================
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Geometry Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           &self.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load:  wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
            });
            pass.set_pipeline(&self.main_pipeline);
            pass.set_bind_group(0, &self.frame_buffers.global_bg, &[]);
            for (i, obj) in scene.objects.iter().enumerate() {
                pass.set_bind_group(1, &self.frame_buffers.material_bgs[i], &[]);
                pass.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }

        // ============================================================
        // Pass 4+5: Bloom (HDR → surface) or plain blit for Low tier
        // ============================================================
        if self.quality.bloom_enabled() {
            self.bloom.execute(device, &mut encoder, &self.hdr_target.view, view);
        } else {
            self.bloom.blit_to_output(device, &mut encoder, &self.hdr_target.view, view);
        }

        queue.submit(Some(encoder.finish()));
    }
}
