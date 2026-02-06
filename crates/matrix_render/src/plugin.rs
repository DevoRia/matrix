use bevy::prelude::*;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::universe::UniverseState;

use super::camera::{self, FlyCamera};
use super::cosmos;
use super::particles;
use super::ui;

/// Main render plugin for the Matrix simulation
pub struct MatrixRenderPlugin;

impl Plugin for MatrixRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ui::HudThrottle>()
        .add_systems(
            Startup,
            (
                camera::spawn_camera,
                ui::spawn_hud,
                cosmos::init_cosmos_state,
            ),
        )
        .add_systems(
            Startup,
            particles::spawn_particle_visuals.after(camera::spawn_camera),
        )
        .add_systems(
            Update,
            (
                camera::fly_camera_system,
                camera::navigation_system,
                camera::tracking_system.after(camera::navigation_system),
                lazy_universe_lod_tick,
                cosmos::update_cosmos_visuals.after(lazy_universe_lod_tick),
                cosmos::animate_life_planets,
                particles::update_particle_visuals,
                ui::update_hud,
                ui::time_control_system,
            ),
        );
    }
}

/// Update LazyUniverse LOD based on camera position
/// Also toggles particle gravity on/off based on camera distance from origin
fn lazy_universe_lod_tick(
    mut lazy: ResMut<LazyUniverse>,
    mut universe: ResMut<UniverseState>,
    camera_query: Query<&Transform, With<FlyCamera>>,
) {
    let Ok(cam_transform) = camera_query.get_single() else {
        return;
    };
    lazy.update_lod(cam_transform.translation, universe.age);

    // If camera is far from origin (in cosmos mode), disable particle gravity
    let dist_from_origin = cam_transform.translation.length();
    universe.particles_active = dist_from_origin < 200.0;
}
