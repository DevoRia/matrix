use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::window::PrimaryWindow;
use matrix_core::{AtmosphereType, Planet, PlanetType, SpectralClass};
use matrix_sim::lazy_universe::LazyUniverse;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::camera::{FlyCamera, ZoomLevel};
use super::cosmos::{PlanetVisual, RegionVisual, AU_RENDER_SCALE};

// --- Constants ---

const TERRAIN_SIZE: f32 = 200.0;
const TERRAIN_RES: usize = 64;
const WALK_SPEED: f32 = 10.0;
const MAX_CREATURES: usize = 80;
const MAX_DETAIL: usize = 50;
const DETAIL_RANGE: f32 = 30.0;
const DETAIL_RESPAWN_DIST: f32 = 15.0;
const MAX_MICROBES: usize = 30;
const MICROBE_RANGE: f32 = 0.5;

// --- Surface zoom levels ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceZoom {
    Landscape,   // 5.0 - 10.0
    Ground,      // 1.0 - 5.0
    CloseUp,     // 0.3 - 1.0
    Microscopic, // 0.05 - 0.3
}

impl SurfaceZoom {
    pub fn from_height(h: f32) -> Self {
        if h > 5.0 {
            Self::Landscape
        } else if h > 1.0 {
            Self::Ground
        } else if h > 0.3 {
            Self::CloseUp
        } else {
            Self::Microscopic
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Landscape => "LANDSCAPE",
            Self::Ground => "GROUND",
            Self::CloseUp => "CLOSE-UP",
            Self::Microscopic => "MICROSCOPIC",
        }
    }
}

// --- Resources ---

#[derive(Resource)]
pub struct SurfaceState {
    pub active: bool,
    pub planet: Option<Planet>,
    pub star_spectral: Option<SpectralClass>,
    pub space_return_pos: Vec3,
    pub generation: u32,
    pub render_generation: u32,
    pub terrain_seed: u64,
    pub eye_height: f32,
    pub surface_zoom: SurfaceZoom,
}

impl Default for SurfaceState {
    fn default() -> Self {
        Self {
            active: false,
            planet: None,
            star_spectral: None,
            space_return_pos: Vec3::ZERO,
            generation: 0,
            render_generation: 0,
            terrain_seed: 0,
            eye_height: 2.0,
            surface_zoom: SurfaceZoom::Ground,
        }
    }
}

#[derive(Resource)]
pub struct PlanetSelection {
    pub hovered: Option<Entity>,
    pub selected_planet: Option<(Planet, SpectralClass)>,
    pub highlight_material: Handle<StandardMaterial>,
    pub original_materials: Vec<(Entity, Handle<StandardMaterial>)>,
    /// Hovered region entity (at Cosmic/Galactic zoom)
    pub hovered_region: Option<Entity>,
    /// Selected region ID ready for entry with [B]
    pub selected_region: Option<u64>,
}

#[derive(Resource, Default)]
pub struct DetailState {
    pub last_spawn_pos: Vec3,
}

#[derive(Resource, Default)]
pub struct NearestCreatureInfo {
    pub distance: f32,
    pub description: String,
}

// --- Components ---

#[derive(Component)]
pub struct TerrainMesh;

#[derive(Component)]
pub struct WaterPlane;

#[derive(Component)]
pub struct SurfaceLight;

#[derive(Component)]
pub struct Creature {
    pub speed: f32,
    pub wander_target: Vec3,
    pub wander_timer: f32,
    pub is_flying: bool,
}

#[derive(Component)]
pub struct SurfaceDetail;

#[derive(Component)]
pub struct Microbe {
    pub drift_speed: f32,
    pub drift_dir: Vec3,
}

#[derive(Component)]
pub struct SkyDomeStar;

// --- Run conditions ---

pub fn on_surface(state: Res<SurfaceState>) -> bool {
    state.active
}

pub fn not_on_surface(state: Res<SurfaceState>) -> bool {
    !state.active
}

// --- Startup ---

pub fn init_planet_selection(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let highlight_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.6),
        emissive: LinearRgba::from(Color::srgb(1.0, 1.0, 0.6)) * 40.0,
        unlit: true,
        ..default()
    });
    commands.insert_resource(PlanetSelection {
        hovered: None,
        selected_planet: None,
        highlight_material: highlight_mat,
        original_materials: Vec::new(),
        hovered_region: None,
        selected_region: None,
    });
}

// --- Planet hover/selection system (space mode) ---

