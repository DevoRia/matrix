use bevy::prelude::*;
use matrix_core::constants::NEAR_FIELD_K;
use matrix_core::{GpuParticle, SimConfig, UniversePhase, MAX_ENTROPY};
use matrix_physics::forces::{near_field_gravity, SpatialHash};
use matrix_physics::spacetime;
use matrix_physics::thermodynamics;

/// Global universe state, tracked as a Bevy Resource
#[derive(Resource)]
pub struct UniverseState {
    /// Age of the universe in Gyr (billions of years)
    pub age: f64,
    /// Current scale factor (1.0 at Big Bang, grows with expansion)
    pub scale_factor: f64,
    /// Total entropy of the system
    pub total_entropy: f64,
    /// Current phase of the universe
    pub phase: UniversePhase,
    /// Universe cycle number (increments after each heat death → collapse)
    pub cycle: u32,
    /// Average temperature
    pub temperature: f64,
    /// Whether simulation is paused
    pub paused: bool,
    /// Time scale multiplier (1.0 = normal, 1000.0 = fast, etc.)
    pub time_scale: f64,
    /// Particle data on CPU (synced from GPU periodically)
    pub particles: Vec<GpuParticle>,
    /// Simulation config
    pub config: SimConfig,
    /// Frame counter for throttling gravity
    pub gravity_frame: u32,
    /// Whether particle gravity should be computed (set by render based on camera distance)
    pub particles_active: bool,
    /// Cached alive particle count (updated periodically, not every frame)
    pub cached_alive_count: usize,
    /// Incremented when particles are replaced by lazy loading (render uses this)
    pub particles_generation: u32,
}

impl UniverseState {
    /// Placeholder with no particles (used before world generation completes)
    pub fn empty(config: SimConfig) -> Self {
        Self::new(config, Vec::new())
    }

    pub fn new(config: SimConfig, particles: Vec<GpuParticle>) -> Self {
        let count = particles.len();
        Self {
            age: 0.0,
            scale_factor: 1.0,
            total_entropy: 0.0,
            phase: UniversePhase::BigBang,
            cycle: 1,
            temperature: 1e10,
            paused: false,
            time_scale: 1.0,
            particles,
            config,
            gravity_frame: 0,
            particles_active: true,
            cached_alive_count: count,
            particles_generation: 0,
        }
    }

    /// Advance the universe by one tick
    pub fn tick(&mut self, dt: f64) {
        if self.paused {
            return;
        }

        let effective_dt = dt * self.time_scale;
        self.age += effective_dt;

        self.gravity_frame = self.gravity_frame.wrapping_add(1);

        // Throttle gravity: hybrid gravity is heavy (~400M ops)
        // Even at time_scale 1, run every 3rd frame for smooth 60fps
        let gravity_interval = if self.time_scale >= 1_000_000.0 {
            120
        } else if self.time_scale >= 10_000.0 {
            30
        } else if self.time_scale >= 100.0 {
            5
        } else {
            3
        };

        let run_gravity = self.particles_active && (self.gravity_frame % gravity_interval == 0);

        if run_gravity {
            self.tick_particles(effective_dt);
        }

        // These are cheap — always run
        let hubble = spacetime::hubble_parameter(self.age, self.phase) as f64;
        self.scale_factor =
            spacetime::expand_scale_factor(self.scale_factor, hubble, effective_dt);

        // Thermodynamics + alive count: every 30 frames
        if self.gravity_frame % 30 == 0 {
            let (entropy, temp) =
                thermodynamics::calculate_entropy_and_temperature(&self.particles);
            self.total_entropy = entropy;
            self.temperature = temp;
            self.cached_alive_count = self.particles.iter().filter(|p| p.is_alive()).count();
        }

        // Compact: remove dead particles every 100 frames
        if self.gravity_frame % 100 == 0 {
            self.compact_particles();
        }

        // Phase transitions
        self.update_phase();
    }

