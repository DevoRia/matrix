use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::render::view::RenderLayers;
use matrix_core::SerializedParticle;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::universe::UniverseState;
use matrix_storage::UniverseSnapshot;
use std::path::PathBuf;

/// Scale levels for the multi-level zoom system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoomLevel {
    /// >500 units: region outlines, density stats only
    Cosmic,
    /// 100-500: star clusters as points
    Galactic,
    /// 10-100: individual stars + orbits + sparse particles
    Stellar,
    /// 1-10: planet detail, dense particles
    Planetary,
    /// <1: biosphere indicators, full particle detail
    Surface,
}

impl ZoomLevel {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cosmic => "Cosmic",
            Self::Galactic => "Galactic",
            Self::Stellar => "Stellar",
            Self::Planetary => "Planetary",
            Self::Surface => "Surface",
        }
    }

    /// Whether particles should be simulated at this zoom level
    pub fn particles_active(&self) -> bool {
        matches!(self, Self::Planetary | Self::Surface)
    }
}

/// Marker for our free-fly camera
#[derive(Component)]
pub struct FlyCamera {
    pub speed: f32,
    pub sensitivity: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// Index of particle being tracked (None = free fly)
    pub tracking: Option<usize>,
    /// Current particle kind filter for Tab cycling
    pub kind_filter_idx: usize,
    /// Current zoom level (computed from distance to nearest object)
    pub zoom_level: ZoomLevel,
    /// Distance to nearest object (for HUD display)
    pub nearest_dist: f32,
    /// Frame counter for throttling zoom computation
    pub zoom_frame: u32,
    /// Current index for region cycling (G/H keys)
    pub region_nav_idx: usize,
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            speed: 50.0,
            sensitivity: 0.003,
            yaw: 0.0,
            pitch: 0.0,
            tracking: None,
            kind_filter_idx: 0,
            zoom_level: ZoomLevel::Cosmic,
            nearest_dist: 999.0,
            zoom_frame: 0,
            region_nav_idx: 0,
        }
    }
}

/// Marker for the minimap camera
#[derive(Component)]
pub struct MinimapCamera;

/// Marker for the camera position indicator visible on minimap
#[derive(Component)]
pub struct MinimapIndicator;

/// Particle kinds for Tab cycling
const PARTICLE_KINDS: &[(u32, &str)] = &[
    (0, "Up Quark"),
    (1, "Down Quark"),
    (2, "Electron"),
    (4, "Photon"),
    (10, "Proton"),
    (11, "Neutron"),
    (20, "Hydrogen"),
    (21, "Helium"),
    (100, "Dark Matter"),
];

/// Spawn the 3D camera above the origin (Big Bang singularity)
pub fn spawn_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Start above origin at Cosmic distance — Big Bang particles are at (0,0,0)
    let pos = Vec3::new(0.0, 400.0, 500.0);
    let look_at = Vec3::ZERO;

    info!("Camera spawned at ({:.0}, {:.0}, {:.0})", pos.x, pos.y, pos.z);

    commands.spawn((
        Camera3d::default(),
        IsDefaultUiCamera,
        Transform::from_translation(pos).looking_at(look_at, Vec3::Y),
        FlyCamera::default(),
    ));

    // Ambient light so planets without emissive are still visible
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.3, 0.3, 0.5),
        brightness: 50.0,
    });

    // Minimap camera — STATIC orthographic top-down overview (proper scale, no perspective distortion)
    let region_center = Vec3::ZERO;
    let minimap_height = 2000.0;
    commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 800.0 },
            near: 0.1,
            far: 5000.0,
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            order: 1,
            viewport: Some(bevy::render::camera::Viewport {
                physical_position: UVec2::new(0, 0),
                physical_size: UVec2::new(280, 280),
                ..default()
            }),
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.05, 0.8)),
            ..default()
        },
        Transform::from_translation(region_center + Vec3::new(0.0, minimap_height, 0.0))
            .looking_at(region_center, Vec3::Z),
        MinimapCamera,
        RenderLayers::from_layers(&[0, 1]),
    ));

    // Camera position indicator (flat bright rectangle — ONLY visible on minimap via layer 1)
    let indicator_mesh = meshes.add(Cuboid::new(1.0, 0.1, 1.0));
    let indicator_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.0),
        emissive: LinearRgba::from(Color::srgb(1.0, 1.0, 0.0)) * 80.0,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(indicator_mesh),
        MeshMaterial3d(indicator_mat),
        Transform::from_translation(pos).with_scale(Vec3::new(10.0, 1.0, 10.0)),
        MinimapIndicator,
        RenderLayers::layer(1), // only visible to minimap camera
    ));
}

