use serde::{Deserialize, Serialize};

/// A region of space at cosmological scale.
/// The universe is divided into regions; each has statistical properties
/// computed from equations, not individual particles.
/// Detail is generated procedurally when the camera enters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    /// Unique region ID
    pub id: u64,
    /// Center position in universe coordinates
    pub center: [f64; 3],
    /// Size of this region (cube side length)
    pub size: f64,
    /// Matter density (kg/m^3 equivalent, relative to cosmic average)
    pub density: f64,
    /// Temperature (Kelvin)
    pub temperature: f64,
    /// Chemical composition fractions [H, He, metals]
    pub composition: [f64; 3],
    /// Dark matter density fraction
    pub dark_matter: f64,
    /// Number of stars (estimated from density + star formation rate)
    pub star_count: u64,
    /// Number of planets (estimated)
    pub planet_count: u64,
    /// Whether life conditions are met on any planet
    pub has_life: bool,
    /// Detail level currently loaded
    pub detail: RegionDetail,
    /// Seed for deterministic procedural generation
    pub seed: u64,
}

/// How much detail is loaded for a region
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegionDetail {
    /// Only statistical properties (density, temp, composition)
    /// Used for distant regions — zero CPU cost
    Statistical,
    /// Galaxy-level: ~100 representative mass points for N-body
    Galactic,
    /// Stellar-level: individual star systems with orbital mechanics
    Stellar,
    /// Planetary-level: surface detail, atmosphere, geology
    Planetary,
    /// Biosphere: life simulation, evolution running
    Biosphere,
}

/// A star within a detailed region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Star {
    pub id: u64,
    /// Position relative to region center
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    /// Solar masses
    pub mass: f64,
    /// Solar luminosities
    pub luminosity: f64,
    /// Kelvin
    pub surface_temp: f64,
    /// Spectral class determines color
    pub spectral_class: SpectralClass,
    /// Age in Gyr
    pub age: f64,
    /// Planets orbiting this star
    pub planets: Vec<Planet>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SpectralClass {
    O, // Blue giant, >30000K
    B, // Blue-white, 10000-30000K
    A, // White, 7500-10000K
    F, // Yellow-white, 6000-7500K
    G, // Yellow (like Sun), 5200-6000K
    K, // Orange, 3700-5200K
    M, // Red dwarf, 2400-3700K
}

impl SpectralClass {
    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::O => [0.6, 0.7, 1.0, 1.0],
            Self::B => [0.7, 0.8, 1.0, 1.0],
            Self::A => [0.9, 0.9, 1.0, 1.0],
            Self::F => [1.0, 1.0, 0.9, 1.0],
            Self::G => [1.0, 1.0, 0.7, 1.0],
            Self::K => [1.0, 0.8, 0.5, 1.0],
            Self::M => [1.0, 0.5, 0.3, 1.0],
        }
    }

    pub fn from_temperature(temp: f64) -> Self {
        if temp > 30000.0 {
            Self::O
        } else if temp > 10000.0 {
            Self::B
        } else if temp > 7500.0 {
            Self::A
        } else if temp > 6000.0 {
            Self::F
        } else if temp > 5200.0 {
            Self::G
        } else if temp > 3700.0 {
            Self::K
        } else {
            Self::M
        }
    }
}

