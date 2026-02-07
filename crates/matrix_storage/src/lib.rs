use matrix_core::{Region, SerializedParticle, SimConfig, Star, UniversePhase};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Complete universe snapshot for save/load
#[derive(Serialize, Deserialize)]
pub struct UniverseSnapshot {
    pub age: f64,
    pub scale_factor: f64,
    pub phase: UniversePhase,
    pub cycle: u32,
    pub temperature: f64,
    pub total_entropy: f64,
    pub config: SimConfig,
    pub particles: Vec<SerializedParticle>,
    pub regions: Vec<Region>,
    pub current_region_id: Option<u64>,
    pub loaded_stars: Vec<Star>,
    pub life_planets: Vec<(u64, String)>,
    pub civilization_count: u32,
    pub time_scale: f64,
    pub paused: bool,
}

/// Save a snapshot to disk as bincode
pub fn save_snapshot(snapshot: &UniverseSnapshot, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {e}"))?;
    }
    let data = bincode::serialize(snapshot).map_err(|e| format!("Serialize error: {e}"))?;
    fs::write(path, data).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

/// Load a snapshot from disk
pub fn load_snapshot(path: &Path) -> Result<UniverseSnapshot, String> {
    let data = fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let snapshot =
        bincode::deserialize(&data).map_err(|e| format!("Deserialize error: {e}"))?;
    Ok(snapshot)
}