/// Handle camera movement with WASD + mouse
pub fn fly_camera_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&mut Transform, &mut FlyCamera)>,
) {
    let Ok((mut transform, mut cam)) = query.get_single_mut() else {
        return;
    };

    let dt = time.delta_secs();

    // Mouse look (only when right-click held)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        cam.yaw -= delta.x * cam.sensitivity;
        cam.pitch -= delta.y * cam.sensitivity;
        cam.pitch = cam.pitch.clamp(-1.5, 1.5);
    }

    // Apply rotation
    transform.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);

    // Scroll to adjust speed
    let scroll = mouse_scroll.delta.y;
    if scroll != 0.0 {
        cam.speed = (cam.speed * (1.0 + scroll * 0.1)).clamp(1.0, 10000.0);
    }

    // WASD movement (cancels tracking)
    let forward = *transform.forward();
    let right = *transform.right();
    let up = Vec3::Y;

    let mut velocity = Vec3::ZERO;

    if keyboard.pressed(KeyCode::KeyW) {
        velocity += forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        velocity -= forward;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        velocity -= right;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        velocity += right;
    }
    if keyboard.pressed(KeyCode::KeyE) {
        velocity += up;
    }
    if keyboard.pressed(KeyCode::KeyQ) {
        velocity -= up;
    }

    // Boost with shift
    let boost = if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        5.0
    } else {
        1.0
    };

    if velocity.length_squared() > 0.0 {
        velocity = velocity.normalize();
        transform.translation += velocity * cam.speed * boost * dt;
        // Cancel tracking if manually moving
        cam.tracking = None;
    }
}

