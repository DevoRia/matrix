use bevy::prelude::*;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::universe::UniverseState;

use super::camera::FlyCamera;
use super::surface::{NearestCreatureInfo, PlanetSelection, SurfaceState, SurfaceZoom};

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
    surface: Res<SurfaceState>,
    selection: Res<PlanetSelection>,
    nearest_creature: Res<NearestCreatureInfo>,
    mut throttle: ResMut<HudThrottle>,
    mut hud_query: Query<&mut Text, (With<HudText>, Without<LifePanel>)>,
    mut life_query: Query<&mut Text, (With<LifePanel>, Without<HudText>)>,
    cam_query: Query<(&Transform, &FlyCamera)>,
) {
    throttle.frame = throttle.frame.wrapping_add(1);
    if throttle.frame % 10 != 0 {
        return;
    }

    let cam_pos = cam_query
        .get_single()
        .map(|(t, _)| t.translation)
        .unwrap_or(Vec3::ZERO);

    // === SURFACE MODE HUD ===
    if surface.active {
        if let Ok(mut text) = hud_query.get_single_mut() {
            if let Some(ref planet) = surface.planet {
                let planet_name = format!("{:?}", planet.planet_type);
                let life_str = if let Some(ref bio) = planet.life {
                    format!(
                        "Complexity: {:.1}/10 | Species: {} | Biomass: {:.1}",
                        bio.complexity,
                        fmt_count(bio.species_count),
                        bio.biomass,
                    )
                } else {
                    "No life detected".to_string()
                };

                let genome_str = if let Some(ref bio) = planet.life {
                    bio.dominant_genome.describe()
                } else {
                    String::new()
                };

                let tech_str = planet
                    .life
                    .as_ref()
                    .is_some_and(|b| b.has_technology)
                    .then_some("** TECHNOLOGICAL CIVILIZATION **")
                    .unwrap_or("");

                let zoom_name = surface.surface_zoom.name();
                let micro_banner = if surface.surface_zoom == SurfaceZoom::Microscopic {
                    "\n** MICROSCOPIC VIEW **"
                } else {
                    ""
                };

                let creature_str = if !nearest_creature.description.is_empty()
                    && nearest_creature.distance < 5.0
                {
                    format!("\nNearest creature ({:.1}m): {}", nearest_creature.distance, nearest_creature.description)
                } else {
                    String::new()
                };

                **text = format!(
                    "SURFACE VIEW | {} planet\n\
                     Temp: {:.0}K | Atmosphere: {:?}\n\
                     Water: {} | Radius: {:.1} Earth\n\
                     Zoom: {} | Height: {:.2}m{}\n\
                     \n\
                     {}\n\
                     {}\n\
                     {}{}\n\
                     \n\
                     Pos: ({:.1}, {:.1}, {:.1})\n\
                     Age: {:.6} Gyr | Speed: {:.0}x\n\
                     \n\
                     === NAVIGATION ===\n\
                     [WASD] Walk  [Mouse] Look  [Shift] Sprint\n\
                     [Scroll] Zoom height\n\
                     [Esc] or [B] Return to space\n\
                     [Space] Pause  [1-5] Time",
                    planet_name,
                    planet.surface_temp,
                    planet.atmosphere,
                    if planet.has_water { "Yes" } else { "No" },
                    planet.radius,
                    zoom_name,
                    surface.eye_height,
                    micro_banner,
                    life_str,
                    genome_str,
                    tech_str,
                    creature_str,
                    cam_pos.x,
                    cam_pos.y,
                    cam_pos.z,
                    universe.age,
                    universe.time_scale,
                );
            }
        }

        // Right panel in surface mode — life info + creature proximity
        if let Ok(mut text) = life_query.get_single_mut() {
            let mut lines = Vec::new();

            if let Some(ref planet) = surface.planet {
                if let Some(ref bio) = planet.life {
                    let genome = &bio.dominant_genome;
                    lines.push("=== LIFE ON THIS PLANET ===".to_string());
                    lines.push(String::new());
                    lines.push(genome.describe());
                    lines.push(format!("Senses: {}", genome.sense_list().join(", ")));
                    lines.push(format!("Age: {:.1} Gyr | Complexity: {:.1}/10", bio.age, bio.complexity));
                    lines.push(format!("Species: {} | Biomass: {:.1}", fmt_count(bio.species_count), bio.biomass));
                    if bio.has_technology {
                        lines.push("** TECHNOLOGICAL CIVILIZATION **".to_string());
                    }
                }
            }

            // Creature proximity detail
            if !nearest_creature.description.is_empty() && nearest_creature.distance < 5.0 {
                lines.push(String::new());
                lines.push("=== NEARBY CREATURE ===".to_string());
                lines.push(format!("Distance: {:.1}m", nearest_creature.distance));
                lines.push(nearest_creature.description.clone());
            }

            // Microscopic hint
            if surface.surface_zoom == SurfaceZoom::Microscopic {
                lines.push(String::new());
                lines.push("Observing microscopic life...".to_string());
            }

            **text = lines.join("\n");
        }
        return;
    }

    // === SPACE MODE HUD ===
    let (zoom_name, nearest_dist) = cam_query
        .get_single()
        .map(|(_, c)| (c.zoom_level.name(), c.nearest_dist))
        .unwrap_or(("?", 0.0));

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

        let selection_str = if selection.selected_region.is_some() {
            let rid = selection.selected_region.unwrap();
            if let Some(region) = lazy.regions.iter().find(|r| r.id == rid) {
                format!(
                    "\n[Selected] Region #{} (density: {:.2}x, stars: {}) — [B] to ENTER",
                    rid, region.density, region.star_count
                )
            } else {
                format!("\n[Selected] Region #{} — [B] to ENTER", rid)
            }
        } else if selection.selected_planet.is_some() {
            let (planet, _) = selection.selected_planet.as_ref().unwrap();
            format!(
                "\n[Selected] {:?} {:.0}K — [B] to LAND",
                planet.planet_type, planet.surface_temp,
            )
        } else if selection.hovered_region.is_some() {
            "\n[Hover] Region — click to select".to_string()
        } else if selection.hovered.is_some() {
            "\n[Hover] Planet — click to select".to_string()
        } else {
            String::new()
        };

        let view_mode = match zoom_name {
            "Cosmic" => "** REGIONS (overview) **",
            "Galactic" => "** CLUSTERS + regions **",
            "Stellar" => "STARS + planets",
            "Planetary" => "DETAIL (full)",
            _ => "SURFACE",
        };

        **text = format!(
            "MATRIX v0.3 | Cycle: {}\n\
             Phase: {} | Age: {:.6} Gyr\n\
             Scale: {:.4} | Entropy: {:.1}\n\
             Particles: {} | Speed: {:.0}x{}\n\
             \n\
             === RENDER LEVEL: {} ===\n\
             Zoom: {} | Dist: {:.1}\n\
             Pos: ({:.1}, {:.1}, {:.1})\n\
             \n\
             Regions: {} | Stars: {} | Planets: {}\n\
             {}{}\n\
             \n\
             === NAVIGATION ===\n\
             [WASD] Move  [RMB+Drag] Look  [Scroll] Speed\n\
             [-/=] Zoom in/out\n\
             [LMB] Select  [B] ENTER selected  [Esc] EXIT level\n\
             \n\
             [G/H] Next/Prev region  [F] Densest  [L] Life\n\
             [N] Nearest  [T] Track  [O] Origin\n\
             [Space] Pause  [1-5] Time  [F5/F9] Save/Load",
            universe.cycle,
            universe.phase.name(),
            universe.age,
            universe.scale_factor,
            universe.total_entropy,
            universe.alive_count(),
            universe.time_scale,
            paused,
            zoom_name,
            view_mode,
            nearest_dist,
            cam_pos.x,
            cam_pos.y,
            cam_pos.z,
            lazy.region_count(),
            fmt_count(lazy.total_stars()),
            fmt_count(lazy.total_planets()),
            region_info,
            selection_str,
        );
    }

    // Right panel: clear in space mode (only used in surface mode for creature info)
    if let Ok(mut text) = life_query.get_single_mut() {
        **text = String::new();
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