pub fn planet_hover_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform, &FlyCamera)>,
    planet_q: Query<(Entity, &Transform, &PlanetVisual, &MeshMaterial3d<StandardMaterial>)>,
    mut selection: ResMut<PlanetSelection>,
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    lazy: Res<LazyUniverse>,
) {
    // Only active at Stellar/Planetary zoom (not Cosmic/Galactic)
    if let Ok((_, _, cam)) = camera_q.get_single() {
        if matches!(cam.zoom_level, ZoomLevel::Cosmic | ZoomLevel::Galactic) {
            clear_hover(&mut selection, &mut commands, &planet_q);
            return;
        }
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        // Cursor outside window — clear hover
        clear_hover(&mut selection, &mut commands, &planet_q);
        return;
    };

    let Ok((camera, cam_gtf, _)) = camera_q.get_single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_gtf, cursor_pos) else {
        return;
    };

    // Test all planets
    let mut closest: Option<(Entity, f32)> = None;
    for (entity, transform, _pv, _mat) in planet_q.iter() {
        let radius = transform.scale.x;
        if let Some(t) = ray_sphere_intersect(ray.origin, *ray.direction, transform.translation, radius) {
            if closest.map_or(true, |(_, best_t)| t < best_t) {
                closest = Some((entity, t));
            }
        }
    }

    let new_hovered = closest.map(|(e, _)| e);

    // Handle hover change
    if new_hovered != selection.hovered {
        // Restore old material
        if let Some(old_entity) = selection.hovered {
            if let Some(pos) = selection.original_materials.iter().position(|(e, _)| *e == old_entity) {
                let (_, original_mat) = selection.original_materials.remove(pos);
                if planet_q.get(old_entity).is_ok() {
                    commands.entity(old_entity).insert(MeshMaterial3d(original_mat));
                }
            }
        }
        // Set new highlight
        if let Some(new_entity) = new_hovered {
            if let Ok((_, _, _, current_mat)) = planet_q.get(new_entity) {
                selection.original_materials.push((new_entity, current_mat.0.clone()));
                commands
                    .entity(new_entity)
                    .insert(MeshMaterial3d(selection.highlight_material.clone()));
            }
        }
        selection.hovered = new_hovered;
    }

    // Left-click: select planet
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(hovered_entity) = selection.hovered {
            if let Ok((_, _, pv, _)) = planet_q.get(hovered_entity) {
                // Look up Planet + SpectralClass
                for star in &lazy.loaded_stars {
                    if star.id == pv.star_id {
                        for planet in &star.planets {
                            if planet.id == pv.planet_id {
                                selection.selected_planet =
                                    Some((planet.clone(), star.spectral_class));
                                info!(
                                    "Selected: {:?} planet id={} ({:.0}K)",
                                    planet.planet_type, planet.id, planet.surface_temp
                                );
                                break;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn clear_hover(
    selection: &mut PlanetSelection,
    commands: &mut Commands,
    planet_q: &Query<(Entity, &Transform, &PlanetVisual, &MeshMaterial3d<StandardMaterial>)>,
) {
    if let Some(old_entity) = selection.hovered.take() {
        if let Some(pos) = selection.original_materials.iter().position(|(e, _)| *e == old_entity) {
            let (_, original_mat) = selection.original_materials.remove(pos);
            if planet_q.get(old_entity).is_ok() {
                commands.entity(old_entity).insert(MeshMaterial3d(original_mat));
            }
        }
    }
}

fn ray_sphere_intersect(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let a = dir.dot(dir);
    let b = 2.0 * oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_d = discriminant.sqrt();
    let t1 = (-b - sqrt_d) / (2.0 * a);
    let t2 = (-b + sqrt_d) / (2.0 * a);
    if t1 > 0.0 {
        Some(t1)
    } else if t2 > 0.0 {
        Some(t2)
    } else {
        None
    }
}

// --- Region hover/selection system (space mode, Cosmic/Galactic zoom) ---

pub fn region_hover_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform, &FlyCamera)>,
    region_q: Query<(Entity, &Transform, &RegionVisual, &MeshMaterial3d<StandardMaterial>)>,
    mut selection: ResMut<PlanetSelection>,
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    // Only active at Cosmic/Galactic zoom
    let Ok((camera, cam_gtf, cam)) = camera_q.get_single() else {
        return;
    };
    if !matches!(cam.zoom_level, ZoomLevel::Cosmic | ZoomLevel::Galactic) {
        // Clear region hover when not at right zoom
        if let Some(old_entity) = selection.hovered_region.take() {
            if let Some(pos) = selection
                .original_materials
                .iter()
                .position(|(e, _)| *e == old_entity)
            {
                let (_, original_mat) = selection.original_materials.remove(pos);
                if region_q.get(old_entity).is_ok() {
                    commands
                        .entity(old_entity)
                        .insert(MeshMaterial3d(original_mat));
                }
            }
        }
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_gtf, cursor_pos) else {
        return;
    };

    // Test all region cubes (use sphere intersection with radius = scale)
    let mut closest: Option<(Entity, f32)> = None;
    for (entity, transform, _rv, _mat) in region_q.iter() {
        let radius = transform.scale.x; // cube is uniform scale
        if let Some(t) =
            ray_sphere_intersect(ray.origin, *ray.direction, transform.translation, radius)
        {
            if closest.map_or(true, |(_, best_t)| t < best_t) {
                closest = Some((entity, t));
            }
        }
    }

    let new_hovered = closest.map(|(e, _)| e);

    // Handle hover change
    if new_hovered != selection.hovered_region {
        // Restore old material
        if let Some(old_entity) = selection.hovered_region {
            if let Some(pos) = selection
                .original_materials
                .iter()
                .position(|(e, _)| *e == old_entity)
            {
                let (_, original_mat) = selection.original_materials.remove(pos);
                if region_q.get(old_entity).is_ok() {
                    commands
                        .entity(old_entity)
                        .insert(MeshMaterial3d(original_mat));
                }
            }
        }
        // Set new highlight
        if let Some(new_entity) = new_hovered {
            if let Ok((_, _, _, current_mat)) = region_q.get(new_entity) {
                selection
                    .original_materials
                    .push((new_entity, current_mat.0.clone()));
                commands
                    .entity(new_entity)
                    .insert(MeshMaterial3d(selection.highlight_material.clone()));
            }
        }
        selection.hovered_region = new_hovered;
    }

    // Left-click: select region
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(hovered_entity) = selection.hovered_region {
            if let Ok((_, _, rv, _)) = region_q.get(hovered_entity) {
                selection.selected_region = Some(rv.region_id);
                info!("Selected region #{}", rv.region_id);
            }
        }
    }
}

// --- Surface toggle system ---

/// [B] key: enter region / land on planet / exit surface
/// [Esc] key: exit surface / exit to Cosmic view
pub fn surface_toggle_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SurfaceState>,
    mut selection: ResMut<PlanetSelection>,
    lazy: Res<LazyUniverse>,
    mut camera_query: Query<(&mut Transform, &mut FlyCamera)>,
) {
    let b_pressed = keyboard.just_pressed(KeyCode::KeyB);
    let esc_pressed = keyboard.just_pressed(KeyCode::Escape);

    if !b_pressed && !esc_pressed {
        return;
    }

    // === EXIT SURFACE ===
    if state.active && (b_pressed || esc_pressed) {
        state.active = false;
        state.generation = state.generation.wrapping_add(1);
        info!("Surface: leaving planet");
        return;
    }

    // === ESC: exit to Cosmic view (from any non-Cosmic zoom) ===
    if esc_pressed && !state.active {
        let Ok((mut transform, mut cam)) = camera_query.get_single_mut() else {
            return;
        };
        if !matches!(cam.zoom_level, ZoomLevel::Cosmic) {
            let target = lazy
                .current_region_id
                .and_then(|rid| lazy.regions.iter().find(|r| r.id == rid))
                .map(|r| {
                    Vec3::new(
                        r.center[0] as f32,
                        r.center[1] as f32,
                        r.center[2] as f32,
                    )
                })
                .unwrap_or(transform.translation);
            transform.translation = target + Vec3::new(0.0, 300.0, 600.0);
            cam.zoom_level = ZoomLevel::Cosmic;
            cam.tracking = None;
            info!("Level: exited to Cosmic view");
        }
        return;
    }

    // === B: enter selected region (teleport to region center) ===
    if b_pressed {
        if let Some(region_id) = selection.selected_region.take() {
            if let Some(region) = lazy.regions.iter().find(|r| r.id == region_id) {
                let Ok((mut transform, mut cam)) = camera_query.get_single_mut() else {
                    return;
                };
                let rc = Vec3::new(
                    region.center[0] as f32,
                    region.center[1] as f32,
                    region.center[2] as f32,
                );
                transform.translation = rc + Vec3::new(0.0, 20.0, 50.0);
                cam.zoom_level = ZoomLevel::Stellar;
                cam.tracking = None;
                selection.hovered_region = None;
                selection.original_materials.clear();
                info!(
                    "Level: entered region #{} (density: {:.2}x, stars: {})",
                    region_id, region.density, region.star_count
                );
            }
            return;
        }
    }

    // === B: land on selected planet ===
    if b_pressed {
        let Ok((transform, cam)) = camera_query.get_single_mut() else {
            return;
        };

        let planet_data = selection.selected_planet.take().or_else(|| {
            if matches!(cam.zoom_level, ZoomLevel::Planetary | ZoomLevel::Surface) {
                find_nearest_planet(&lazy, transform.translation)
            } else {
                info!("Select a planet (click) or a region (click) then press [B]");
                None
            }
        });

        if let Some((planet, spectral)) = planet_data {
            info!(
                "Surface: landing on {:?} planet (id={})",
                planet.planet_type, planet.id
            );
            state.space_return_pos = transform.translation;
            state.terrain_seed = planet.id;
            state.star_spectral = Some(spectral);
            state.planet = Some(planet);
            state.active = true;
            state.eye_height = 2.0;
            state.surface_zoom = SurfaceZoom::Ground;
            state.generation = state.generation.wrapping_add(1);

            selection.hovered = None;
            selection.original_materials.clear();
        }
    }
}

// --- Surface enter/exit system ---

pub fn surface_enter_exit_system(
    mut commands: Commands,
    mut state: ResMut<SurfaceState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clear_color: ResMut<ClearColor>,
    mut camera_query: Query<(&mut Transform, &mut FlyCamera)>,
    terrain_q: Query<Entity, With<TerrainMesh>>,
    water_q: Query<Entity, With<WaterPlane>>,
    light_q: Query<Entity, With<SurfaceLight>>,
    creature_q: Query<Entity, With<Creature>>,
    detail_q: Query<Entity, With<SurfaceDetail>>,
    microbe_q: Query<Entity, With<Microbe>>,
    sky_q: Query<Entity, With<SkyDomeStar>>,
) {
    if state.generation == state.render_generation {
        return;
    }
    state.render_generation = state.generation;

    if state.active {
        // === ENTER SURFACE ===
        let Some(ref planet) = state.planet else {
            return;
        };

        // Terrain mesh with vertex-colored biomes
        let terrain_mesh = build_terrain_mesh(state.terrain_seed, &planet.planet_type);
        let terrain_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE, // vertex colors handle coloring
            perceptual_roughness: 0.9,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(terrain_mat),
            Transform::IDENTITY,
            TerrainMesh,
        ));

        // Water plane
        if planet.has_water {
            let water_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.1, 0.3, 0.8, 0.6),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.1,
                ..default()
            });
            let water_mesh = meshes.add(Plane3d::default().mesh().size(TERRAIN_SIZE, TERRAIN_SIZE));
            commands.spawn((
                Mesh3d(water_mesh),
                MeshMaterial3d(water_mat),
                Transform::from_xyz(0.0, -0.5, 0.0),
                WaterPlane,
            ));
        }

        // Sky color
        clear_color.0 = sky_color(&planet.atmosphere);

        // Directional light (sun)
        let sun_color = state
            .star_spectral
            .as_ref()
            .map(|s| {
                let c = s.color();
                Color::srgb(c[0], c[1], c[2])
            })
            .unwrap_or(Color::WHITE);
        commands.spawn((
            DirectionalLight {
                color: sun_color,
                illuminance: 10_000.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.3, 0.0)),
            SurfaceLight,
        ));

        // Ambient light for terrain visibility
        commands.insert_resource(AmbientLight {
            color: sun_color,
            brightness: 300.0,
        });

        // Sky dome: scatter stars across a large sphere
        spawn_sky_dome(&mut commands, &mut meshes, &mut materials, &planet.atmosphere);

        // Creatures
        spawn_creatures(&mut commands, &mut meshes, &mut materials, planet, state.terrain_seed);

        // Teleport camera
        if let Ok((mut transform, mut cam)) = camera_query.get_single_mut() {
            let ground_y = terrain_height(0.0, 0.0, state.terrain_seed, &planet.planet_type);
            transform.translation = Vec3::new(0.0, ground_y + state.eye_height, 0.0);
            cam.yaw = 0.0;
            cam.pitch = 0.0;
            transform.rotation = Quat::IDENTITY;
        }

        let life_str = if planet.life.is_some() {
            "with life"
        } else {
            "barren"
        };
        info!(
            "Surface: spawned {:?} terrain ({}) | water={} | atmo={:?}",
            planet.planet_type, life_str, planet.has_water, planet.atmosphere
        );
    } else {
        // === EXIT SURFACE ===
        for entity in terrain_q.iter() {
            commands.entity(entity).despawn();
        }
        for entity in water_q.iter() {
            commands.entity(entity).despawn();
        }
        for entity in light_q.iter() {
            commands.entity(entity).despawn();
        }
        for entity in creature_q.iter() {
            commands.entity(entity).despawn();
        }
        for entity in detail_q.iter() {
            commands.entity(entity).despawn();
        }
        for entity in microbe_q.iter() {
            commands.entity(entity).despawn();
        }
        for entity in sky_q.iter() {
            commands.entity(entity).despawn();
        }

        // Reset ambient light
        commands.insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 0.0,
        });

        clear_color.0 = Color::srgb(0.0, 0.0, 0.02);

        if let Ok((mut transform, _cam)) = camera_query.get_single_mut() {
            transform.translation = state.space_return_pos;
        }

        state.planet = None;
        state.star_spectral = None;
        state.eye_height = 2.0;
        state.surface_zoom = SurfaceZoom::Ground;
        info!("Surface: returned to space");
    }
}

