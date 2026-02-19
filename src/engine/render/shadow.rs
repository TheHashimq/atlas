use wgpu::{Device, Texture, TextureView, Sampler};

pub const SHADOW_SIZE: u32 = 2048;

pub struct ShadowMap {
    pub texture : Texture,
    pub view    : TextureView,
    pub sampler : Sampler,
}

impl ShadowMap {
    pub fn new(device: &Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("Shadow Map"),
            size:            wgpu::Extent3d {
                width:                 SHADOW_SIZE,
                height:                SHADOW_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          wgpu::TextureFormat::Depth32Float,
            usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                           | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:    &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Comparison sampler for PCF
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label:              Some("Shadow Sampler"),
            address_mode_u:     wgpu::AddressMode::ClampToEdge,
            address_mode_v:     wgpu::AddressMode::ClampToEdge,
            address_mode_w:     wgpu::AddressMode::ClampToEdge,
            mag_filter:         wgpu::FilterMode::Linear,
            min_filter:         wgpu::FilterMode::Linear,
            mipmap_filter:      wgpu::FilterMode::Nearest,
            compare:            Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self { texture, view, sampler }
    }
}
