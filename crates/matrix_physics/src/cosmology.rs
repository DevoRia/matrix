use matrix_core::UniversePhase;

/// Friedmann equation: compute scale factor a(t) for a flat universe
/// with matter + dark energy (Lambda-CDM model simplified)
/// Returns scale factor relative to present (a=1 at t=13.8 Gyr)
pub fn scale_factor(age_gyr: f64) -> f64 {
    // Simplified: a(t) ~ t^(2/3) for matter-dominated, exponential for dark energy
    let t_present = 13.8;
    if age_gyr <= 0.0 {
        return 0.001;
    }
    if age_gyr < t_present {
        // Matter-dominated era: a ∝ t^(2/3)
        (age_gyr / t_present).powf(2.0 / 3.0)
    } else {
        // Dark energy dominated: exponential expansion
        let t_excess = age_gyr - t_present;
        (t_excess * 0.07).exp() // Hubble constant ~70 km/s/Mpc
    }
}

/// Cosmic temperature as function of age (CMB temperature evolution)
/// T(t) = T_0 / a(t) where T_0 = 2.725 K today
pub fn cosmic_temperature(age_gyr: f64) -> f64 {
    let a = scale_factor(age_gyr);
    if a < 1e-10 {
        return 1e12; // Quark-gluon plasma temperature
    }
    2.725 / a
}

/// Star formation rate density (Madau & Dickinson 2014, simplified)
/// Returns solar masses per year per Mpc^3
pub fn star_formation_rate(age_gyr: f64) -> f64 {
    // Peak star formation at z~2 (age ~3.3 Gyr), then decline
    if age_gyr < 0.4 {
        return 0.0; // No stars before Cosmic Dawn
    }
    let peak_age = 3.3;
    let rate = if age_gyr < peak_age {
        // Rising phase
        0.15 * (age_gyr / peak_age).powf(2.5)
    } else {
        // Declining phase
        0.15 * (-0.12 * (age_gyr - peak_age)).exp()
    };
    rate.max(0.0)
}

/// Nucleosynthesis: compute chemical composition fractions as function of age
/// Returns [hydrogen_fraction, helium_fraction, metals_fraction]
pub fn chemical_composition(age_gyr: f64) -> [f64; 3] {
    // Big Bang nucleosynthesis: ~75% H, 25% He, trace Li
    // Stars gradually convert H->He->metals over time
    let base_h = 0.75;
    let base_he = 0.25;

    // Metallicity increases with time (roughly linear in log)
    let metals = if age_gyr < 0.4 {
        0.0 // No stars yet
    } else {
        // Z increases from 0 to ~0.02 (solar) over 13 Gyr
        0.02 * ((age_gyr - 0.4) / 13.0).min(1.0)
    };

    let h = base_h - metals * 0.6;
    let he = base_he - metals * 0.4;
    [h, he, metals]
}

/// Estimate number of stars in a region based on density and age
pub fn estimate_stars(density_ratio: f64, region_volume_mpc3: f64, age_gyr: f64) -> u64 {
    // Integrate star formation rate over time, scaled by density
    // SFR gives solar masses per year per Mpc^3
    // Multiply by age in years to get total stellar mass formed per Mpc^3
    // Divide by ~average star mass (~1 solar mass) to get star count
    let sfr = star_formation_rate(age_gyr);
    let stars_per_mpc3 = sfr * age_gyr * 1e9 * density_ratio;
    let n = stars_per_mpc3 * region_volume_mpc3;
    n.max(0.0) as u64
}

/// Determine current universe phase from age
pub fn phase_from_age(age_gyr: f64) -> UniversePhase {
    if age_gyr < 1e-9 {
        UniversePhase::BigBang
    } else if age_gyr < 1e-6 {
        UniversePhase::Inflation
    } else if age_gyr < 0.001 {
        UniversePhase::NuclearEra
    } else if age_gyr < 0.38 {
        UniversePhase::AtomicEra
    } else if age_gyr < 1.0 {
        UniversePhase::CosmicDawn
    } else if age_gyr < 10.0 {
        UniversePhase::StellarEra
    } else if age_gyr < 13.0 {
        UniversePhase::BiologicalEra
    } else {
        UniversePhase::CivilizationEra
    }
}

/// Check if a planet has conditions for life (habitable zone)
pub fn is_habitable(surface_temp_k: f64, has_water: bool, has_atmosphere: bool) -> bool {
    // Liquid water range: ~273K - 373K (but with pressure it can vary)
    let temp_ok = (200.0..=400.0).contains(&surface_temp_k);
    temp_ok && has_water && has_atmosphere
}

/// Estimate surface temperature of a planet from star luminosity and orbital radius
pub fn planet_surface_temp(star_luminosity_solar: f64, orbital_radius_au: f64) -> f64 {
    // Stefan-Boltzmann: T = 278 * (L/L_sun)^0.25 / sqrt(d/AU)
    let r = orbital_radius_au.max(0.01);
    278.0 * star_luminosity_solar.powf(0.25) / r.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_factor_monotonic() {
        let a1 = scale_factor(1.0);
        let a2 = scale_factor(5.0);
        let a3 = scale_factor(13.8);
        assert!(a1 < a2);
        assert!(a2 < a3);
        // In our simplified model, a(13.8) ≈ 1.0
        assert!(a3 > 0.5 && a3 < 2.0, "a(13.8) = {}", a3);
    }

    #[test]
    fn test_temperature_decreasing() {
        let t1 = cosmic_temperature(0.001);
        let t2 = cosmic_temperature(1.0);
        let t3 = cosmic_temperature(13.8);
        assert!(t1 > t2);
        assert!(t2 > t3);
        // Simplified model: T should be low at present
        assert!(t3 < 20.0, "T(13.8) = {}", t3);
    }

    #[test]
    fn test_composition_sums_to_one() {
        for age in [0.0, 1.0, 5.0, 10.0, 13.8] {
            let c = chemical_composition(age);
            let sum = c[0] + c[1] + c[2];
            assert!((sum - 1.0).abs() < 0.01, "age={}: sum={}", age, sum);
        }
    }

    #[test]
    fn test_habitable_zone() {
        // Earth-like: 1 solar luminosity, 1 AU
        let temp = planet_surface_temp(1.0, 1.0);
        assert!((temp - 278.0).abs() < 5.0);
        assert!(is_habitable(temp, true, true));

        // Mercury-like: too hot
        let temp_merc = planet_surface_temp(1.0, 0.39);
        assert!(temp_merc > 400.0);

        // Neptune-like: too cold
        let temp_nep = planet_surface_temp(1.0, 30.0);
        assert!(temp_nep < 100.0);
    }
}
