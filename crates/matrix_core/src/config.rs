use serde::{Deserialize, Serialize};

/// Simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    /// Number of particles at Big Bang
    pub particle_count: u32,
    /// Random seed for deterministic simulation
    pub seed: u64,
    /// Initial explosion velocity range
    pub big_bang_velocity: f32,
    /// Gravitational constant scaling
    pub gravity_scale: f32,
    /// Dark matter fraction (0.0 - 1.0)
    pub dark_matter_fraction: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            particle_count: 100_000,
            seed: 42,
            big_bang_velocity: 5.0,
            gravity_scale: 1.0,
            dark_matter_fraction: 0.27,
        }
    }
}
