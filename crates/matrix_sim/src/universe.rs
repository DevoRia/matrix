use bevy::prelude::*;
use matrix_core::{GpuParticle, SimConfig, UniversePhase, MAX_ENTROPY};
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
}

impl UniverseState {
    pub fn new(config: SimConfig, particles: Vec<GpuParticle>) -> Self {
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

        // Throttle gravity: at high time scales, skip most frames
        // time_scale 1 → every frame, 100 → every 5th, 10K+ → every 30th, 1M+ → every 120th
        let gravity_interval = if self.time_scale >= 1_000_000.0 {
            120
        } else if self.time_scale >= 10_000.0 {
            30
        } else if self.time_scale >= 100.0 {
            5
        } else {
            1
        };

        let run_gravity = self.particles_active && (self.gravity_frame % gravity_interval == 0);

        if run_gravity {
            self.tick_particles(effective_dt);
        }

        // These are cheap — always run
        let hubble = spacetime::hubble_parameter(self.age, self.phase) as f64;
        self.scale_factor =
            spacetime::expand_scale_factor(self.scale_factor, hubble, effective_dt);

        // Thermodynamics: only every 30 frames (it iterates all particles)
        if self.gravity_frame % 30 == 0 {
            self.total_entropy = thermodynamics::calculate_entropy(&self.particles);
            self.temperature = thermodynamics::average_temperature(&self.particles);
        }

        // Phase transitions
        self.update_phase();
    }

    /// Heavy particle simulation: grid gravity + integration
    fn tick_particles(&mut self, effective_dt: f64) {
        let sim_dt = effective_dt as f32 * 0.1;
        let hubble = spacetime::hubble_parameter(self.age, self.phase) as f32;

        // --- Grid-based gravity approximation (O(n) instead of O(n^2)) ---
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

        let gravity_strength = self.config.gravity_scale * 0.5;
        let softening = 0.5f32;

        // --- Update each particle ---
        for p in self.particles.iter_mut() {
            if !p.is_alive() {
                continue;
            }

            // Gravity: interact with all grid cell centers-of-mass
            let mut ax = 0.0f32;
            let mut ay = 0.0f32;
            let mut az = 0.0f32;

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

    /// Get the current Hubble parameter
    pub fn hubble(&self) -> f64 {
        spacetime::hubble_parameter(self.age, self.phase)
    }

    /// Particle count (alive)
    pub fn alive_count(&self) -> usize {
        self.particles.iter().filter(|p| p.is_alive()).count()
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
