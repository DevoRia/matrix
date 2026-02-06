use bevy::prelude::*;
use std::collections::HashMap;
use matrix_core::{GpuParticle, ParticleKind};
use matrix_sim::universe::UniverseState;

/// Marker for particle point entities in the render world
#[derive(Component)]
pub struct ParticlePoint {
    pub index: usize,
}

/// Maximum rendered particles (subset of simulation for performance)
const MAX_RENDER_PARTICLES: usize = 20_000;

/// Spawn particle visualization entities with shared materials
pub fn spawn_particle_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    universe: Res<UniverseState>,
) {
    // Low-poly sphere (12 triangles) — very efficient when batched
    let mesh = meshes.add(Sphere::new(0.03).mesh().ico(0).unwrap());

    // Pre-create shared materials per particle kind
    let mut material_cache: HashMap<u32, Handle<StandardMaterial>> = HashMap::new();

    let count = universe.particles.len().min(MAX_RENDER_PARTICLES);

    // Stride to evenly sample particles if count < total
    let stride = if universe.particles.len() > MAX_RENDER_PARTICLES {
        universe.particles.len() / MAX_RENDER_PARTICLES
    } else {
        1
    };

    for render_idx in 0..count {
        let sim_idx = render_idx * stride;
        if sim_idx >= universe.particles.len() {
            break;
        }

        let p = &universe.particles[sim_idx];
        let kind_id = p.kind;

        // Get or create shared material for this particle type
        let mat = material_cache
            .entry(kind_id)
            .or_insert_with(|| {
                let color = particle_color(p);
                materials.add(StandardMaterial {
                    base_color: color,
                    emissive: LinearRgba::from(color) * 3.0,
                    unlit: true,
                    ..default()
                })
            })
            .clone();

        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(p.position[0], p.position[1], p.position[2]),
            ParticlePoint { index: sim_idx },
        ));
    }

    info!("Spawned {} render particles from {} simulation particles", count, universe.particles.len());
}

/// Update particle positions from simulation state
/// Skips entirely when camera is in cosmos mode (particles_active = false)
pub fn update_particle_visuals(
    universe: Res<UniverseState>,
    mut query: Query<(&mut Transform, &ParticlePoint)>,
) {
    // Skip 20K entity updates when camera is far from particles
    if !universe.particles_active {
        return;
    }

    for (mut transform, particle) in query.iter_mut() {
        if particle.index >= universe.particles.len() {
            continue;
        }
        let p = &universe.particles[particle.index];
        transform.translation.x = p.position[0];
        transform.translation.y = p.position[1];
        transform.translation.z = p.position[2];
    }
}

fn particle_color(p: &GpuParticle) -> Color {
    let kind = match p.kind {
        0 => ParticleKind::UpQuark,
        1 => ParticleKind::DownQuark,
        2 => ParticleKind::Electron,
        3 => ParticleKind::Neutrino,
        4 => ParticleKind::Photon,
        5 => ParticleKind::Gluon,
        10 => ParticleKind::Proton,
        11 => ParticleKind::Neutron,
        20 => ParticleKind::Hydrogen,
        21 => ParticleKind::Helium,
        100 => ParticleKind::DarkMatter,
        _ => ParticleKind::Hydrogen,
    };

    let c = kind.color();
    Color::srgba(c[0], c[1], c[2], c[3])
}
