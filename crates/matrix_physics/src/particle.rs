use matrix_core::{GpuParticle, ParticleKind, SimConfig};
use rand::Rng;

/// Generate the initial particle distribution for the Big Bang
pub fn generate_big_bang(config: &SimConfig, rng: &mut impl Rng) -> Vec<GpuParticle> {
    let mut particles = Vec::with_capacity(config.particle_count as usize);
    let n = config.particle_count as usize;
    let n_dark = (n as f32 * config.dark_matter_fraction) as usize;
    let n_baryonic = n - n_dark;

    // Baryonic matter: quarks and leptons at Big Bang temperature
    for _ in 0..n_baryonic {
        let kind = match rng.gen_range(0..4) {
            0 => ParticleKind::UpQuark,
            1 => ParticleKind::DownQuark,
            2 => ParticleKind::Electron,
            _ => ParticleKind::Photon,
        };

        let particle = create_big_bang_particle(kind, config.big_bang_velocity, rng);
        particles.push(particle);
    }

    // Dark matter
    for _ in 0..n_dark {
        let particle =
            create_big_bang_particle(ParticleKind::DarkMatter, config.big_bang_velocity * 0.8, rng);
        particles.push(particle);
    }

    particles
}

fn create_big_bang_particle(
    kind: ParticleKind,
    max_vel: f32,
    rng: &mut impl Rng,
) -> GpuParticle {
    // Position: tiny random offset from origin (singularity)
    let pos = [
        rng.gen_range(-0.01..0.01f32),
        rng.gen_range(-0.01..0.01f32),
        rng.gen_range(-0.01..0.01f32),
    ];

    // Velocity: random direction with magnitude up to max_vel
    // Use spherical coordinates for uniform distribution on sphere
    let theta = rng.gen_range(0.0..std::f32::consts::TAU);
    let phi = (rng.gen_range(-1.0..1.0f32)).acos();
    let speed = rng.gen_range(0.1..max_vel);

    let vel = [
        speed * phi.sin() * theta.cos(),
        speed * phi.sin() * theta.sin(),
        speed * phi.cos(),
    ];

    let mass = kind.default_mass() * rng.gen_range(0.5..1.5f32);

    GpuParticle::new(pos, vel, mass.max(0.001), 0.0, kind)
}
