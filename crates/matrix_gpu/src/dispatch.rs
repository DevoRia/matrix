use super::context::{GpuContext, SimParams};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::render_resource::*;
use matrix_core::constants::WORKGROUP_SIZE;

/// Dispatch the N-body compute shader for one simulation step
pub fn dispatch_nbody(
    device: &RenderDevice,
    queue: &RenderQueue,
    ctx: &mut GpuContext,
    params: &SimParams,
) {
    // Update params uniform
    queue.write_buffer(&ctx.params_buffer, 0, bytemuck::bytes_of(params));

    // Create command encoder
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("nbody_compute_encoder"),
    });

    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("nbody_compute_pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&ctx.pipeline);

        // Select bind group based on ping-pong state
        let bind_group = if ctx.current_buffer == 0 {
            &ctx.bind_group_a
        } else {
            &ctx.bind_group_b
        };
        pass.set_bind_group(0, bind_group, &[]);

        let workgroups = (ctx.particle_count + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
        pass.dispatch_workgroups(workgroups, 1, 1);
    }

    queue.submit(std::iter::once(encoder.finish()));

    // Flip ping-pong
    ctx.current_buffer = 1 - ctx.current_buffer;
}
