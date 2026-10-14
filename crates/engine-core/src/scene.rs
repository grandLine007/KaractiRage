use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSpawn {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinEntity {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub script_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorEntity {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub script_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallEntity {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneData {
    pub name: String,
    pub description: String,
    pub player: PlayerSpawn,
    pub coins: Vec<CoinEntity>,
    pub doors: Vec<DoorEntity>,
    #[serde(default)]
    pub walls: Vec<WallEntity>,
}

impl SceneData {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let scene: Self = serde_json::from_str(&content)?;
        Ok(scene)
    }

    #[allow(dead_code)]
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }
}