// --- Surface camera system ---

pub fn surface_camera_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut state: ResMut<SurfaceState>,
    mut query: Query<(&mut Transform, &mut FlyCamera)>,
) {
    let Ok((mut transform, mut cam)) = query.get_single_mut() else {
        return;
    };
    let Some(planet_type) = state.planet.as_ref().map(|p| p.planet_type) else {
        return;
    };
    let terrain_seed = state.terrain_seed;

    let dt = time.delta_secs();

    // Mouse look (always active on surface)
    let delta = mouse_motion.delta;
    if delta.length_squared() > 0.0 {
        cam.yaw -= delta.x * cam.sensitivity;
        cam.pitch -= delta.y * cam.sensitivity;
        cam.pitch = cam.pitch.clamp(-1.5, 1.5);
    }
    transform.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);

    // Scroll wheel adjusts eye height
    let scroll = mouse_scroll.delta.y;
    if scroll != 0.0 {
        let factor = 1.0 - scroll * 0.15;
        state.eye_height = (state.eye_height * factor).clamp(0.05, 10.0);
        let new_zoom = SurfaceZoom::from_height(state.eye_height);
        if new_zoom != state.surface_zoom {
            info!(
                "Surface zoom: {} (height: {:.2})",
                new_zoom.name(),
                state.eye_height
            );
            state.surface_zoom = new_zoom;
        }
    }

    // WASD on XZ plane
    let forward = *transform.forward();
    let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let right_xz = Vec3::new(forward.z, 0.0, -forward.x).normalize_or_zero();

    let mut velocity = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        velocity += forward_xz;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        velocity -= forward_xz;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        velocity -= right_xz;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        velocity += right_xz;
    }

    let boost = if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        3.0
    } else {
        1.0
    };

    // Walk speed scales with height
    let speed_mult = (state.eye_height / 2.0).clamp(0.1, 3.0);

    if velocity.length_squared() > 0.0 {
        velocity = velocity.normalize();
        transform.translation += velocity * WALK_SPEED * speed_mult * boost * dt;
    }

    // Clamp to terrain bounds
    let half = TERRAIN_SIZE / 2.0 * 0.95;
    transform.translation.x = transform.translation.x.clamp(-half, half);
    transform.translation.z = transform.translation.z.clamp(-half, half);

    // Snap to ground + eye height
    let ground_y = terrain_height(
        transform.translation.x,
        transform.translation.z,
        terrain_seed,
        &planet_type,
    );
    transform.translation.y = ground_y + state.eye_height;
}

