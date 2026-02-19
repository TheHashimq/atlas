//! Render Pass Trait
//!
//! Implemented by features. Core defines the abstraction only.

pub trait RenderPass {
    fn execute(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    );
}

