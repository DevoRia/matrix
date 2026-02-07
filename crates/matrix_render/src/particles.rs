use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::HashMap;
use matrix_core::ParticleKind;
use matrix_sim::universe::UniverseState;

/// Marker for particle cloud entities (one per particle kind)
#[derive(Component)]
pub struct ParticleCloud {
    pub kind: u32,
}

/// Max particles to sample for rendering (fewer = faster)
const MAX_SAMPLE: usize = 3_000;

/// Distance culling for particle updates (squared) — large enough for cosmic view
const CULL_DIST_SQ: f32 = 2000.0 * 2000.0;

/// Base triangle size (close-up). Scales with camera distance for cosmic visibility.
const BASE_TRI_SIZE: f32 = 0.04;

/// Compute triangle size based on camera distance from particle cloud center.
/// At 640 units (Cosmic): ~2.6 — visible as glowing dots.
/// At 50 units (Stellar): ~0.2. At 5 units (Planetary): ~0.04 (base).
fn compute_tri_size(cam_pos: Vec3, cloud_center: Vec3) -> f32 {
    let dist = cam_pos.distance(cloud_center);
    (dist * 0.004).clamp(BASE_TRI_SIZE, 3.0)
}

/// Tracks point-cloud render state
#[derive(Resource)]
pub struct ParticleCloudState {
    /// Last generation rendered
    pub render_generation: u32,
    /// Per-kind: (entity, mesh_handle)
    pub clouds: HashMap<u32, (Entity, Handle<Mesh>)>,
    /// Per-kind material
    pub materials: HashMap<u32, Handle<StandardMaterial>>,
    /// Frame counter for throttling mesh updates
    pub update_frame: u32,
}

impl Default for ParticleCloudState {
    fn default() -> Self {
        Self {
            render_generation: u32::MAX,
            clouds: HashMap::new(),
            materials: HashMap::new(),
            update_frame: 0,
        }
    }
}

/// Startup: insert resource
pub fn init_particle_cloud(mut commands: Commands) {
    commands.insert_resource(ParticleCloudState::default());
}

/// When particle generation changes: rebuild cloud entities (one mesh per kind)
pub fn sync_particle_clouds(
    mut commands: Commands,
    universe: Res<UniverseState>,
    mut state: ResMut<ParticleCloudState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_query: Query<&Transform, With<super::camera::FlyCamera>>,
) {
    if universe.particles_generation == state.render_generation {
        return;
    }
    state.render_generation = universe.particles_generation;

    // Despawn old cloud entities
    for (_, (entity, _)) in state.clouds.drain() {
        commands.entity(entity).despawn();
    }
    state.materials.clear();

    if universe.particles.is_empty() {
        return;
    }

    let cam_pos = camera_query
        .get_single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    // Group particle positions by kind (with stride sampling)
    let stride = (universe.particles.len() / MAX_SAMPLE).max(1);
    let mut groups: HashMap<u32, Vec<[f32; 3]>> = HashMap::new();

    for (i, p) in universe.particles.iter().enumerate() {
        if i % stride != 0 {
            continue;
        }
        if !p.is_alive() {
            continue;
        }
        groups.entry(p.kind).or_default().push(p.pos());
    }

    let total_sampled: usize = groups.values().map(|v| v.len()).sum();

    // Compute cloud center (average of all positions) for tri_size scaling
    let cloud_center = compute_cloud_center(&groups);
    let tri_size = compute_tri_size(cam_pos, cloud_center);

    for (kind_id, positions) in &groups {
        let mesh = build_triangle_cloud(positions, tri_size);
        let mesh_handle = meshes.add(mesh);

        let color = kind_color(*kind_id);
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::from(color) * 3.0,
            unlit: true,
            ..default()
        });

        let entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::IDENTITY,
                ParticleCloud { kind: *kind_id },
            ))
            .id();

        state.clouds.insert(*kind_id, (entity, mesh_handle));
        state.materials.insert(*kind_id, mat);
    }

    info!(
        "Particle clouds: {} kinds, {} triangles ({} sim particles, tri_size={:.3})",
        groups.len(),
        total_sampled,
        universe.particles.len(),
        tri_size,
    );
}

