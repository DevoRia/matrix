use bevy::prelude::*;
use matrix_core::SimConfig;
use matrix_physics::particle::generate_big_bang;
use matrix_render::plugin::MatrixRenderPlugin;
use matrix_sim::lazy_universe::LazyUniverse;
use matrix_sim::pipeline::SimulationPlugin;
use matrix_sim::universe::UniverseState;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn main() {
    let config = SimConfig::default();

    // Generate initial particles deterministically
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let particles = generate_big_bang(&config, &mut rng);

    // Start at 10 Gyr — stars, planets, and life already exist
    let initial_age = 10.0;
    let lazy_universe = LazyUniverse::new(config.clone(), initial_age);

    info!(
        "Matrix initialized: {} particles, {} regions, seed: {}",
        particles.len(),
        lazy_universe.region_count(),
        config.seed
    );

    let mut universe = UniverseState::new(config, particles);
    universe.age = initial_age;

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
        .insert_resource(universe)
        .insert_resource(lazy_universe)
        .add_plugins(SimulationPlugin)
        .add_plugins(MatrixRenderPlugin)
        .run();
}
