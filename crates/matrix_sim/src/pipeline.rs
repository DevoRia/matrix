use bevy::prelude::*;

use super::universe::UniverseState;

/// Bevy plugin for the simulation pipeline
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, simulation_tick);
    }
}

/// Main simulation tick — updates particles and universe state
fn simulation_tick(mut universe: ResMut<UniverseState>, time: Res<Time>) {
    let dt = time.delta_secs_f64();
    universe.tick(dt);
}
