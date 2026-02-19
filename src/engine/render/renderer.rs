use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer,
    Device, Queue, RenderPipeline, Texture, TextureFormat, TextureView,
};

use crate::{
    engine::{
        gpu::bind_group::{
            create_main_pass_bind_group, create_main_pass_layout,
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

const MAX_LIGHTS: usize = 4;

// ---- Uniforms -------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniforms {
    view_proj       : [[f32; 4]; 4],
    light_view_proj : [[f32; 4]; 4],
    camera_pos      : [f32; 4],
    time            : [f32; 4],
    light_pos       : [[f32; 4]; MAX_LIGHTS],
    light_color     : [[f32; 4]; MAX_LIGHTS],
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
}

// ---- HDR render target ---------------------------------------------

struct HdrTarget {
    texture : Texture,
    view    : TextureView,
}

impl HdrTarget {
    fn new(device: &Device, format: TextureFormat, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("HDR Target"),
            size:            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format,
            usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                           | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:    &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}

// ---- Renderer -------------------------------------------------------

pub struct Renderer {
    format          : TextureFormat,

    // Passes
    shadow_map      : ShadowMap,
    shadow_pipeline : RenderPipeline,
    shadow_layout   : BindGroupLayout,

    main_pipeline   : RenderPipeline,
    main_layout     : BindGroupLayout,

    skybox          : Skybox,
    bloom           : BloomPass,

    // Render targets
    hdr_target      : HdrTarget,
    depth_texture   : Texture,
    depth_view      : TextureView,

    width           : u32,
    height          : u32,
}

impl Renderer {
    pub fn new(
        device         : &Device,
        surface_format : TextureFormat,
        width          : u32,
        height         : u32,
    ) -> Self {
        let shadow_layout   = create_shadow_pass_layout(device);
        let shadow_pipeline = Self::make_shadow_pipeline(device, &shadow_layout);

        let main_layout   = create_main_pass_layout(device);
        let main_pipeline = Self::make_main_pipeline(device, &main_layout, surface_format);

        let skybox     = Skybox::new(device, surface_format);
        let bloom      = BloomPass::new(device, surface_format, width, height);
        let shadow_map = ShadowMap::new(device);
        let hdr_target = HdrTarget::new(device, surface_format, width, height);

        let (depth_texture, depth_view) = Self::make_depth(device, width, height);

        Self {
            format: surface_format,
            shadow_map, shadow_pipeline, shadow_layout,
            main_pipeline, main_layout,
            skybox, bloom,
            hdr_target, depth_texture, depth_view,
            width, height,
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        self.width     = width;
        self.height    = height;
        let (dt, dv)   = Self::make_depth(device, width, height);
        self.depth_texture = dt;
        self.depth_view    = dv;
        self.hdr_target    = HdrTarget::new(device, self.format, width, height);
        self.bloom         = BloomPass::new(device, self.format, width, height);
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
        device : &Device,
        layout : &BindGroupLayout,
        format : TextureFormat,
    ) -> RenderPipeline {
        use crate::engine::render::mesh::Vertex;
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main PL"), bind_group_layouts: &[layout], push_constant_ranges: &[],
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
    ) {
        let time       = js_sys::Date::now() as f32 * 0.001;
        let light_vp   = Self::light_view_proj(scene);
        let camera_ref = scene.camera.borrow();
        let view_proj  = camera_ref.view_projection();
        let camera_pos = camera_ref.position;
        drop(camera_ref);

        // Pack lights
        let mut light_pos   = [[0.0f32; 4]; MAX_LIGHTS];
        let mut light_color = [[0.0f32; 4]; MAX_LIGHTS];
        for (i, (t, l)) in scene.point_lights.iter().take(MAX_LIGHTS).enumerate() {
            let p = t.borrow().translation;
            light_pos[i]   = [p.x, p.y, p.z, 1.0];
            light_color[i] = [l.color[0], l.color[1], l.color[2], l.intensity];
        }

        let scene_ub = SceneUniforms {
            view_proj:       view_proj.to_cols_array_2d(),
            light_view_proj: light_vp.to_cols_array_2d(),
            camera_pos:      [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            time:            [time, 0.0, 0.0, 0.0],
            light_pos,
            light_color,
        };

        let scene_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene UB"), contents: bytemuck::bytes_of(&scene_ub),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Sky uniform buffer
        let sky_ub = SkyUniforms {
            view_proj:  view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            time:       [time, 0.0, 0.0, 0.0],
        };
        let sky_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sky UB"), contents: bytemuck::bytes_of(&sky_ub),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let sky_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("Sky BG"),
            layout:  &self.skybox.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0, resource: sky_buf.as_entire_binding(),
            }],
        });

        // Per-object shadow bind groups
        let shadow_bgs: Vec<(Buffer, BindGroup)> = scene.objects.iter().map(|obj| {
            let su = ShadowUniforms {
                light_view_proj: light_vp.to_cols_array_2d(),
                model:           obj.transform.borrow().matrix().to_cols_array_2d(),
            };
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shadow UB"), contents: bytemuck::bytes_of(&su),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bg = create_uniform_bind_group(device, &self.shadow_layout, &buf);
            (buf, bg)
        }).collect();

        // Per-object main bind groups
        let main_bgs: Vec<(Buffer, BindGroup)> = scene.objects.iter().map(|obj| {
            let ou = ObjectUniforms {
                model:    obj.transform.borrow().matrix().to_cols_array_2d(),
                material: obj.material,
                _pad:     [0.0; 4],
            };
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Object UB"), contents: bytemuck::bytes_of(&ou),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bg = create_main_pass_bind_group(
                device, &self.main_layout, &scene_buf, &buf,
                &self.shadow_map.view, &self.shadow_map.sampler,
            );
            (buf, bg)
        }).collect();

        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("ATLAS Encoder") },
        );

        // ============================================================
        // Pass 1: Shadow map
        // ============================================================
        {
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
            for (obj, (_buf, bg)) in scene.objects.iter().zip(shadow_bgs.iter()) {
                if obj.material.is_light > 0.5 { continue; }
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }

        // ============================================================
        // Pass 2: Skybox → HDR target
        // ============================================================
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sky Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           &self.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
            pass.set_pipeline(&self.skybox.pipeline);
            pass.set_bind_group(0, &sky_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // ============================================================
        // Pass 3: Geometry → HDR target (load, don't clear)
        // ============================================================
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Geometry Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           &self.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Load,   // preserve skybox
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load:  wgpu::LoadOp::Load,   // preserve skybox depth
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
            });
            pass.set_pipeline(&self.main_pipeline);
            for (obj, (_buf, bg)) in scene.objects.iter().zip(main_bgs.iter()) {
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, obj.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(obj.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..obj.mesh.index_count, 0, 0..1);
            }
        }
// ============================================================
// Pass 4: Copy HDR → Swapchain (initialize it properly)
// ============================================================
{
    self.bloom.blit_to_output(
    device,
    &mut encoder,
    &self.hdr_target.view,
    view,
);


// ============================================================
// Pass 4: Copy HDR → Swapchain
// ============================================================
self.bloom.blit_to_output(
    device,
    &mut encoder,
    &self.hdr_target.view,
    view,
);

// ============================================================
// Pass 5: Bloom additive
// ============================================================
self.bloom.execute(
    device,
    &mut encoder,
    &self.hdr_target.view,
    view,
);

queue.submit(Some(encoder.finish()));

    }
}
}
