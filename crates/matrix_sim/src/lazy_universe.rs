use bevy::prelude::*;
use matrix_core::*;
use matrix_physics::{cosmology, particle, procgen};
use rand::SeedableRng;

/// The LazyUniverse manages the region-based simulation.
/// Regions far from the camera are purely mathematical.
/// Regions near the camera get procedurally generated detail.
#[derive(Resource)]
pub struct LazyUniverse {
    /// All regions of the universe
    pub regions: Vec<Region>,
    /// Stars currently loaded (from detailed regions)
    pub loaded_stars: Vec<Star>,
    /// Current camera position (updated each frame)
    pub camera_pos: [f64; 3],
    /// Which region the camera is currently in
    pub current_region_id: Option<u64>,
    /// Planets with life (discovered so far)
    pub life_planets: Vec<(u64, String)>, // (planet_id, description)
    /// Total count of civilizations discovered
    pub civilization_count: u32,
    /// Configuration
    pub config: SimConfig,
    /// Last age at which region stats were recalculated
    pub last_stats_age: f64,
    /// Last age at which stars were regenerated for the loaded region
    pub last_reload_age: f64,
    /// Frame counter for throttling LOD updates
    pub lod_frame: u32,
    /// Incremented each time loaded_stars changes (cosmos renderer uses this)
    pub stars_generation: u32,
    /// Particles currently loaded for the active region
    pub loaded_particles: Vec<matrix_core::GpuParticle>,
    /// Incremented each time loaded_particles changes (particle renderer uses this)
    pub particles_generation: u32,
}

impl LazyUniverse {
    /// Placeholder with no regions (used before world generation completes)
    pub fn empty(config: SimConfig) -> Self {
        Self {
            regions: Vec::new(),
            loaded_stars: Vec::new(),
            camera_pos: [0.0; 3],
            current_region_id: None,
            life_planets: Vec::new(),
            civilization_count: 0,
            config,
            last_stats_age: 0.0,
            last_reload_age: 0.0,
            lod_frame: 0,
            stars_generation: 0,
            loaded_particles: Vec::new(),
            particles_generation: 0,
        }
    }

    pub fn new(config: SimConfig, age_gyr: f64) -> Self {
        let regions = procgen::generate_regions(&config, age_gyr);

        Self {
            regions,
            loaded_stars: Vec::new(),
            camera_pos: [0.0; 3],
            current_region_id: None,
            life_planets: Vec::new(),
            civilization_count: 0,
            config,
            last_stats_age: age_gyr,
            last_reload_age: age_gyr,
            lod_frame: 0,
            stars_generation: 0,
            loaded_particles: Vec::new(),
            particles_generation: 0,
        }
    }

