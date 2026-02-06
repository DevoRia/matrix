use bevy::prelude::*;
use matrix_core::SpectralClass;
use matrix_sim::lazy_universe::LazyUniverse;

use super::camera::FlyCamera;

/// Marker for star visual entities
#[derive(Component)]
pub struct StarVisual {
    pub star_id: u64,
}

/// Marker for planet visual entities
#[derive(Component)]
pub struct PlanetVisual {
    pub planet_id: u64,
    pub star_id: u64,
    pub has_life: bool,
    pub has_tech: bool,
    pub base_scale: f32,
}

/// Tracks when cosmos visuals were last rebuilt
#[derive(Resource, Default)]
pub struct CosmosRenderState {
    pub stars_generation: u32,
}

/// Scale factor: 1 AU in render units
const AU_RENDER_SCALE: f64 = 2.0;
/// Max stars to render (limit entity count)
const MAX_RENDER_STARS: usize = 200;

/// Spawn cosmos render state resource
pub fn init_cosmos_state(mut commands: Commands) {
    commands.insert_resource(CosmosRenderState::default());
}

/// Sync star/planet visuals with LazyUniverse loaded_stars
pub fn update_cosmos_visuals(
    mut commands: Commands,
    lazy: Res<LazyUniverse>,
    mut state: ResMut<CosmosRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    star_query: Query<Entity, With<StarVisual>>,
    planet_query: Query<Entity, With<PlanetVisual>>,
    camera_query: Query<&Transform, With<FlyCamera>>,
) {
    // Only rebuild when stars actually changed
    if lazy.stars_generation == state.stars_generation {
        return;
    }
    state.stars_generation = lazy.stars_generation;

    // Despawn old visuals
    for entity in star_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in planet_query.iter() {
        commands.entity(entity).despawn();
    }

    if lazy.loaded_stars.is_empty() {
        return;
    }

    let cam_pos = camera_query
        .get_single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    // Sort stars by distance to camera, take nearest MAX_RENDER_STARS
    let mut star_dists: Vec<(usize, f32)> = lazy.loaded_stars.iter().enumerate().map(|(i, s)| {
        let sp = Vec3::new(s.position[0] as f32, s.position[1] as f32, s.position[2] as f32);
        (i, cam_pos.distance_squared(sp))
    }).collect();
    star_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    star_dists.truncate(MAX_RENDER_STARS);

    // Shared meshes — low poly for performance
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let planet_mesh = meshes.add(Sphere::new(1.0).mesh().ico(0).unwrap());

    // Shared materials per spectral class (avoid 1000 unique materials)
    let mut star_mats: [Option<Handle<StandardMaterial>>; 7] = Default::default();

    for (idx, (star_idx, _dist)) in star_dists.iter().enumerate() {
        let star = &lazy.loaded_stars[*star_idx];
        let color = spectral_color(&star.spectral_class);
        let star_radius = (star.luminosity.log10() * 0.5 + 1.0).clamp(0.5, 5.0) as f32;

        // Reuse material per spectral class
        let class_idx = star.spectral_class as usize;
        let star_mat = star_mats[class_idx].get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: color,
                emissive: LinearRgba::from(color) * 10.0,
                unlit: true,
                ..default()
            })
        }).clone();

        let star_pos = Vec3::new(
            star.position[0] as f32,
            star.position[1] as f32,
            star.position[2] as f32,
        );

        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat),
            Transform::from_translation(star_pos).with_scale(Vec3::splat(star_radius)),
            StarVisual { star_id: star.id },
        ));

        // Only 4 nearest stars get point lights (GPU limit)
        if idx < 4 {
            commands.spawn((
                PointLight {
                    color: color,
                    intensity: (star.luminosity as f32).min(100.0) * 20_000.0,
                    range: 25.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(star_pos),
                StarVisual { star_id: star.id },
            ));
        }

        // Planets — only for nearest 50 stars (skip far ones)
        if idx < 50 {
            for planet in &star.planets {
                let has_life = planet.life.is_some();
                let has_tech = planet.life.as_ref().is_some_and(|b| b.has_technology);

                let (planet_color, emissive_mult) = if has_tech {
                    (Color::srgb(1.0, 0.85, 0.0), 20.0)
                } else if has_life {
                    (Color::srgb(0.1, 1.0, 0.3), 15.0)
                } else {
                    (planet_type_color(&planet.planet_type), 3.0)
                };

                let size_mult = if has_tech { 4.0 } else if has_life { 2.5 } else { 1.0 };
                let planet_radius = (planet.radius as f32 * 0.15).clamp(0.15, 1.5) * size_mult;

                let planet_mat = materials.add(StandardMaterial {
                    base_color: planet_color,
                    emissive: LinearRgba::from(planet_color) * emissive_mult,
                    unlit: true, // All unlit for performance
                    ..default()
                });

                let orbit_r = planet.orbital_radius * AU_RENDER_SCALE;
                let px = star_pos.x + (orbit_r * planet.orbital_angle.cos()) as f32;
                let py = star_pos.y;
                let pz = star_pos.z + (orbit_r * planet.orbital_angle.sin()) as f32;

                commands.spawn((
                    Mesh3d(planet_mesh.clone()),
                    MeshMaterial3d(planet_mat),
                    Transform::from_xyz(px, py, pz).with_scale(Vec3::splat(planet_radius)),
                    PlanetVisual {
                        planet_id: planet.id,
                        star_id: star.id,
                        has_life,
                        has_tech,
                        base_scale: planet_radius,
                    },
                ));
            }
        }
    }

    let life_count = lazy.loaded_stars.iter()
        .flat_map(|s| &s.planets)
        .filter(|p| p.life.is_some())
        .count();

    info!(
        "Cosmos: rendered {}/{} stars, {} with life",
        star_dists.len(), lazy.loaded_stars.len(), life_count
    );
}

/// Pulse life planets (stable oscillation using base_scale)
pub fn animate_life_planets(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &PlanetVisual)>,
) {
    let t = time.elapsed_secs();
    for (mut transform, pv) in query.iter_mut() {
        if pv.has_tech {
            let pulse = 1.0 + (t * 3.0).sin() * 0.2;
            transform.scale = Vec3::splat(pv.base_scale * pulse);
        } else if pv.has_life {
            let pulse = 1.0 + (t * 2.0).sin() * 0.1;
            transform.scale = Vec3::splat(pv.base_scale * pulse);
        }
    }
}

fn spectral_color(class: &SpectralClass) -> Color {
    let c = class.color();
    Color::srgba(c[0], c[1], c[2], c[3])
}

fn planet_type_color(pt: &matrix_core::PlanetType) -> Color {
    let c = pt.color();
    Color::srgba(c[0], c[1], c[2], c[3])
}