// --- Creature systems ---

pub fn creature_behavior_system(
    time: Res<Time>,
    state: Res<SurfaceState>,
    mut query: Query<(&mut Transform, &mut Creature)>,
) {
    let Some(ref planet) = state.planet else {
        return;
    };
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    for (mut transform, mut creature) in query.iter_mut() {
        if creature.speed < 0.01 {
            continue;
        }

        creature.wander_timer -= dt;

        let dir = Vec3::new(
            creature.wander_target.x - transform.translation.x,
            0.0,
            creature.wander_target.z - transform.translation.z,
        );
        let dist = dir.length();

        if dist > 1.0 {
            let move_dir = dir.normalize();
            transform.translation.x += move_dir.x * creature.speed * dt;
            transform.translation.z += move_dir.z * creature.speed * dt;

            let y = terrain_height(
                transform.translation.x,
                transform.translation.z,
                state.terrain_seed,
                &planet.planet_type,
            );
            let hover = if creature.is_flying { 3.0 } else { 0.0 };
            transform.translation.y = y + transform.scale.x * 0.5 + hover;
        }

        if dist < 2.0 || creature.wander_timer < 0.0 {
            let hash = ((transform.translation.x * 100.0) as u64)
                .wrapping_mul(((transform.translation.z * 100.0) as u64).wrapping_add(1))
                .wrapping_add(elapsed as u64);
            let mut rng = ChaCha8Rng::seed_from_u64(hash);
            let half = TERRAIN_SIZE / 2.0 * 0.8;
            creature.wander_target =
                Vec3::new(rng.gen_range(-half..half), 0.0, rng.gen_range(-half..half));
            creature.wander_timer = rng.gen_range(3.0..10.0);
        }
    }
}

