use web_sys::HtmlCanvasElement;
use wgpu::{Adapter, Instance, Surface, SurfaceConfiguration, TextureFormat};

pub struct GpuSurface<'a> {
    pub surface: Surface<'a>,
    pub config: SurfaceConfiguration,
}

impl<'a> GpuSurface<'a> {
    pub fn new(instance: &Instance, canvas: &'a HtmlCanvasElement) -> GpuSurface<'a> {
        #[cfg(target_arch = "wasm32")]
        let target = wgpu::SurfaceTarget::Canvas(canvas.clone());
        #[cfg(not(target_arch = "wasm32"))]
        let target: wgpu::SurfaceTarget = unimplemented!("Canvas surface is only supported on wasm32-unknown-unknown");

        let surface = instance
            .create_surface(target)
            .expect("Failed to create surface");

        let width = canvas.width().max(1);
        let height = canvas.height().max(1);

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Rgba8Unorm,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        GpuSurface { surface, config }
    }

    pub fn configure(
        &mut self,
        adapter: &Adapter,
        device: &wgpu::Device,
    ) {
        let caps = self.surface.get_capabilities(adapter);

        self.config.format = caps.formats[0];

        self.surface.configure(device, &self.config);
    }

    pub fn resize(
        &mut self,
        adapter: &Adapter,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);

        self.configure(adapter, device);
    }
}

