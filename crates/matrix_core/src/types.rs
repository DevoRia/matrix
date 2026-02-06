use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

/// GPU-compatible particle representation
/// Must be repr(C) and Pod for GPU buffer upload
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuParticle {
    /// Position (x, y, z) + mass packed in w
    pub position: [f32; 4],
    /// Velocity (x, y, z) + charge packed in w
    pub velocity: [f32; 4],
    /// Particle type index (see ParticleKind)
    pub kind: u32,
    /// Bitflags: bit 0 = alive, bit 1 = interacting
    pub flags: u32,
    /// Temperature of this particle
    pub temperature: f32,
    /// Padding for 16-byte alignment
    pub _pad: f32,
}

impl GpuParticle {
    pub fn new(pos: [f32; 3], vel: [f32; 3], mass: f32, charge: f32, kind: ParticleKind) -> Self {
        Self {
            position: [pos[0], pos[1], pos[2], mass],
            velocity: [vel[0], vel[1], vel[2], charge],
            kind: kind as u32,
            flags: 1, // alive
            temperature: 1e10, // very hot at Big Bang
            _pad: 0.0,
        }
    }

    pub fn mass(&self) -> f32 {
        self.position[3]
    }

    pub fn pos(&self) -> [f32; 3] {
        [self.position[0], self.position[1], self.position[2]]
    }

    pub fn vel(&self) -> [f32; 3] {
        [self.velocity[0], self.velocity[1], self.velocity[2]]
    }

    pub fn is_alive(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// Types of particles in the simulation
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParticleKind {
    // Quarks
    UpQuark = 0,
    DownQuark = 1,
    // Leptons
    Electron = 2,
    Neutrino = 3,
    // Bosons
    Photon = 4,
    Gluon = 5,
    // Composite
    Proton = 10,
    Neutron = 11,
    // Atoms
    Hydrogen = 20,
    Helium = 21,
    Carbon = 22,
    Nitrogen = 23,
    Oxygen = 24,
    Iron = 25,
    // Cosmic structures
    DarkMatter = 100,
}

impl ParticleKind {
    /// Get the color for rendering this particle type [r, g, b, a]
    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::UpQuark => [1.0, 0.2, 0.2, 1.0],       // Red
            Self::DownQuark => [0.2, 0.2, 1.0, 1.0],      // Blue
            Self::Electron => [0.2, 1.0, 1.0, 1.0],       // Cyan
            Self::Neutrino => [0.5, 0.5, 0.5, 0.3],       // Gray, transparent
            Self::Photon => [1.0, 1.0, 0.8, 0.8],         // Warm white
            Self::Gluon => [0.0, 1.0, 0.0, 0.5],          // Green
            Self::Proton => [1.0, 0.5, 0.2, 1.0],         // Orange
            Self::Neutron => [0.6, 0.6, 0.6, 1.0],        // Gray
            Self::Hydrogen => [1.0, 1.0, 1.0, 1.0],       // White
            Self::Helium => [1.0, 1.0, 0.3, 1.0],         // Yellow
            Self::Carbon => [0.3, 0.3, 0.3, 1.0],         // Dark gray
            Self::Nitrogen => [0.3, 0.3, 1.0, 1.0],       // Blue
            Self::Oxygen => [0.2, 0.6, 1.0, 1.0],         // Light blue
            Self::Iron => [0.7, 0.4, 0.2, 1.0],           // Brown
            Self::DarkMatter => [0.1, 0.0, 0.2, 0.15],    // Very faint purple
        }
    }

    /// Get the relative mass for this particle type
    pub fn default_mass(&self) -> f32 {
        match self {
            Self::UpQuark | Self::DownQuark => 0.003,
            Self::Electron => 0.0005,
            Self::Neutrino | Self::Photon | Self::Gluon => 0.0,
            Self::Proton | Self::Neutron => 1.0,
            Self::Hydrogen => 1.0,
            Self::Helium => 4.0,
            Self::Carbon => 12.0,
            Self::Nitrogen => 14.0,
            Self::Oxygen => 16.0,
            Self::Iron => 56.0,
            Self::DarkMatter => 10.0,
        }
    }
}

/// Universe phase enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UniversePhase {
    BigBang,
    Inflation,
    NuclearEra,
    AtomicEra,
    CosmicDawn,
    StellarEra,
    BiologicalEra,
    CivilizationEra,
    HeatDeath,
    Collapse,
}

impl UniversePhase {
    pub fn name(&self) -> &'static str {
        match self {
            Self::BigBang => "Big Bang",
            Self::Inflation => "Inflation",
            Self::NuclearEra => "Nuclear Era",
            Self::AtomicEra => "Atomic Era",
            Self::CosmicDawn => "Cosmic Dawn",
            Self::StellarEra => "Stellar Era",
            Self::BiologicalEra => "Biological Era",
            Self::CivilizationEra => "Civilization Era",
            Self::HeatDeath => "Heat Death",
            Self::Collapse => "Collapse",
        }
    }
}
