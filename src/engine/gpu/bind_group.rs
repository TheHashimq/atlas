use wgpu::{BindGroup, BindGroupLayout, Device};

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub fn create_uniform_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label:   Some("Uniform Layout"),
        entries: &[uniform_entry(0)],
    })
}

pub fn create_scene_material_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label:   Some("Scene+Material Layout"),
        entries: &[uniform_entry(0), uniform_entry(1)],
    })
}

/// Layout for the main pass: scene UB + material UB + shadow texture + shadow sampler
pub fn create_main_pass_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label:   Some("Main Pass Layout"),
        entries: &[
            uniform_entry(0),  // scene uniforms
            uniform_entry(1),  // object+material uniforms
            // shadow map texture
            wgpu::BindGroupLayoutEntry {
                binding:    2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type:    wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled:   false,
                },
                count: None,
            },
            // shadow comparison sampler
            wgpu::BindGroupLayoutEntry {
                binding:    3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    })
}

/// Layout for shadow pass: just one uniform (light_view_proj + model)
pub fn create_shadow_pass_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label:   Some("Shadow Pass Layout"),
        entries: &[uniform_entry(0)],
    })
}

pub fn create_uniform_bind_group(
    device : &Device,
    layout : &BindGroupLayout,
    buffer : &wgpu::Buffer,
) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   Some("Uniform BG"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
    })
}

pub fn create_scene_material_bind_group(
    device          : &Device,
    layout          : &BindGroupLayout,
    scene_buffer    : &wgpu::Buffer,
    material_buffer : &wgpu::Buffer,
) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   Some("Scene+Material BG"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: scene_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: material_buffer.as_entire_binding() },
        ],
    })
}

pub fn create_main_pass_bind_group(
    device          : &Device,
    layout          : &BindGroupLayout,
    scene_buffer    : &wgpu::Buffer,
    material_buffer : &wgpu::Buffer,
    shadow_view     : &wgpu::TextureView,
    shadow_sampler  : &wgpu::Sampler,
) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   Some("Main Pass BG"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: scene_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: material_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(shadow_view) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(shadow_sampler) },
        ],
    })
}