pub fn creature_proximity_system(
    state: Res<SurfaceState>,
    camera_q: Query<&Transform, With<FlyCamera>>,
    mut creature_q: Query<(&Transform, &mut Creature), Without<FlyCamera>>,
    mut nearest_info: ResMut<NearestCreatureInfo>,
) {
    let Some(ref planet) = state.planet else {
        return;
    };
    let Ok(cam_tf) = camera_q.get_single() else {
        return;
    };

    let mut closest_dist = f32::MAX;

    for (tf, mut creature) in creature_q.iter_mut() {
        let dist = cam_tf.translation.distance(tf.translation);
        if dist < closest_dist {
            closest_dist = dist;
        }
        // Freeze creature when observer is very close
        if dist < 3.0 {
            creature.wander_timer = 5.0;
            creature.wander_target = tf.translation;
        }
    }

    nearest_info.distance = closest_dist;

    if closest_dist < 5.0 {
        if let Some(ref bio) = planet.life {
            nearest_info.description = format!(
                "CREATURE (dist: {:.1}m)\n{}\nSenses: {}",
                closest_dist,
                bio.dominant_genome.describe(),
                bio.dominant_genome.sense_list().join(", ")
            );
        }
    } else {
        nearest_info.description.clear();
    }
}

// --- Detail objects system ---

pub fn surface_detail_system(
    mut commands: Commands,
    state: Res<SurfaceState>,
    mut detail_state: ResMut<DetailState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_q: Query<&Transform, With<FlyCamera>>,
    detail_q: Query<Entity, With<SurfaceDetail>>,
) {
    let Some(ref planet) = state.planet else {
        return;
    };
    let Ok(cam_tf) = camera_q.get_single() else {
        return;
    };

    let cam_pos = cam_tf.translation;

    // Only respawn if camera moved enough
    if cam_pos.distance(detail_state.last_spawn_pos) < DETAIL_RESPAWN_DIST
        && detail_q.iter().count() > 0
    {
        return;
    }
    detail_state.last_spawn_pos = cam_pos;

    // Despawn old
    for entity in detail_q.iter() {
        commands.entity(entity).despawn();
    }

    let (detail_mesh, detail_mat) = match planet.planet_type {
        PlanetType::Rocky => (
            meshes.add(Cuboid::new(0.3, 0.4, 0.3)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.45, 0.38),
                ..default()
            }),
        ),
        PlanetType::Ocean => (
            meshes.add(Cuboid::new(0.15, 0.6, 0.15)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.15, 0.55, 0.1),
                ..default()
            }),
        ),
        PlanetType::Frozen => (
            meshes.add(Sphere::new(0.2).mesh().ico(0).unwrap()),
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.7, 0.85, 1.0, 0.7),
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        ),
        PlanetType::Lava => (
            meshes.add(Sphere::new(0.25).mesh().ico(0).unwrap()),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.3, 0.05),
                emissive: LinearRgba::from(Color::srgb(1.0, 0.4, 0.0)) * 5.0,
                ..default()
            }),
        ),
        _ => return, // no details for gas/ice giants
    };

    let mut rng = ChaCha8Rng::seed_from_u64(
        state
            .terrain_seed
            .wrapping_add((cam_pos.x * 10.0) as u64)
            .wrapping_add((cam_pos.z * 10.0) as u64),
    );

    for _ in 0..MAX_DETAIL {
        let dx = rng.gen_range(-DETAIL_RANGE..DETAIL_RANGE);
        let dz = rng.gen_range(-DETAIL_RANGE..DETAIL_RANGE);
        let x = cam_pos.x + dx;
        let z = cam_pos.z + dz;

        let half = TERRAIN_SIZE / 2.0 * 0.95;
        if x.abs() > half || z.abs() > half {
            continue;
        }

        let y = terrain_height(x, z, state.terrain_seed, &planet.planet_type);
        let scale = rng.gen_range(0.5..1.5);

        commands.spawn((
            Mesh3d(detail_mesh.clone()),
            MeshMaterial3d(detail_mat.clone()),
            Transform::from_xyz(x, y + scale * 0.2, z).with_scale(Vec3::splat(scale)),
            SurfaceDetail,
        ));
    }
}

