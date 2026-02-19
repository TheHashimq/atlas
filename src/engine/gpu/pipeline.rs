//! Render Pipeline Creation
//!
//! Pure pipeline construction.
//! No bind group logic here.

use wgpu::{
    Device, PipelineLayout, RenderPipeline, ShaderModule,
    TextureFormat,
};

use crate::engine::render::mesh::Vertex;

pub struct RenderPipelineHandle {
    pub pipeline: RenderPipeline,
}

impl RenderPipelineHandle {
    pub fn create(
        device: &Device,
        layout: &PipelineLayout,
        shader: &ShaderModule,
        format: TextureFormat,
    ) -> RenderPipelineHandle {

        let pipeline = device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("ATLAS Pipeline"),
                layout: Some(layout),

                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },

                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(format.into())],
                    compilation_options: Default::default(),
                }),

                primitive: wgpu::PrimitiveState::default(),

                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),

                multisample: Default::default(),
                multiview: None,
                cache: None,
            },
        );

        RenderPipelineHandle { pipeline }
    }
}

