use matrix_core::GpuParticle;

/// Calculate total entropy of the system (simplified: based on velocity dispersion)
pub fn calculate_entropy(particles: &[GpuParticle]) -> f64 {
    if particles.is_empty() {
        return 0.0;
    }

    // Calculate mean velocity
    let n = particles.len() as f64;
    let mut mean_vx = 0.0f64;
    let mut mean_vy = 0.0f64;
    let mut mean_vz = 0.0f64;

    for p in particles {
        if p.is_alive() {
            mean_vx += p.velocity[0] as f64;
            mean_vy += p.velocity[1] as f64;
            mean_vz += p.velocity[2] as f64;
        }
    }
    mean_vx /= n;
    mean_vy /= n;
    mean_vz /= n;

    // Velocity dispersion (proxy for temperature/entropy)
    let mut dispersion = 0.0f64;
    for p in particles {
        if p.is_alive() {
            let dvx = p.velocity[0] as f64 - mean_vx;
            let dvy = p.velocity[1] as f64 - mean_vy;
            let dvz = p.velocity[2] as f64 - mean_vz;
            dispersion += dvx * dvx + dvy * dvy + dvz * dvz;
        }
    }
    dispersion /= n;

    // Entropy grows with uniformity of velocity distribution
    // Higher dispersion = higher entropy (more disordered)
    dispersion.ln().max(0.0) * n
}

/// Calculate average temperature from particle kinetic energy
pub fn average_temperature(particles: &[GpuParticle]) -> f64 {
    if particles.is_empty() {
        return 0.0;
    }

    let total_ke: f64 = particles
        .iter()
        .filter(|p| p.is_alive())
        .map(|p| {
            let v2 = p.velocity[0] * p.velocity[0]
                + p.velocity[1] * p.velocity[1]
                + p.velocity[2] * p.velocity[2];
            0.5 * p.mass() as f64 * v2 as f64
        })
        .sum();

    total_ke / particles.len() as f64
}