/// Handle navigation hotkeys (teleport, track, search)
pub fn navigation_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    universe: Res<UniverseState>,
    lazy: Res<LazyUniverse>,
    mut query: Query<(&mut Transform, &mut FlyCamera)>,
) {
    let Ok((mut transform, mut cam)) = query.get_single_mut() else {
        return;
    };

    // [O] Origin — teleport to center
    if keyboard.just_pressed(KeyCode::KeyO) {
        transform.translation = Vec3::new(0.0, 5.0, 50.0);
        cam.tracking = None;
        info!("Camera: teleported to origin");
    }

    // [F] Find densest cluster — teleport there
    if keyboard.just_pressed(KeyCode::KeyF) {
        // Try region-based dense cluster first, fallback to particle-based
        if let Some(center) = lazy.find_densest_region() {
            transform.translation =
                Vec3::new(center[0] as f32, center[1] as f32 + 20.0, center[2] as f32 + 50.0);
            cam.tracking = None;
            info!(
                "Camera: teleported to densest region at ({:.0}, {:.0}, {:.0})",
                center[0], center[1], center[2]
            );
        } else {
            let center = universe.find_densest_cluster();
            transform.translation = Vec3::new(center[0], center[1] + 2.0, center[2] + 10.0);
            cam.tracking = None;
        }
    }

    // [N] Nearest particle — jump to closest
    if keyboard.just_pressed(KeyCode::KeyN) {
        let cam_pos = [
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
        ];
        if let Some((_idx, pos)) = universe.find_nearest_particle(cam_pos) {
            transform.translation = Vec3::new(pos[0], pos[1] + 0.5, pos[2] + 2.0);
            info!(
                "Camera: jumped to nearest particle at ({:.1}, {:.1}, {:.1})",
                pos[0], pos[1], pos[2]
            );
        }
    }

    // [T] Track — follow a random particle
    if keyboard.just_pressed(KeyCode::KeyT) {
        if cam.tracking.is_some() {
            cam.tracking = None;
            info!("Camera: stopped tracking");
        } else if let Some((idx, pos)) = universe.find_particle_by_kind(None) {
            cam.tracking = Some(idx);
            transform.translation = Vec3::new(pos[0], pos[1] + 1.0, pos[2] + 5.0);
            info!("Camera: tracking particle #{}", idx);
        }
    }

    // [Tab] Cycle through particle types and jump to one
    if keyboard.just_pressed(KeyCode::Tab) {
        cam.kind_filter_idx = (cam.kind_filter_idx + 1) % PARTICLE_KINDS.len();
        let (kind, name) = PARTICLE_KINDS[cam.kind_filter_idx];
        if let Some((idx, pos)) = universe.find_particle_by_kind(Some(kind)) {
            transform.translation = Vec3::new(pos[0], pos[1] + 1.0, pos[2] + 5.0);
            cam.tracking = Some(idx);
            info!("Camera: found {} (particle #{})", name, idx);
        } else {
            info!("Camera: no {} particles found", name);
        }
    }

    // [G] Next region — cycle forward through regions
    if keyboard.just_pressed(KeyCode::KeyG) {
        if !lazy.regions.is_empty() {
            cam.region_nav_idx = (cam.region_nav_idx + 1) % lazy.regions.len();
            let r = &lazy.regions[cam.region_nav_idx];
            transform.translation = Vec3::new(
                r.center[0] as f32,
                r.center[1] as f32 + 20.0,
                r.center[2] as f32 + 50.0,
            );
            cam.tracking = None;
            info!(
                "Camera: region #{} ({}/{}) density={:.2} stars={}",
                r.id,
                cam.region_nav_idx + 1,
                lazy.regions.len(),
                r.density,
                r.star_count
            );
        }
    }

    // [H] Previous region — cycle backward
    if keyboard.just_pressed(KeyCode::KeyH) {
        if !lazy.regions.is_empty() {
            if cam.region_nav_idx == 0 {
                cam.region_nav_idx = lazy.regions.len() - 1;
            } else {
                cam.region_nav_idx -= 1;
            }
            let r = &lazy.regions[cam.region_nav_idx];
            transform.translation = Vec3::new(
                r.center[0] as f32,
                r.center[1] as f32 + 20.0,
                r.center[2] as f32 + 50.0,
            );
            cam.tracking = None;
            info!(
                "Camera: region #{} ({}/{}) density={:.2} stars={}",
                r.id,
                cam.region_nav_idx + 1,
                lazy.regions.len(),
                r.density,
                r.star_count
            );
        }
    }

    // [P] Go to coordinates — reads from goto.txt (format: "x y z")
    if keyboard.just_pressed(KeyCode::KeyP) {
        if let Ok(content) = std::fs::read_to_string("goto.txt") {
            let parts: Vec<f32> = content
                .trim()
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() >= 3 {
                transform.translation = Vec3::new(parts[0], parts[1], parts[2]);
                cam.tracking = None;
                info!(
                    "Camera: teleported to ({:.1}, {:.1}, {:.1}) from goto.txt",
                    parts[0], parts[1], parts[2]
                );
            } else {
                info!("Camera: goto.txt should contain 'x y z' (e.g. '150 -40 200')");
            }
        } else {
            info!("Camera: no goto.txt found. Create file with 'x y z' coordinates");
        }
    }

    // [-] Zoom out — stay within current level (no level transition)
    if keyboard.just_pressed(KeyCode::Minus) {
        let old_pos = transform.translation;
        let dir = transform.forward().as_vec3();
        let jump = match cam.zoom_level {
            ZoomLevel::Surface | ZoomLevel::Planetary => 2.0,
            ZoomLevel::Stellar => 15.0,
            ZoomLevel::Galactic => 60.0,
            ZoomLevel::Cosmic => 200.0,
        };
        transform.translation -= dir * jump;
        // Prevent crossing level boundary — undo if level would change
        let new_dist = nearest_dist_from(&transform.translation, &lazy);
        if dist_to_level(new_dist) != cam.zoom_level {
            transform.translation = old_pos;
        }
        cam.tracking = None;
    }

    // [=] Zoom in — stay within current level (no level transition)
    if keyboard.just_pressed(KeyCode::Equal) {
        let old_pos = transform.translation;
        let dir = transform.forward().as_vec3();
        let jump = match cam.zoom_level {
            ZoomLevel::Cosmic => 60.0,
            ZoomLevel::Galactic => 15.0,
            ZoomLevel::Stellar => 2.0,
            ZoomLevel::Planetary => 0.5,
            ZoomLevel::Surface => 0.1,
        };
        transform.translation += dir * jump;
        // Prevent crossing level boundary — undo if level would change
        let new_dist = nearest_dist_from(&transform.translation, &lazy);
        if dist_to_level(new_dist) != cam.zoom_level {
            transform.translation = old_pos;
        }
        cam.tracking = None;
    }

    // [L] Find life — teleport to a planet with life
    if keyboard.just_pressed(KeyCode::KeyL) {
        if let Some(pos) = lazy.find_life() {
            transform.translation =
                Vec3::new(pos[0] as f32, pos[1] as f32 + 2.0, pos[2] as f32 + 10.0);
            cam.tracking = None;
            info!("Camera: teleported to life at ({:.1}, {:.1}, {:.1})", pos[0], pos[1], pos[2]);
        } else {
            info!("Camera: no life found yet (try exploring more regions or speeding up time)");
        }
    }

}

