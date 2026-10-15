use crate::adaptation::AdaptationManager;
use crate::audio::{AudioEngine, SoundType};
use crate::camera::Camera2D;
use crate::dialogue::{DialogueAction, DialogueBox, QuestTracker};
use crate::editor::{EditorMode, EditorState, SelectedEntity, StudioTool};
use crate::entity::{Entity, MovementMode, ShapeKind};
use crate::input::InputState;
use crate::renderer::Canvas;
use crate::scene::SceneData;
use crate::spatial::{SpatialHashGrid, SpatialItem, SpatialKind};
use crate::telemetry::Telemetry;
use kvl_core::*;
use kvl_runtime::*;
use std::collections::HashMap;
use std::fs;
use winit::keyboard::KeyCode;

pub const AVAILABLE_SCENES: &[(&str, &str)] = &[
    ("assets/scenes/level_01.scene.json", "Aventure Donjon (Top-Down)"),
    ("assets/scenes/platformer_01.scene.json", "Plateforme 2D (Gravite/Saut)"),
    ("assets/scenes/space_01.scene.json", "Shoot'em Up (Inertie/Tirs)"),
    ("assets/scenes/puzzle_01.scene.json", "Puzzle Sokoban (Blocs/Dalles)"),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub ax: f32,
    pub ay: f32,
    pub life: f32,
    pub max_life: f32,
    pub color_start: u32,
    pub color_end: u32,
}

/// Interpolation linéaire rapide de deux couleurs ARGB 32-bit sans allocation
#[inline]
pub fn lerp_color(c1: u32, c2: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let r1 = ((c1 >> 16) & 0xFF) as f32;
    let g1 = ((c1 >> 8) & 0xFF) as f32;
    let b1 = (c1 & 0xFF) as f32;

    let r2 = ((c2 >> 16) & 0xFF) as f32;
    let g2 = ((c2 >> 8) & 0xFF) as f32;
    let b2 = (c2 & 0xFF) as f32;

    let r = (r1 + (r2 - r1) * t) as u32;
    let g = (g1 + (g2 - g1) * t) as u32;
    let b = (b1 + (b2 - b1) * t) as u32;

    (r << 16) | (g << 8) | b
}

impl Particle {
    #[inline]
    pub fn current_color(&self) -> u32 {
        if self.color_start == self.color_end || self.max_life <= 0.0 {
            self.color_start
        } else {
            let t = 1.0 - (self.life / self.max_life).clamp(0.0, 1.0);
            lerp_color(self.color_start, self.color_end, t)
        }
    }
}

/// Algorithme de Liang-Barsky ultra-rapide et déterministe pour l'intersection segment de vue / AABB
pub fn segment_intersects_aabb(x1: f32, y1: f32, x2: f32, y2: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let p = [-dx, dx, -dy, dy];
    let q = [
        x1 - bx,
        (bx + bw) - x1,
        y1 - by,
        (by + bh) - y1,
    ];

    let mut u1 = 0.0f32;
    let mut u2 = 1.0f32;

    for i in 0..4 {
        if p[i] == 0.0 {
            if q[i] < 0.0 {
                return false;
            }
        } else {
            let t = q[i] / p[i];
            if p[i] < 0.0 {
                if t > u2 {
                    return false;
                }
                if t > u1 {
                    u1 = t;
                }
            } else {
                if t < u1 {
                    return false;
                }
                if t < u2 {
                    u2 = t;
                }
            }
        }
    }
    u1 <= u2
}

/// Moteur de jeu universel KaractiRage
/// Découplé de tout gameplay spécifique : toutes les entités du monde sont gérées
/// via un modèle modulaire générique Entité-Composants (ECS) et pilotées par KVL.
pub struct GameState {
    pub scene_path: String,
    pub scene_name: String,
    pub scene_objective: Option<String>,
    pub entities: Vec<Entity>,
    pub particles: Vec<Particle>,
    pub kvl_graphs: HashMap<String, Graph>,
    pub kvl_context: ExecutionContext,
    pub status_message: String,
    pub telemetry: Telemetry,
    pub adaptation: AdaptationManager,
    pub editor: EditorState,
    pub audio: AudioEngine,
    pub show_minimap: bool,
    pub spatial_grid: SpatialHashGrid,
    pub elapsed_time: f32,
    pub player_spawn_pos: (f32, f32),
    pub camera: Camera2D,
    pub world_bounds: (f32, f32, f32, f32),
    pub dialogue: DialogueBox,
    pub quest: Option<QuestTracker>,
}

impl GameState {
    /// Trouve l'index de l'entité contrôlée par le joueur
    pub fn player_index(&self) -> Option<usize> {
        self.entities.iter().position(|e| {
            e.is_active && e.movement.as_ref().map_or(false, |m| m.is_player_controlled)
        })
    }

    /// Référence immuable vers l'entité joueur
    #[allow(dead_code)]
    pub fn player_entity(&self) -> Option<&Entity> {
        self.entities.iter().find(|e| {
            e.is_active && e.movement.as_ref().map_or(false, |m| m.is_player_controlled)
        })
    }

