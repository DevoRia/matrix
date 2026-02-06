use matrix_core::*;
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::cosmology;

/// Generate the initial set of universe regions (octree-like subdivision)
pub fn generate_regions(config: &SimConfig, age_gyr: f64) -> Vec<Region> {
    let mut regions = Vec::new();

    // Create a grid of regions covering the observable universe
    // 8x8x8 = 512 regions, each ~100 Mpc across
    let grid = 8i64;
    let region_size = 100.0; // Mpc
    let offset = (grid as f64 * region_size) / 2.0;

    let composition = cosmology::chemical_composition(age_gyr);

    for x in 0..grid {
        for y in 0..grid {
            for z in 0..grid {
                let id = (x * grid * grid + y * grid + z) as u64;
                let seed = config.seed.wrapping_add(id * 7919);
                let mut local_rng = ChaCha8Rng::seed_from_u64(seed);

                // Density fluctuation (cosmic web: filaments, voids, clusters)
                let density = generate_density(&mut local_rng);

                let center = [
                    x as f64 * region_size - offset + region_size / 2.0,
                    y as f64 * region_size - offset + region_size / 2.0,
                    z as f64 * region_size - offset + region_size / 2.0,
                ];

                let volume = region_size.powi(3);
                let star_count = cosmology::estimate_stars(density, volume, age_gyr);

                // Rough planet estimate: ~1-10 planets per star
                let planet_count = (star_count as f64 * local_rng.gen_range(1.0..8.0)) as u64;

                regions.push(Region {
                    id,
                    center,
                    size: region_size,
                    density,
                    temperature: cosmology::cosmic_temperature(age_gyr),
                    composition,
                    dark_matter: config.dark_matter_fraction as f64,
                    star_count,
                    planet_count,
                    has_life: false, // Computed later
                    detail: RegionDetail::Statistical,
                    seed,
                });
            }
        }
    }

    regions
}

/// Generate density fluctuation using simple power spectrum approximation
fn generate_density(rng: &mut impl Rng) -> f64 {
    // Log-normal distribution for cosmic density field
    // Most regions are near average, some are voids, some are clusters
    let normal: f64 = rng.gen_range(-2.0..2.0f64);
    (normal * 0.5).exp() // density ratio: 0.3x to 3x average
}

/// Generate detailed star systems for a region when camera enters
pub fn generate_stellar_detail(region: &Region, age_gyr: f64) -> Vec<Star> {
    let mut rng = ChaCha8Rng::seed_from_u64(region.seed.wrapping_add(1));
    let mut stars = Vec::new();

    // Generate representative stars (max ~1000 for rendering)
    let n = (region.star_count).min(1000) as usize;

    for i in 0..n {
        let star = generate_star(i as u64, region, age_gyr, &mut rng);
        stars.push(star);
    }

    stars
}

fn generate_star(id: u64, region: &Region, age_gyr: f64, rng: &mut impl Rng) -> Star {
    // Position: random within region
    let half = region.size / 2.0;
    let position = [
        region.center[0] + rng.gen_range(-half..half),
        region.center[1] + rng.gen_range(-half..half),
        region.center[2] + rng.gen_range(-half..half),
    ];

    let velocity = [
        rng.gen_range(-100.0..100.0),
        rng.gen_range(-100.0..100.0),
        rng.gen_range(-100.0..100.0),
    ];

    // Initial Mass Function (Kroupa IMF): most stars are low mass
    // P(m) ∝ m^(-2.3) for m > 0.5 M_sun
    let u: f64 = rng.gen_range(0.0..1.0);
    let mass = 0.08 + (1.0 - u).powf(-1.0 / 1.3) * 0.3; // Range: 0.08 to ~50 solar masses
    let mass = mass.min(100.0);

    // Main sequence luminosity: L ∝ M^3.5
    let luminosity = mass.powf(3.5);

    // Surface temperature from mass-luminosity relation
    let surface_temp = 5778.0 * (luminosity / (mass * mass)).powf(0.25);

    let spectral_class = SpectralClass::from_temperature(surface_temp);

    // Star age: random fraction of universe age
    let star_age = rng.gen_range(0.0..age_gyr.max(0.1));

    // Generate planets for this star
    let planet_count = rng.gen_range(0..12);
    let mut planets = Vec::new();
    for j in 0..planet_count {
        planets.push(generate_planet(
            id * 1000 + j,
            luminosity,
            age_gyr,
            j,
            rng,
        ));
    }

    Star {
        id,
        position,
        velocity,
        mass,
        luminosity,
        surface_temp,
        spectral_class,
        age: star_age,
        planets,
    }
}