/// If tracking a particle, follow it smoothly
pub fn tracking_system(
    universe: Res<UniverseState>,
    mut query: Query<(&mut Transform, &mut FlyCamera)>,
) {
    let Ok((mut transform, mut cam)) = query.get_single_mut() else {
        return;
    };

    if let Some(idx) = cam.tracking {
        if idx < universe.particles.len() && universe.particles[idx].is_alive() {
            let p = &universe.particles[idx];
            let target = Vec3::new(p.position[0], p.position[1] + 1.0, p.position[2] + 5.0);
            // Smooth follow
            transform.translation = transform.translation.lerp(target, 0.1);
        } else {
            cam.tracking = None;
        }
    }
}

/// Compute nearest distance from a position to origin and loaded stars
fn nearest_dist_from(pos: &Vec3, lazy: &LazyUniverse) -> f32 {
    let mut min_dist = pos.length();
    for star in &lazy.loaded_stars {
        let sp = Vec3::new(
            star.position[0] as f32,
            star.position[1] as f32,
            star.position[2] as f32,
        );
        let d = pos.distance(sp);
        if d < min_dist {
            min_dist = d;
        }
    }
    min_dist
}

/// Map distance to zoom level
fn dist_to_level(dist: f32) -> ZoomLevel {
    if dist > 500.0 {
        ZoomLevel::Cosmic
    } else if dist > 100.0 {
        ZoomLevel::Galactic
    } else if dist > 10.0 {
        ZoomLevel::Stellar
    } else if dist > 1.0 {
        ZoomLevel::Planetary
    } else {
        ZoomLevel::Surface
    }
}

/// Update nearest_dist for HUD display. Does NOT change zoom_level.
/// Zoom level changes ONLY via explicit B (enter) / Esc (exit) actions.
/// Throttled: only recomputes every 15 frames.
pub fn zoom_update_system(
    lazy: Res<LazyUniverse>,
    mut query: Query<(&Transform, &mut FlyCamera)>,
) {
    let Ok((transform, mut cam)) = query.get_single_mut() else {
        return;
    };

    cam.zoom_frame = cam.zoom_frame.wrapping_add(1);
    if cam.zoom_frame % 15 != 0 {
        return;
    }

    let cam_pos = transform.translation;
    let mut min_dist = cam_pos.length();

    for star in &lazy.loaded_stars {
        let sp = Vec3::new(
            star.position[0] as f32,
            star.position[1] as f32,
            star.position[2] as f32,
        );
        let d = cam_pos.distance(sp);
        if d < min_dist {
            min_dist = d;
        }
    }

    cam.nearest_dist = min_dist;
    // zoom_level is NOT auto-changed — only set by B/Esc level transitions
}

/// Update minimap: STATIC camera above region center, indicator rectangle follows player
pub fn minimap_system(
    main_cam_q: Query<(&Transform, &FlyCamera), (Without<MinimapCamera>, Without<MinimapIndicator>)>,
    mut mini_cam_q: Query<
        (&mut Transform, &mut Camera),
        (With<MinimapCamera>, Without<MinimapIndicator>, Without<FlyCamera>),
    >,
    mut indicator_q: Query<
        &mut Transform,
        (With<MinimapIndicator>, Without<MinimapCamera>, Without<FlyCamera>),
    >,
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    surface: Res<super::surface::SurfaceState>,
    lazy: Res<LazyUniverse>,
) {
    let Ok((main_tf, main_cam)) = main_cam_q.get_single() else {
        return;
    };
    let Ok((mut mini_tf, mut mini_camera)) = mini_cam_q.get_single_mut() else {
        return;
    };

    // Hide minimap + indicator on surface
    if surface.active {
        mini_camera.is_active = false;
        if let Ok(mut ind_tf) = indicator_q.get_single_mut() {
            ind_tf.scale = Vec3::ZERO;
        }
        return;
    }
    mini_camera.is_active = true;

    // STATIC: reposition minimap camera above current region center (only moves on region change)
    let minimap_height = 2000.0;
    if let Some(rid) = lazy.current_region_id {
        if let Some(region) = lazy.regions.iter().find(|r| r.id == rid) {
            let rc = Vec3::new(
                region.center[0] as f32,
                region.center[1] as f32,
                region.center[2] as f32,
            );
            mini_tf.translation = rc + Vec3::new(0.0, minimap_height, 0.0);
            mini_tf.look_at(rc, Vec3::Z);
        }
    }

    // Move indicator rectangle to main camera position
    if let Ok(mut ind_tf) = indicator_q.get_single_mut() {
        ind_tf.translation = Vec3::new(
            main_tf.translation.x,
            main_tf.translation.y + 2.0,
            main_tf.translation.z,
        );
        // Scale indicator based on zoom level (represents visible area)
        let size = match main_cam.zoom_level {
            ZoomLevel::Surface | ZoomLevel::Planetary => 5.0,
            ZoomLevel::Stellar => 15.0,
            ZoomLevel::Galactic => 40.0,
            ZoomLevel::Cosmic => 80.0,
        };
        ind_tf.scale = Vec3::new(size, 1.0, size);
    }

    // Update viewport position to bottom-right
    if let Ok(window) = window_q.get_single() {
        let w = window.physical_width();
        let h = window.physical_height();
        let size = 280u32.min(w / 3).min(h / 3);
        let margin = 10u32;
        if let Some(ref mut vp) = mini_camera.viewport {
            vp.physical_position = UVec2::new(w - size - margin, h - size - margin);
            vp.physical_size = UVec2::new(size, size);
        }
    }
}

