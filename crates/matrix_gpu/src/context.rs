use bevy::render::renderer::RenderDevice;
use bevy::render::render_resource::*;
use matrix_core::GpuParticle;

/// Simulation parameters sent to GPU as uniform buffer
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    pub dt: f32,
    pub softening: f32,
    pub gravity_scale: f32,
    pub particle_count: u32,
    pub scale_factor: f32,
    pub hubble: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

/// Holds all GPU resources for the compute pipeline
pub struct GpuContext {
    pub pipeline: ComputePipeline,
    pub bind_group_layout: BindGroupLayout,
    pub particle_buffer_a: Buffer,
    pub particle_buffer_b: Buffer,
    pub params_buffer: Buffer,
    pub bind_group_a: BindGroup,
    pub bind_group_b: BindGroup,
    pub particle_count: u32,
    pub current_buffer: usize, // 0 = A->B, 1 = B->A (ping-pong)
}

impl GpuContext {
    pub fn new(
        device: &RenderDevice,
        particles: &[GpuParticle],
        params: &SimParams,
    ) -> Self {
        let particle_count = particles.len() as u32;
        let particle_bytes = bytemuck::cast_slice(particles);
        let params_bytes = bytemuck::bytes_of(params);

        // Create shader module
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("nbody_shader"),
            source: ShaderSource::Wgsl(include_str!("../shaders/nbody.wgsl").into()),
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(
            Some("nbody_bind_group_layout"),
            &[
                // particles_in (read)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // particles_out (read_write)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // params (uniform)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("nbody_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Compute pipeline
        let pipeline = device.create_compute_pipeline(&RawComputePipelineDescriptor {
            label: Some("nbody_compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Particle buffers (ping-pong)
        let particle_buffer_a = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("particles_a"),
            contents: particle_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        });

        let particle_buffer_b = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("particles_b"),
            contents: particle_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        });

        // Params uniform buffer
        let params_buffer = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("sim_params"),
            contents: params_bytes,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Bind groups for ping-pong
        let bind_group_a = device.create_bind_group(
            Some("nbody_bind_group_a"),
            &bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer_a.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer_b.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        );

        let bind_group_b = device.create_bind_group(
            Some("nbody_bind_group_b"),
            &bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer_b.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer_a.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        );

        Self {
            pipeline,
            bind_group_layout,
            particle_buffer_a,
            particle_buffer_b,
            params_buffer,
            bind_group_a,
            bind_group_b,
            particle_count,
            current_buffer: 0,
        }
    }

    /// Get the buffer that has the latest particle data
    pub fn current_read_buffer(&self) -> &Buffer {
        if self.current_buffer == 0 {
            &self.particle_buffer_a
        } else {
            &self.particle_buffer_b
        }
    }
}
