use wgpu::{Device, TextureFormat, TextureView, Texture};

pub struct BloomPass {
    // Downsample → blur → upsample textures
    pub ping_texture  : Texture,
    pub ping_view     : TextureView,
    pub pong_texture  : Texture,
    pub pong_view     : TextureView,

    // Pipelines
    pub threshold_pipeline  : wgpu::RenderPipeline,
    pub blur_h_pipeline     : wgpu::RenderPipeline,
    pub blur_v_pipeline     : wgpu::RenderPipeline,
    pub composite_pipeline  : wgpu::RenderPipeline,

    // Layouts
    pub blit_layout   : wgpu::BindGroupLayout,
    pub sampler       : wgpu::Sampler,
}

impl BloomPass {
    pub fn new(device: &Device, format: TextureFormat, width: u32, height: u32) -> Self {
        let bw = (width  / 2).max(1);
        let bh = (height / 2).max(1);

        let make_tex = |label: &str| {
            let t = device.create_texture(&wgpu::TextureDescriptor {
                label:           Some(label),
                size:            wgpu::Extent3d { width: bw, height: bh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count:    1,
                dimension:       wgpu::TextureDimension::D2,
                format,
                usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                               | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats:    &[],
            });
            let v = t.create_view(&wgpu::TextureViewDescriptor::default());
            (t, v)
        };

        let (ping_texture, ping_view) = make_tex("Bloom Ping");
        let (pong_texture, pong_view) = make_tex("Bloom Pong");

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label:          Some("Bloom Sampler"),
            mag_filter:     wgpu::FilterMode::Linear,
            min_filter:     wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("Bloom Blit Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("Bloom PL"),
            bind_group_layouts:   &[&blit_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Bloom Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bloom.wgsl").into()),
        });

        let make_pipeline = |entry: &'static str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label:  Some(entry),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module:              &shader,
                    entry_point:         Some("vs_fullscreen"),
                    buffers:             &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module:              &shader,
                    entry_point:         Some(entry),
                    targets:             &[Some(format.into())],
                    compilation_options: Default::default(),
                }),
                primitive:     wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample:   Default::default(),
                multiview:     None,
                cache:         None,
            })
        };

        Self {
            ping_texture, ping_view,
            pong_texture, pong_view,
            threshold_pipeline : make_pipeline("fs_threshold"),
            blur_h_pipeline    : make_pipeline("fs_blur_h"),
            blur_v_pipeline    : make_pipeline("fs_blur_v"),
            composite_pipeline : make_pipeline("fs_composite"),
            blit_layout,
            sampler,
        }
    }

    fn make_bg(&self, device: &Device, view: &TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("Bloom BG"),
            layout:  &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    /// scene_view  = the HDR scene texture (before bloom)
    /// output_view = the final swapchain view
    pub fn execute(
        &self,
        device   : &wgpu::Device,
        encoder  : &mut wgpu::CommandEncoder,
        scene_view  : &TextureView,
        output_view : &TextureView,
    ) {
        let scene_bg = self.make_bg(device, scene_view);
        let ping_bg  = self.make_bg(device, &self.ping_view);
        let pong_bg  = self.make_bg(device, &self.pong_view);

        // 1. Threshold — extract bright pixels into ping
        self.run_pass(encoder, &self.ping_view, &self.threshold_pipeline, &scene_bg, "Bloom Threshold");

        // 2. Horizontal blur ping → pong
        self.run_pass(encoder, &self.pong_view, &self.blur_h_pipeline, &ping_bg, "Bloom BlurH");

        // 3. Vertical blur pong → ping
        self.run_pass(encoder, &self.ping_view, &self.blur_v_pipeline, &pong_bg, "Bloom BlurV");

        // 4. Composite — scene + bloom ping → output
        // Need two textures bound for composite — use scene_bg + ping
        // We'll do scene in binding 0, bloom in a second bind group isn't
        // possible with current layout, so composite reads scene from ping
        // via the additive blend — scene was already written to output by
        // the main pass, so we just additively blend bloom on top.
        self.run_pass_additive(encoder, output_view, &self.composite_pipeline, &ping_bg, "Bloom Composite");
    }

    pub fn blit_to_output(
    &self,
    device      : &wgpu::Device,
    encoder     : &mut wgpu::CommandEncoder,
    scene_view  : &TextureView,
    output_view : &TextureView,
) {
    let bg = self.make_bg(device, scene_view);

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Bloom HDR Blit"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    // reuse threshold pipeline as simple texture blit
    pass.set_pipeline(&self.threshold_pipeline);
    pass.set_bind_group(0, &bg, &[]);
    pass.draw(0..3, 0..1);
}

    fn run_pass(
        &self,
        encoder  : &mut wgpu::CommandEncoder,
        target   : &TextureView,
        pipeline : &wgpu::RenderPipeline,
        bg       : &wgpu::BindGroup,
        label    : &str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view:           target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set:      None,
            timestamp_writes:         None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);  // fullscreen triangle
    }

    fn run_pass_additive(
        &self,
        encoder  : &mut wgpu::CommandEncoder,
        target   : &TextureView,
        pipeline : &wgpu::RenderPipeline,
        bg       : &wgpu::BindGroup,
        label    : &str,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view:           target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Load,   // keep existing scene pixels
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set:      None,
            timestamp_writes:         None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);
    }
}