/// Get the saves directory path
fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

/// Handle F5 (save) / F9 (load) snapshot hotkeys
pub fn snapshot_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut universe: ResMut<UniverseState>,
    mut lazy: ResMut<LazyUniverse>,
) {
    // F5 — Save snapshot
    if keyboard.just_pressed(KeyCode::F5) {
        let snapshot = UniverseSnapshot {
            age: universe.age,
            scale_factor: universe.scale_factor,
            phase: universe.phase,
            cycle: universe.cycle,
            temperature: universe.temperature,
            total_entropy: universe.total_entropy,
            config: universe.config.clone(),
            particles: universe.particles.iter().map(SerializedParticle::from).collect(),
            regions: lazy.regions.clone(),
            current_region_id: lazy.current_region_id,
            loaded_stars: lazy.loaded_stars.clone(),
            life_planets: lazy.life_planets.clone(),
            civilization_count: lazy.civilization_count,
            time_scale: universe.time_scale,
            paused: universe.paused,
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = saves_dir().join(format!("snapshot_{timestamp}.bin"));

        match matrix_storage::save_snapshot(&snapshot, &path) {
            Ok(()) => info!("Snapshot saved: {}", path.display()),
            Err(e) => error!("Failed to save snapshot: {e}"),
        }
    }

    // F9 — Load latest snapshot
    if keyboard.just_pressed(KeyCode::F9) {
        let dir = saves_dir();
        let latest = std::fs::read_dir(&dir)
            .ok()
            .and_then(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map_or(false, |ext| ext == "bin")
                    })
                    .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                    .map(|e| e.path())
            });

        let Some(path) = latest else {
            warn!("No snapshots found in {}", dir.display());
            return;
        };

        match matrix_storage::load_snapshot(&path) {
            Ok(snapshot) => {
                universe.age = snapshot.age;
                universe.scale_factor = snapshot.scale_factor;
                universe.phase = snapshot.phase;
                universe.cycle = snapshot.cycle;
                universe.temperature = snapshot.temperature;
                universe.total_entropy = snapshot.total_entropy;
                universe.config = snapshot.config;
                universe.particles = snapshot.particles.iter().map(|p| p.into()).collect();
                universe.time_scale = snapshot.time_scale;
                universe.paused = snapshot.paused;

                lazy.regions = snapshot.regions;
                lazy.current_region_id = snapshot.current_region_id;
                lazy.loaded_stars = snapshot.loaded_stars;
                lazy.life_planets = snapshot.life_planets;
                lazy.civilization_count = snapshot.civilization_count;
                lazy.stars_generation = lazy.stars_generation.wrapping_add(1);
                lazy.particles_generation = lazy.particles_generation.wrapping_add(1);
                universe.cached_alive_count = universe.particles.len();
                universe.particles_generation = universe.particles_generation.wrapping_add(1);

                info!("Snapshot loaded: {} (age: {:.4} Gyr)", path.display(), snapshot.age);
            }
            Err(e) => error!("Failed to load snapshot: {e}"),
        }
    }
}
