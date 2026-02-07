use bevy::prelude::*;
use matrix_core::SimConfig;
use matrix_render::menu::{AppState, MenuPlugin};
use matrix_render::plugin::MatrixRenderPlugin;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::pipeline::SimulationPlugin;
use matrix_sim::universe::UniverseState;

fn main() {
    let config = SimConfig::default();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Matrix — Universe Simulation".into(),
                resolution: (1920.0, 1080.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.02)))
        .insert_resource(UniverseState::empty(config.clone()))
        .insert_resource(LazyUniverse::empty(config))
        .init_state::<AppState>()
        .add_plugins(SimulationPlugin)
        .add_plugins(MatrixRenderPlugin)
        .add_plugins(MenuPlugin)
        .run();
}