fn generate_planet(
    id: u64,
    star_luminosity: f64,
    age_gyr: f64,
    orbit_index: u64,
    rng: &mut impl Rng,
) -> Planet {
    // Titius-Bode-like orbital spacing
    let orbital_radius = 0.2 * (1.5f64).powf(orbit_index as f64) + rng.gen_range(-0.1..0.1);
    let orbital_radius = orbital_radius.max(0.05);

    // Kepler's third law: P^2 = a^3 (in AU and years)
    let orbital_period = orbital_radius.powf(1.5);
    let orbital_angle = rng.gen_range(0.0..std::f64::consts::TAU);

    // Planet mass (log-uniform distribution)
    let mass_log: f64 = rng.gen_range(-1.0..3.5); // 0.1 to ~3000 Earth masses
    let mass = 10.0f64.powf(mass_log);

    // Radius from mass (simplified mass-radius relation)
    let radius = if mass < 2.0 {
        mass.powf(0.27) // Rocky
    } else if mass < 100.0 {
        mass.powf(0.06) * 2.0 // Sub-Neptune to Neptune
    } else {
        mass.powf(-0.04) * 11.0 // Gas giant (radius plateaus)
    };

    let surface_temp = cosmology::planet_surface_temp(star_luminosity, orbital_radius);

    // Planet type from mass and temperature
    let planet_type = if mass > 100.0 {
        PlanetType::GasGiant
    } else if mass > 15.0 {
        PlanetType::IceGiant
    } else if surface_temp > 500.0 {
        PlanetType::Lava
    } else if surface_temp < 200.0 {
        PlanetType::Frozen
    } else if mass > 0.5 && rng.gen_bool(0.3) {
        PlanetType::Ocean
    } else {
        PlanetType::Rocky
    };

    // Atmosphere and water
    let has_atmosphere = mass > 0.3 && surface_temp < 2000.0;
    let has_water = has_atmosphere && (240.0..=400.0).contains(&surface_temp);

    let atmosphere = if !has_atmosphere {
        AtmosphereType::None
    } else if mass > 100.0 {
        AtmosphereType::Hydrogen
    } else if has_water {
        if rng.gen_bool(0.3) {
            AtmosphereType::NitrogenOxygen
        } else {
            AtmosphereType::ThinCO2
        }
    } else if surface_temp > 400.0 {
        AtmosphereType::ThickCO2
    } else {
        AtmosphereType::Methane
    };

    // Life — much rarer than before. Requires:
    // 1. Habitable zone (temp, water, atmosphere)
    // 2. Enough time (>1 Gyr minimum for even prokaryotes)
    // 3. Probabilistic abiogenesis (most planets stay sterile)
    let habitable = cosmology::is_habitable(surface_temp, has_water, has_atmosphere);
    let life = if habitable && age_gyr > 1.0 {
        let life_age = (age_gyr - 1.0).max(0.0);
        let p = probability_of_life(surface_temp, has_water, &planet_type, life_age);
        if life_age > 0.0 && rng.gen_bool(p) {
            Some(generate_biosphere(life_age, surface_temp, &planet_type, &atmosphere, rng))
        } else {
            None
        }
    } else {
        None
    };

    Planet {
        id,
        orbital_radius,
        orbital_period,
        orbital_angle,
        mass,
        radius,
        surface_temp,
        has_water,
        has_atmosphere,
        atmosphere,
        planet_type,
        life,
    }
}

/// Probability of life arising — Drake-equation inspired, MUCH rarer than before.
/// On Earth, life appeared after ~0.5 Gyr. But we have n=1.
/// Most habitable planets probably stay sterile.
fn probability_of_life(surface_temp: f64, has_water: bool, planet_type: &PlanetType, life_age_gyr: f64) -> f64 {
    // Without liquid water: extremely unlikely (but not zero — exotic chemistries)
    if !has_water {
        return 1e-6;
    }

    // Base probability: ~10% of habitable planets develop even microbial life
    // (generous end of abiogenesis estimates)
    let mut p = 0.1;

    // Temperature sweet spot: 270-310K optimal, falls off sharply outside
    let temp_factor = (-(surface_temp - 288.0).powi(2) / 800.0).exp();
    p *= temp_factor;

    // Planet type modifier
    p *= match planet_type {
        PlanetType::Rocky => 1.0,     // Best candidate
        PlanetType::Ocean => 0.5,     // Possible but no land for complexity
        PlanetType::Frozen => 0.01,   // Subsurface ocean maybe (Europa)
        _ => 0.001,                   // Gas giants, lava worlds — very unlikely
    };

    // Time factor: more time = slightly better odds (multiple attempts at abiogenesis)
    // But diminishing returns — if it didn't happen in 5 Gyr, probably won't
    let time_factor = (1.0 - (-life_age_gyr * 0.3).exp()).max(0.0);
    p *= time_factor;

    p.clamp(1e-7, 0.15)
}