// --- Microbe system ---

pub fn surface_microbe_system(
    mut commands: Commands,
    state: Res<SurfaceState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    camera_q: Query<&Transform, With<FlyCamera>>,
    mut microbe_q: Query<(Entity, &mut Transform, &Microbe), Without<FlyCamera>>,
) {
    let Some(ref planet) = state.planet else {
        return;
    };

    if state.surface_zoom != SurfaceZoom::Microscopic {
        // Despawn all microbes when not microscopic
        for (entity, _, _) in microbe_q.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let Ok(cam_tf) = camera_q.get_single() else {
        return;
    };
    let cam_pos = cam_tf.translation;
    let dt = time.delta_secs();

    // Move existing + despawn if too far
    let mut count = 0;
    for (entity, mut tf, microbe) in microbe_q.iter_mut() {
        tf.translation += microbe.drift_dir * microbe.drift_speed * dt;
        if tf.translation.distance(cam_pos) > MICROBE_RANGE * 3.0 {
            commands.entity(entity).despawn();
        } else {
            count += 1;
        }
    }

    // Spawn new
    if count < MAX_MICROBES {
        let microbe_mesh = meshes.add(Sphere::new(1.0).mesh().ico(0).unwrap());
        let color = if planet.life.is_some() {
            Color::srgba(0.2, 0.8, 0.3, 0.7)
        } else {
            Color::srgba(0.5, 0.5, 0.6, 0.4)
        };
        let microbe_mat = materials.add(StandardMaterial {
            base_color: color,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        let mut rng = ChaCha8Rng::seed_from_u64(
            state
                .terrain_seed
                .wrapping_add(time.elapsed_secs() as u64)
                .wrapping_add(count as u64),
        );

        let to_spawn = (MAX_MICROBES - count).min(5);
        for _ in 0..to_spawn {
            let offset = Vec3::new(
                rng.gen_range(-MICROBE_RANGE..MICROBE_RANGE),
                rng.gen_range(-MICROBE_RANGE * 0.5..MICROBE_RANGE * 0.5),
                rng.gen_range(-MICROBE_RANGE..MICROBE_RANGE),
            );
            let scale = rng.gen_range(0.002..0.01);
            let drift_dir = Vec3::new(
                rng.gen_range(-1.0..1.0),
                rng.gen_range(-0.5..0.5),
                rng.gen_range(-1.0..1.0),
            )
            .normalize_or_zero();

            commands.spawn((
                Mesh3d(microbe_mesh.clone()),
                MeshMaterial3d(microbe_mat.clone()),
                Transform::from_translation(cam_pos + offset).with_scale(Vec3::splat(scale)),
                Microbe {
                    drift_speed: rng.gen_range(0.01..0.05),
                    drift_dir,
                },
            ));
        }
    }
}

// --- Helpers ---

fn find_nearest_planet(lazy: &LazyUniverse, cam_pos: Vec3) -> Option<(Planet, SpectralClass)> {
    let mut best: Option<(Planet, SpectralClass, f32)> = None;

    for star in &lazy.loaded_stars {
        let star_pos = Vec3::new(
            star.position[0] as f32,
            star.position[1] as f32,
            star.position[2] as f32,
        );
        for planet in &star.planets {
            let orbit_r = planet.orbital_radius * AU_RENDER_SCALE;
            let px = star_pos.x + (orbit_r * planet.orbital_angle.cos()) as f32;
            let py = star_pos.y;
            let pz = star_pos.z + (orbit_r * planet.orbital_angle.sin()) as f32;
            let dist = cam_pos.distance(Vec3::new(px, py, pz));

            let closer = best.as_ref().map_or(true, |(_, _, d)| dist < *d);
            if closer {
                best = Some((planet.clone(), star.spectral_class, dist));
            }
        }
    }

    best.map(|(p, s, _)| (p, s))
}

fn terrain_height(x: f32, z: f32, seed: u64, planet_type: &PlanetType) -> f32 {
    let s = seed as f32 * 0.0001;
    let amplitude = match planet_type {
        PlanetType::Rocky => 20.0,
        PlanetType::Ocean => 6.0,
        PlanetType::Frozen => 12.0,
        PlanetType::Lava => 25.0,
        PlanetType::GasGiant => 2.0,
        PlanetType::IceGiant => 4.0,
    };

    // Domain warping for organic shapes
    let warp_x = (x * 0.02 + s * 0.5).sin() * 5.0;
    let warp_z = (z * 0.03 + s * 0.7).cos() * 5.0;
    let wx = x + warp_x;
    let wz = z + warp_z;

    // 5 octaves
    let h1 = (wx * 0.05 + s).sin() * (wz * 0.07 + s * 1.3).sin() * amplitude;
    let h2 = (wx * 0.13 + s * 2.1).sin() * (wz * 0.11 + s * 0.7).sin() * amplitude * 0.4;
    let h3 = (wx * 0.31 + s * 3.7).sin() * (wz * 0.29 + s * 1.9).sin() * amplitude * 0.15;
    let h4 = (wx * 0.67 + s * 5.3).sin() * (wz * 0.59 + s * 2.3).sin() * amplitude * 0.07;
    let h5 = (wx * 1.31 + s * 7.1).sin() * (wz * 1.19 + s * 3.1).sin() * amplitude * 0.03;

    h1 + h2 + h3 + h4 + h5
}

fn biome_color(height_t: f32, planet_type: &PlanetType) -> [f32; 4] {
    match planet_type {
        PlanetType::Rocky => {
            if height_t < 0.15 {
                [0.76, 0.70, 0.50, 1.0] // shore/sand
            } else if height_t < 0.4 {
                [0.25, 0.50, 0.18, 1.0] // grassland
            } else if height_t < 0.7 {
                [0.18, 0.38, 0.12, 1.0] // forest
            } else if height_t < 0.85 {
                [0.50, 0.45, 0.38, 1.0] // rock
            } else {
                [0.90, 0.92, 0.95, 1.0] // snow
            }
        }
        PlanetType::Frozen => {
            if height_t < 0.3 {
                [0.70, 0.80, 0.90, 1.0]
            } else if height_t < 0.7 {
                [0.80, 0.85, 0.92, 1.0]
            } else {
                [0.95, 0.97, 1.0, 1.0]
            }
        }
        PlanetType::Lava => {
            if height_t < 0.2 {
                [1.0, 0.4, 0.0, 1.0] // lava glow
            } else if height_t < 0.5 {
                [0.25, 0.08, 0.02, 1.0] // dark basalt
            } else {
                [0.35, 0.20, 0.10, 1.0] // cooled rock
            }
        }
        PlanetType::Ocean => {
            if height_t < 0.2 {
                [0.60, 0.58, 0.40, 1.0] // sandy shore
            } else if height_t < 0.6 {
                [0.30, 0.55, 0.25, 1.0] // vegetation
            } else {
                [0.40, 0.50, 0.35, 1.0] // highlands
            }
        }
        PlanetType::GasGiant => [0.70, 0.60, 0.40, 1.0],
        PlanetType::IceGiant => [0.50, 0.60, 0.80, 1.0],
    }
}

fn build_terrain_mesh(seed: u64, planet_type: &PlanetType) -> Mesh {
    let res = TERRAIN_RES;
    let half = TERRAIN_SIZE / 2.0;
    let step = TERRAIN_SIZE / res as f32;

    let vert_count = (res + 1) * (res + 1);
    let mut positions = Vec::with_capacity(vert_count);
    let mut normals = Vec::with_capacity(vert_count);
    let mut uvs = Vec::with_capacity(vert_count);
    let mut heights = Vec::with_capacity(vert_count);

    for zi in 0..=res {
        for xi in 0..=res {
            let x = xi as f32 * step - half;
            let z = zi as f32 * step - half;
            let y = terrain_height(x, z, seed, planet_type);
            positions.push([x, y, z]);
            heights.push(y);
            uvs.push([xi as f32 / res as f32, zi as f32 / res as f32]);

            let dx = terrain_height(x + 0.1, z, seed, planet_type)
                - terrain_height(x - 0.1, z, seed, planet_type);
            let dz = terrain_height(x, z + 0.1, seed, planet_type)
                - terrain_height(x, z - 0.1, seed, planet_type);
            let n = Vec3::new(-dx, 0.2, -dz).normalize();
            normals.push([n.x, n.y, n.z]);
        }
    }

    // Compute height range for biome coloring
    let min_h = heights.iter().cloned().fold(f32::MAX, f32::min);
    let max_h = heights.iter().cloned().fold(f32::MIN, f32::max);
    let range = (max_h - min_h).max(0.01);

    let colors: Vec<[f32; 4]> = heights
        .iter()
        .map(|h| {
            let t = (*h - min_h) / range;
            biome_color(t, planet_type)
        })
        .collect();

    let mut indices: Vec<u32> = Vec::with_capacity(res * res * 6);
    for zi in 0..res {
        for xi in 0..res {
            let tl = (zi * (res + 1) + xi) as u32;
            let tr = tl + 1;
            let bl = tl + (res + 1) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

fn sky_color(atmosphere: &AtmosphereType) -> Color {
    // Twilight/night tones so stars on the sky dome remain visible
    match atmosphere {
        AtmosphereType::NitrogenOxygen => Color::srgb(0.05, 0.07, 0.15),
        AtmosphereType::ThickCO2 => Color::srgb(0.12, 0.08, 0.04),
        AtmosphereType::ThinCO2 => Color::srgb(0.10, 0.06, 0.05),
        AtmosphereType::Hydrogen => Color::srgb(0.08, 0.06, 0.04),
        AtmosphereType::Methane => Color::srgb(0.04, 0.07, 0.08),
        AtmosphereType::Exotic => Color::srgb(0.07, 0.04, 0.09),
        AtmosphereType::None => Color::srgb(0.01, 0.01, 0.03),
    }
}

fn spawn_creatures(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &Planet,
    terrain_seed: u64,
) {
    let Some(ref bio) = planet.life else {
        return;
    };
    let genome = &bio.dominant_genome;

    let count = ((bio.biomass * 5.0) as usize).clamp(5, MAX_CREATURES);

    let creature_mesh = match genome.structure {
        0 | 1 | 2 => meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        3 => meshes.add(Sphere::new(1.0).mesh().ico(0).unwrap()),
        4 => meshes.add(Cuboid::new(0.6, 0.4, 1.0)),
        5 | 6 => meshes.add(Cuboid::new(0.5, 1.5, 0.5)),
        _ => meshes.add(Cuboid::new(0.8, 0.6, 0.7)),
    };

    let creature_color = match genome.substrate {
        0 => Color::srgb(0.2, 0.7, 0.3),
        1 => Color::srgb(0.3, 0.3, 0.7),
        2 => Color::srgb(0.6, 0.4, 0.2),
        3 => Color::srgb(0.5, 0.5, 0.5),
        4 => Color::srgb(0.7, 0.3, 0.1),
        5 => Color::srgb(0.8, 0.7, 0.2),
        _ => Color::srgb(0.5, 0.5, 0.5),
    };

    let creature_mat = materials.add(StandardMaterial {
        base_color: creature_color,
        ..default()
    });

    let scale = 10.0f32.powf(genome.size_log as f32).clamp(0.2, 5.0);

    let speed = match genome.motility {
        0 => 0.0,
        1 => 0.5,
        2 => 1.0,
        3 => 2.0,
        4 => 3.0,
        5 => 4.0,
        6 => 3.5,
        _ => 6.0,
    };

    let is_flying = genome.motility == 7;

    let mut rng = ChaCha8Rng::seed_from_u64(terrain_seed.wrapping_add(777));
    let half = TERRAIN_SIZE / 2.0 * 0.8;

    for _ in 0..count {
        let x = rng.gen_range(-half..half);
        let z = rng.gen_range(-half..half);
        let y = terrain_height(x, z, terrain_seed, &planet.planet_type)
            + scale * 0.5
            + if is_flying { 3.0 } else { 0.0 };

        let wander_x = rng.gen_range(-half..half);
        let wander_z = rng.gen_range(-half..half);

        commands.spawn((
            Mesh3d(creature_mesh.clone()),
            MeshMaterial3d(creature_mat.clone()),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(scale)),
            Creature {
                speed,
                wander_target: Vec3::new(wander_x, 0.0, wander_z),
                wander_timer: rng.gen_range(3.0..10.0),
                is_flying,
            },
        ));
    }

    info!(
        "Surface: spawned {} creatures (structure={}, substrate={}, motility={}, size={:.1})",
        count, genome.structure, genome.substrate, genome.motility, scale
    );
}

fn spawn_sky_dome(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    atmosphere: &AtmosphereType,
) {
    let sky_radius = 500.0;
    let mut rng = ChaCha8Rng::seed_from_u64(42);

    // Atmosphere thickness affects how many stars are visible
    let star_count = match atmosphere {
        AtmosphereType::None => 400,      // No atmosphere — full starfield
        AtmosphereType::ThinCO2 => 300,
        AtmosphereType::NitrogenOxygen => 150, // Earth-like — fewer visible
        AtmosphereType::ThickCO2 => 60,
        AtmosphereType::Hydrogen => 40,
        AtmosphereType::Methane => 80,
        AtmosphereType::Exotic => 120,
    };

    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(0).unwrap());

    // Shared materials for different star colors
    let star_colors = [
        Color::srgb(1.0, 1.0, 1.0),    // white
        Color::srgb(0.8, 0.9, 1.0),    // blue-white
        Color::srgb(1.0, 0.95, 0.8),   // yellow-white
        Color::srgb(1.0, 0.7, 0.5),    // orange
        Color::srgb(0.6, 0.7, 1.0),    // blue
    ];
    let star_mats: Vec<Handle<StandardMaterial>> = star_colors
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                emissive: LinearRgba::from(*c) * 50.0,
                unlit: true,
                ..default()
            })
        })
        .collect();

    for _ in 0..star_count {
        // Random point on upper hemisphere (above horizon)
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.05..std::f32::consts::FRAC_PI_2); // 0 = horizon, PI/2 = zenith
        let x = sky_radius * phi.cos() * theta.cos();
        let z = sky_radius * phi.cos() * theta.sin();
        let y = sky_radius * phi.sin();

        let size = rng.gen_range(0.3..1.5);
        let mat_idx = rng.gen_range(0..star_mats.len());

        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mats[mat_idx].clone()),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(size)),
            SkyDomeStar,
        ));
    }

    // Add a few "bright" stars (larger, more emissive)
    let bright_count = star_count / 10;
    let bright_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::from(Color::WHITE) * 200.0,
        unlit: true,
        ..default()
    });

    for _ in 0..bright_count {
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.1..std::f32::consts::FRAC_PI_2);
        let x = sky_radius * phi.cos() * theta.cos();
        let z = sky_radius * phi.cos() * theta.sin();
        let y = sky_radius * phi.sin();

        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(bright_mat.clone()),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(rng.gen_range(1.5..3.0))),
            SkyDomeStar,
        ));
    }

    info!(
        "Surface: spawned {} sky stars ({} bright), atmo={:?}",
        star_count, bright_count, atmosphere
    );
}
