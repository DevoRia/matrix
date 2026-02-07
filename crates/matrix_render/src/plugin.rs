use bevy::prelude::*;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::state::AppState;
use matrix_sim::universe::UniverseState;

use super::camera::{self, FlyCamera};
use super::cosmos;
use super::particles;
use super::surface;
use super::ui;

/// Main render plugin for the Matrix simulation
pub struct MatrixRenderPlugin;

impl Plugin for MatrixRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ui::HudThrottle>()
        .init_resource::<surface::SurfaceState>()
        .init_resource::<surface::DetailState>()
        .init_resource::<surface::NearestCreatureInfo>()
        .add_systems(
            Startup,
            (
                camera::spawn_camera,
                ui::spawn_hud,
                cosmos::init_cosmos_state,
                particles::init_particle_cloud,
                surface::init_planet_selection,
            ),
        )
        // Space-mode + always-active systems (only in Running state)
        .add_systems(
            Update,
            (
                surface::surface_toggle_system,
                surface::surface_enter_exit_system.after(surface::surface_toggle_system),
                ui::update_hud,
                ui::time_control_system,
                camera::snapshot_system,
                camera::minimap_system,

                camera::fly_camera_system.run_if(surface::not_on_surface),
                camera::navigation_system.run_if(surface::not_on_surface),
                camera::tracking_system
                    .run_if(surface::not_on_surface)
                    .after(camera::navigation_system),
                camera::zoom_update_system
                    .run_if(surface::not_on_surface)
                    .after(camera::tracking_system),
                lazy_universe_lod_tick
                    .run_if(surface::not_on_surface)
                    .after(camera::zoom_update_system),
                cosmos::update_region_visuals
                    .run_if(surface::not_on_surface)
                    .after(camera::zoom_update_system),
                cosmos::update_cosmos_visuals
                    .run_if(surface::not_on_surface)
                    .after(lazy_universe_lod_tick),
            )
                .run_if(in_state(AppState::Running)),
        )
        // Cosmos visuals + particles + surface systems (only in Running state)
        .add_systems(
            Update,
            (
                cosmos::animate_life_planets
                    .run_if(surface::not_on_surface),
                particles::sync_particle_clouds
                    .run_if(surface::not_on_surface),
                particles::update_particle_clouds
                    .run_if(surface::not_on_surface)
                    .after(particles::sync_particle_clouds),
                surface::planet_hover_system
                    .run_if(surface::not_on_surface),
                surface::region_hover_system
                    .run_if(surface::not_on_surface),

                surface::surface_camera_system
                    .run_if(surface::on_surface),
                surface::creature_behavior_system
                    .run_if(surface::on_surface),
                surface::surface_detail_system
                    .run_if(surface::on_surface),
                surface::surface_microbe_system
                    .run_if(surface::on_surface),
                surface::creature_proximity_system
                    .run_if(surface::on_surface)
                    .after(surface::creature_behavior_system),
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

/// Update LazyUniverse LOD based on camera position.
/// Syncs particles from lazy→universe when generation changes.
/// During early universe (Big Bang): particles always active, region particles don't replace Big Bang.
fn lazy_universe_lod_tick(
    mut lazy: ResMut<LazyUniverse>,
    mut universe: ResMut<UniverseState>,
    camera_query: Query<(&Transform, &FlyCamera)>,
) {
    let Ok((cam_transform, cam)) = camera_query.get_single() else {
        return;
    };
    // During Big Bang / early universe: particles always visible, skip region LOD entirely
    let big_bang_phase = universe.age < 1.0;

    // Only run LOD (region loading) after Stellar Era — no regions during Big Bang
    if !big_bang_phase {
        lazy.update_lod(cam_transform.translation, universe.age);
    }
    let was_active = universe.particles_active;
    universe.particles_active = cam.zoom_level.particles_active() || big_bang_phase;

    // Sync region particles only after Stellar Era begins (don't overwrite Big Bang particles)
    if !big_bang_phase
        && lazy.particles_generation != universe.particles_generation
        && !lazy.loaded_particles.is_empty()
        && universe.particles_active
    {
        universe.replace_particles(lazy.loaded_particles.clone());
    }

    // On zoom-out clear: only after Big Bang phase
    if !big_bang_phase && was_active && !universe.particles_active && !universe.particles.is_empty()
    {
        universe.replace_particles(Vec::new());
    }
}