    /// Update the LOD system based on camera position
    pub fn update_lod(&mut self, camera_pos: Vec3, age_gyr: f64) {
        self.lod_frame = self.lod_frame.wrapping_add(1);

        // Only check distances every 5th frame (512 regions × distance calc is not free)
        if self.lod_frame % 5 != 0 {
            return;
        }

        self.camera_pos = [camera_pos.x as f64, camera_pos.y as f64, camera_pos.z as f64];

        // Update region stats (just numbers for HUD) — max once per 2 Gyr, very cheap
        let stats_delta = (age_gyr - self.last_stats_age).abs();
        if stats_delta > 2.0 {
            self.update_region_stats(age_gyr);
            self.last_stats_age = age_gyr;
        }

        let mut closest_id = None;
        let mut closest_dist = f64::MAX;

        for region in &mut self.regions {
            let dx = region.center[0] - self.camera_pos[0];
            let dy = region.center[1] - self.camera_pos[1];
            let dz = region.center[2] - self.camera_pos[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

            let desired = if dist < region.size * 0.5 {
                RegionDetail::Stellar
            } else if dist < region.size * 2.0 {
                RegionDetail::Galactic
            } else {
                RegionDetail::Statistical
            };

            if desired != region.detail {
                region.detail = desired.clone();
            }

            if dist < closest_dist {
                closest_dist = dist;
                closest_id = Some(region.id);
            }
        }

        // Only regenerate stars when camera enters a NEW region
        // Age-based reload: max once per 5 Gyr AND only if >60 real frames passed
        let region_changed = closest_id != self.current_region_id;
        let age_reload_delta = (age_gyr - self.last_reload_age).abs();
        let age_reload_needed = age_reload_delta > 5.0 && closest_id.is_some();

        if region_changed {
            self.current_region_id = closest_id;
        }

        if region_changed || age_reload_needed {
            if let Some(id) = closest_id {
                self.load_region_detail(id, age_gyr);
                self.last_reload_age = age_gyr;
            }
        }
    }

    /// Recalculate region statistics based on current universe age
    fn update_region_stats(&mut self, age_gyr: f64) {
        let composition = cosmology::chemical_composition(age_gyr);
        let temperature = cosmology::cosmic_temperature(age_gyr);

        for region in &mut self.regions {
            let volume = region.size.powi(3);
            region.star_count = cosmology::estimate_stars(region.density, volume, age_gyr);
            region.temperature = temperature;
            region.composition = composition;

            // Rough planet estimate
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(region.seed.wrapping_add(99));
            use rand::Rng;
            region.planet_count =
                (region.star_count as f64 * rng.gen_range(1.0..8.0)) as u64;
        }
    }

    /// Generate detailed stars for a region
    fn load_region_detail(&mut self, region_id: u64, age_gyr: f64) {
        if let Some(region) = self.regions.iter().find(|r| r.id == region_id) {
            info!(
                "Loading detail for region {} (density: {:.2}, stars: {})",
                region_id, region.density, region.star_count
            );

            let stars = procgen::generate_stellar_detail(region, age_gyr);

            // Check for life on planets (deduplicate by planet_id)
            for star in &stars {
                for planet in &star.planets {
                    if let Some(ref bio) = planet.life {
                        // Skip if already discovered
                        if self.life_planets.iter().any(|(id, _)| *id == planet.id) {
                            continue;
                        }

                        let desc = format!(
                            "Planet {} orbiting Star {} — {} (complexity: {:.1}, species: {})",
                            planet.id,
                            star.id,
                            bio.dominant_genome.describe(),
                            bio.complexity,
                            bio.species_count,
                        );
                        info!("LIFE FOUND: {}", desc);
                        self.life_planets.push((planet.id, desc));

                        if bio.has_technology {
                            self.civilization_count += 1;
                            info!(
                                "CIVILIZATION #{} detected! {}",
                                self.civilization_count,
                                bio.dominant_genome.describe()
                            );
                        }
                    }
                }
            }

            self.loaded_stars = stars;
            self.stars_generation = self.stars_generation.wrapping_add(1);

            // Generate particles for this region
            self.loaded_particles = particle::generate_region_particles(region, age_gyr);
            self.particles_generation = self.particles_generation.wrapping_add(1);
            info!(
                "Loaded {} particles for region {}",
                self.loaded_particles.len(),
                region_id
            );
        }
    }

    /// Get total statistics across all regions
    pub fn total_stars(&self) -> u64 {
        self.regions.iter().fold(0u64, |acc, r| acc.saturating_add(r.star_count))
    }

    pub fn total_planets(&self) -> u64 {
        self.regions.iter().fold(0u64, |acc, r| acc.saturating_add(r.planet_count))
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    pub fn loaded_star_count(&self) -> usize {
        self.loaded_stars.len()
    }

    /// Find the nearest region with the highest density (to teleport to)
    pub fn find_densest_region(&self) -> Option<[f64; 3]> {
        self.regions
            .iter()
            .max_by(|a, b| a.density.partial_cmp(&b.density).unwrap())
            .map(|r| r.center)
    }

    /// Find a planet with life
    pub fn find_life(&self) -> Option<[f64; 3]> {
        for star in &self.loaded_stars {
            for planet in &star.planets {
                if planet.life.is_some() {
                    // Compute planet world position from orbit
                    let px = star.position[0]
                        + planet.orbital_radius * planet.orbital_angle.cos();
                    let py = star.position[1];
                    let pz = star.position[2]
                        + planet.orbital_radius * planet.orbital_angle.sin();
                    return Some([px, py, pz]);
                }
            }
        }
        None
    }
}
