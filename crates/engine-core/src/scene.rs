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
pub struct EnemyEntity {
    pub id: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_enemy_size")]
    pub size: f32,
    pub patrol_min_y: f32,
    pub patrol_max_y: f32,
    #[serde(default = "default_enemy_speed")]
    pub speed: f32,
    #[serde(default = "default_vision_range")]
    pub vision_range: f32,
    pub script_path: String,
}

fn default_enemy_size() -> f32 {
    40.0
}
fn default_enemy_speed() -> f32 {
    100.0
}
fn default_vision_range() -> f32 {
    200.0
}

use crate::entity::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneData {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub entities: Vec<Entity>,

    // Rétrocompatibilité avec les scènes existantes
    #[serde(default)]
    pub player: Option<PlayerSpawn>,
    #[serde(default)]
    pub coins: Vec<CoinEntity>,
    #[serde(default)]
    pub doors: Vec<DoorEntity>,
    #[serde(default)]
    pub walls: Vec<WallEntity>,
    #[serde(default)]
    pub enemies: Vec<EnemyEntity>,
}

impl SceneData {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let scene: Self = serde_json::from_str(&content)?;
        Ok(scene)
    }

    pub fn from_json_str(json: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let scene: Self = serde_json::from_str(json)?;
        Ok(scene)
    }

    /// Convertit toutes les entités de la scène en entités génériques du moteur
    pub fn to_entities(&self) -> Vec<Entity> {
        let mut list = self.entities.clone();
        if !list.is_empty() {
            for ent in &mut list {
                ent.is_active = true;
            }
            return list;
        }

        let mut next_id = 1;

        // Joueur
        if let Some(ref p) = self.player {
            let ent = Entity::new(next_id, "Joueur", "player", Transform2D::new(p.x, p.y, p.size, p.size))
                .with_renderable(Renderable2D::rect(0x0038BDF8, 10).with_outline(0x00FFFFFF, 2))
                .with_collider(Collider2D::solid(p.size, p.size))
                .with_movement(Movement2D::player(p.speed))
                .with_health(Health2D::new(3.0));
            list.push(ent);
            next_id += 1;
        }

        // Murs
        for w in &self.walls {
            let ent = Entity::new(next_id, w.id.clone(), "wall", Transform2D::new(w.x, w.y, w.w, w.h))
                .with_renderable(Renderable2D::rect(0x001E293B, 1).with_outline(0x00475569, 2))
                .with_collider(Collider2D::solid(w.w, w.h));
            list.push(ent);
            next_id += 1;
        }

        // Portes
        for d in &self.doors {
            let ent = Entity::new(next_id, d.id.clone(), "door", Transform2D::new(d.x, d.y, d.w, d.h))
                .with_renderable(Renderable2D::rect(0x00EF4444, 2))
                .with_collider(Collider2D::solid(d.w, d.h))
                .with_script(KVLScriptComponent::new(&d.script_path).with_interact("OnDoorInteract"));
            list.push(ent);
            next_id += 1;
        }

        // Pièces
        for c in &self.coins {
            let ent = Entity::new(next_id, c.id.clone(), "collectible", Transform2D::new(c.x - c.radius, c.y - c.radius, c.radius * 2.0, c.radius * 2.0))
                .with_renderable(Renderable2D::circle(c.radius, 0x00F59E0B, 5))
                .with_collider(Collider2D::trigger(c.radius * 2.0, c.radius * 2.0))
                .with_script(KVLScriptComponent::new(&c.script_path).with_overlap("OnPlayerOverlap"));
            list.push(ent);
            next_id += 1;
        }

        // Sentinelles
        for e in &self.enemies {
            let ent = Entity::new(next_id, e.id.clone(), "enemy", Transform2D::new(e.x, e.y, e.size, e.size))
                .with_renderable(Renderable2D::rect(0x00DC2626, 8).with_outline(0x00F87171, 1))
                .with_movement(Movement2D::patrol_vertical(e.speed, e.patrol_min_y, e.patrol_max_y))
                .with_vision(VisionSensing::new(e.vision_range))
                .with_script(KVLScriptComponent::new(&e.script_path).with_seen("OnPlayerSeen"));
            list.push(ent);
            next_id += 1;
        }

        list
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
