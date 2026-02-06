use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::universe::UniverseState;

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
        }
    }
}

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

/// Spawn the 3D camera at the densest region so there's something to see
pub fn spawn_camera(mut commands: Commands, lazy: Res<LazyUniverse>) {
    let start = lazy.find_densest_region().unwrap_or([0.0, 0.0, 0.0]);
    let pos = Vec3::new(start[0] as f32, start[1] as f32 + 10.0, start[2] as f32 + 30.0);
    let look_at = Vec3::new(start[0] as f32, start[1] as f32, start[2] as f32);

    info!("Camera spawned at ({:.0}, {:.0}, {:.0})", pos.x, pos.y, pos.z);

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(pos).looking_at(look_at, Vec3::Y),
        FlyCamera::default(),
    ));

    // Ambient light so planets without emissive are still visible
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.3, 0.3, 0.5),
        brightness: 50.0,
    });
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
