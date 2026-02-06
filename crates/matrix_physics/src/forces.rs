use matrix_core::constants::{G, SOFTENING};

/// Calculate gravitational acceleration from particle j on particle i
/// Returns [ax, ay, az]
pub fn gravity_acceleration(
    pos_i: [f32; 3],
    pos_j: [f32; 3],
    mass_j: f32,
) -> [f32; 3] {
    let dx = pos_j[0] - pos_i[0];
    let dy = pos_j[1] - pos_i[1];
    let dz = pos_j[2] - pos_i[2];

    let r2 = dx * dx + dy * dy + dz * dz + SOFTENING * SOFTENING;
    let r = r2.sqrt();
    let r3 = r2 * r;

    let f = G * mass_j / r3;

    [f * dx, f * dy, f * dz]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gravity_symmetry() {
        let a1 = gravity_acceleration([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 1.0);
        let a2 = gravity_acceleration([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0);

        // Opposite directions
        assert!((a1[0] + a2[0]).abs() < 1e-6);
        assert!((a1[1] + a2[1]).abs() < 1e-6);
        assert!((a1[2] + a2[2]).abs() < 1e-6);
    }

    #[test]
    fn test_gravity_inverse_square() {
        let a_near = gravity_acceleration([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 1.0);
        let a_far = gravity_acceleration([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], 1.0);

        // At 2x distance, acceleration should be ~1/4 (ignoring softening)
        let ratio = a_near[0] / a_far[0];
        assert!((ratio - 4.0).abs() < 0.5); // approximate due to softening
    }
}