    /// Remove dead particles from the array to reduce iteration cost
    fn compact_particles(&mut self) {
        let before = self.particles.len();
        self.particles.retain(|p| p.is_alive());
        let after = self.particles.len();
        if before != after {
            info!("Compacted particles: {} → {} (removed {})", before, after, before - after);
        }
    }

    /// Heavy particle simulation: hybrid gravity (near-field direct + far-field grid) + integration
    fn tick_particles(&mut self, effective_dt: f64) {
        let sim_dt = effective_dt as f32 * 0.1;
        let hubble = spacetime::hubble_parameter(self.age, self.phase) as f32;
        let gravity_strength = self.config.gravity_scale * 0.5;

        // --- Far-field: grid-based gravity approximation ---
        let grid_size: i32 = 16;
        let total_cells = (grid_size * grid_size * grid_size) as usize;

        // Find bounding box
        let mut bb_min = [f32::MAX; 3];
        let mut bb_max = [f32::MIN; 3];
        for p in self.particles.iter() {
            if !p.is_alive() {
                continue;
            }
            for i in 0..3 {
                bb_min[i] = bb_min[i].min(p.position[i]);
                bb_max[i] = bb_max[i].max(p.position[i]);
            }
        }
        let bb_range = [
            (bb_max[0] - bb_min[0]).max(1.0),
            (bb_max[1] - bb_min[1]).max(1.0),
            (bb_max[2] - bb_min[2]).max(1.0),
        ];

        // Accumulate mass and position per grid cell
        let mut cell_mass = vec![0.0f32; total_cells];
        let mut cell_pos = vec![[0.0f64; 3]; total_cells];

        for p in self.particles.iter() {
            if !p.is_alive() {
                continue;
            }
            let gx = (((p.position[0] - bb_min[0]) / bb_range[0] * grid_size as f32) as i32)
                .clamp(0, grid_size - 1);
            let gy = (((p.position[1] - bb_min[1]) / bb_range[1] * grid_size as f32) as i32)
                .clamp(0, grid_size - 1);
            let gz = (((p.position[2] - bb_min[2]) / bb_range[2] * grid_size as f32) as i32)
                .clamp(0, grid_size - 1);
            let idx = (gx * grid_size * grid_size + gy * grid_size + gz) as usize;
            let m = p.mass();
            cell_mass[idx] += m;
            cell_pos[idx][0] += p.position[0] as f64 * m as f64;
            cell_pos[idx][1] += p.position[1] as f64 * m as f64;
            cell_pos[idx][2] += p.position[2] as f64 * m as f64;
        }

        // Finalize center-of-mass
        for i in 0..total_cells {
            if cell_mass[i] > 0.0 {
                let m = cell_mass[i] as f64;
                cell_pos[i][0] /= m;
                cell_pos[i][1] /= m;
                cell_pos[i][2] /= m;
            }
        }

        // --- Near-field: spatial hash for K-nearest neighbor direct gravity ---
        // Cell size chosen so average cell has ~24 particles (for ~100K alive)
        let alive_count = self.particles.iter().filter(|p| p.is_alive()).count();
        let spatial_cell_size = if alive_count > 0 {
            let avg_range = (bb_range[0] + bb_range[1] + bb_range[2]) / 3.0;
            // Target ~24 particles per cell: cells³ ≈ alive/24
            let cells_per_dim = ((alive_count as f32 / 24.0).cbrt()).max(1.0);
            avg_range / cells_per_dim
        } else {
            1.0
        };
        let spatial_hash = SpatialHash::build(&self.particles, spatial_cell_size);

        // --- Pre-compute near-field neighbor lists ---
        // (need immutable borrow for particles, then mutable for updates)
        let neighbor_lists: Vec<(usize, Vec<usize>, [f32; 3])> = self
            .particles
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_alive())
            .map(|(i, p)| {
                let pos = p.pos();
                let neighbors =
                    spatial_hash.nearest_neighbors(pos, i, &self.particles, NEAR_FIELD_K);
                (i, neighbors, pos)
            })
            .collect();

