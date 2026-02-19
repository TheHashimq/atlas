//! GPU Device Initialization (wgpu 27 compatible)

use log::{error, info};
use wgpu::{Adapter, Device, Instance, Queue, Surface};

pub struct GpuDevice {
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
}

impl GpuDevice {
    pub async fn new(instance: &Instance, surface: &Surface<'_>) -> GpuDevice {
        info!("🚀 Initializing GPU");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("No compatible adapter found");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ATLAS Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features:
                    wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .unwrap_or_else(|e| {
                error!("Device creation failed: {:?}", e);
                panic!("Device creation failed");
            });

        info!("✅ GPU ready");

        GpuDevice {
            adapter,
            device,
            queue,
        }
    }
}