/// Generate a biosphere — realistic complexity curve based on Earth's timeline.
/// Most biospheres are microbial. Multicellular life is rare. Intelligence is extremely rare.
fn generate_biosphere(
    life_age_gyr: f64,
    surface_temp: f64,
    planet_type: &PlanetType,
    atmosphere: &AtmosphereType,
    rng: &mut impl Rng,
) -> Biosphere {
    // Complexity follows Earth's timeline with probabilistic gates:
    // 0-0.5 Gyr: prebiotic → prokaryotes (complexity 0.0-1.0)
    // 0.5-2 Gyr: prokaryotic diversification (complexity 1.0-2.0)
    // 2-3 Gyr: eukaryotes — requires ~20% chance (complexity 2.0-3.0)
    // 3-4 Gyr: multicellular — requires ~10% chance (complexity 3.0-5.0)
    // 4-5 Gyr: complex animals — requires ~5% chance (complexity 5.0-7.0)
    // 5+ Gyr: intelligence — requires ~1% chance (complexity 7.0-10.0)

    let mut complexity = 0.0;

    // Stage 1: Prokaryotes (guaranteed if life exists)
    if life_age_gyr > 0.0 {
        complexity = (life_age_gyr * 2.0).min(1.0);
    }

    // Stage 2: Prokaryotic diversification
    if life_age_gyr > 0.5 {
        complexity = 1.0 + ((life_age_gyr - 0.5) / 1.5).min(1.0);
    }

    // Stage 3: Eukaryotes — the Great Oxidation Event equivalent
    // On Earth this took ~2 Gyr and may have been a fluke
    if life_age_gyr > 2.0 && rng.gen_bool(0.2) {
        complexity = 2.0 + ((life_age_gyr - 2.0) / 1.0).min(1.0);

        // Stage 4: Multicellular life — another major transition
        if life_age_gyr > 3.0 && rng.gen_bool(0.1) {
            complexity = 3.0 + ((life_age_gyr - 3.0) / 1.0).min(2.0);

            // Stage 5: Complex body plans (Cambrian explosion equivalent)
            if life_age_gyr > 3.5 && rng.gen_bool(0.05) {
                complexity = 5.0 + ((life_age_gyr - 3.5) / 1.5).min(2.0);

                // Stage 6: Intelligence — extremely rare
                if life_age_gyr > 4.5 && rng.gen_bool(0.01) {
                    complexity = 7.0 + ((life_age_gyr - 4.5) / 2.0).min(3.0);
                }
            }
        }
    }

    // Environmental modifiers — harsh environments cap complexity
    let max_complexity = match planet_type {
        PlanetType::Ocean => 6.0,   // No land → hard to develop fire/tools
        PlanetType::Frozen => 2.0,  // Subsurface life stays simple
        _ => 10.0,
    };
    complexity = complexity.min(max_complexity);

    let species_count = if complexity < 1.0 {
        rng.gen_range(1..100) as u64
    } else if complexity < 3.0 {
        rng.gen_range(100..10_000) as u64
    } else if complexity < 5.0 {
        rng.gen_range(10_000..1_000_000) as u64
    } else {
        rng.gen_range(1_000_000..50_000_000) as u64
    };

    let mut genome = Genome::primordial();
    evolve_genome(&mut genome, life_age_gyr, complexity, surface_temp, planet_type, atmosphere, rng);

    let has_technology = genome.cognition > 0.8 && complexity >= 7.0;
    let biomass = complexity.powf(1.5) * rng.gen_range(0.1..5.0);

    Biosphere {
        age: life_age_gyr,
        complexity,
        species_count,
        dominant_genome: genome,
        has_technology,
        biomass,
    }
}