        // Pre-compute near-field accelerations
        let near_accels: Vec<(usize, [f32; 3])> = neighbor_lists
            .iter()
            .map(|(i, neighbors, pos)| {
                let acc = near_field_gravity(*pos, neighbors, &self.particles, gravity_strength);
                (*i, acc)
            })
            .collect();

        let softening = 0.5f32;

        // --- Update each particle with combined near + far gravity ---
        // Build a map from particle index to near-field acceleration
        let mut near_acc_map = vec![[0.0f32; 3]; self.particles.len()];
        for (idx, acc) in near_accels {
            near_acc_map[idx] = acc;
        }

        for (pi, p) in self.particles.iter_mut().enumerate() {
            if !p.is_alive() {
                continue;
            }

            // Near-field: direct gravity from K nearest (butterfly effect)
            let mut ax = near_acc_map[pi][0];
            let mut ay = near_acc_map[pi][1];
            let mut az = near_acc_map[pi][2];

            // Far-field: grid cell centers-of-mass
            for ci in 0..total_cells {
                if cell_mass[ci] < 0.001 {
                    continue;
                }
                let cx = cell_pos[ci][0] as f32;
                let cy = cell_pos[ci][1] as f32;
                let cz = cell_pos[ci][2] as f32;

                let dx = cx - p.position[0];
                let dy = cy - p.position[1];
                let dz = cz - p.position[2];
                let r2 = dx * dx + dy * dy + dz * dz + softening * softening;
                let r = r2.sqrt();
                let inv_r3 = 1.0 / (r2 * r);

                let f = gravity_strength * cell_mass[ci] * inv_r3;
                ax += f * dx;
                ay += f * dy;
                az += f * dz;
            }

            p.velocity[0] += ax * sim_dt;
            p.velocity[1] += ay * sim_dt;
            p.velocity[2] += az * sim_dt;

            p.position[0] += p.velocity[0] * sim_dt;
            p.position[1] += p.velocity[1] * sim_dt;
            p.position[2] += p.velocity[2] * sim_dt;

            // Hubble expansion
            p.position[0] += p.position[0] * hubble * sim_dt * 0.001;
            p.position[1] += p.position[1] * hubble * sim_dt * 0.001;
            p.position[2] += p.position[2] * hubble * sim_dt * 0.001;

            // Velocity damping
            let damping = 1.0 - sim_dt * 0.002;
            p.velocity[0] *= damping;
            p.velocity[1] *= damping;
            p.velocity[2] *= damping;

            // Cool down temperature
            p.temperature *= 1.0 - sim_dt * 0.01;
        }
    }

    fn update_phase(&mut self) {
        let new_phase = match self.phase {
            UniversePhase::BigBang if self.age > 0.000001 => Some(UniversePhase::Inflation),
            UniversePhase::Inflation if self.age > 0.00001 => Some(UniversePhase::NuclearEra),
            UniversePhase::NuclearEra if self.age > 0.0004 => Some(UniversePhase::AtomicEra),
            UniversePhase::AtomicEra if self.age > 0.4 => Some(UniversePhase::CosmicDawn),
            UniversePhase::CosmicDawn if self.age > 1.0 => Some(UniversePhase::StellarEra),
            UniversePhase::StellarEra if self.age > 10.0 => Some(UniversePhase::BiologicalEra),
            UniversePhase::BiologicalEra if self.age > 13.0 => {
                Some(UniversePhase::CivilizationEra)
            }
            UniversePhase::CivilizationEra if self.total_entropy > MAX_ENTROPY * 0.9 => {
                Some(UniversePhase::HeatDeath)
            }
            UniversePhase::HeatDeath if self.total_entropy > MAX_ENTROPY => {
                Some(UniversePhase::Collapse)
            }
            _ => None,
        };

        if let Some(phase) = new_phase {
            info!(
                "Universe phase transition: {} -> {} (age: {:.6} Gyr)",
                self.phase.name(),
                phase.name(),
                self.age
            );
            self.phase = phase;
        }
    }

    /// Replace particle vec with new data (lazy loading)
    pub fn replace_particles(&mut self, particles: Vec<GpuParticle>) {
        self.cached_alive_count = particles.len();
        self.particles = particles;
        self.particles_generation = self.particles_generation.wrapping_add(1);
    }

    /// Get the current Hubble parameter
    pub fn hubble(&self) -> f64 {
        spacetime::hubble_parameter(self.age, self.phase)
    }

    /// Particle count (alive) — returns cached value, updated every 30 frames
    pub fn alive_count(&self) -> usize {
        self.cached_alive_count
    }

    /// Find the center of the densest particle cluster using grid-based density estimation
    pub fn find_densest_cluster(&self) -> [f32; 3] {
        if self.particles.is_empty() {
            return [0.0, 0.0, 0.0];
        }

        // Divide space into a coarse grid and count particles per cell
        let grid_size: i32 = 20;
        let mut best_count = 0u32;
        let mut best_center = [0.0f32; 3];

        // Find bounding box
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &self.particles {
            if !p.is_alive() {
                continue;
            }
            for i in 0..3 {
                min[i] = min[i].min(p.position[i]);
                max[i] = max[i].max(p.position[i]);
            }
        }

        let range = [
            (max[0] - min[0]).max(0.001),
            (max[1] - min[1]).max(0.001),
            (max[2] - min[2]).max(0.001),
        ];

        // Count particles per grid cell (use HashMap-like approach with flat array)
        let total_cells = (grid_size * grid_size * grid_size) as usize;
        let mut counts = vec![0u32; total_cells];
        let mut sums = vec![[0.0f64; 3]; total_cells];

        for p in &self.particles {
            if !p.is_alive() {
                continue;
            }
            let gx = (((p.position[0] - min[0]) / range[0] * grid_size as f32) as i32)
                .clamp(0, grid_size - 1);
            let gy = (((p.position[1] - min[1]) / range[1] * grid_size as f32) as i32)
                .clamp(0, grid_size - 1);
            let gz = (((p.position[2] - min[2]) / range[2] * grid_size as f32) as i32)
                .clamp(0, grid_size - 1);
            let idx = (gx * grid_size * grid_size + gy * grid_size + gz) as usize;
            counts[idx] += 1;
            sums[idx][0] += p.position[0] as f64;
            sums[idx][1] += p.position[1] as f64;
            sums[idx][2] += p.position[2] as f64;
        }

        for (i, &count) in counts.iter().enumerate() {
            if count > best_count {
                best_count = count;
                best_center = [
                    (sums[i][0] / count as f64) as f32,
                    (sums[i][1] / count as f64) as f32,
                    (sums[i][2] / count as f64) as f32,
                ];
            }
        }

        best_center
    }

    /// Find the nearest alive particle to a given position
    pub fn find_nearest_particle(&self, pos: [f32; 3]) -> Option<(usize, [f32; 3])> {
        let mut best_dist = f32::MAX;
        let mut best_idx = None;

        for (i, p) in self.particles.iter().enumerate() {
            if !p.is_alive() {
                continue;
            }
            let dx = p.position[0] - pos[0];
            let dy = p.position[1] - pos[1];
            let dz = p.position[2] - pos[2];
            let dist = dx * dx + dy * dy + dz * dz;
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some((i, p.pos()));
            }
        }

        best_idx
    }

    /// Find a random alive particle of a specific type (or any type if None)
    pub fn find_particle_by_kind(&self, kind: Option<u32>) -> Option<(usize, [f32; 3])> {
        let candidates: Vec<(usize, [f32; 3])> = self
            .particles
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.is_alive() && kind.map_or(true, |k| p.kind == k)
            })
            .map(|(i, p)| (i, p.pos()))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Pick one near the middle of the list (deterministic)
        Some(candidates[candidates.len() / 2])
    }
}
