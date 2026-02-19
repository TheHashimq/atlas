use wgpu::{Device, TextureFormat};

pub struct Skybox {
    pub pipeline : wgpu::RenderPipeline,
    pub layout   : wgpu::BindGroupLayout,
}

impl Skybox {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("Skybox Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("Skybox PL"),
            bind_group_layouts:   &[&layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Skybox Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("skybox.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("Skybox Pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module:              &shader,
                entry_point:         Some("vs_sky"),
                buffers:             &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &shader,
                entry_point:         Some("fs_sky"),
                targets:             &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive:     wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format:              wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: false,
                depth_compare:       wgpu::CompareFunction::LessEqual,
                stencil:             Default::default(),
                bias:                Default::default(),
            }),
            multisample: Default::default(),
            multiview:   None,
            cache:       None,
        });

        Self { pipeline, layout }
    }
}
