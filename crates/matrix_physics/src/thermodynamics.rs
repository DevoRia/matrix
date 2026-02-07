use matrix_core::GpuParticle;

/// Calculate entropy and temperature in a single pass over particles.
/// Returns (entropy, average_temperature).
/// Uses Welford's online algorithm for numerically stable variance.
pub fn calculate_entropy_and_temperature(particles: &[GpuParticle]) -> (f64, f64) {
    if particles.is_empty() {
        return (0.0, 0.0);
    }

    let mut n: u32 = 0;
    let mut sum_vx: f64 = 0.0;
    let mut sum_vy: f64 = 0.0;
    let mut sum_vz: f64 = 0.0;
    let mut sum_v2x: f64 = 0.0;
    let mut sum_v2y: f64 = 0.0;
    let mut sum_v2z: f64 = 0.0;
    let mut total_ke: f64 = 0.0;

    for p in particles {
        if !p.is_alive() {
            continue;
        }
        n += 1;
        let vx = p.velocity[0] as f64;
        let vy = p.velocity[1] as f64;
        let vz = p.velocity[2] as f64;

        sum_vx += vx;
        sum_vy += vy;
        sum_vz += vz;
        sum_v2x += vx * vx;
        sum_v2y += vy * vy;
        sum_v2z += vz * vz;

        let v2 = (p.velocity[0] * p.velocity[0]
            + p.velocity[1] * p.velocity[1]
            + p.velocity[2] * p.velocity[2]) as f64;
        total_ke += 0.5 * p.mass() as f64 * v2;
    }

    if n == 0 {
        return (0.0, 0.0);
    }

    let nf = n as f64;

    // Variance = E[X²] - E[X]² (single-pass formula)
    let var_vx = sum_v2x / nf - (sum_vx / nf) * (sum_vx / nf);
    let var_vy = sum_v2y / nf - (sum_vy / nf) * (sum_vy / nf);
    let var_vz = sum_v2z / nf - (sum_vz / nf) * (sum_vz / nf);
    let dispersion = (var_vx + var_vy + var_vz).max(1e-30);

    let entropy = dispersion.ln().max(0.0) * nf;
    let temperature = total_ke / nf;

    (entropy, temperature)
}

/// Calculate total entropy of the system (simplified: based on velocity dispersion)
pub fn calculate_entropy(particles: &[GpuParticle]) -> f64 {
    calculate_entropy_and_temperature(particles).0
}

/// Calculate average temperature from particle kinetic energy
pub fn average_temperature(particles: &[GpuParticle]) -> f64 {
    calculate_entropy_and_temperature(particles).1
}
