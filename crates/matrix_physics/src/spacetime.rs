use matrix_core::UniversePhase;

/// Hubble parameter as function of universe age (simplified model)
/// Returns expansion rate in simulation units
pub fn hubble_parameter(_age: f64, phase: UniversePhase) -> f64 {
    match phase {
        UniversePhase::BigBang => 100.0,        // Rapid initial expansion
        UniversePhase::Inflation => 1000.0,      // Exponential inflation
        UniversePhase::NuclearEra => 50.0,       // Slowing down
        UniversePhase::AtomicEra => 20.0,
        UniversePhase::CosmicDawn => 10.0,
        UniversePhase::StellarEra => 5.0,
        UniversePhase::BiologicalEra => 3.0,
        UniversePhase::CivilizationEra => 2.0,
        UniversePhase::HeatDeath => 1.0,        // Still expanding but slowly
        UniversePhase::Collapse => -10.0,        // Contracting
    }
}

/// Calculate new scale factor based on Hubble expansion
pub fn expand_scale_factor(current: f64, hubble: f64, dt: f64) -> f64 {
    current * (1.0 + hubble * dt * 0.001)
}