    /// Référence mutable vers l'entité joueur
    #[allow(dead_code)]
    pub fn player_entity_mut(&mut self) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|e| {
            e.is_active && e.movement.as_ref().map_or(false, |m| m.is_player_controlled)
        })
    }

    /// Recherche une entité par son nom
    #[allow(dead_code)]
    pub fn find_entity(&self, name: &str) -> Option<&Entity> {
        self.entities.iter().find(|e| e.name == name)
    }

    /// Recherche une entité par son nom (mutable)
    #[allow(dead_code)]
    pub fn find_entity_mut(&mut self, name: &str) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|e| e.name == name)
    }

    /// Calcule un nouvel identifiant unique d'entité (séquentiel et sans collision)
    pub fn next_entity_id(&self) -> usize {
        self.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1
    }

    /// Crée et insère une nouvelle entité dans le monde à partir d'un gabarit de base
    pub fn spawn_entity(&mut self, kind: &str, x: f32, y: f32) -> usize {
        let id = self.next_entity_id();
        let entity = match kind {
            "wall" => Entity::spawn_wall(id, x, y, 32.0, 32.0),
            "collectible" => Entity::spawn_collectible(id, x, y, 12.0, "assets/scripts/coin_pickup.kvl.json"),
            "door" => Entity::spawn_door(id, x, y, 40.0, 56.0, "assets/scripts/door_interaction.kvl.json"),
            "enemy" => Entity::spawn_enemy(id, x, y, 28.0, 80.0, (y - 100.0).max(80.0), (y + 100.0).min(560.0), "assets/scripts/guard_alert.kvl.json"),
            "portal" => Entity::spawn_portal(id, x, y, 36.0, 48.0, "assets/scripts/coin_pickup.kvl.json"),
            "torch" => Entity::spawn_torch(id, x, y),
            "crate" => Entity::spawn_crate(id, x, y, 44.0),
            "plate" => Entity::spawn_plate(id, x, y, 48.0),
            "hazard" => Entity::spawn_hazard(id, x, y, 32.0),
            "heart" => Entity::spawn_heart(id, x, y, 12.0),
            "npc" => Entity::spawn_npc(
                id,
                format!("gardien_{}", id),
                x,
                y,
                0x0038BDF8,
                "Gardien",
                "Bienvenue voyageur ! Le sanctuaire ancien requiert votre aide pour retrouver les 3 runes sacrees.",
                vec!["Accepter la quete".into(), "Plus tard".into()],
            ),
            _ => Entity::spawn_wall(id, x, y, 32.0, 32.0),
        };
        self.entities.push(entity);
        let new_idx = self.entities.len() - 1;
        self.editor.selected = SelectedEntity::Entity(new_idx);
        self.audio.play(SoundType::UiClick);
        self.spawn_particles(x, y, 15, 0x0038BDF8);
        self.status_message = format!("Entité '{}' (ID {}) créée avec succès !", kind, id);
        new_idx
    }

    /// Duplique l'entité actuellement sélectionnée avec un décalage spatial de confort
    pub fn duplicate_selected_entity(&mut self) -> Option<usize> {
        let selected_idx = match self.editor.selected {
            SelectedEntity::Entity(idx) if idx < self.entities.len() => idx,
            _ => {
                self.status_message = "Veuillez d'abord sélectionner une entité à dupliquer (Ctrl+D).".into();
                return None;
            }
        };

        let mut dup = self.entities[selected_idx].clone();
        let new_id = self.next_entity_id();
        dup.id = new_id;
        dup.name = format!("{}_copy", dup.name);
        dup.transform.x = (dup.transform.x + 24.0).min(1150.0);
        dup.transform.y = (dup.transform.y + 24.0).min(580.0);

        // Si c'est un clone de joueur, désactiver le contrôle direct pour éviter les conflits
        if let Some(ref mut mov) = dup.movement {
            mov.is_player_controlled = false;
        }

        self.entities.push(dup);
        let new_idx = self.entities.len() - 1;
        self.editor.selected = SelectedEntity::Entity(new_idx);
        self.audio.play(SoundType::UiClick);
        self.spawn_particles(self.entities[new_idx].transform.x, self.entities[new_idx].transform.y, 12, 0x00F59E0B);
        self.status_message = format!("Entité dupliquée avec succès (Nouvel ID : {}) !", new_id);
        Some(new_idx)
    }

    /// Supprime l'entité actuellement sélectionnée en protégeant le joueur principal
    pub fn remove_selected_entity(&mut self) -> bool {
        let selected_idx = match self.editor.selected {
            SelectedEntity::Entity(idx) if idx < self.entities.len() => idx,
            _ => {
                self.status_message = "Aucune entité sélectionnée à supprimer (Suppr).".into();
                return false;
            }
        };

        // Sécurité : interdiction de supprimer l'avatar joueur
        if self.entities[selected_idx].movement.as_ref().map_or(false, |m| m.is_player_controlled) {
            self.status_message = "Suppression refusée : l'entité Joueur principale est protégée !".into();
            self.audio.play(SoundType::AccessDenied);
            return false;
        }

        let name = self.entities[selected_idx].name.clone();
        let x = self.entities[selected_idx].transform.x;
        let y = self.entities[selected_idx].transform.y;
        self.entities.remove(selected_idx);
        self.editor.selected = SelectedEntity::None;
        self.audio.play(SoundType::WireCut);
        self.spawn_particles(x, y, 16, 0x00EF4444);
        self.status_message = format!("Entité '{}' supprimée de la scène.", name);
        true
    }

    /// Ajoute dynamiquement un composant manquant à l'entité sélectionnée
    pub fn add_component_to_selected(&mut self, comp_type: &str) -> bool {
        let selected_idx = match self.editor.selected {
            SelectedEntity::Entity(idx) if idx < self.entities.len() => idx,
            _ => return false,
        };

        let ent = &mut self.entities[selected_idx];
        match comp_type {
            "Collider" => {
                if ent.collider.is_none() {
                    ent.collider = Some(crate::entity::Collider2D::solid(ent.transform.w, ent.transform.h));
                    self.status_message = format!("Composant Collider2D (Solide) attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "Movement" => {
                if ent.movement.is_none() {
                    ent.movement = Some(crate::entity::Movement2D::new(140.0));
                    self.status_message = format!("Composant Movement2D (140 px/s) attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "Vision" => {
                if ent.vision.is_none() {
                    ent.vision = Some(crate::entity::VisionSensing::new(200.0));
                    self.status_message = format!("Composant VisionSensing (Portée 200) attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "Script" => {
                if ent.script.is_none() {
                    let script_path = "assets/scripts/coin_pickup.kvl.json";
                    ent.script = Some(crate::entity::KVLScriptComponent::new(script_path).with_overlap("OnPlayerOverlap"));
                    self.status_message = format!("Composant KVLScript ({}) attaché à '{}'.", script_path, ent.name);
                } else {
                    return false;
                }
            }
            "Emitter" => {
                if ent.emitter.is_none() {
                    ent.emitter = Some(crate::entity::ParticleEmitter2D::torch_fire());
                    self.status_message = format!("Composant ParticleEmitter2D (Torche) attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "Dialogue" => {
                if ent.dialogue.is_none() {
                    ent.dialogue = Some(crate::entity::DialogueComponent::new(
                        "Gardien",
                        "Salutations voyageur ! Que puis-je faire pour vous ?",
                    ).with_choices(vec!["Demander conseil".into(), "Partir".into()]));
                    self.status_message = format!("Composant DialogueComponent attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "CycleMovementMode" => {
                // Change le modèle physique de l'entité sans toucher au reste de ses composants
                match ent.movement {
                    Some(ref mut mov) => {
                        mov.mode = mov.mode.next_preset();
                        mov.vx = 0.0;
                        mov.vy = 0.0;
                        ent.transform.rotation = 0.0;
                        self.status_message = format!("Physique de '{}' : mode {}.", ent.name, mov.mode.label());
                    }
                    None => return false,
                }
            }
            "SpeedDec" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        mov.speed = (mov.speed - 25.0).max(25.0);
                        self.status_message = format!("Vitesse de '{}' : {:.0} px/s.", ent.name, mov.speed);
                    }
                    None => return false,
                }
            }
            "SpeedInc" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        mov.speed = (mov.speed + 25.0).min(900.0);
                        self.status_message = format!("Vitesse de '{}' : {:.0} px/s.", ent.name, mov.speed);
                    }
                    None => return false,
                }
            }
            "JumpDec" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        if let MovementMode::Platformer { ref mut jump_force, .. } = mov.mode {
                            *jump_force = (*jump_force - 40.0).max(120.0);
                            self.status_message = format!("Impulsion saut de '{}' : {:.0} px/s.", ent.name, *jump_force);
                        } else {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            "JumpInc" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        if let MovementMode::Platformer { ref mut jump_force, .. } = mov.mode {
                            *jump_force = (*jump_force + 40.0).min(1200.0);
                            self.status_message = format!("Impulsion saut de '{}' : {:.0} px/s.", ent.name, *jump_force);
                        } else {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            "GravityDec" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        if let MovementMode::Platformer { ref mut gravity, .. } = mov.mode {
                            *gravity = (*gravity - 80.0).max(100.0);
                            self.status_message = format!("Gravité de '{}' : {:.0} px/s².", ent.name, *gravity);
                        } else {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            "GravityInc" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        if let MovementMode::Platformer { ref mut gravity, .. } = mov.mode {
                            *gravity = (*gravity + 80.0).min(2500.0);
                            self.status_message = format!("Gravité de '{}' : {:.0} px/s².", ent.name, *gravity);
                        } else {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            "ThrustDec" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        if let MovementMode::SpaceInertia { ref mut thrust, .. } = mov.mode {
                            *thrust = (*thrust - 50.0).max(100.0);
                            self.status_message = format!("Poussée spatiale de '{}' : {:.0}.", ent.name, *thrust);
                        } else {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            "ThrustInc" => {
                match ent.movement {
                    Some(ref mut mov) => {
                        if let MovementMode::SpaceInertia { ref mut thrust, .. } = mov.mode {
                            *thrust = (*thrust + 50.0).min(1500.0);
                            self.status_message = format!("Poussée spatiale de '{}' : {:.0}.", ent.name, *thrust);
                        } else {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            "TogglePushable" => {
                match ent.collider {
                    Some(ref mut col) if col.is_solid || col.is_pushable => {
                        col.is_pushable = !col.is_pushable;
                        col.is_solid = true;
                        self.status_message = if col.is_pushable {
                            format!("'{}' est maintenant un bloc poussable.", ent.name)
                        } else {
                            format!("'{}' redevient un solide fixe.", ent.name)
                        };
                    }
                    _ => return false,
                }
            }
            "Health" => {
                if ent.health.is_none() {
                    ent.health = Some(crate::entity::Health2D::new(3.0));
                    self.status_message = format!("Composant Health2D (3 PV) attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "DamageSelected" => {
                match ent.health {
                    Some(ref mut h) => {
                        let _ = h.damage(1.0);
                        self.status_message = format!("1 dégât infligé à '{}' : {:.0}/{:.0} PV", ent.name, h.current, h.max);
                    }
                    None => return false,
                }
            }
            "HealSelected" => {
                match ent.health {
                    Some(ref mut h) => {
                        let _ = h.heal(1.0);
                        self.status_message = format!("1 PV soigné sur '{}' : {:.0}/{:.0} PV", ent.name, h.current, h.max);
                    }
                    None => return false,
                }
            }
            "MaxHpInc" => {
                match ent.health {
                    Some(ref mut h) => {
                        h.max = (h.max + 1.0).min(20.0);
                        h.current = (h.current + 1.0).min(h.max);
                        self.status_message = format!("Points de vie max de '{}' : {:.0} PV", ent.name, h.max);
                    }
                    None => return false,
                }
            }
            "MaxHpDec" => {
                match ent.health {
                    Some(ref mut h) => {
                        h.max = (h.max - 1.0).max(1.0);
                        h.current = h.current.min(h.max);
                        self.status_message = format!("Points de vie max de '{}' : {:.0} PV", ent.name, h.max);
                    }
                    None => return false,
                }
            }
            _ => return false,
        }
        self.audio.play(SoundType::UiClick);
        true
    }

    pub fn load_from_scene(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let scene_data = match SceneData::load_from_file(path) {
            Ok(s) => s,
            Err(_) => {
                if let Ok(s) = SceneData::load_from_file(format!("../../{}", path)) {
                    s
                } else if path.contains("platformer") {
                    let json = include_str!("../../../assets/scenes/platformer_01.scene.json");
                    SceneData::from_json_str(json)?
                } else if path.contains("space") {
                    let json = include_str!("../../../assets/scenes/space_01.scene.json");
                    SceneData::from_json_str(json)?
                } else if path.contains("puzzle") {
                    let json = include_str!("../../../assets/scenes/puzzle_01.scene.json");
                    SceneData::from_json_str(json)?
                } else {
                    let default_json = include_str!("../../../assets/scenes/level_01.scene.json");
                    SceneData::from_json_str(default_json)?
                }
            }
        };

        let mut entities = scene_data.to_entities();

        let mut kvl_graphs = HashMap::new();
        let mut kvl_context = ExecutionContext::new();
        kvl_context.set_var("score", Value::Int(0));

        let resolve_script_content = |script_path: &str| -> Option<String> {
            if let Ok(c) = fs::read_to_string(script_path) {
                Some(c)
            } else if let Ok(c) = fs::read_to_string(format!("../../{}", script_path)) {
                Some(c)
            } else if script_path.contains("coin_pickup") {
                Some(include_str!("../../../assets/scripts/coin_pickup.kvl.json").to_string())
            } else if script_path.contains("door_interaction") {
                Some(include_str!("../../../assets/scripts/door_interaction.kvl.json").to_string())
            } else if script_path.contains("guard_alert") {
                Some(include_str!("../../../assets/scripts/guard_alert.kvl.json").to_string())
            } else {
                None
            }
        };

        // Chargement data-driven universel des graphes KVL pour TOUTES les entités scriptées
        for ent in &entities {
            if let Some(ref script_comp) = ent.script {
                let p = &script_comp.script_path;
                if !kvl_graphs.contains_key(p) {
                    if let Some(content) = resolve_script_content(p) {
                        if let Ok(graph) = Graph::from_json(&content) {
                            for (k, v) in &graph.variables {
                                if kvl_context.get_var(k).is_none() {
                                    kvl_context.set_var(k.clone(), v.initial_value.clone());
                                }
                            }
                            kvl_graphs.insert(p.clone(), graph);
                        }
                    }
                }
            }
        }

        let scene_objective = scene_data.objective.clone();

        let mut spawn_x = 100.0;
        let mut spawn_y = 100.0;
        for ent in &mut entities {
            if ent.movement.as_ref().map_or(false, |m| m.is_player_controlled) {
                spawn_x = ent.transform.x;
                spawn_y = ent.transform.y;
                if ent.health.is_none() {
                    ent.health = Some(crate::entity::Health2D::new(3.0));
                }
            }
        }

        if let Some(player) = entities.iter().find(|e| e.movement.as_ref().map_or(false, |m| m.is_player_controlled)) {
            if let Some(ref h) = player.health {
                kvl_context.set_var("player_hp", Value::Int(h.current as i64));
                kvl_context.set_var("player_max_hp", Value::Int(h.max as i64));
            }
        }

        let mut max_w: f32 = 1240.0;
        let mut max_h: f32 = 470.0;
        for ent in &entities {
            let right = ent.transform.x + ent.transform.w;
            let bottom = ent.transform.y + ent.transform.h;
            if right > max_w {
                max_w = right;
            }
            if bottom > max_h {
                max_h = bottom;
            }
        }
        let world_max_x = (max_w + 120.0).max(1280.0);
        let world_max_y = (max_h + 120.0).max(720.0);
        let world_bounds = (0.0, 0.0, world_max_x, world_max_y);

        let mut camera = Camera2D::new(20, 66, 1240, 470);
        camera.set_bounds(world_bounds.0, world_bounds.1, world_bounds.2, world_bounds.3);
        camera.target_x = spawn_x;
        camera.target_y = spawn_y;
        camera.snap_to_target();

        let quest = scene_objective.as_ref().map(|obj| {
            QuestTracker::new("main_quest", "Objectif Principal", obj, 1)
        });

        Ok(Self {
            scene_path: path.to_string(),
            scene_name: scene_data.name,
            scene_objective,
            entities,
            particles: Vec::new(),
            kvl_graphs,
            kvl_context,
            status_message: "Moteur KaractiRage initialisé ! Modèle ECS universel multi-genre actif.".into(),
            telemetry: Telemetry::new(),
            adaptation: AdaptationManager::new(),
            editor: EditorState::new(),
            audio: AudioEngine::new(),
            show_minimap: true,
            spatial_grid: SpatialHashGrid::new(64.0, 256),
            elapsed_time: 0.0,
            player_spawn_pos: (spawn_x, spawn_y),
            camera,
            world_bounds,
            dialogue: DialogueBox::new(),
            quest,
        })
    }

    pub fn reload(&mut self) {
        if let Ok(new_state) = Self::load_from_scene(&self.scene_path) {
            *self = new_state;
            self.status_message = "Hot-Reloading ECS réussi depuis les fichiers JSON !".into();
        } else {
            self.status_message = "Erreur de rechargement JSON.".into();
        }
    }

    pub fn cycle_scene_forward(&mut self) {
        let current = &self.scene_path;
        let idx = AVAILABLE_SCENES.iter().position(|(p, _)| current.contains(p) || p.contains(current.as_str())).unwrap_or(0);
        let next_idx = (idx + 1) % AVAILABLE_SCENES.len();
        let (next_path, next_genre) = AVAILABLE_SCENES[next_idx];
        self.switch_scene(next_path);
        self.status_message = format!("Genre basculé vers : {} (F2/F3: changer)", next_genre);
    }

    pub fn cycle_scene_backward(&mut self) {
        let current = &self.scene_path;
        let idx = AVAILABLE_SCENES.iter().position(|(p, _)| current.contains(p) || p.contains(current.as_str())).unwrap_or(0);
        let prev_idx = if idx == 0 { AVAILABLE_SCENES.len() - 1 } else { idx - 1 };
        let (prev_path, prev_genre) = AVAILABLE_SCENES[prev_idx];
        self.switch_scene(prev_path);
        self.status_message = format!("Genre basculé vers : {} (F2/F3: changer)", prev_genre);
    }

    pub fn switch_scene(&mut self, path: &str) -> bool {
        if let Ok(new_state) = Self::load_from_scene(path) {
            *self = new_state;
            self.audio.play(SoundType::Teleport);
            true
        } else {
            false
        }
    }

    pub fn fire_player_projectile(&mut self) -> bool {
        if let Some(p_idx) = self.player_index() {
            let (px, py, pw, ph, rot, mode) = {
                let p = &self.entities[p_idx];
                (
                    p.transform.x,
                    p.transform.y,
                    p.transform.w,
                    p.transform.h,
                    p.transform.rotation,
                    p.movement.as_ref().map_or(MovementMode::TopDown, |m| m.mode),
                )
            };

            let (spawn_x, spawn_y, vx, vy) = match mode {
                MovementMode::SpaceInertia { .. } => {
                    let dir_x = rot.cos();
                    let dir_y = rot.sin();
                    let sx = px + pw * 0.5 + dir_x * (pw * 0.6);
                    let sy = py + ph * 0.5 + dir_y * (ph * 0.6);
                    (sx, sy, dir_x * 520.0, dir_y * 520.0)
                }
                MovementMode::Platformer { .. } => {
                    let p_vx = self.entities[p_idx].movement.as_ref().map_or(1.0, |m| m.vx);
                    let dir_x = if p_vx < -10.0 { -1.0 } else { 1.0 };
                    let sx = if dir_x > 0.0 { px + pw + 4.0 } else { px - 12.0 };
                    let sy = py + ph * 0.35;
                    (sx, sy, dir_x * 480.0, 0.0)
                }
                MovementMode::TopDown | MovementMode::Grid { .. } => {
                    let sx = px + pw * 0.5 - 4.0;
                    let sy = py - 12.0;
                    (sx, sy, 0.0, -480.0)
                }
            };

            let next_id = self.entities.len() + 1;
            let bullet = Entity::spawn_projectile(next_id, spawn_x, spawn_y, vx, vy, p_idx);
            self.entities.push(bullet);
            self.audio.play(SoundType::LaserShoot);
            true
        } else {
            false
        }
    }

    pub fn save_scene_and_scripts(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Sauvegarder la scène JSON universelle avec les entités génériques
        let mut saved_entities = self.entities.clone();
        for ent in &mut saved_entities {
            ent.is_active = true;
        }

        let scene_data = SceneData {
            name: self.scene_name.clone(),
            description: "Scène universelle KaractiRage avec entités ECS et scripts KVL.".into(),
            objective: self.scene_objective.clone(),
            entities: saved_entities,
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        };

        scene_data.save_to_file(&self.scene_path)?;

        // 2. Sauvegarder chaque graphe KVL modifié sur le disque
        for (path, graph) in &self.kvl_graphs {
            let json = graph.to_json()?;
            let target_path = if std::path::Path::new(path).exists() {
                path.clone()
            } else if std::path::Path::new(&format!("../../{}", path)).exists() {
                format!("../../{}", path)
            } else {
                path.clone()
            };
            if let Some(parent) = std::path::Path::new(&target_path).parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(target_path, json)?;
        }

        self.audio.play(SoundType::BonusFanfare);
        self.status_message = "Scène ECS et scripts KVL sauvegardés sur disque avec succès ! (Ctrl+S)".into();
        Ok(())
    }

    pub fn spawn_particles(&mut self, x: f32, y: f32, count: usize, color: u32) {
        let max_allowed = self.adaptation.current_profile.max_particles();
        let to_spawn = count.min(max_allowed.saturating_sub(self.particles.len()));
        let end_col = lerp_color(color, 0x000F172A, 0.75);

        for i in 0..to_spawn {
            let angle = (i as f32) * (std::f32::consts::TAU / (to_spawn as f32));
            let speed = 60.0 + (i as f32 % 5.0) * 30.0;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                ax: 0.0,
                ay: 35.0,
                life: 1.0,
                max_life: 1.0,
                color_start: color,
                color_end: end_col,
            });
        }
    }

    /// Émet une gerbe particulaire instantanée avec gradient de couleur et physique retombante
    pub fn emit_burst_at(&mut self, x: f32, y: f32, count: usize, color_start: u32, color_end: u32) {
        let max_allowed = self.adaptation.current_profile.max_particles();
        let to_spawn = count.min(max_allowed.saturating_sub(self.particles.len()));

        for i in 0..to_spawn {
            let angle = (i as f32) * (std::f32::consts::TAU / (to_spawn as f32));
            let speed = 75.0 + (i as f32 % 6.0) * 32.0;
            let life = 0.5 + (i as f32 % 4.0) * 0.15;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                ax: 0.0,
                ay: 65.0,
                life,
                max_life: life,
                color_start,
                color_end,
            });
        }
    }

    pub fn update(&mut self, dt: f32, input: &InputState) {
        self.elapsed_time += dt;
        let frame_start = std::time::Instant::now();
        let mut kvl_steps_this_frame = 0;

        let is_ctrl_or_cmd = input.is_key_down(KeyCode::ControlLeft)
            || input.is_key_down(KeyCode::ControlRight)
            || input.is_key_down(KeyCode::SuperLeft)
            || input.is_key_down(KeyCode::SuperRight);

        // Raccourci Sauvegarde Globale (Ctrl+S / Cmd+S)
        if is_ctrl_or_cmd && input.is_key_just_pressed(KeyCode::KeyS) {
            if let Err(e) = self.save_scene_and_scripts() {
                self.status_message = format!("Erreur lors de la sauvegarde : {}", e);
            }
            return;
        }

        // Bascule Son (Touche M)
        if input.is_key_just_pressed(KeyCode::KeyM) {
            let muted = self.audio.toggle_mute();
            self.status_message = if muted {
                "Audio procédural : MUET (Mute actif)".into()
            } else {
                "Audio procédural : ACTIF (Synthèse 44.1 kHz)".into()
            };
        }

        // Bascule Radar Tactique / Minimap (Touche N)
        if input.is_key_just_pressed(KeyCode::KeyN) {
            self.show_minimap = !self.show_minimap;
            self.audio.play(SoundType::RadarPing);
            self.status_message = if self.show_minimap {
                "Radar Tactique (Minimap) : ACTIVÉ".into()
            } else {
                "Radar Tactique (Minimap) : MASQUÉ".into()
            };
        }

        // Bascule Visualisation Grille Spatiale (Touche G)
        if input.is_key_just_pressed(KeyCode::KeyG) {
            let vis = self.spatial_grid.toggle_debug();
            self.status_message = if vis {
                "Grille Spatiale (Spatial Hash Grid) : AFFICHÉE".into()
            } else {
                "Grille Spatiale (Spatial Hash Grid) : MASQUÉE".into()
            };
        }

        // Bascule Guide Pédagogique Interactif (Touche H)
        if input.is_key_just_pressed(KeyCode::KeyH) {
            self.editor.toggle_help();
            self.audio.play(SoundType::UiClick);
            self.status_message = if self.editor.show_help_modal {
                "Guide pédagogique interactif : OUVERT (Appuyez sur H ou Echap pour fermer)".into()
            } else {
                "Guide pédagogique fermé.".into()
            };
            return;
        }

        // Fermer le guide avec Échap en mode jeu
        if self.editor.show_help_modal && (input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked) {
            self.editor.show_help_modal = false;
            self.audio.play(SoundType::UiClick);
            self.status_message = "Guide pédagogique fermé.".into();
            return;
        }

        // Clic sur le bouton d'aide ou fermeture par clic modal
        if input.mouse_just_clicked {
            let mx = input.mouse_x;
            let my = input.mouse_y;
            if self.editor.is_help_btn_clicked(mx, my) {
                self.editor.toggle_help();
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.show_help_modal {
                self.editor.show_help_modal = false;
                self.audio.play(SoundType::UiClick);
                return;
            }
        }

        // Bascule Mode Jeu / Mode Édition (TAB)
        if input.is_key_just_pressed(KeyCode::Tab) {
            self.editor.toggle_mode();
            self.status_message = match self.editor.mode {
                EditorMode::Play => "Mode Jeu actif ! (ZQSD/Flèches pour jouer | TAB pour éditer)".into(),
                EditorMode::Edit => "Mode Édition actif ! Cliquez sur une entité pour inspecter ou relier ses blocs KVL.".into(),
            };
            return;
        }

        // ==========================================
        // GESTION DU SYSTÈME DE DIALOGUE (MACHINE À ÉCRIRE ET CHOIX MULTIPLES)
        // ==========================================
        if self.dialogue.is_active {
            self.dialogue.update(dt, &mut self.audio);
            let dlg_action = self.dialogue.handle_input(input);
            match dlg_action {
                DialogueAction::ChoiceSelected(choice_idx) => {
                    self.kvl_context.set_var("dialogue_selected_choice", Value::Int(choice_idx as i64));
                    self.kvl_context.set_var("dialogue_active", Value::Bool(false));
                    self.audio.play(SoundType::UiClick);
                    self.status_message = format!("Choix [{}] validé auprès de {} !", choice_idx, self.dialogue.speaker);

                    // Si choix 1 (ex: acceptation quête), faire avancer l'étape de quête
                    if choice_idx == 1 {
                        if let Some(ref mut q) = self.quest {
                            let next = q.current_step + 1;
                            q.set_step(next);
                            self.kvl_context.set_var(format!("quest_{}", q.id), Value::Int(next));
                        }
                    }

                    // Déclencher l'événement KVL OnDialogueChoice sur tous les graphes chargés
                    for graph in self.kvl_graphs.values() {
                        if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, "OnDialogueChoice") {
                            kvl_steps_this_frame += steps;
                        }
                    }
                }
                DialogueAction::Closed => {
                    self.kvl_context.set_var("dialogue_active", Value::Bool(false));
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Discussion terminée.".into();

                    for graph in self.kvl_graphs.values() {
                        if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, "OnDialogueClose") {
                            kvl_steps_this_frame += steps;
                        }
                    }
                }
                _ => {}
            }
        }

        // Mise à jour du timer de notification de quête HUD
        if let Some(ref mut q) = self.quest {
            q.update(dt);
        }

        // ==========================================
        // GESTION DU MODE ÉDITION GÉNÉRIQUE
        // ==========================================
        if self.editor.mode == EditorMode::Edit {
            self.editor.mouse_x = input.mouse_x;
            self.editor.mouse_y = input.mouse_y;

            // Annuler le câblage, la palette ou l'aide avec Échap ou Clic droit
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
                if self.editor.show_help_modal {
                    self.editor.show_help_modal = false;
                    self.status_message = "Guide pédagogique fermé.".into();
                    return;
                }
                if self.editor.show_spawn_palette {
                    self.editor.show_spawn_palette = false;
                    self.status_message = "Palette de spawn fermée.".into();
                    return;
                }
                if self.editor.show_node_palette {
                    self.editor.show_node_palette = false;
                    self.status_message = "Palette de création fermée.".into();
                    return;
                }
                if self.editor.active_wire.is_some() {
                    self.editor.cancel_wire();
                    self.status_message = "Câblage annulé.".into();
                    return;
                }
            }

            // Raccourcis Outils Studio : V (Select), B (Pinceau), X (Gomme), T (Changer tuile)
            if input.is_key_just_pressed(KeyCode::KeyV) {
                self.editor.set_tool(StudioTool::Select);
                self.audio.play(SoundType::UiClick);
                self.status_message = "Outil [SELECT / INSPECT] actif (V) : Cliquez sur une entité pour inspecter ou déplacer.".into();
                return;
            }
            if input.is_key_just_pressed(KeyCode::KeyB) {
                self.editor.set_tool(StudioTool::Brush);
                self.audio.play(SoundType::UiClick);
                self.status_message = format!("Outil [PINCEAU] actif (B) : Peignez à la souris (Tuile : {} | T: Changer).", self.editor.brush_tile.label());
                return;
            }
            if input.is_key_just_pressed(KeyCode::KeyX) {
                self.editor.set_tool(StudioTool::Eraser);
                self.audio.play(SoundType::UiClick);
                self.status_message = "Outil [GOMME] actif (X) : Cliquez ou glissez pour effacer des blocs.".into();
                return;
            }
            if input.is_key_just_pressed(KeyCode::KeyT) {
                let next_tile = self.editor.cycle_brush_tile();
                self.audio.play(SoundType::UiClick);
                self.status_message = format!("Tuile active du pinceau : {} (T)", next_tile.label());
                return;
            }

            self.camera.set_viewport(20, 66, 790, 470);

            // Défilement de la Caméra Studio avec les touches Fléchées et recentrage C
            let pan_speed = 500.0 * dt;
            if input.is_key_down(KeyCode::ArrowLeft) {
                self.camera.pan(-pan_speed, 0.0);
            }
            if input.is_key_down(KeyCode::ArrowRight) {
                self.camera.pan(pan_speed, 0.0);
            }
            if input.is_key_down(KeyCode::ArrowUp) {
                self.camera.pan(0.0, -pan_speed);
            }
            if input.is_key_down(KeyCode::ArrowDown) {
                self.camera.pan(0.0, pan_speed);
            }
            if input.is_key_just_pressed(KeyCode::KeyC) {
                let center_target = match self.editor.selected {
                    SelectedEntity::Entity(idx) if idx < self.entities.len() => {
                        Some((self.entities[idx].transform.center_x(), self.entities[idx].transform.center_y()))
                    }
                    _ => self.player_entity().map(|p| (p.transform.center_x(), p.transform.center_y())),
                };
                if let Some((cx, cy)) = center_target {
                    self.camera.target_x = cx;
                    self.camera.target_y = cy;
                    self.camera.snap_to_target();
                    self.status_message = "Caméra Studio recentrée (C) !".into();
                }
            }
            self.camera.update(dt);

            let in_viewport = input.mouse_x >= 20.0 && input.mouse_x <= 810.0 && input.mouse_y >= 66.0 && input.mouse_y <= 536.0;

            // Clic droit rapide : Effacement d'entité sous le curseur dans le viewport (quel que soit l'outil actif)
            if input.mouse_right_just_clicked && in_viewport {
                let (world_mx, world_my) = self.camera.screen_to_world(input.mouse_x, input.mouse_y);
                let to_delete = self.entities.iter().enumerate().rposition(|(_, e)| {
                    let is_player = e.movement.as_ref().map_or(false, |m| m.is_player_controlled);
                    !is_player
                        && world_mx >= e.transform.x - 4.0
                        && world_mx <= e.transform.x + e.transform.w + 4.0
                        && world_my >= e.transform.y - 4.0
                        && world_my <= e.transform.y + e.transform.h + 4.0
                });
                if let Some(del_idx) = to_delete {
                    let name = self.entities[del_idx].name.clone();
                    let x = self.entities[del_idx].transform.x;
                    let y = self.entities[del_idx].transform.y;
                    self.entities.remove(del_idx);
                    self.editor.selected = SelectedEntity::None;
                    self.audio.play(SoundType::WireCut);
                    self.spawn_particles(x, y, 14, 0x00EF4444);
                    self.status_message = format!("Entité '{}' supprimée au clic droit.", name);
                    return;
                }
            }

            // Peinture continue au pinceau (StudioTool::Brush) sur grille 32x32 avec anti-doublon en coordonnées monde
            if self.editor.active_tool == StudioTool::Brush && input.mouse_pressed && in_viewport {
                let (world_mx, world_my) = self.camera.screen_to_world(input.mouse_x, input.mouse_y);
                let cell_col = (world_mx / 32.0).floor() as i32;
                let cell_row = (world_my / 32.0).floor() as i32;
                if cell_col >= 0 && cell_row >= 0 && self.editor.last_painted_cell != Some((cell_col, cell_row)) {
                    self.editor.last_painted_cell = Some((cell_col, cell_row));
                    let grid_x = cell_col as f32 * 32.0;
                    let grid_y = cell_row as f32 * 32.0;

                    let occupied = self.entities.iter().any(|e| {
                        (e.transform.x - grid_x).abs() < 16.0 && (e.transform.y - grid_y).abs() < 16.0
                    });

                    if !occupied {
                        let tile_id = self.editor.brush_tile.id();
                        self.spawn_entity(tile_id, grid_x, grid_y);
                        self.editor.selected = SelectedEntity::None;
                        self.editor.is_dragging = false;
                        self.status_message = format!("Tuile '{}' peinte en ({:.0}, {:.0}) !", self.editor.brush_tile.label(), grid_x, grid_y);
                    }
                }
            }

            // Effacement continu à la gomme (StudioTool::Eraser)
            if self.editor.active_tool == StudioTool::Eraser && input.mouse_pressed && in_viewport {
                let (world_mx, world_my) = self.camera.screen_to_world(input.mouse_x, input.mouse_y);
                let to_delete = self.entities.iter().enumerate().rposition(|(_, e)| {
                    let is_player = e.movement.as_ref().map_or(false, |m| m.is_player_controlled);
                    !is_player
                        && world_mx >= e.transform.x - 4.0
                        && world_mx <= e.transform.x + e.transform.w + 4.0
                        && world_my >= e.transform.y - 4.0
                        && world_my <= e.transform.y + e.transform.h + 4.0
                });
                if let Some(del_idx) = to_delete {
                    let name = self.entities[del_idx].name.clone();
                    let x = self.entities[del_idx].transform.x;
                    let y = self.entities[del_idx].transform.y;
                    self.entities.remove(del_idx);
                    self.editor.selected = SelectedEntity::None;
                    self.audio.play(SoundType::WireCut);
                    self.spawn_particles(x, y, 14, 0x00EF4444);
                    self.status_message = format!("Entité '{}' effacée à la gomme.", name);
                }
            }

            // Raccourci P : Basculer la palette de spawn d'entités
            if input.is_key_just_pressed(KeyCode::KeyP) {
                self.editor.toggle_spawn_palette();
                self.status_message = if self.editor.show_spawn_palette {
                    "Palette de spawn d'entités ouverte (P). Choisissez un gabarit.".into()
                } else {
                    "Palette de spawn fermée.".into()
                };
                return;
            }

            // Raccourcis clavier studio avec touches Ctrl ou Cmd (Mac)
            let ctrl_or_cmd = input.is_key_pressed(KeyCode::ControlLeft) || input.is_key_pressed(KeyCode::ControlRight)
                || input.is_key_pressed(KeyCode::SuperLeft) || input.is_key_pressed(KeyCode::SuperRight);

            // Ctrl+S : Sauvegarder la scène et scripts
            if ctrl_or_cmd && input.is_key_just_pressed(KeyCode::KeyS) {
                if let Err(e) = self.save_scene_and_scripts() {
                    self.status_message = format!("Erreur lors de la sauvegarde : {}", e);
                }
                return;
            }

            // Ctrl+D : Dupliquer l'entité sélectionnée
            if ctrl_or_cmd && input.is_key_just_pressed(KeyCode::KeyD) {
                self.duplicate_selected_entity();
                return;
            }

            // Suppr / Backspace : Supprimer l'entité sélectionnée
            if input.is_key_just_pressed(KeyCode::Delete) || input.is_key_just_pressed(KeyCode::Backspace) {
                self.remove_selected_entity();
                return;
            }

            if input.is_key_just_pressed(KeyCode::Space) {
                self.editor.toggle_view_detail();
                self.status_message = if self.editor.is_detailed_view {
                    "Inspecteur KVL : Vue détaillée technique dépliée (câblage actif)".into()
                } else {
                    "Inspecteur KVL : Vue simplifiée (macro-blocs)".into()
                };
            }

            // Gestion des clics en mode édition
            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                // Récupérer le script_path de l'entité sélectionnée si elle en possède un
                let selected_script_path = match self.editor.selected {
                    SelectedEntity::Entity(idx) if idx < self.entities.len() => {
                        self.entities[idx].script.as_ref().map(|s| s.script_path.clone())
                    }
                    _ => None,
                };

                // 00. Clic sur les boutons d'outils studio (Select, Brush, Eraser, Cycle Tile)
                if self.editor.is_tool_select_clicked(mx, my) {
                    self.editor.set_tool(StudioTool::Select);
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Outil [SELECT / INSPECT] actif (V) : Cliquez sur une entité pour inspecter ou déplacer.".into();
                    return;
                }
                if self.editor.is_tool_brush_clicked(mx, my) {
                    self.editor.set_tool(StudioTool::Brush);
                    self.audio.play(SoundType::UiClick);
                    self.status_message = format!("Outil [PINCEAU] actif (B) : Peignez à la souris (Tuile : {} | T: Changer).", self.editor.brush_tile.label());
                    return;
                }
                if self.editor.is_tool_eraser_clicked(mx, my) {
                    self.editor.set_tool(StudioTool::Eraser);
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Outil [GOMME] actif (X) : Cliquez ou glissez pour effacer des blocs.".into();
                    return;
                }
                if self.editor.is_tool_tile_cycle_clicked(mx, my) {
                    let tile = self.editor.cycle_brush_tile();
                    self.audio.play(SoundType::UiClick);
                    self.status_message = format!("Tuile active du pinceau : {} (T)", tile.label());
                    return;
                }

                // 0. Clic sur le bouton Sauvegarder
                if self.editor.is_save_btn_clicked(mx, my) {
                    if let Err(e) = self.save_scene_and_scripts() {
                        self.status_message = format!("Erreur lors de la sauvegarde : {}", e);
                    }
                    return;
                }

                // 0b. Clic sur le bouton Spawn (+ SPAWN)
                if self.editor.is_spawn_btn_clicked(mx, my) {
                    self.editor.toggle_spawn_palette();
                    self.status_message = if self.editor.show_spawn_palette {
                        "Palette de spawn d'entités ouverte : choisissez un gabarit.".into()
                    } else {
                        "Palette de spawn fermée.".into()
                    };
                    return;
                }

                // 0c. Clic sur le bouton Dupliquer (Ctrl+D)
                if self.editor.is_duplicate_btn_clicked(mx, my) {
                    self.duplicate_selected_entity();
                    return;
                }

                // 0d. Clic sur le bouton Supprimer (Suppr)
                if self.editor.is_delete_btn_clicked(mx, my) {
                    self.remove_selected_entity();
                    return;
                }

                // 0e. Si la palette de spawn est ouverte : clic sur un gabarit
                if self.editor.show_spawn_palette {
                    if let Some(item) = self.editor.find_spawn_palette_item_at(mx, my) {
                        let tpl_id = item.template_id.clone();
                        self.editor.show_spawn_palette = false;
                        if tpl_id != "cancel" {
                            let spawn_x = (mx - 16.0).clamp(40.0, 700.0);
                            let spawn_y = (my - 16.0).clamp(80.0, 560.0);
                            self.spawn_entity(&tpl_id, spawn_x, spawn_y);
                        } else {
                            self.status_message = "Spawn annulé.".into();
                        }
                    } else {
                        self.editor.show_spawn_palette = false;
                    }
                    return;
                }

                // 0f. Clic sur les boutons d'ajout de composants dans l'inspecteur
                if let Some(comp_name) = self.editor.find_add_comp_at(mx, my) {
                    let comp_name = comp_name.to_string();
                    self.add_component_to_selected(&comp_name);
                    return;
                }

                // 1. Clic sur le bouton [+ Nouveau Nœud] (Palette)
                if self.editor.is_add_node_btn_clicked(mx, my) {
                    if selected_script_path.is_some() {
                        self.editor.toggle_palette();
                        self.status_message = if self.editor.show_node_palette {
                            "Palette de nœuds ouverte : choisissez une action KVL.".into()
                        } else {
                            "Palette fermée.".into()
                        };
                    } else {
                        self.status_message = "Veuillez d'abord sélectionner une entité dotée d'un script KVL.".into();
                    }
                    return;
                }

                // 2. Si la palette est ouverte : gestion des clics sur la palette
                if self.editor.show_node_palette {
                    if let Some(item) = self.editor.find_palette_item_at(mx, my) {
                        let template_id = item.template_id.clone();
                        if template_id == "cancel" {
                            self.editor.show_node_palette = false;
                            self.status_message = "Ajout de nœud annulé.".into();
                            return;
                        }
                        if let Some(ref script_path) = selected_script_path {
                            if let Some(graph) = self.kvl_graphs.get_mut(script_path) {
                                let count = graph.nodes.len() + 1;
                                let (new_node, name) = match template_id.as_str() {
                                    "BonusPoints" => {
                                        let id = format!("bonus_pts_{}", count);
                                        let n = Node::new(&id, "Bonus Majeur (+50)", Category::Progression, "Accorde +50 points bonus au score", NodeKind::Action { action_type: "BonusPoints".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Bonus Majeur (+50)")
                                    }
                                    "BoostSpeed" => {
                                        let id = format!("boost_spd_{}", count);
                                        let n = Node::new(&id, "Boost Vitesse (+120)", Category::Movement, "Augmente la vitesse de déplacement du joueur", NodeKind::Action { action_type: "BoostSpeed".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Boost Vitesse (+120)")
                                    }
                                    "SpawnSparkles" => {
                                        let id = format!("sparkles_{}", count);
                                        let n = Node::new(&id, "Pluie Etincelles", Category::World, "Déclenche une pluie de particules étincelantes", NodeKind::Action { action_type: "SpawnSparkles".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Pluie Etincelles")
                                    }
                                    "ConditionScore50" => {
                                        let id = format!("cond_score_{}", count);
                                        let n = Node::new(&id, "Si Score >= 50", Category::MathLogic, "Vérifie si le score atteint 50 points", NodeKind::Condition { operator: "ScoreAtLeast50".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
                                            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
                                        (n, "Condition (Score >= 50)")
                                    }
                                    "DenyAccess" => {
                                        let id = format!("act_deny_{}", count);
                                        let n = Node::new(&id, "Alerte Refus Accès", Category::World, "Émet un refus d'accès sonore et visuel", NodeKind::Action { action_type: "DenyAccess".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Alerte Refus")
                                    }
                                    "TriggerAlarm" => {
                                        let id = format!("act_alarm_{}", count);
                                        let n = Node::new(&id, "Déclencher Alerte", Category::World, "Active l'état d'alerte et la sirène", NodeKind::Action { action_type: "TriggerAlarm".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Déclencher Alerte")
                                    }
                                    "LockAllDoors" => {
                                        let id = format!("act_lock_{}", count);
                                        let n = Node::new(&id, "Verrouiller Portes", Category::World, "Verrouille d'urgence les portes du complexe", NodeKind::Action { action_type: "LockAllDoors".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Verrouiller Portes")
                                    }
                                    "TeleportPlayer" => {
                                        let id = format!("act_teleport_{}", count);
                                        let n = Node::new(&id, "Téléportation (120, 480)", Category::Movement, "Téléporte instantanément l'entité", NodeKind::Action { action_type: "TeleportPlayer".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Téléportation")
                                    }
                                    "SlowSpeed" => {
                                        let id = format!("act_slow_{}", count);
                                        let n = Node::new(&id, "Ralentir Vitesse (-80)", Category::Movement, "Réduit la vitesse de déplacement", NodeKind::Action { action_type: "SlowSpeed".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Ralentir Vitesse")
                                    }
                                    "IncrementKeys" => {
                                        let id = format!("act_inc_keys_{}", count);
                                        let n = Node::new(&id, "Incrémenter Clés (+1)", Category::Progression, "Ajoute 1 au compteur de clés", NodeKind::Action { action_type: "IncrementCounter:keys".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
                                        (n, "Incrémenter Clés")
                                    }
                                    "ConditionKeys3" => {
                                        let id = format!("cond_keys_{}", count);
                                        let n = Node::new(&id, "Si Clés >= 3", Category::MathLogic, "Vérifie si 3 clés ont été réunies", NodeKind::Condition { operator: "CounterAtLeast:keys:3".into() })
                                            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
                                            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
                                            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
                                        (n, "Condition (Clés >= 3)")
                                    }
                                    _ => return,
                                };
                                let id_str = new_node.id.clone();
                                graph.add_node(new_node);
                                self.audio.play(SoundType::UiClick);
                                self.editor.show_node_palette = false;
                                self.status_message = format!("Nœud '{}' ({}) créé ! Reliez ses pins à la souris.", id_str, name);
                                return;
                            }
                        }
                    } else {
                        self.editor.show_node_palette = false;
                        return;
                    }
                }

                // 3. Clic sur le bouton de bascule [Vue Détaillée / Vue Simplifiée]
                if self.editor.is_view_btn_clicked(mx, my) {
                    self.editor.toggle_view_detail();
                    self.status_message = if self.editor.is_detailed_view {
                        "Inspecteur KVL : Vue détaillée technique dépliée (câblage actif)".into()
                    } else {
                        "Inspecteur KVL : Vue simplifiée (macro-blocs)".into()
                    };
                    return;
                }

                // 4. Si vue détaillée active : gestion du câblage KVL
                if self.editor.is_detailed_view {
                    if let Some(ref script_path) = selected_script_path {
                        // Couper une liaison
                        if let Some(cut_hit) = self.editor.find_connection_cut_at(mx, my) {
                            let conn_id = cut_hit.connection_id.clone();
                            if let Some(graph) = self.kvl_graphs.get_mut(script_path) {
                                if graph.remove_connection_by_id(&conn_id) {
                                    self.audio.play(SoundType::WireCut);
                                    self.status_message = format!("Connexion '{}' coupée avec succès !", conn_id);
                                    return;
                                }
                            }
                        }

                        // Broche (Pin)
                        if let Some(pin_hit) = self.editor.find_pin_at(mx, my) {
                            let node_id = pin_hit.node_id.clone();
                            let pin_id = pin_hit.pin_id.clone();
                            let is_output = pin_hit.is_output;
                            let sock_x = pin_hit.socket_x;
                            let sock_y = pin_hit.socket_y;

                            if let Some(wire) = self.editor.active_wire.take() {
                                if !is_output {
                                    if let Some(graph) = self.kvl_graphs.get_mut(script_path) {
                                        match graph.connect_pins(&wire.from_node, &wire.from_pin, &node_id, &pin_id) {
                                            Ok(_) => {
                                                self.audio.play(SoundType::UiClick);
                                                self.status_message = format!("Nouvelle liaison établie : {}:{} -> {}:{} !", wire.from_node, wire.from_pin, node_id, pin_id);
                                            }
                                            Err(err) => {
                                                self.status_message = format!("Liaison refusée : {}", err);
                                            }
                                        }
                                    }
                                } else if wire.from_node == node_id && wire.from_pin == pin_id {
                                    self.status_message = "Câblage annulé.".into();
                                } else {
                                    self.editor.start_wire(node_id.clone(), pin_id.clone(), sock_x, sock_y);
                                    self.status_message = format!("Nouvelle source de câblage : {}:{}", node_id, pin_id);
                                }
                            } else {
                                if is_output {
                                    self.editor.start_wire(node_id.clone(), pin_id.clone(), sock_x, sock_y);
                                    self.status_message = format!("Fil de câblage actif depuis {}:{}. Cliquez sur une entrée.", node_id, pin_id);
                                } else {
                                    if let Some(graph) = self.kvl_graphs.get_mut(script_path) {
                                        let removed = graph.disconnect_pin(&node_id, &pin_id);
                                        if removed > 0 {
                                            self.audio.play(SoundType::WireCut);
                                            self.status_message = format!("Broche {}:{} déconnectée ({} fil retiré).", node_id, pin_id, removed);
                                        } else {
                                            self.status_message = format!("Broche {}:{} libre (prête à recevoir un fil).", node_id, pin_id);
                                        }
                                    }
                                }
                            }
                            return;
                        }
                    }

                    if self.editor.active_wire.is_some() {
                        self.editor.cancel_wire();
                        self.status_message = "Câblage annulé.".into();
                        return;
                    }
                }

                // 5. Sélection universelle d'entité à la souris en coordonnées monde
                if self.editor.active_tool == StudioTool::Select {
                    let in_game_viewport = mx >= 20.0 && mx <= 810.0 && my >= 66.0 && my <= 536.0;
                    if in_game_viewport {
                        let (world_mx, world_my) = self.camera.screen_to_world(mx, my);
                        let mut found_idx = None;
                        for (idx, ent) in self.entities.iter().enumerate().rev() {
                            let t = &ent.transform;
                            if world_mx >= t.x - 8.0 && world_mx <= t.x + t.w + 8.0 && world_my >= t.y - 8.0 && world_my <= t.y + t.h + 8.0 {
                                found_idx = Some(idx);
                                break;
                            }
                        }

                        if let Some(idx) = found_idx {
                            self.editor.selected = SelectedEntity::Entity(idx);
                            self.editor.is_dragging = true;
                            self.editor.drag_offset_x = self.entities[idx].transform.x - world_mx;
                            self.editor.drag_offset_y = self.entities[idx].transform.y - world_my;
                            self.status_message = format!("Entité '{}' ({}) sélectionnée.", self.entities[idx].name, self.entities[idx].tag);
                            return;
                        }

                        // Clic dans la zone de jeu vide -> désélectionner
                        self.editor.selected = SelectedEntity::None;
                    }
                }
            }

            if input.mouse_just_released {
                self.editor.is_dragging = false;
                self.editor.last_painted_cell = None;
            }

            // Déplacement universel de l'entité sélectionnée à la souris en coordonnées monde (Outil Select uniquement)
            if self.editor.active_tool == StudioTool::Select && self.editor.is_dragging && input.mouse_pressed {
                if let SelectedEntity::Entity(idx) = self.editor.selected {
                    if idx < self.entities.len() {
                        let (world_mx, world_my) = self.camera.screen_to_world(input.mouse_x, input.mouse_y);
                        let ew = self.entities[idx].transform.w;
                        let eh = self.entities[idx].transform.h;
                        let new_x = (world_mx + self.editor.drag_offset_x).clamp(self.world_bounds.0, self.world_bounds.2 - ew);
                        let new_y = (world_my + self.editor.drag_offset_y).clamp(self.world_bounds.1, self.world_bounds.3 - eh);
                        self.entities[idx].transform.x = new_x;
                        self.entities[idx].transform.y = new_y;
                    }
                }
            }

            return;
        }

        // ==========================================
        // GESTION DU MODE JEU
        // ==========================================
        if input.is_key_just_pressed(KeyCode::F1) {
            self.telemetry.toggle_overlay();
        }
        if input.is_key_just_pressed(KeyCode::F2) {
            self.adaptation.cycle_profile_manual();
            self.status_message = format!("Profil forcé : {}", self.adaptation.current_profile.name());
        }
        if input.is_key_just_pressed(KeyCode::F3) {
            self.adaptation.toggle_auto_mode();
            self.status_message = if self.adaptation.auto_adapt {
                "Adaptation dynamique : ACTIVEE".into()
            } else {
                "Adaptation dynamique : MANUELLE".into()
            };
        }
        if input.is_key_just_pressed(KeyCode::F4) || input.is_key_just_pressed(KeyCode::KeyL) {
            self.cycle_scene_forward();
            return;
        }
        if input.is_key_just_pressed(KeyCode::F5) {
            self.cycle_scene_backward();
            return;
        }
        if input.is_key_just_pressed(KeyCode::KeyR) {
            self.reload();
            return;
        }

        // Action de tir universelle (Touche F ou Espace en mode Spatial)
        let is_f = input.is_key_just_pressed(KeyCode::KeyF);
        let is_space = input.is_key_just_pressed(KeyCode::Space);
        let is_space_mode = self.player_entity().and_then(|p| p.movement.as_ref()).map_or(false, |m| matches!(m.mode, MovementMode::SpaceInertia { .. }));
        if is_f || (is_space && is_space_mode) {
            self.fire_player_projectile();
        }

        // Gestion de l'invulnérabilité (i-frames) pour tous les composants Health2D
        for ent in &mut self.entities {
            if let Some(ref mut h) = ent.health {
                if h.invulnerability_timer > 0.0 {
                    h.invulnerability_timer = (h.invulnerability_timer - dt).max(0.0);
                }
            }
        }

        // Déplacement joueur via vecteur d'entrée
        let mut move_x: f32 = 0.0;
        let mut move_y: f32 = 0.0;

        if input.is_key_down(KeyCode::ArrowLeft) || input.is_key_down(KeyCode::KeyA) || input.is_key_down(KeyCode::KeyQ) {
            move_x -= 1.0;
        }
        if input.is_key_down(KeyCode::ArrowRight) || input.is_key_down(KeyCode::KeyD) {
            move_x += 1.0;
        }
        if input.is_key_down(KeyCode::ArrowUp) || input.is_key_down(KeyCode::KeyW) || input.is_key_down(KeyCode::KeyZ) {
            move_y -= 1.0;
        }
        if !is_ctrl_or_cmd && (input.is_key_down(KeyCode::ArrowDown) || input.is_key_down(KeyCode::KeyS)) {
            move_y += 1.0;
        }

        let len: f32 = (move_x * move_x + move_y * move_y).sqrt();
        if len > 0.0 {
            move_x /= len;
            move_y /= len;
        }

        // Si une discussion est active, le joueur reste attentif et immobile
        if self.dialogue.is_active {
            move_x = 0.0;
            move_y = 0.0;
        }

        let speed_boost = match self.kvl_context.get_var("speed_boost") {
            Some(Value::Int(b)) => *b as f32,
            _ => 0.0,
        };

        // ==========================================
        // SYSTEM 1 : PARTITIONNEMENT SPATIAL (ZERO-ALLOCATION)
        // ==========================================
        self.spatial_grid.clear();
        for (idx, ent) in self.entities.iter().enumerate() {
            if !ent.is_active {
                continue;
            }
            if let Some(ref col) = ent.collider {
                let kind = if col.is_solid {
                    SpatialKind::Solid
                } else if col.is_trigger {
                    SpatialKind::Trigger
                } else {
                    SpatialKind::Entity(idx)
                };
                self.spatial_grid.insert(SpatialItem {
                    id: idx,
                    kind,
                    x: ent.transform.x + col.offset_x,
                    y: ent.transform.y + col.offset_y,
                    w: col.w,
                    h: col.h,
                });
            }
        }

        // ==========================================
        // SYSTEM 2 : PHYSIQUE ET GLISSEMENT AABB POUR LE JOUEUR (MULTI-GENRES)
        // ==========================================
        let mut player_pos = None;
        if let Some(p_idx) = self.player_index() {
            let (px, py, pw, ph, speed, p_mode) = {
                let p = &self.entities[p_idx];
                let speed = p.movement.as_ref().map_or(240.0, |m| m.speed) + speed_boost;
                let mode = p.movement.as_ref().map_or(MovementMode::TopDown, |m| m.mode);
                (p.transform.x, p.transform.y, p.transform.w, p.transform.h, speed, mode)
            };

            let mut cur_x = px;
            let mut cur_y = py;

            match p_mode {
                MovementMode::TopDown => {
                    // 1. Contrôleur Top-Down / Aventure / RPG (déplacement libre 4/8 axes)
                    if move_x != 0.0 {
                        cur_x += move_x * speed * dt;
                        cur_x = cur_x.clamp(30.0, 1160.0 - pw);

                        let mut cand_buf = [SpatialItem { id: 0, kind: SpatialKind::Solid, x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; 16];
                        let cand_count = {
                            let raw = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                            let len = raw.len().min(16);
                            cand_buf[..len].copy_from_slice(&raw[..len]);
                            len
                        };

                        for item in &cand_buf[..cand_count] {
                            if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                                let is_pushable = self.entities.get(item.id).and_then(|e| e.collider.as_ref()).map_or(false, |c| c.is_pushable);
                                if is_pushable {
                                    let bw = self.entities[item.id].transform.w;
                                    let bh = self.entities[item.id].transform.h;
                                    let bx = self.entities[item.id].transform.x;
                                    let by = self.entities[item.id].transform.y;
                                    let push_dx = move_x * speed * dt;
                                    let target_bx = bx + push_dx;
                                    let behind = self.spatial_grid.query_aabb(target_bx, by, bw, bh);
                                    let blocked = behind.iter().any(|obs| obs.id != item.id && obs.id != p_idx && matches!(obs.kind, SpatialKind::Solid));
                                    if !blocked && target_bx >= 30.0 && target_bx + bw <= 1160.0 {
                                        self.entities[item.id].transform.x = target_bx;
                                        self.audio.play(SoundType::BlockPush);
                                    } else if move_x > 0.0 {
                                        cur_x = item.x - pw;
                                    } else if move_x < 0.0 {
                                        cur_x = item.x + item.w;
                                    }
                                } else if move_x > 0.0 {
                                    cur_x = item.x - pw;
                                } else if move_x < 0.0 {
                                    cur_x = item.x + item.w;
                                }
                            }
                        }
                    }

                    if move_y != 0.0 {
                        cur_y += move_y * speed * dt;
                        cur_y = cur_y.clamp(75.0, 595.0 - ph);

                        let mut cand_buf = [SpatialItem { id: 0, kind: SpatialKind::Solid, x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; 16];
                        let cand_count = {
                            let raw = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                            let len = raw.len().min(16);
                            cand_buf[..len].copy_from_slice(&raw[..len]);
                            len
                        };

                        for item in &cand_buf[..cand_count] {
                            if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                                let is_pushable = self.entities.get(item.id).and_then(|e| e.collider.as_ref()).map_or(false, |c| c.is_pushable);
                                if is_pushable {
                                    let bw = self.entities[item.id].transform.w;
                                    let bh = self.entities[item.id].transform.h;
                                    let bx = self.entities[item.id].transform.x;
                                    let by = self.entities[item.id].transform.y;
                                    let push_dy = move_y * speed * dt;
                                    let target_by = by + push_dy;
                                    let behind = self.spatial_grid.query_aabb(bx, target_by, bw, bh);
                                    let blocked = behind.iter().any(|obs| obs.id != item.id && obs.id != p_idx && matches!(obs.kind, SpatialKind::Solid));
                                    if !blocked && target_by >= 75.0 && target_by + bh <= 595.0 {
                                        self.entities[item.id].transform.y = target_by;
                                        self.audio.play(SoundType::BlockPush);
                                    } else if move_y > 0.0 {
                                        cur_y = item.y - ph;
                                    } else if move_y < 0.0 {
                                        cur_y = item.y + item.h;
                                    }
                                } else if move_y > 0.0 {
                                    cur_y = item.y - ph;
                                } else if move_y < 0.0 {
                                    cur_y = item.y + item.h;
                                }
                            }
                        }
                    }

                    if let Some(ref mut mov) = self.entities[p_idx].movement {
                        mov.vx = move_x * speed;
                        mov.vy = move_y * speed;
                    }
                }

                MovementMode::Platformer { gravity, jump_force, mut is_grounded, max_fall_speed } => {
                    // 2. Contrôleur Plateforme 2D (Side-Scroller avec Gravité & Saut)
                    let horiz_dir = if input.is_key_down(KeyCode::ArrowLeft) || input.is_key_down(KeyCode::KeyA) || input.is_key_down(KeyCode::KeyQ) {
                        -1.0
                    } else if input.is_key_down(KeyCode::ArrowRight) || input.is_key_down(KeyCode::KeyD) {
                        1.0
                    } else {
                        0.0
                    };

                    let mut cur_vy = self.entities[p_idx].movement.as_ref().map_or(0.0, |m| m.vy);

                    // Saut : déclenché par Espace ou Flèche Haut ou Z / W si posé au sol
                    let jump_key = input.is_key_just_pressed(KeyCode::Space)
                        || input.is_key_just_pressed(KeyCode::ArrowUp)
                        || input.is_key_just_pressed(KeyCode::KeyW)
                        || input.is_key_just_pressed(KeyCode::KeyZ);

                    if jump_key && is_grounded {
                        cur_vy = -jump_force;
                        self.audio.play(SoundType::PlayerJump);
                    }

                    // Application de la pesanteur
                    cur_vy += gravity * dt;
                    cur_vy = cur_vy.min(max_fall_speed);

                    // Résolution horizontale X
                    if horiz_dir != 0.0 {
                        cur_x += horiz_dir * speed * dt;
                        cur_x = cur_x.clamp(30.0, 1160.0 - pw);

                        let candidates = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                        for item in candidates {
                            if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                                if horiz_dir > 0.0 {
                                    cur_x = item.x - pw;
                                } else if horiz_dir < 0.0 {
                                    cur_x = item.x + item.w;
                                }
                            }
                        }
                    }

                    // Résolution verticale Y (atterrissage ou plafond)
                    cur_y += cur_vy * dt;
                    cur_y = cur_y.clamp(75.0, 595.0 - ph);

                    let mut grounded_this_frame = false;
                    let candidates = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                    for item in candidates {
                        if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                            if cur_vy > 0.0 {
                                // Contact avec le sol sous les pieds
                                cur_y = item.y - ph;
                                cur_vy = 0.0;
                                grounded_this_frame = true;
                            } else if cur_vy < 0.0 {
                                // Heurt de la tête contre un plafond
                                cur_y = item.y + item.h;
                                cur_vy = 0.0;
                            }
                        }
                    }

                    // Support du bas du viewport comme plancher
                    if cur_y >= (590.0 - ph) {
                        grounded_this_frame = true;
                        cur_vy = 0.0;
                    }

                    is_grounded = grounded_this_frame;

                    if let Some(ref mut mov) = self.entities[p_idx].movement {
                        mov.vx = horiz_dir * speed;
                        mov.vy = cur_vy;
                        mov.mode = MovementMode::Platformer {
                            gravity,
                            jump_force,
                            is_grounded,
                            max_fall_speed,
                        };
                    }
                }

                MovementMode::SpaceInertia { angular_speed, thrust, drag } => {
                    // 3. Contrôleur Vaisseau Spatial / Asteroids / Shmup (Inertie vectorielle et poussée)
                    let rot_dir = if input.is_key_down(KeyCode::ArrowLeft) || input.is_key_down(KeyCode::KeyA) || input.is_key_down(KeyCode::KeyQ) {
                        -1.0
                    } else if input.is_key_down(KeyCode::ArrowRight) || input.is_key_down(KeyCode::KeyD) {
                        1.0
                    } else {
                        0.0
                    };

                    self.entities[p_idx].transform.rotation += rot_dir * angular_speed * dt;
                    let angle = self.entities[p_idx].transform.rotation;

                    let is_thrusting = input.is_key_down(KeyCode::ArrowUp)
                        || input.is_key_down(KeyCode::KeyW)
                        || input.is_key_down(KeyCode::KeyZ);

                    let mut cur_vx = self.entities[p_idx].movement.as_ref().map_or(0.0, |m| m.vx);
                    let mut cur_vy = self.entities[p_idx].movement.as_ref().map_or(0.0, |m| m.vy);

                    if is_thrusting {
                        cur_vx += angle.cos() * thrust * dt;
                        cur_vy += angle.sin() * thrust * dt;
                        let cur_speed = (cur_vx * cur_vx + cur_vy * cur_vy).sqrt();
                        if cur_speed > speed {
                            cur_vx = (cur_vx / cur_speed) * speed;
                            cur_vy = (cur_vy / cur_speed) * speed;
                        }
                    }

                    // Amortissement de dérive (friction spatiale)
                    let damping = drag.powf(dt * 60.0);
                    cur_vx *= damping;
                    cur_vy *= damping;

                    cur_x += cur_vx * dt;
                    cur_x = cur_x.clamp(30.0, 1160.0 - pw);
                    let candidates_x = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                    for item in candidates_x {
                        if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                            cur_vx = -cur_vx * 0.5;
                            if cur_vx > 0.0 { cur_x = item.x + item.w; } else { cur_x = item.x - pw; }
                        }
                    }

                    cur_y += cur_vy * dt;
                    cur_y = cur_y.clamp(75.0, 595.0 - ph);
                    let candidates_y = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                    for item in candidates_y {
                        if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                            cur_vy = -cur_vy * 0.5;
                            if cur_vy > 0.0 { cur_y = item.y + item.h; } else { cur_y = item.y - ph; }
                        }
                    }

                    if let Some(ref mut mov) = self.entities[p_idx].movement {
                        mov.vx = cur_vx;
                        mov.vy = cur_vy;
                    }
                }

                MovementMode::Grid { cell_size, cooldown, mut timer } => {
                    // 4. Contrôleur Déplacement Grille / Sokoban (déplacement quantifié case par case)
                    timer -= dt;
                    let mut step_x = 0.0;
                    let mut step_y = 0.0;

                    if timer <= 0.0 {
                        if input.is_key_just_pressed(KeyCode::ArrowLeft) || input.is_key_just_pressed(KeyCode::KeyA) || input.is_key_just_pressed(KeyCode::KeyQ) {
                            step_x = -cell_size;
                            timer = cooldown;
                        } else if input.is_key_just_pressed(KeyCode::ArrowRight) || input.is_key_just_pressed(KeyCode::KeyD) {
                            step_x = cell_size;
                            timer = cooldown;
                        } else if input.is_key_just_pressed(KeyCode::ArrowUp) || input.is_key_just_pressed(KeyCode::KeyW) || input.is_key_just_pressed(KeyCode::KeyZ) {
                            step_y = -cell_size;
                            timer = cooldown;
                        } else if input.is_key_just_pressed(KeyCode::ArrowDown) || input.is_key_just_pressed(KeyCode::KeyS) {
                            step_y = cell_size;
                            timer = cooldown;
                        }
                    }

                    if step_x != 0.0 || step_y != 0.0 {
                        let target_x = (cur_x + step_x).clamp(30.0, 1160.0 - pw);
                        let target_y = (cur_y + step_y).clamp(75.0, 595.0 - ph);

                        let mut cand_buf = [SpatialItem { id: 0, kind: SpatialKind::Solid, x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; 16];
                        let cand_count = {
                            let raw = self.spatial_grid.query_aabb(target_x, target_y, pw, ph);
                            let len = raw.len().min(16);
                            cand_buf[..len].copy_from_slice(&raw[..len]);
                            len
                        };

                        let mut can_step = true;
                        for item in &cand_buf[..cand_count] {
                            if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                                let is_pushable = self.entities.get(item.id).and_then(|e| e.collider.as_ref()).map_or(false, |c| c.is_pushable);
                                if is_pushable {
                                    let bw = self.entities[item.id].transform.w;
                                    let bh = self.entities[item.id].transform.h;
                                    let bx = self.entities[item.id].transform.x;
                                    let by = self.entities[item.id].transform.y;
                                    let b_target_x = bx + step_x;
                                    let b_target_y = by + step_y;
                                    let behind = self.spatial_grid.query_aabb(b_target_x, b_target_y, bw, bh);
                                    let blocked = behind.iter().any(|obs| obs.id != item.id && obs.id != p_idx && matches!(obs.kind, SpatialKind::Solid));
                                    if !blocked && b_target_x >= 30.0 && b_target_x + bw <= 1160.0 && b_target_y >= 75.0 && b_target_y + bh <= 595.0 {
                                        self.entities[item.id].transform.x = b_target_x;
                                        self.entities[item.id].transform.y = b_target_y;
                                        self.audio.play(SoundType::BlockPush);
                                    } else {
                                        can_step = false;
                                    }
                                } else {
                                    can_step = false;
                                }
                            }
                        }
                        if can_step {
                            cur_x = target_x;
                            cur_y = target_y;
                        }
                    }

                    if let Some(ref mut mov) = self.entities[p_idx].movement {
                        mov.vx = if step_x != 0.0 { speed } else { 0.0 };
                        mov.vy = if step_y != 0.0 { speed } else { 0.0 };
                        mov.mode = MovementMode::Grid { cell_size, cooldown, timer };
                    }
                }
            }

            self.entities[p_idx].transform.x = cur_x;
            self.entities[p_idx].transform.y = cur_y;
            player_pos = Some((p_idx, cur_x, cur_y, pw, ph));
        }

        // ==========================================
        // SYSTEM 2.5 : SIMULATION DES PROJECTILES BALISTIQUES (ZERO-ALLOCATION)
        // ==========================================
        let mut hit_events = Vec::new();
        for (idx, ent) in self.entities.iter_mut().enumerate() {
            if !ent.is_active {
                continue;
            }
            if let Some(ref mut proj) = ent.projectile {
                proj.lifetime += dt;
                if proj.lifetime >= proj.max_lifetime {
                    ent.is_active = false;
                    continue;
                }

                if let Some(ref mov) = ent.movement {
                    ent.transform.x += mov.vx * dt;
                    ent.transform.y += mov.vy * dt;
                }

                let bx = ent.transform.x;
                let by = ent.transform.y;
                let bw = ent.transform.w;
                let bh = ent.transform.h;

                let candidates = self.spatial_grid.query_aabb(bx, by, bw, bh);
                for item in candidates {
                    if item.id == proj.source_id || item.id == idx {
                        continue;
                    }
                    if matches!(item.kind, SpatialKind::Solid) {
                        ent.is_active = false;
                        hit_events.push((idx, item.id, false));
                        break;
                    } else if let SpatialKind::Entity(other_idx) = item.kind {
                        ent.is_active = false;
                        hit_events.push((idx, other_idx, true));
                        break;
                    }
                }
            }
        }

        for (_, target_idx, is_entity) in hit_events {
            if is_entity && target_idx < self.entities.len() {
                if self.entities[target_idx].tag == "enemy" {
                    self.entities[target_idx].is_active = false;
                    self.audio.play(SoundType::BonusFanfare);
                    let cur_score = match self.kvl_context.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };
                    self.kvl_context.set_var("score", Value::Int(cur_score + 25));
                    self.status_message = "Cible ennemie neutralisée par tir balistique ! (+25 pts)".into();
                }
            } else {
                self.audio.play(SoundType::Sparkles);
            }
        }

        // ==========================================
        // SYSTEM 3 : PATROUILLE DES ENNEMIS / SENTINELLES
        // ==========================================
        for ent in &mut self.entities {
            if !ent.is_active {
                continue;
            }
            if let Some(ref mut mov) = ent.movement {
                if let Some(ref mut patrol) = mov.patrol {
                    ent.transform.y += mov.speed * patrol.dir * dt;
                    if ent.transform.y >= patrol.max_y {
                        ent.transform.y = patrol.max_y;
                        patrol.dir = -1.0;
                    } else if ent.transform.y <= patrol.min_y {
                        ent.transform.y = patrol.min_y;
                        patrol.dir = 1.0;
                    }
                }
            }
        }

        // ==========================================
        // SYSTEM : SIMULATION DES ÉMETTEURS DE PARTICULES ECS
        // ==========================================
        let max_particles = self.adaptation.current_profile.max_particles();
        for ent in &mut self.entities {
            if !ent.is_active {
                continue;
            }
            if let Some(ref mut emitter) = ent.emitter {
                if !emitter.is_emitting {
                    continue;
                }
                match emitter.mode {
                    crate::entity::EmitterMode::Continuous => {
                        emitter.accumulator += dt;
                        let interval = 1.0 / emitter.rate.max(0.1);
                        let center_x = ent.transform.center_x();
                        let center_y = ent.transform.center_y();

                        while emitter.accumulator >= interval {
                            emitter.accumulator -= interval;
                            if self.particles.len() >= max_particles {
                                break;
                            }
                            // Pseudo-aléatoire déterministe inline sans allocation
                            let count_f = self.particles.len() as f32;
                            let seed1 = ((count_f * 29.3 + emitter.accumulator * 137.9).sin() * 43758.547).fract();
                            let seed2 = ((count_f * 47.1 + emitter.accumulator * 211.3).cos() * 43758.547).fract();

                            let spread_angle = emitter.angle + (seed1 - 0.5) * emitter.spread;
                            let speed = emitter.speed_min + seed2.abs() * (emitter.speed_max - emitter.speed_min);
                            let life = emitter.lifetime_min + seed1.abs() * (emitter.lifetime_max - emitter.lifetime_min);

                            self.particles.push(Particle {
                                x: center_x,
                                y: center_y,
                                vx: spread_angle.cos() * speed,
                                vy: spread_angle.sin() * speed,
                                ax: emitter.gravity_x,
                                ay: emitter.gravity_y,
                                life,
                                max_life: life,
                                color_start: emitter.color_start,
                                color_end: emitter.color_end,
                            });
                        }
                    }
                    crate::entity::EmitterMode::Burst => {}
                }
            }
        }

        // Mise à jour physique des particules (Vitesse, Accélération gravitationnelle, Durée de vie)
        for p in &mut self.particles {
            p.vx += p.ax * dt;
            p.vy += p.ay * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);

        let mut spawn_requests = Vec::new();

        // ==========================================
        // SYSTEM 4 : TRIGGER OVERLAP & COLLECTIBLES (PIÈCES, CŒURS, PIÈGES)
        // ==========================================
        if let Some((p_idx, px, py, pw, ph)) = player_pos {
            let mut collected_indices = Vec::new();
            let mut heart_indices = Vec::new();
            let mut hazard_hit = false;
            {
                let candidates = self.spatial_grid.query_aabb(px - 2.0, py - 2.0, pw + 4.0, ph + 4.0);
                for item in candidates {
                    if matches!(item.kind, SpatialKind::Trigger) {
                        let idx = item.id;
                        if idx < self.entities.len() && self.entities[idx].is_active {
                            let ent = &self.entities[idx];
                            // Distance centre à centre
                            let p_cx = px + pw * 0.5;
                            let p_cy = py + ph * 0.5;
                            let dx = p_cx - ent.transform.center_x();
                            let dy = p_cy - ent.transform.center_y();
                            let dist = (dx * dx + dy * dy).sqrt();

                            let max_reach = (pw.max(ph) * 0.5) + (ent.transform.w.max(ent.transform.h) * 0.5);
                            let is_collectible = ent.tag == "collectible" || ent.tag == "coin";
                            let is_heart = ent.tag == "heart" || ent.name.contains("heart");
                            let is_hazard = ent.tag == "hazard" || ent.tag == "trap" || ent.tag == "spike";

                            if dist < max_reach {
                                if is_collectible {
                                    collected_indices.push(idx);
                                } else if is_heart {
                                    heart_indices.push(idx);
                                } else if is_hazard {
                                    hazard_hit = true;
                                }
                            }
                        }
                    }
                }
            }

            // Contact hostile direct avec les ennemis patrouilleurs ou sentinelles
            let mut enemy_contact = false;
            for (idx, ent) in self.entities.iter().enumerate() {
                if idx != p_idx && ent.is_active && ent.tag == "enemy" {
                    let ex = ent.transform.x;
                    let ey = ent.transform.y;
                    let ew = ent.transform.w;
                    let eh = ent.transform.h;
                    if px < ex + ew && px + pw > ex && py < ey + eh && py + ph > ey {
                        enemy_contact = true;
                        break;
                    }
                }
            }

            // Gestion de l'absorption des cœurs de santé
            for idx in heart_indices {
                let (ent_name, ent_cx, ent_cy) = {
                    let ent = &mut self.entities[idx];
                    ent.is_active = false;
                    (ent.name.clone(), ent.transform.center_x(), ent.transform.center_y())
                };

                if let Some(ref mut h) = self.entities[p_idx].health {
                    h.heal(1.0);
                    self.audio.play(SoundType::HealPickup);
                    spawn_requests.push((ent_cx, ent_cy, 22, 0x0010B981));
                    self.status_message = format!("Fiole de vitalité '{}' absorbée ! ({:.0}/{:.0} PV)", ent_name, h.current, h.max);
                }
            }

            // Gestion des dégâts subis (Pièges ou Ennemis)
            if hazard_hit || enemy_contact {
                let p_cx = px + pw * 0.5;
                let p_cy = py + ph * 0.5;
                let mut took_damage = false;
                let mut is_dead = false;
                let mut cur_hp = 0.0;
                let mut max_hp = 0.0;

                if let Some(ref mut h) = self.entities[p_idx].health {
                    if h.damage(1.0) {
                        took_damage = true;
                        is_dead = h.is_dead;
                        if is_dead {
                            h.current = h.max;
                            h.is_dead = false;
                            h.invulnerability_timer = 1.5;
                        }
                        cur_hp = h.current;
                        max_hp = h.max;
                    }
                }

                if took_damage {
                    self.audio.play(SoundType::PlayerDamage);
                    spawn_requests.push((p_cx, p_cy, 25, 0x00EF4444));

                    if is_dead {
                        self.camera.add_shake(16.0);
                        self.audio.play(SoundType::PlayerDeath);
                        self.entities[p_idx].transform.x = self.player_spawn_pos.0;
                        self.entities[p_idx].transform.y = self.player_spawn_pos.1;
                        if let Some(ref mut mov) = self.entities[p_idx].movement {
                            mov.vx = 0.0;
                            mov.vy = 0.0;
                        }
                        self.camera.target_x = self.player_spawn_pos.0;
                        self.camera.target_y = self.player_spawn_pos.1;
                        self.camera.snap_to_target();
                        spawn_requests.push((self.player_spawn_pos.0, self.player_spawn_pos.1, 40, 0x00F59E0B));
                        self.status_message = "VOUS AVEZ PÉRI ! Réapparition au point de départ...".into();
                    } else {
                        self.camera.add_shake(8.0);
                        let source = if hazard_hit { "Piège" } else { "Contact hostile" };
                        self.status_message = format!("AÏE ! Dégâts subis ({}) ! Santé restante : {:.0}/{:.0} PV", source, cur_hp, max_hp);
                    }
                }
            }

            for idx in collected_indices {
                let (ent_name, ent_cx, ent_cy, script_opt) = {
                    let ent = &mut self.entities[idx];
                    ent.is_active = false;
                    (ent.name.clone(), ent.transform.center_x(), ent.transform.center_y(), ent.script.clone())
                };

                let prev_score = match self.kvl_context.get_var("score") {
                    Some(Value::Int(s)) => *s,
                    _ => 0,
                };
                let prev_boost = match self.kvl_context.get_var("speed_boost") {
                    Some(Value::Int(b)) => *b,
                    _ => 0,
                };

                if let Some(ref script_comp) = script_opt {
                    if let Some(ref evt) = script_comp.on_overlap_event {
                        if let Some(graph) = self.kvl_graphs.get(&script_comp.script_path) {
                            if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, evt) {
                                kvl_steps_this_frame += steps;
                            }
                        }
                    }
                }

                spawn_requests.push((ent_cx, ent_cy, 25, 0x00FDE047));

                let score = match self.kvl_context.get_var("score") {
                    Some(Value::Int(s)) => *s,
                    _ => 0,
                };
                let new_boost = match self.kvl_context.get_var("speed_boost") {
                    Some(Value::Int(b)) => *b,
                    _ => 0,
                };

                if new_boost > prev_boost {
                    self.audio.play(SoundType::SpeedBoost);
                } else if score >= prev_score + 50 {
                    self.audio.play(SoundType::BonusFanfare);
                } else {
                    self.audio.play(SoundType::Coin);
                }

                self.status_message = format!("Entité '{}' collectée ! Score KVL : {}", ent_name, score);
            }

            // Synchronisation de la santé joueur avec le contexte KVL
            if let Some(ref h) = self.entities[p_idx].health {
                self.kvl_context.set_var("player_hp", Value::Int(h.current as i64));
                self.kvl_context.set_var("player_max_hp", Value::Int(h.max as i64));
            }
        }

        // ==========================================
        // SYSTEM 4.5 : GESTION DES DALLES DE PRESSION ET PUZZLES ENVIRONNEMENTAUX
        // ==========================================
        let mut plates_pressed = 0;
        let mut total_plates = 0;
        let mut plates_covered = Vec::new();

        for (idx, ent) in self.entities.iter().enumerate() {
            if ent.tag == "plate" && ent.is_active {
                total_plates += 1;
                let px = ent.transform.x;
                let py = ent.transform.y;
                let pw = ent.transform.w;
                let ph = ent.transform.h;

                let candidates = self.spatial_grid.query_aabb(px, py, pw, ph);
                let has_crate = candidates.iter().any(|c| {
                    c.id != idx && self.entities.get(c.id).map_or(false, |e| {
                        e.is_active && (e.tag == "crate" || e.collider.map_or(false, |col| col.is_pushable))
                    })
                });

                if has_crate {
                    plates_pressed += 1;
                    plates_covered.push(idx);
                }
            }
        }

        // Coloration dynamique des dalles activées (vert émeraude si pressée, cyan sinon)
        for (idx, ent) in self.entities.iter_mut().enumerate() {
            if ent.tag == "plate" {
                if plates_covered.contains(&idx) {
                    if let Some(ref mut r) = ent.renderable {
                        r.color = 0x00065F46;
                        r.outline_color = Some(0x0010B981);
                    }
                } else if let Some(ref mut r) = ent.renderable {
                    r.color = 0x00082F49;
                    r.outline_color = Some(0x000284C7);
                }
            }
        }

        // Déverrouillage automatique de la porte si toutes les dalles sont pressées
        if total_plates > 0 {
            self.kvl_context.set_var("plates_pressed", Value::Int(plates_pressed as i64));
            self.kvl_context.set_var("total_plates", Value::Int(total_plates as i64));
            if plates_pressed >= total_plates {
                self.kvl_context.set_var("puzzle_solved", Value::Bool(true));
                self.kvl_context.set_var("objective", Value::String("OBJECTIF: ENIGME RESOLUE ! SORTIE DU SANCTUAIRE".into()));

                for ent in &mut self.entities {
                    if ent.tag == "door" {
                        if let Some(ref mut col) = ent.collider {
                            if col.is_solid {
                                col.is_solid = false;
                                if let Some(ref mut r) = ent.renderable {
                                    r.color = 0x0010B981;
                                    r.outline_color = Some(0x006EE7B7);
                                }
                                self.audio.play(SoundType::DoorOpen);
                                self.audio.play(SoundType::BonusFanfare);
                                spawn_requests.push((ent.transform.center_x(), ent.transform.center_y(), 30, 0x0010B981));
                                self.status_message = "ENIGME RESOLUE ! Dalles enclenchées, portail ouvert !".to_string();
                            }
                        }
                    }
                }
            } else {
                self.kvl_context.set_var("puzzle_solved", Value::Bool(false));
                self.kvl_context.set_var("objective", Value::String(format!("OBJECTIF: DALLES ACTIVEES ({}/{})", plates_pressed, total_plates)));
            }
        }

        // ==========================================
        // SYSTEM 5 : INTERACTION (TOUCHE E / ESPACE)
        // ==========================================
        if let Some((_p_idx, px, py, pw, ph)) = player_pos {
            let p_cx = px + pw * 0.5;
            let p_cy = py + ph * 0.5;

            if input.is_key_just_pressed(KeyCode::KeyE) || input.is_key_just_pressed(KeyCode::Space) {
                for ent in &mut self.entities {
                    if !ent.is_active {
                        continue;
                    }
                    if let Some(ref script_comp) = ent.script {
                        if let Some(ref interact_evt) = script_comp.on_interact_event {
                            let dx = p_cx - ent.transform.center_x();
                            let dy = p_cy - ent.transform.center_y();
                            let dist = (dx * dx + dy * dy).sqrt();

                            if dist < 80.0 {
                                self.kvl_context.set_var("access_denied", Value::Bool(false));

                                if let Some(graph) = self.kvl_graphs.get(&script_comp.script_path) {
                                    if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, interact_evt) {
                                        kvl_steps_this_frame += steps;
                                    }
                                }

                                let was_denied = match self.kvl_context.get_var("access_denied") {
                                    Some(Value::Bool(d)) => *d,
                                    _ => false,
                                };

                                if was_denied {
                                    self.audio.play(SoundType::AccessDenied);
                                    spawn_requests.push((ent.transform.center_x(), ent.transform.center_y(), 25, 0x00EF4444));
                                    let current_score = match self.kvl_context.get_var("score") {
                                        Some(Value::Int(s)) => *s,
                                        _ => 0,
                                    };
                                    self.status_message = format!("Accès Refusé ({}) ! Score insuffisant : {} / 50 requis.", ent.name, current_score);
                                } else {
                                    let is_open = match self.kvl_context.get_var("door_is_open") {
                                        Some(Value::Bool(b)) => *b,
                                        _ => true,
                                    };

                                    // Si la porte est ouverte, désactiver sa solidité
                                    if let Some(ref mut col) = ent.collider {
                                        col.is_solid = !is_open;
                                    }
                                    if let Some(ref mut rend) = ent.renderable {
                                        rend.color = if is_open { 0x0010B981 } else { 0x00EF4444 };
                                    }

                                    if is_open {
                                        self.audio.play(SoundType::DoorOpen);
                                    } else {
                                        self.audio.play(SoundType::DoorClose);
                                    }
                                    let p_col = if is_open { 0x0010B981 } else { 0x00EF4444 };
                                    spawn_requests.push((ent.transform.center_x(), ent.transform.center_y(), 20, p_col));

                                    self.status_message = if is_open {
                                        format!("Entité ({}) déverrouillée et ouverte !", ent.name)
                                    } else {
                                        format!("Entité ({}) fermée.", ent.name)
                                    };
                                }
                            }
                        }
                    }

                    // Interaction PNJ avec DialogueComponent
                    if !self.dialogue.is_active {
                        if let Some(ref dlg_comp) = ent.dialogue {
                            let dx = p_cx - ent.transform.center_x();
                            let dy = p_cy - ent.transform.center_y();
                            let dist = (dx * dx + dy * dy).sqrt();

                            if dist < dlg_comp.interact_radius {
                                self.dialogue.open(
                                    &dlg_comp.speaker,
                                    &dlg_comp.text,
                                    dlg_comp.choices.clone(),
                                    &dlg_comp.avatar_label,
                                    dlg_comp.avatar_color,
                                );
                                self.kvl_context.set_var("dialogue_active", Value::Bool(true));
                                self.kvl_context.set_var("dialogue_speaker", Value::String(dlg_comp.speaker.clone()));
                                self.kvl_context.set_var("dialogue_text", Value::String(dlg_comp.text.clone()));
                                self.kvl_context.set_var("dialogue_has_choices", Value::Bool(!dlg_comp.choices.is_empty()));
                                self.kvl_context.set_var("dialogue_selected_choice", Value::Int(0));
                                if let Some(c1) = dlg_comp.choices.first() {
                                    self.kvl_context.set_var("dialogue_choice_1", Value::String(c1.clone()));
                                }
                                if dlg_comp.choices.len() >= 2 {
                                    self.kvl_context.set_var("dialogue_choice_2", Value::String(dlg_comp.choices[1].clone()));
                                }

                                // Quête associée au PNJ
                                if let Some(ref qid) = dlg_comp.quest_id {
                                    if let Some(step) = dlg_comp.quest_step {
                                        self.kvl_context.set_var(format!("quest_{}", qid), Value::Int(step));
                                        if let Some(ref mut q) = self.quest {
                                            if &q.id == qid {
                                                q.set_step(step);
                                            }
                                        }
                                    }
                                }

                                self.audio.play(SoundType::UiClick);
                                self.status_message = format!("Discussion engagée avec {} (ESPACE / E : passer)", dlg_comp.speaker);

                                for graph in self.kvl_graphs.values() {
                                    if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, "OnDialogueStart") {
                                        kvl_steps_this_frame += steps;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Synchronisation KVL -> Boîte de dialogue (actions ShowDialogue de scripts)
        if let Some(Value::Bool(true)) = self.kvl_context.get_var("dialogue_active") {
            if !self.dialogue.is_active {
                let spk = match self.kvl_context.get_var("dialogue_speaker") {
                    Some(Value::String(s)) => s.clone(),
                    _ => "PNJ".into(),
                };
                let txt = match self.kvl_context.get_var("dialogue_text") {
                    Some(Value::String(t)) => t.clone(),
                    _ => "...".into(),
                };
                let mut choices = Vec::new();
                if let Some(Value::String(c1)) = self.kvl_context.get_var("dialogue_choice_1") {
                    if !c1.is_empty() {
                        choices.push(c1.clone());
                    }
                }
                if let Some(Value::String(c2)) = self.kvl_context.get_var("dialogue_choice_2") {
                    if !c2.is_empty() {
                        choices.push(c2.clone());
                    }
                }
                self.dialogue.open(spk, txt, choices, "NPC", 0x0038BDF8);
            }
        }

        // Vérification d'accomplissement de quête KVL (CompleteQuest:)
        let mut completed_any_quest = false;
        if let Some(Value::Bool(true)) = self.kvl_context.get_var("quest_completed_main_quest") {
            completed_any_quest = true;
        }
        if completed_any_quest {
            if let Some(ref mut q) = self.quest {
                if !q.is_completed {
                    q.complete();
                    self.audio.play(SoundType::QuestSuccess);
                    self.emit_burst_at(600.0, 300.0, 40, 0x00FDE047, 0x0010B981);
                    self.status_message = "QUÊTE PRINCIPALE ACCOMPLIE ! Félicitations !".into();
                }
            }
        }

        // Traitement des étincelles déclenchées par KVL (SpawnSparkles)
        if let Some(Value::Int(sparkles)) = self.kvl_context.get_var("sparkle_burst") {
            if *sparkles > 0 {
                self.audio.play(SoundType::Sparkles);
                if let Some((_p_idx, px, py, pw, ph)) = player_pos {
                    spawn_requests.push((px + pw * 0.5, py + ph * 0.5, (*sparkles as usize) * 35, 0x00FDE047));
                }
                self.kvl_context.set_var("sparkle_burst", Value::Int(0));
            }
        }

        // Traitement des gerbes d'émetteur déclenchées par KVL (EmitBurst)
        if let Some(Value::Int(burst)) = self.kvl_context.get_var("emitter_burst").cloned() {
            if burst > 0 {
                self.audio.play(SoundType::Sparkles);
                let (bx, by) = if let Some((_p_idx, px, py, pw, ph)) = player_pos {
                    (px + pw * 0.5, py + ph * 0.5)
                } else {
                    (400.0, 300.0)
                };
                self.emit_burst_at(bx, by, burst as usize, 0x00FDE047, 0x00EA580C);
                self.kvl_context.set_var("emitter_burst", Value::Int(0));
            }
        }

        // Traitement de l'activation/désactivation d'émetteur déclenchée par KVL (ToggleEmitter)
        if let Some(Value::Bool(active)) = self.kvl_context.get_var("emitter_active").cloned() {
            for ent in &mut self.entities {
                if let Some(ref mut em) = ent.emitter {
                    em.is_emitting = active;
                }
            }
        }

        // Traitement de la téléportation déclenchée par KVL (TeleportPlayer / TeleportTo)
        if let (Some(Value::Int(tx)), Some(Value::Int(ty))) = (
            self.kvl_context.get_var("teleport_target_x").cloned(),
            self.kvl_context.get_var("teleport_target_y").cloned(),
        ) {
            if tx >= 0 && ty >= 0 {
                if let Some(p_idx) = self.player_index() {
                    self.entities[p_idx].transform.x = tx as f32;
                    self.entities[p_idx].transform.y = ty as f32;
                    self.audio.play(SoundType::Teleport);
                    spawn_requests.push((tx as f32 + 15.0, ty as f32 + 15.0, 35, 0x0038BDF8));
                    self.status_message = format!("Téléportation quantique de l'entité vers ({}, {}) !", tx, ty);
                }
                self.kvl_context.set_var("teleport_target_x", Value::Int(-1));
                self.kvl_context.set_var("teleport_target_y", Value::Int(-1));
            }
        }

        // ==========================================
        // SYSTEM 6 : VISION SENSING & RAYCASTING LINE-OF-SIGHT
        // ==========================================
        if let Some((p_idx, px, py, pw, ph)) = player_pos {
            let p_cx = px + pw * 0.5;
            let p_cy = py + ph * 0.5;

            for (ent_idx, ent) in self.entities.iter_mut().enumerate() {
                if !ent.is_active || ent_idx == p_idx {
                    continue;
                }
                if let Some(ref mut vision) = ent.vision {
                    let e_cx = ent.transform.center_x();
                    let e_cy = ent.transform.center_y();

                    let dx = p_cx - e_cx;
                    let dy = p_cy - e_cy;
                    let dist_sq = dx * dx + dy * dy;
                    let range_sq = vision.range * vision.range;

                    // Détection dans le cône (vers la gauche / approche)
                    let in_cone = dist_sq < range_sq && dx < 25.0 && dy.abs() < (vision.range * 0.70);

                    if in_cone {
                        // Raycast accéléré par partitionnement spatial
                        let mut sight_blocked = false;
                        let candidates = self.spatial_grid.query_segment(e_cx, e_cy, p_cx, p_cy);
                        for obs in candidates {
                            if obs.id != p_idx && obs.id != ent_idx && matches!(obs.kind, SpatialKind::Solid) {
                                if segment_intersects_aabb(e_cx, e_cy, p_cx, p_cy, obs.x, obs.y, obs.w, obs.h) {
                                    sight_blocked = true;
                                    break;
                                }
                            }
                        }

                        if !sight_blocked {
                            if !vision.is_alerted {
                                vision.is_alerted = true;
                                self.audio.play(SoundType::Alarm);
                                spawn_requests.push((e_cx, e_cy - 25.0, 30, 0x00EF4444));
                                self.status_message = format!("ALERTE ROUGE ! L'entité '{}' vous a repéré !", ent.name);

                                if let Some(ref script_comp) = ent.script {
                                    if let Some(ref seen_evt) = script_comp.on_seen_event {
                                        if let Some(graph) = self.kvl_graphs.get(&script_comp.script_path) {
                                            if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, seen_evt) {
                                                kvl_steps_this_frame += steps;
                                            }
                                        }
                                    }
                                }
                            }
                            vision.alert_timer = 2.5;
                        }
                    }

                    if vision.is_alerted {
                        vision.alert_timer -= dt;
                        if vision.alert_timer <= 0.0 {
                            vision.is_alerted = false;
                            self.kvl_context.set_var("alarm_active", Value::Bool(false));
                            self.status_message = format!("L'entité '{}' a perdu votre trace. Alerte levée.", ent.name);
                        }
                    }
                }
            }
        }

        // Verrouillage global des portes si demandé par KVL
        if let Some(Value::Bool(open)) = self.kvl_context.get_var("door_is_open") {
            for ent in &mut self.entities {
                if ent.tag == "door" {
                    if let Some(ref mut col) = ent.collider {
                        col.is_solid = !*open;
                    }
                    if let Some(ref mut rend) = ent.renderable {
                        rend.color = if *open { 0x0010B981 } else { 0x00EF4444 };
                    }
                }
            }
        }

        for (px, py, count, col) in spawn_requests {
            self.spawn_particles(px, py, count, col);
        }

        // Suivi fluide de la caméra sur le joueur en Mode Jeu
        self.camera.set_viewport(20, 66, 1240, 470);
        if let Some(p_idx) = self.player_index() {
            let px = self.entities[p_idx].transform.center_x();
            let py = self.entities[p_idx].transform.center_y();
            self.camera.target_x = px;
            self.camera.target_y = py;
        }
        self.camera.update(dt);

        let frame_duration_us = frame_start.elapsed().as_micros() as u64;
        self.telemetry.record_frame(frame_duration_us, kvl_steps_this_frame);

        if let Some(msg) = self.adaptation.evaluate_adaptation(frame_duration_us) {
            self.status_message = msg.into();
        }
    }

    pub fn render(&mut self, canvas: &mut Canvas) {
        canvas.clear(0x000B0F19);

        let is_edit_mode = self.editor.mode == EditorMode::Edit;
        let w = canvas.width as i32;
        let h = canvas.height as i32;

        // 1. Bandeau supérieur (HUD) : Hauteur 56px
        let header_bg = if is_edit_mode { 0x001E1035 } else { 0x000F172A };
        let header_border = if is_edit_mode { 0x007C3AED } else { 0x000284C7 };
        canvas.draw_rect(0, 0, w as u32, 56, header_bg);
        canvas.draw_rect(0, 55, w as u32, 2, header_border);

        if is_edit_mode {
            self.editor.pin_hitboxes.clear();
            self.editor.connection_hitboxes.clear();
            self.editor.palette_hitboxes.clear();
            self.editor.spawn_palette_hitboxes.clear();
            self.editor.add_comp_hitboxes.clear();
            self.editor.save_btn_hitbox = None;
            self.editor.top_save_btn_hitbox = None;
            self.editor.add_node_btn_hitbox = None;
            self.editor.view_btn_hitbox = None;
            self.editor.spawn_btn_hitbox = None;
            self.editor.duplicate_btn_hitbox = None;
            self.editor.delete_btn_hitbox = None;
            self.editor.help_btn_hitbox = None;
            self.editor.tool_select_btn_hitbox = None;
            self.editor.tool_brush_btn_hitbox = None;
            self.editor.tool_eraser_btn_hitbox = None;
            self.editor.tool_tile_cycle_btn_hitbox = None;

            // Brand Badge Studio
            canvas.draw_badge(16, 12, 105, 32, 0x002E1065, 0x00A855F7, "STUDIO ECS", 0x00F3E8FF, 2);

            // Outil 1 : [V: SELECT]
            let sel_bg = if self.editor.active_tool == StudioTool::Select { 0x001E3A8A } else { 0x000F172A };
            let sel_bd = if self.editor.active_tool == StudioTool::Select { 0x003B82F6 } else { 0x00334155 };
            let sel_fg = if self.editor.active_tool == StudioTool::Select { 0x00EFF6FF } else { 0x0094A3B8 };
            canvas.draw_badge(129, 12, 88, 32, sel_bg, sel_bd, "[V: SELECT]", sel_fg, 2);
            self.editor.tool_select_btn_hitbox = Some((129, 12, 88, 32));

            // Outil 2 : [B: PINCEAU]
            let bru_bg = if self.editor.active_tool == StudioTool::Brush { 0x007C2D12 } else { 0x000F172A };
            let bru_bd = if self.editor.active_tool == StudioTool::Brush { 0x00F97316 } else { 0x00334155 };
            let bru_fg = if self.editor.active_tool == StudioTool::Brush { 0x00FFEDD5 } else { 0x0094A3B8 };
            canvas.draw_badge(223, 12, 96, 32, bru_bg, bru_bd, "[B: PINCEAU]", bru_fg, 2);
            self.editor.tool_brush_btn_hitbox = Some((223, 12, 96, 32));

            // Sélecteur cyclique de Tuile : [T: <Nom> >]
            let tile_col = self.editor.brush_tile.color();
            let tile_lbl = format!("[T: {} >]", self.editor.brush_tile.short_label());
            canvas.draw_badge(325, 12, 108, 32, 0x0018181B, tile_col, &tile_lbl, tile_col, 2);
            self.editor.tool_tile_cycle_btn_hitbox = Some((325, 12, 108, 32));

            // Outil 3 : [X: GOMME]
            let era_bg = if self.editor.active_tool == StudioTool::Eraser { 0x007F1D1D } else { 0x000F172A };
            let era_bd = if self.editor.active_tool == StudioTool::Eraser { 0x00EF4444 } else { 0x00334155 };
            let era_fg = if self.editor.active_tool == StudioTool::Eraser { 0x00FEE2E2 } else { 0x0094A3B8 };
            canvas.draw_badge(439, 12, 85, 32, era_bg, era_bd, "[X: GOMME]", era_fg, 2);
            self.editor.tool_eraser_btn_hitbox = Some((439, 12, 85, 32));

            // Action : [+ SPAWN (P)]
            let spawn_x = 530;
            let spawn_y = 12;
            let spawn_w = 112;
            let spawn_h = 32;
            let spawn_bg = if self.editor.show_spawn_palette { 0x007C3AED } else { 0x003730A3 };
            canvas.draw_badge(spawn_x, spawn_y, spawn_w as u32, spawn_h as u32, spawn_bg, 0x00818CF8, "[+ SPAWN (P)]", 0x00FFFFFF, 2);
            self.editor.spawn_btn_hitbox = Some((spawn_x, spawn_y, spawn_w as u32, spawn_h as u32));

            // Action : [SAUVER (C+S)]
            let save_x = 648;
            let save_y = 12;
            let save_w = 118;
            let save_h = 32;
            canvas.draw_badge(save_x, save_y, save_w as u32, save_h as u32, 0x00064E3B, 0x0010B981, "[SAUVER (C+S)]", 0x00D1FAE5, 2);
            self.editor.top_save_btn_hitbox = Some((save_x, save_y, save_w as u32, save_h as u32));

            // Actions contextuelles si une entité est sélectionnée
            if let SelectedEntity::Entity(idx) = self.editor.selected {
                if idx < self.entities.len() {
                    let dup_x = 772;
                    let dup_y = 12;
                    let dup_w = 105;
                    let dup_h = 32;
                    canvas.draw_badge(dup_x, dup_y, dup_w as u32, dup_h as u32, 0x0078350F, 0x00F59E0B, "[DUP (C+D)]", 0x00FEF3C7, 2);
                    self.editor.duplicate_btn_hitbox = Some((dup_x, dup_y, dup_w as u32, dup_h as u32));

                    let is_player = self.entities[idx].movement.as_ref().map_or(false, |m| m.is_player_controlled);
                    if !is_player {
                        let del_x = 883;
                        let del_y = 12;
                        let del_w = 85;
                        let del_h = 32;
                        canvas.draw_badge(del_x, del_y, del_w as u32, del_h as u32, 0x007F1D1D, 0x00EF4444, "[SUPPR]", 0x00FEE2E2, 2);
                        self.editor.delete_btn_hitbox = Some((del_x, del_y, del_w as u32, del_h as u32));
                    }
                }
            }

            // Statut Audio
            let (audio_txt, audio_col) = if !self.audio.is_available {
                ("AUDIO: N/D", 0x0094A3B8)
            } else if self.audio.is_muted() {
                ("AUDIO: OFF", 0x00EF4444)
            } else {
                ("AUDIO: ON", 0x0010B981)
            };
            canvas.draw_badge(w - 295, 12, 90, 32, 0x001E293B, audio_col, audio_txt, audio_col, 2);

            // Bouton Guide Interactif (? AIDE)
            let help_x = w - 195;
            let help_y = 12;
            let help_w = 95;
            let help_h = 32;
            let help_bg = if self.editor.show_help_modal { 0x004F46E5 } else { 0x001E1B4B };
            canvas.draw_badge(help_x, help_y, help_w as u32, help_h as u32, help_bg, 0x00818CF8, "[? AIDE]", 0x00E0E7FF, 2);
            self.editor.help_btn_hitbox = Some((help_x, help_y, help_w as u32, help_h as u32));

            // Bouton Bascule Mode Jeu [TAB]
            canvas.draw_badge(w - 92, 12, 76, 32, 0x00064E3B, 0x0010B981, "JOUER", 0x00D1FAE5, 2);
        } else {
            self.editor.help_btn_hitbox = None;

            // Brand Badge Jeu
            canvas.draw_badge(16, 12, 140, 32, 0x00082F49, 0x000284C7, "KARACTIRAGE", 0x00E0F2FE, 2);

            // Genre déduit des composants réels (agnostique du nom de fichier de scène)
            let player_mode = self
                .player_index()
                .and_then(|i| self.entities[i].movement.as_ref().map(|m| m.mode))
                .unwrap_or_default();
            let has_pushables = self
                .entities
                .iter()
                .any(|e| e.is_active && e.collider.map_or(false, |c| c.is_pushable));
            let (genre_badge, genre_col, genre_bd) = match player_mode {
                MovementMode::Platformer { .. } => ("PLATEFORME 2D", 0x0010B981, 0x0034D399),
                MovementMode::SpaceInertia { .. } => ("ESPACE SHMUP", 0x00A855F7, 0x00C084FC),
                MovementMode::Grid { .. } => ("GRILLE / TACTIQUE", 0x00F59E0B, 0x00FBBF24),
                MovementMode::TopDown if has_pushables => ("PUZZLE SOKOBAN", 0x00F59E0B, 0x00FBBF24),
                MovementMode::TopDown => ("TOP-DOWN RPG", 0x000284C7, 0x0038BDF8),
            };
            canvas.draw_badge(164, 12, 120, 32, 0x001E293B, genre_bd, genre_badge, genre_col, 2);

            let score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };

            // Carte d'objectif dynamique hiérarchique et 100% découplée :
            // 1. Variable KVL runtime "objective" (pilotée par scripts KVL ou puzzles dynamiques)
            // 2. scene_objective (défini par l'auteur dans le fichier de scène JSON)
            // 3. Fallback dynamique déduit du modèle physique ECS actif du joueur
            let (obj_txt, obj_bg, obj_bd, obj_fg) = if let Some(Value::String(s)) = self.kvl_context.get_var("objective") {
                (s.as_str(), 0x00064E3B, 0x0010B981, 0x006EE7B7)
            } else if let Some(ref s_obj) = self.scene_objective {
                (s_obj.as_str(), 0x00451A03, 0x00D97706, 0x00FDE047)
            } else {
                match player_mode {
                    MovementMode::Platformer { .. } => ("OBJECTIF: SAUTEZ (ESPACE/Z) ET ATTEIGNEZ LE PORTAIL", 0x00064E3B, 0x0010B981, 0x006EE7B7),
                    MovementMode::SpaceInertia { .. } => ("OBJECTIF: PILOTEZ LE VAISSEAU ET TIREZ (F/ESPACE)", 0x002E1065, 0x00A855F7, 0x00F3E8FF),
                    MovementMode::Grid { .. } => ("OBJECTIF: DEPLACEZ LES BLOCS SUR LA GRILLE TACTIQUE", 0x00451A03, 0x00D97706, 0x00FDE047),
                    MovementMode::TopDown if has_pushables => ("OBJECTIF: POUSSEZ LES BLOCS SUR LES DALLES", 0x00451A03, 0x00D97706, 0x00FDE047),
                    MovementMode::TopDown => {
                        if score < 50 {
                            ("OBJECTIF: COLLECTEZ 50 OR POUR LA PORTE", 0x00451A03, 0x00D97706, 0x00FDE047)
                        } else {
                            ("OBJECTIF ATTEINT ! PORTE OUVERTE, SORTEZ", 0x00064E3B, 0x0010B981, 0x006EE7B7)
                        }
                    }
                }
            };
            canvas.draw_badge(292, 12, 380, 32, obj_bg, obj_bd, obj_txt, obj_fg, 2);

            // Score Pill
            let score_txt = format!("SCORE: {}", score);
            canvas.draw_badge(678, 12, 115, 32, 0x001E293B, 0x00F59E0B, &score_txt, 0x00FBBF24, 2);

            // Profil d'adaptation
            let profile_color = self.adaptation.current_profile.color();
            let profile_txt = format!("{}", self.adaptation.current_profile.name());
            canvas.draw_badge(801, 12, 85, 32, 0x001E293B, profile_color, &profile_txt, profile_color, 2);

            // Audio
            let (audio_txt, audio_col) = if !self.audio.is_available {
                ("AUDIO: N/D", 0x0094A3B8)
            } else if self.audio.is_muted() {
                ("AUDIO: OFF", 0x00EF4444)
            } else {
                ("AUDIO: ON", 0x0010B981)
            };
            canvas.draw_badge(894, 12, 95, 32, 0x001E293B, audio_col, audio_txt, audio_col, 2);

            // Bouton Guide (? AIDE)
            let help_x = w - 195;
            let help_y = 12;
            let help_w = 95;
            let help_h = 32;
            let help_bg = if self.editor.show_help_modal { 0x004F46E5 } else { 0x001E1B4B };
            canvas.draw_badge(help_x, help_y, help_w as u32, help_h as u32, help_bg, 0x00818CF8, "[? AIDE]", 0x00E0E7FF, 2);
            self.editor.help_btn_hitbox = Some((help_x, help_y, help_w as u32, help_h as u32));

            // Bouton Bascule Studio [TAB]
            canvas.draw_badge(w - 92, 12, 76, 32, 0x002E1065, 0x00A855F7, "STUDIO", 0x00F3E8FF, 2);
        }

        // 2. Zone de jeu (Viewport)
        let play_area_w = if is_edit_mode { (w - 490).max(400) as u32 } else { (w - 40) as u32 };
        let play_area_h = (h - 250).max(300) as u32;

        let vx = 20;
        let vy = 66;
        let vw = play_area_w as i32;
        let vh = play_area_h as i32;

        self.camera.set_viewport(vx, vy, vw as u32, vh as u32);
        canvas.set_clip_rect(Some((vx, vy, play_area_w, play_area_h)));

        // Sol de jeu texturé (Dallage sci-fi dungeon défilant)
        canvas.draw_rect(vx, vy, play_area_w, play_area_h, 0x000D1525);

        // Lignes de joints de dallage 48x48
        let tile_sz = 48;
        let max_x = vx + vw;
        let max_y = vy + vh;

        let offset_x = (self.camera.x.rem_euclid(tile_sz as f32)) as i32;
        let offset_y = (self.camera.y.rem_euclid(tile_sz as f32)) as i32;

        let mut tx = vx - offset_x;
        while tx < max_x {
            if tx >= vx {
                canvas.draw_line(tx, vy, tx, max_y, 0x00131E33);
            }
            tx += tile_sz;
        }
        let mut ty = vy - offset_y;
        while ty < max_y {
            if ty >= vy {
                canvas.draw_line(vx, ty, max_x, ty, 0x00131E33);
            }
            ty += tile_sz;
        }

        // Rivets discrets aux carrefours de dallage
        let mut cy = vy - offset_y;
        while cy < max_y {
            if cy >= vy {
                let mut cx = vx - offset_x;
                while cx < max_x {
                    if cx >= vx {
                        canvas.draw_rect(cx - 1, cy - 1, 3, 3, 0x001D2C48);
                    }
                    cx += tile_sz;
                }
            }
            cy += tile_sz;
        }

        let frame_border = if is_edit_mode { 0x004C1D95 } else { 0x001E293B };
        let bracket_col = if is_edit_mode { 0x00A855F7 } else { 0x0038BDF8 };
        canvas.draw_rect_outline(vx, vy, play_area_w, play_area_h, frame_border, 2);
        canvas.draw_corner_brackets(vx, vy, play_area_w, play_area_h, 16, bracket_col);

        // Légende discrète de zone de jeu
        let viewport_tag = if is_edit_mode {
            match self.editor.active_tool {
                StudioTool::Select => "STUDIO [SELECT] : CLIQUEZ/GLISSEZ | FLECHES: DEFILEMENT | C: RECENTRER | V / B / X",
                StudioTool::Brush => "STUDIO [PINCEAU] : CLIC GAUCHE GLISSE POUR PEINDRE | T: CHANGER TUILE | CLIC DROIT: EFFACER",
                StudioTool::Eraser => "STUDIO [GOMME] : CLIC GAUCHE GLISSE POUR EFFACER LES BLOCS | V: SELECT | B: PINCEAU",
            }
        } else if self.scene_path.contains("platformer") {
            "PLATEFORME 2D (Q/D/FLECHES : COURIR | ESPACE/Z/HAUT : SAUTER | F : TIRER | F4/L : GENRE)"
        } else if self.scene_path.contains("space") {
            "ESPACE SHMUP (Q/D/FLECHES : TOURNER | Z/HAUT : POUSSEE | ESPACE/F : TIRER | F4/L : GENRE)"
        } else if self.scene_path.contains("puzzle") {
            "PUZZLE SOKOBAN (ZQSD/FLECHES : DEPLACER | POUSSER BLOCS SUR DALLES | F4/L : GENRE)"
        } else {
            "TOP-DOWN RPG (ZQSD / FLECHES : DEPLACEMENT | E : INTERACTION | F : TIRER | F4/L : GENRE)"
        };
        canvas.draw_text(30, 72, viewport_tag, 0x00475569, 1);

        // Jauge de Vitalité / Santé Joueur en overlay élégant dans le viewport (zéro-allocation tas)
        if !is_edit_mode {
            if let Some(p_idx) = self.player_index() {
                if let Some(ref h) = self.entities[p_idx].health {
                    let hp_card_x = 30;
                    let hp_card_y = if self.spatial_grid.debug_visible { 108 } else { 86 };
                    let cur = h.current.ceil() as i32;
                    let max = h.max.ceil() as i32;
                    let hearts_count = max.clamp(1, 10);
                    let hp_card_w = (hearts_count * 16 + 22).max(110) as u32;
                    let hp_card_h = 34;

                    canvas.draw_rect(hp_card_x, hp_card_y, hp_card_w, hp_card_h, 0x000F172A);
                    canvas.draw_rect_outline(hp_card_x, hp_card_y, hp_card_w, hp_card_h, 0x00334155, 1);
                    canvas.draw_corner_brackets(hp_card_x, hp_card_y, hp_card_w, hp_card_h, 6, 0x00EF4444);

                    let label = if cur > 0 { "VITALITE" } else { "MORT !" };
                    let lbl_col = if cur > 0 { 0x0094A3B8 } else { 0x00EF4444 };
                    canvas.draw_text(hp_card_x + 8, hp_card_y + 4, label, lbl_col, 1);

                    for i in 0..hearts_count {
                        let heart_icon = if i < cur { "♥" } else { "♡" };
                        let heart_col = if i < cur { 0x00EF4444 } else { 0x00475569 };
                        canvas.draw_text(hp_card_x + 8 + i * 16, hp_card_y + 16, heart_icon, heart_col, 2);
                    }
                }
            }
        }

        // Visualisation de débogage de la Grille Spatiale (Touche G)
        if self.spatial_grid.debug_visible {
            let cs = self.spatial_grid.cell_size as i32;
            let mut gx = 20;
            while gx <= max_x {
                canvas.draw_line(gx, 66, gx, max_y, 0x001B263B);
                gx += cs;
            }
            let mut gy = 66;
            while gy <= max_y {
                canvas.draw_line(20, gy, max_x, gy, 0x001B263B);
                gy += cs;
            }
            canvas.draw_text(30, 88, "GRILLE SPATIALE O(1) : ACTIVE (64px)", 0x0038BDF8, 1);
        }

        // Grille de repérage en mode édition
        if is_edit_mode {
            for gx in (30..max_x).step_by(40) {
                for gy in (90..max_y).step_by(40) {
                    canvas.draw_rect(gx, gy, 2, 2, 0x00334155);
                }
            }
        }

        // ==========================================
        // SYSTEM 7 : RENDU GÉNÉRIQUE Z-SORTED DES ENTITÉS
        // ==========================================
        let mut render_order: Vec<usize> = (0..self.entities.len())
            .filter(|&i| self.entities[i].is_active || is_edit_mode)
            .filter(|&i| self.entities[i].renderable.is_some())
            .collect();

        render_order.sort_by_key(|&i| self.entities[i].renderable.as_ref().map_or(0, |r| r.z_index));

        for idx in render_order {
            let ent = &self.entities[idx];
            if !self.camera.is_visible(ent.transform.x, ent.transform.y, ent.transform.w, ent.transform.h) {
                continue;
            }
            let (rx, ry) = self.camera.world_to_screen(ent.transform.x, ent.transform.y);
            let rend = ent.renderable.as_ref().unwrap();
            let is_selected = is_edit_mode && self.editor.selected == SelectedEntity::Entity(idx);

            // Halo d'illumination de sol pour les torches
            if ent.tag == "torch" {
                let (tx, ty) = self.camera.world_to_screen(ent.transform.center_x(), ent.transform.center_y());
                let flicker = ((self.elapsed_time * 8.0).sin() * 3.0) as i32;
                let halo_r = (42 + flicker).max(20);
                canvas.draw_circle(tx, ty, halo_r, 0x00221306);
                canvas.draw_circle(tx, ty, halo_r * 2 / 3, 0x003B1E08);
            }

            match rend.shape {
                ShapeKind::Rect => {
                    let rw = ent.transform.w as u32;
                    let rh = ent.transform.h as u32;

                    // Ombre portée au sol (Drop shadow)
                    if ent.tag != "floor" {
                        canvas.draw_rect(rx + 3, ry + 4, rw, rh, 0x00060911);
                    }

                    match ent.tag.as_str() {
                        "wall" => {
                            // Mur Donjon/Cyber : Corps solide, biseau haut et bas, plaque de renfort
                            canvas.draw_rect(rx, ry, rw, rh, rend.color);
                            // Biseau de surbrillance supérieur (Light edge)
                            canvas.draw_line(rx, ry, rx + rw as i32 - 1, ry, 0x00475569);
                            canvas.draw_line(rx, ry + 1, rx + rw as i32 - 1, ry + 1, 0x00334155);
                            // Biseau d'ombre inférieur (Shadow edge)
                            canvas.draw_line(rx, ry + rh as i32 - 1, rx + rw as i32 - 1, ry + rh as i32 - 1, 0x000F172A);
                            canvas.draw_line(rx, ry + rh as i32 - 2, rx + rw as i32 - 1, ry + rh as i32 - 2, 0x00131C2E);

                            // Bordure générale
                            if let Some(outline_col) = rend.outline_color {
                                canvas.draw_rect_outline(rx, ry, rw, rh, outline_col, rend.outline_width);
                            }

                            // Plaque interne gravée si surface suffisante
                            if rw >= 24 && rh >= 24 {
                                canvas.draw_rect_outline(rx + 4, ry + 4, rw - 8, rh - 8, 0x001E293B, 1);
                                if rw >= 40 && rh >= 40 {
                                    // Rivets aux 4 coins du mur
                                    canvas.draw_rect(rx + 6, ry + 6, 2, 2, 0x00475569);
                                    canvas.draw_rect(rx + rw as i32 - 8, ry + 6, 2, 2, 0x00475569);
                                    canvas.draw_rect(rx + 6, ry + rh as i32 - 8, 2, 2, 0x00475569);
                                    canvas.draw_rect(rx + rw as i32 - 8, ry + rh as i32 - 8, 2, 2, 0x00475569);
                                }
                            }
                        }
                        "player" => {
                            // Joueur Cyber-Exosquelette (avec clignotement i-frames)
                            let is_invulnerable = ent.health.as_ref().map_or(false, |h| h.invulnerability_timer > 0.0);
                            let flash_white = is_invulnerable && (((ent.health.as_ref().unwrap().invulnerability_timer * 20.0) as i32 % 2) == 0);
                            let body_col = if flash_white { 0x00FFFFFF } else { rend.color };
                            let border_col = if flash_white { 0x00EF4444 } else { 0x0038BDF8 };

                            canvas.draw_rect(rx, ry, rw, rh, body_col);
                            // Biseau armure
                            canvas.draw_rect_outline(rx, ry, rw, rh, border_col, 2);

                            // Visière HUD cyan brillante
                            let visor_y = ry + 8;
                            let visor_h = 7.min(rh.saturating_sub(12));
                            if rw > 12 && visor_h > 2 {
                                canvas.draw_rect(rx + 6, visor_y, rw - 12, visor_h, if flash_white { 0x00EF4444 } else { 0x00E0F2FE });
                                canvas.draw_line(rx + 7, visor_y + 1, rx + rw as i32 - 8, visor_y + 1, 0x00FFFFFF);
                            }

                            // Noyau réacteur pectoral (pulsant)
                            if rw >= 20 && rh >= 28 {
                                let core_x = rx + (rw as i32 / 2) - 3;
                                let core_y = ry + (rh as i32 / 2) + 2;
                                let core_glow = if (self.elapsed_time * 6.0).sin() > 0.0 { 0x00FDE047 } else { 0x0034D399 };
                                canvas.draw_rect(core_x, core_y, 6, 6, core_glow);
                                canvas.draw_rect(core_x + 1, core_y + 1, 4, 4, 0x00FFFFFF);
                            }

                            // Épaulettes blindées
                            if rw >= 24 {
                                canvas.draw_rect(rx + 2, ry + 2, 4, 6, 0x000284C7);
                                canvas.draw_rect(rx + rw as i32 - 6, ry + 2, 4, 6, 0x000284C7);
                            }

                            // Viseur / Cockpit vectoriel orienté (Mode Spatial)
                            if ent.transform.rotation != 0.0 {
                                let cx = rx + (rw as i32 / 2);
                                let cy = ry + (rh as i32 / 2);
                                let nx = cx + (ent.transform.rotation.cos() * 18.0) as i32;
                                let ny = cy + (ent.transform.rotation.sin() * 18.0) as i32;
                                canvas.draw_line(cx, cy, nx, ny, 0x00FDE047);
                                canvas.draw_circle(nx, ny, 3, 0x0038BDF8);
                            }
                        }
                        "enemy" => {
                            // Sentinelle de patrouille / Droïde d'assaut
                            canvas.draw_rect(rx, ry, rw, rh, rend.color);
                            canvas.draw_rect_outline(rx, ry, rw, rh, 0x00EF4444, 2);

                            // Visière optique rouge
                            let eye_y = ry + 10;
                            let eye_h = 6.min(rh.saturating_sub(14));
                            if rw > 12 && eye_h > 2 {
                                let is_alert = ent.vision.as_ref().map_or(false, |v| v.is_alerted);
                                let eye_col = if is_alert { 0x00FEE2E2 } else { 0x00DC2626 };
                                canvas.draw_rect(rx + 6, eye_y, rw - 12, eye_h, eye_col);
                                // Pupille de balayage
                                let scan_offset = (((self.elapsed_time * 4.0).sin() + 1.0) * 0.5 * (rw as f32 - 18.0).max(0.0)) as i32;
                                canvas.draw_rect(rx + 7 + scan_offset, eye_y + 1, 4, eye_h.saturating_sub(2), 0x00FFFFFF);
                            }

                            // Ailettes d'armure / Diodes tactiques
                            canvas.draw_rect(rx + 2, ry + 2, 3, 3, 0x00EF4444);
                            canvas.draw_rect(rx + rw as i32 - 5, ry + 2, 3, 3, 0x00EF4444);
                        }
                        "door" => {
                            // Porte Blindée Sécurisée
                            let score = match self.kvl_context.get_var("score") {
                                Some(Value::Int(s)) => *s,
                                _ => 0,
                            };
                            let is_unlocked = score >= 50;
                            let lock_col = if is_unlocked { 0x0010B981 } else { 0x00EF4444 };

                            canvas.draw_rect(rx, ry, rw, rh, 0x001E293B);
                            canvas.draw_rect_outline(rx, ry, rw, rh, 0x00475569, 2);

                            // Montants verticaux blindés gauche et droite
                            if rw >= 16 {
                                canvas.draw_rect(rx, ry, 6, rh, 0x00334155);
                                canvas.draw_rect(rx + rw as i32 - 6, ry, 6, rh, 0x00334155);
                            }

                            // Lamelles horizontales de compression
                            let mut sy = ry + 8;
                            while sy < (ry + rh as i32 - 8) {
                                canvas.draw_line(rx + 7, sy, rx + rw as i32 - 8, sy, 0x000F172A);
                                sy += 8;
                            }

                            // Verrou numérique central
                            if rw >= 18 && rh >= 24 {
                                let lk_w = (rw - 12).min(24);
                                let lk_h = 16.min(rh - 8);
                                let lk_x = rx + (rw as i32 / 2) - (lk_w as i32 / 2);
                                let lk_y = ry + (rh as i32 / 2) - (lk_h as i32 / 2);
                                canvas.draw_badge(lk_x, lk_y, lk_w, lk_h, 0x000F172A, lock_col, if is_unlocked { "OK" } else { "X" }, lock_col, 1);
                            }
                        }
                        "torch" => {
                            // Brasero de torche avec flammes
                            let flame_h = (10.0 + (self.elapsed_time * 9.0).sin() * 2.5) as i32;
                            // Flamme externe orange
                            canvas.draw_rect(rx + 4, ry - flame_h + 8, rw.saturating_sub(8), flame_h as u32, 0x00F97316);
                            // Flamme interne jaune
                            canvas.draw_rect(rx + 6, ry - flame_h + 12, rw.saturating_sub(12), (flame_h - 4).max(2) as u32, 0x00FDE047);
                            // Socle métallique
                            canvas.draw_rect(rx, ry + 8, rw, rh.saturating_sub(8), 0x0078350F);
                            canvas.draw_rect_outline(rx, ry + 8, rw, rh.saturating_sub(8), 0x00F59E0B, 1);
                        }
                        "hazard" => {
                            // Piège à piques / Danger environnemental
                            canvas.draw_rect(rx, ry, rw, rh, 0x001E293B);
                            canvas.draw_rect_outline(rx, ry, rw, rh, 0x00991B1B, 1);
                            // Base métallique
                            canvas.draw_rect(rx, ry + rh as i32 - 6, rw, 6, 0x00334155);
                            canvas.draw_line(rx, ry + rh as i32 - 3, rx + rw as i32 - 1, ry + rh as i32 - 3, 0x00EF4444);

                            // Piques acérées
                            let spike_count = ((rw / 10).max(2)) as i32;
                            let spike_step = rw as f32 / spike_count as f32;
                            for s in 0..spike_count {
                                let sx1 = rx + (s as f32 * spike_step) as i32;
                                let sx_mid = rx + ((s as f32 + 0.5) * spike_step) as i32;
                                let sx2 = rx + ((s as f32 + 1.0) * spike_step) as i32;
                                let sy_base = ry + rh as i32 - 6;
                                let sy_peak = ry + 2;
                                canvas.draw_line(sx1, sy_base, sx_mid, sy_peak, 0x00E2E8F0);
                                canvas.draw_line(sx_mid, sy_peak, sx2, sy_base, 0x0094A3B8);
                                canvas.draw_line(sx_mid, sy_peak, sx_mid, sy_base, 0x00EF4444);
                            }
                        }
                        "heart" => {
                            // Fiole ou cristal de vitalité
                            let pulse = ((self.elapsed_time * 6.0).sin() * 2.0) as i32;
                            canvas.draw_rect(rx, ry, rw, rh, 0x00831843);
                            canvas.draw_rect_outline(rx, ry, rw, rh, 0x00EC4899, 1);
                            let hx = rx + (rw as i32 / 2) - 5;
                            let hy = ry + (rh as i32 / 2) - 6 + (pulse / 2);
                            canvas.draw_text(hx, hy, "♥", 0x00F472B6, 2);
                        }
                        _ => {
                            // Entité rectangulaire générique
                            canvas.draw_rect(rx, ry, rw, rh, rend.color);
                            if let Some(outline_col) = rend.outline_color {
                                canvas.draw_rect_outline(rx, ry, rw, rh, outline_col, rend.outline_width);
                            }
                        }
                    }

                    if is_selected {
                        canvas.draw_rect_outline(rx - 4, ry - 4, rw + 8, rh + 8, 0x00FBBF24, 3);
                        let label = format!("{}: {}", ent.tag.to_uppercase(), ent.name);
                        canvas.draw_text(rx, ry - 22, &label, 0x00FBBF24, 2);
                    }
                }
                ShapeKind::Circle { radius } => {
                    let (base_cx, base_cy) = self.camera.world_to_screen(ent.transform.x + radius, ent.transform.y + radius);

                    // Micro-animation de bobbing / lévitation pour les pièces et collectibles
                    let (cx, cy, shadow_scale) = if ent.tag == "collectible" {
                        let bob = ((self.elapsed_time * 5.0 + idx as f32 * 1.5).sin() * 3.0) as i32;
                        (base_cx, base_cy + bob, 1.0 - (bob as f32 * 0.05))
                    } else {
                        (base_cx, base_cy, 1.0)
                    };

                    // Ombre projetée au sol
                    let shadow_r = ((radius * 0.7) * shadow_scale) as i32;
                    canvas.draw_circle(base_cx + 2, base_cy + radius as i32 + 4, shadow_r.max(2), 0x00060911);

                    match ent.tag.as_str() {
                        "collectible" => {
                            // Pièce d'or / Émeraude étincelante
                            // Anneau extérieur doré chaud
                            canvas.draw_circle(cx, cy, radius as i32, 0x00D97706);
                            // Face de la pièce
                            canvas.draw_circle(cx, cy, (radius * 0.8) as i32, 0x00F59E0B);
                            // Éclat de réflexion spéculaire en haut à gauche
                            let spec_offset = (radius * 0.35) as i32;
                            canvas.draw_circle(cx - spec_offset, cy - spec_offset, (radius * 0.3).max(2.0) as i32, 0x00FEF08A);
                            // Relief central (losange / glyphe d'or)
                            canvas.draw_rect(cx - 2, cy - 2, 4, 4, 0x00FFFBEB);
                        }
                        "projectile" => {
                            // Projectile balistique / laser étincelant
                            canvas.draw_circle(cx, cy, radius as i32, 0x00FDE047);
                            canvas.draw_circle(cx, cy, (radius * 0.5).max(1.0) as i32, 0x00FFFFFF);
                        }
                        "portal" => {
                            // Portail Quantique dimensionnel (anneaux concentriques)
                            let p_rot = (self.elapsed_time * 3.0).sin();
                            let col1 = if p_rot > 0.0 { 0x0006B6D4 } else { 0x008B5CF6 };
                            let col2 = if p_rot > 0.0 { 0x00A855F7 } else { 0x0022D3EE };
                            canvas.draw_circle(cx, cy, radius as i32, col1);
                            canvas.draw_circle(cx, cy, (radius * 0.7) as i32, col2);
                            canvas.draw_circle(cx, cy, (radius * 0.4) as i32, 0x00FFFFFF);
                        }
                        "heart" => {
                            // Fiole de Vitalité / Cœur Énergétique pulsant
                            let pulse = ((self.elapsed_time * 6.0).sin() * 2.0) as i32;
                            let heart_r = (radius as i32 + pulse).max(6);
                            canvas.draw_circle(cx, cy, heart_r + 2, 0x00831843);
                            canvas.draw_circle(cx, cy, heart_r, 0x00EC4899);
                            canvas.draw_text(cx - 5, cy - 6, "♥", 0x00FFFFFF, 2);
                        }
                        _ => {
                            // Rendu circulaire par défaut
                            canvas.draw_circle(cx, cy, radius as i32, rend.color);
                            canvas.draw_circle(cx, cy, (radius * 0.6) as i32, 0x00FDE047);
                        }
                    }

                    if is_selected {
                        let sz = (radius + 6.0) * 2.0;
                        canvas.draw_rect_outline(
                            base_cx - radius as i32 - 6,
                            base_cy - radius as i32 - 6,
                            sz as u32,
                            sz as u32,
                            0x00FBBF24,
                            3,
                        );
                        let label = format!("{}: {}", ent.tag.to_uppercase(), ent.name);
                        canvas.draw_text(base_cx - 20, base_cy - radius as i32 - 24, &label, 0x00FBBF24, 2);
                    }
                }
                ShapeKind::Text { ref label, scale } => {
                    let (tx, ty) = self.camera.world_to_screen(ent.transform.x, ent.transform.y);
                    canvas.draw_text(tx, ty, label, rend.color, scale as usize);
                }
            }

            // Mini barre de vie au-dessus de toute entité blessée (joueur, ennemi, boss)
            if let Some(ref h) = ent.health {
                if h.current < h.max && !h.is_dead {
                    let bar_w = (ent.transform.w as u32).max(24);
                    let bar_h = 4;
                    let bar_x = ent.transform.x as i32 + ((ent.transform.w as i32 - bar_w as i32) / 2);
                    let bar_y = ent.transform.y as i32 - 7;

                    canvas.draw_rect(bar_x, bar_y, bar_w, bar_h, 0x000F172A);
                    canvas.draw_rect_outline(bar_x, bar_y, bar_w, bar_h, 0x00334155, 1);

                    let pct = (h.current / h.max).clamp(0.0, 1.0);
                    let fill_w = ((bar_w.saturating_sub(2)) as f32 * pct) as u32;
                    let hp_col = if pct > 0.5 {
                        0x0010B981 // Vert
                    } else if pct > 0.25 {
                        0x00F59E0B // Jaune / Orange
                    } else {
                        0x00EF4444 // Rouge critique
                    };
                    if fill_w > 0 {
                        canvas.draw_rect(bar_x + 1, bar_y + 1, fill_w, bar_h.saturating_sub(2), hp_col);
                    }
                }
            }

            // Rendu des cônes de vision et alertes pour les entités douées de perception
            if let Some(ref vision) = ent.vision {
                let (ex, ey) = self.camera.world_to_screen(ent.transform.center_x(), ent.transform.center_y());
                let cone_len = vision.range;
                let cone_col = if vision.is_alerted { 0x00EF4444 } else { 0x0038BDF8 };
                let steps = 7;
                for s in 0..=steps {
                    let factor = (s as f32 / steps as f32) - 0.5;
                    let target_x = ex - cone_len as i32;
                    let target_y = ey + (factor * cone_len * 1.3) as i32;
                    canvas.draw_line(ex - (ent.transform.w as i32 / 2), ey, target_x, target_y, cone_col);
                }

                if vision.is_alerted {
                    canvas.draw_badge(ex - 18, ey - (ent.transform.h as i32 / 2) - 30, 36, 20, 0x007F1D1D, 0x00EF4444, "(!)", 0x00FEE2E2, 1);
                }
            }
        }

        // Particules étincelantes défilantes et frustum culled
        for p in &self.particles {
            if self.camera.is_visible(p.x, p.y, 4.0, 4.0) {
                let (px, py) = self.camera.world_to_screen(p.x, p.y);
                let col = p.current_color();
                canvas.draw_rect(px, py, 3, 3, col);
                canvas.set_pixel(px + 1, py + 1, 0x00FFFFFF);
            }
        }

        // Curseur fantôme (Ghost Preview) pour le Pinceau, la Gomme et la Sélection
        if is_edit_mode {
            let mx = self.editor.mouse_x;
            let my = self.editor.mouse_y;
            if mx >= vx as f32 && mx < (vx + vw) as f32 && my >= vy as f32 && my < (vy + vh) as f32 {
                let (world_mx, world_my) = self.camera.screen_to_world(mx, my);
                let cell_col = (world_mx / 32.0).floor() as i32;
                let cell_row = (world_my / 32.0).floor() as i32;
                let world_gx = cell_col as f32 * 32.0;
                let world_gy = cell_row as f32 * 32.0;
                let (gx, gy) = self.camera.world_to_screen(world_gx, world_gy);

                match self.editor.active_tool {
                    StudioTool::Brush => {
                        let col = self.editor.brush_tile.color();
                        canvas.draw_rect_outline(gx, gy, 32, 32, col, 2);
                        canvas.draw_corner_brackets(gx, gy, 32, 32, 6, col);
                        let short_lbl = self.editor.brush_tile.short_label();
                        canvas.draw_text(gx + 3, gy + 11, short_lbl, col, 1);
                    }
                    StudioTool::Eraser => {
                        canvas.draw_rect_outline(gx, gy, 32, 32, 0x00EF4444, 2);
                        canvas.draw_line(gx + 4, gy + 4, gx + 28, gy + 28, 0x00EF4444);
                        canvas.draw_line(gx + 28, gy + 4, gx + 4, gy + 28, 0x00EF4444);
                    }
                    StudioTool::Select => {
                        if self.editor.selected == SelectedEntity::None {
                            canvas.draw_rect_outline(gx, gy, 32, 32, 0x0038BDF8, 1);
                        }
                    }
                }
            }
        }

        // Désactivation du scissor clipping avant le rendu de l'interface (Minimap, Inspecteur, HUD)
        canvas.set_clip_rect(None);

        // 4. Rendu du Radar Tactique 2D Universel (Minimap)
        if self.show_minimap {
            let mm_w = 175;
            let mm_h = 110;
            let mm_x = if is_edit_mode { w - 460 - mm_w - 20 } else { w - mm_w - 30 };
            let mm_y = 72;

            canvas.draw_rect(mm_x, mm_y, mm_w as u32, mm_h as u32, 0x0008101E);
            canvas.draw_rect_outline(mm_x, mm_y, mm_w as u32, mm_h as u32, 0x000284C7, 2);
            canvas.draw_corner_brackets(mm_x, mm_y, mm_w as u32, mm_h as u32, 8, 0x0038BDF8);

            canvas.draw_rect(mm_x, mm_y, mm_w as u32, 18, 0x000369A1);
            canvas.draw_text(mm_x + 8, mm_y + 4, "RADAR TACTIQUE (N)", 0x00E0F2FE, 1);

            // Anneaux concentriques radar
            let rcx = mm_x + (mm_w / 2);
            let rcy = mm_y + 18 + ((mm_h - 18) / 2);
            canvas.draw_circle(rcx, rcy, 22, 0x000D2847);
            canvas.draw_circle(rcx, rcy, 40, 0x000A1C30);
            canvas.draw_line(mm_x, rcy, mm_x + mm_w, rcy, 0x000E2A47);
            canvas.draw_line(rcx, mm_y + 18, rcx, mm_y + mm_h, 0x000E2A47);

            let world_w = (self.world_bounds.2 - self.world_bounds.0).max(100.0);
            let world_h = (self.world_bounds.3 - self.world_bounds.1).max(100.0);
            let scale_x = (mm_w - 14) as f32 / world_w;
            let scale_y = (mm_h - 32) as f32 / world_h;

            let map_to_radar = |rx: f32, ry: f32| -> (i32, i32) {
                let px = mm_x + 7 + ((rx - self.world_bounds.0) * scale_x) as i32;
                let py = mm_y + 22 + ((ry - self.world_bounds.1) * scale_y) as i32;
                (px, py)
            };

            for ent in &self.entities {
                if !ent.is_active {
                    continue;
                }
                let (ex, ey) = map_to_radar(ent.transform.x, ent.transform.y);
                match ent.tag.as_str() {
                    "player" => {
                        canvas.draw_rect(ex - 2, ey - 2, 5, 5, 0x0038BDF8);
                        canvas.draw_rect_outline(ex - 4, ey - 4, 9, 9, 0x00FFFFFF, 1);
                    }
                    "enemy" => {
                        let is_alerted = ent.vision.as_ref().map_or(false, |v| v.is_alerted);
                        let ecol = if is_alerted { 0x00EF4444 } else { 0x00A855F7 };
                        canvas.draw_rect(ex - 2, ey - 2, 5, 5, ecol);
                        canvas.draw_line(ex, ey, ex - 8, ey, ecol);
                    }
                    "collectible" => {
                        canvas.draw_rect(ex - 1, ey - 1, 3, 3, 0x00FDE047);
                    }
                    "door" => {
                        let col = ent.renderable.as_ref().map_or(0x0010B981, |r| r.color);
                        canvas.draw_rect(ex, ey, 4, 8, col);
                    }
                    "wall" => {
                        let ww = ((ent.transform.w * scale_x) as u32).max(2);
                        let wh = ((ent.transform.h * scale_y) as u32).max(2);
                        canvas.draw_rect(ex, ey, ww, wh, 0x00475569);
                    }
                    "torch" => {
                        canvas.draw_rect(ex - 1, ey - 1, 3, 3, 0x00F97316);
                    }
                    "hazard" => {
                        canvas.draw_rect(ex - 1, ey - 1, 3, 3, 0x00EF4444);
                    }
                    "heart" => {
                        canvas.draw_rect(ex - 1, ey - 1, 3, 3, 0x00EC4899);
                    }
                    _ => {
                        canvas.draw_rect(ex, ey, 3, 3, 0x0094A3B8);
                    }
                }
            }

            // Cadre jaune représentant le frustum de la caméra 2D dans le monde complet
            let (cam_rx, cam_ry) = map_to_radar(self.camera.x, self.camera.y);
            let cam_rw = ((self.camera.viewport_w as f32 * scale_x) as u32).max(4);
            let cam_rh = ((self.camera.viewport_h as f32 * scale_y) as u32).max(4);
            let clamped_cam_x = cam_rx.clamp(mm_x + 2, mm_x + mm_w - 6);
            let clamped_cam_y = cam_ry.clamp(mm_y + 20, mm_y + mm_h - 6);
            let clamped_cam_w = cam_rw.min((mm_x + mm_w - 4).saturating_sub(clamped_cam_x) as u32);
            let clamped_cam_h = cam_rh.min((mm_y + mm_h - 4).saturating_sub(clamped_cam_y) as u32);
            canvas.draw_rect_outline(clamped_cam_x, clamped_cam_y, clamped_cam_w, clamped_cam_h, 0x00FDE047, 1);
        }

        // ========================================================
        // PANNEAU INSPECTEUR KVL UNIVERSEL (EN MODE ÉDITION) : Largeur 450px
        // ========================================================
        if is_edit_mode {
            let insp_x = w - 460;
            let insp_y = 66;
            let insp_w = 440;
            let insp_h = play_area_h;

            self.editor.pin_hitboxes.clear();
            self.editor.connection_hitboxes.clear();
            self.editor.palette_hitboxes.clear();
            self.editor.save_btn_hitbox = None;
            self.editor.add_node_btn_hitbox = None;
            self.editor.view_btn_hitbox = None;

            canvas.draw_rect(insp_x, insp_y, insp_w as u32, insp_h, 0x000F172A);
            canvas.draw_rect_outline(insp_x, insp_y, insp_w as u32, insp_h, 0x007C3AED, 2);
            canvas.draw_corner_brackets(insp_x, insp_y, insp_w as u32, insp_h, 16, 0x00A855F7);

            canvas.draw_badge(insp_x + 18, insp_y + 14, (insp_w - 36) as u32, 28, 0x002E1065, 0x00A855F7, "INSPECTEUR D'OBJET & KVL", 0x00F3E8FF, 2);

            match self.editor.selected {
                SelectedEntity::None => {
                    // Carte pédagogique de bienvenue dans le studio
                    let card_y = insp_y + 54;
                    canvas.draw_badge(insp_x + 18, card_y, (insp_w - 36) as u32, 26, 0x001E293B, 0x0038BDF8, "GUIDE PEDAGOGIQUE DU STUDIO", 0x00E0F2FE, 2);

                    let guide_steps = [
                        ("1. SELECTIONNER", "Cliquez sur une entite (ex: Joueur, Porte, Torche)", 0x0038BDF8),
                        ("2. DEPLACER", "Glissez-deposez a la souris pour positionner l'entite", 0x0034D399),
                        ("3. COMPOSER ECS", "Ajoutez Collider2D, Movement, Vision ou Emetteur", 0x00FBBF24),
                        ("4. LOGIQUE KVL", "Editez les noeuds visuels et reliez les broches In / Out", 0x00C084FC),
                        ("5. TESTER EN DIRECT", "Appuyez sur TAB pour jouer sans recharger !", 0x00F43F5E),
                    ];

                    let mut sy = card_y + 36;
                    for (step_title, step_desc, col) in guide_steps {
                        canvas.draw_rect(insp_x + 18, sy, (insp_w - 36) as u32, 46, 0x00131C2E);
                        canvas.draw_rect_outline(insp_x + 18, sy, (insp_w - 36) as u32, 46, col, 1);
                        canvas.draw_text(insp_x + 26, sy + 6, step_title, col, 2);
                        canvas.draw_text(insp_x + 26, sy + 28, step_desc, 0x0094A3B8, 1);
                        sy += 54;
                    }

                    canvas.draw_rect(insp_x + 18, sy + 6, (insp_w - 36) as u32, 1, 0x00334155);
                    canvas.draw_text(insp_x + 18, sy + 16, "COMMANDES RAPIDES :", 0x00FBBF24, 2);
                    canvas.draw_text(insp_x + 18, sy + 38, "P: Spawn | C+D: Dupliquer | Del: Suppr | C+S: Sauver", 0x00E2E8F0, 1);
                    canvas.draw_text(insp_x + 18, sy + 54, "Espace: Vue KVL | H: Guide complet | TAB: Mode Jeu", 0x0038BDF8, 1);
                }
                SelectedEntity::Entity(idx) if idx < self.entities.len() => {
                    let ent = &self.entities[idx];
                    let ent_title = format!("{} [{}]", ent.name.to_uppercase(), ent.tag.to_uppercase());
                    let pos_str = format!("Pos: X:{:.0} Y:{:.0} | Dim: {:.0}x{:.0}", ent.transform.x, ent.transform.y, ent.transform.w, ent.transform.h);
                    let ent_col = ent.renderable.as_ref().map_or(0x0038BDF8, |r| r.color);

                    if let Some(ref script_comp) = ent.script {
                        let p = script_comp.script_path.clone();
                        self.render_entity_inspector(canvas, insp_x, insp_y, insp_w, &ent_title, ent_col, &pos_str, &p);
                    } else {
                        // Inspecteur de composant pour entité non scriptée (ex: mur solide, torche)
                        canvas.draw_badge(insp_x + 18, insp_y + 54, (insp_w - 36) as u32, 28, 0x00131C2E, ent_col, &ent_title, ent_col, 2);
                        canvas.draw_text(insp_x + 24, insp_y + 90, &pos_str, 0x00F8FAFC, 1);

                        canvas.draw_rect(insp_x + 18, insp_y + 115, (insp_w - 36) as u32, 1, 0x00334155);
                        canvas.draw_text(insp_x + 18, insp_y + 128, "COMPOSANTS ACTIFS (ECS) :", 0x00FBBF24, 2);

                        let mut comp_y = insp_y + 154;
                        let row_w = (insp_w - 36) as u32;
                        if let Some(ref col) = ent.collider {
                            let col_desc = if col.is_pushable {
                                "Collider2D : Bloc Poussable (Puzzle / Sokoban)"
                            } else if col.is_solid {
                                "Collider2D : Solide AABB (Bloquant)"
                            } else {
                                "Collider2D : Trigger (Zone detection)"
                            };
                            canvas.draw_badge(insp_x + 18, comp_y, row_w, 24, 0x00064E3B, 0x0010B981, col_desc, 0x00A7F3D0, 1);
                            comp_y += 28;
                            if col.is_solid {
                                let push_txt = if col.is_pushable { "[ POUSSABLE : OUI  > rendre fixe ]" } else { "[ POUSSABLE : NON  > rendre poussable ]" };
                                canvas.draw_badge(insp_x + 30, comp_y, row_w - 12, 22, 0x00131C2E, 0x0034D399, push_txt, 0x006EE7B7, 1);
                                self.editor.add_comp_hitboxes.push(("TogglePushable".into(), insp_x + 30, comp_y, row_w - 12, 22));
                                comp_y += 28;
                            }
                        }
                        if let Some(ref mov) = ent.movement {
                            let role = if mov.is_player_controlled { "Joueur" } else { "IA" };
                            let mov_desc = format!("Movement2D : {} | Physique {}", role, mov.mode.label());
                            canvas.draw_badge(insp_x + 18, comp_y, row_w, 24, 0x000C4A6E, 0x000284C7, &mov_desc, 0x00BAE6FD, 1);
                            comp_y += 28;
                            let next_txt = format!("[ CHANGER PHYSIQUE > {} ]", mov.mode.next_preset().label());
                            canvas.draw_badge(insp_x + 30, comp_y, row_w - 12, 22, 0x00131C2E, 0x0038BDF8, &next_txt, 0x0038BDF8, 1);
                            self.editor.add_comp_hitboxes.push(("CycleMovementMode".into(), insp_x + 30, comp_y, row_w - 12, 22));
                            comp_y += 26;

                            // Paramètres physiques ajustables en direct
                            let sub_w = row_w - 12;
                            let speed_label = format!("Vitesse: {:.0} px/s", mov.speed);
                            canvas.draw_badge(insp_x + 30, comp_y, sub_w - 56, 20, 0x000F172A, 0x000369A1, &speed_label, 0x007DD3FC, 1);
                            canvas.draw_badge(insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20, 0x001E293B, 0x00EF4444, " - ", 0x00F87171, 1);
                            self.editor.add_comp_hitboxes.push(("SpeedDec".into(), insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20));
                            canvas.draw_badge(insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20, 0x001E293B, 0x0010B981, " + ", 0x0034D399, 1);
                            self.editor.add_comp_hitboxes.push(("SpeedInc".into(), insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20));
                            comp_y += 24;

                            match mov.mode {
                                MovementMode::Platformer { jump_force, gravity, .. } => {
                                    let jump_label = format!("Saut: {:.0} px/s", jump_force);
                                    canvas.draw_badge(insp_x + 30, comp_y, sub_w - 56, 20, 0x00064E3B, 0x00059669, &jump_label, 0x006EE7B7, 1);
                                    canvas.draw_badge(insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20, 0x001E293B, 0x00EF4444, " - ", 0x00F87171, 1);
                                    self.editor.add_comp_hitboxes.push(("JumpDec".into(), insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20));
                                    canvas.draw_badge(insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20, 0x001E293B, 0x0010B981, " + ", 0x0034D399, 1);
                                    self.editor.add_comp_hitboxes.push(("JumpInc".into(), insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20));
                                    comp_y += 24;

                                    let grav_label = format!("Gravite: {:.0}", gravity);
                                    canvas.draw_badge(insp_x + 30, comp_y, sub_w - 56, 20, 0x00451A03, 0x00D97706, &grav_label, 0x00FDE047, 1);
                                    canvas.draw_badge(insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20, 0x001E293B, 0x00EF4444, " - ", 0x00F87171, 1);
                                    self.editor.add_comp_hitboxes.push(("GravityDec".into(), insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20));
                                    canvas.draw_badge(insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20, 0x001E293B, 0x0010B981, " + ", 0x0034D399, 1);
                                    self.editor.add_comp_hitboxes.push(("GravityInc".into(), insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20));
                                    comp_y += 24;
                                }
                                MovementMode::SpaceInertia { thrust, .. } => {
                                    let thrust_label = format!("Poussee: {:.0}", thrust);
                                    canvas.draw_badge(insp_x + 30, comp_y, sub_w - 56, 20, 0x002E1065, 0x007C3AED, &thrust_label, 0x00DDD6FE, 1);
                                    canvas.draw_badge(insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20, 0x001E293B, 0x00EF4444, " - ", 0x00F87171, 1);
                                    self.editor.add_comp_hitboxes.push(("ThrustDec".into(), insp_x + 30 + (sub_w - 52) as i32, comp_y, 24, 20));
                                    canvas.draw_badge(insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20, 0x001E293B, 0x0010B981, " + ", 0x0034D399, 1);
                                    self.editor.add_comp_hitboxes.push(("ThrustInc".into(), insp_x + 30 + (sub_w - 24) as i32, comp_y, 24, 20));
                                    comp_y += 24;
                                }
                                _ => {}
                            }
                        }
                        if let Some(ref vis) = ent.vision {
                            let vis_desc = format!("VisionSensing : Radar Raycast (Portee: {:.0}px)", vis.range);
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x00450A0A, 0x00DC2626, &vis_desc, 0x00FECACA, 1);
                            comp_y += 30;
                        }
                        if let Some(ref em) = ent.emitter {
                            let em_status = if em.is_emitting { "Actif" } else { "En Pause" };
                            let em_desc = format!("ParticleEmitter2D : Flammes ({:.0} p/s, {})", em.rate, em_status);
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x00431407, 0x00EA580C, &em_desc, 0x00FFEDD5, 1);
                            comp_y += 30;
                        }
                        if ent.renderable.is_some() {
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x002E1065, 0x007C3AED, "Renderable2D : Forme Vectorielle Logique", 0x00EDE9FE, 1);
                            comp_y += 30;
                        }
                        if let Some(ref h) = ent.health {
                            let health_desc = format!("Health2D : {:.0} / {:.0} PV", h.current, h.max);
                            canvas.draw_badge(insp_x + 18, comp_y, row_w, 24, 0x004C0519, 0x00E11D48, &health_desc, 0x00FECDD3, 1);
                            comp_y += 28;

                            let sub_w = row_w - 12;
                            canvas.draw_badge(insp_x + 30, comp_y, sub_w / 2 - 4, 20, 0x001E293B, 0x00EF4444, " - 1 PV ", 0x00FCA5A5, 1);
                            self.editor.add_comp_hitboxes.push(("DamageSelected".into(), insp_x + 30, comp_y, sub_w / 2 - 4, 20));

                            canvas.draw_badge(insp_x + 30 + (sub_w / 2) as i32 + 4, comp_y, sub_w / 2 - 4, 20, 0x001E293B, 0x0010B981, " + 1 PV ", 0x0086EFAC, 1);
                            self.editor.add_comp_hitboxes.push(("HealSelected".into(), insp_x + 30 + (sub_w / 2) as i32 + 4, comp_y, sub_w / 2 - 4, 20));
                            comp_y += 24;

                            canvas.draw_badge(insp_x + 30, comp_y, sub_w / 2 - 4, 20, 0x001E293B, 0x0094A3B8, " - Max PV ", 0x00CBD5E1, 1);
                            self.editor.add_comp_hitboxes.push(("MaxHpDec".into(), insp_x + 30, comp_y, sub_w / 2 - 4, 20));

                            canvas.draw_badge(insp_x + 30 + (sub_w / 2) as i32 + 4, comp_y, sub_w / 2 - 4, 20, 0x001E293B, 0x0038BDF8, " + Max PV ", 0x00BAE6FD, 1);
                            self.editor.add_comp_hitboxes.push(("MaxHpInc".into(), insp_x + 30 + (sub_w / 2) as i32 + 4, comp_y, sub_w / 2 - 4, 20));
                            comp_y += 26;
                        }

                        // Boutons d'ajout de composants manquants
                        comp_y += 8;
                        canvas.draw_rect(insp_x + 18, comp_y, (insp_w - 36) as u32, 1, 0x00334155);
                        comp_y += 12;
                        canvas.draw_text(insp_x + 18, comp_y, "ATTACHER UN COMPOSANT :", 0x0038BDF8, 2);
                        comp_y += 24;

                        let btn_w = (insp_w - 36) as u32;
                        let btn_h = 28;

                        if ent.collider.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x0034D399, "[ + COLLIDER 2D (SOLIDE AABB) ]", 0x0034D399, 2);
                            self.editor.add_comp_hitboxes.push(("Collider".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.movement.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x0038BDF8, "[ + MOVEMENT 2D (CINEMATIQUE) ]", 0x0038BDF8, 2);
                            self.editor.add_comp_hitboxes.push(("Movement".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.vision.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00F87171, "[ + VISION SENSING (RADAR) ]", 0x00F87171, 2);
                            self.editor.add_comp_hitboxes.push(("Vision".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.emitter.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00F97316, "[ + PARTICLE EMITTER (FLAMMES) ]", 0x00F97316, 2);
                            self.editor.add_comp_hitboxes.push(("Emitter".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.health.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00F43F5E, "[ + SANTE / VIE (HEALTH2D) ]", 0x00FDA4AF, 2);
                            self.editor.add_comp_hitboxes.push(("Health".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.script.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00A78BFA, "[ + SCRIPT KVL (LOGIQUE) ]", 0x00A78BFA, 2);
                            self.editor.add_comp_hitboxes.push(("Script".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.dialogue.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x0038BDF8, "[ + DIALOGUE PNJ & QUETE ]", 0x0038BDF8, 2);
                            self.editor.add_comp_hitboxes.push(("Dialogue".into(), insp_x + 18, comp_y, btn_w, btn_h));
                        }
                    }
                }
                _ => {}
            }
        }

        // Overlay de la palette de spawn d'entités (Touche P ou Bouton [+ SPAWN])
        if is_edit_mode && self.editor.show_spawn_palette {
            let pal_w = 440;
            let pal_h = 580;
            let pal_x = 160;
            let pal_y = 40;

            canvas.draw_rect(pal_x, pal_y, pal_w as u32, pal_h as u32, 0x000F172A);
            canvas.draw_rect_outline(pal_x, pal_y, pal_w as u32, pal_h as u32, 0x0038BDF8, 2);
            canvas.draw_corner_brackets(pal_x, pal_y, pal_w as u32, pal_h as u32, 16, 0x0038BDF8);

            canvas.draw_rect(pal_x, pal_y, pal_w as u32, 34, 0x000284C7);
            canvas.draw_text(pal_x + 14, pal_y + 8, "STUDIO : CREER UNE NOUVELLE ENTITE", 0x00FFFFFF, 2);

            let spawn_templates = [
                ("wall", "+ Mur Solide AABB (Bloquant)", 0x0094A3B8),
                ("collectible", "+ Objet Collectible (Piece KVL)", 0x00F59E0B),
                ("door", "+ Porte Interactive Securisee", 0x00EF4444),
                ("enemy", "+ Sentinelle Patrouilleuse (Vision)", 0x00DC2626),
                ("hazard", "+ Piege a Piques (Degats -1 PV)", 0x00EF4444),
                ("heart", "+ Fiole de Coeur (Soin +1 PV)", 0x00EC4899),
                ("portal", "+ Portail Quantique (Teleporteur)", 0x0006B6D4),
                ("torch", "+ Torche Brulante (Emetteur Flammes)", 0x00F97316),
                ("crate", "+ Bloc Poussable (Puzzle / Sokoban)", 0x00F4AF6A),
                ("plate", "+ Dalle de Pression (Ouvre les portes)", 0x0034D399),
                ("npc", "+ PNJ / Gardien (Dialogues KVL & Quetes)", 0x0038BDF8),
                ("cancel", "[ Fermer la palette (Echap) ]", 0x0064748B),
            ];

            for (idx, (tpl_id, tpl_name, col)) in spawn_templates.iter().enumerate() {
                let item_y = pal_y + 44 + (idx as i32 * 44);
                let item_w = pal_w - 24;
                let item_h = 34;

                canvas.draw_badge(pal_x + 12, item_y, item_w as u32, item_h as u32, 0x001E293B, *col, tpl_name, *col, 2);

                self.editor.spawn_palette_hitboxes.push(crate::editor::PaletteItemHitbox {
                    template_id: tpl_id.to_string(),
                    x: pal_x + 12,
                    y: item_y,
                    w: item_w as u32,
                    h: item_h as u32,
                });
            }
        }

        // 3. Bandeau inférieur : Commandes pédagogiques, statut et traces KVL (Hauteur 175px)
        let bottom_y = h - 175;
        canvas.draw_rect(0, bottom_y, w as u32, 175, 0x000F172A);
        canvas.draw_rect(0, bottom_y, w as u32, 2, 0x001E293B);

        // Ruban de commandes rapides claires et pédagogiques
        if is_edit_mode {
            let pills = [
                ("V: Select", 0x001E3A8A, 0x0093C5FD),
                ("B: Pinceau", 0x007C2D12, 0x00FDBA74),
                ("T: Tuile", 0x00065F46, 0x006EE7B7),
                ("X: Gomme", 0x007F1D1D, 0x00FCA5A5),
                ("P: Spawn", 0x007C3AED, 0x00C084FC),
                ("C+D: Dupliquer", 0x00B45309, 0x00FDE68A),
                ("Del: Suppr", 0x00B91C1C, 0x00FECACA),
                ("C+S: Sauver", 0x00047857, 0x00A7F3D0),
                ("Espace: KVL", 0x000369A1, 0x00BAE6FD),
                ("H: Guide", 0x004338CA, 0x00E0E7FF),
                ("TAB: Mode Jeu", 0x00065F46, 0x00D1FAE5),
            ];
            let mut px = 20;
            for (p_txt, p_bg, p_fg) in pills {
                let pw = (p_txt.len() as i32 * 12 + 18) as u32;
                canvas.draw_badge(px, bottom_y + 8, pw, 26, p_bg, p_fg, p_txt, p_fg, 2);
                px += pw as i32 + 8;
            }
        } else {
            let pills = [
                ("F4 / L: Changer Genre", 0x00047857, 0x00D1FAE5),
                ("ZQSD / Fleches: Bouger", 0x000369A1, 0x00E0F2FE),
                ("F: Tirer Laser", 0x00B45309, 0x00FDE68A),
                ("E: Interagir", 0x001E293B, 0x0094A3B8),
                ("TAB: Studio ECS", 0x006B21A8, 0x00F3E8FF),
                ("H: Guide", 0x004338CA, 0x00E0E7FF),
                ("N: Radar", 0x001E293B, 0x0094A3B8),
                ("G: Grille", 0x001E293B, 0x0094A3B8),
                ("M: Son", 0x001E293B, 0x0094A3B8),
            ];
            let mut px = 20;
            for (p_txt, p_bg, p_fg) in pills {
                let pw = (p_txt.len() as i32 * 12 + 18) as u32;
                canvas.draw_badge(px, bottom_y + 8, pw, 26, p_bg, p_fg, p_txt, p_fg, 2);
                px += pw as i32 + 8;
            }
        }

        // Ligne de statut du moteur
        canvas.draw_badge(20, bottom_y + 42, 110, 24, 0x001E293B, 0x0038BDF8, "ETAT MOTEUR", 0x0038BDF8, 1);
        canvas.draw_text(140, bottom_y + 46, &self.status_message, 0x00F8FAFC, 2);

        // Console de traçabilité d'exécution KVL
        let console_y = bottom_y + 74;
        canvas.draw_rect(20, console_y, (w - 40) as u32, 92, 0x000A0F1D);
        canvas.draw_rect_outline(20, console_y, (w - 40) as u32, 92, 0x001E293B, 1);
        canvas.draw_text(30, console_y + 6, "CONSOLE RUNTIME KVL (FLUX REACTIF DE LA LOGIQUE) :", 0x0064748B, 1);

        let log_count = self.kvl_context.execution_log.len();
        let start_idx = log_count.saturating_sub(3);
        if self.kvl_context.execution_log.is_empty() {
            canvas.draw_text(30, console_y + 30, "En attente d'evenements d'execution (deplacez-vous sur une piece ou vers la porte)...", 0x00475569, 2);
        } else {
            for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
                let trace_line = format!("> [{}] {} : {}", step.node_id, step.node_name, step.message);
                canvas.draw_text(30, console_y + 26 + (i as i32 * 20), &trace_line, 0x00A7F3D0, 2);
            }
        }

        // Overlay Télémétrie en Mode Jeu (F1)
        if !is_edit_mode && self.telemetry.show_overlay {
            let ov_w = 400;
            let ov_h = 346;
            let ov_x = w - ov_w - 20;
            let ov_y = 70;

            canvas.draw_rect(ov_x, ov_y, ov_w as u32, ov_h as u32, 0x000A0F1D);
            canvas.draw_rect_outline(ov_x, ov_y, ov_w as u32, ov_h as u32, 0x0038BDF8, 2);
            canvas.draw_corner_brackets(ov_x, ov_y, ov_w as u32, ov_h as u32, 12, 0x0038BDF8);

            canvas.draw_text(ov_x + 16, ov_y + 16, "PROFILAGE & TELEMETRIE (F1)", 0x0038BDF8, 2);

            let fps_line = format!("FPS : {:.1} (Cible : {:.0})", self.telemetry.current_fps, self.adaptation.target_fps);
            canvas.draw_text(ov_x + 16, ov_y + 42, &fps_line, 0x00F8FAFC, 2);

            let cpu_line = format!("CPU Frame : {} us (Avg)", self.telemetry.avg_frame_time_us);
            canvas.draw_text(ov_x + 16, ov_y + 66, &cpu_line, 0x0034D399, 2);

            let budget_status = if self.telemetry.avg_frame_time_us <= self.adaptation.target_budget_us {
                "BUDGET : RESPECTE (<16.6ms)"
            } else {
                "BUDGET : DEPASSE !"
            };
            let budget_color = if self.telemetry.avg_frame_time_us <= self.adaptation.target_budget_us { 0x0010B981 } else { 0x00EF4444 };
            canvas.draw_text(ov_x + 16, ov_y + 90, budget_status, budget_color, 2);

            let auto_str = if self.adaptation.auto_adapt { "AUTO-ADAPTATIF (ON)" } else { "MANUEL (OFF)" };
            canvas.draw_text(ov_x + 16, ov_y + 114, auto_str, 0x00FCD34D, 2);

            let particles_line = format!("Particules : {} / {}", self.particles.len(), self.adaptation.current_profile.max_particles());
            canvas.draw_text(ov_x + 16, ov_y + 138, &particles_line, 0x0094A3B8, 2);

            let spatial_line = format!("Grille Spatiale : {} req | {} cand", self.spatial_grid.stats_queries, self.spatial_grid.stats_candidates);
            canvas.draw_text(ov_x + 16, ov_y + 160, &spatial_line, 0x0038BDF8, 2);

            canvas.draw_text(ov_x + 16, ov_y + 186, "TEMPS DE FRAME (HISTORIQUE) :", 0x0094A3B8, 2);
            let graph_y = ov_y + 210;
            let graph_h = 100;
            canvas.draw_rect(ov_x + 16, graph_y, (ov_w - 32) as u32, graph_h as u32, 0x00161F33);
            canvas.draw_rect_outline(ov_x + 16, graph_y, (ov_w - 32) as u32, graph_h as u32, 0x002B364F, 1);

            let samples = &self.telemetry.frame_times_us;
            for (i, &time_us) in samples.iter().enumerate() {
                let bar_x = (ov_x + 18) + (i as i32 * 3);
                if bar_x < (ov_x + ov_w - 18) {
                    let bar_h = ((time_us as f32 / 3000.0) * (graph_h as f32 * 0.7)).min(graph_h as f32 - 4.0) as u32;
                    let bar_color = if time_us > 16_666 { 0x00EF4444 } else if time_us > 8_000 { 0x00FBBF24 } else { 0x0010B981 };
                    canvas.draw_rect(bar_x, graph_y + graph_h - (bar_h as i32) - 2, 2, bar_h.max(2), bar_color);
                }
            }
        }

        // 4. Suivi de quête HUD (en haut à droite sous le bandeau en Mode Jeu)
        if !is_edit_mode {
            if let Some(ref q) = self.quest {
                let q_x = if self.telemetry.show_overlay { 24 } else { (w - 296).max(20) };
                let q_y = 66;
                q.render(canvas, q_x, q_y);
            }
        }

        // 5. Boîte de Dialogue, Effet Machine à Écrire & Choix Multiples (au premier plan)
        if self.dialogue.is_active {
            self.dialogue.render(canvas);
        }

        // Fenêtre Modale de Guide Pédagogique Interactif (Touche H ou Bouton ? AIDE)
        if self.editor.show_help_modal {
            self.render_help_modal(canvas, w, h);
        }
    }

    /// Rendu de la fenêtre modale de guide pédagogique interactif
    fn render_help_modal(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = 760;
        let mh = 560;
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        // Ombrage d'arrière-plan
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000F172A);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x0038BDF8, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 20, 0x0038BDF8);

        // Barre d'en-tête
        canvas.draw_rect(mx, my, mw as u32, 40, 0x000284C7);
        canvas.draw_text(mx + 16, my + 11, "GUIDE PEDAGOGIQUE ET ARCHITECTURE DU MOTEUR", 0x00FFFFFF, 2);
        canvas.draw_badge(mx + mw - 110, my + 7, 95, 26, 0x000369A1, 0x0038BDF8, "[ECHAP]", 0x00E0F2FE, 1);

        // Section 1 : ARCHITECTURE UNIVERSELLE & MULTI-GENRES
        let mut cy = my + 54;
        canvas.draw_badge(mx + 20, cy, 380, 24, 0x001E293B, 0x0038BDF8, "1. ARCHITECTURE UNIVERSELLE & MULTI-GENRES", 0x0038BDF8, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "- MOTEUR AGNOSTIQUE : Concu pour creer TOUS les genres de jeu 2D sans limitation :", 0x00F8FAFC, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Top-Down RPG : Deplacement libre 4/8 axes, sentinelles a cone de vue raycaste.", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Plateforme 2D : Gravite continue, impulsion de saut, plateformes suspendues.", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Shoot'em Up Spatial : Poussee vectorielle, inertie newtonienne, rotation, lasers.", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Puzzle & Sokoban : Deplacement quantifie par grille, caisses poussables, dalles.", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- BASCULE LIVE : [F4 / L] Cycle instantane entre les 4 genres de jeu de demonstration.", 0x0038BDF8, 1);
        cy += 28;

        // Section 2 : ARCHITECTURE ENTITE-COMPOSANTS (ECS)
        canvas.draw_badge(mx + 20, cy, 330, 24, 0x001E293B, 0x0010B981, "2. ARCHITECTURE ENTITE-COMPOSANTS (ECS)", 0x0010B981, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "- ENTITE : Conteneur universel modulaire identifie par ID unique, nom et tag.", 0x00E2E8F0, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- COMPOSANTS : Caracteristiques modulaires combinables a volonte :", 0x00E2E8F0, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Transform2D | Collider2D (Solide, Trigger, Bloc Poussable)", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Movement2D (TopDown, Platformer, SpaceInertia, Grid) | VisionSensing", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Projectile2D (Balistique & degats) | ParticleEmitter2D | KVLScriptComponent", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Health2D (Vitalite, degats, soins, i-frames, mort & respawn automatique)", 0x00F43F5E, 1);
        cy += 28;

        // Section 3 : LANGAGE VISUEL KVL
        canvas.draw_badge(mx + 20, cy, 310, 24, 0x001E293B, 0x00F59E0B, "3. LE LANGAGE VISUEL KVL EN ACTION", 0x00F59E0B, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "- EVENT : Declenche par le moteur (OnOverlap, OnInteract, OnSeen, OnHit...).", 0x00FDE047, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- CONDITION : Logique orientant vers [Vrai/Faux] (ex: Score, Cle, IsPlayerAlive...).", 0x00FDE047, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- ACTION : Effet de jeu (BonusPoints, BoostSpeed, Damage/HealPlayer, Burst...).", 0x00FDE047, 1);
        cy += 28;

        // Section 4 : RACCOURCIS CLAVIER & CONTROLES
        canvas.draw_badge(mx + 20, cy, 340, 24, 0x001E293B, 0x00A855F7, "4. RACCOURCIS CLAVIER & COMMANDES", 0x00A855F7, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "F4 / L : Changer Genre | F : Tirer Laser | Espace : Saut / Poussee | TAB : Mode Studio", 0x00C084FC, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "P : Spawn Entite | C+D : Dupliquer | Suppr : Effacer | G : Grille O(1) | M : Audio | F1 : Taux", 0x00C084FC, 1);

        // Bouton de fermeture
        let close_btn_y = my + mh - 46;
        canvas.draw_badge(mx + 20, close_btn_y, (mw - 40) as u32, 34, 0x00065F46, 0x0010B981, "[ FERMER LE GUIDE PEDAGOGIQUE (H OU ECHAP) ]", 0x00ECFDF5, 2);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_entity_inspector(
        &mut self,
        canvas: &mut Canvas,
        insp_x: i32,
        insp_y: i32,
        insp_w: i32,
        title: &str,
        title_color: u32,
        pos_str: &str,
        script_path: &str,
    ) {
        canvas.draw_text(insp_x + 18, insp_y + 55, title, title_color, 2);
        canvas.draw_text(insp_x + 18, insp_y + 78, pos_str, 0x00F8FAFC, 2);

        let btn_w = (insp_w - 36) as u32;
        let btn_half_w = (btn_w - 8) / 2;

        let row1_y = insp_y + 105;
        let save_x = insp_x + 18;
        let add_x = save_x + btn_half_w as i32 + 8;

        canvas.draw_badge(save_x, row1_y, btn_half_w, 32, 0x00064E3B, 0x0010B981, "[ SAUVER (C+S) ]", 0x00D1FAE5, 2);
        self.editor.save_btn_hitbox = Some((save_x, row1_y, btn_half_w, 32));

        canvas.draw_badge(add_x, row1_y, btn_half_w, 32, 0x003730A3, 0x00818CF8, "[ + NOEUD ]", 0x00E0E7FF, 2);
        self.editor.add_node_btn_hitbox = Some((add_x, row1_y, btn_half_w, 32));

        let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001E3A8A };
        let view_btn_y = insp_y + 144;
        let view_btn_txt = if self.editor.is_detailed_view {
            "[ VUE DETAILLEE : CABLAGE (ESPACE) ]"
        } else {
            "[ VUE SIMPLIFIEE : MACRO (ESPACE) ]"
        };
        canvas.draw_badge(insp_x + 18, view_btn_y, btn_w, 32, view_btn_bg, 0x0038BDF8, view_btn_txt, 0x00FFFFFF, 2);
        self.editor.view_btn_hitbox = Some((insp_x + 18, view_btn_y, btn_w, 32));

        let graph = match self.kvl_graphs.get(script_path) {
            Some(g) => g,
            None => {
                canvas.draw_text(insp_x + 18, insp_y + 190, "Script KVL introuvable.", 0x00EF4444, 2);
                return;
            }
        };

        if !self.editor.is_detailed_view {
            canvas.draw_badge(insp_x + 18, insp_y + 186, btn_w, 24, 0x000C4A6E, 0x000284C7, "FLUX D'EXECUTION LOGIQUE (KVL) :", 0x00BAE6FD, 1);

            let mut sorted_nodes: Vec<&Node> = graph.nodes.values().collect();
            sorted_nodes.sort_by_key(|n| match n.kind {
                NodeKind::Event { .. } => 0,
                NodeKind::Condition { .. } => 1,
                NodeKind::Action { ref action_type } if action_type == "AddScore" => 2,
                _ => 3,
            });

            for (idx, node) in sorted_nodes.iter().enumerate() {
                let block_y = insp_y + 218 + (idx as i32 * 54);
                let (box_bg, box_bd, badge) = match node.kind {
                    NodeKind::Event { .. } => (0x00082F49, 0x000284C7, "1. QUAND :"),
                    NodeKind::Condition { .. } => (0x00451A03, 0x00D97706, "2. SI (TEST) :"),
                    NodeKind::Action { ref action_type } if action_type == "BonusPoints" => (0x0078350F, 0x00F59E0B, "BONUS :"),
                    NodeKind::Action { ref action_type } if action_type == "BoostSpeed" => (0x00164E63, 0x0006B6D4, "VITESSE :"),
                    NodeKind::Action { ref action_type } if action_type == "SpawnSparkles" => (0x00831843, 0x00EC4899, "PARTICULES :"),
                    NodeKind::Action { ref action_type } if action_type == "DenyAccess" => (0x007F1D1D, 0x00DC2626, "! REFUS :"),
                    _ if idx == 1 => (0x00064E3B, 0x00059669, "3. ALORS :"),
                    _ => (0x003B0764, 0x009333EA, "EFFET :"),
                };

                canvas.draw_rect(insp_x + 18, block_y, btn_w, 36, box_bg);
                canvas.draw_rect_outline(insp_x + 18, block_y, btn_w, 36, box_bd, 1);

                let label = format!("{} {}", badge, node.name);
                canvas.draw_text(insp_x + 28, block_y + 10, &label, 0x00F8FAFC, 2);

                if idx + 1 < sorted_nodes.len() {
                    canvas.draw_text(insp_x + (btn_w as i32 / 2) - 4, block_y + 38, "V", 0x00FBBF24, 2);
                }
            }
        } else {
            canvas.draw_badge(insp_x + 18, insp_y + 186, btn_w, 24, 0x00064E3B, 0x0010B981, "CABLAGE VISUEL : RELIEZ SORTIE > ENTREE", 0x00D1FAE5, 1);

            let mut sorted_nodes: Vec<&Node> = graph.nodes.values().collect();
            sorted_nodes.sort_by_key(|n| match n.kind {
                NodeKind::Event { .. } => 0,
                NodeKind::Condition { .. } => 1,
                NodeKind::Action { ref action_type } if action_type == "AddScore" => 2,
                _ => 3,
            });

            let mut pin_sockets: HashMap<(String, String), (i32, i32)> = HashMap::new();
            let mut cur_y = insp_y + 212;

            for node in &sorted_nodes {
                let max_pins = node.inputs.len().max(node.outputs.len()).max(1);
                let card_h = 30 + (max_pins as i32 * 32);

                let (header_col, header_tag) = match node.kind {
                    NodeKind::Event { .. } => (0x000284C7, "EVENT"),
                    NodeKind::Condition { .. } => (0x00D97706, "CONDITION"),
                    _ => (0x00059669, "ACTION"),
                };

                canvas.draw_rect(insp_x + 18, cur_y, btn_w, card_h as u32, 0x00131C2E);
                canvas.draw_rect_outline(insp_x + 18, cur_y, btn_w, card_h as u32, header_col, 2);

                canvas.draw_rect(insp_x + 18, cur_y, btn_w, 24, header_col);
                let header_title = format!("[{}] {}", header_tag, node.id);
                canvas.draw_text(insp_x + 26, cur_y + 5, &header_title, 0x00FFFFFF, 2);

                // Inputs (à gauche)
                for (p_idx, pin) in node.inputs.iter().enumerate() {
                    let py = cur_y + 32 + (p_idx as i32 * 30);
                    let sock_x = insp_x + 26;
                    let sock_y = py;
                    let is_connected = graph.is_pin_connected(&node.id, &pin.id);

                    let sock_col = if is_connected { 0x0010B981 } else { 0x00475569 };
                    canvas.draw_rect(sock_x, sock_y, 22, 20, sock_col);
                    canvas.draw_rect_outline(sock_x, sock_y, 22, 20, 0x00FFFFFF, 1);
                    canvas.draw_text(sock_x + 6, sock_y + 3, ">", 0x00FFFFFF, 2);

                    let pin_lbl = format!("{}:In", pin.id);
                    canvas.draw_text(sock_x + 28, sock_y + 3, &pin_lbl, 0x00CBD5E1, 2);

                    let socket_center = (sock_x + 11, sock_y + 10);
                    pin_sockets.insert((node.id.clone(), pin.id.clone()), socket_center);
                    self.editor.pin_hitboxes.push(crate::editor::PinHitbox {
                        node_id: node.id.clone(),
                        pin_id: pin.id.clone(),
                        is_output: false,
                        x: sock_x,
                        y: sock_y,
                        w: 120,
                        h: 22,
                        socket_x: socket_center.0,
                        socket_y: socket_center.1,
                    });
                }

                // Outputs (à droite)
                for (p_idx, pin) in node.outputs.iter().enumerate() {
                    let py = cur_y + 32 + (p_idx as i32 * 30);
                    let sock_x = insp_x + (btn_w as i32) - 32;
                    let sock_y = py;
                    let is_connected = graph.is_pin_connected(&node.id, &pin.id);

                    let sock_col = if pin.id == "out_true" {
                        0x0010B981
                    } else if pin.id == "out_false" {
                        0x00EF4444
                    } else if is_connected {
                        0x0006B6D4
                    } else {
                        0x00FBBF24
                    };
                    canvas.draw_rect(sock_x, sock_y, 22, 20, sock_col);
                    canvas.draw_rect_outline(sock_x, sock_y, 22, 20, 0x00FFFFFF, 1);
                    canvas.draw_text(sock_x + 6, sock_y + 3, ">", 0x00000000, 2);

                    let pin_lbl = if pin.id == "out_true" {
                        "Vrai:Out".to_string()
                    } else if pin.id == "out_false" {
                        "Faux:Out".to_string()
                    } else {
                        format!("{}:Out", pin.id)
                    };
                    let text_w = pin_lbl.len() as i32 * 12;
                    let text_col = if pin.id == "out_true" {
                        0x0034D399
                    } else if pin.id == "out_false" {
                        0x00F87171
                    } else {
                        0x00FDE047
                    };
                    canvas.draw_text(sock_x - text_w - 6, sock_y + 3, &pin_lbl, text_col, 2);

                    let socket_center = (sock_x + 11, sock_y + 10);
                    pin_sockets.insert((node.id.clone(), pin.id.clone()), socket_center);
                    self.editor.pin_hitboxes.push(crate::editor::PinHitbox {
                        node_id: node.id.clone(),
                        pin_id: pin.id.clone(),
                        is_output: true,
                        x: sock_x - text_w - 8,
                        y: sock_y,
                        w: (text_w + 32) as u32,
                        h: 22,
                        socket_x: socket_center.0,
                        socket_y: socket_center.1,
                    });
                }

                cur_y += card_h + 18;
            }

            // Tracé des liaisons et bouton [X]
            for conn in &graph.connections {
                let from_key = (conn.from_node.clone(), conn.from_pin.clone());
                let to_key = (conn.to_node.clone(), conn.to_pin.clone());

                if let (Some(&(fx, fy)), Some(&(tx, ty))) = (pin_sockets.get(&from_key), pin_sockets.get(&to_key)) {
                    canvas.draw_thick_line(fx, fy, tx, ty, 3, 0x0010B981);

                    let cut_x = (fx + tx) / 2 - 10;
                    let cut_y = (fy + ty) / 2 - 10;
                    canvas.draw_rect(cut_x, cut_y, 20, 20, 0x00991B1B);
                    canvas.draw_rect_outline(cut_x, cut_y, 20, 20, 0x00EF4444, 1);
                    canvas.draw_text(cut_x + 5, cut_y + 3, "X", 0x00FFFFFF, 2);

                    self.editor.connection_hitboxes.push(crate::editor::ConnectionHitbox {
                        connection_id: conn.id.clone(),
                        x: cut_x,
                        y: cut_y,
                        w: 20,
                        h: 20,
                    });
                }
            }

            // Câblage actif (Drag & hover)
            if let Some(ref wire) = self.editor.active_wire {
                let mx = self.editor.mouse_x as i32;
                let my = self.editor.mouse_y as i32;

                canvas.draw_thick_line(wire.start_x, wire.start_y, mx, my, 3, 0x00F59E0B);
                canvas.draw_circle(mx, my, 6, 0x00FBBF24);
                canvas.draw_text(mx + 12, my - 10, "RELIER A L'ENTREE", 0x00FDE047, 1);
            }
        }

        // Palette de création
        if self.editor.show_node_palette {
            let pal_x = insp_x + 12;
            let pal_y = insp_y + 80;
            let pal_w = btn_w + 12;
            let pal_h = 570;

            canvas.draw_rect(pal_x, pal_y, pal_w, pal_h, 0x000B0F19);
            canvas.draw_rect_outline(pal_x, pal_y, pal_w, pal_h, 0x00A855F7, 3);

            canvas.draw_rect(pal_x, pal_y, pal_w, 32, 0x006B21A8);
            canvas.draw_text(pal_x + 12, pal_y + 8, "PALETTE KVL : CHOISIR UN NOEUD", 0x00FFFFFF, 2);

            let templates = [
                ("BonusPoints", "+ Bonus Majeur (+50 pts)", 0x00F59E0B),
                ("BoostSpeed", "+ Boost Vitesse (+120 px/s)", 0x0006B6D4),
                ("SlowSpeed", "- Ralentir Vitesse (-80 px/s)", 0x00A855F7),
                ("TeleportPlayer", "@ Teleportation (120, 480)", 0x0038BDF8),
                ("IncrementKeys", "+ Compteur Cles (+1)", 0x0010B981),
                ("ConditionKeys3", "? Condition: Cles >= 3", 0x00FBBF24),
                ("SpawnSparkles", "+ Pluie d'Etincelles Dorees", 0x00EC4899),
                ("ConditionScore50", "? Condition: Si Score >= 50", 0x00F97316),
                ("DenyAccess", "! Action: Alerte Refus", 0x00EF4444),
                ("TriggerAlarm", "! Action: Declencher Alerte", 0x00DC2626),
                ("LockAllDoors", "# Action: Verrouiller Portes", 0x009333EA),
                ("cancel", "[ Fermer la palette ]", 0x0064748B),
            ];

            for (idx, (tpl_id, tpl_name, col)) in templates.iter().enumerate() {
                let item_y = pal_y + 40 + (idx as i32 * 45);
                let item_w = pal_w - 24;
                let item_h = 36;

                canvas.draw_rect(pal_x + 12, item_y, item_w, item_h, 0x001E293B);
                canvas.draw_rect_outline(pal_x + 12, item_y, item_w, item_h, *col, 1);
                canvas.draw_text(pal_x + 20, item_y + 10, tpl_name, *col, 2);

                self.editor.palette_hitboxes.push(crate::editor::PaletteItemHitbox {
                    template_id: tpl_id.to_string(),
                    x: pal_x + 12,
                    y: item_y,
                    w: item_w,
                    h: item_h,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::BrushTile;
    use crate::entity::*;

    fn get_test_scene_path() -> &'static str {
        if std::path::Path::new("assets/scenes/level_01.scene.json").exists() {
            "assets/scenes/level_01.scene.json"
        } else {
            "../../assets/scenes/level_01.scene.json"
        }
    }

    #[test]
    fn test_generic_entity_architecture_and_components() {
        let state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(!state.entities.is_empty(), "La scène doit contenir des entités génériques");
        assert!(state.player_entity().is_some(), "Le joueur doit être identifié par son composant Movement2D");

        let player = state.player_entity().unwrap();
        assert_eq!(player.tag, "player");
        assert!(player.movement.as_ref().unwrap().is_player_controlled);
        assert!(player.collider.as_ref().unwrap().is_solid);

        // Vérifier la présence de murs, portes, collectibles et ennemis sous forme d'entités génériques
        let has_wall = state.entities.iter().any(|e| e.tag == "wall" && e.collider.as_ref().unwrap().is_solid);
        let has_door = state.entities.iter().any(|e| e.tag == "door" && e.script.is_some());
        let has_coin = state.entities.iter().any(|e| e.tag == "collectible" && e.collider.as_ref().unwrap().is_trigger);
        let has_enemy = state.entities.iter().any(|e| e.tag == "enemy" && e.vision.is_some());

        assert!(has_wall, "Les murs doivent être des entités solides");
        assert!(has_door, "Les portes doivent être des entités scriptées");
        assert!(has_coin, "Les pièces doivent être des entités triggers");
        assert!(has_enemy, "Les sentinelles doivent être des entités douées de vision");
    }

    #[test]
    fn test_aabb_wall_blocking_and_sliding() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let wall_ent = state.entities.iter().find(|e| e.tag == "wall").unwrap();
        let wall_x = wall_ent.transform.x;
        let wall_y = wall_ent.transform.y;

        let p_idx = state.player_index().unwrap();
        state.entities[p_idx].transform.x = wall_x - 35.0;
        state.entities[p_idx].transform.y = wall_y + 10.0;

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyD);

        for _ in 0..10 {
            state.update(0.016, &input);
        }

        let p = &state.entities[p_idx];
        assert!(
            p.transform.x <= wall_x,
            "Le joueur ne doit pas franchir le mur solide (AABB sliding)"
        );
    }

    #[test]
    fn test_coin_collection_and_kvl_triggers() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let coin_idx = state.entities.iter().position(|e| e.tag == "collectible").unwrap();
        let coin_x = state.entities[coin_idx].transform.x;
        let coin_y = state.entities[coin_idx].transform.y;

        let p_idx = state.player_index().unwrap();
        state.entities[p_idx].transform.x = coin_x;
        state.entities[p_idx].transform.y = coin_y;

        let empty_input = InputState::new();
        state.update(0.016, &empty_input);

        assert!(!state.entities[coin_idx].is_active, "La pièce doit être désactivée après collecte");
        let score = match state.kvl_context.get_var("score") {
            Some(Value::Int(s)) => *s,
            _ => 0,
        };
        assert!(score > 0, "Le score KVL doit être incrémenté lors de la collecte");
    }

    #[test]
    fn test_door_lock_condition_and_kvl_evaluation() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let door_idx = state.entities.iter().position(|e| e.tag == "door").unwrap();
        let door_x = state.entities[door_idx].transform.x;
        let door_y = state.entities[door_idx].transform.y;

        let p_idx = state.player_index().unwrap();
        state.entities[p_idx].transform.x = door_x - 30.0;
        state.entities[p_idx].transform.y = door_y;

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyE);

        // 1. Score initial = 0 (< 50) -> Refus d'accès
        state.update(0.016, &input);
        assert!(state.entities[door_idx].collider.as_ref().unwrap().is_solid, "La porte doit rester solide quand score < 50");
        assert!(state.status_message.contains("Accès Refusé"));

        // 2. Score >= 50 -> Ouverture
        state.kvl_context.set_var("score", Value::Int(50));
        input.on_key_up(KeyCode::KeyE);
        input.on_key_down(KeyCode::KeyE);
        state.update(0.016, &input);

        assert!(!state.entities[door_idx].collider.as_ref().unwrap().is_solid, "La porte ne doit plus être solide une fois ouverte");
        assert!(state.status_message.contains("déverrouillée et ouverte"));
    }

    #[test]
    fn test_audio_mute_toggle_and_gameplay_sound_triggers() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(!state.audio.is_muted());

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyM);
        state.update(0.016, &input);

        assert!(state.audio.is_muted());
        assert!(state.status_message.contains("MUET"));

        input.on_key_up(KeyCode::KeyM);
        input.on_key_down(KeyCode::KeyM);
        state.update(0.016, &input);

        assert!(!state.audio.is_muted());
        assert!(state.status_message.contains("ACTIF"));
    }

    #[test]
    fn test_enemy_patrol_and_vision_raycast_alert() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let enemy_idx = state.entities.iter().position(|e| e.tag == "enemy").unwrap();
        let initial_y = state.entities[enemy_idx].transform.y;

        let input = InputState::new();
        let p_idx = state.player_index().unwrap();
        state.entities[p_idx].transform.x = 100.0;
        state.entities[p_idx].transform.y = 100.0;

        state.update(0.1, &input);

        assert_ne!(state.entities[enemy_idx].transform.y, initial_y, "L'ennemi doit patrouiller");
        assert!(!state.entities[enemy_idx].vision.as_ref().unwrap().is_alerted);

        // Positionner le joueur dans le cône de vision dégagé
        let e_x = state.entities[enemy_idx].transform.x;
        let e_y = state.entities[enemy_idx].transform.y;
        state.entities[p_idx].transform.x = e_x - 80.0;
        state.entities[p_idx].transform.y = e_y;

        state.update(0.016, &input);

        assert!(state.entities[enemy_idx].vision.as_ref().unwrap().is_alerted);
        assert!(state.status_message.contains("ALERTE"));

        // 3. Test d'occultation par une entité mur solide
        state.entities[enemy_idx].vision.as_mut().unwrap().is_alerted = false;
        state.entities[enemy_idx].vision.as_mut().unwrap().alert_timer = 0.0;
        let shield_wall = Entity::new(
            999,
            "test_shield_wall",
            "wall",
            Transform2D::new(e_x - 40.0, e_y - 50.0, 20.0, 100.0),
        ).with_collider(Collider2D::solid(20.0, 100.0));
        state.entities.push(shield_wall);

        state.update(0.016, &input);
        assert!(!state.entities[enemy_idx].vision.as_ref().unwrap().is_alerted, "Le mur solide doit masquer le joueur (occultation Raycast)");
    }

    #[test]
    fn test_minimap_radar_toggle() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(state.show_minimap);

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyN);
        state.update(0.016, &input);

        assert!(!state.show_minimap);

        input.on_key_up(KeyCode::KeyN);
        input.on_key_down(KeyCode::KeyN);
        state.update(0.016, &input);

        assert!(state.show_minimap);
    }

    #[test]
    fn test_spatial_hash_grid_collision_and_queries() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(!state.spatial_grid.debug_visible);

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyG);
        state.update(0.016, &input);

        assert!(state.spatial_grid.debug_visible);

        input.on_key_up(KeyCode::KeyG);
        input.on_key_down(KeyCode::KeyD);
        state.update(0.016, &input);

        assert!(state.spatial_grid.stats_queries > 0);
    }

    #[test]
    fn test_teleport_action_dispatches_player_movement() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let p_idx = state.player_index().unwrap();
        state.entities[p_idx].transform.x = 100.0;
        state.entities[p_idx].transform.y = 100.0;

        // Déclencher une instruction de téléportation KVL
        state.kvl_context.set_var("teleport_target_x", Value::Int(340));
        state.kvl_context.set_var("teleport_target_y", Value::Int(420));

        let input = InputState::new();
        state.update(0.016, &input);

        let p = &state.entities[p_idx];
        assert_eq!(p.transform.x, 340.0, "Le joueur doit être instantanément téléporté en X");
        assert_eq!(p.transform.y, 420.0, "Le joueur doit être instantanément téléporté en Y");
        assert!(state.status_message.contains("Téléportation quantique"));
        assert_eq!(state.kvl_context.get_var("teleport_target_x"), Some(&Value::Int(-1)), "Les flags de téléportation doivent être consommés");
    }

    #[test]
    fn test_spawn_entity_runtime() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let initial_count = state.entities.len();
        let wall_idx = state.spawn_entity("wall", 450.0, 320.0);
        assert_eq!(state.entities.len(), initial_count + 1);
        assert_eq!(state.entities[wall_idx].tag, "wall");
        assert!(state.entities[wall_idx].collider.as_ref().unwrap().is_solid);

        let coin_idx = state.spawn_entity("collectible", 480.0, 320.0);
        assert_eq!(state.entities.len(), initial_count + 2);
        assert_eq!(state.entities[coin_idx].tag, "collectible");
        assert!(state.entities[coin_idx].script.is_some());

        let portal_idx = state.spawn_entity("portal", 520.0, 320.0);
        assert_eq!(state.entities.len(), initial_count + 3);
        assert_eq!(state.entities[portal_idx].tag, "portal");
        assert_eq!(state.editor.selected, SelectedEntity::Entity(portal_idx));
    }

    #[test]
    fn test_duplicate_selected_entity() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        // Spawner une entité et la dupliquer
        let spawned_idx = state.spawn_entity("wall", 200.0, 200.0);
        state.editor.selected = SelectedEntity::Entity(spawned_idx);

        let dup_idx = state.duplicate_selected_entity().expect("La duplication doit réussir");
        assert_eq!(state.entities[dup_idx].name, format!("{}_copy", state.entities[spawned_idx].name));
        assert_eq!(state.entities[dup_idx].transform.x, 224.0);
        assert_eq!(state.entities[dup_idx].transform.y, 224.0);
        assert_ne!(state.entities[dup_idx].id, state.entities[spawned_idx].id);
        assert_eq!(state.editor.selected, SelectedEntity::Entity(dup_idx));
    }

    #[test]
    fn test_remove_selected_entity_safeguard() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        // Tentative de suppression du joueur -> doit échouer
        let p_idx = state.player_index().unwrap();
        state.editor.selected = SelectedEntity::Entity(p_idx);
        let removed_player = state.remove_selected_entity();
        assert!(!removed_player, "Le joueur principal ne doit jamais être supprimable");
        assert!(state.status_message.contains("refusée"));

        // Suppression d'un mur -> doit réussir
        let wall_idx = state.spawn_entity("wall", 300.0, 300.0);
        state.editor.selected = SelectedEntity::Entity(wall_idx);
        let initial_count = state.entities.len();
        let removed_wall = state.remove_selected_entity();
        assert!(removed_wall, "La suppression de l'entité personnalisée doit réussir");
        assert_eq!(state.entities.len(), initial_count - 1);
        assert_eq!(state.editor.selected, SelectedEntity::None);
    }

    #[test]
    fn test_add_component_to_selected() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        // Créer une entité brute sans mouvement ni vision
        let raw_wall_idx = state.spawn_entity("wall", 100.0, 100.0);
        state.editor.selected = SelectedEntity::Entity(raw_wall_idx);
        assert!(state.entities[raw_wall_idx].movement.is_none());
        assert!(state.entities[raw_wall_idx].vision.is_none());

        // Attacher dynamiquement Movement2D
        let added_mov = state.add_component_to_selected("Movement");
        assert!(added_mov);
        assert!(state.entities[raw_wall_idx].movement.is_some());

        // Attacher dynamiquement VisionSensing
        let added_vis = state.add_component_to_selected("Vision");
        assert!(added_vis);
        assert!(state.entities[raw_wall_idx].vision.is_some());

        // Attacher un composant déjà présent -> doit renvoyer false
        let readded = state.add_component_to_selected("Movement");
        assert!(!readded);
    }

    #[test]
    fn test_particle_lerp_color() {
        let white = 0x00FFFFFF;
        let black = 0x00000000;
        assert_eq!(lerp_color(white, black, 0.0), white);
        assert_eq!(lerp_color(white, black, 1.0), black);

        // Mi-chemin : ~127 (0x7F)
        let mid = lerp_color(white, black, 0.5);
        let r = (mid >> 16) & 0xFF;
        let g = (mid >> 8) & 0xFF;
        let b = mid & 0xFF;
        assert!((r as i32 - 127).abs() <= 1);
        assert!((g as i32 - 127).abs() <= 1);
        assert!((b as i32 - 127).abs() <= 1);
    }

    #[test]
    fn test_particle_emitter_continuous_simulation() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        // Profil High pour permettre jusqu'à 100 particules pour les tests de flux continu
        state.adaptation.current_profile = crate::adaptation::QualityProfile::High;
        state.adaptation.auto_adapt = false;

        // Spawner une torche avec émetteur de flammes
        let torch_idx = state.spawn_entity("torch", 300.0, 300.0);
        assert!(state.entities[torch_idx].emitter.is_some());
        assert_eq!(state.particles.len(), 15); // Particules initiales de spawn

        let input = InputState::new();
        // Simuler 10 frames de 16ms
        for _ in 0..10 {
            state.update(0.016, &input);
        }

        // L'émetteur doit avoir injecté des particules de flammes
        assert!(state.particles.len() > 15, "L'émetteur de particules doit générer un flux continu");
        let flame = state.particles.last().unwrap();
        assert!(flame.life > 0.0);
        assert!(flame.max_life > 0.0);
        assert_eq!(flame.color_start, 0x00FDE047);
        assert_eq!(flame.color_end, 0x00DC2626);
    }

    #[test]
    fn test_particle_emitter_adaptation_cap() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        // Spawner plusieurs torches pour pousser l'émission
        for i in 0..5 {
            state.spawn_entity("torch", 100.0 + (i as f32 * 50.0), 300.0);
        }

        let max_cap = state.adaptation.current_profile.max_particles();
        let input = InputState::new();

        // Simuler 60 frames (1 seconde)
        for _ in 0..60 {
            state.update(0.016, &input);
            assert!(state.particles.len() <= max_cap, "Le budget de particules adaptatif ne doit JAMAIS être dépassé");
        }
    }

    #[test]
    fn test_kvl_emitter_burst_and_toggle() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        state.adaptation.current_profile = crate::adaptation::QualityProfile::High;
        state.adaptation.auto_adapt = false;

        let torch_idx = state.spawn_entity("torch", 250.0, 250.0);
        state.particles.clear();

        // Déclencher une gerbe KVL (EmitBurst)
        state.kvl_context.set_var("emitter_burst", Value::Int(40));
        let input = InputState::new();
        state.update(0.016, &input);

        assert!(state.particles.len() >= 40, "La gerbe KVL doit injecter 40 particules");
        assert_eq!(state.kvl_context.get_var("emitter_burst"), Some(&Value::Int(0)), "Le trigger burst doit être consommé");

        // Désactiver les émetteurs via KVL (ToggleEmitter)
        state.kvl_context.set_var("emitter_active", Value::Bool(false));
        state.update(0.016, &input);
        assert!(!state.entities[torch_idx].emitter.as_ref().unwrap().is_emitting, "L'émetteur doit être mis en pause");
    }

    #[test]
    fn test_interactive_help_modal_toggle_and_dismiss() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(!state.editor.show_help_modal, "L'aide doit être fermée par défaut");

        // 1. Ouvrir avec la touche H
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyH);
        state.update(0.016, &input);
        assert!(state.editor.show_help_modal, "La touche H doit ouvrir le guide pédagogique");

        // 2. Fermer avec Échap
        let mut input2 = InputState::new();
        input2.on_key_down(KeyCode::Escape);
        state.update(0.016, &input2);
        assert!(!state.editor.show_help_modal, "La touche Échap doit fermer le guide pédagogique");

        // 3. Rouvrir avec H et fermer par clic
        let mut input3 = InputState::new();
        input3.on_key_down(KeyCode::KeyH);
        state.update(0.016, &input3);
        assert!(state.editor.show_help_modal);

        let mut input4 = InputState::new();
        input4.on_mouse_down();
        state.update(0.016, &input4);
        assert!(!state.editor.show_help_modal, "Un clic doit fermer la modale d'aide");
    }

    #[test]
    fn test_pedagogical_hud_objective_and_badges() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let mut canvas = Canvas::new(1200, 800);

        // 1. Rendu en mode jeu avec objectif non atteint (< 50)
        state.render(&mut canvas);
        assert!(canvas.buffer.iter().any(|&p| p != 0x000B0F19), "Le canvas doit être rendu avec des éléments d'interface");

        // 2. Passer le score à 60 (objectif atteint) et vérifier le rendu sans régression
        state.kvl_context.set_var("score", Value::Int(60));
        state.render(&mut canvas);

        // 3. Basculer en mode Studio ECS et vérifier l'affichage du guide et des badges
        state.editor.toggle_mode();
        state.render(&mut canvas);
        assert!(state.editor.spawn_btn_hitbox.is_some(), "Le bouton SPAWN doit avoir une hitbox en mode studio");
        assert!(state.editor.top_save_btn_hitbox.is_some(), "Le bouton SAUVER doit avoir une hitbox");
        assert!(state.editor.help_btn_hitbox.is_some(), "Le bouton ? AIDE doit avoir une hitbox");
    }

    #[test]
    fn test_stylized_world_rendering_and_time_increment() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert_eq!(state.elapsed_time, 0.0, "Le temps écoulé doit être initialisé à 0.0");

        let input = InputState::new();
        state.update(0.016, &input);
        assert!((state.elapsed_time - 0.016).abs() < 1e-4, "Le temps écoulé doit s'incrémenter par dt");

        // Rendu complet avec viewport texturé, dallage, ombres et sprites stylisés
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);

        // Vérifier la présence de la couleur de sol texturé cyber-dungeon (0x000D1525)
        let floor_pixels = canvas.buffer.iter().filter(|&&p| p == 0x000D1525).count();
        assert!(floor_pixels > 1000, "Le sol texturé cyber-dungeon doit être rendu dans la zone de jeu");

        // Vérifier la présence d'ombres portées au sol (0x00060911)
        let shadow_pixels = canvas.buffer.iter().filter(|&&p| p == 0x00060911).count();
        assert!(shadow_pixels > 50, "Des ombres portées au sol doivent être rasterisées");
    }

    #[test]
    fn test_platformer_physics_gravity_and_jump() {
        let mut state = GameState::load_from_scene("assets/scenes/platformer_01.scene.json")
            .expect("Chargement du niveau plateforme");

        let p_idx = state.player_index().expect("Joueur plateforme présent");
        assert!(matches!(
            state.entities[p_idx].movement.as_ref().unwrap().mode,
            MovementMode::Platformer { .. }
        ), "Le joueur doit être configuré en mode Platformer");

        let input = InputState::new();

        // 1. Simuler quelques frames pour faire chuter le joueur sur le sol solide
        for _ in 0..30 {
            state.update(0.016, &input);
        }

        // Le joueur doit avoir atterri sur le sol et être grounded
        let p = &state.entities[p_idx];
        if let MovementMode::Platformer { is_grounded, .. } = p.movement.as_ref().unwrap().mode {
            assert!(is_grounded, "Le joueur doit être au sol après la chute initiale");
            assert_eq!(p.movement.as_ref().unwrap().vy, 0.0, "La vitesse verticale doit être nulle au sol");
        } else {
            panic!("Mode Platformer attendu");
        }

        // 2. Déclencher un saut avec la touche Espace
        let mut jump_input = InputState::new();
        jump_input.on_key_down(KeyCode::Space);
        state.update(0.016, &jump_input);

        let p_jumping = &state.entities[p_idx];
        assert!(p_jumping.movement.as_ref().unwrap().vy < 0.0, "Le saut doit donner une impulsion vers le haut (vy < 0)");
    }

    #[test]
    fn test_space_inertia_physics_and_projectile_shooting() {
        let mut state = GameState::load_from_scene("assets/scenes/space_01.scene.json")
            .expect("Chargement du niveau spatial");

        let p_idx = state.player_index().expect("Joueur spatial présent");
        assert!(matches!(
            state.entities[p_idx].movement.as_ref().unwrap().mode,
            MovementMode::SpaceInertia { .. }
        ), "Le joueur doit être configuré en mode SpaceInertia");

        // 1. Rotation vers la droite
        let mut rot_input = InputState::new();
        rot_input.on_key_down(KeyCode::ArrowRight);
        state.update(0.05, &rot_input);
        assert!(state.entities[p_idx].transform.rotation > 0.0, "Le vaisseau doit tourner vers la droite");

        // 2. Poussée vectorielle
        let mut thrust_input = InputState::new();
        thrust_input.on_key_down(KeyCode::ArrowUp);
        state.update(0.05, &thrust_input);
        let mov = state.entities[p_idx].movement.as_ref().unwrap();
        assert!(mov.vx.abs() > 0.0 || mov.vy.abs() > 0.0, "La poussée doit générer une vitesse vectorielle");

        // 3. Tir de projectile
        let initial_entity_count = state.entities.len();
        let fired = state.fire_player_projectile();
        assert!(fired, "Le tir balistique doit réussir");
        assert_eq!(state.entities.len(), initial_entity_count + 1);

        let bullet = state.entities.last().unwrap();
        assert_eq!(bullet.tag, "projectile");
        assert!(bullet.projectile.is_some());
        assert!(bullet.movement.is_some());
    }

    #[test]
    fn test_multi_genre_scene_switching() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json")
            .expect("Chargement de la scène initiale");

        assert!(state.scene_name.contains("Niveau 1") || state.scene_name.contains("Salle"));

        // 1. Bascule vers la scène suivante (Platformer 2D)
        state.cycle_scene_forward();
        assert!(state.scene_path.contains("platformer"));
        assert!(state.scene_name.contains("Plateforme"));

        // 2. Bascule vers la scène suivante (Space Shmup)
        state.cycle_scene_forward();
        assert!(state.scene_path.contains("space"));
        assert!(state.scene_name.contains("Spatial"));

        // 3. Bascule vers la scène suivante (Puzzle Sokoban)
        state.cycle_scene_forward();
        assert!(state.scene_path.contains("puzzle"));
        assert!(state.scene_name.contains("Sanctuaire"));

        // 4. Rebouclage vers la première scène (Top-Down Aventure)
        state.cycle_scene_forward();
        assert!(state.scene_path.contains("level_01"));
    }

    #[test]
    fn test_puzzle_sokoban_pushable_block_and_pressure_plates() {
        let mut state = GameState::load_from_scene("assets/scenes/puzzle_01.scene.json")
            .expect("Chargement du niveau puzzle");

        let p_idx = state.player_index().expect("Joueur puzzle présent");
        assert_eq!(state.entities[p_idx].tag, "player");

        // Vérifier la présence des caisses poussables et des dalles
        let crate_count = state.entities.iter().filter(|e| e.tag == "crate").count();
        let plate_count = state.entities.iter().filter(|e| e.tag == "plate").count();
        let door_idx = state.entities.iter().position(|e| e.tag == "door").expect("Porte de sanctuaire présente");

        assert_eq!(crate_count, 2, "Le niveau puzzle doit contenir 2 caisses");
        assert_eq!(plate_count, 2, "Le niveau puzzle doit contenir 2 dalles de pression");
        assert!(state.entities[door_idx].collider.as_ref().unwrap().is_solid, "La porte doit être fermée initialement");

        // 1. Positionner directement les deux caisses sur leurs dalles respectives
        let plate_positions: Vec<(f32, f32)> = state.entities.iter()
            .filter(|e| e.tag == "plate")
            .map(|e| (e.transform.x, e.transform.y))
            .collect();

        let crate_indices: Vec<usize> = state.entities.iter()
            .enumerate()
            .filter(|(_, e)| e.tag == "crate")
            .map(|(idx, _)| idx)
            .collect();

        for (i, &c_idx) in crate_indices.iter().enumerate() {
            let (px, py) = plate_positions[i];
            state.entities[c_idx].transform.x = px;
            state.entities[c_idx].transform.y = py;
        }

        // 2. Mettre à jour une frame pour déclencher SYSTEM 4.5
        let input = InputState::new();
        state.update(0.016, &input);

        // La porte doit maintenant être déverrouillée (is_solid = false)
        assert!(!state.entities[door_idx].collider.as_ref().unwrap().is_solid, "Les deux caisses sur les dalles doivent ouvrir la porte !");
    }

    #[test]
    fn test_grid_movement_controller() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json")
            .expect("Scène chargée");

        let p_idx = state.player_index().unwrap();
        // Remplacer le mouvement par le mode Grille (40px par case, cooldown 0.1s)
        state.entities[p_idx].movement = Some(Movement2D::grid(40.0, 0.1));

        let initial_x = state.entities[p_idx].transform.x;
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyD);

        state.update(0.016, &input);

        // Le joueur s'est déplacé d'exactement 40 pixels (une case de grille)
        let new_x = state.entities[p_idx].transform.x;
        assert_eq!(new_x, initial_x + 40.0, "Le joueur en mode Grille doit avancer d'une case complète");
    }

    #[test]
    fn test_studio_genre_authoring_tools() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json")
            .expect("Scène chargée");

        // 1. Cycle complet du modèle physique du joueur depuis l'inspecteur
        let p_idx = state.player_index().unwrap();
        state.editor.selected = SelectedEntity::Entity(p_idx);
        let expected = ["PLATEFORME", "INERTIE SPATIALE", "GRILLE", "TOP-DOWN"];
        for label in expected {
            assert!(state.add_component_to_selected("CycleMovementMode"));
            assert_eq!(state.entities[p_idx].movement.unwrap().mode.label(), label);
        }

        // 2. Gabarits puzzle de la palette
        let crate_idx = state.spawn_entity("crate", 300.0, 300.0);
        assert!(state.entities[crate_idx].collider.unwrap().is_pushable);
        let plate_idx = state.spawn_entity("plate", 400.0, 300.0);
        assert!(state.entities[plate_idx].collider.unwrap().is_trigger);

        // 3. Bascule poussable sur un bloc puis refus sur une dalle (trigger)
        state.editor.selected = SelectedEntity::Entity(crate_idx);
        assert!(state.add_component_to_selected("TogglePushable"));
        assert!(!state.entities[crate_idx].collider.unwrap().is_pushable);
        state.editor.selected = SelectedEntity::Entity(plate_idx);
        assert!(!state.add_component_to_selected("TogglePushable"), "Un trigger ne peut pas devenir poussable");

        // 4. Rendu Studio sans panique avec les nouveaux boutons d'inspecteur
        state.editor.selected = SelectedEntity::Entity(p_idx);
        state.editor.toggle_mode();
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "CycleMovementMode"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "TogglePushable"));
    }

    #[test]
    fn test_data_driven_objective_hierarchy() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json")
            .expect("Scène chargée");

        // 1. Scene objective chargé depuis le JSON
        assert!(state.scene_objective.is_some());
        assert_eq!(state.scene_objective.as_deref().unwrap(), "COLLECTEZ 50 OR POUR OUVRIR LA PORTE DU TEMPLE");

        // 2. HUD rendu en jeu
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);

        // 3. Priorité KVL : si une variable "objective" est positionnée, elle prévaut sur scene_objective
        state.kvl_context.set_var("objective", Value::String("OBJECTIF KVL SURMESURE EN COURS".into()));
        state.render(&mut canvas);
        assert_eq!(
            state.kvl_context.get_var("objective"),
            Some(&Value::String("OBJECTIF KVL SURMESURE EN COURS".into()))
        );

        // 4. Fallback sans scene_objective ni KVL var
        state.kvl_context.variables.remove("objective");
        state.scene_objective = None;
        state.render(&mut canvas); // Ne doit pas paniquer et utilise le fallback physique
    }

    #[test]
    fn test_studio_live_physics_tuning() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json")
            .expect("Scène chargée");

        let p_idx = state.player_index().unwrap();
        state.editor.selected = SelectedEntity::Entity(p_idx);

        let initial_speed = state.entities[p_idx].movement.as_ref().unwrap().speed;
        assert!(state.add_component_to_selected("SpeedInc"));
        assert_eq!(state.entities[p_idx].movement.as_ref().unwrap().speed, initial_speed + 25.0);

        assert!(state.add_component_to_selected("SpeedDec"));
        assert_eq!(state.entities[p_idx].movement.as_ref().unwrap().speed, initial_speed);

        // Basculer en mode Platformer
        state.entities[p_idx].movement.as_mut().unwrap().mode = MovementMode::Platformer {
            gravity: 920.0,
            jump_force: 480.0,
            is_grounded: false,
            max_fall_speed: 650.0,
        };

        assert!(state.add_component_to_selected("JumpInc"));
        if let MovementMode::Platformer { jump_force, .. } = state.entities[p_idx].movement.as_ref().unwrap().mode {
            assert_eq!(jump_force, 520.0);
        } else {
            panic!("Mode attendu : Platformer");
        }

        assert!(state.add_component_to_selected("GravityDec"));
        if let MovementMode::Platformer { gravity, .. } = state.entities[p_idx].movement.as_ref().unwrap().mode {
            assert_eq!(gravity, 840.0);
        } else {
            panic!("Mode attendu : Platformer");
        }

        // Basculer en mode SpaceInertia
        state.entities[p_idx].movement.as_mut().unwrap().mode = MovementMode::SpaceInertia {
            angular_speed: 4.0,
            thrust: 520.0,
            drag: 0.985,
        };

        assert!(state.add_component_to_selected("ThrustInc"));
        if let MovementMode::SpaceInertia { thrust, .. } = state.entities[p_idx].movement.as_ref().unwrap().mode {
            assert_eq!(thrust, 570.0);
        } else {
            panic!("Mode attendu : SpaceInertia");
        }

        // Vérification de la présence des boutons d'ajustement dans l'inspecteur
        state.editor.toggle_mode();
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "SpeedInc"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "SpeedDec"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "ThrustInc"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "ThrustDec"));
    }

    #[test]
    fn test_environmental_puzzle_kvl_synchronization() {
        let mut state = GameState::load_from_scene("assets/scenes/puzzle_01.scene.json")
            .expect("Scène chargée");

        // État initial : 0 dalle pressée
        let input = InputState::new();
        state.update(0.016, &input);

        assert_eq!(state.kvl_context.get_var("plates_pressed"), Some(&Value::Int(0)));
        assert_eq!(state.kvl_context.get_var("total_plates"), Some(&Value::Int(2)));
        assert_eq!(state.kvl_context.get_var("puzzle_solved"), Some(&Value::Bool(false)));
        assert_eq!(state.kvl_context.get_var("objective"), Some(&Value::String("OBJECTIF: DALLES ACTIVEES (0/2)".into())));

        // Placer une première caisse sur une dalle
        let plate_pos: Vec<(f32, f32)> = state.entities.iter()
            .filter(|e| e.tag == "plate")
            .map(|e| (e.transform.x, e.transform.y))
            .collect();
        let crate_indices: Vec<usize> = state.entities.iter()
            .enumerate()
            .filter(|(_, e)| e.tag == "crate")
            .map(|(i, _)| i)
            .collect();

        state.entities[crate_indices[0]].transform.x = plate_pos[0].0;
        state.entities[crate_indices[0]].transform.y = plate_pos[0].1;
        state.update(0.016, &input);

        assert_eq!(state.kvl_context.get_var("plates_pressed"), Some(&Value::Int(1)));
        assert_eq!(state.kvl_context.get_var("puzzle_solved"), Some(&Value::Bool(false)));
        assert_eq!(state.kvl_context.get_var("objective"), Some(&Value::String("OBJECTIF: DALLES ACTIVEES (1/2)".into())));

        // Placer la seconde caisse sur la deuxième dalle
        state.entities[crate_indices[1]].transform.x = plate_pos[1].0;
        state.entities[crate_indices[1]].transform.y = plate_pos[1].1;
        state.update(0.016, &input);

        assert_eq!(state.kvl_context.get_var("plates_pressed"), Some(&Value::Int(2)));
        assert_eq!(state.kvl_context.get_var("puzzle_solved"), Some(&Value::Bool(true)));
        assert_eq!(state.kvl_context.get_var("objective"), Some(&Value::String("OBJECTIF: ENIGME RESOLUE ! SORTIE DU SANCTUAIRE".into())));
    }

    #[test]
    fn test_health2d_damage_invulnerability_and_heal() {
        let mut h = crate::entity::Health2D::new(3.0);
        assert_eq!(h.current, 3.0);
        assert_eq!(h.max, 3.0);
        assert!(!h.is_dead);
        assert_eq!(h.invulnerability_timer, 0.0);

        // 1. Dégât initial : -1 PV et activation des i-frames (0.8s)
        assert!(h.damage(1.0));
        assert_eq!(h.current, 2.0);
        assert_eq!(h.invulnerability_timer, 0.8);

        // 2. Dégât consécutif sous i-frames : ignoré
        assert!(!h.damage(1.0));
        assert_eq!(h.current, 2.0);

        // 3. Écoulement du temps
        h.invulnerability_timer = 0.0;
        assert!(h.damage(1.5));
        assert_eq!(h.current, 0.5);

        // 4. Coup fatal
        h.invulnerability_timer = 0.0;
        assert!(h.damage(1.0));
        assert_eq!(h.current, 0.0);
        assert!(h.is_dead);

        // 5. Soin et rétablissement
        h.heal(2.0);
        assert_eq!(h.current, 2.0);
        assert!(!h.is_dead);

        // 6. Plafonnement à la santé max
        h.heal(10.0);
        assert_eq!(h.current, 3.0);
    }

    #[test]
    fn test_player_hazard_damage_and_respawn() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        let input = InputState::new();
        let p_idx = state.player_index().unwrap();

        state.player_spawn_pos = (50.0, 50.0);
        state.entities[p_idx].transform.x = 50.0;
        state.entities[p_idx].transform.y = 50.0;
        state.entities[p_idx].health = Some(crate::entity::Health2D::new(3.0));

        // Placer un piège de piques à (150, 150)
        let trap_idx = state.spawn_entity("hazard", 150.0, 150.0);

        // Joueur marche sur le piège
        state.entities[p_idx].transform.x = state.entities[trap_idx].transform.x;
        state.entities[p_idx].transform.y = state.entities[trap_idx].transform.y;
        state.update(0.016, &input);

        // Vérification des dégâts et de la synchronisation KVL
        let h = state.entities[p_idx].health.as_ref().unwrap();
        assert_eq!(h.current, 2.0);
        assert!(h.invulnerability_timer > 0.0);
        assert_eq!(state.kvl_context.get_var("player_hp"), Some(&Value::Int(2)));

        // Coup fatal infligé par le piège lors de la frame pour tester la boucle de réapparition sûre
        state.entities[p_idx].health.as_mut().unwrap().current = 1.0;
        state.entities[p_idx].health.as_mut().unwrap().invulnerability_timer = 0.0;
        state.entities[p_idx].health.as_mut().unwrap().is_dead = false;
        state.entities[p_idx].transform.x = state.entities[trap_idx].transform.x;
        state.entities[p_idx].transform.y = state.entities[trap_idx].transform.y;
        state.update(0.016, &input);

        // Le joueur a péri et a réapparu au point de départ avec 3 PV et 1.5s d'i-frames
        assert_eq!(state.entities[p_idx].transform.x, 50.0);
        assert_eq!(state.entities[p_idx].transform.y, 50.0);
        let respawned_h = state.entities[p_idx].health.as_ref().unwrap();
        assert_eq!(respawned_h.current, 3.0);
        assert!(!respawned_h.is_dead);
        assert!(respawned_h.invulnerability_timer > 1.0);
    }

    #[test]
    fn test_heart_pickup_healing() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        let input = InputState::new();
        let p_idx = state.player_index().unwrap();

        // Joueur blessé à 1 PV / 3 PV
        state.entities[p_idx].health = Some(crate::entity::Health2D::new(3.0));
        state.entities[p_idx].health.as_mut().unwrap().current = 1.0;
        state.entities[p_idx].transform.x = 80.0;
        state.entities[p_idx].transform.y = 80.0;

        // Fiole de cœur spawnée à (120, 80)
        let heart_idx = state.spawn_entity("heart", 120.0, 80.0);
        assert!(state.entities[heart_idx].is_active);

        // Déplacement du joueur sur la fiole
        state.entities[p_idx].transform.x = 120.0;
        state.entities[p_idx].transform.y = 80.0;
        state.update(0.016, &input);

        // Fiole absorbée et PV passés à 2
        assert!(!state.entities[heart_idx].is_active);
        let h = state.entities[p_idx].health.as_ref().unwrap();
        assert_eq!(h.current, 2.0);
        assert_eq!(state.kvl_context.get_var("player_hp"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_studio_health_authoring_tools() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        let p_idx = state.player_index().unwrap();
        state.editor.selected = SelectedEntity::Entity(p_idx);

        // Mode édition et rendu
        state.editor.toggle_mode();
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);

        // Vérification des boutons de santé dans l'inspecteur
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "DamageSelected"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "HealSelected"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "MaxHpInc"));
        assert!(state.editor.add_comp_hitboxes.iter().any(|(n, ..)| n == "MaxHpDec"));

        // Actionner le bouton de dégât via add_component_to_selected
        assert!(state.add_component_to_selected("DamageSelected"));
        let cur = state.entities[p_idx].health.as_ref().unwrap().current;
        assert_eq!(cur, 2.0);

        // Actionner le bouton de soin
        assert!(state.add_component_to_selected("HealSelected"));
        assert_eq!(state.entities[p_idx].health.as_ref().unwrap().current, 3.0);

        // Augmenter la santé max
        assert!(state.add_component_to_selected("MaxHpInc"));
        assert_eq!(state.entities[p_idx].health.as_ref().unwrap().max, 4.0);

        // Attachement de composant Health2D à un mur neutre
        let wall_idx = state.entities.iter().position(|e| e.tag == "wall").unwrap();
        state.editor.selected = SelectedEntity::Entity(wall_idx);
        state.entities[wall_idx].health = None;

        assert!(state.add_component_to_selected("Health"));
        assert!(state.entities[wall_idx].health.is_some());

        // Palette de spawn : vérification de la présence de hazard et heart
        state.editor.toggle_spawn_palette();
        state.render(&mut canvas);
        assert!(state.editor.spawn_palette_hitboxes.iter().any(|h| h.template_id == "hazard"));
        assert!(state.editor.spawn_palette_hitboxes.iter().any(|h| h.template_id == "heart"));
    }

    #[test]
    fn test_studio_tool_switching_and_tile_cycling() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        state.editor.toggle_mode();
        assert_eq!(state.editor.active_tool, StudioTool::Select);

        // Raccourci B : Pinceau
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyB);
        state.update(0.016, &input);
        assert_eq!(state.editor.active_tool, StudioTool::Brush);

        // Raccourci T : Cycle des tuiles (Mur -> Piège -> Pièce -> Cœur -> Caisse -> Dalle -> Mur)
        assert_eq!(state.editor.brush_tile, BrushTile::Wall);
        assert_eq!(state.editor.brush_tile.id(), "wall");
        assert_eq!(state.editor.brush_tile.short_label(), "Mur");
        assert_eq!(state.editor.brush_tile.color(), 0x0094A3B8);

        let mut input_t = InputState::new();
        input_t.on_key_down(KeyCode::KeyT);
        state.update(0.016, &input_t);
        assert_eq!(state.editor.brush_tile, BrushTile::Hazard);
        assert_eq!(state.editor.brush_tile.id(), "hazard");
        assert_eq!(state.editor.brush_tile.color(), 0x00EF4444);

        state.editor.cycle_brush_tile();
        assert_eq!(state.editor.brush_tile, BrushTile::Collectible);
        state.editor.cycle_brush_tile();
        assert_eq!(state.editor.brush_tile, BrushTile::Heart);
        state.editor.cycle_brush_tile();
        assert_eq!(state.editor.brush_tile, BrushTile::Crate);
        state.editor.cycle_brush_tile();
        assert_eq!(state.editor.brush_tile, BrushTile::Plate);
        state.editor.cycle_brush_tile();
        assert_eq!(state.editor.brush_tile, BrushTile::Wall);

        // Raccourci X : Gomme
        let mut input_x = InputState::new();
        input_x.on_key_down(KeyCode::KeyX);
        state.update(0.016, &input_x);
        assert_eq!(state.editor.active_tool, StudioTool::Eraser);

        // Raccourci V : Select
        let mut input_v = InputState::new();
        input_v.on_key_down(KeyCode::KeyV);
        state.update(0.016, &input_v);
        assert_eq!(state.editor.active_tool, StudioTool::Select);
    }

    #[test]
    fn test_tile_brush_continuous_painting_no_duplicates() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        state.editor.toggle_mode();
        state.editor.set_tool(StudioTool::Brush);
        state.editor.brush_tile = BrushTile::Wall;

        let initial_count = state.entities.len();

        // 1. Clic à l'intérieur du viewport (ex: x=148.0, y=162.0)
        let mut input = InputState::new();
        input.on_mouse_move(148.0, 162.0);
        input.on_mouse_down();
        state.update(0.016, &input);

        // Entité créée
        assert_eq!(state.entities.len(), initial_count + 1);
        let spawned = &state.entities[initial_count];
        assert_eq!(spawned.tag, "wall");

        // 2. Maintien du clic sur la même cellule au frame suivant (anti-doublon)
        input.clear_transient();
        state.update(0.016, &input);
        assert_eq!(state.entities.len(), initial_count + 1, "Ne doit pas créer de doublons sur la même cellule");

        // 3. Glissement vers la cellule voisine (+32px sur x)
        input.on_mouse_move(180.0, 162.0);
        state.update(0.016, &input);
        assert_eq!(state.entities.len(), initial_count + 2, "Doit peindre la cellule voisine");

        // 4. Relâchement de la souris
        input.on_mouse_up();
        state.update(0.016, &input);
        assert_eq!(state.editor.last_painted_cell, None);
    }

    #[test]
    fn test_tile_eraser_tool_and_right_click() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        state.editor.toggle_mode();

        // Créer un mur de test à une coordonnée écran projetée en monde
        let (test_wx, test_wy) = state.camera.screen_to_world(212.0, 194.0);
        let test_wall_idx = state.spawn_entity("wall", test_wx, test_wy);
        let wall_id = state.entities[test_wall_idx].id;
        assert!(state.entities.iter().any(|e| e.id == wall_id));

        // 1. Gomme avec StudioTool::Eraser
        state.editor.set_tool(StudioTool::Eraser);
        let mut input = InputState::new();
        input.on_mouse_move(215.0, 198.0);
        input.on_mouse_down();
        state.update(0.016, &input);

        // Le mur doit avoir été effacé
        assert!(!state.entities.iter().any(|e| e.id == wall_id));

        // 2. Créer une caisse et l'effacer par Clic Droit rapide (en mode Select)
        let (crate_wx, crate_wy) = state.camera.screen_to_world(244.0, 194.0);
        let crate_idx = state.spawn_entity("crate", crate_wx, crate_wy);
        let crate_id = state.entities[crate_idx].id;
        state.editor.set_tool(StudioTool::Select);

        let mut right_click_input = InputState::new();
        right_click_input.on_mouse_move(248.0, 198.0);
        right_click_input.on_mouse_right_click();
        state.update(0.016, &right_click_input);

        // La caisse doit avoir été effacée
        assert!(!state.entities.iter().any(|e| e.id == crate_id));

        // 3. Sécurité : le clic droit sur le joueur ne doit JAMAIS effacer le joueur
        let p_idx = state.player_index().unwrap();
        let px = state.entities[p_idx].transform.x;
        let py = state.entities[p_idx].transform.y;

        let (screen_px, screen_py) = state.camera.world_to_screen(px + 4.0, py + 4.0);
        let mut click_player = InputState::new();
        click_player.on_mouse_move(screen_px as f32, screen_py as f32);
        click_player.on_mouse_right_click();
        state.update(0.016, &click_player);

        assert!(state.player_index().is_some(), "Le joueur doit être protégé contre l'effacement");
    }

    #[test]
    fn test_studio_tool_hitboxes_and_canvas_render() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Scène chargée");
        state.editor.toggle_mode();

        let mut canvas = Canvas::new(1280, 720);
        state.render(&mut canvas);

        // Vérification des hitboxes des outils dans le bandeau supérieur
        assert!(state.editor.tool_select_btn_hitbox.is_some());
        assert!(state.editor.tool_brush_btn_hitbox.is_some());
        assert!(state.editor.tool_tile_cycle_btn_hitbox.is_some());
        assert!(state.editor.tool_eraser_btn_hitbox.is_some());

        // Clic sur l'outil Pinceau via hitbox
        let (bx, by, ..) = state.editor.tool_brush_btn_hitbox.unwrap();
        let mut click_brush = InputState::new();
        click_brush.on_mouse_move(bx as f32 + 5.0, by as f32 + 5.0);
        click_brush.on_mouse_down();
        state.update(0.016, &click_brush);
        assert_eq!(state.editor.active_tool, StudioTool::Brush);

        // Clic sur l'outil Gomme via hitbox
        let (ex, ey, ..) = state.editor.tool_eraser_btn_hitbox.unwrap();
        let mut click_eraser = InputState::new();
        click_eraser.on_mouse_move(ex as f32 + 5.0, ey as f32 + 5.0);
        click_eraser.on_mouse_down();
        state.update(0.016, &click_eraser);
        assert_eq!(state.editor.active_tool, StudioTool::Eraser);

        // Clic sur le cycleur de tuile via hitbox
        let (tx, ty, ..) = state.editor.tool_tile_cycle_btn_hitbox.unwrap();
        let prev_tile = state.editor.brush_tile;
        let mut click_tile = InputState::new();
        click_tile.on_mouse_move(tx as f32 + 5.0, ty as f32 + 5.0);
        click_tile.on_mouse_down();
        state.update(0.016, &click_tile);
        assert_ne!(state.editor.brush_tile, prev_tile);

        // Clic sur Select via hitbox
        let (sx, sy, ..) = state.editor.tool_select_btn_hitbox.unwrap();
        let mut click_select = InputState::new();
        click_select.on_mouse_move(sx as f32 + 5.0, sy as f32 + 5.0);
        click_select.on_mouse_down();
        state.update(0.016, &click_select);
        assert_eq!(state.editor.active_tool, StudioTool::Select);
    }

    #[test]
    fn test_camera_player_tracking_in_play_mode() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");
        let initial_cam_x = state.camera.x;

        // Positionner le joueur plus loin dans le monde
        if let Some(p_idx) = state.player_index() {
            state.entities[p_idx].transform.x = 900.0;
            state.entities[p_idx].transform.y = 500.0;
        }

        let input = InputState::new();
        state.update(0.1, &input);

        // La caméra a dû déplacer sa cible et interpoler vers la nouvelle position
        assert!(state.camera.target_x > initial_cam_x);
        assert!(state.camera.x > initial_cam_x);
        assert_eq!(state.camera.viewport_w, 1240);
    }

    #[test]
    fn test_camera_studio_panning_and_world_tile_painting() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");
        state.editor.mode = EditorMode::Edit;

        let initial_cam_x = state.camera.x;

        // Défilement studio avec Flèche Droite
        let mut input_pan = InputState::new();
        input_pan.on_key_down(KeyCode::ArrowRight);
        state.update(0.1, &input_pan);

        assert!(state.camera.x > initial_cam_x);

        // Recentrage avec la touche C
        let mut input_center = InputState::new();
        input_center.on_key_down(KeyCode::KeyC);
        state.update(0.016, &input_center);
        assert!((state.camera.target_x - state.player_spawn_pos.0).abs() < 50.0);

        // Peinture au pinceau en coordonnées monde défilées
        state.editor.set_tool(StudioTool::Brush);
        let mut click_brush = InputState::new();
        click_brush.on_mouse_move(200.0, 200.0);
        click_brush.on_mouse_down();
        let prev_entity_count = state.entities.len();
        state.update(0.016, &click_brush);
        assert_eq!(state.entities.len(), prev_entity_count + 1);
    }

    #[test]
    fn test_camera_screen_shake_on_player_damage() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");
        assert_eq!(state.camera.shake_intensity, 0.0);

        state.camera.add_shake(12.0);
        assert_eq!(state.camera.shake_intensity, 12.0);

        let input = InputState::new();
        state.update(0.1, &input);
        assert!(state.camera.shake_intensity < 12.0);
        assert!(state.camera.shake_intensity > 0.0);
    }

    #[test]
    fn test_npc_interaction_proximity_and_dialogue_choices() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let p_idx = state.player_index().expect("Joueur présent");
        let px = state.entities[p_idx].transform.x;
        let py = state.entities[p_idx].transform.y;

        // Spawner un PNJ à proximité immédiate (à 30px du joueur)
        let npc = Entity::spawn_npc(
            999,
            "Sage du Donjon",
            px + 30.0,
            py,
            0x00A855F7,
            "Sage",
            "Bienvenue noble aventurier ! Acceptez-vous d'explorer ces ruines ?",
            vec!["Oui, j'accepte !".into(), "Non, j'ai peur.".into()],
        );
        state.entities.push(npc);

        assert!(!state.dialogue.is_active);

        // 1. Appui sur E à proximité -> Ouverture de la boîte de dialogue
        let mut input_e = InputState::new();
        input_e.on_key_down(KeyCode::KeyE);
        state.update(0.016, &input_e);

        assert!(state.dialogue.is_active, "Le dialogue doit s'ouvrir lors de l'interaction avec le PNJ");
        assert_eq!(state.dialogue.speaker, "Sage");
        assert_eq!(state.dialogue.choices.len(), 2);
        assert_eq!(state.kvl_context.get_var("dialogue_active"), Some(&Value::Bool(true)));

        // 2. Le joueur est immobilisé pendant le dialogue
        let mut input_move = InputState::new();
        input_move.on_key_down(KeyCode::KeyD);
        let pos_before = state.entities[p_idx].transform.x;
        state.update(0.016, &input_move);
        let pos_after = state.entities[p_idx].transform.x;
        assert_eq!(pos_before, pos_after, "Le joueur ne doit pas se déplacer pendant un dialogue actif");

        // 3. Accélération machine à écrire puis sélection du Choix 1
        state.dialogue.fast_forward();
        let mut input_choice = InputState::new();
        input_choice.on_key_down(KeyCode::Digit1);
        state.update(0.016, &input_choice);

        assert!(!state.dialogue.is_active, "Le dialogue doit se fermer après sélection du choix");
        assert_eq!(state.kvl_context.get_var("dialogue_selected_choice"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_kvl_quest_synchronization_and_completion() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        // Vérification quête par défaut initialisée
        assert!(state.quest.is_some());
        if let Some(ref q) = state.quest {
            assert_eq!(q.current_step, 0);
            assert!(!q.is_completed);
        }

        // Action KVL CompleteQuest
        state.kvl_context.set_var("quest_completed_main_quest", Value::Bool(true));
        let input = InputState::new();
        state.update(0.016, &input);

        // La quête doit être passée à terminée
        let q = state.quest.as_ref().unwrap();
        assert!(q.is_completed);
    }
}


