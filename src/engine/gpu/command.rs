//! Command Encoder Lifecycle
//!
//! Handles encoder creation and submission.

use wgpu::{CommandEncoder, Device, Queue};
use web_sys::console;

pub fn begin(device: &Device) -> CommandEncoder {
    console::log_1(&"Beginning command encoder".into());

    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ATLAS Encoder"),
    })
}

pub fn submit(queue: &Queue, encoder: CommandEncoder) {
    console::log_1(&"Submitting command buffer".into());
    queue.submit(Some(encoder.finish()));
}