/// Evolve a genome — constrained by environment, complexity, and physics.
/// No magic. No plasma beings on 300K planets. No telekinesis.
/// Structure must follow complexity gates. Senses follow environment.
fn evolve_genome(
    genome: &mut Genome,
    _time_gyr: f64,
    complexity: f64,
    surface_temp: f64,
    planet_type: &PlanetType,
    atmosphere: &AtmosphereType,
    rng: &mut impl Rng,
) {
    // --- SUBSTRATE: determined by planet, not random ---
    genome.substrate = match planet_type {
        PlanetType::Frozen => {
            if surface_temp < 100.0 { 2 } // carbon-methane (Titan-like)
            else { 1 } // carbon-ammonia
        }
        PlanetType::Lava => {
            if rng.gen_bool(0.3) { 3 } // silicon-based (speculative)
            else { 4 } // sulfur-iron (hydrothermal)
        }
        PlanetType::Ocean | PlanetType::Rocky => {
            if surface_temp > 350.0 { 4 } // sulfur-iron at high temp
            else { 0 } // carbon-water (most common)
        }
        _ => 0, // default carbon-water
    };

    // --- STRUCTURE: must follow complexity stages ---
    genome.structure = if complexity < 1.0 {
        0 // single-cell
    } else if complexity < 2.0 {
        rng.gen_range(0..=1) // single-cell or colonial
    } else if complexity < 3.0 {
        rng.gen_range(0..=2) // up to biofilm
    } else if complexity < 5.0 {
        // Multicellular: radial or bilateral or modular
        *[3, 4, 5, 6].choose(rng).unwrap_or(&4)
    } else {
        // Complex: bilateral most likely (like Earth's successful body plan)
        if rng.gen_bool(0.7) { 4 } // bilateral dominates
        else { *[3, 5, 6, 7].choose(rng).unwrap_or(&3) }
    };

    // --- SENSES: constrained by environment and complexity ---
    genome.senses = 4; // chemoreception is universal (even bacteria have it)

    if complexity > 0.5 {
        genome.senses |= 8; // thermoreception — very basic
    }
    if complexity > 1.0 {
        genome.senses |= 2; // mechanoreception (touch/hearing)
    }
    if complexity > 2.0 {
        // Photoreception — only useful where there's light
        match atmosphere {
            AtmosphereType::None | AtmosphereType::ThinCO2 |
            AtmosphereType::NitrogenOxygen | AtmosphereType::ThickCO2 => {
                genome.senses |= 1; // light reaches surface
            }
            _ => {} // thick methane/hydrogen may block light
        }
    }
    if complexity > 3.0 && rng.gen_bool(0.3) {
        genome.senses |= 64; // proprioception (body awareness, multicellular)
    }
    if complexity > 4.0 {
        // Electroreception — mainly aquatic organisms
        if matches!(planet_type, PlanetType::Ocean) || rng.gen_bool(0.15) {
            genome.senses |= 16;
        }
        // Magnetoreception — navigation
        if rng.gen_bool(0.2) {
            genome.senses |= 32;
        }
    }

    // --- SIZE: constrained by complexity and gravity ---
    genome.size_log = if complexity < 1.0 {
        rng.gen_range(-6.0..-4.0) // bacteria: 1μm
    } else if complexity < 2.0 {
        rng.gen_range(-5.0..-3.0) // protists: 10μm-1mm
    } else if complexity < 3.0 {
        rng.gen_range(-4.0..-2.0) // small multicellular
    } else if complexity < 5.0 {
        rng.gen_range(-3.0..0.0) // mm to 1m
    } else if complexity < 7.0 {
        rng.gen_range(-2.0..1.0) // cm to 10m
    } else {
        rng.gen_range(-1.0..1.5) // intelligent: 10cm to ~30m
    };
    // Cap at realistic max (~3km fungal network equivalent)
    genome.size_log = genome.size_log.clamp(-6.0, 2.0);

    // --- ENERGY: constrained by environment ---
    genome.energy_source = if complexity < 1.5 {
        // Early life: chemosynthesis or photosynthesis
        match atmosphere {
            AtmosphereType::None | AtmosphereType::ThinCO2 => {
                if rng.gen_bool(0.5) { 0 } else { 1 } // photo or chemo
            }
            _ => 1, // chemosynthesis in dark/thick atmospheres
        }
    } else if complexity < 3.0 {
        // Diversification of energy strategies
        *[0, 1, 2, 4, 6].choose(rng).unwrap_or(&0) // photo, chemo, geo, ferment, thermo
    } else {
        // Complex organisms: heterotrophy becomes dominant
        if rng.gen_bool(0.6) { 7 } // heterotroph (eats others)
        else if rng.gen_bool(0.5) { 0 } // photosynthetic (plants)
        else { *[1, 2, 5].choose(rng).unwrap_or(&1) }
    };

    // --- COGNITION: requires complexity, nervous system, TIME ---
    // Must have bilateral or radial structure (nervous system prerequisite)
    genome.cognition = if complexity < 2.0 {
        rng.gen_range(0.0..0.05) // bacteria: purely reactive
    } else if complexity < 3.0 {
        rng.gen_range(0.0..0.1) // protists: basic taxis
    } else if complexity < 5.0 && genome.structure >= 3 {
        rng.gen_range(0.1..0.3) // simple nervous system: worm-level
    } else if complexity < 7.0 && genome.structure >= 3 {
        rng.gen_range(0.2..0.6) // complex brain: octopus to crow level
    } else if complexity >= 7.0 && genome.structure >= 3 {
        rng.gen_range(0.5..0.95) // intelligence: dolphin to human+
    } else {
        rng.gen_range(0.0..0.15) // no nervous system possible
    };

    // --- COLLECTIVE: follows complexity ---
    genome.collective = if complexity < 1.0 {
        0.0 // solitary microbes
    } else if complexity < 3.0 {
        rng.gen_range(0.0..0.3) // biofilms, loose colonies
    } else if complexity < 5.0 {
        rng.gen_range(0.0..0.6) // herds, schools
    } else {
        rng.gen_range(0.1..1.0) // full range: solitary predators to eusocial
    };

    // --- MOTILITY: constrained by structure and medium ---
    genome.motility = if complexity < 1.0 {
        *[0, 1, 2].choose(rng).unwrap_or(&0) // sessile, drift, flagellar
    } else if complexity < 3.0 {
        *[0, 1, 2, 3].choose(rng).unwrap_or(&2) // add crawling
    } else if complexity < 5.0 {
        *[0, 3, 4, 5, 6].choose(rng).unwrap_or(&4) // swimming, walking, gliding
    } else {
        // Complex organisms: full range except sessile is rare
        let options = if matches!(planet_type, PlanetType::Ocean) {
            vec![3, 4] // crawling, swimming (no flight underwater)
        } else if matches!(atmosphere, AtmosphereType::None | AtmosphereType::ThinCO2) {
            vec![3, 5, 6] // no flight in thin atmosphere
        } else {
            vec![3, 4, 5, 6, 7] // all options including flight
        };
        *options.choose(rng).unwrap_or(&5)
    };

    // --- INTERFACE: follows complexity and environment ---
    genome.interface = if complexity < 1.0 {
        *[0, 1].choose(rng).unwrap_or(&0) // cell-membrane, cell-wall
    } else if complexity < 3.0 {
        *[0, 1, 5].choose(rng).unwrap_or(&0) // add mucous
    } else if complexity < 5.0 {
        *[2, 3, 4].choose(rng).unwrap_or(&2) // exoskeleton, endoskeleton, shell
    } else {
        // Complex: exo or endoskeleton, with coverings
        if rng.gen_bool(0.4) { 3 } // endoskeleton (vertebrate-like)
        else if rng.gen_bool(0.3) { 2 } // exoskeleton (arthropod-like)
        else { 6 } // fur/feathers/scales
    };

    // --- PROPAGATION: follows complexity ---
    genome.propagation = if complexity < 1.0 {
        0 // binary fission
    } else if complexity < 2.0 {
        *[0, 1, 2].choose(rng).unwrap_or(&0) // fission, budding, spore
    } else if complexity < 3.0 {
        *[2, 3, 5].choose(rng).unwrap_or(&2) // spore, fragmentation, parthenogenesis
    } else {
        // Multicellular: sexual reproduction dominates
        if rng.gen_bool(0.7) { 4 } // sexual (two-parent)
        else { *[2, 5].choose(rng).unwrap_or(&2) }
    };

    // Mutation rate: lower for complex organisms (they have DNA repair)
    genome.mutation_rate = if complexity < 2.0 {
        rng.gen_range(0.05..0.2)
    } else if complexity < 5.0 {
        rng.gen_range(0.01..0.1)
    } else {
        rng.gen_range(0.001..0.05)
    };
}