/// A planet orbiting a star
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Planet {
    pub id: u64,
    /// Orbital radius in AU
    pub orbital_radius: f64,
    /// Orbital period in years
    pub orbital_period: f64,
    /// Current angle in orbit (radians)
    pub orbital_angle: f64,
    /// Planet mass in Earth masses
    pub mass: f64,
    /// Planet radius in Earth radii
    pub radius: f64,
    /// Surface temperature in Kelvin
    pub surface_temp: f64,
    /// Does it have liquid water?
    pub has_water: bool,
    /// Does it have atmosphere?
    pub has_atmosphere: bool,
    /// Atmosphere composition
    pub atmosphere: AtmosphereType,
    /// Planet type
    pub planet_type: PlanetType,
    /// Life on this planet (if any)
    pub life: Option<Biosphere>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PlanetType {
    Rocky,      // Like Earth, Mars
    GasGiant,   // Like Jupiter
    IceGiant,   // Like Neptune
    Ocean,      // Water world
    Lava,       // Too close to star
    Frozen,     // Too far from star
}

impl PlanetType {
    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::Rocky => [0.6, 0.5, 0.4, 1.0],
            Self::GasGiant => [0.8, 0.7, 0.5, 1.0],
            Self::IceGiant => [0.5, 0.7, 0.9, 1.0],
            Self::Ocean => [0.2, 0.4, 0.9, 1.0],
            Self::Lava => [1.0, 0.3, 0.1, 1.0],
            Self::Frozen => [0.8, 0.9, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AtmosphereType {
    None,
    ThinCO2,       // Mars-like
    ThickCO2,      // Venus-like
    NitrogenOxygen, // Earth-like
    Hydrogen,       // Gas giant
    Methane,        // Titan-like
    Exotic,         // Unknown mix
}

/// Life on a planet — abstract, emergent, NOT human-specific
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biosphere {
    /// How long life has existed (Gyr)
    pub age: f64,
    /// Complexity level (0.0 = molecules, 1.0 = cells, 5.0 = multicellular, 10.0 = intelligent)
    pub complexity: f64,
    /// Number of distinct species
    pub species_count: u64,
    /// Dominant species traits (abstract genome)
    pub dominant_genome: Genome,
    /// Has technology been developed?
    pub has_technology: bool,
    /// Biomass (relative units)
    pub biomass: f64,
}

/// Genome — grounded in real biochemistry and astrobiology.
/// Constrained by planetary environment. No magic.
/// Most life is microbial. Complex life is rare. Intelligence is extremely rare.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genome {
    /// Substrate: biochemical basis (constrained by planet conditions)
    /// 0=carbon-water (Earth-like), 1=carbon-ammonia (cold worlds),
    /// 2=carbon-methane (very cold, Titan-like), 3=silicon-based (hot rocky),
    /// 4=sulfur-iron (hydrothermal/volcanic), 5=hydrocarbon-lipid (oil worlds)
    pub substrate: u32,
    /// Structure: body plan (constrained by complexity)
    /// 0=single-cell, 1=colonial, 2=biofilm/mat, 3=radial (jellyfish-like),
    /// 4=bilateral (worm→fish→mammal), 5=modular (coral/plant),
    /// 6=fractal-branching (tree-like), 7=asymmetric
    pub structure: u32,
    /// Sensory modalities (bitmap):
    /// 1=photoreception(light), 2=mechanoreception(touch/hearing),
    /// 4=chemoreception(smell/taste), 8=thermoreception(heat),
    /// 16=electroreception(electric fields), 32=magnetoreception(navigation),
    /// 64=proprioception(body awareness)
    pub senses: u32,
    /// Size scale (log10 of meters): -6=virus, -5=bacterium, -4=protist,
    /// -3=mm insect, -2=cm, -1=10cm, 0=1m, 1=10m(whale), 2=100m(fungus network)
    /// Max realistic: ~2.0 (largest organism = ~3km fungal network)
    pub size_log: f64,
    /// Energy source (constrained by environment)
    /// 0=photosynthesis, 1=chemosynthesis, 2=geothermal,
    /// 3=radiotrophic(radiation), 4=fermentation, 5=osmotic,
    /// 6=thermosynthesis(temp gradient), 7=heterotrophy(eats others)
    pub energy_source: u32,
    /// Cognition (0.0-1.0): requires multicellularity and nervous system
    /// 0.0=reactive(bacteria), 0.1=taxis, 0.2=learning(worm),
    /// 0.4=memory+problem-solving(octopus), 0.6=tool-use(crow/chimp),
    /// 0.8=language+abstract-thought(human), 0.9+=hypothetical superintelligence
    pub cognition: f64,
    /// Collective: social structure
    /// 0.0=solitary, 0.2=territorial-pairs, 0.4=herds/schools,
    /// 0.6=eusocial(ants/bees), 0.8=cooperative-culture, 1.0=superorganism
    pub collective: f64,
    /// Propagation: reproductive strategy
    /// 0=binary-fission, 1=budding, 2=spore, 3=fragmentation,
    /// 4=sexual(two-parent), 5=parthenogenesis
    pub propagation: u32,
    /// Motility: locomotion mode (constrained by medium)
    /// 0=sessile, 1=passive-drift, 2=flagellar/cilia,
    /// 3=muscular-crawling, 4=swimming, 5=walking/running,
    /// 6=gliding/burrowing, 7=flight
    pub motility: u32,
    /// Interface: outer boundary
    /// 0=cell-membrane, 1=cell-wall, 2=exoskeleton,
    /// 3=endoskeleton, 4=mineralized-shell, 5=mucous,
    /// 6=fur/feathers/scales
    pub interface: u32,
    /// Mutation rate (affects evolution speed)
    pub mutation_rate: f64,
}

impl Genome {
    /// Generate a primordial genome — simplest possible self-replicating structure
    pub fn primordial() -> Self {
        Self {
            substrate: 0,       // Organic molecules (simplest starting point)
            structure: 0,       // Amorphous
            senses: 4,          // Chemical sensing only
            size_log: -5.0,     // Microscopic
            energy_source: 1,   // Chemical
            cognition: 0.0,
            collective: 0.0,
            propagation: 0,     // Simple division
            motility: 0,        // Anchored
            interface: 0,       // Membrane
            mutation_rate: 0.1,
        }
    }

    /// Describe this life form — grounded in real biochemistry
    pub fn describe(&self) -> String {
        let substrate = match self.substrate {
            0 => "carbon-water",
            1 => "carbon-ammonia",
            2 => "carbon-methane",
            3 => "silicon",
            4 => "sulfur-iron",
            5 => "hydrocarbon",
            _ => "carbon-water",
        };

        let form = match self.structure {
            0 => "unicellular",
            1 => "colonial",
            2 => "biofilm",
            3 => "radial",
            4 => "bilateral",
            5 => "modular",
            6 => "branching",
            _ => "asymmetric",
        };

        let scale = if self.size_log < -4.0 {
            "molecular"
        } else if self.size_log < -2.0 {
            "micro"
        } else if self.size_log < 0.0 {
            "meso"
        } else if self.size_log < 2.0 {
            "macro"
        } else {
            "mega"
        };

        let mind = if self.cognition > 0.8 {
            "sapient"
        } else if self.cognition > 0.6 {
            "tool-using"
        } else if self.cognition > 0.4 {
            "problem-solving"
        } else if self.cognition > 0.2 {
            "learning"
        } else if self.cognition > 0.1 {
            "taxis"
        } else {
            "reactive"
        };

        let social = if self.collective > 0.8 {
            "superorganism"
        } else if self.collective > 0.6 {
            "eusocial"
        } else if self.collective > 0.4 {
            "herd"
        } else if self.collective > 0.2 {
            "social"
        } else {
            "solitary"
        };

        let energy = match self.energy_source {
            0 => "photosynthetic",
            1 => "chemosynthetic",
            2 => "geothermal",
            3 => "radiotrophic",
            4 => "fermenter",
            5 => "osmotrophic",
            6 => "thermosynthetic",
            _ => "heterotroph",
        };

        let motion = match self.motility {
            0 => "sessile",
            1 => "drifting",
            2 => "flagellar",
            3 => "crawling",
            4 => "swimming",
            5 => "walking",
            6 => "gliding",
            _ => "flying",
        };

        format!(
            "{} {} {} {} ({}, {}, {})",
            scale, substrate, form, mind, social, energy, motion
        )
    }

    /// Short emoji-free tag for HUD
    pub fn short_desc(&self) -> String {
        let substrate = match self.substrate {
            0 => "C-H2O",
            1 => "C-NH3",
            2 => "C-CH4",
            3 => "Si",
            4 => "S-Fe",
            5 => "HC",
            _ => "C-H2O",
        };
        let mind = if self.cognition > 0.8 {
            "SAPIENT"
        } else if self.cognition > 0.4 {
            "COMPLEX"
        } else {
            "SIMPLE"
        };
        format!("{}-{}", substrate, mind)
    }

    /// Count active sensory modalities
    pub fn sense_count(&self) -> u32 {
        self.senses.count_ones()
    }

    /// List active senses as strings
    pub fn sense_list(&self) -> Vec<&'static str> {
        let mut senses = Vec::new();
        if self.senses & 1 != 0 { senses.push("light"); }
        if self.senses & 2 != 0 { senses.push("touch/hearing"); }
        if self.senses & 4 != 0 { senses.push("smell/taste"); }
        if self.senses & 8 != 0 { senses.push("heat"); }
        if self.senses & 16 != 0 { senses.push("electric"); }
        if self.senses & 32 != 0 { senses.push("magnetic"); }
        if self.senses & 64 != 0 { senses.push("proprioception"); }
        senses
    }
}
