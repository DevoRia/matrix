use matrix_core::{GpuParticle, ParticleKind, Region, SimConfig};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

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

/// Generate particles for a specific region, appropriate for the universe age.
/// Deterministic: seeded from region.seed + 42_000.
/// Denser regions get more particles. Particle kinds match the current cosmological era.
pub fn generate_region_particles(region: &Region, age_gyr: f64) -> Vec<GpuParticle> {
    let mut rng = ChaCha8Rng::seed_from_u64(region.seed.wrapping_add(42_000));
    let count = (region.density * 5000.0).clamp(500.0, 10_000.0) as usize;
    let dark_fraction = region.dark_matter.min(0.9);

    let n_dark = (count as f64 * dark_fraction) as usize;
    let n_baryonic = count - n_dark;

    let kinds = phase_appropriate_kinds(age_gyr);
    let max_vel = velocity_for_age(age_gyr);
    let temp = temperature_for_age(age_gyr);

    let half_size = region.size as f32 * 0.4; // scatter within 80% of region volume
    let center = [
        region.center[0] as f32,
        region.center[1] as f32,
        region.center[2] as f32,
    ];

    let mut particles = Vec::with_capacity(count);

    // Baryonic matter
    for _ in 0..n_baryonic {
        let kind = kinds[rng.gen_range(0..kinds.len())];
        let pos = [
            center[0] + rng.gen_range(-half_size..half_size),
            center[1] + rng.gen_range(-half_size..half_size),
            center[2] + rng.gen_range(-half_size..half_size),
        ];
        let vel = random_velocity(&mut rng, max_vel);
        let mass = kind.default_mass() * rng.gen_range(0.5..1.5f32);
        let mut p = GpuParticle::new(pos, vel, mass.max(0.001), 0.0, kind);
        p.temperature = temp;
        particles.push(p);
    }

    // Dark matter
    for _ in 0..n_dark {
        let pos = [
            center[0] + rng.gen_range(-half_size..half_size),
            center[1] + rng.gen_range(-half_size..half_size),
            center[2] + rng.gen_range(-half_size..half_size),
        ];
        let vel = random_velocity(&mut rng, max_vel * 0.8);
        let mass = ParticleKind::DarkMatter.default_mass() * rng.gen_range(0.5..1.5f32);
        let mut p = GpuParticle::new(pos, vel, mass.max(0.001), 0.0, ParticleKind::DarkMatter);
        p.temperature = temp * 0.1; // dark matter is "cold"
        particles.push(p);
    }

    particles
}

/// Particle kinds appropriate for the universe age
fn phase_appropriate_kinds(age_gyr: f64) -> Vec<ParticleKind> {
    if age_gyr < 0.0001 {
        // Big Bang / Inflation: quarks, leptons, photons
        vec![
            ParticleKind::UpQuark,
            ParticleKind::DownQuark,
            ParticleKind::Electron,
            ParticleKind::Photon,
            ParticleKind::Gluon,
        ]
    } else if age_gyr < 0.001 {
        // Nuclear era: protons, neutrons forming
        vec![
            ParticleKind::Proton,
            ParticleKind::Neutron,
            ParticleKind::Electron,
            ParticleKind::Photon,
        ]
    } else if age_gyr < 1.0 {
        // Atomic / Cosmic Dawn: hydrogen, helium
        vec![
            ParticleKind::Hydrogen,
            ParticleKind::Helium,
            ParticleKind::Photon,
        ]
    } else {
        // Stellar era and beyond: heavier elements from stellar nucleosynthesis
        vec![
            ParticleKind::Hydrogen,
            ParticleKind::Helium,
            ParticleKind::Carbon,
            ParticleKind::Nitrogen,
            ParticleKind::Oxygen,
            ParticleKind::Iron,
        ]
    }
}

/// Max particle velocity appropriate for universe age (decreases as universe cools)
fn velocity_for_age(age_gyr: f64) -> f32 {
    if age_gyr < 0.001 {
        5.0  // very fast at early universe
    } else if age_gyr < 1.0 {
        2.0
    } else {
        0.5  // slow thermal motion in mature universe
    }
}

/// Temperature appropriate for universe age
fn temperature_for_age(age_gyr: f64) -> f32 {
    if age_gyr < 0.0001 {
        1e10
    } else if age_gyr < 0.001 {
        1e8
    } else if age_gyr < 1.0 {
        1e4
    } else {
        2.7 * (1.0 + 1.0 / (age_gyr as f32 + 0.1)) // approaches CMB ~2.7K
    }
}

/// Random velocity vector with uniform direction and random magnitude up to max_vel
fn random_velocity(rng: &mut impl Rng, max_vel: f32) -> [f32; 3] {
    let theta = rng.gen_range(0.0..std::f32::consts::TAU);
    let phi = (rng.gen_range(-1.0f32..1.0)).acos();
    let speed = rng.gen_range(0.01..max_vel);
    [
        speed * phi.sin() * theta.cos(),
        speed * phi.sin() * theta.sin(),
        speed * phi.cos(),
    ]
}
