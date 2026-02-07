// Physical constants (simulation-scaled units)
// We use scaled units to keep values in f32-friendly ranges:
// - Distance: 1 unit = 1 megaparsec (Mpc)
// - Mass: 1 unit = 10^10 solar masses
// - Time: 1 unit = ~1 billion years (Gyr)
// This means G ≈ 1.0 in these units (convenient for N-body)

/// Gravitational constant in simulation units
pub const G: f32 = 1.0;

/// Speed of light (unused for now, placeholder for relativistic effects)
pub const C: f32 = 3000.0; // ~300,000 km/s in Mpc/Gyr

/// Softening parameter to prevent singularities in gravity calculation
pub const SOFTENING: f32 = 0.01;

/// Boltzmann constant (simulation units)
pub const K_B: f32 = 1.0;

/// Initial number of particles at Big Bang
pub const INITIAL_PARTICLE_COUNT: u32 = 100_000;

/// Maximum entropy threshold for heat death
pub const MAX_ENTROPY: f64 = 1_000_000.0;

/// Time step for simulation (in Gyr)
pub const DT: f32 = 0.001;

/// Barnes-Hut opening angle (theta)
pub const BH_THETA: f32 = 0.5;

/// Near-field neighbor count for hybrid gravity (butterfly effect)
pub const NEAR_FIELD_K: usize = 32;

/// Near-field softening (much smaller than grid softening for fine-grained interactions)
pub const NEAR_FIELD_SOFTENING: f32 = 0.01;

/// Workgroup size for GPU compute shaders
pub const WORKGROUP_SIZE: u32 = 256;
