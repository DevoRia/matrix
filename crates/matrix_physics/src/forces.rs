use matrix_core::constants::{G, NEAR_FIELD_SOFTENING, SOFTENING};
use matrix_core::GpuParticle;
use std::collections::HashMap;

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

/// Spatial hash for fast neighbor lookups
pub struct SpatialHash {
    pub cells: HashMap<(i32, i32, i32), Vec<usize>>,
    pub cell_size: f32,
}

impl SpatialHash {
    /// Build spatial hash from alive particles
    pub fn build(particles: &[GpuParticle], cell_size: f32) -> Self {
        let mut cells: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in particles.iter().enumerate() {
            if !p.is_alive() {
                continue;
            }
            let key = Self::cell_key(p.position[0], p.position[1], p.position[2], cell_size);
            cells.entry(key).or_default().push(i);
        }
        Self { cells, cell_size }
    }

    #[inline]
    fn cell_key(x: f32, y: f32, z: f32, cell_size: f32) -> (i32, i32, i32) {
        (
            (x / cell_size).floor() as i32,
            (y / cell_size).floor() as i32,
            (z / cell_size).floor() as i32,
        )
    }

    /// Find K nearest neighbors for a particle at given position
    /// Returns indices sorted by distance (closest first)
    pub fn nearest_neighbors(
        &self,
        pos: [f32; 3],
        exclude_idx: usize,
        particles: &[GpuParticle],
        k: usize,
    ) -> Vec<usize> {
        let key = Self::cell_key(pos[0], pos[1], pos[2], self.cell_size);

        // Collect candidates from own cell + 26 neighbors
        let mut candidates: Vec<(usize, f32)> = Vec::with_capacity(128);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbor_key = (key.0 + dx, key.1 + dy, key.2 + dz);
                    if let Some(indices) = self.cells.get(&neighbor_key) {
                        for &idx in indices {
                            if idx == exclude_idx {
                                continue;
                            }
                            let p = &particles[idx];
                            let ddx = p.position[0] - pos[0];
                            let ddy = p.position[1] - pos[1];
                            let ddz = p.position[2] - pos[2];
                            let dist_sq = ddx * ddx + ddy * ddy + ddz * ddz;
                            candidates.push((idx, dist_sq));
                        }
                    }
                }
            }
        }

        // Partial sort: only need K closest
        if candidates.len() > k {
            candidates.select_nth_unstable_by(k, |a, b| a.1.partial_cmp(&b.1).unwrap());
            candidates.truncate(k);
        }

        candidates.iter().map(|&(idx, _)| idx).collect()
    }

    /// Get the set of cell keys that are "near" a given position (own cell + 26 neighbors)
    pub fn neighbor_cell_keys(&self, pos: [f32; 3]) -> Vec<(i32, i32, i32)> {
        let key = Self::cell_key(pos[0], pos[1], pos[2], self.cell_size);
        let mut keys = Vec::with_capacity(27);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    keys.push((key.0 + dx, key.1 + dy, key.2 + dz));
                }
            }
        }
        keys
    }
}

/// Compute near-field direct gravity acceleration from K nearest neighbors
pub fn near_field_gravity(
    pos: [f32; 3],
    neighbors: &[usize],
    particles: &[GpuParticle],
    gravity_strength: f32,
) -> [f32; 3] {
    let mut ax = 0.0f32;
    let mut ay = 0.0f32;
    let mut az = 0.0f32;
    let soft2 = NEAR_FIELD_SOFTENING * NEAR_FIELD_SOFTENING;

    for &j in neighbors {
        let p = &particles[j];
        let dx = p.position[0] - pos[0];
        let dy = p.position[1] - pos[1];
        let dz = p.position[2] - pos[2];
        let r2 = dx * dx + dy * dy + dz * dz + soft2;
        let r = r2.sqrt();
        let inv_r3 = 1.0 / (r2 * r);
        let f = gravity_strength * p.mass() * inv_r3;
        ax += f * dx;
        ay += f * dy;
        az += f * dz;
    }

    [ax, ay, az]
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
