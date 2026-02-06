use bevy::render::renderer::RenderDevice;
use bevy::render::render_resource::*;
use matrix_core::GpuParticle;

/// Staging buffer for reading particles back from GPU to CPU
pub struct ReadbackBuffer {
    pub staging: Buffer,
    pub size: u64,
}

impl ReadbackBuffer {
    pub fn new(device: &RenderDevice, particle_count: usize) -> Self {
        let size = (std::mem::size_of::<GpuParticle>() * particle_count) as u64;
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("readback_staging"),
            size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { staging, size }
    }
}