/// Update cloud mesh vertices every 3rd frame (position sync from simulation)
pub fn update_particle_clouds(
    universe: Res<UniverseState>,
    mut state: ResMut<ParticleCloudState>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera_query: Query<&Transform, (With<super::camera::FlyCamera>, Without<ParticleCloud>)>,
) {
    if !universe.particles_active || universe.particles.is_empty() || state.clouds.is_empty() {
        return;
    }

    state.update_frame = state.update_frame.wrapping_add(1);
    if state.update_frame % 3 != 0 {
        return;
    }

    let cam_pos = camera_query
        .get_single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    // Rebuild per-kind position lists with distance culling
    let stride = (universe.particles.len() / MAX_SAMPLE).max(1);
    let mut groups: HashMap<u32, Vec<[f32; 3]>> = HashMap::new();

    for (i, p) in universe.particles.iter().enumerate() {
        if i % stride != 0 {
            continue;
        }
        if !p.is_alive() {
            continue;
        }
        let dx = p.position[0] - cam_pos.x;
        let dy = p.position[1] - cam_pos.y;
        let dz = p.position[2] - cam_pos.z;
        if dx * dx + dy * dy + dz * dz > CULL_DIST_SQ {
            continue;
        }
        groups.entry(p.kind).or_default().push(p.pos());
    }

    // Dynamic triangle size based on camera distance from cloud center
    let cloud_center = compute_cloud_center(&groups);
    let tri_size = compute_tri_size(cam_pos, cloud_center);

    // Update each cloud mesh
    for (kind_id, (_entity, mesh_handle)) in &state.clouds {
        if let Some(mesh) = meshes.get_mut(mesh_handle) {
            let positions = groups.remove(kind_id).unwrap_or_default();
            rebuild_triangle_cloud(mesh, &positions, tri_size);
        }
    }
}

/// Compute approximate center of all particle groups
fn compute_cloud_center(groups: &HashMap<u32, Vec<[f32; 3]>>) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let mut count = 0u32;
    for positions in groups.values() {
        for pos in positions {
            sum += Vec3::new(pos[0], pos[1], pos[2]);
            count += 1;
        }
    }
    if count > 0 {
        sum / count as f32
    } else {
        Vec3::ZERO
    }
}

/// Build a mesh where each particle = 1 small triangle (3 vertices)
/// Total: N particles -> 3N vertices, N triangles, ONE draw call
fn build_triangle_cloud(positions: &[[f32; 3]], tri_size: f32) -> Mesh {
    let vert_count = positions.len() * 3;
    let mut verts = Vec::with_capacity(vert_count);
    let mut normals = Vec::with_capacity(vert_count);

    let s = tri_size;
    for pos in positions {
        verts.push([pos[0] - s, pos[1] - s, pos[2]]);
        verts.push([pos[0] + s, pos[1] - s, pos[2]]);
        verts.push([pos[0], pos[1] + s, pos[2]]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, verts)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

/// Update an existing mesh's vertices in place (avoids reallocation)
fn rebuild_triangle_cloud(mesh: &mut Mesh, positions: &[[f32; 3]], tri_size: f32) {
    let vert_count = positions.len() * 3;
    let mut verts = Vec::with_capacity(vert_count);
    let mut normals = Vec::with_capacity(vert_count);

    let s = tri_size;
    for pos in positions {
        verts.push([pos[0] - s, pos[1] - s, pos[2]]);
        verts.push([pos[0] + s, pos[1] - s, pos[2]]);
        verts.push([pos[0], pos[1] + s, pos[2]]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
}

fn kind_color(kind_id: u32) -> Color {
    let kind = match kind_id {
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
        22 => ParticleKind::Carbon,
        23 => ParticleKind::Nitrogen,
        24 => ParticleKind::Oxygen,
        25 => ParticleKind::Iron,
        100 => ParticleKind::DarkMatter,
        _ => ParticleKind::Hydrogen,
    };

    let c = kind.color();
    Color::srgba(c[0], c[1], c[2], c[3])
}
