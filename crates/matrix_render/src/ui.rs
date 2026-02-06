use bevy::prelude::*;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::universe::UniverseState;

/// Marker for the HUD text
#[derive(Component)]
pub struct HudText;

/// Marker for the life details panel (right side)
#[derive(Component)]
pub struct LifePanel;

/// Spawn the HUD overlay
pub fn spawn_hud(mut commands: Commands) {
    // Left panel — universe stats
    commands.spawn((
        Text::new("Matrix Universe"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgba(0.0, 1.0, 0.4, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        HudText,
    ));

    // Right panel — life discoveries
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(0.4, 1.0, 0.6, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            max_width: Val::Px(500.0),
            ..default()
        },
        LifePanel,
    ));
}

/// Format large numbers in human-readable form
fn fmt_count(n: u64) -> String {
    if n >= 1_000_000_000_000 {
        format!("{:.1}T", n as f64 / 1e12)
    } else if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        format!("{}", n)
    }
}

/// HUD frame counter for throttling
#[derive(Resource, Default)]
pub struct HudThrottle {
    pub frame: u32,
}

/// Update HUD text every 10th frame (string formatting is expensive)
pub fn update_hud(
    universe: Res<UniverseState>,
    lazy: Res<LazyUniverse>,
    mut throttle: ResMut<HudThrottle>,
    mut hud_query: Query<&mut Text, (With<HudText>, Without<LifePanel>)>,
    mut life_query: Query<&mut Text, (With<LifePanel>, Without<HudText>)>,
) {
    throttle.frame = throttle.frame.wrapping_add(1);
    if throttle.frame % 10 != 0 {
        return;
    }
    // === LEFT PANEL: Universe stats ===
    if let Ok(mut text) = hud_query.get_single_mut() {
        let paused = if universe.paused { " [PAUSED]" } else { "" };

        let region_info = if let Some(rid) = lazy.current_region_id {
            if let Some(r) = lazy.regions.iter().find(|r| r.id == rid) {
                format!(
                    "Region #{} | Density: {:.2}x | Stars: {} | Loaded: {}",
                    rid,
                    r.density,
                    fmt_count(r.star_count),
                    lazy.loaded_star_count()
                )
            } else {
                "No region".to_string()
            }
        } else {
            "Deep space".to_string()
        };

        **text = format!(
            "MATRIX v0.3 | Cycle: {}\n\
             Phase: {} | Age: {:.6} Gyr\n\
             Scale: {:.4} | Entropy: {:.1}\n\
             Particles: {} | Speed: {:.0}x{}\n\
             \n\
             Regions: {} | Stars: {} | Planets: {}\n\
             {}\n\
             \n\
             [WASD] Move  [RMB+Mouse] Look  [Scroll] Speed\n\
             [Space] Pause  [1-5] Time scale\n\
             [F] Dense region  [L] Find life  [O] Origin",
            universe.cycle,
            universe.phase.name(),
            universe.age,
            universe.scale_factor,
            universe.total_entropy,
            universe.alive_count(),
            universe.time_scale,
            paused,
            lazy.region_count(),
            fmt_count(lazy.total_stars()),
            fmt_count(lazy.total_planets()),
            region_info,
        );
    }

    // === RIGHT PANEL: Life discoveries ===
    if let Ok(mut text) = life_query.get_single_mut() {
        if lazy.life_planets.is_empty() && lazy.loaded_stars.is_empty() {
            **text = String::new();
            return;
        }

        let mut lines = Vec::new();

        // Count life in current loaded stars
        let mut life_here = Vec::new();
        for star in &lazy.loaded_stars {
            for planet in &star.planets {
                if let Some(ref bio) = planet.life {
                    life_here.push((star, planet, bio));
                }
            }
        }

        if !life_here.is_empty() {
            lines.push(format!("=== LIFE IN THIS REGION ({}) ===", life_here.len()));
            lines.push(String::new());

            for (i, (star, planet, bio)) in life_here.iter().enumerate().take(5) {
                let genome = &bio.dominant_genome;
                lines.push(format!("--- Life Form #{} ---", i + 1));
                lines.push(format!("  {}", genome.describe()));
                lines.push(format!(
                    "  Age: {:.1} Gyr | Complexity: {:.1}/10",
                    bio.age, bio.complexity
                ));
                lines.push(format!(
                    "  Species: {} | Biomass: {:.1}",
                    fmt_count(bio.species_count), bio.biomass
                ));
                lines.push(format!(
                    "  Senses: {}",
                    genome.sense_list().join(", ")
                ));
                if bio.has_technology {
                    lines.push("  ** HAS TECHNOLOGY **".to_string());
                }
                lines.push(format!(
                    "  Star: {:.0}K {} | Planet: {} {:.0}K",
                    star.surface_temp,
                    match star.spectral_class {
                        matrix_core::SpectralClass::O => "O-blue",
                        matrix_core::SpectralClass::B => "B-blue",
                        matrix_core::SpectralClass::A => "A-white",
                        matrix_core::SpectralClass::F => "F-yellow",
                        matrix_core::SpectralClass::G => "G-sun",
                        matrix_core::SpectralClass::K => "K-orange",
                        matrix_core::SpectralClass::M => "M-red",
                    },
                    match planet.planet_type {
                        matrix_core::PlanetType::Rocky => "Rocky",
                        matrix_core::PlanetType::GasGiant => "Gas Giant",
                        matrix_core::PlanetType::IceGiant => "Ice Giant",
                        matrix_core::PlanetType::Ocean => "Ocean",
                        matrix_core::PlanetType::Lava => "Lava",
                        matrix_core::PlanetType::Frozen => "Frozen",
                    },
                    planet.surface_temp,
                ));
                lines.push(String::new());
            }
            if life_here.len() > 5 {
                lines.push(format!("  ...and {} more", life_here.len() - 5));
            }
        }

        // Total discoveries
        if !lazy.life_planets.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "TOTAL: {} planets with life | {} civilizations",
                lazy.life_planets.len(),
                lazy.civilization_count
            ));
        }

        **text = lines.join("\n");
    }
}

/// Handle keyboard input for time controls
pub fn time_control_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut universe: ResMut<UniverseState>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        universe.paused = !universe.paused;
    }
    if keyboard.just_pressed(KeyCode::Digit1) {
        universe.time_scale = 1.0;
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        universe.time_scale = 100.0;
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        universe.time_scale = 10_000.0;
    }
    if keyboard.just_pressed(KeyCode::Digit4) {
        universe.time_scale = 1_000_000.0;
    }
    if keyboard.just_pressed(KeyCode::Digit5) {
        universe.time_scale = 1_000_000_000.0;
    }
}
