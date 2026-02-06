// N-body gravity compute shader
// Each invocation computes the gravitational acceleration on one particle
// from all other particles (direct summation O(n^2) — will upgrade to Barnes-Hut later)

struct Particle {
    position: vec4<f32>,  // xyz + mass in w
    velocity: vec4<f32>,  // xyz + charge in w
    kind: u32,
    flags: u32,
    temperature: f32,
    _pad: f32,
}

struct SimParams {
    dt: f32,
    softening: f32,
    gravity_scale: f32,
    particle_count: u32,
    scale_factor: f32,
    hubble: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<storage, read> particles_in: array<Particle>;
@group(0) @binding(1) var<storage, read_write> particles_out: array<Particle>;
@group(0) @binding(2) var<uniform> params: SimParams;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= params.particle_count) {
        return;
    }

    let p = particles_in[idx];

    // Skip dead particles
    if ((p.flags & 1u) == 0u) {
        particles_out[idx] = p;
        return;
    }

    let pos = p.position.xyz;
    let mass_i = p.position.w;

    // Accumulate gravitational acceleration from all other particles
    var acc = vec3<f32>(0.0, 0.0, 0.0);

    for (var j = 0u; j < params.particle_count; j = j + 1u) {
        if (j == idx) {
            continue;
        }

        let other = particles_in[j];
        if ((other.flags & 1u) == 0u) {
            continue;
        }

        let r = other.position.xyz - pos;
        let dist_sq = dot(r, r) + params.softening * params.softening;
        let dist = sqrt(dist_sq);
        let inv_dist3 = 1.0 / (dist_sq * dist);

        acc += r * (params.gravity_scale * other.position.w * inv_dist3);
    }

    // Velocity Verlet integration
    let new_vel = p.velocity.xyz + acc * params.dt;

    // Apply Hubble expansion: positions drift apart
    let expansion = pos * params.hubble * params.dt * 0.001;
    let new_pos = pos + new_vel * params.dt + expansion;

    // Cool down temperature over time
    let new_temp = p.temperature * (1.0 - params.dt * 0.01);

    var out = p;
    out.position = vec4<f32>(new_pos, mass_i);
    out.velocity = vec4<f32>(new_vel, p.velocity.w);
    out.temperature = new_temp;

    particles_out[idx] = out;
}
