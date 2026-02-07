use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use matrix_core::SimConfig;
use matrix_sim::lazy_universe::LazyUniverse;
pub use matrix_sim::state::AppState;
use matrix_sim::universe::UniverseState;
use rand::SeedableRng;
use std::path::PathBuf;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn_menu)
            .add_systems(OnExit(AppState::Menu), despawn_menu)
            .add_systems(
                Update,
                menu_button_system.run_if(in_state(AppState::Menu)),
            )
            .add_systems(OnEnter(AppState::Loading), spawn_loading_screen)
            .add_systems(OnExit(AppState::Loading), despawn_loading_screen)
            .add_systems(
                Update,
                loading_poll_system.run_if(in_state(AppState::Loading)),
            );
    }
}

// --- Markers ---

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct NewWorldButton;

#[derive(Component)]
struct LoadSaveButton;

#[derive(Component)]
struct LoadingRoot;

#[derive(Component)]
struct LoadingText;

#[derive(Resource)]
struct WorldGenTask(Task<WorldGenResult>);

#[derive(Resource)]
struct LoadAction {
    is_save_load: bool,
}

enum WorldGenResult {
    NewWorld {
        universe: UniverseState,
        lazy: LazyUniverse,
    },
    LoadedSave {
        snapshot: matrix_storage::UniverseSnapshot,
    },
}

fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

fn has_saves() -> bool {
    std::fs::read_dir(saves_dir())
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().map_or(false, |ext| ext == "bin"))
        })
        .unwrap_or(false)
}

fn find_latest_save() -> Option<PathBuf> {
    std::fs::read_dir(saves_dir())
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "bin"))
                .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                .map(|e| e.path())
        })
}

// --- Menu ---

fn spawn_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(20.0),
                ..default()
            },
            MenuRoot,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("MATRIX"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 1.0, 0.4, 0.9)),
            ));

            parent.spawn((
                Text::new("Universe Simulation"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.8, 0.3, 0.7)),
            ));

            // Spacer
            parent.spawn(Node {
                height: Val::Px(40.0),
                ..default()
            });

            // "New Universe" button
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(300.0),
                        height: Val::Px(60.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.4, 0.1, 0.9)),
                    NewWorldButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("New Universe"),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // "Load Save" button — only if saves exist
            if has_saves() {
                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(300.0),
                            height: Val::Px(60.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.2, 0.5, 0.9)),
                        LoadSaveButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Load Save"),
                            TextFont {
                                font_size: 28.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }
        });
}

fn despawn_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn menu_button_system(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    new_world_q: Query<&Interaction, (Changed<Interaction>, With<NewWorldButton>)>,
    load_save_q: Query<&Interaction, (Changed<Interaction>, With<LoadSaveButton>)>,
    universe: Res<UniverseState>,
) {
    // Hover color changes
    // (keeping it simple — just check for Pressed)

    for interaction in &new_world_q {
        if *interaction == Interaction::Pressed {
            let config = universe.config.clone();
            let pool = AsyncComputeTaskPool::get();
            let task = pool.spawn(async move {
                let lazy = LazyUniverse::new(config.clone(), 0.0);
                let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(config.seed);
                let particles = matrix_physics::particle::generate_big_bang(&config, &mut rng);
                let uni = UniverseState::new(config, particles);
                WorldGenResult::NewWorld {
                    universe: uni,
                    lazy,
                }
            });
            commands.insert_resource(WorldGenTask(task));
            commands.insert_resource(LoadAction {
                is_save_load: false,
            });
            next_state.set(AppState::Loading);
            return;
        }
    }

    for interaction in &load_save_q {
        if *interaction == Interaction::Pressed {
            if let Some(path) = find_latest_save() {
                let pool = AsyncComputeTaskPool::get();
                let task = pool.spawn(async move {
                    match matrix_storage::load_snapshot(&path) {
                        Ok(snapshot) => WorldGenResult::LoadedSave { snapshot },
                        Err(e) => {
                            error!("Failed to load snapshot: {e}");
                            // Fallback: generate new world
                            let config = SimConfig::default();
                            let lazy = LazyUniverse::new(config.clone(), 0.0);
                            let mut rng =
                                rand_chacha::ChaCha8Rng::seed_from_u64(config.seed);
                            let particles =
                                matrix_physics::particle::generate_big_bang(&config, &mut rng);
                            let uni = UniverseState::new(config, particles);
                            WorldGenResult::NewWorld {
                                universe: uni,
                                lazy,
                            }
                        }
                    }
                });
                commands.insert_resource(WorldGenTask(task));
                commands.insert_resource(LoadAction {
                    is_save_load: true,
                });
                next_state.set(AppState::Loading);
            }
            return;
        }
    }
}

// --- Loading screen ---

fn spawn_loading_screen(mut commands: Commands, action: Option<Res<LoadAction>>) {
    let msg = if action.map_or(false, |a| a.is_save_load) {
        "Loading save..."
    } else {
        "Generating universe..."
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            LoadingRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(msg),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 1.0, 0.4, 0.9)),
                LoadingText,
            ));
        });
}

fn despawn_loading_screen(mut commands: Commands, query: Query<Entity, With<LoadingRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn loading_poll_system(
    mut commands: Commands,
    task: Option<ResMut<WorldGenTask>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut universe: ResMut<UniverseState>,
    mut lazy: ResMut<LazyUniverse>,
) {
    let Some(mut gen_task) = task else { return };

    let Some(result) = block_on(poll_once(&mut gen_task.0)) else {
        return;
    };

    match result {
        WorldGenResult::NewWorld {
            universe: new_uni,
            lazy: new_lazy,
        } => {
            *universe = new_uni;
            *lazy = new_lazy;
            info!(
                "World generated: {} regions, {} particles",
                lazy.region_count(),
                universe.particles.len()
            );
        }
        WorldGenResult::LoadedSave { snapshot } => {
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
            universe.cached_alive_count = universe.particles.len();
            universe.particles_generation = universe.particles_generation.wrapping_add(1);

            lazy.regions = snapshot.regions;
            lazy.current_region_id = snapshot.current_region_id;
            lazy.loaded_stars = snapshot.loaded_stars;
            lazy.life_planets = snapshot.life_planets;
            lazy.civilization_count = snapshot.civilization_count;
            lazy.stars_generation = lazy.stars_generation.wrapping_add(1);

            info!(
                "Save loaded: age {:.4} Gyr, {} particles",
                universe.age,
                universe.particles.len()
            );
        }
    }

    commands.remove_resource::<WorldGenTask>();
    commands.remove_resource::<LoadAction>();
    next_state.set(AppState::Running);
}
