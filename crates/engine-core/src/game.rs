use crate::adaptation::AdaptationManager;
use crate::audio::{AudioEngine, SoundType};
use crate::camera::Camera2D;
use crate::dialogue::{DialogueAction, DialogueBox, QuestTracker};
use crate::editor::{EditorMode, EditorState, GameRecipeWizardState, SelectedEntity, StudioCommand, StudioTool, WizardCategory};
use crate::entity::{BossAttackType, BossPhase, Entity, MovementMode, Renderable2D, ShapeKind, TelegraphShape, TelegraphZone};
use crate::input::InputState;
use crate::inventory::{Inventory, Item, ItemUseResult, HOTBAR_SLOTS_COUNT};
use crate::renderer::Canvas;
use crate::replay::{compute_deterministic_state_hash, FrameInputSnapshot, ReplayManager, ReplayMode};
use crate::scene::SceneData;
use crate::spatial::{SpatialHashGrid, SpatialItem, SpatialKind};
use crate::recipe::*;
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
    ("assets/scenes/boss_01.scene.json", "Boss Fight (Telegraphie/KVL)"),
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
    pub status_timer: f32,
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
    pub dialogue_cooldown: f32,
    pub quest: Option<QuestTracker>,
    pub inventory: Inventory,
    pub replay: ReplayManager,
    pub sim_tick: u64,
    pub pathfinder: crate::navigation::AStarPathfinder,
    pub ai_rx: Option<std::sync::mpsc::Receiver<(String, Result<crate::ai_client::AiGeneratedGamePayload, String>)>>,
    pub static_entity_count: usize,
    /// Nombre de pas KVL franchis lors de la dernière frame (renseigné par `update`).
    pub last_kvl_steps: usize,
    /// Ordre de rendu Z-trié réutilisé à chaque frame (zéro allocation).
    pub render_order: Vec<usize>,
    /// Événements de collision de projectiles réutilisés à chaque frame.
    pub hit_events: Vec<(usize, usize, bool)>,
    /// Cooldown anti-empilement des voix `BlockPush` (évite le bruit continu).
    pub block_push_cooldown: f32,
    /// Tampon réutilisé pour les candidats de collision (sans plafond artificiel).
    pub collision_candidates: Vec<SpatialItem>,
}

impl GameState {
    /// Notifie l'utilisateur via la barre d'état (toast temporaire Dark Atelier avec auto-fade après 4s)
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_timer = 4.0;
    }

    /// Joue le son de poussée de bloc avec un cooldown (évite l'empilement des
    /// voix à chaque frame pendant une poussée continue).
    fn play_block_push(&mut self) {
        if self.block_push_cooldown <= 0.0 {
            self.audio.play(SoundType::BlockPush);
            self.block_push_cooldown = 0.15;
        }
    }

    /// Annule la dernière action d'édition dans le Studio (Ctrl+Z)
    pub fn undo(&mut self) -> bool {
        if let Some(cmd) = self.editor.undo_stack.pop_undo() {
            let desc = cmd.description();
            match cmd {
                StudioCommand::SpawnEntity { entity_id, entity_name: _, cell: _ } => {
                    if let Some(idx) = self.entities.iter().position(|e| e.id == entity_id) {
                        let entity = self.entities.remove(idx);
                        self.editor.undo_stack.push_redo(StudioCommand::DeleteEntity { entity, index: idx });
                        self.status_message = format!("Annulé : {}", desc);
                        self.audio.play(SoundType::WireCut);
                        return true;
                    }
                }
                StudioCommand::DeleteEntity { entity, index } => {
                    let ent_id = entity.id;
                    let ent_name = entity.name.clone();
                    let insert_idx = index.min(self.entities.len());
                    self.entities.insert(insert_idx, entity);
                    self.editor.undo_stack.push_redo(StudioCommand::SpawnEntity {
                        entity_id: ent_id,
                        entity_name: ent_name,
                        cell: None,
                    });
                    self.status_message = format!("Annulé : {}", desc);
                    self.audio.play(SoundType::UiClick);
                    return true;
                }
                StudioCommand::MoveEntity { entity_id, old_x, old_y, new_x, new_y } => {
                    if let Some(idx) = self.entities.iter().position(|e| e.id == entity_id) {
                        self.entities[idx].transform.x = old_x;
                        self.entities[idx].transform.y = old_y;
                        self.editor.undo_stack.push_redo(StudioCommand::MoveEntity {
                            entity_id,
                            old_x: new_x,
                            old_y: new_y,
                            new_x: old_x,
                            new_y: old_y,
                        });
                        self.status_message = format!("Annulé : {}", desc);
                        self.audio.play(SoundType::UiClick);
                        return true;
                    }
                }
                StudioCommand::ApplyAiPatch {
                    patch_id,
                    description,
                    created_entity_ids,
                    created_entities,
                    target_graph_id,
                    previous_graph,
                    patched_graph,
                } => {
                    self.entities.retain(|e| !created_entity_ids.contains(&e.id));
                    if let Some(prev) = &previous_graph {
                        self.kvl_graphs.insert(target_graph_id.clone(), prev.clone());
                    }
                    self.editor.undo_stack.push_redo(StudioCommand::ApplyAiPatch {
                        patch_id,
                        description: description.clone(),
                        created_entity_ids,
                        created_entities,
                        target_graph_id,
                        previous_graph,
                        patched_graph,
                    });
                    self.set_status(&format!("↶ Annulé : Patch IA '{}'", description));
                    self.audio.play(SoundType::UiClick);
                    return true;
                }
            }
        }
        false
    }

    /// Rétablit la dernière action d'édition annulée dans le Studio (Ctrl+Y / Ctrl+Shift+Z)
    pub fn redo(&mut self) -> bool {
        if let Some(cmd) = self.editor.undo_stack.pop_redo() {
            let desc = cmd.description();
            match cmd {
                StudioCommand::SpawnEntity { entity_id, entity_name: _, cell: _ } => {
                    if let Some(idx) = self.entities.iter().position(|e| e.id == entity_id) {
                        let entity = self.entities.remove(idx);
                        self.editor.undo_stack.push_undo(StudioCommand::DeleteEntity { entity, index: idx });
                        self.status_message = format!("Rétabli : {}", desc);
                        self.audio.play(SoundType::WireCut);
                        return true;
                    }
                }
                StudioCommand::DeleteEntity { entity, index } => {
                    let ent_id = entity.id;
                    let ent_name = entity.name.clone();
                    let insert_idx = index.min(self.entities.len());
                    self.entities.insert(insert_idx, entity);
                    self.editor.undo_stack.push_undo(StudioCommand::SpawnEntity {
                        entity_id: ent_id,
                        entity_name: ent_name,
                        cell: None,
                    });
                    self.status_message = format!("Rétabli : {}", desc);
                    self.audio.play(SoundType::UiClick);
                    return true;
                }
                StudioCommand::MoveEntity { entity_id, old_x, old_y, new_x, new_y } => {
                    if let Some(idx) = self.entities.iter().position(|e| e.id == entity_id) {
                        self.entities[idx].transform.x = old_x;
                        self.entities[idx].transform.y = old_y;
                        self.editor.undo_stack.push_undo(StudioCommand::MoveEntity {
                            entity_id,
                            old_x: new_x,
                            old_y: new_y,
                            new_x: old_x,
                            new_y: old_y,
                        });
                        self.status_message = format!("Rétabli : {}", desc);
                        self.audio.play(SoundType::UiClick);
                        return true;
                    }
                }
                StudioCommand::ApplyAiPatch {
                    patch_id,
                    description,
                    created_entity_ids,
                    created_entities,
                    target_graph_id,
                    previous_graph,
                    patched_graph,
                } => {
                    for ent in &created_entities {
                        if !self.entities.iter().any(|e| e.id == ent.id) {
                            self.entities.push(ent.clone());
                        }
                    }
                    self.kvl_graphs.insert(target_graph_id.clone(), patched_graph.clone());
                    self.editor.undo_stack.push_undo(StudioCommand::ApplyAiPatch {
                        patch_id,
                        description: description.clone(),
                        created_entity_ids,
                        created_entities,
                        target_graph_id,
                        previous_graph,
                        patched_graph,
                    });
                    self.set_status(&format!("↷ Rétabli : Patch IA '{}'", description));
                    self.audio.play(SoundType::UiClick);
                    return true;
                }
            }
        }
        false
    }

    /// Calcule le hash d'état déterministe FNV-1a de la simulation
    pub fn state_hash(&self) -> u64 {
        let (px, py, php) = if let Some(p_idx) = self.player_index() {
            let p = &self.entities[p_idx];
            let hp = p.health.as_ref().map_or(3.0, |h| h.current);
            (p.transform.x, p.transform.y, hp)
        } else {
            (0.0, 0.0, 0.0)
        };
        let score = self.kvl_context.get_var("score").and_then(|v| match v {
            Value::Int(i) => Some(*i),
            _ => None,
        }).unwrap_or(0);
        let boss_hp = self.entities.iter().find(|e| e.tag == "boss").and_then(|b| b.health.as_ref().map(|h| h.current));
        compute_deterministic_state_hash(self.sim_tick, px, py, php, score, boss_hp)
    }
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

    /// Utilise ou consomme l'objet d'un slot de la barre d'action rapide (Hotbar)
    pub fn use_hotbar_item(&mut self, slot_idx: usize) -> ItemUseResult {
        let p_idx_opt = self.player_index();
        let res = if let Some(p_idx) = p_idx_opt {
            let mut hp = self.entities[p_idx].health;
            let r = self.inventory.use_slot(slot_idx, hp.as_mut(), &mut self.audio);
            if let Some(new_h) = hp {
                self.entities[p_idx].health = Some(new_h);
                self.kvl_context.set_var("player_hp", Value::Int(new_h.current as i64));
            }
            r
        } else {
            self.inventory.use_slot(slot_idx, None, &mut self.audio)
        };

        if let ItemUseResult::KeyActivated { ref item_id } = res {
            self.kvl_context.set_var(format!("used_key_{}", item_id), Value::Bool(true));
            for graph in self.kvl_graphs.values() {
                let _ = Interpreter::trigger_event(graph, &mut self.kvl_context, "OnItemUse");
            }
        }

        res
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
            "item_potion" => Entity::spawn_item_pickup(id, x, y, Item::health_potion(1), 1),
            "item_key" => Entity::spawn_item_pickup(id, x, y, Item::dungeon_key(format!("key_{}", id), "Clef d'Or"), 1),
            "item_crystal" => Entity::spawn_item_pickup(id, x, y, Item::energy_crystal(format!("crystal_{}", id), "Cristal d'Energie"), 1),
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
            "boss" => Entity::spawn_boss(id, format!("Titan_{}", id), x, y, 80.0, 60.0, "assets/scripts/boss_titan.kvl.json"),
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

        let dup_name = dup.name.clone();
        self.entities.push(dup);
        let new_idx = self.entities.len() - 1;
        self.editor.undo_stack.push(StudioCommand::SpawnEntity {
            entity_id: new_id,
            entity_name: dup_name,
            cell: None,
        });
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

        let ent_deleted = self.entities[selected_idx].clone();
        let name = ent_deleted.name.clone();
        let x = ent_deleted.transform.x;
        let y = ent_deleted.transform.y;
        self.editor.undo_stack.push(StudioCommand::DeleteEntity {
            entity: ent_deleted,
            index: selected_idx,
        });
        self.entities.remove(selected_idx);
        self.rebuild_static_spatial_grid();
        self.editor.selected = SelectedEntity::None;
        self.audio.play(SoundType::WireCut);
        self.spawn_particles(x, y, 16, 0x00EF4444);
        self.status_message = format!("Entité '{}' supprimée de la scène.", name);
        true
    }

    /// Ajoute dynamiquement un composant manquant à l'entité sélectionnée
    pub fn add_component_to_selected(&mut self, comp_type: &str) -> bool {
        if comp_type == "OpenGuidedWizard" {
            self.editor.wizard.open();
            self.audio.play(SoundType::UiClick);
            self.status_message = "Assistant Guidé KVL ouvert (Niveau 1) !".into();
            return true;
        }

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
            "Navigation" => {
                if ent.nav_agent.is_none() {
                    ent.nav_agent = Some(crate::navigation::NavAgent::chase_player(90.0));
                    self.status_message = format!("Composant NavAgent (A* Intelligent) attaché à '{}'.", ent.name);
                } else {
                    return false;
                }
            }
            "Flock" => {
                if ent.flock.is_none() {
                    ent.flock = Some(crate::flocking::FlockMember::drone(1));
                    if ent.movement.is_none() {
                        ent.movement = Some(crate::entity::Movement2D {
                            vx: 0.0,
                            vy: 0.0,
                            speed: 160.0,
                            is_player_controlled: false,
                            patrol: None,
                            mode: crate::entity::MovementMode::TopDown,
                        });
                    }
                    self.status_message = format!("Composant FlockMember (Essaim / Meute Boid) attaché à '{}'.", ent.name);
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

        let mut inventory = Inventory::new(16);
        inventory.add_item(Item::health_potion(1), 2);
        inventory.add_item(Item::dungeon_key("key_bronze", "Clef de Bronze"), 1);

        kvl_context.set_var("item_count_potion_heal", Value::Int(2));
        kvl_context.set_var("has_potion_heal", Value::Bool(true));
        kvl_context.set_var("item_count_key_bronze", Value::Int(1));
        kvl_context.set_var("has_key_bronze", Value::Bool(true));
        kvl_context.set_var("hotbar_selected_slot", Value::Int(0));

        // Spawner des objets ramassables dans le monde si la scène en est dépourvue
        let has_pickup = entities.iter().any(|e| e.tag == "item_pickup");
        if !has_pickup && scene_data.name.contains("Donjon") {
            let next_id = entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
            entities.push(Entity::spawn_item_pickup(next_id, 460.0, 160.0, Item::health_potion(1), 1));
            entities.push(Entity::spawn_item_pickup(next_id + 1, 280.0, 320.0, Item::energy_crystal("crystal_01", "Cristal Bleu"), 1));
        }

        let mut state = Self {
            scene_path: path.to_string(),
            scene_name: scene_data.name,
            scene_objective,
            entities,
            particles: Vec::new(),
            kvl_graphs,
            kvl_context,
            status_message: "Moteur KaractiRage initialisé ! Modèle ECS universel multi-genre actif.".into(),
            status_timer: 4.0,
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
            dialogue_cooldown: 0.0,
            quest,
            inventory,
            replay: ReplayManager::new(),
            sim_tick: 0,
            pathfinder: crate::navigation::AStarPathfinder::default(),
            ai_rx: None,
            static_entity_count: 0,
            last_kvl_steps: 0,
            render_order: Vec::new(),
            hit_events: Vec::new(),
            block_push_cooldown: 0.0,
            collision_candidates: Vec::new(),
        };
        state.rebuild_static_spatial_grid();
        Ok(state)
    }

    /// Reconstruit les seaux statiques persistants du partitionnement spatial (murs, obstacles fixes, plateformes)
    pub fn rebuild_static_spatial_grid(&mut self) {
        self.spatial_grid.clear_static();
        self.static_entity_count = self.entities.len();
        for (idx, ent) in self.entities.iter().enumerate() {
            if !ent.is_active {
                continue;
            }
            if let Some(ref col) = ent.collider {
                // Les éléments sans mouvement sont statiques (sauf blocs poussables Sokoban)
                if ent.movement.is_none() && (col.is_solid || col.is_one_way) && !col.is_pushable {
                    let kind = if col.is_one_way {
                        SpatialKind::OneWay
                    } else if col.is_solid {
                        SpatialKind::Solid
                    } else if col.is_trigger {
                        SpatialKind::Trigger
                    } else {
                        SpatialKind::Entity(idx)
                    };
                    self.spatial_grid.insert_static(SpatialItem {
                        id: idx,
                        kind,
                        x: ent.transform.x + col.offset_x,
                        y: ent.transform.y + col.offset_y,
                        w: col.w,
                        h: col.h,
                    });
                }
            }
        }
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
        let prev_steps = self.editor.guide_step_done;
        let prev_tab = self.editor.guide_tab;
        if let Ok(mut new_state) = Self::load_from_scene(path) {
            new_state.editor.guide_step_done = prev_steps;
            new_state.editor.guide_step_done[2] = true; // Étape 3 (Genre) accomplie !
            new_state.editor.guide_tab = prev_tab;
            *self = new_state;
            self.audio.play(SoundType::Teleport);
            true
        } else {
            false
        }
    }

    /// Charge un projet KaractiProject complet dans le moteur de jeu
    pub fn load_project(&mut self, project: crate::project::KaractiProject) {
        self.scene_name = project.metadata.title;
        self.scene_objective = project.scene.objective;
        self.entities = project.scene.entities;
        self.kvl_graphs = project.kvl_graphs;
        self.kvl_context = ExecutionContext::new();

        for (k, v) in project.kvl_variables {
            self.kvl_context.set_var(k, v);
        }
        if self.kvl_context.get_var("score").is_none() {
            self.kvl_context.set_var("score", Value::Int(0));
        }

        self.rebuild_static_spatial_grid();
        self.editor.selected = crate::editor::SelectedEntity::None;
        self.editor.mode = match project.settings.default_mode.as_str() {
            "edit" => EditorMode::Edit,
            _ => EditorMode::Play,
        };

        if let Some(p_idx) = self.player_index() {
            let px = self.entities[p_idx].transform.center_x();
            let py = self.entities[p_idx].transform.center_y();
            self.camera.set_target(px, py);
            self.camera.snap_to_target();
        }

        self.set_status(format!("Projet '{}' instancié ({} entités)", self.scene_name, self.entities.len()));
    }

    /// Initialise un nouveau projet vierge dans le moteur (Studio prêt à créer)
    pub fn new_blank_project(&mut self, title: &str, genre: crate::recipe::GameGenre) {
        let proj = crate::project::KaractiProject::new_empty(title, "Créateur", genre);
        self.load_project(proj);
    }

    /// Exporte l'état actuel complet du jeu dans une structure KaractiProject
    pub fn export_current_project(&self) -> crate::project::KaractiProject {
        let genre = match self.scene_name.to_lowercase().as_str() {
            s if s.contains("platformer") => crate::recipe::GameGenre::Platformer,
            s if s.contains("space") || s.contains("shmup") => crate::recipe::GameGenre::ShootEmUp,
            s if s.contains("puzzle") || s.contains("sokoban") => crate::recipe::GameGenre::Puzzle,
            s if s.contains("boss") || s.contains("titan") => crate::recipe::GameGenre::Combat,
            _ => crate::recipe::GameGenre::Adventure,
        };

        let metadata = crate::project::ProjectMetadata {
            id: format!("proj_{:x}", (self.scene_name.len() as u64) ^ 0xCAFEBABE),
            title: self.scene_name.clone(),
            author: "Créateur KaractiRage".to_string(),
            version: "1.0.0".to_string(),
            created_at: "2026-10-17T00:00:00Z".to_string(),
            description: "Projet sauvegardé depuis l'Atelier Studio KaractiRage.".to_string(),
            genre_primary: genre,
            genres_secondary: Vec::new(),
        };

        let settings = crate::project::ProjectSettings {
            target_fps: 60,
            software_width: 1200,
            software_height: 800,
            gravity: 980.0,
            performance_profile: "LowEnd".to_string(),
            show_minimap: self.show_minimap,
            default_mode: if self.editor.mode == EditorMode::Edit { "edit".to_string() } else { "play".to_string() },
        };

        let scene = crate::scene::SceneData {
            name: self.scene_name.clone(),
            description: self.scene_name.clone(),
            objective: self.scene_objective.clone(),
            entities: self.entities.clone(),
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        };

        let mut kvl_vars = HashMap::new();
        if let Some(s) = self.kvl_context.get_var("score") {
            kvl_vars.insert("score".to_string(), s.clone());
        }

        crate::project::KaractiProject {
            schema_version: crate::project::PROJECT_SCHEMA_VERSION,
            metadata,
            settings,
            recipe: None,
            scene,
            kvl_graphs: self.kvl_graphs.clone(),
            kvl_variables: kvl_vars,
        }
    }

    /// Exporte le résumé JSON de toutes les entités actives en mémoire
    pub fn get_entities_summary_json(&self) -> String {
        #[derive(serde::Serialize)]
        struct EntitySummary {
            id: usize,
            name: String,
            tag: String,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            health: Option<f32>,
            max_health: Option<f32>,
            speed: Option<f32>,
            movement_mode: Option<String>,
            is_solid: bool,
            is_trigger: bool,
            is_pushable: bool,
            is_active: bool,
        }

        let summaries: Vec<EntitySummary> = self.entities.iter().map(|e| {
            EntitySummary {
                id: e.id,
                name: e.name.clone(),
                tag: e.tag.clone(),
                x: e.transform.x,
                y: e.transform.y,
                w: e.transform.w,
                h: e.transform.h,
                health: e.health.as_ref().map(|h| h.current),
                max_health: e.health.as_ref().map(|h| h.max),
                speed: e.movement.as_ref().map(|m| m.speed),
                movement_mode: e.movement.as_ref().map(|m| m.mode.label().to_string()),
                is_solid: e.collider.as_ref().map_or(false, |c| c.is_solid),
                is_trigger: e.collider.as_ref().map_or(false, |c| c.is_trigger),
                is_pushable: e.collider.as_ref().map_or(false, |c| c.is_pushable),
                is_active: e.is_active,
            }
        }).collect();

        serde_json::to_string(&summaries).unwrap_or_else(|_| "[]".to_string())
    }

    /// Met à jour la position et dimension d'une entité par son ID
    pub fn update_entity_transform_by_id(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32) -> bool {
        if let Some(ent) = self.entities.iter_mut().find(|e| e.id == id) {
            ent.transform.x = x;
            ent.transform.y = y;
            ent.transform.w = w;
            ent.transform.h = h;
            if let Some(ref mut col) = ent.collider {
                col.w = w;
                col.h = h;
            }
            self.rebuild_static_spatial_grid();
            return true;
        }
        false
    }

    /// Supprime définitivement une entité par son ID
    pub fn delete_entity_by_id(&mut self, id: usize) -> bool {
        if let Some(pos) = self.entities.iter().position(|e| e.id == id) {
            self.entities.remove(pos);
            self.rebuild_static_spatial_grid();
            return true;
        }
        false
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

fn key_to_char(key: KeyCode, shift: bool, layout: crate::editor::KeyboardLayout) -> Option<char> {
    match layout {
        crate::editor::KeyboardLayout::FrenchAzerty => match key {
            KeyCode::KeyA => Some(if shift { 'Q' } else { 'q' }),
            KeyCode::KeyB => Some(if shift { 'B' } else { 'b' }),
            KeyCode::KeyC => Some(if shift { 'C' } else { 'c' }),
            KeyCode::KeyD => Some(if shift { 'D' } else { 'd' }),
            KeyCode::KeyE => Some(if shift { 'E' } else { 'e' }),
            KeyCode::KeyF => Some(if shift { 'F' } else { 'f' }),
            KeyCode::KeyG => Some(if shift { 'G' } else { 'g' }),
            KeyCode::KeyH => Some(if shift { 'H' } else { 'h' }),
            KeyCode::KeyI => Some(if shift { 'I' } else { 'i' }),
            KeyCode::KeyJ => Some(if shift { 'J' } else { 'j' }),
            KeyCode::KeyK => Some(if shift { 'K' } else { 'k' }),
            KeyCode::KeyL => Some(if shift { 'L' } else { 'l' }),
            KeyCode::KeyM => Some(if shift { '?' } else { ',' }),
            KeyCode::KeyN => Some(if shift { 'N' } else { 'n' }),
            KeyCode::KeyO => Some(if shift { 'O' } else { 'o' }),
            KeyCode::KeyP => Some(if shift { 'P' } else { 'p' }),
            KeyCode::KeyQ => Some(if shift { 'A' } else { 'a' }),
            KeyCode::KeyR => Some(if shift { 'R' } else { 'r' }),
            KeyCode::KeyS => Some(if shift { 'S' } else { 's' }),
            KeyCode::KeyT => Some(if shift { 'T' } else { 't' }),
            KeyCode::KeyU => Some(if shift { 'U' } else { 'u' }),
            KeyCode::KeyV => Some(if shift { 'V' } else { 'v' }),
            KeyCode::KeyW => Some(if shift { 'Z' } else { 'z' }),
            KeyCode::KeyX => Some(if shift { 'X' } else { 'x' }),
            KeyCode::KeyY => Some(if shift { 'Y' } else { 'y' }),
            KeyCode::KeyZ => Some(if shift { 'W' } else { 'w' }),
            KeyCode::Semicolon => Some(if shift { 'M' } else { 'm' }),
            KeyCode::Comma => Some(if shift { '?' } else { ',' }),
            KeyCode::Period => Some(if shift { '.' } else { ';' }),
            KeyCode::Slash => Some(if shift { '/' } else { ':' }),
            KeyCode::Quote => Some(if shift { '%' } else { 'ù' }),
            KeyCode::Backslash => Some(if shift { '*' } else { '!' }),
            KeyCode::Minus => Some(if shift { '°' } else { ')' }),
            KeyCode::Equal => Some(if shift { '+' } else { '=' }),
            KeyCode::Space => Some(' '),
            KeyCode::Digit1 => Some(if shift { '1' } else { '&' }),
            KeyCode::Digit2 => Some(if shift { '2' } else { 'é' }),
            KeyCode::Digit3 => Some(if shift { '3' } else { '"' }),
            KeyCode::Digit4 => Some(if shift { '4' } else { '\'' }),
            KeyCode::Digit5 => Some(if shift { '5' } else { '(' }),
            KeyCode::Digit6 => Some(if shift { '6' } else { '-' }),
            KeyCode::Digit7 => Some(if shift { '7' } else { 'è' }),
            KeyCode::Digit8 => Some(if shift { '8' } else { '_' }),
            KeyCode::Digit9 => Some(if shift { '9' } else { 'ç' }),
            KeyCode::Digit0 => Some(if shift { '0' } else { 'à' }),
            _ => None,
        },
        crate::editor::KeyboardLayout::UsQwerty => match key {
            KeyCode::KeyA => Some(if shift { 'A' } else { 'a' }),
            KeyCode::KeyB => Some(if shift { 'B' } else { 'b' }),
            KeyCode::KeyC => Some(if shift { 'C' } else { 'c' }),
            KeyCode::KeyD => Some(if shift { 'D' } else { 'd' }),
            KeyCode::KeyE => Some(if shift { 'E' } else { 'e' }),
            KeyCode::KeyF => Some(if shift { 'F' } else { 'f' }),
            KeyCode::KeyG => Some(if shift { 'G' } else { 'g' }),
            KeyCode::KeyH => Some(if shift { 'H' } else { 'h' }),
            KeyCode::KeyI => Some(if shift { 'I' } else { 'i' }),
            KeyCode::KeyJ => Some(if shift { 'J' } else { 'j' }),
            KeyCode::KeyK => Some(if shift { 'K' } else { 'k' }),
            KeyCode::KeyL => Some(if shift { 'L' } else { 'l' }),
            KeyCode::KeyM => Some(if shift { 'M' } else { 'm' }),
            KeyCode::KeyN => Some(if shift { 'N' } else { 'n' }),
            KeyCode::KeyO => Some(if shift { 'O' } else { 'o' }),
            KeyCode::KeyP => Some(if shift { 'P' } else { 'p' }),
            KeyCode::KeyQ => Some(if shift { 'Q' } else { 'q' }),
            KeyCode::KeyR => Some(if shift { 'R' } else { 'r' }),
            KeyCode::KeyS => Some(if shift { 'S' } else { 's' }),
            KeyCode::KeyT => Some(if shift { 'T' } else { 't' }),
            KeyCode::KeyU => Some(if shift { 'U' } else { 'u' }),
            KeyCode::KeyV => Some(if shift { 'V' } else { 'v' }),
            KeyCode::KeyW => Some(if shift { 'W' } else { 'w' }),
            KeyCode::KeyX => Some(if shift { 'X' } else { 'x' }),
            KeyCode::KeyY => Some(if shift { 'Y' } else { 'y' }),
            KeyCode::KeyZ => Some(if shift { 'Z' } else { 'z' }),
            KeyCode::Space => Some(' '),
            KeyCode::Digit0 => Some(if shift { ')' } else { '0' }),
            KeyCode::Digit1 => Some(if shift { '!' } else { '1' }),
            KeyCode::Digit2 => Some(if shift { '@' } else { '2' }),
            KeyCode::Digit3 => Some(if shift { '#' } else { '3' }),
            KeyCode::Digit4 => Some(if shift { '$' } else { '4' }),
            KeyCode::Digit5 => Some(if shift { '%' } else { '5' }),
            KeyCode::Digit6 => Some(if shift { '^' } else { '6' }),
            KeyCode::Digit7 => Some(if shift { '&' } else { '7' }),
            KeyCode::Digit8 => Some(if shift { '*' } else { '8' }),
            KeyCode::Digit9 => Some(if shift { '(' } else { '9' }),
            KeyCode::Minus => Some(if shift { '_' } else { '-' }),
            KeyCode::Equal => Some(if shift { '+' } else { '=' }),
            KeyCode::Comma => Some(if shift { '<' } else { ',' }),
            KeyCode::Period => Some(if shift { '>' } else { '.' }),
            KeyCode::Slash => Some(if shift { '?' } else { '/' }),
            KeyCode::Quote => Some(if shift { '"' } else { '\'' }),
            _ => None,
        },
    }
}

    pub fn update(&mut self, dt: f32, input: &InputState) {
        self.elapsed_time += dt;
        self.sim_tick += 1;
        if self.block_push_cooldown > 0.0 {
            self.block_push_cooldown -= dt;
        }

        // Disponibilité universelle des coordonnées souris pour le survol (hover) et les infobulles
        self.editor.mouse_x = input.mouse_x;
        self.editor.mouse_y = input.mouse_y;

        // Auto-fade du message de statut vers le repos sobre après 4 secondes
        if self.status_timer > 0.0 {
            self.status_timer -= dt;
            if self.status_timer <= 0.0 {
                self.status_message = "KaractiRage Studio • Prêt [Appuyez sur H pour le guide]".to_string();
            }
        }

        // Réception asynchrone non-bloquante des réponses du modèle IA (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI)
        if let Some(ref rx) = self.ai_rx {
            match rx.try_recv() {
                Ok((prompt, Ok(payload))) => {
                    self.editor.ai_generation_in_progress = false;
                    self.apply_ai_payload(payload, &prompt);
                    self.ai_rx = None;
                }
                Ok((_prompt, Err(err))) => {
                    self.editor.ai_generation_in_progress = false;
                    self.set_status(format!("⚠️ {}", err));
                    self.ai_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Toujours en cours d'exécution dans le thread de travail
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.editor.ai_generation_in_progress = false;
                    self.set_status("⚠️ Connexion au modèle IA interrompue.");
                    self.ai_rx = None;
                }
            }
        }

        // Suivi en temps réel des 6 étapes du parcours d'initiation
        if input.is_key_down(KeyCode::KeyW) || input.is_key_down(KeyCode::KeyA) || input.is_key_down(KeyCode::KeyS) || input.is_key_down(KeyCode::KeyD)
            || input.is_key_down(KeyCode::ArrowUp) || input.is_key_down(KeyCode::ArrowDown) || input.is_key_down(KeyCode::ArrowLeft) || input.is_key_down(KeyCode::ArrowRight) {
            self.editor.guide_step_done[0] = true;
        }
        let score_val = self.kvl_context.get_var("score").map_or(0, |v| match v { Value::Int(n) => *n, _ => 0 });
        if self.inventory.slots.iter().any(|i| i.is_some()) || score_val > 0 {
            self.editor.guide_step_done[1] = true;
        }
        if self.editor.mode == EditorMode::Edit {
            self.editor.guide_step_done[3] = true;
        }
        if self.editor.selected != SelectedEntity::None {
            self.editor.guide_step_done[4] = true;
        }
        if self.editor.wizard.is_open {
            self.editor.guide_step_done[5] = true;
        }

        // Gestion de la Review Bar de patch IA (Phase C : Boucle de Confiance)
        // (zéro allocation : on teste la présence du patch et on capture les
        //  données nécessaires avant d'éventuels emprunts mutables)
        if self.editor.pending_ai_patch.is_some() {
            let accept = input.is_key_just_pressed(KeyCode::Enter)
                || (input.mouse_just_clicked
                    && self.editor.is_ai_patch_accept_clicked(input.mouse_x, input.mouse_y));
            let modify = input.is_key_just_pressed(KeyCode::Space)
                || (input.mouse_just_clicked
                    && self.editor.is_ai_patch_modify_clicked(input.mouse_x, input.mouse_y));
            let reject = input.is_key_just_pressed(KeyCode::Escape)
                || (input.mouse_just_clicked
                    && self.editor.is_ai_patch_reject_clicked(input.mouse_x, input.mouse_y));

            if accept {
                self.accept_ai_patch();
                return;
            }
            if modify {
                let first_id = self
                    .editor
                    .pending_ai_patch
                    .as_ref()
                    .and_then(|p| p.created_entities.first())
                    .map(|e| e.id);
                self.editor.mode = EditorMode::Edit;
                if let Some(id) = first_id {
                    self.editor.selected = SelectedEntity::Entity(id);
                }
                self.set_status("Mode Studio activé : Ajustez les composants ou les nœuds KVL avant de valider.");
                self.audio.play(SoundType::UiClick);
                return;
            }
            if reject {
                self.reject_ai_patch();
                return;
            }
        }

        // Gestion exclusive du Hub de Projets & Accueil KaractiRage (Section 1 Spécification)
        if self.editor.show_welcome_modal {
            if input.is_key_just_pressed(KeyCode::Escape) {
                self.editor.show_welcome_modal = false;
                self.audio.play(SoundType::UiClick);
                return;
            }
            if input.is_key_just_pressed(KeyCode::KeyI) {
                self.editor.welcome_tab = 2;
                self.audio.play(SoundType::UiClick);
                self.set_status("Intention IA & Modèle LLM ouvert au démarrage.");
                return;
            }
            if input.is_key_just_pressed(KeyCode::KeyW) {
                self.editor.show_welcome_modal = false;
                self.editor.recipe_wizard.open();
                self.audio.play(SoundType::UiClick);
                self.set_status("Assistant de Recette de Jeu ouvert : Concevez votre jeu en 6 étapes !");
                return;
            }
            if input.is_key_just_pressed(KeyCode::KeyH) {
                self.editor.show_welcome_modal = false;
                self.editor.show_help_modal = true;
                self.editor.guide_tab = 0;
                self.audio.play(SoundType::UiClick);
                self.set_status("Guide Pédagogique ouvert.");
                return;
            }

            // Saisie au clavier dans le Hub lorsque l'onglet Intention IA (Tab 2) est sélectionné
            if self.editor.welcome_tab == 2 {
                if input.is_key_just_pressed(KeyCode::Backspace) {
                    self.editor.intent_input_text.pop();
                }
                if input.is_key_just_pressed(KeyCode::Enter) {
                    let prompt = self.editor.intent_input_text.trim().to_string();
                    if !prompt.is_empty() {
                        self.editor.show_welcome_modal = false;
                        self.trigger_ai_intent(&prompt);
                        return;
                    }
                }
                if !input.received_chars.is_empty() {
                    for &ch in &input.received_chars {
                        if self.editor.intent_input_text.chars().count() < 400 {
                            self.editor.intent_input_text.push(ch);
                        }
                    }
                } else {
                    let shift = input.is_key_pressed(KeyCode::ShiftLeft) || input.is_key_pressed(KeyCode::ShiftRight);
                    for &k in &input.just_pressed_keys {
                        if let Some(ch) = Self::key_to_char(k, shift, self.editor.keyboard_layout) {
                            if self.editor.intent_input_text.chars().count() < 400 {
                                self.editor.intent_input_text.push(ch);
                            }
                        }
                    }
                }
            }

            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                // 1. Changement d'onglet du Hub (0: Nouveau Projet 1.1, 1: Charger Projet 1.2, 2: Intention IA 1.3)
                if let Some(tab_idx) = self.editor.clicked_welcome_tab(mx, my) {
                    self.editor.welcome_tab = tab_idx;
                    self.audio.play(SoundType::UiClick);
                    return;
                }

                // Clic sur le bouton de création IA rapide depuis la bannière de l'onglet 0 ou la barre d'action inférieure
                if self.editor.is_welcome_hero_ai_clicked(mx, my) || self.editor.is_welcome_bottom_ai_clicked(mx, my) {
                    self.editor.welcome_tab = 2;
                    self.audio.play(SoundType::UiClick);
                    self.set_status("Intention IA & Configuration LLM ouverte (Onglet 1.3).");
                    return;
                }

                // Clics spécifiques à l'onglet Intention IA & LLM (Tab 2)
                if self.editor.welcome_tab == 2 {
                    if self.editor.is_welcome_ai_test_clicked(mx, my) {
                        let res = crate::ai_client::AiClient::test_connection(&self.editor.ai_config);
                        self.editor.ai_test_result = match res {
                            Ok(msg) => Some((true, msg)),
                            Err(err) => Some((false, err)),
                        };
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if let Some(provider) = self.editor.clicked_welcome_ai_provider(mx, my) {
                        self.editor.ai_config.provider = provider;
                        self.editor.ai_test_result = None;
                        self.audio.play(SoundType::UiClick);
                        self.set_status(format!("Modèle IA sélectionné : {}", provider.label()));
                        return;
                    }

                    if self.editor.is_keyboard_layout_clicked(mx, my) {
                        self.editor.keyboard_layout = match self.editor.keyboard_layout {
                            crate::editor::KeyboardLayout::FrenchAzerty => crate::editor::KeyboardLayout::UsQwerty,
                            crate::editor::KeyboardLayout::UsQwerty => crate::editor::KeyboardLayout::FrenchAzerty,
                        };
                        self.audio.play(SoundType::UiClick);
                        self.set_status(format!("Disposition clavier : {}", self.editor.keyboard_layout.label()));
                        return;
                    }

                    if self.editor.is_welcome_ai_generate_clicked(mx, my) {
                        let prompt = self.editor.intent_input_text.trim().to_string();
                        if !prompt.is_empty() {
                            self.editor.show_welcome_modal = false;
                            self.trigger_ai_intent(&prompt);
                            return;
                        }
                    }

                    if let Some(p_idx) = self.editor.clicked_welcome_ai_preset(mx, my) {
                        let prompt = match p_idx {
                            0 => "Donjon sombre avec héros ninja, gouffres de lave ardents, clé dorée et boss titan démon",
                            1 => "Forteresse glacée avec chevalier errant, slimes de givre, dalles de saut et coffre scellé",
                            2 => "Laboratoire cybernétique avec drones patrouilleurs A*, portails quantiques et cristaux d'énergie",
                            3 => "Forêt mystique avec golem de pierre, énigme de blocs Sokoban et potions de vie",
                            _ => "Niveau d'aventure complet avec boucle de gameplay",
                        };
                        self.editor.intent_input_text = prompt.to_string();
                        self.editor.show_welcome_modal = false;
                        self.trigger_ai_intent(prompt);
                        return;
                    }
                }

                // 2. Clic sur une Scène Récente du Disque (Charger & Jouer OU Ouvrir en Studio)
                if let Some((scene_idx, is_studio)) = self.editor.clicked_welcome_scene(mx, my) {
                    if scene_idx < AVAILABLE_SCENES.len() {
                        let (scene_path, scene_genre) = AVAILABLE_SCENES[scene_idx];
                        self.switch_scene(scene_path);
                        self.editor.mode = if is_studio { EditorMode::Edit } else { EditorMode::Play };
                        self.editor.show_welcome_modal = false;
                        self.audio.play(SoundType::Teleport);
                        self.set_status(&format!("Projet chargé : {} (Mode {})", scene_genre, if is_studio { "Studio ECS" } else { "Jeu 60 FPS" }));
                        return;
                    }
                }

                // 3. Clic sur un Archétype Rapide 1-Clic (Génération & Remplacement Scène)
                if let Some(arch_key) = self.editor.clicked_welcome_archetype(mx, my) {
                    self.editor.recipe_wizard.apply_archetype(arch_key);
                    self.apply_active_recipe();
                    self.editor.show_welcome_modal = false;
                    self.editor.mode = EditorMode::Play;
                    self.audio.play(SoundType::BonusFanfare);
                    self.set_status(&format!("Archétype instancié : {} ! Bon jeu.", arch_key));
                    return;
                }

                // 4. Boutons d'action généraux
                if self.editor.is_welcome_recipe_clicked(mx, my) {
                    self.editor.show_welcome_modal = false;
                    self.editor.recipe_wizard.open();
                    self.audio.play(SoundType::UiClick);
                    self.set_status("Assistant de Recette de Jeu ouvert : Concevez votre jeu en 6 étapes !");
                    return;
                }
                if self.editor.is_welcome_play_clicked(mx, my) {
                    self.editor.mode = EditorMode::Play;
                    self.editor.show_welcome_modal = false;
                    self.audio.play(SoundType::UiClick);
                    self.set_status("Bienvenue dans KaractiRage ! Mode Jeu actif. Utilisez ZQSD pour vous déplacer.");
                    return;
                }
                if self.editor.is_welcome_studio_clicked(mx, my) {
                    self.editor.mode = EditorMode::Edit;
                    self.editor.show_welcome_modal = false;
                    self.audio.play(SoundType::UiClick);
                    self.set_status("Bienvenue dans l'Atelier Studio ! Cliquez sur les entités pour concevoir.");
                    return;
                }
                if self.editor.is_welcome_guide_clicked(mx, my) {
                    self.editor.show_welcome_modal = false;
                    self.editor.show_help_modal = true;
                    self.editor.guide_tab = 0;
                    self.audio.play(SoundType::UiClick);
                    self.set_status("Parcours d'initiation ouvert. Suivez les 6 étapes guidées !");
                    return;
                }
                if self.editor.is_welcome_close_clicked(mx, my) {
                    self.editor.show_welcome_modal = false;
                    self.audio.play(SoundType::UiClick);
                    return;
                }
            }
        }

        // Gestion exclusive de la Fenêtre Modale d'Exportation Standalone
        if self.editor.show_export_modal {
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
                self.editor.show_export_modal = false;
                self.audio.play(SoundType::UiClick);
                self.status_message = "Exportation annulée.".into();
                return;
            }

            if input.is_key_just_pressed(KeyCode::Enter) {
                self.perform_export();
                return;
            }

            if input.is_key_just_pressed(KeyCode::Tab)
                || input.is_key_just_pressed(KeyCode::ArrowRight)
                || input.is_key_just_pressed(KeyCode::ArrowDown)
            {
                self.editor.export_format_idx = (self.editor.export_format_idx + 1) % 3;
                self.audio.play(SoundType::UiClick);
                return;
            }

            if input.is_key_just_pressed(KeyCode::ArrowLeft) || input.is_key_just_pressed(KeyCode::ArrowUp) {
                self.editor.export_format_idx = if self.editor.export_format_idx == 0 { 2 } else { self.editor.export_format_idx - 1 };
                self.audio.play(SoundType::UiClick);
                return;
            }

            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                if self.editor.is_export_close_clicked(mx, my) {
                    self.editor.show_export_modal = false;
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Exportation fermée.".into();
                    return;
                }

                if let Some(fmt_idx) = self.editor.clicked_export_format(mx, my) {
                    self.editor.export_format_idx = fmt_idx;
                    self.audio.play(SoundType::UiClick);
                    return;
                }

                if self.editor.is_export_action_clicked(mx, my) {
                    self.perform_export();
                    return;
                }
            }

            // Ne pas propager les inputs au reste du jeu tant que la modale d'export est ouverte
            return;
        }

        // Gestion exclusive de la Fenêtre Modale de Recette de Jeu en 6 Étapes (Desktop Native Recipe Wizard)
        if self.editor.recipe_wizard.is_open {
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
                self.editor.recipe_wizard.close();
                self.audio.play(SoundType::UiClick);
                self.status_message = "Assistant Recette de Jeu fermé.".into();
                return;
            }

            if input.is_key_just_pressed(KeyCode::KeyW) {
                self.editor.recipe_wizard.close();
                self.audio.play(SoundType::UiClick);
                self.status_message = "Assistant Recette de Jeu fermé.".into();
                return;
            }

            if input.is_key_just_pressed(KeyCode::Enter) {
                if self.editor.recipe_wizard.step == 6 {
                    self.apply_active_recipe();
                } else {
                    self.editor.recipe_wizard.next_step();
                    self.audio.play(SoundType::UiClick);
                }
                return;
            }

            if input.is_key_just_pressed(KeyCode::Tab) || input.is_key_just_pressed(KeyCode::ArrowRight) {
                self.editor.recipe_wizard.next_step();
                self.audio.play(SoundType::UiClick);
                return;
            }

            if input.is_key_just_pressed(KeyCode::ArrowLeft) {
                self.editor.recipe_wizard.prev_step();
                self.audio.play(SoundType::UiClick);
                return;
            }

            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                // Clic sur fermeture
                if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.close_btn_hitbox, mx, my) {
                    self.editor.recipe_wizard.close();
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Assistant Recette de Jeu fermé.".into();
                    return;
                }

                // Clic sur un onglet d'étape 1..=6
                if let Some(step) = self.editor.recipe_wizard.clicked_step_tab(mx, my) {
                    self.editor.recipe_wizard.step = step;
                    self.editor.recipe_wizard.run_audit();
                    self.audio.play(SoundType::UiClick);
                    return;
                }

                // Clic sur bouton Précédent
                if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.prev_btn_hitbox, mx, my) {
                    self.editor.recipe_wizard.prev_step();
                    self.audio.play(SoundType::UiClick);
                    return;
                }

                // Clic sur bouton Suivant
                if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.next_btn_hitbox, mx, my) {
                    self.editor.recipe_wizard.next_step();
                    self.audio.play(SoundType::UiClick);
                    return;
                }

                // Clic sur bouton Exporter Recette .json
                if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.export_recipe_btn_hitbox, mx, my) {
                    let _ = std::fs::create_dir_all("export");
                    if let Ok(json) = self.editor.recipe_wizard.recipe.to_json() {
                        let _ = std::fs::write("export/game.recipe.json", &json);
                        self.set_status("💾 Recette sauvegardée dans 'export/game.recipe.json' !");
                    } else {
                        self.set_status("💾 Recette sérialisée avec succès !");
                    }
                    self.audio.play(SoundType::QuestSuccess);
                    return;
                }

                // Clic sur bouton Ultime : INSTANCIER & JOUER
                if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.apply_btn_hitbox, mx, my) {
                    self.apply_active_recipe();
                    return;
                }

                // Interactions spécifiques à l'Étape 1 : Archetypes & Genres
                if self.editor.recipe_wizard.step == 1 {
                    if let Some(arch_key) = self.editor.recipe_wizard.clicked_archetype(mx, my) {
                        self.editor.recipe_wizard.apply_archetype(arch_key);
                        self.audio.play(SoundType::QuestSuccess);
                        self.set_status(&format!("✨ Archétype '{}' appliqué avec succès !", arch_key));
                        return;
                    }

                    if let Some(genre) = self.editor.recipe_wizard.clicked_genre(mx, my) {
                        self.editor.recipe_wizard.recipe.genres.primary = genre;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }
                }

                // Interactions spécifiques à l'Étape 2 : Dimension & Caméra
                if self.editor.recipe_wizard.step == 2 {
                    if let Some(dim) = self.editor.recipe_wizard.clicked_dimension(mx, my) {
                        self.editor.recipe_wizard.recipe.dimension = dim;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if let Some(cam) = self.editor.recipe_wizard.clicked_camera(mx, my) {
                        self.editor.recipe_wizard.recipe.camera = cam;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }
                }

                // Interactions spécifiques à l'Étape 3 : Déplacement & Capacités
                if self.editor.recipe_wizard.step == 3 {
                    if let Some(mvt) = self.editor.recipe_wizard.clicked_movement(mx, my) {
                        self.editor.recipe_wizard.recipe.player.directions = mvt;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.ability_jump_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.player.abilities.jump = !self.editor.recipe_wizard.recipe.player.abilities.jump;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.ability_dash_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.player.abilities.dash = !self.editor.recipe_wizard.recipe.player.abilities.dash;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.ability_shoot_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.player.combat = if self.editor.recipe_wizard.recipe.player.combat == CombatStyle::Projectiles {
                            CombatStyle::None
                        } else {
                            CombatStyle::Projectiles
                        };
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.ability_melee_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.player.combat = if self.editor.recipe_wizard.recipe.player.combat == CombatStyle::Melee {
                            CombatStyle::None
                        } else {
                            CombatStyle::Melee
                        };
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.ability_interact_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.player.abilities.interact = !self.editor.recipe_wizard.recipe.player.abilities.interact;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }
                }

                // Interactions spécifiques à l'Étape 4 : Monde & Éléments
                if self.editor.recipe_wizard.step == 4 {
                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.world_gravity_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.world.gravity_enabled = !self.editor.recipe_wizard.recipe.world.gravity_enabled;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    let toggle_elem = |elements: &mut Vec<InteractiveElement>, elem: InteractiveElement| {
                        if let Some(pos) = elements.iter().position(|&x| x == elem) {
                            elements.remove(pos);
                        } else {
                            elements.push(elem);
                        }
                    };

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.elem_collectible_hitbox, mx, my) {
                        toggle_elem(&mut self.editor.recipe_wizard.recipe.world.interactive_elements, InteractiveElement::Collectible);
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.elem_door_hitbox, mx, my) {
                        toggle_elem(&mut self.editor.recipe_wizard.recipe.world.interactive_elements, InteractiveElement::Door);
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.elem_chest_hitbox, mx, my) {
                        toggle_elem(&mut self.editor.recipe_wizard.recipe.world.interactive_elements, InteractiveElement::Chest);
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.elem_lever_hitbox, mx, my) {
                        toggle_elem(&mut self.editor.recipe_wizard.recipe.world.interactive_elements, InteractiveElement::Lever);
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.elem_trap_hitbox, mx, my) {
                        toggle_elem(&mut self.editor.recipe_wizard.recipe.world.interactive_elements, InteractiveElement::Trap);
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }
                }

                // Interactions spécifiques à l'Étape 5 : Gameplay & Objectifs
                if self.editor.recipe_wizard.step == 5 {
                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.feat_inventory_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.gameplay.inventory = !self.editor.recipe_wizard.recipe.gameplay.inventory;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.feat_dialogues_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.gameplay.dialogues = !self.editor.recipe_wizard.recipe.gameplay.dialogues;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.feat_quests_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.gameplay.quest_objectives = !self.editor.recipe_wizard.recipe.gameplay.quest_objectives;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.feat_boss_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.gameplay.boss_fight = !self.editor.recipe_wizard.recipe.gameplay.boss_fight;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.feat_score_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.gameplay.score_system = !self.editor.recipe_wizard.recipe.gameplay.score_system;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if GameRecipeWizardState::is_hitbox_clicked(self.editor.recipe_wizard.feat_waves_hitbox, mx, my) {
                        self.editor.recipe_wizard.recipe.gameplay.wave_spawner = !self.editor.recipe_wizard.recipe.gameplay.wave_spawner;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }

                    if let Some(perf) = self.editor.recipe_wizard.clicked_perf(mx, my) {
                        self.editor.recipe_wizard.recipe.performance = perf;
                        self.editor.recipe_wizard.run_audit();
                        self.audio.play(SoundType::UiClick);
                        return;
                    }
                }
            }

            // Ne pas propager les inputs au reste du jeu tant que la modale de recette est ouverte
            return;
        }

        // Raccourci I en Mode Studio pour ouvrir l'Assistant IA (uniquement si la modale n'est pas déjà ouverte)
        if self.editor.mode == EditorMode::Edit && !self.editor.show_intent_bar && input.is_key_just_pressed(KeyCode::KeyI) {
            self.editor.show_intent_bar = true;
            self.editor.ai_test_result = None;
            self.audio.play(SoundType::UiClick);
            return;
        }

        if self.editor.show_intent_bar {
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
                self.editor.show_intent_bar = false;
                self.audio.play(SoundType::UiClick);
                return;
            }

            // Saisie au clavier dans le champ de prompt libre
            if input.is_key_just_pressed(KeyCode::Backspace) {
                self.editor.intent_input_text.pop();
            }

            if input.is_key_just_pressed(KeyCode::Enter) {
                let prompt = self.editor.intent_input_text.trim().to_string();
                if !prompt.is_empty() {
                    self.trigger_ai_intent(&prompt);
                    return;
                }
            }

            if !input.received_chars.is_empty() {
                for &ch in &input.received_chars {
                    if self.editor.intent_input_text.chars().count() < 400 {
                        self.editor.intent_input_text.push(ch);
                    }
                }
            } else {
                let shift = input.is_key_pressed(KeyCode::ShiftLeft) || input.is_key_pressed(KeyCode::ShiftRight);
                for &k in &input.just_pressed_keys {
                    if let Some(ch) = Self::key_to_char(k, shift, self.editor.keyboard_layout) {
                        if self.editor.intent_input_text.chars().count() < 400 {
                            self.editor.intent_input_text.push(ch);
                        }
                    }
                }
            }

            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                if self.editor.is_ai_test_clicked(mx, my) {
                    let res = crate::ai_client::AiClient::test_connection(&self.editor.ai_config);
                    self.editor.ai_test_result = match res {
                        Ok(msg) => Some((true, msg)),
                        Err(err) => Some((false, err)),
                    };
                    self.audio.play(SoundType::UiClick);
                    return;
                }

                if let Some(provider) = self.editor.clicked_ai_provider(mx, my) {
                    self.editor.ai_config.provider = provider;
                    self.editor.ai_test_result = None;
                    self.audio.play(SoundType::UiClick);
                    self.set_status(format!("Modèle IA sélectionné : {}", provider.label()));
                    return;
                }

                if self.editor.is_keyboard_layout_clicked(mx, my) {
                    self.editor.keyboard_layout = match self.editor.keyboard_layout {
                        crate::editor::KeyboardLayout::FrenchAzerty => crate::editor::KeyboardLayout::UsQwerty,
                        crate::editor::KeyboardLayout::UsQwerty => crate::editor::KeyboardLayout::FrenchAzerty,
                    };
                    self.audio.play(SoundType::UiClick);
                    self.set_status(format!("Disposition clavier : {}", self.editor.keyboard_layout.label()));
                    return;
                }

                if self.editor.is_intent_close_clicked(mx, my) {
                    self.editor.show_intent_bar = false;
                    self.audio.play(SoundType::UiClick);
                    return;
                }
                if self.editor.is_intent_submit_clicked(mx, my) {
                    let prompt = self.editor.intent_input_text.trim().to_string();
                    if !prompt.is_empty() {
                        self.trigger_ai_intent(&prompt);
                        return;
                    }
                }
                if let Some(p_idx) = self.editor.clicked_intent_preset(mx, my) {
                    let prompt = match p_idx {
                        0 => "Garde patrouilleur hostile A* avec alerte sonore, vision conique et poursuite",
                        1 => "Coffre au trésor scellé nécessitant la clé dorée du donjon et conférant 150 points",
                        2 => "Zone de lave ardente infligeant 35 dégâts au contact avec particules et piège sismique",
                        3 => "Portail dimensionnel quantique téléportant le joueur avec distorsion arcanique et audio",
                        _ => "Niveau d'aventure complet avec boucle de gameplay",
                    };
                    self.editor.intent_input_text = prompt.to_string();
                    self.trigger_ai_intent(prompt);
                    return;
                }
            }
            return;
        }

        // Snapshot de la position de la frame précédente pour l'interpolation alpha
        for ent in &mut self.entities {
            ent.transform.snapshot_prev();
        }

        // Mise à jour de la hiérarchie de scène parent-enfant
        for i in 0..self.entities.len() {
            if let Some(pid) = self.entities[i].transform.parent_id {
                let parent_pos = self.entities.iter().find(|e| e.id == pid && e.is_active).map(|p| (p.transform.x, p.transform.y));
                if let Some((px, py)) = parent_pos {
                    self.entities[i].transform.x = px + self.entities[i].transform.local_offset_x;
                    self.entities[i].transform.y = py + self.entities[i].transform.local_offset_y;
                }
            }
        }

        // Raccourci F3 : Bascule Enregistrement / Lecture Replay Déterministe
        if input.is_key_just_pressed(KeyCode::F3) {
            match self.replay.mode {
                ReplayMode::Idle => {
                    let hash = self.state_hash();
                    self.replay.start_recording(&self.scene_path, hash);
                    self.audio.play(SoundType::RadarPing);
                    self.status_message = "Replay : Enregistrement démarré [● REC] (F3 pour stopper)".into();
                }
                ReplayMode::Recording => {
                    let hash = self.state_hash();
                    let score = self.kvl_context.get_var("score").and_then(|v| match v {
                        Value::Int(i) => Some(*i),
                        _ => None,
                    }).unwrap_or(0);
                    self.replay.stop_recording(hash, score);
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Replay : Enregistrement terminé ! (F3 pour rejouer)".into();
                }
                ReplayMode::Playing => {
                    self.replay.stop_playback();
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Replay : Relecture arrêtée.".into();
                }
            }
        }

        // Raccourci F5 : Bascule Fantôme (Ghost Player)
        if input.is_key_just_pressed(KeyCode::F5) {
            let active = self.replay.toggle_ghost();
            self.status_message = if active {
                "Fantôme Replay : ACTIVÉ (F5 pour masquer)".into()
            } else {
                "Fantôme Replay : MASQUÉ".into()
            };
        }

        // Enregistrement des entrées de la frame pour le replay déterministe
        if self.replay.mode == ReplayMode::Recording {
            let mut snapshot = FrameInputSnapshot::default();
            snapshot.set_key(FrameInputSnapshot::KEY_LEFT, input.is_key_down(KeyCode::ArrowLeft) || input.is_key_down(KeyCode::KeyA) || input.is_key_down(KeyCode::KeyQ));
            snapshot.set_key(FrameInputSnapshot::KEY_RIGHT, input.is_key_down(KeyCode::ArrowRight) || input.is_key_down(KeyCode::KeyD));
            snapshot.set_key(FrameInputSnapshot::KEY_UP, input.is_key_down(KeyCode::ArrowUp) || input.is_key_down(KeyCode::KeyW) || input.is_key_down(KeyCode::KeyZ));
            snapshot.set_key(FrameInputSnapshot::KEY_DOWN, input.is_key_down(KeyCode::ArrowDown) || input.is_key_down(KeyCode::KeyS));
            snapshot.set_key(FrameInputSnapshot::KEY_JUMP, input.is_key_down(KeyCode::Space));
            snapshot.set_key(FrameInputSnapshot::KEY_FIRE, input.is_key_down(KeyCode::KeyF));
            snapshot.set_key(FrameInputSnapshot::KEY_INTERACT, input.is_key_down(KeyCode::KeyE));
            snapshot.mouse_x = input.mouse_x as i16;
            snapshot.mouse_y = input.mouse_y as i16;
            snapshot.mouse_clicked = input.mouse_pressed;

            let p_pos = self.player_index().map(|idx| (self.entities[idx].transform.x, self.entities[idx].transform.y)).unwrap_or((0.0, 0.0));
            self.replay.record_tick(snapshot, p_pos);
        }

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

        // Bascule Assistant Guidé KVL (Touche G en mode Studio) ou Grille Spatiale (en mode Jeu)
        if input.is_key_just_pressed(KeyCode::KeyG) {
            if self.editor.mode == EditorMode::Edit {
                self.editor.wizard.is_open = !self.editor.wizard.is_open;
                if self.editor.wizard.is_open {
                    self.editor.wizard.step = 1;
                }
                self.audio.play(SoundType::UiClick);
                self.status_message = if self.editor.wizard.is_open {
                    "Assistant Guidé KVL (Niveau 1) : OUVERT. Répondez aux questions pour générer une mécanique.".into()
                } else {
                    "Assistant Guidé KVL fermé.".into()
                };
                return;
            } else {
                let vis = self.spatial_grid.toggle_debug();
                self.status_message = if vis {
                    "Grille Spatiale (Spatial Hash Grid) : AFFICHÉE".into()
                } else {
                    "Grille Spatiale (Spatial Hash Grid) : MASQUÉE".into()
                };
            }
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

        // Bascule Modale Exportation Standalone (Touche U ou Ctrl+E)
        if input.is_key_just_pressed(KeyCode::KeyU) || (is_ctrl_or_cmd && input.is_key_just_pressed(KeyCode::KeyE)) {
            self.editor.show_export_modal = !self.editor.show_export_modal;
            self.audio.play(SoundType::UiClick);
            self.status_message = if self.editor.show_export_modal {
                "Exportation Standalone : Sélectionnez un format puis appuyez sur Entrée pour exporter.".into()
            } else {
                "Exportation Standalone fermée.".into()
            };
            return;
        }

        // Bascule Modale Recette de Jeu en 6 Étapes (Touche W)
        if input.is_key_just_pressed(KeyCode::KeyW) {
            if self.editor.recipe_wizard.is_open {
                self.editor.recipe_wizard.close();
                self.status_message = "Assistant Recette de Jeu fermé.".into();
            } else {
                self.editor.recipe_wizard.open();
                self.status_message = "Assistant Recette de Jeu ouvert : Concevez votre jeu en 6 étapes !".into();
            }
            self.audio.play(SoundType::UiClick);
            return;
        }

        // Bascule Console KVL escamotable (~ ou Backquote)
        if input.is_key_just_pressed(KeyCode::Backquote) {
            self.editor.show_console_drawer = !self.editor.show_console_drawer;
            self.audio.play(SoundType::UiClick);
            self.status_message = if self.editor.show_console_drawer {
                "Console KVL étendue (~ pour replier).".into()
            } else {
                "Console KVL repliée.".into()
            };
        }

        // Fermer le guide avec Échap en mode jeu
        if self.editor.show_help_modal && (input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked) {
            self.editor.show_help_modal = false;
            self.audio.play(SoundType::UiClick);
            self.status_message = "Guide pédagogique fermé.".into();
            return;
        }

        // Interaction exclusive avec l'Assistant de Création Guidée KVL
        if self.editor.wizard.is_open {
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
                self.editor.wizard.close();
                self.audio.play(SoundType::UiClick);
                self.status_message = "Assistant Guidé KVL fermé.".into();
                return;
            }

            if input.is_key_just_pressed(KeyCode::Enter) {
                if self.editor.wizard.step == 3 {
                    self.apply_guided_wizard_mechanic();
                    self.editor.wizard.close();
                    self.audio.play(SoundType::ItemPickup);
                    return;
                } else {
                    self.editor.wizard.next_step();
                    self.audio.play(SoundType::UiClick);
                    return;
                }
            }

            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                if self.editor.wizard.is_close_clicked(mx, my) {
                    self.editor.wizard.close();
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Assistant Guidé KVL fermé.".into();
                    return;
                }

                match self.editor.wizard.step {
                    1 => {
                        if let Some(cat) = self.editor.wizard.clicked_category(mx, my) {
                            self.editor.wizard.selected_category = cat;
                            self.editor.wizard.next_step();
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                        if self.editor.wizard.is_next_clicked(mx, my) {
                            self.editor.wizard.next_step();
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                    }
                    2 => {
                        if let Some(c_idx) = self.editor.wizard.clicked_condition_option(mx, my) {
                            self.editor.wizard.condition_choice = c_idx;
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                        if let Some(a_idx) = self.editor.wizard.clicked_action_option(mx, my) {
                            self.editor.wizard.action_choice = a_idx;
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                        if let Some(s_idx) = self.editor.wizard.clicked_sound_option(mx, my) {
                            self.editor.wizard.sound_choice = s_idx;
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                        if self.editor.wizard.is_prev_clicked(mx, my) {
                            self.editor.wizard.prev_step();
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                        if self.editor.wizard.is_next_clicked(mx, my) {
                            self.editor.wizard.next_step();
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                    }
                    3 => {
                        if self.editor.wizard.is_prev_clicked(mx, my) {
                            self.editor.wizard.prev_step();
                            self.audio.play(SoundType::UiClick);
                            return;
                        }
                        if self.editor.wizard.is_generate_clicked(mx, my) {
                            self.apply_guided_wizard_mechanic();
                            self.editor.wizard.close();
                            self.audio.play(SoundType::ItemPickup);
                            return;
                        }
                    }
                    _ => {}
                }
                return;
            }
            return;
        }

        // Clic sur l'en-tête unifié, onglets, outils ou fermeture par clic modal
        if input.mouse_just_clicked {
            let mx = input.mouse_x;
            let my = input.mouse_y;
            if self.editor.is_hub_btn_clicked(mx, my) {
                self.editor.show_welcome_modal = true;
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.is_help_btn_clicked(mx, my) {
                self.editor.toggle_help();
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.is_play_tab_clicked(mx, my) {
                if self.editor.mode != EditorMode::Play {
                    self.editor.mode = EditorMode::Play;
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Mode Jeu immersif actif ! (ZQSD/Flèches pour jouer | TAB pour Studio)".into();
                }
                return;
            }
            if self.editor.is_studio_tab_clicked(mx, my) {
                if self.editor.mode != EditorMode::Edit {
                    self.editor.mode = EditorMode::Edit;
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Mode Studio Atelier actif ! Cliquez sur une entité pour inspecter ou déplacer.".into();
                }
                return;
            }
            if self.editor.is_console_toggle_clicked(mx, my) {
                self.editor.show_console_drawer = !self.editor.show_console_drawer;
                self.audio.play(SoundType::UiClick);
                self.status_message = if self.editor.show_console_drawer {
                    "Console KVL runtime étendue (~ pour replier).".into()
                } else {
                    "Console KVL repliée.".into()
                };
                return;
            }
            if self.editor.mode == EditorMode::Edit {
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
                if self.editor.is_guided_btn_clicked(mx, my) {
                    self.editor.wizard.open();
                    self.audio.play(SoundType::UiClick);
                    self.status_message = "Assistant Guidé KVL ouvert (Niveau 1) !".into();
                    return;
                }
                if self.editor.is_intent_bar_btn_clicked(mx, my) {
                    self.editor.show_intent_bar = !self.editor.show_intent_bar;
                    self.audio.play(SoundType::UiClick);
                    return;
                }
                if self.editor.is_undo_clicked(mx, my) {
                    self.undo();
                    return;
                }
                if self.editor.is_redo_clicked(mx, my) {
                    self.redo();
                    return;
                }
            }
            if self.editor.is_mode_toggle_btn_clicked(mx, my) {
                self.editor.toggle_mode();
                self.audio.play(SoundType::UiClick);
                self.status_message = match self.editor.mode {
                    EditorMode::Play => "Mode Jeu actif ! (ZQSD/Flèches pour jouer | TAB pour Studio)".into(),
                    EditorMode::Edit => "Mode Studio actif ! Cliquez sur une entité pour inspecter ou relier ses blocs KVL.".into(),
                };
                return;
            }
            if self.editor.is_genre_btn_clicked(mx, my) {
                self.cycle_scene_forward();
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.is_radar_btn_clicked(mx, my) {
                self.show_minimap = !self.show_minimap;
                self.audio.play(SoundType::UiClick);
                self.status_message = if self.show_minimap { "Radar tactique activé." } else { "Radar tactique masqué." }.into();
                return;
            }
            if self.editor.is_grid_btn_clicked(mx, my) {
                self.spatial_grid.debug_visible = !self.spatial_grid.debug_visible;
                self.audio.play(SoundType::UiClick);
                self.status_message = if self.spatial_grid.debug_visible { "Grille spatiale O(1) affichée." } else { "Grille spatiale masquée." }.into();
                return;
            }
            if self.editor.is_telemetry_btn_clicked(mx, my) {
                self.telemetry.show_overlay = !self.telemetry.show_overlay;
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.is_audio_btn_clicked(mx, my) {
                self.audio.toggle_mute();
                self.audio.play(SoundType::UiClick);
                self.status_message = if self.audio.is_muted() { "Audio désactivé (Mute)." } else { "Audio activé." }.into();
                return;
            }
            if self.editor.is_reset_btn_clicked(mx, my) {
                self.reload();
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.is_export_btn_clicked(mx, my) {
                self.editor.show_export_modal = !self.editor.show_export_modal;
                self.audio.play(SoundType::UiClick);
                self.status_message = if self.editor.show_export_modal {
                    "Exportation Standalone : Sélectionnez un format puis cliquez sur EXPORTER (Entrée).".into()
                } else {
                    "Exportation Standalone fermée.".into()
                };
                return;
            }
            if self.editor.is_recipe_btn_clicked(mx, my) {
                if self.editor.recipe_wizard.is_open {
                    self.editor.recipe_wizard.close();
                    self.status_message = "Assistant Recette de Jeu fermé.".into();
                } else {
                    self.editor.recipe_wizard.open();
                    self.status_message = "Assistant Recette de Jeu ouvert : Concevez votre jeu en 6 étapes !".into();
                }
                self.audio.play(SoundType::UiClick);
                return;
            }
            if self.editor.show_help_modal {
                if let Some(tab_idx) = self.editor.clicked_guide_tab(mx, my) {
                    self.editor.guide_tab = tab_idx;
                    self.audio.play(SoundType::UiClick);
                    return;
                }
                self.editor.show_help_modal = false;
                self.audio.play(SoundType::UiClick);
                return;
            }
        }

        // Bascule Mode Jeu / Mode Édition (TAB)
        if input.is_key_just_pressed(KeyCode::Tab) {
            self.editor.toggle_mode();
            if self.editor.mode == EditorMode::Edit {
                self.editor.guide_step_done[3] = true;
            }
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
                    self.dialogue_cooldown = 0.5;
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
                    self.dialogue_cooldown = 0.5;
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

        // Décrémentation du cooldown d'interaction de dialogue
        if self.dialogue_cooldown > 0.0 {
            self.dialogue_cooldown = (self.dialogue_cooldown - dt).max(0.0);
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

            let in_viewport = if self.editor.embedded_mode {
                input.mouse_x >= 0.0 && input.mouse_x <= 1200.0 && input.mouse_y >= 0.0 && input.mouse_y <= 800.0
            } else {
                input.mouse_x >= 20.0 && input.mouse_x <= 810.0 && input.mouse_y >= 66.0 && input.mouse_y <= 536.0
            };

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
                    let ent_deleted = self.entities[del_idx].clone();
                    let name = ent_deleted.name.clone();
                    let x = ent_deleted.transform.x;
                    let y = ent_deleted.transform.y;
                    self.editor.undo_stack.push(StudioCommand::DeleteEntity { entity: ent_deleted, index: del_idx });
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
                        let new_id = self.spawn_entity(tile_id, grid_x, grid_y);
                        self.editor.undo_stack.push(StudioCommand::SpawnEntity {
                            entity_id: new_id,
                            entity_name: format!("Tuile {}", self.editor.brush_tile.label()),
                            cell: Some((cell_col, cell_row)),
                        });
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
                    let ent_deleted = self.entities[del_idx].clone();
                    let name = ent_deleted.name.clone();
                    let x = ent_deleted.transform.x;
                    let y = ent_deleted.transform.y;
                    self.editor.undo_stack.push(StudioCommand::DeleteEntity { entity: ent_deleted, index: del_idx });
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

            // Ctrl+Z : Annuler la dernière action (Undo)
            if ctrl_or_cmd && !input.is_key_pressed(KeyCode::ShiftLeft) && !input.is_key_pressed(KeyCode::ShiftRight) && input.is_key_just_pressed(KeyCode::KeyZ) {
                self.undo();
                return;
            }

            // Ctrl+Y ou Ctrl+Shift+Z : Rétablir l'action (Redo)
            if (ctrl_or_cmd && input.is_key_just_pressed(KeyCode::KeyY))
                || (ctrl_or_cmd && (input.is_key_pressed(KeyCode::ShiftLeft) || input.is_key_pressed(KeyCode::ShiftRight)) && input.is_key_just_pressed(KeyCode::KeyZ))
            {
                self.redo();
                return;
            }

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

                // 00b. Clic sur les boutons Undo / Redo
                if self.editor.is_undo_clicked(mx, my) {
                    self.undo();
                    return;
                }
                if self.editor.is_redo_clicked(mx, my) {
                    self.redo();
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
                            let new_id = self.spawn_entity(&tpl_id, spawn_x, spawn_y);
                            self.editor.undo_stack.push(StudioCommand::SpawnEntity {
                                entity_id: new_id,
                                entity_name: tpl_id.clone(),
                                cell: None,
                            });
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
                    let in_game_viewport = if self.editor.embedded_mode {
                        mx >= 0.0 && mx <= 1200.0 && my >= 0.0 && my <= 800.0
                    } else {
                        mx >= 20.0 && mx <= 810.0 && my >= 66.0 && my <= 536.0
                    };
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

        // Mise à jour temporelle de l'inventaire
        self.inventory.update(dt);

        // Sélection et utilisation des emplacements rapides (Hotbar 1 à 5)
        if !self.dialogue.is_active {
            let hotbar_keys = [
                (KeyCode::Digit1, 0),
                (KeyCode::Digit2, 1),
                (KeyCode::Digit3, 2),
                (KeyCode::Digit4, 3),
                (KeyCode::Digit5, 4),
            ];

            for (k, slot_idx) in hotbar_keys {
                if input.is_key_just_pressed(k) {
                    if self.inventory.selected_hotbar_index == slot_idx {
                        self.use_hotbar_item(slot_idx);
                    } else {
                        self.inventory.select_slot(slot_idx);
                        self.kvl_context.set_var("hotbar_selected_slot", Value::Int(slot_idx as i64));
                        self.audio.play(SoundType::UiClick);
                    }
                }
            }

            // Touche d'utilisation directe de l'objet sélectionné (U)
            if input.is_key_just_pressed(KeyCode::KeyU) {
                let cur_slot = self.inventory.selected_hotbar_index;
                self.use_hotbar_item(cur_slot);
            }

            // Clic souris sur la barre d'action ou sur le terrain (Navigation A* Click-to-Move)
            if input.mouse_just_clicked {
                let mx = input.mouse_x as i32;
                let my = input.mouse_y as i32;
                let mut hit_hotbar = self.inventory.is_use_btn_clicked(input.mouse_x, input.mouse_y);
                if hit_hotbar {
                    let cur_slot = self.inventory.selected_hotbar_index;
                    self.use_hotbar_item(cur_slot);
                } else {
                    for i in 0..HOTBAR_SLOTS_COUNT {
                        if let Some((hx, hy, hw, hh)) = self.inventory.hotbar_hitboxes[i] {
                            if mx >= hx && mx < hx + hw as i32 && my >= hy && my < hy + hh as i32 {
                                hit_hotbar = true;
                                if self.inventory.selected_hotbar_index == i {
                                    self.use_hotbar_item(i);
                                } else {
                                    self.inventory.select_slot(i);
                                    self.kvl_context.set_var("hotbar_selected_slot", Value::Int(i as i64));
                                    self.audio.play(SoundType::UiClick);
                                }
                                break;
                            }
                        }
                    }
                }

                // Clic-déplacement intelligent (A* Click-to-Move) en mode Jeu
                if !hit_hotbar && !self.dialogue.is_active && self.editor.mode == crate::editor::EditorMode::Play {
                    let (wx, wy) = self.camera.screen_to_world(mx as f32, my as f32);
                    if let Some(p_idx) = self.player_index() {
                        let mut nav = crate::navigation::NavAgent::go_to(260.0, wx, wy);
                        nav.reach_distance = 18.0;
                        nav.avoid_hazards = true;
                        self.entities[p_idx].nav_agent = Some(nav);
                        self.status_message = format!("Navigation A* vers ({:.0}, {:.0}) enclenchée.", wx, wy);
                    }
                }
            }
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

            // Priorité absolue aux entrées clavier : annulation de toute navigation automatique
            if let Some(p_idx) = self.player_index() {
                if let Some(ref mut nav) = self.entities[p_idx].nav_agent {
                    if matches!(nav.mode, crate::navigation::NavMode::GoTo { .. }) {
                        nav.mode = crate::navigation::NavMode::Idle;
                    }
                }
            }
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
        if self.entities.len() != self.static_entity_count {
            self.rebuild_static_spatial_grid();
        }
        self.spatial_grid.clear();
        for (idx, ent) in self.entities.iter().enumerate() {
            if !ent.is_active {
                continue;
            }
            if let Some(ref col) = ent.collider {
                // Les murs et plateformes fixes sont déjà conservés dans static_buckets
                let is_static = ent.movement.is_none() && (col.is_solid || col.is_one_way) && !col.is_pushable;
                if !is_static {
                    let kind = if col.is_one_way {
                        SpatialKind::OneWay
                    } else if col.is_solid {
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

                        self.collision_candidates.clear();
                        {
                            let raw = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                            self.collision_candidates.extend_from_slice(raw);
                        }

                        for i in 0..self.collision_candidates.len() {
                            let item = self.collision_candidates[i];
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
                                        self.play_block_push();
                                    } else if move_x > 0.0 {
                                        cur_x = item.x - pw;
                                    } else if move_x < 0.0 {
                                        cur_x = item.x + item.w;
                                    }
                                } else if move_x > 0.0 {
                                    let p_prev_cx = px + pw * 0.5;
                                    let item_cx = item.x + item.w * 0.5;
                                    if p_prev_cx <= item_cx {
                                        cur_x = (item.x - pw).min(cur_x);
                                    }
                                } else if move_x < 0.0 {
                                    let p_prev_cx = px + pw * 0.5;
                                    let item_cx = item.x + item.w * 0.5;
                                    if p_prev_cx >= item_cx {
                                        cur_x = (item.x + item.w).max(cur_x);
                                    }
                                }
                            }
                        }
                    }

                    if move_y != 0.0 {
                        cur_y += move_y * speed * dt;
                        cur_y = cur_y.clamp(75.0, 595.0 - ph);

                        self.collision_candidates.clear();
                        {
                            let raw = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                            self.collision_candidates.extend_from_slice(raw);
                        }

                        for i in 0..self.collision_candidates.len() {
                            let item = self.collision_candidates[i];
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
                                        self.play_block_push();
                                    } else if move_y > 0.0 {
                                        let p_prev_cy = py + ph * 0.5;
                                        let item_cy = item.y + item.h * 0.5;
                                        if p_prev_cy <= item_cy {
                                            cur_y = (item.y - ph).min(cur_y);
                                        }
                                    } else if move_y < 0.0 {
                                        let p_prev_cy = py + ph * 0.5;
                                        let item_cy = item.y + item.h * 0.5;
                                        if p_prev_cy >= item_cy {
                                            cur_y = (item.y + item.h).max(cur_y);
                                        }
                                    }
                                } else if move_y > 0.0 {
                                    let p_prev_cy = py + ph * 0.5;
                                    let item_cy = item.y + item.h * 0.5;
                                    if p_prev_cy <= item_cy {
                                        cur_y = (item.y - ph).min(cur_y);
                                    }
                                } else if move_y < 0.0 {
                                    let p_prev_cy = py + ph * 0.5;
                                    let item_cy = item.y + item.h * 0.5;
                                    if p_prev_cy >= item_cy {
                                        cur_y = (item.y + item.h).max(cur_y);
                                    }
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
                    let drop_through = input.is_key_down(KeyCode::ArrowDown) || input.is_key_down(KeyCode::KeyS);

                    for item in candidates {
                        if item.id != p_idx {
                            if matches!(item.kind, SpatialKind::Solid) {
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
                            } else if matches!(item.kind, SpatialKind::OneWay) {
                                // Plateforme traversable par le bas : atterrissage si le joueur tombe et ne force pas la descente
                                if cur_vy > 0.0 && !drop_through {
                                    let prev_player_bottom = py + ph;
                                    if prev_player_bottom <= item.y + (cur_vy * dt).max(8.0) {
                                        cur_y = item.y - ph;
                                        cur_vy = 0.0;
                                        grounded_this_frame = true;
                                    }
                                }
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

                        self.collision_candidates.clear();
                        {
                            let raw = self.spatial_grid.query_aabb(target_x, target_y, pw, ph);
                            self.collision_candidates.extend_from_slice(raw);
                        }

                        let mut can_step = true;
                        for i in 0..self.collision_candidates.len() {
                            let item = self.collision_candidates[i];
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
                                        self.play_block_push();
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
        self.hit_events.clear();
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
                    ent.is_active = false;
                    self.hit_events.push((idx, item.id, true));
                    break;
                }
            }
        }

        for i in 0..self.hit_events.len() {
            let (bullet_idx, target_idx, _) = self.hit_events[i];
            if target_idx < self.entities.len() {
                let target_tag = self.entities[target_idx].tag.clone();
                let proj_dmg = if bullet_idx < self.entities.len() {
                    self.entities[bullet_idx].projectile.as_ref().map_or(10.0, |p| p.damage as f32)
                } else {
                    10.0
                };

                if target_tag == "enemy" {
                    self.entities[target_idx].is_active = false;
                    self.audio.play(SoundType::BonusFanfare);
                    let cur_score = match self.kvl_context.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };
                    self.kvl_context.set_var("score", Value::Int(cur_score + 25));
                    self.status_message = "Cible ennemie neutralisée par tir balistique ! (+25 pts)".into();
                } else if target_tag == "boss" {
                    let mut took_dmg = false;
                    let mut cur_hp = 0.0;
                    let mut max_hp = 60.0;
                    let mut should_phase_2 = false;
                    let mut should_phase_3 = false;
                    let mut is_defeated = false;

                    if let Some(ref mut h) = self.entities[target_idx].health {
                        h.current = (h.current - proj_dmg).max(0.0);
                        cur_hp = h.current;
                        max_hp = h.max;
                        took_dmg = true;
                        if cur_hp <= 0.0 {
                            h.is_dead = true;
                            is_defeated = true;
                        }
                    }

                    if took_dmg {
                        self.audio.play(SoundType::BossRoar);
                        self.camera.add_shake(7.0);
                        let bcx = self.entities[target_idx].transform.center_x();
                        let bcy = self.entities[target_idx].transform.center_y();
                        self.spawn_particles(bcx, bcy, 20, 0x0038BDF8);
                        self.kvl_context.set_var("boss_hp", Value::Int(cur_hp as i64));

                        if let Some(ref mut boss) = self.entities[target_idx].boss {
                            if !is_defeated {
                                let ratio = cur_hp / max_hp;
                                if ratio <= 0.33 && boss.phase != BossPhase::Phase3 {
                                    boss.phase = BossPhase::Phase3;
                                    boss.enrage_multiplier = 1.75;
                                    should_phase_3 = true;
                                } else if ratio <= 0.66 && boss.phase == BossPhase::Phase1 {
                                    boss.phase = BossPhase::Phase2;
                                    boss.enrage_multiplier = 1.35;
                                    should_phase_2 = true;
                                }
                            } else if boss.phase != BossPhase::Defeated {
                                boss.phase = BossPhase::Defeated;
                            }
                        }

                        if should_phase_2 {
                            self.kvl_context.set_var("boss_phase", Value::Int(2));
                            self.audio.play(SoundType::Alarm);
                            self.camera.add_shake(14.0);
                            self.status_message = "[SURCHARGE PLASMIQUE] Le Titan entre en Phase 2 ! Vitesse et attaques accrues !".into();
                            if let Some(ref mut r) = self.entities[target_idx].renderable {
                                r.color = 0x00F59E0B;
                                r.outline_color = Some(0x00FBBF24);
                            }
                            if let Some(ref script) = self.entities[target_idx].script.clone() {
                                if let Some(ref evt) = script.on_seen_event {
                                    if let Some(graph) = self.kvl_graphs.get(&script.script_path) {
                                        let _ = Interpreter::trigger_event(graph, &mut self.kvl_context, evt);
                                    }
                                }
                            }
                        } else if should_phase_3 {
                            self.kvl_context.set_var("boss_phase", Value::Int(3));
                            self.kvl_context.set_var("boss_enraged", Value::Bool(true));
                            self.audio.play(SoundType::Alarm);
                            self.camera.add_shake(22.0);
                            self.status_message = "[OVERDRIVE ENRAGÉ] Le Titan déchaîne son mode critique ! Bombardements orbitaux imminents !".into();
                            if let Some(ref mut r) = self.entities[target_idx].renderable {
                                r.color = 0x00EF4444;
                                r.outline_color = Some(0x00FCA5A5);
                            }
                            if let Some(ref script) = self.entities[target_idx].script.clone() {
                                if let Some(ref evt) = script.on_seen_event {
                                    if let Some(graph) = self.kvl_graphs.get(&script.script_path) {
                                        let _ = Interpreter::trigger_event(graph, &mut self.kvl_context, evt);
                                    }
                                }
                            }
                        } else if is_defeated {
                            self.kvl_context.set_var("boss_defeated", Value::Bool(true));
                            self.kvl_context.set_var("boss_phase", Value::Int(4));
                            self.audio.play(SoundType::BonusFanfare);
                            self.audio.play(SoundType::QuestSuccess);
                            self.camera.add_shake(25.0);
                            self.spawn_particles(bcx, bcy, 80, 0x00FDE047);
                            self.status_message = "TRIOMPHE TOTAL ! Le Titan KVL-MK3 a été terrassé ! Victoire !".into();
                            if let Some(ref script) = self.entities[target_idx].script.clone() {
                                if let Some(graph) = self.kvl_graphs.get(&script.script_path) {
                                    let _ = Interpreter::trigger_event(graph, &mut self.kvl_context, "OnBossDefeated");
                                }
                            }
                        } else {
                            self.status_message = format!("Impact direct sur le Titan ! PV restants : {:.0} / {:.0}", cur_hp, max_hp);
                        }
                    }
                } else if target_tag == "player" {
                    let mut took_dmg = false;
                    let mut is_dead = false;
                    let mut cur_hp = 0.0;
                    let mut max_hp = 0.0;
                    if let Some(ref mut h) = self.entities[target_idx].health {
                        if h.damage(1.0) {
                            took_dmg = true;
                            is_dead = h.is_dead;
                            cur_hp = h.current;
                            max_hp = h.max;
                        }
                    }
                    if took_dmg {
                        self.audio.play(SoundType::PlayerDamage);
                        self.camera.add_shake(10.0);
                        let pcx = self.entities[target_idx].transform.center_x();
                        let pcy = self.entities[target_idx].transform.center_y();
                        self.spawn_particles(pcx, pcy, 25, 0x00EF4444);
                        if is_dead {
                            self.audio.play(SoundType::PlayerDeath);
                            self.camera.add_shake(18.0);
                            self.entities[target_idx].transform.x = self.player_spawn_pos.0;
                            self.entities[target_idx].transform.y = self.player_spawn_pos.1;
                            if let Some(ref mut mov) = self.entities[target_idx].movement {
                                mov.vx = 0.0;
                                mov.vy = 0.0;
                            }
                            if let Some(ref mut h) = self.entities[target_idx].health {
                                h.current = h.max;
                                h.is_dead = false;
                                h.invulnerability_timer = 1.5;
                            }
                            self.status_message = "VOUS AVEZ PÉRI FACE AU TITAN ! Réapparition...".into();
                        } else {
                            self.status_message = format!("TOUCHÉ ! Tir hostile subi ! Santé : {:.0}/{:.0} PV", cur_hp, max_hp);
                        }
                    }
                } else {
                    self.audio.play(SoundType::Sparkles);
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
            // Si l'entité navigue activement avec A*, le simple va-et-vient est supplanté
            if ent.nav_agent.as_ref().map_or(false, |n| n.mode != crate::navigation::NavMode::Idle) {
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
        // SYSTEM 3.5 : NAVIGATION AUTONOME & PATHFINDING A*
        // ==========================================
        let player_target = if let Some(p_idx) = self.player_index() {
            Some((self.entities[p_idx].transform.center_x(), self.entities[p_idx].transform.center_y()))
        } else {
            None
        };

        let obstacles: Vec<(usize, f32, f32, f32, f32)> = self.entities.iter()
            .filter(|e| e.is_active && e.collider.map_or(false, |c| c.is_solid))
            .map(|e| {
                let col = e.collider.as_ref().unwrap();
                (e.id, e.transform.x + col.offset_x, e.transform.y + col.offset_y, col.w, col.h)
            })
            .collect();

        let hazards: Vec<(f32, f32, f32, f32)> = self.entities.iter()
            .filter(|e| e.is_active && (e.tag == "hazard" || e.collider.map_or(false, |c| c.is_trigger && e.tag == "hazard")))
            .map(|e| (e.transform.x, e.transform.y, e.transform.w, e.transform.h))
            .collect();

        let wb = self.world_bounds;

        for ent_idx in 0..self.entities.len() {
            if !self.entities[ent_idx].is_active {
                continue;
            }
            if self.entities[ent_idx].nav_agent.is_none() {
                continue;
            }

            let ent_id = self.entities[ent_idx].id;
            let (start_pos, mut nav) = {
                let ent = &self.entities[ent_idx];
                ((ent.transform.center_x(), ent.transform.center_y()), ent.nav_agent.clone().unwrap())
            };

            let target = match nav.mode {
                crate::navigation::NavMode::Idle => None,
                crate::navigation::NavMode::ChasePlayer => player_target,
                crate::navigation::NavMode::GoTo { x, y } => Some((x as f32, y as f32)),
                crate::navigation::NavMode::FleePlayer => {
                    if let Some(p) = player_target {
                        let dx = start_pos.0 - p.0;
                        let dy = start_pos.1 - p.1;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        Some((start_pos.0 + (dx / dist) * 200.0, start_pos.1 + (dy / dist) * 200.0))
                    } else {
                        None
                    }
                }
                crate::navigation::NavMode::PatrolWaypoints => {
                    if nav.waypoints.is_empty() {
                        None
                    } else {
                        Some(nav.waypoints[nav.current_waypoint_idx % nav.waypoints.len()])
                    }
                }
            };

            nav.repath_timer -= dt;

            if let Some(tgt) = target {
                let need_repath = nav.repath_timer <= 0.0
                    || nav.waypoints.is_empty()
                    || nav.target_pos.map_or(true, |tp| {
                        let t_dx = tp.0 - tgt.0;
                        let t_dy = tp.1 - tgt.1;
                        (t_dx * t_dx + t_dy * t_dy) > 32.0 * 32.0
                    });

                if need_repath {
                    let avoid_haz = nav.avoid_hazards;
                    let path = self.pathfinder.find_path(start_pos, tgt, |cx, cy| {
                        let cell_x = cx as f32 * 32.0;
                        let cell_y = cy as f32 * 32.0;

                        if cell_x < wb.0 || cell_x + 32.0 > wb.2 || cell_y < wb.1 || cell_y + 32.0 > wb.3 {
                            return (false, 0);
                        }

                        for &(obs_id, ox, oy, ow, oh) in &obstacles {
                            if obs_id == ent_id {
                                continue;
                            }
                            if cell_x + 30.0 > ox && cell_x + 2.0 < ox + ow && cell_y + 30.0 > oy && cell_y + 2.0 < oy + oh {
                                return (false, 0);
                            }
                        }

                        let mut penalty = 0;
                        if avoid_haz {
                            for &(hx, hy, hw, hh) in &hazards {
                                if cell_x + 28.0 > hx && cell_x + 4.0 < hx + hw && cell_y + 28.0 > hy && cell_y + 4.0 < hy + hh {
                                    penalty = 120;
                                    break;
                                }
                            }
                        }

                        (true, penalty)
                    });

                    if let Some(new_wps) = path {
                        nav.waypoints = new_wps;
                        nav.current_waypoint_idx = 0;
                        nav.target_pos = Some(tgt);
                        nav.repath_timer = nav.repath_interval;
                    }
                }
            }

            if nav.current_waypoint_idx < nav.waypoints.len() {
                let wp = nav.waypoints[nav.current_waypoint_idx];
                let dx = wp.0 - start_pos.0;
                let dy = wp.1 - start_pos.1;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist <= nav.reach_distance {
                    nav.current_waypoint_idx += 1;
                    if nav.current_waypoint_idx >= nav.waypoints.len() {
                        if nav.mode == crate::navigation::NavMode::PatrolWaypoints && !nav.waypoints.is_empty() {
                            nav.current_waypoint_idx = 0;
                        } else if matches!(nav.mode, crate::navigation::NavMode::GoTo { .. }) {
                            nav.mode = crate::navigation::NavMode::Idle;
                        }
                    }
                } else {
                    let step = (nav.speed * dt).min(dist);
                    let vx = (dx / dist) * step;
                    let vy = (dy / dist) * step;

                    let ent = &mut self.entities[ent_idx];

                    ent.transform.x += vx;
                    let (ew, eh) = (ent.transform.w, ent.transform.h);
                    for &(obs_id, ox, oy, ow, oh) in &obstacles {
                        if obs_id == ent_id {
                            continue;
                        }
                        if ent.transform.x + ew > ox && ent.transform.x < ox + ow && ent.transform.y + eh > oy && ent.transform.y < oy + oh {
                            if vx > 0.0 {
                                ent.transform.x = ox - ew;
                            } else if vx < 0.0 {
                                ent.transform.x = ox + ow;
                            }
                        }
                    }

                    ent.transform.y += vy;
                    for &(obs_id, ox, oy, ow, oh) in &obstacles {
                        if obs_id == ent_id {
                            continue;
                        }
                        if ent.transform.x + ew > ox && ent.transform.x < ox + ow && ent.transform.y + eh > oy && ent.transform.y < oy + oh {
                            if vy > 0.0 {
                                ent.transform.y = oy - eh;
                            } else if vy < 0.0 {
                                ent.transform.y = oy + oh;
                            }
                        }
                    }
                }
            }

            self.entities[ent_idx].nav_agent = Some(nav);
        }

        // ==========================================
        // SYSTEM 3.6 : COMPORTEMENT DE MEUTE & ESSAIM (FLOCKING / BOIDS)
        // ==========================================
        let has_flocks = self.entities.iter().any(|e| e.is_active && e.flock.is_some());
        if has_flocks {
            // 1. Collecter les instantanés légers des membres de meutes
            let boid_snapshots: Vec<(usize, u32, f32, f32, f32, f32, bool)> = self.entities.iter()
                .filter(|e| e.is_active && e.flock.is_some())
                .map(|e| {
                    let fl = e.flock.as_ref().unwrap();
                    let (vx, vy) = e.movement.as_ref().map_or((0.0, 0.0), |m| (m.vx, m.vy));
                    (e.id, fl.swarm_id, e.transform.center_x(), e.transform.center_y(), vx, vy, fl.role == crate::flocking::FlockRole::Leader)
                })
                .collect();

            // Mettre à jour les comptages de meutes dans script_ctx
            let mut swarm_counts: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
            for (_, swarm_id, _, _, _, _, _) in &boid_snapshots {
                *swarm_counts.entry(*swarm_id).or_insert(0) += 1;
            }
            for (sid, count) in &swarm_counts {
                self.kvl_context.set_var(format!("swarm_count_{}", sid), Value::Int(*count));
            }

            for ent_idx in 0..self.entities.len() {
                if !self.entities[ent_idx].is_active {
                    continue;
                }
                if self.entities[ent_idx].flock.is_none() {
                    continue;
                }

                let ent_id = self.entities[ent_idx].id;
                let mut flock = self.entities[ent_idx].flock.clone().unwrap();
                let pos = (self.entities[ent_idx].transform.center_x(), self.entities[ent_idx].transform.center_y());
                let vel = self.entities[ent_idx].movement.as_ref().map_or((0.0, 0.0), |m| (m.vx, m.vy));

                // Synchronisation des variables KVL pour cet essaim
                let sid = flock.swarm_id;
                if let Some(Value::Float(sc_dur)) = self.kvl_context.get_var(&format!("swarm_scatter_{}", sid)) {
                    if *sc_dur > 0.0 {
                        flock.scatter(*sc_dur as f32);
                        self.kvl_context.set_var(format!("swarm_scatter_{}", sid), Value::Float(0.0));
                    }
                }
                if let Some(Value::Float(co_w)) = self.kvl_context.get_var(&format!("swarm_cohesion_{}", sid)) {
                    flock.weight_cohesion = *co_w as f32;
                }

                // Décompte du timer de dispersion
                if flock.scatter_timer > 0.0 {
                    flock.scatter_timer = (flock.scatter_timer - dt).max(0.0);
                    self.kvl_context.set_var(format!("swarm_is_scattered_{}", sid), Value::Bool(flock.scatter_timer > 0.0));
                }

                // Cible dynamique
                let mut override_target = None;
                if let Some(Value::String(ref mode)) = self.kvl_context.get_var(&format!("swarm_target_mode_{}", sid)) {
                    if mode == "Player" {
                        override_target = player_target;
                    }
                }
                if override_target.is_none() {
                    let tx_var = self.kvl_context.get_var(&format!("swarm_target_x_{}", sid));
                    let ty_var = self.kvl_context.get_var(&format!("swarm_target_y_{}", sid));
                    if let (Some(Value::Int(tx)), Some(Value::Int(ty))) = (tx_var, ty_var) {
                        override_target = Some((*tx as f32, *ty as f32));
                    }
                }

                // Si pas de cible KVL mais que le joueur existe et que l'entité est agressive
                if override_target.is_none() && self.entities[ent_idx].tag.contains("enemy") {
                    override_target = player_target;
                }

                // Filtrer les voisins appartenant à la même meute
                let neighbors: Vec<crate::flocking::BoidNeighbor> = boid_snapshots.iter()
                    .filter(|(id, s_id, _, _, _, _, _)| *id != ent_id && *s_id == sid)
                    .map(|(id, _, nx, ny, nvx, nvy, is_lead)| crate::flocking::BoidNeighbor {
                        id: *id,
                        x: *nx,
                        y: *ny,
                        vx: *nvx,
                        vy: *nvy,
                        is_leader: *is_lead,
                    })
                    .collect();

                let (fx, fy) = crate::flocking::compute_flock_steering(&flock, pos, vel, &neighbors, override_target);

                // Appliquer la force d'accélération
                let mut nvx = vel.0 + fx * dt;
                let mut nvy = vel.1 + fy * dt;
                let speed = (nvx * nvx + nvy * nvy).sqrt();
                if speed > flock.max_speed {
                    nvx = (nvx / speed) * flock.max_speed;
                    nvy = (nvy / speed) * flock.max_speed;
                }

                // Déplacement continu avec résolution de collisions
                let ent = &mut self.entities[ent_idx];
                ent.transform.x += nvx * dt;
                let (ew, eh) = (ent.transform.w, ent.transform.h);
                for &(obs_id, ox, oy, ow, oh) in &obstacles {
                    if obs_id == ent_id {
                        continue;
                    }
                    if ent.transform.x + ew > ox && ent.transform.x < ox + ow && ent.transform.y + eh > oy && ent.transform.y < oy + oh {
                        if nvx > 0.0 {
                            ent.transform.x = ox - ew;
                            nvx = -nvx * 0.3;
                        } else if nvx < 0.0 {
                            ent.transform.x = ox + ow;
                            nvx = -nvx * 0.3;
                        }
                    }
                }

                ent.transform.y += nvy * dt;
                for &(obs_id, ox, oy, ow, oh) in &obstacles {
                    if obs_id == ent_id {
                        continue;
                    }
                    if ent.transform.x + ew > ox && ent.transform.x < ox + ow && ent.transform.y + eh > oy && ent.transform.y < oy + oh {
                        if nvy > 0.0 {
                            ent.transform.y = oy - eh;
                            nvy = -nvy * 0.3;
                        } else if nvy < 0.0 {
                            ent.transform.y = oy + oh;
                            nvy = -nvy * 0.3;
                        }
                    }
                }

                // Confinement dans les bordures du monde
                ent.transform.x = ent.transform.x.clamp(wb.0 + 8.0, wb.2 - ew - 8.0);
                ent.transform.y = ent.transform.y.clamp(wb.1 + 8.0, wb.3 - eh - 8.0);

                if let Some(ref mut mov) = ent.movement {
                    mov.vx = nvx;
                    mov.vy = nvy;
                }

                ent.flock = Some(flock);
            }
        }

        // ==========================================
        // SYSTEM : COMBAT DE BOSS, TÉLÉGRAPHIE D'ATTAQUES & MOTIFS BALISTIQUES
        // ==========================================
        let mut boss_bullet_spawns = Vec::new();
        let mut boss_slams = Vec::new();
        let mut boss_telegraph_sound = false;
        let mut boss_slam_sound = false;
        let mut boss_laser_sound = false;
        let mut boss_shake = 0.0f32;

        if let Some(p_idx) = self.player_index() {
            let (p_cx, p_cy) = (self.entities[p_idx].transform.center_x(), self.entities[p_idx].transform.center_y());

            for (b_idx, ent) in self.entities.iter_mut().enumerate() {
                if !ent.is_active {
                    continue;
                }
                if let Some(ref mut boss) = ent.boss {
                    if boss.phase == BossPhase::Defeated {
                        continue;
                    }

                    let b_cx = ent.transform.center_x();
                    let b_cy = ent.transform.center_y();

                    // 1. Mise à jour des télégraphies actives
                    for t in &mut boss.telegraphs {
                        if !t.is_active {
                            continue;
                        }
                        t.timer += dt;
                        if t.timer >= t.duration {
                            t.is_active = false;
                            match t.attack_type {
                                BossAttackType::RadialNova => {
                                    let bullet_count = match boss.phase {
                                        BossPhase::Phase1 => 12,
                                        BossPhase::Phase2 => 16,
                                        BossPhase::Phase3 => 20,
                                        BossPhase::Defeated => 0,
                                    };
                                    let speed = match boss.phase {
                                        BossPhase::Phase1 => 220.0,
                                        BossPhase::Phase2 => 270.0,
                                        BossPhase::Phase3 => 330.0,
                                        BossPhase::Defeated => 0.0,
                                    };
                                    let color = match boss.phase {
                                        BossPhase::Phase1 => 0x0038BDF8,
                                        BossPhase::Phase2 => 0x00F59E0B,
                                        _ => 0x00EF4444,
                                    };
                                    let step = std::f32::consts::TAU / bullet_count as f32;
                                    let offset = (boss.attack_counter as f32 * 0.25).sin();
                                    for i in 0..bullet_count {
                                        let ang = i as f32 * step + offset;
                                        boss_bullet_spawns.push((b_cx, b_cy, ang.cos() * speed, ang.sin() * speed, b_idx, color));
                                    }
                                    boss_laser_sound = true;
                                    boss_shake = boss_shake.max(6.0);
                                }
                                BossAttackType::TargetedSpread => {
                                    let count = if boss.phase == BossPhase::Phase3 { 7 } else { 5 };
                                    let speed = 300.0;
                                    let target_ang = (p_cy - b_cy).atan2(p_cx - b_cx);
                                    let spread = 0.55;
                                    let color = if boss.phase == BossPhase::Phase3 { 0x00EF4444 } else { 0x00F59E0B };
                                    for i in 0..count {
                                        let t_ratio = if count > 1 { (i as f32 / (count - 1) as f32) - 0.5 } else { 0.0 };
                                        let ang = target_ang + t_ratio * spread;
                                        boss_bullet_spawns.push((b_cx, b_cy, ang.cos() * speed, ang.sin() * speed, b_idx, color));
                                    }
                                    boss_laser_sound = true;
                                    boss_shake = boss_shake.max(5.0);
                                }
                                BossAttackType::SeismicSlam => {
                                    boss_slams.push((t.x, t.y, 110.0, 1.0));
                                    boss_slam_sound = true;
                                    boss_shake = boss_shake.max(18.0);
                                }
                                BossAttackType::OrbitalStrike => {
                                    boss_slams.push((t.x, t.y, 65.0, 1.0));
                                    boss_laser_sound = true;
                                    boss_shake = boss_shake.max(10.0);
                                }
                            }
                        }
                    }

                    boss.telegraphs.retain(|t| t.is_active);

                    // 2. IA de cadence d'attaque et armement de télégraphies
                    boss.attack_timer -= dt;
                    if boss.attack_timer <= 0.0 {
                        boss.attack_counter += 1;
                        let cooldown = (boss.attack_cooldown / boss.enrage_multiplier).max(0.8);
                        boss.attack_timer = cooldown;

                        match boss.phase {
                            BossPhase::Phase1 => {
                                if boss.attack_counter % 2 == 0 {
                                    let ang = (p_cy - b_cy).atan2(p_cx - b_cx);
                                    boss.telegraphs.push(TelegraphZone::new_line(
                                        b_cx, b_cy, 280.0, 32.0, ang, 1.1, BossAttackType::TargetedSpread, 0x0006B6D4, 1.0,
                                    ));
                                } else {
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        b_cx, b_cy, 95.0, 1.2, BossAttackType::RadialNova, 0x0038BDF8, 1.0,
                                    ));
                                }
                                boss_telegraph_sound = true;
                            }
                            BossPhase::Phase2 => {
                                let cycle = boss.attack_counter % 3;
                                if cycle == 0 {
                                    let ang = (p_cy - b_cy).atan2(p_cx - b_cx);
                                    boss.telegraphs.push(TelegraphZone::new_line(
                                        b_cx, b_cy, 320.0, 36.0, ang, 0.9, BossAttackType::TargetedSpread, 0x00F59E0B, 1.0,
                                    ));
                                } else if cycle == 1 {
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        b_cx, b_cy, 120.0, 1.0, BossAttackType::RadialNova, 0x00F59E0B, 1.0,
                                    ));
                                } else {
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        p_cx, p_cy, 110.0, 1.2, BossAttackType::SeismicSlam, 0x00DC2626, 1.0,
                                    ));
                                }
                                boss_telegraph_sound = true;
                            }
                            BossPhase::Phase3 => {
                                let cycle = boss.attack_counter % 4;
                                if cycle == 0 {
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        p_cx, p_cy, 65.0, 0.95, BossAttackType::OrbitalStrike, 0x00EF4444, 1.0,
                                    ));
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        p_cx - 85.0, p_cy + 20.0, 60.0, 1.05, BossAttackType::OrbitalStrike, 0x00EF4444, 1.0,
                                    ));
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        p_cx + 85.0, p_cy - 20.0, 60.0, 1.05, BossAttackType::OrbitalStrike, 0x00EF4444, 1.0,
                                    ));
                                } else if cycle == 1 {
                                    let ang = (p_cy - b_cy).atan2(p_cx - b_cx);
                                    boss.telegraphs.push(TelegraphZone::new_line(
                                        b_cx, b_cy, 360.0, 40.0, ang, 0.75, BossAttackType::TargetedSpread, 0x00EF4444, 1.0,
                                    ));
                                } else if cycle == 2 {
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        b_cx, b_cy, 140.0, 0.85, BossAttackType::RadialNova, 0x00EF4444, 1.0,
                                    ));
                                } else {
                                    boss.telegraphs.push(TelegraphZone::new_circle(
                                        p_cx, p_cy, 110.0, 0.85, BossAttackType::SeismicSlam, 0x00DC2626, 1.0,
                                    ));
                                }
                                boss_telegraph_sound = true;
                            }
                            BossPhase::Defeated => {}
                        }
                    }
                }
            }
        }

        if boss_telegraph_sound {
            self.audio.play(SoundType::BossTelegraph);
        }
        if boss_slam_sound {
            self.audio.play(SoundType::BossSlam);
        }
        if boss_laser_sound {
            self.audio.play(SoundType::LaserShoot);
        }
        if boss_shake > 0.0 {
            self.camera.add_shake(boss_shake);
        }

        // Spawns des projectiles de Boss
        for (bx, by, vx, vy, src_id, color) in boss_bullet_spawns {
            let next_id = self.entities.len() + 1;
            let mut bullet = Entity::spawn_projectile(next_id, bx - 5.0, by - 5.0, vx, vy, src_id);
            bullet.renderable = Some(Renderable2D::circle(5.0, color, 12).with_outline(0x00FFFFFF, 1));
            self.entities.push(bullet);
        }

        // Dégâts de zone des Slams & Frappes Orbitales
        if let Some(p_idx) = self.player_index() {
            let pcx = self.entities[p_idx].transform.center_x();
            let pcy = self.entities[p_idx].transform.center_y();

            for (sx, sy, radius, dmg) in boss_slams {
                self.spawn_particles(sx, sy, 30, 0x00F59E0B);
                let dist = ((pcx - sx).powi(2) + (pcy - sy).powi(2)).sqrt();
                if dist <= radius {
                    let mut took_dmg = false;
                    let mut is_dead = false;
                    if let Some(ref mut h) = self.entities[p_idx].health {
                        if h.damage(dmg) {
                            took_dmg = true;
                            is_dead = h.is_dead;
                        }
                    }
                    if took_dmg {
                        self.audio.play(SoundType::PlayerDamage);
                        self.camera.add_shake(12.0);
                        self.spawn_particles(pcx, pcy, 25, 0x00EF4444);
                        if is_dead {
                            self.audio.play(SoundType::PlayerDeath);
                            self.camera.add_shake(20.0);
                            self.entities[p_idx].transform.x = self.player_spawn_pos.0;
                            self.entities[p_idx].transform.y = self.player_spawn_pos.1;
                            if let Some(ref mut mov) = self.entities[p_idx].movement {
                                mov.vx = 0.0;
                                mov.vy = 0.0;
                            }
                            if let Some(ref mut h) = self.entities[p_idx].health {
                                h.current = h.max;
                                h.is_dead = false;
                                h.invulnerability_timer = 1.5;
                            }
                            self.status_message = "ÉCRASÉ PAR L'ONDE DE CHOC DU TITAN ! Réapparition...".into();
                        } else {
                            self.status_message = "OUCH ! Onde de choc subie de plein fouet !".into();
                        }
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

        // Mise à jour de l'oscillation flottante des objets ramassables dans le monde
        for ent in &mut self.entities {
            if ent.is_active {
                if let Some(ref mut pickup) = ent.item_pickup {
                    pickup.bob_timer += dt * 3.5;
                    ent.transform.y = pickup.base_y + (pickup.bob_timer.sin() * 3.5);
                }
            }
        }

        let mut spawn_requests = Vec::new();

        // ==========================================
        // SYSTEM 4 : TRIGGER OVERLAP & COLLECTIBLES (PIÈCES, CŒURS, OBJETS, PIÈGES)
        // ==========================================
        if let Some((p_idx, px, py, pw, ph)) = player_pos {
            let mut collected_indices = Vec::new();
            let mut heart_indices = Vec::new();
            let mut item_pickup_indices = Vec::new();
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
                            let is_item_pickup = ent.tag == "item_pickup" || ent.item_pickup.is_some();

                            if dist < max_reach {
                                if is_collectible {
                                    collected_indices.push(idx);
                                } else if is_heart {
                                    heart_indices.push(idx);
                                } else if is_item_pickup {
                                    item_pickup_indices.push(idx);
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
            let mut contact_enemy_pos: Option<(f32, f32)> = None;
            for (idx, ent) in self.entities.iter().enumerate() {
                if idx != p_idx && ent.is_active && ent.tag == "enemy" {
                    let ex = ent.transform.x;
                    let ey = ent.transform.y;
                    let ew = ent.transform.w;
                    let eh = ent.transform.h;
                    if px < ex + ew && px + pw > ex && py < ey + eh && py + ph > ey {
                        enemy_contact = true;
                        contact_enemy_pos = Some((ent.transform.center_x(), ent.transform.center_y()));
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

            // Gestion du ramassage d'objets dans l'inventaire
            for idx in item_pickup_indices {
                let (_ent_name, ent_cx, ent_cy, opt_pickup) = {
                    let ent = &mut self.entities[idx];
                    (ent.name.clone(), ent.transform.center_x(), ent.transform.center_y(), ent.item_pickup.clone())
                };

                if let Some(pickup) = opt_pickup {
                    let rem = self.inventory.add_item(pickup.item.clone(), pickup.count);
                    if rem < pickup.count {
                        self.entities[idx].is_active = false;
                        self.audio.play(SoundType::ItemPickup);
                        spawn_requests.push((ent_cx, ent_cy, 24, pickup.item.color));
                        let total = self.inventory.get_item_count(&pickup.item.id);
                        self.kvl_context.set_var(format!("item_count_{}", pickup.item.id), Value::Int(total as i64));
                        self.kvl_context.set_var(format!("has_{}", pickup.item.id), Value::Bool(true));
                        self.kvl_context.set_var("last_picked_item", Value::String(pickup.item.id.clone()));

                        for graph in self.kvl_graphs.values() {
                            if let Ok(steps) = Interpreter::trigger_event(graph, &mut self.kvl_context, "OnItemPickup") {
                                kvl_steps_this_frame += steps;
                            }
                        }

                        self.status_message = format!("Objet obtenu : {} (+{}) !", pickup.item.name, pickup.count - rem);
                    }
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

                        // Impulsion de recul (Knockback) pour séparer les personnages et éliminer tout blocage mutuel
                        if let Some((ex, ey)) = contact_enemy_pos {
                            let kx = p_cx - ex;
                            let ky = p_cy - ey;
                            let k_dist = (kx * kx + ky * ky).sqrt().max(1.0);
                            let push_dist = 30.0;
                            let new_px = (self.entities[p_idx].transform.x + (kx / k_dist) * push_dist).clamp(32.0, 1150.0);
                            let new_py = (self.entities[p_idx].transform.y + (ky / k_dist) * push_dist).clamp(75.0, 580.0);
                            self.entities[p_idx].transform.x = new_px;
                            self.entities[p_idx].transform.y = new_py;
                        }
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

                    // Interaction PNJ avec DialogueComponent (cooldown protecteur pour éviter toute réouverture immédiate)
                    if !self.dialogue.is_active && self.dialogue_cooldown <= 0.0 {
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
                                self.dialogue_cooldown = 0.5;
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

                                if let Some(ref mut nav) = ent.nav_agent {
                                    nav.mode = crate::navigation::NavMode::ChasePlayer;
                                }

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
                            if let Some(ref mut nav) = ent.nav_agent {
                                nav.mode = crate::navigation::NavMode::Idle;
                                nav.waypoints.clear();
                            }
                            self.kvl_context.set_var("alarm_active", Value::Bool(false));
                            self.status_message = format!("L'entité '{}' a perdu votre trace. Alerte levée.", ent.name);
                        }
                    }
                }
            }
        }

        // Synchronisation des ordres de navigation KVL vers les entités dotées de NavAgent
        if let Some(Value::String(mode_str)) = self.kvl_context.get_var("nav_mode") {
            for ent in &mut self.entities {
                if let Some(ref mut nav) = ent.nav_agent {
                    if mode_str == "ChasePlayer" {
                        nav.mode = crate::navigation::NavMode::ChasePlayer;
                    } else if mode_str == "Idle" {
                        nav.mode = crate::navigation::NavMode::Idle;
                        nav.waypoints.clear();
                    } else if mode_str.starts_with("GoTo:") {
                        let coords = mode_str.trim_start_matches("GoTo:");
                        let mut parts = coords.split(',');
                        let tx = parts.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                        let ty = parts.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                        nav.mode = crate::navigation::NavMode::GoTo { x: tx, y: ty };
                        nav.target_pos = Some((tx as f32, ty as f32));
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

        // Le temps complet de frame (update + render + transport) est mesuré côté
        // `NativeEngine::render_frame` puis injecté via `finish_frame_telemetry`.
        self.last_kvl_steps = kvl_steps_this_frame;
    }

    /// Enregistre la télémétrie d'une frame complète et déclenche l'adaptation
    /// de qualité sur la base du temps CPU total (pas du seul temps d'update).
    pub fn finish_frame_telemetry(&mut self, total_us: u64) {
        self.telemetry.record_frame(total_us, self.last_kvl_steps);
        if let Some(msg) = self.adaptation.evaluate_adaptation(total_us) {
            self.status_message = msg.into();
        }
    }

    pub fn render(&mut self, canvas: &mut Canvas) {
        canvas.clear(0x00080B14);

        let is_edit_mode = self.editor.mode == EditorMode::Edit;
        let is_embedded = self.editor.embedded_mode;
        let w = canvas.width as i32;
        let h = canvas.height as i32;

        let header_h = if is_embedded { 0 } else { 44 };

        // Hauteur de la barre inférieure escamotable / Console KVL
        let bottom_h = if is_embedded {
            0
        } else if is_edit_mode {
            160
        } else if self.editor.show_console_drawer {
            160
        } else {
            34
        };
        let bottom_y = h - bottom_h;

        // Géométrie réactive du Viewport
        let (vx, vy, vw, vh) = if is_embedded {
            (0, 0, w, h)
        } else if is_edit_mode {
            let insp_w = 380;
            let v_x = 14;
            let v_y = header_h + 8;
            let v_w = (w - insp_w - 36).max(300);
            let v_h = (bottom_y - v_y - 8).max(200);
            (v_x, v_y, v_w, v_h)
        } else {
            // Mode Plein Écran Immersif
            (0, header_h, w, bottom_y - header_h)
        };

        // Réinitialisation des hitboxes UI de l'en-tête et des outils
        self.editor.play_tab_hitbox = None;
        self.editor.studio_tab_hitbox = None;
        self.editor.mode_toggle_btn_hitbox = None;
        self.editor.genre_btn_hitbox = None;
        self.editor.radar_btn_hitbox = None;
        self.editor.grid_btn_hitbox = None;
        self.editor.telemetry_btn_hitbox = None;
        self.editor.audio_btn_hitbox = None;
        self.editor.reset_btn_hitbox = None;
        self.editor.help_btn_hitbox = None;
        self.editor.console_toggle_btn_hitbox = None;

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
            self.editor.tool_select_btn_hitbox = None;
            self.editor.tool_brush_btn_hitbox = None;
            self.editor.tool_eraser_btn_hitbox = None;
            self.editor.tool_tile_cycle_btn_hitbox = None;
            self.editor.guided_btn_hitbox = None;
            self.editor.undo_btn_hitbox = None;
            self.editor.redo_btn_hitbox = None;
        }

        // Genre déduit des composants réels
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

        // ==========================================
        // 1. EN-TÊTE MASTER UNIFIÉ (y = 0..44, Hauteur 44px)
        // ==========================================
        if !is_embedded {
            let header_bg = if is_edit_mode { 0x00130E26 } else { 0x000B0F19 };
        canvas.draw_rect(0, 0, w as u32, header_h as u32, header_bg);
        canvas.draw_line(0, header_h - 1, w, header_h - 1, if is_edit_mode { 0x007C3AED } else { 0x001E293B });

        let mx = self.editor.mouse_x as i32;
        let my = self.editor.mouse_y as i32;
        self.editor.active_tooltip = None;

        let is_hov = |bx: i32, by: i32, bw: u32, bh: u32| -> bool {
            mx >= bx && mx <= (bx + bw as i32) && my >= by && my <= (by + bh as i32)
        };

        // Marque KaractiRage & Badge
        canvas.draw_badge(14, 7, 28, 28, 0x002E1065, 0x00A855F7, "KR", 0x00F3E8FF, 2);
        canvas.draw_text(48, 16, "KARACTIRAGE", 0x00CBD5E1, 1);

        // Bouton [PROJETS] (Hub Accueil 1-Clic, accessible F2)
        let hub_w = 70;
        let hub_x = 126;
        let hub_hov = is_hov(hub_x, 8, hub_w, 28);
        if hub_hov {
            self.editor.active_tooltip = Some(("Hub de Projets".into(), "F2".into(), "Ouvrir l'écran d'accueil pour créer ou charger un projet / scène.".into(), mx, my));
        }
        canvas.draw_badge_interactive(hub_x, 8, hub_w, 28, 0x0017122E, 0x00A855F7, "PROJETS", 0x00F3E8FF, 1, hub_hov);
        self.editor.hub_btn_hitbox = Some((hub_x, 8, hub_w, 28));

        // Segmented Control Pill [ ▶ JEU | ★ STUDIO ]
        let seg_w = 136;
        let seg_x = 202;
        let seg_y = 7;
        let seg_h = 30;
        canvas.draw_rect(seg_x, seg_y, seg_w as u32, seg_h as u32, 0x000F172A);
        canvas.draw_rect_outline(seg_x, seg_y, seg_w as u32, seg_h as u32, 0x00334155, 1);

        let (play_bg, play_bd, play_fg) = if !is_edit_mode {
            (0x00064E3B, 0x0010B981, 0x00D1FAE5)
        } else {
            (0x000F172A, 0x001E293B, 0x0064748B)
        };
        let play_hov = is_hov(seg_x + 2, seg_y + 2, 64, 26);
        if play_hov {
            self.editor.active_tooltip = Some(("Mode Jeu".into(), "TAB".into(), "Basculer en exécution plein écran pour tester le gameplay et les contrôles.".into(), mx, my));
        }
        canvas.draw_badge_interactive(seg_x + 2, seg_y + 2, 64, 26, play_bg, play_bd, "▶ JEU", play_fg, 1, play_hov);
        self.editor.play_tab_hitbox = Some((seg_x + 2, seg_y + 2, 64, 26));

        let (studio_bg, studio_bd, studio_fg) = if is_edit_mode {
            (0x002E1065, 0x00A855F7, 0x00F3E8FF)
        } else {
            (0x000F172A, 0x001E293B, 0x0064748B)
        };
        let studio_hov = is_hov(seg_x + 68, seg_y + 2, 66, 26);
        if studio_hov {
            self.editor.active_tooltip = Some(("Mode Studio".into(), "TAB".into(), "Éditeur de niveau temps réel : inspecteur d'entités, logique KVL et dessin.".into(), mx, my));
        }
        canvas.draw_badge_interactive(seg_x + 68, seg_y + 2, 66, 26, studio_bg, studio_bd, "★ STUDIO", studio_fg, 1, studio_hov);
        self.editor.studio_tab_hitbox = Some((seg_x + 68, seg_y + 2, 66, 26));

        self.editor.mode_toggle_btn_hitbox = Some((seg_x, seg_y, seg_w as u32, seg_h as u32));

        // Outils et Chip Scène à gauche du bandeau
        let mut lx = seg_x + seg_w + 8;
        if !is_edit_mode {
            let scene_label = format!("● {}", self.scene_name);
            let scene_w = (canvas.measure_text(&scene_label, 1).0 as u32 + 18).max(96);
            let sc_hov = is_hov(lx, 8, scene_w, 28);
            if sc_hov {
                self.editor.active_tooltip = Some(("Changer de Genre".into(), "F4 / L".into(), "Cycle instantané : RPG Top-Down, Plateforme 2D, Shoot'em Up, Sokoban.".into(), mx, my));
            }
            canvas.draw_badge_interactive(lx, 8, scene_w, 28, 0x00131C2E, 0x0038BDF8, &scene_label, 0x007DD3FC, 1, sc_hov);
            self.editor.genre_btn_hitbox = Some((lx, 8, scene_w, 28));
        } else {
            self.editor.genre_btn_hitbox = None;

            // [V: SÉL]
            let sel_bg = if self.editor.active_tool == StudioTool::Select { 0x001E3A8A } else { 0x000F172A };
            let sel_bd = if self.editor.active_tool == StudioTool::Select { 0x003B82F6 } else { 0x00334155 };
            let sel_fg = if self.editor.active_tool == StudioTool::Select { 0x00EFF6FF } else { 0x0094A3B8 };
            let sel_hov = is_hov(lx, 8, 34, 28);
            if sel_hov {
                self.editor.active_tooltip = Some(("Outil Sélection".into(), "V".into(), "Sélectionner et déplacer les entités par glisser-déposer.".into(), mx, my));
            }
            canvas.draw_badge_interactive(lx, 8, 34, 28, sel_bg, sel_bd, "V", sel_fg, 1, sel_hov);
            self.editor.tool_select_btn_hitbox = Some((lx, 8, 34, 28));
            lx += 38;

            // [B: PINCEAU]
            let bru_bg = if self.editor.active_tool == StudioTool::Brush { 0x007C2D12 } else { 0x000F172A };
            let bru_bd = if self.editor.active_tool == StudioTool::Brush { 0x00F97316 } else { 0x00334155 };
            let bru_fg = if self.editor.active_tool == StudioTool::Brush { 0x00FFEDD5 } else { 0x0094A3B8 };
            let bru_hov = is_hov(lx, 8, 34, 28);
            if bru_hov {
                self.editor.active_tooltip = Some(("Outil Pinceau".into(), "B".into(), "Peindre des tuiles de décor en maintenant le clic gauche.".into(), mx, my));
            }
            canvas.draw_badge_interactive(lx, 8, 34, 28, bru_bg, bru_bd, "B", bru_fg, 1, bru_hov);
            self.editor.tool_brush_btn_hitbox = Some((lx, 8, 34, 28));
            lx += 38;

            // [T: TUILE]
            let tile_col = self.editor.brush_tile.color();
            let t_hov = is_hov(lx, 8, 34, 28);
            if t_hov {
                self.editor.active_tooltip = Some((format!("Matériau: {}", self.editor.brush_tile.label()), "T".into(), "Changer le matériau de dessin (Mur, Eau, Glace, Lave).".into(), mx, my));
            }
            canvas.draw_badge_interactive(lx, 8, 34, 28, 0x0018181B, tile_col, "T", tile_col, 1, t_hov);
            self.editor.tool_tile_cycle_btn_hitbox = Some((lx, 8, 34, 28));
            lx += 38;

            // [X: GOMME]
            let era_bg = if self.editor.active_tool == StudioTool::Eraser { 0x007F1D1D } else { 0x000F172A };
            let era_bd = if self.editor.active_tool == StudioTool::Eraser { 0x00EF4444 } else { 0x00334155 };
            let era_fg = if self.editor.active_tool == StudioTool::Eraser { 0x00FEE2E2 } else { 0x0094A3B8 };
            let era_hov = is_hov(lx, 8, 34, 28);
            if era_hov {
                self.editor.active_tooltip = Some(("Outil Gomme".into(), "X".into(), "Effacer des blocs ou tuiles de décor sur la grille.".into(), mx, my));
            }
            canvas.draw_badge_interactive(lx, 8, 34, 28, era_bg, era_bd, "X", era_fg, 1, era_hov);
            self.editor.tool_eraser_btn_hitbox = Some((lx, 8, 34, 28));
        }

        // Right Contextual Toolbar
        let mut rx = w - 14;

        // [? AIDE]
        rx -= 52;
        let help_bg = if self.editor.show_help_modal { 0x004F46E5 } else { 0x001E1B4B };
        let h_hov = is_hov(rx, 8, 52, 28);
        if h_hov {
            self.editor.active_tooltip = Some(("Guide Pédagogique".into(), "H".into(), "Ouvrir le guide interactif complet en 3 onglets (initiation, touches, concepts).".into(), mx, my));
        }
        canvas.draw_badge_interactive(rx, 8, 52, 28, help_bg, 0x00818CF8, "? AIDE", 0x00E0E7FF, 1, h_hov);
        self.editor.help_btn_hitbox = Some((rx, 8, 52, 28));
        rx -= 4;

        // [SON / AUDIO]
        rx -= 46;
        let (audio_txt, audio_col) = if !self.audio.is_available {
            ("N/D", 0x0094A3B8)
        } else if self.audio.is_muted() {
            ("MUTE", 0x00EF4444)
        } else {
            ("SON", 0x0010B981)
        };
        let a_hov = is_hov(rx, 8, 46, 28);
        if a_hov {
            self.editor.active_tooltip = Some(("Contrôle Audio".into(), "M".into(), "Activer ou couper les bruitages et effets sonores synthétisés.".into(), mx, my));
        }
        canvas.draw_badge_interactive(rx, 8, 46, 28, 0x00131C2E, audio_col, audio_txt, audio_col, 1, a_hov);
        self.editor.audio_btn_hitbox = Some((rx, 8, 46, 28));
        rx -= 4;

        // [EXPORT] (Universel : Accessible en Mode Jeu et en Mode Studio)
        rx -= 62;
        let exp_hov = is_hov(rx, 8, 62, 28);
        if exp_hov {
            self.editor.active_tooltip = Some(("Exporter le Jeu".into(), "U".into(), "Packager un fichier HTML5 standalone ou bundle distribuable en 1 clic.".into(), mx, my));
        }
        canvas.draw_badge_interactive(rx, 8, 62, 28, 0x00082F49, 0x000284C7, "EXPORT", 0x00BAE6FD, 1, exp_hov);
        self.editor.export_btn_hitbox = Some((rx, 8, 62, 28));
        rx -= 4;

        // [RECETTE] (Universel : Accessible en Mode Jeu et en Mode Studio via W ou Clic)
        rx -= 66;
        let rec_hov = is_hov(rx, 8, 66, 28);
        if rec_hov {
            self.editor.active_tooltip = Some(("Assistant Recette de Jeu".into(), "W".into(), "Créer ou générer un jeu complet en 6 étapes (Platformer, Shmup, Donjon, Sokoban, Titan).".into(), mx, my));
        }
        let rec_bg = if self.editor.recipe_wizard.is_open { 0x003B0764 } else { 0x0017122E };
        let rec_border = if self.editor.recipe_wizard.is_open { 0x00C084FC } else { 0x00A855F7 };
        canvas.draw_badge_interactive(rx, 8, 66, 28, rec_bg, rec_border, "RECETTE", 0x00F3E8FF, 1, rec_hov);
        self.editor.recipe_btn_hitbox = Some((rx, 8, 66, 28));
        rx -= 4;

        if !is_edit_mode {
            // [R: RESET]
            rx -= 64;
            let r_hov = is_hov(rx, 8, 64, 28);
            if r_hov {
                self.editor.active_tooltip = Some(("Réinitialiser Scène".into(), "R".into(), "Recharger la scène depuis le fichier JSON d'origine.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 64, 28, 0x000F172A, 0x0064748B, "RESET", 0x00CBD5E1, 1, r_hov);
            self.editor.reset_btn_hitbox = Some((rx, 8, 64, 28));
            rx -= 6;

            // [F1: FPS]
            rx -= 58;
            let tel_col = if self.telemetry.show_overlay { 0x00F59E0B } else { 0x0064748B };
            let f1_hov = is_hov(rx, 8, 58, 28);
            if f1_hov {
                self.editor.active_tooltip = Some(("Télémétrie & FPS".into(), "F1".into(), "Afficher l'overlay d'analyse des temps de frame et budget 60 FPS.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 58, 28, 0x000F172A, tel_col, "F1: FPS", tel_col, 1, f1_hov);
            self.editor.telemetry_btn_hitbox = Some((rx, 8, 58, 28));
            rx -= 6;

            // [N: RADAR]
            rx -= 64;
            let rad_col = if self.show_minimap { 0x0010B981 } else { 0x0064748B };
            let n_hov = is_hov(rx, 8, 64, 28);
            if n_hov {
                self.editor.active_tooltip = Some(("Minimap & Radar".into(), "N".into(), "Afficher la carte tactique en surimpression pour repérer les sentinelles.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 64, 28, 0x000F172A, rad_col, "RADAR", rad_col, 1, n_hov);
            self.editor.radar_btn_hitbox = Some((rx, 8, 64, 28));
            rx -= 6;

            // Genre Badge
            rx -= 110;
            canvas.draw_badge(rx, 8, 110, 28, 0x001E293B, genre_bd, genre_badge, genre_col, 1);

            // Replay Status Badge (en mode Jeu)
            if self.replay.mode == ReplayMode::Recording {
                rx -= 6;
                rx -= 72;
                let blink = (self.elapsed_time * 8.0).sin() > 0.0;
                let rec_col = if blink { 0x00EF4444 } else { 0x00991B1B };
                canvas.draw_badge(rx, 8, 72, 28, 0x00450A0A, rec_col, "● REC", 0x00FEE2E2, 1);
            } else if self.replay.mode == ReplayMode::Playing {
                rx -= 6;
                rx -= 72;
                canvas.draw_badge(rx, 8, 72, 28, 0x00083344, 0x0006B6D4, "▶ REPLAY", 0x00CFFAFE, 1);
            }
        } else {
            // [SAUVER]
            rx -= 58;
            let s_hov = is_hov(rx, 8, 58, 28);
            if s_hov {
                self.editor.active_tooltip = Some(("Sauvegarder Scène".into(), "Ctrl+S".into(), "Enregistrer les entités et graphes KVL sur le disque local.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 58, 28, 0x00064E3B, 0x0010B981, "SAUVER", 0x00D1FAE5, 1, s_hov);
            self.editor.top_save_btn_hitbox = Some((rx, 8, 58, 28));
            rx -= 4;

            // [REDO (Ctrl+Y)]
            rx -= 38;
            let can_r = self.editor.undo_stack.can_redo();
            let (r_bg, r_bd, r_fg) = if can_r { (0x001E293B, 0x00A855F7, 0x00E9D5FF) } else { (0x000F172A, 0x001E293B, 0x00475569) };
            let r_hov = is_hov(rx, 8, 38, 28);
            if r_hov {
                self.editor.active_tooltip = Some(("Rétablir Action".into(), "Ctrl+Y".into(), "Rétablir la dernière opération annulée.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 38, 28, r_bg, r_bd, "REDO", r_fg, 1, r_hov);
            self.editor.redo_btn_hitbox = Some((rx, 8, 38, 28));
            rx -= 4;

            // [UNDO (Ctrl+Z)]
            rx -= 38;
            let can_u = self.editor.undo_stack.can_undo();
            let (u_bg, u_bd, u_fg) = if can_u { (0x001E293B, 0x0038BDF8, 0x00BAE6FD) } else { (0x000F172A, 0x001E293B, 0x00475569) };
            let u_hov = is_hov(rx, 8, 38, 28);
            if u_hov {
                self.editor.active_tooltip = Some(("Annuler Action".into(), "Ctrl+Z".into(), "Annuler la dernière opération d'édition dans le Studio.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 38, 28, u_bg, u_bd, "UNDO", u_fg, 1, u_hov);
            self.editor.undo_btn_hitbox = Some((rx, 8, 38, 28));
            rx -= 4;

            // [+ ENTITÉ]
            rx -= 64;
            let sp_bg = if self.editor.show_spawn_palette { 0x007C3AED } else { 0x003730A3 };
            let ent_hov = is_hov(rx, 8, 64, 28);
            if ent_hov {
                self.editor.active_tooltip = Some(("Créer une Entité".into(), "P".into(), "Ouvrir la palette pour insérer monstres, portes, clés ou pièges.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 64, 28, sp_bg, 0x00818CF8, "+ ENTITÉ", 0x00FFFFFF, 1, ent_hov);
            self.editor.spawn_btn_hitbox = Some((rx, 8, 64, 28));
            rx -= 4;

            // [GUIDE (G)]
            rx -= 48;
            let wiz_bg = if self.editor.wizard.is_open { 0x0078350F } else { 0x001E1B4B };
            let wiz_bd = if self.editor.wizard.is_open { 0x00F59E0B } else { 0x00818CF8 };
            let wiz_fg = if self.editor.wizard.is_open { 0x00FEF3C7 } else { 0x00C7D2FE };
            let wiz_hov = is_hov(rx, 8, 48, 28);
            if wiz_hov {
                self.editor.active_tooltip = Some(("Assistant Guidé KVL".into(), "G".into(), "Créer des mécaniques de jeu en 3 étapes guidées sans code.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 48, 28, wiz_bg, wiz_bd, "GUIDE", wiz_fg, 1, wiz_hov);
            self.editor.guided_btn_hitbox = Some((rx, 8, 48, 28));
            rx -= 4;

            // [⚡ IA] (Phase A)
            rx -= 46;
            let ia_bg = if self.editor.show_intent_bar { 0x00581C87 } else { 0x001E1B4B };
            let ia_bd = if self.editor.show_intent_bar { 0x00C084FC } else { 0x00A855F7 };
            let ia_hov = is_hov(rx, 8, 46, 28);
            if ia_hov {
                self.editor.active_tooltip = Some(("Assistant IA Intent".into(), "I".into(), "Formuler une intention textuelle pour pré-remplir le KVL Wizard.".into(), mx, my));
            }
            canvas.draw_badge_interactive(rx, 8, 46, 28, ia_bg, ia_bd, "⚡ IA", 0x00F3E8FF, 1, ia_hov);
            self.editor.intent_bar_btn_hitbox = Some((rx, 8, 46, 28));
            rx -= 4;

            if let SelectedEntity::Entity(idx) = self.editor.selected {
                if idx < self.entities.len() {
                    let is_player = self.entities[idx].movement.as_ref().map_or(false, |m| m.is_player_controlled);
                    if !is_player {
                        rx -= 52;
                        let del_hov = is_hov(rx, 8, 52, 28);
                        if del_hov {
                            self.editor.active_tooltip = Some(("Supprimer Entité".into(), "Suppr".into(), "Retirer définitivement l'entité sélectionnée de la scène.".into(), mx, my));
                        }
                        canvas.draw_badge_interactive(rx, 8, 52, 28, 0x007F1D1D, 0x00EF4444, "SUPPR", 0x00FEE2E2, 1, del_hov);
                        self.editor.delete_btn_hitbox = Some((rx, 8, 52, 28));
                        rx -= 4;
                    }
                    rx -= 46;
                    let dup_hov = is_hov(rx, 8, 46, 28);
                    if dup_hov {
                        self.editor.active_tooltip = Some(("Dupliquer Entité".into(), "Ctrl+D".into(), "Créer un clone exact de l'entité sélectionnée avec ses composants.".into(), mx, my));
                    }
                    canvas.draw_badge_interactive(rx, 8, 46, 28, 0x0078350F, 0x00F59E0B, "DUP", 0x00FEF3C7, 1, dup_hov);
                    self.editor.duplicate_btn_hitbox = Some((rx, 8, 46, 28));
                }
            }
        }
        }

        // ==========================================
        // 2. ZONE DE JEU (VIEWPORT)
        // ==========================================
        self.camera.set_viewport(vx, vy, vw as u32, vh as u32);
        canvas.set_clip_rect(Some((vx, vy, vw as u32, vh as u32)));

        // Sol texturé sci-fi dungeon défilant
        canvas.draw_rect(vx, vy, vw as u32, vh as u32, 0x000D1525);

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

        // Rivets discrets aux carrefours
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

        if is_edit_mode {
            let frame_border = 0x004C1D95;
            let bracket_col = 0x00A855F7;
            canvas.draw_rect_outline(vx, vy, vw as u32, vh as u32, frame_border, 2);
            canvas.draw_corner_brackets(vx, vy, vw as u32, vh as u32, 16, bracket_col);
        }

        // Légende discrète d'outil actif dans le viewport (en mode Studio uniquement)
        if is_edit_mode {
            let viewport_tag = match self.editor.active_tool {
                StudioTool::Select => "STUDIO [SÉLECTION] : CLIQUEZ/GLISSEZ | FLÈCHES: DÉFILEMENT | C: RECENTRER | V / B / X",
                StudioTool::Brush => "STUDIO [PINCEAU] : CLIC GAUCHE GLISSÉ POUR PEINDRE | T: CHANGER TUILE | CLIC DROIT: EFFACER",
                StudioTool::Eraser => "STUDIO [GOMME] : CLIC GAUCHE GLISSÉ POUR EFFACER LES BLOCS | V: SELECT | B: PINCEAU",
            };
            canvas.draw_text(vx + 16, vy + 12, viewport_tag, 0x0064748B, 1);
        }

        // Glass HUD Pill en haut du viewport (en mode jeu)
        if !is_edit_mode {
            let score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };

            let obj_txt = if let Some(Value::String(s)) = self.kvl_context.get_var("objective") {
                s.as_str()
            } else if let Some(ref s_obj) = self.scene_objective {
                s_obj.as_str()
            } else {
                match player_mode {
                    MovementMode::Platformer { .. } => "ATTEIGNEZ LE PORTAIL QUANTIQUE",
                    MovementMode::SpaceInertia { .. } => "PILOTEZ LE VAISSEAU SPATIAL",
                    MovementMode::Grid { .. } => "DÉPLACEZ LES BLOCS SUR LA DALLE",
                    MovementMode::TopDown if has_pushables => "POUSSEZ LES BLOCS SUR LES DALLES",
                    MovementMode::TopDown => {
                        if score < 50 {
                            "COLLECTEZ 50 OR POUR LA PORTE"
                        } else {
                            "PORTE DÉVERROUILLÉE ! SORTEZ DU DONJON"
                        }
                    }
                }
            };

            let (cur_hp, max_hp) = if let Some(p_idx) = self.player_index() {
                if let Some(ref h) = self.entities[p_idx].health {
                    (h.current.ceil() as i32, h.max.ceil() as i32)
                } else {
                    (3, 3)
                }
            } else {
                (3, 3)
            };
            let hearts_count = max_hp.clamp(1, 10);

            let hearts_w = hearts_count * 16 + 8;
            let score_str = format!("🪙 {} OR", score);
            let score_w = canvas.measure_text(&score_str, 1).0 as i32 + 16;
            let obj_prefix = "🎯 ";
            let full_obj_str = format!("{}{}", obj_prefix, obj_txt);
            let obj_w = canvas.measure_text(&full_obj_str, 1).0 as i32 + 20;

            let has_boss = self.entities.iter().any(|e| e.is_active && e.boss.is_some());
            let hud_pill_w = if has_boss {
                (hearts_w + score_w + 22) as u32
            } else {
                (hearts_w + score_w + obj_w + 32) as u32
            };
            let hud_x = 18;
            let hud_y = vy + 12;
            let hud_h = 34u32;

            canvas.draw_rect(hud_x, hud_y, hud_pill_w, hud_h, 0x000F172A);
            canvas.draw_rect_outline(hud_x, hud_y, hud_pill_w, hud_h, 0x00334155, 1);
            canvas.draw_corner_brackets(hud_x, hud_y, hud_pill_w, hud_h, 8, 0x0038BDF8);

            let mut hx = hud_x + 10;
            for i in 0..hearts_count {
                let heart_icon = if i < cur_hp { "♥" } else { "♡" };
                let heart_col = if i < cur_hp { 0x00EF4444 } else { 0x00475569 };
                canvas.draw_text(hx, hud_y + 9, heart_icon, heart_col, 2);
                hx += 16;
            }

            hx += 6;
            canvas.draw_line(hx, hud_y + 6, hx, hud_y + (hud_h as i32) - 6, 0x00334155);
            hx += 10;

            canvas.draw_text(hx, hud_y + 10, &score_str, 0x00FBBF24, 1);
            if !has_boss {
                hx += score_w + 2;
                canvas.draw_line(hx, hud_y + 6, hx, hud_y + (hud_h as i32) - 6, 0x00334155);
                hx += 10;
                canvas.draw_text(hx, hud_y + 10, &full_obj_str, 0x006EE7B7, 1);
            }
        }

        // Bandeau Boss Health Bar & Alerte de Phase (centré en haut du viewport en mode jeu)
        if !is_edit_mode {
            if let Some(boss_ent) = self.entities.iter().find(|e| e.is_active && e.boss.is_some()) {
                if let (Some(ref boss), Some(ref health)) = (&boss_ent.boss, &boss_ent.health) {
                    let b_hud_w = 420;
                    let b_hud_h = 44;
                    let b_hud_x = vx + (vw - b_hud_w) / 2;
                    let b_hud_y = vy + 12;
                    let phase_col = boss.phase.color();

                    // Boîtier glassmorphism sombre
                    canvas.draw_rect(b_hud_x, b_hud_y, b_hud_w as u32, b_hud_h as u32, 0x000F172A);
                    canvas.draw_rect_outline(b_hud_x, b_hud_y, b_hud_w as u32, b_hud_h as u32, phase_col, 1);
                    canvas.draw_corner_brackets(b_hud_x, b_hud_y, b_hud_w as u32, b_hud_h as u32, 10, phase_col);

                    // Titre et phase
                    let boss_title = format!("{} [{}]", boss.name.to_uppercase(), boss.phase.label());
                    canvas.draw_text(b_hud_x + 12, b_hud_y + 6, &boss_title, 0x00F8FAFC, 1);

                    // Alerte d'attaque ou défaite
                    if let Some(t) = boss.telegraphs.iter().find(|t| t.is_active) {
                        let tele_msg = format!("! {} : {:.0}%", t.attack_type.label(), t.progress() * 100.0);
                        canvas.draw_text(b_hud_x + b_hud_w - 180, b_hud_y + 6, &tele_msg, 0x00FCA5A5, 1);
                    } else if boss.phase == BossPhase::Defeated {
                        canvas.draw_text(b_hud_x + b_hud_w - 110, b_hud_y + 6, "[VAINCU !]", 0x0010B981, 1);
                    }

                    // Jauge de PV segmentée en 3 blocs
                    let bar_x = b_hud_x + 12;
                    let bar_y = b_hud_y + 22;
                    let bar_w = b_hud_w - 24;
                    let bar_h = 12;
                    canvas.draw_rect(bar_x, bar_y, bar_w as u32, bar_h as u32, 0x001E293B);

                    let hp_ratio = (health.current / health.max).clamp(0.0, 1.0);
                    let fill_w = (bar_w as f32 * hp_ratio) as u32;
                    if fill_w > 0 {
                        canvas.draw_rect(bar_x, bar_y, fill_w, bar_h as u32, phase_col);
                        canvas.draw_line(bar_x, bar_y, bar_x + fill_w as i32 - 1, bar_y, 0x00FFFFFF);
                    }

                    // Graduations aux tiers 33% et 66%
                    let tier1_x = bar_x + (bar_w as f32 * 0.33) as i32;
                    let tier2_x = bar_x + (bar_w as f32 * 0.66) as i32;
                    canvas.draw_line(tier1_x, bar_y, tier1_x, bar_y + bar_h - 1, 0x000F172A);
                    canvas.draw_line(tier2_x, bar_y, tier2_x, bar_y + bar_h - 1, 0x000F172A);

                    canvas.draw_rect_outline(bar_x, bar_y, bar_w as u32, bar_h as u32, 0x00475569, 1);
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
        self.render_order.clear();
        self.render_order.extend(
            (0..self.entities.len())
                .filter(|&i| self.entities[i].is_active || is_edit_mode)
                .filter(|&i| self.entities[i].renderable.is_some()),
        );
        self.render_order.sort_unstable_by_key(|&i| {
            self.entities[i].renderable.as_ref().map_or(0, |r| r.z_index)
        });

        // Rendu immersif des zones d'avertissement au sol (Telegraphing vectoriel)
        for ent in &self.entities {
            if !ent.is_active {
                continue;
            }
            if let Some(ref boss) = ent.boss {
                for t in &boss.telegraphs {
                    if !t.is_active {
                        continue;
                    }
                    let (sx, sy) = self.camera.world_to_screen(t.x, t.y);
                    let pulse = ((self.elapsed_time * 14.0).sin() * 2.0) as i32;
                    let prog = t.progress();

                    match t.shape {
                        TelegraphShape::Circle { radius } => {
                            let r_i = radius as i32;
                            // Anneau extérieur pulsant d'avertissement
                            canvas.draw_circle(sx, sy, (r_i + pulse).max(4), t.color);
                            canvas.draw_circle(sx, sy, (r_i + pulse - 1).max(3), t.color);
                            // Progression de charge de l'attaque
                            let charge_r = ((radius * prog) as i32).max(2);
                            canvas.draw_circle(sx, sy, charge_r, t.color);

                            // Libellé d'alerte tactique
                            let pct = (prog * 100.0) as u32;
                            let alert_tag = format!("! {} ({}%)", t.attack_type.label(), pct);
                            canvas.draw_text(sx - 36, sy - r_i - 14, &alert_tag, 0x00FFFFFF, 1);
                        }
                        TelegraphShape::Line { length, thickness, angle } => {
                            let end_x = sx + (angle.cos() * length) as i32;
                            let end_y = sy + (angle.sin() * length) as i32;
                            canvas.draw_line(sx, sy, end_x, end_y, t.color);
                            let perp_x = (-angle.sin() * (thickness * 0.5)) as i32;
                            let perp_y = (angle.cos() * (thickness * 0.5)) as i32;
                            canvas.draw_line(sx + perp_x, sy + perp_y, end_x + perp_x, end_y + perp_y, t.color);
                            canvas.draw_line(sx - perp_x, sy - perp_y, end_x - perp_x, end_y - perp_y, t.color);
                            let pct = (prog * 100.0) as u32;
                            let alert_tag = format!("! {} ({}%)", t.attack_type.label(), pct);
                            canvas.draw_text(sx + 10, sy - 14, &alert_tag, 0x00FFFFFF, 1);
                        }
                        TelegraphShape::Rect { w, h } => {
                            let rw = (w as i32 + pulse).max(4) as u32;
                            let rh = (h as i32 + pulse).max(4) as u32;
                            canvas.draw_rect_outline(sx, sy, rw, rh, t.color, 2);
                        }
                    }
                }
            }
        }

        for &idx in &self.render_order {
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
                        "boss" => {
                            // Boss Titan Proto-Sentinelle imposant
                            let phase_color = ent.boss.as_ref().map_or(0x0006B6D4, |b| b.phase.color());
                            canvas.draw_rect(rx, ry, rw, rh, rend.color);
                            canvas.draw_rect_outline(rx, ry, rw, rh, phase_color, 3);
                            canvas.draw_corner_brackets(rx, ry, rw, rh, 16, phase_color);

                            // Biseau de blindage interne
                            if rw >= 32 && rh >= 32 {
                                canvas.draw_rect_outline(rx + 6, ry + 6, rw - 12, rh - 12, 0x000F172A, 2);
                            }

                            // Réacteurs / Conduits d'énergie aux 4 coins
                            let r_size = 10;
                            canvas.draw_rect(rx + 4, ry + 4, r_size, r_size, phase_color);
                            canvas.draw_rect(rx + rw as i32 - 14, ry + 4, r_size, r_size, phase_color);
                            canvas.draw_rect(rx + 4, ry + rh as i32 - 14, r_size, r_size, phase_color);
                            canvas.draw_rect(rx + rw as i32 - 14, ry + rh as i32 - 14, r_size, r_size, phase_color);

                            // Oeil / Cœur cyclopéen central réactif
                            let eye_cx = rx + (rw as i32 / 2);
                            let eye_cy = ry + (rh as i32 / 2);
                            let pulse = ((self.elapsed_time * 8.0).sin() * 2.0) as i32;
                            let core_r = (16 + pulse).max(10);
                            canvas.draw_circle(eye_cx, eye_cy, core_r, 0x000F172A);
                            canvas.draw_circle(eye_cx, eye_cy, core_r - 4, phase_color);
                            canvas.draw_circle(eye_cx, eye_cy, 4, 0x00FFFFFF);

                            // Cornes / Antennes émettrices de télégraphie
                            canvas.draw_line(rx + 12, ry, rx - 6, ry - 14, phase_color);
                            canvas.draw_line(rx + rw as i32 - 12, ry, rx + rw as i32 + 6, ry - 14, phase_color);
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

                    let is_ai_proposed = self.editor.pending_ai_patch.as_ref()
                        .map_or(false, |p| p.created_entities.iter().any(|ce| ce.id == ent.id));

                    if is_ai_proposed {
                        let pulse = ((self.elapsed_time * 8.0).sin() * 2.5) as i32;
                        let pad = 6 + pulse;
                        let hx = rx - pad;
                        let hy = ry - pad;
                        let hw = (rw as i32 + pad * 2).max(1) as u32;
                        let hh = (rh as i32 + pad * 2).max(1) as u32;
                        canvas.draw_rect_outline(hx, hy, hw, hh, 0x00A855F7, 2);
                        canvas.draw_corner_brackets(hx, hy, hw, hh, 8, 0x00E9D5FF);
                        canvas.draw_badge(rx, ry - 24, 115, 18, 0x002E1065, 0x00A855F7, "✨ IA: PROPOSE", 0x00F3E8FF, 1);
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

                    let is_ai_proposed = self.editor.pending_ai_patch.as_ref()
                        .map_or(false, |p| p.created_entities.iter().any(|ce| ce.id == ent.id));

                    if is_ai_proposed {
                        let pulse = ((self.elapsed_time * 8.0).sin() * 2.5) as i32;
                        let pad = 6 + pulse;
                        let sz = ((radius + pad as f32) * 2.0).max(1.0) as u32;
                        let bx = base_cx - radius as i32 - pad;
                        let by = base_cy - radius as i32 - pad;
                        canvas.draw_rect_outline(bx, by, sz, sz, 0x00A855F7, 2);
                        canvas.draw_corner_brackets(bx, by, sz, sz, 8, 0x00E9D5FF);
                        canvas.draw_badge(base_cx - 40, by - 24, 115, 18, 0x002E1065, 0x00A855F7, "✨ IA: PROPOSE", 0x00F3E8FF, 1);
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
                ShapeKind::PixelMatrix { width, height, ref pixels } => {
                    let dest_w = ent.transform.w as u32;
                    let dest_h = ent.transform.h as u32;
                    // Ombre portée subtile au sol
                    if ent.tag != "floor" && ent.tag != "platform" {
                        canvas.draw_rect(rx + 2, ry + 3, dest_w, dest_h, 0x00060911);
                    }
                    // Rendu du sprite pixel-art à l'échelle
                    canvas.draw_pixel_matrix(rx, ry, dest_w, dest_h, pixels, width, height);
                    if let Some(outline_col) = rend.outline_color {
                        canvas.draw_rect_outline(rx, ry, dest_w, dest_h, outline_col, rend.outline_width);
                    }
                    if is_selected {
                        canvas.draw_rect_outline(rx - 2, ry - 2, dest_w + 4, dest_h + 4, 0x00FBBF24, 2);
                        let label = format!("{}: {}", ent.tag.to_uppercase(), ent.name);
                        canvas.draw_text(rx, ry - 14, &label, 0x00FBBF24, 1);
                    }
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

            // Rendu des chemins A* pour les agents autonomes intelligents et navigation joueur
            if let Some(ref nav) = ent.nav_agent {
                if nav.has_path() && (ent.tag == "player" || self.show_minimap || is_edit_mode || ent.vision.as_ref().map_or(false, |v| v.is_alerted)) {
                    let (ex, ey) = self.camera.world_to_screen(ent.transform.center_x(), ent.transform.center_y());
                    let mut prev_screen = (ex, ey);

                    for idx in nav.current_waypoint_idx..nav.waypoints.len() {
                        let (wx, wy) = nav.waypoints[idx];
                        let (sx, sy) = self.camera.world_to_screen(wx, wy);
                        let line_col = if ent.tag == "player" {
                            0x0038BDF8 // Cyan lumineux pour le joueur
                        } else if idx == nav.current_waypoint_idx {
                            0x00F59E0B
                        } else {
                            0x0006B6D4
                        };
                        canvas.draw_line(prev_screen.0, prev_screen.1, sx, sy, line_col);
                        canvas.draw_rect(sx - 2, sy - 2, 4, 4, if ent.tag == "player" { 0x0038BDF8 } else { 0x0038BDF8 });
                        prev_screen = (sx, sy);
                    }

                    // Cible finale
                    if let Some(tgt) = nav.target_pos {
                        let (tsx, tsy) = self.camera.world_to_screen(tgt.0, tgt.1);
                        canvas.draw_rect_outline(tsx - 4, tsy - 4, 8, 8, if ent.tag == "player" { 0x0038BDF8 } else { 0x00EF4444 }, 1);
                    }
                }
            }

            // Gizmos de débogage pour les membres de meute / essaims (Boids)
            if let Some(ref flock) = ent.flock {
                if is_edit_mode || self.show_minimap {
                    let (ex, ey) = self.camera.world_to_screen(ent.transform.center_x(), ent.transform.center_y());
                    if let Some(ref mov) = ent.movement {
                        let vx = (mov.vx * 0.15) as i32;
                        let vy = (mov.vy * 0.15) as i32;
                        canvas.draw_line(ex, ey, ex + vx, ey + vy, 0x00FDE047);
                    }
                    if flock.is_scattered() {
                        canvas.draw_text(ex - 14, ey - 16, "!SCATTER!", 0x00EF4444, 1);
                    } else if flock.role == crate::flocking::FlockRole::Leader {
                        canvas.draw_text(ex - 12, ey - 16, "[LEAD]", 0x00F59E0B, 1);
                    }
                }
            }

            // Prompt d'interaction contextuelle flottante pour PNJ (si le joueur est à proximité et dialogue inactif)
            if let Some(ref dlg) = ent.dialogue {
                if !self.dialogue.is_active {
                    if let Some(p_idx) = self.player_index() {
                        let p = &self.entities[p_idx];
                        let dx = p.transform.center_x() - ent.transform.center_x();
                        let dy = p.transform.center_y() - ent.transform.center_y();
                        if (dx * dx + dy * dy).sqrt() < dlg.interact_radius {
                            let (nx, ny) = self.camera.world_to_screen(ent.transform.center_x(), ent.transform.y);
                            let pulse = ((self.elapsed_time * 6.0).sin() * 2.0) as i32;
                            let prompt_lbl = format!("[E] Parler à {}", dlg.speaker);
                            let pw = (prompt_lbl.len() as u32 * 8) + 16;
                            canvas.draw_badge(nx - (pw as i32 / 2), ny - 24 + pulse, pw, 20, 0x000F172A, 0x0038BDF8, &prompt_lbl, 0x00E0F2FE, 1);
                        }
                    }
                }
            }

            // Prompt d'interaction contextuelle flottante pour entités avec script interactif (Portes, Coffres, etc.)
            if ent.dialogue.is_none() {
                if let Some(ref script) = ent.script {
                    if script.on_interact_event.is_some() {
                        if let Some(p_idx) = self.player_index() {
                            let p = &self.entities[p_idx];
                            let dx = p.transform.center_x() - ent.transform.center_x();
                            let dy = p.transform.center_y() - ent.transform.center_y();
                            if (dx * dx + dy * dy).sqrt() < 80.0 {
                                let (nx, ny) = self.camera.world_to_screen(ent.transform.center_x(), ent.transform.y);
                                let pulse = ((self.elapsed_time * 6.0).sin() * 2.0) as i32;
                                let prompt_lbl = if ent.tag == "door" { "[E] Ouvrir Porte" } else { "[E] Interagir" };
                                let pw = (prompt_lbl.len() as u32 * 8) + 16;
                                canvas.draw_badge(nx - (pw as i32 / 2), ny - 24 + pulse, pw, 20, 0x000F172A, 0x0010B981, prompt_lbl, 0x00D1FAE5, 1);
                            }
                        }
                    }
                }
            }
        }

        // Rendu du Joueur Fantôme (Ghost Player) pour comparaison / speedrunning
        if let Some((gx, gy)) = self.replay.current_ghost_position(self.sim_tick as usize) {
            if self.camera.is_visible(gx, gy, 24.0, 24.0) {
                let (gsx, gsy) = self.camera.world_to_screen(gx, gy);
                canvas.draw_rect_outline(gsx, gsy, 24, 24, 0x0006B6D4, 1);
                canvas.draw_text(gsx - 4, gsy - 12, "[GHOST]", 0x0067E8F9, 1);
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
            let mm_y = vy + 10;
            let mm_x = if is_embedded {
                if self.telemetry.show_overlay {
                    (w - 16 - 380) - mm_w - 14
                } else {
                    w - 16 - mm_w
                }
            } else if is_edit_mode {
                (w - 380 - 14) - mm_w - 14
            } else if self.telemetry.show_overlay {
                (w - 16 - 380) - mm_w - 14
            } else {
                w - 16 - mm_w
            };

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

        // Fin de la zone découpée du Viewport (pour les fenêtres et overlays hors champ)
        canvas.set_clip_rect(None);

        // ========================================================
        // PANNEAU INSPECTEUR KVL UNIVERSEL (EN MODE ÉDITION) : Largeur 380px
        // ========================================================
        if !is_embedded && is_edit_mode {
            let insp_w = 380;
            let insp_x = w - insp_w - 14;
            let insp_y = vy;
            let insp_h = vh;

            self.editor.pin_hitboxes.clear();
            self.editor.connection_hitboxes.clear();
            self.editor.palette_hitboxes.clear();
            self.editor.save_btn_hitbox = None;
            self.editor.add_node_btn_hitbox = None;
            self.editor.view_btn_hitbox = None;

            canvas.draw_rect(insp_x, insp_y, insp_w as u32, insp_h as u32, 0x000F172A);
            canvas.draw_rect_outline(insp_x, insp_y, insp_w as u32, insp_h as u32, 0x007C3AED, 2);
            canvas.draw_corner_brackets(insp_x, insp_y, insp_w as u32, insp_h as u32, 16, 0x00A855F7);

            canvas.draw_badge(insp_x + 18, insp_y + 14, (insp_w - 36) as u32, 28, 0x002E1065, 0x00A855F7, "INSPECTEUR D'OBJET & KVL", 0x00F3E8FF, 2);

            match self.editor.selected {
                SelectedEntity::None => {
                    // Carte pédagogique de bienvenue dans le studio
                    let card_y = insp_y + 54;
                    canvas.draw_badge(insp_x + 18, card_y, (insp_w - 36) as u32, 26, 0x001E293B, 0x0038BDF8, "GUIDE PÉDAGOGIQUE DU STUDIO", 0x00E0F2FE, 2);

                    let guide_steps = [
                        ("1. SÉLECTIONNER", "Cliquez sur une entité (ex: Joueur, Porte, Torche)", 0x0038BDF8),
                        ("2. DÉPLACER", "Glissez-déposez à la souris pour positionner l'entité", 0x0034D399),
                        ("3. COMPOSER ECS", "Ajoutez Collider2D, Movement, Vision ou Émetteur", 0x00FBBF24),
                        ("4. LOGIQUE KVL", "Éditez les nœuds visuels et reliez les broches In / Out", 0x00C084FC),
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
                    canvas.draw_text(insp_x + 18, sy + 38, "G: Créateur Guidé | P: Créer | C+D: Dupliquer", 0x00E2E8F0, 1);
                    canvas.draw_text(insp_x + 18, sy + 54, "Espace: Vue KVL | H: Guide complet | TAB: Mode Jeu", 0x0038BDF8, 1);

                    let wiz_badge_y = sy + 74;
                    let wiz_badge_w = (insp_w - 36) as u32;
                    canvas.draw_badge(insp_x + 18, wiz_badge_y, wiz_badge_w, 28, 0x001E1B4B, 0x00818CF8, "OUVRIR CREATEUR GUIDE (G)", 0x00C7D2FE, 1);
                    self.editor.add_comp_hitboxes.push(("OpenGuidedWizard".into(), insp_x + 18, wiz_badge_y, wiz_badge_w, 28));
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
                        if let Some(ref nav) = ent.nav_agent {
                            let nav_desc = format!("NavAgent A* : {} ({:.0}px/s, {} wps)", nav.mode.label(), nav.speed, nav.waypoints.len());
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x00083344, 0x0006B6D4, &nav_desc, 0x00CFFAFE, 1);
                            comp_y += 30;
                        }
                        if let Some(ref flock) = ent.flock {
                            let role_lbl = flock.role.label();
                            let status_lbl = if flock.is_scattered() { "PANIQUE" } else { "COHESION" };
                            let flock_desc = format!("Boid #{} : {} [{}] ({:.0}px/s)", flock.swarm_id, role_lbl, status_lbl, flock.max_speed);
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x001E1B4B, 0x00818CF8, &flock_desc, 0x00C7D2FE, 1);
                            comp_y += 30;
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
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x0038BDF8, "[ + MOVEMENT 2D (CINÉMATIQUE) ]", 0x0038BDF8, 2);
                            self.editor.add_comp_hitboxes.push(("Movement".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.vision.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00F87171, "[ + VISION SENSING (RADAR) ]", 0x00F87171, 2);
                            self.editor.add_comp_hitboxes.push(("Vision".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.nav_agent.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x0006B6D4, "[ + NAVIGATION AGENT (IA A*) ]", 0x0067E8F9, 2);
                            self.editor.add_comp_hitboxes.push(("Navigation".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.flock.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00818CF8, "[ + ESSAIM / MEUTE (BOIDS IA) ]", 0x00C7D2FE, 2);
                            self.editor.add_comp_hitboxes.push(("Flock".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.emitter.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00F97316, "[ + PARTICLE EMITTER (FLAMMES) ]", 0x00F97316, 2);
                            self.editor.add_comp_hitboxes.push(("Emitter".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.health.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00F43F5E, "[ + SANTÉ / VIE (HEALTH2D) ]", 0x00FDA4AF, 2);
                            self.editor.add_comp_hitboxes.push(("Health".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.script.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00A78BFA, "[ + SCRIPT KVL (LOGIQUE) ]", 0x00A78BFA, 2);
                            self.editor.add_comp_hitboxes.push(("Script".into(), insp_x + 18, comp_y, btn_w, btn_h));
                            comp_y += 34;
                        }
                        if ent.dialogue.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x0038BDF8, "[ + DIALOGUE PNJ & QUÊTE ]", 0x0038BDF8, 2);
                            self.editor.add_comp_hitboxes.push(("Dialogue".into(), insp_x + 18, comp_y, btn_w, btn_h));
                        }
                    }
                }
                _ => {}
            }
        }

        // Overlay de la palette de spawn d'entités (Touche P ou Bouton [+ SPAWN])
        if !is_embedded && is_edit_mode && self.editor.show_spawn_palette {
            let pal_w = 440;
            let pal_h = 648;
            let pal_x = 160;
            let pal_y = 20;

            canvas.draw_rect(pal_x, pal_y, pal_w as u32, pal_h as u32, 0x000F172A);
            canvas.draw_rect_outline(pal_x, pal_y, pal_w as u32, pal_h as u32, 0x0038BDF8, 2);
            canvas.draw_corner_brackets(pal_x, pal_y, pal_w as u32, pal_h as u32, 16, 0x0038BDF8);

            canvas.draw_rect(pal_x, pal_y, pal_w as u32, 34, 0x000284C7);
            canvas.draw_text(pal_x + 14, pal_y + 8, "STUDIO : CRÉER UNE NOUVELLE ENTITÉ", 0x00FFFFFF, 2);

            let spawn_templates = [
                ("wall", "+ Mur Solide AABB (Bloquant)", 0x0094A3B8),
                ("collectible", "+ Objet Collectible (Pièce d'Or KVL)", 0x00F59E0B),
                ("door", "+ Porte Interactive Sécurisée", 0x00EF4444),
                ("enemy", "+ Sentinelle Patrouilleuse (Vision)", 0x00DC2626),
                ("boss", "+ Boss Titan Proto-Sentinelle (Télégraphies)", 0x0006B6D4),
                ("hazard", "+ Piège à Piques (Dégâts -1 PV)", 0x00EF4444),
                ("heart", "+ Fiole de Cœur (Soin +1 PV)", 0x00EC4899),
                ("item_potion", "+ Objet : Potion de Soin (Hotbar)", 0x00EF4444),
                ("item_key", "+ Objet : Clé de Donjon (KVL)", 0x00F59E0B),
                ("item_crystal", "+ Objet : Cristal d'Énergie", 0x0038BDF8),
                ("portal", "+ Portail Quantique (Téléporteur)", 0x0006B6D4),
                ("torch", "+ Torche Brûlante (Émetteur Flammes)", 0x00F97316),
                ("crate", "+ Bloc Poussable (Puzzle / Sokoban)", 0x00F4AF6A),
                ("plate", "+ Dalle de Pression (Ouvre les portes)", 0x0034D399),
                ("npc", "+ PNJ / Gardien (Dialogues KVL & Quêtes)", 0x0038BDF8),
                ("cancel", "[ Fermer la palette (Échap) ]", 0x0064748B),
            ];

            for (idx, (tpl_id, tpl_name, col)) in spawn_templates.iter().enumerate() {
                let item_y = pal_y + 40 + (idx as i32 * 37);
                let item_w = pal_w - 24;
                let item_h = 30;

                canvas.draw_badge(pal_x + 12, item_y, item_w as u32, item_h as u32, 0x001E293B, *col, tpl_name, *col, 1);

                self.editor.spawn_palette_hitboxes.push(crate::editor::PaletteItemHitbox {
                    template_id: tpl_id.to_string(),
                    x: pal_x + 12,
                    y: item_y,
                    w: item_w as u32,
                    h: item_h as u32,
                });
            }
        }

        // 4. Bandeau inférieur / Tiroir KVL escamotable
        if !is_embedded {
            canvas.draw_rect(0, bottom_y, w as u32, bottom_h as u32, 0x00090D18);
            canvas.draw_line(0, bottom_y, w, bottom_y, 0x001F293D);

            // Ligne de statut du moteur
            canvas.draw_badge(14, bottom_y + 4, 82, 26, 0x00151D33, 0x007C3AED, "⚡ KVL", 0x00F3E8FF, 1);
            let status_col = if self.status_timer > 0.0 { 0x0038BDF8 } else { 0x0094A3B8 };
            canvas.draw_text_body(104, bottom_y + 8, &self.status_message, status_col, 13.5);
            canvas.draw_badge(w - 300, bottom_y + 4, 155, 26, 0x000F172A, 0x00334155, "60 FPS • DÉTERMINISTE", 0x0094A3B8, 1);

            // Bouton bascule Console KVL
            let (drawer_lbl, drawer_col) = if is_edit_mode || self.editor.show_console_drawer {
                ("▼ CONSOLE (~)", 0x00F87171)
            } else {
                ("▲ CONSOLE (~)", 0x0038BDF8)
            };
            canvas.draw_badge(w - 135, bottom_y + 4, 122, 26, 0x001E293B, drawer_col, drawer_lbl, drawer_col, 1);
            self.editor.console_toggle_btn_hitbox = Some((w - 135, bottom_y + 4, 122, 26));

            // Console de traçabilité d'exécution KVL (si dépliée ou mode studio)
            if is_edit_mode || self.editor.show_console_drawer {
                let console_y = bottom_y + 34;
                let console_h = (bottom_h - 40).max(40);
                canvas.draw_rect(14, console_y, (w - 28) as u32, console_h as u32, 0x00050810);
                canvas.draw_rect_outline(14, console_y, (w - 28) as u32, console_h as u32, 0x00162032, 1);

                let log_count = self.kvl_context.execution_log.len();
                let start_idx = log_count.saturating_sub(5);
                if self.kvl_context.execution_log.is_empty() {
                    canvas.draw_text_mono(24, console_y + 14, "> En attente d'événements KVL (déplacez-vous sur une pièce ou interagissez avec la porte)...", 0x00475569, 12.0);
                } else {
                    for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
                        let trace_line = format!("> [{}] {} : {}", step.node_id, step.node_name, step.message);
                        let col = if i % 2 == 0 { 0x00A7F3D0 } else { 0x006EE7B7 };
                        canvas.draw_text_mono(24, console_y + 14 + (i as i32 * 20), &trace_line, col, 12.0);
                    }
                }
            }
        }

        // Overlay Télémétrie en Mode Jeu (F1)
        if !is_edit_mode && self.telemetry.show_overlay {
            let ov_w = 380;
            let ov_h = 320;
            let ov_x = w - ov_w - 24;
            let ov_y = 56;

            canvas.draw_rect(ov_x, ov_y, ov_w as u32, ov_h as u32, 0x000A0F1D);
            canvas.draw_rect_outline(ov_x, ov_y, ov_w as u32, ov_h as u32, 0x0038BDF8, 2);
            canvas.draw_corner_brackets(ov_x, ov_y, ov_w as u32, ov_h as u32, 12, 0x0038BDF8);

            canvas.draw_text(ov_x + 16, ov_y + 14, "PROFILAGE & TÉLÉMÉTRIE (F1)", 0x0038BDF8, 2);

            let fps_line = format!("FPS : {:.1} (Cible : {:.0})", self.telemetry.current_fps, self.adaptation.target_fps);
            canvas.draw_text_mono(ov_x + 16, ov_y + 38, &fps_line, 0x00F8FAFC, 12.5);

            let cpu_line = format!("CPU Frame : {} us (Avg)", self.telemetry.avg_frame_time_us);
            canvas.draw_text_mono(ov_x + 16, ov_y + 58, &cpu_line, 0x0034D399, 12.5);

            let budget_status = if self.telemetry.avg_frame_time_us <= self.adaptation.target_budget_us {
                "BUDGET : RESPECTÉ (<16.6ms)"
            } else {
                "BUDGET : DÉPASSÉ !"
            };
            let budget_color = if self.telemetry.avg_frame_time_us <= self.adaptation.target_budget_us { 0x0010B981 } else { 0x00EF4444 };
            canvas.draw_text_mono(ov_x + 16, ov_y + 78, budget_status, budget_color, 12.5);

            let auto_str = if self.adaptation.auto_adapt { "AUTO-ADAPTATIF (ON)" } else { "MANUEL (OFF)" };
            canvas.draw_text_mono(ov_x + 16, ov_y + 98, auto_str, 0x00FCD34D, 12.5);

            let particles_line = format!("Particules : {} / {}", self.particles.len(), self.adaptation.current_profile.max_particles());
            canvas.draw_text_mono(ov_x + 16, ov_y + 118, &particles_line, 0x0094A3B8, 12.5);

            let spatial_line = format!("Grille Spatiale : {} req | {} cand", self.spatial_grid.stats_queries, self.spatial_grid.stats_candidates);
            canvas.draw_text_mono(ov_x + 16, ov_y + 138, &spatial_line, 0x0038BDF8, 12.5);

            canvas.draw_text(ov_x + 16, ov_y + 164, "TEMPS DE FRAME (HISTORIQUE) :", 0x0094A3B8, 1);
            let graph_y = ov_y + 184;
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

        // 5. Hotbar & Suivi de quête HUD en Mode Jeu
        if !is_edit_mode {
            let hotbar_y = if is_embedded { h - 16 } else { bottom_y - 12 };
            self.inventory.render_hotbar_at(canvas, vx + (vw / 2), hotbar_y);

            if let Some(ref q) = self.quest {
                let q_x = 18;
                let q_y = if is_embedded { vy + 16 } else { vy + 56 };
                q.render(canvas, q_x, q_y);
            }
        }

        // 5. Boîte de Dialogue, Effet Machine à Écrire & Choix Multiples (au premier plan)
        if self.dialogue.is_active {
            self.dialogue.render(canvas);
        }

        // Fenêtre Modale de Guide Pédagogique Interactif (Touche H ou Bouton ? AIDE)
        if !is_embedded && self.editor.show_help_modal {
            self.render_help_modal(canvas, w, h);
        }

        // Mode Guidé d'Intention KVL (Niveau 1 : Questionnaire Assisté)
        if !is_embedded && self.editor.wizard.is_open {
            self.render_guided_wizard(canvas, w, h);
        }

        // Fenêtre Modale d'Assistant IA (Phase A : Intent-to-KVL)
        if !is_embedded && self.editor.show_intent_bar {
            self.render_intent_modal(canvas, w, h);
        }

        // Panneau flottant de Revue de Patch IA (Phase C : Boucle de Confiance)
        if !is_embedded && self.editor.pending_ai_patch.is_some() {
            self.render_ai_patch_review_bar(canvas, w, h);
        }

        // Fenêtre Modale d'Accueil et Onboarding (3 Voies Débutant)
        if !is_embedded && self.editor.show_welcome_modal {
            self.render_welcome_modal(canvas, w, h);
        }

        // Fenêtre Modale d'Exportation Standalone (Option 3 : Packageur Web & Desktop 1 Clic)
        if !is_embedded && self.editor.show_export_modal {
            self.render_export_modal(canvas, w, h);
        }

        // Fenêtre Modale de Recette de Jeu en 6 Étapes (Desktop Native Game Recipe Wizard)
        if !is_embedded && self.editor.recipe_wizard.is_open {
            self.render_game_recipe_wizard(canvas, w, h);
        }

        // Rendu de l'infobulle contextuelle active (au premier plan absolu avec cornières Dark Atelier)
        if let Some((title, shortcut, desc, tx, ty)) = self.editor.active_tooltip.as_ref() {
            canvas.draw_tooltip(*tx, *ty, title, shortcut, desc);
        }
    }

    /// Rendu de la fenêtre modale de guide pédagogique interactif structurée en 3 onglets ergonomiques
    fn render_help_modal(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = 840;
        let mh = 590;
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        self.editor.guide_tab_btn_hitboxes.clear();

        // Ombrage d'arrière-plan et cadre Dark Atelier
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000D1322);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x007C3AED, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 20, 0x00A855F7);

        // Barre d'en-tête (Thème Dark Améthyste)
        canvas.draw_rect(mx, my, mw as u32, 40, 0x002E1065);
        canvas.draw_text(mx + 16, my + 11, "GUIDE PÉDAGOGIQUE — KARACTIRAGE DARK ATELIER", 0x00F3E8FF, 2);
        canvas.draw_badge(mx + mw - 110, my + 7, 95, 26, 0x003B1D66, 0x00A855F7, "[ÉCHAP]", 0x00F3E8FF, 1);

        // Sélecteur d'onglets (Pill Segmented Control sous l'en-tête)
        let tab_bar_y = my + 48;
        let tab_w = 170;
        let tab_h = 30;

        let tabs = [
            (0, "1. DÉCOUVRIR"),
            (1, "2. RACCOURCIS"),
            (2, "3. CONCEPTS"),
        ];

        for &(tab_idx, label) in &tabs {
            let tx = mx + 20 + (tab_idx as i32 * (tab_w + 10));
            let is_active = self.editor.guide_tab == tab_idx;
            let (bg, bd, fg) = if is_active {
                (0x002E1065, 0x00A855F7, 0x00F3E8FF)
            } else {
                (0x00131C2E, 0x00334155, 0x0094A3B8)
            };
            canvas.draw_badge(tx, tab_bar_y, tab_w as u32, tab_h as u32, bg, bd, label, fg, 1);
            self.editor.guide_tab_btn_hitboxes.push((tab_idx, tx, tab_bar_y, tab_w as u32, tab_h as u32));
        }

        let content_y = tab_bar_y + 42;

        match self.editor.guide_tab {
            0 => {
                // ONGLET 1 : PARCOURS D'INITIATION (6 ÉTAPES INTERACTIVES)
                let done_count = self.editor.guide_step_done.iter().filter(|&&d| d).count();

                canvas.draw_text(mx + 20, content_y, "PARCOURS D'INITIATION PAS-À-PAS :", 0x0038BDF8, 2);
                let progress_label = format!("Progression : {} / 6 étapes accomplies", done_count);
                canvas.draw_text_mono(mx + 450, content_y + 2, &progress_label, 0x00FBBF24, 13.0);

                // Barre de jauge de progression
                let gauge_y = content_y + 26;
                canvas.draw_rect(mx + 20, gauge_y, 800, 8, 0x001E293B);
                let fill_w = (done_count as f32 / 6.0 * 800.0) as u32;
                if fill_w > 0 {
                    canvas.draw_rect(mx + 20, gauge_y, fill_w, 8, 0x0010B981);
                }

                let steps = [
                    ("1. Se déplacer dans l'arène", "Utilisez ZQSD ou les Flèches du clavier pour mouvoir le personnage."),
                    ("2. Collecter un trésor ou item", "Marchez sur une pièce d'or ou une fiole pour marquer vos premiers points."),
                    ("3. Découvrir les genres de jeu", "Appuyez sur F4 ou L pour basculer instantanément en Plateforme, Shmup ou Sokoban."),
                    ("4. Entrer dans l'Atelier Studio", "Basculez en mode édition temps réel avec la touche TAB."),
                    ("5. Inspecter et déplacer une entité", "Sélectionnez un objet ou monstre à la souris avec l'outil Sélection (V)."),
                    ("6. Concevoir une mécanique KVL", "Ouvrez l'Assistant Guidé avec la touche G pour générer une mécanique sans code."),
                ];

                for (idx, (stitle, sdesc)) in steps.iter().enumerate() {
                    let sy = gauge_y + 18 + (idx as i32 * 60);
                    let is_done = self.editor.guide_step_done[idx];

                    let (card_bg, card_bd, badge_txt, badge_bg, badge_fg) = if is_done {
                        (0x00064E3B, 0x0010B981, "✓ VALIDÉ", 0x00065F46, 0x00A7F3D0)
                    } else {
                        (0x00131C2E, 0x0026334D, "À FAIRE", 0x001E293B, 0x0094A3B8)
                    };

                    canvas.draw_rect(mx + 20, sy, 800, 52, card_bg);
                    canvas.draw_rect_outline(mx + 20, sy, 800, 52, card_bd, 1);

                    canvas.draw_badge(mx + 30, sy + 13, 85, 26, badge_bg, card_bd, badge_txt, badge_fg, 1);
                    canvas.draw_text(mx + 130, sy + 10, stitle, 0x00F8FAFC, 1);
                    canvas.draw_text_body(mx + 130, sy + 28, sdesc, 0x0094A3B8, 12.5);
                }
            }
            1 => {
                // ONGLET 2 : TABLEAU ERGONOMIQUE DES RACCOURCIS
                let col_w = 390;
                let col1_x = mx + 20;
                let col2_x = mx + 430;

                // En-têtes de colonnes
                canvas.draw_badge(col1_x, content_y, col_w as u32, 28, 0x00064E3B, 0x0010B981, "🎮 COMMANDES EN MODE JEU", 0x00D1FAE5, 2);
                canvas.draw_badge(col2_x, content_y, col_w as u32, 28, 0x002E1065, 0x00A855F7, "🛠️ OUTILS DU STUDIO", 0x00F3E8FF, 2);

                let play_shortcuts = [
                    ("ZQSD / Flèches", "Déplacer le personnage joueur"),
                    ("Espace", "Sauter / Impulsion réacteur spatial"),
                    ("F", "Tirer un projectile balistique laser"),
                    ("E / Clic Gauche", "Interagir avec porte, coffre ou PNJ"),
                    ("1 .. 5", "Utiliser un objet dans la hotbar d'inventaire"),
                    ("F4 / L", "Changer instantanément de genre de jeu"),
                    ("N", "Activer ou masquer la minimap radar"),
                    ("F1", "Afficher l'analyse télémétrique des FPS"),
                    ("R", "Réinitialiser la scène au point de départ"),
                ];

                for (idx, (k, desc)) in play_shortcuts.iter().enumerate() {
                    let ky = content_y + 38 + (idx as i32 * 39);
                    canvas.draw_rect(col1_x, ky, col_w as u32, 34, 0x00131C2E);
                    canvas.draw_rect_outline(col1_x, ky, col_w as u32, 34, 0x001E293B, 1);
                    canvas.draw_badge(col1_x + 6, ky + 4, 115, 26, 0x001E293B, 0x0038BDF8, k, 0x00BAE6FD, 1);
                    canvas.draw_text_body(col1_x + 130, ky + 9, desc, 0x00CBD5E1, 12.5);
                }

                let studio_shortcuts = [
                    ("TAB", "Basculer entre Mode Jeu et Mode Studio"),
                    ("V", "Outil Sélection & déplacement d'entités"),
                    ("B", "Outil Pinceau pour peindre les tuiles"),
                    ("X", "Outil Gomme pour effacer les tuiles"),
                    ("T", "Cycler le matériau de tuile active"),
                    ("P", "Ouvrir la palette de spawn d'entités"),
                    ("G", "Ouvrir l'Assistant Guidé KVL"),
                    ("I", "Ouvrir l'Assistant IA Intent"),
                    ("Ctrl+S", "Sauvegarder scène et scripts KVL sur disque"),
                ];

                for (idx, (k, desc)) in studio_shortcuts.iter().enumerate() {
                    let ky = content_y + 38 + (idx as i32 * 39);
                    canvas.draw_rect(col2_x, ky, col_w as u32, 34, 0x00131C2E);
                    canvas.draw_rect_outline(col2_x, ky, col_w as u32, 34, 0x001E293B, 1);
                    canvas.draw_badge(col2_x + 6, ky + 4, 85, 26, 0x001E293B, 0x00A855F7, k, 0x00E9D5FF, 1);
                    canvas.draw_text_body(col2_x + 100, ky + 9, desc, 0x00CBD5E1, 12.5);
                }
            }
            2 => {
                // ONGLET 3 : CONCEPTS & ARCHITECTURE DARK ATELIER
                canvas.draw_text(mx + 20, content_y, "FONDATIONS ARCHITECTURALES DU MOTEUR :", 0x00A855F7, 2);

                let concepts = [
                    ("Moteur Déterministe Entité-Composants (ECS)", "Architecture universelle découplée : les entités sont de purs conteneurs identifiés par ID, enrichies dynamiquement de composants (Transform2D, Collider2D, Movement2D, Health2D, DialogueComponent, NavAgent A*). Aucun héritage de classes rigide."),
                    ("Logique Visuelle Réactive (KVL)", "Karacti Visual Logic exécute des graphes typés reliant des Événements (OnCollision, OnInteract, OnSeen) à des Actions concrètes (BonusScore, DamagePlayer, Teleport) orientées par des Conditions (Possession de clé, Seuil de score). Zéro code requis."),
                    ("Conception Multi-Genres Native", "Un moteur unique capable d'exécuter simultanément les 4 piliers du jeu vidéo 2D : RPG d'aventure en vue de dessus, Plateforme avec gravité newtonienne, Shoot'em Up spatial à impulsion inertielle, et Puzzle Sokoban à grille quantifiée."),
                    ("Performance Temps Réel & Zéro-Allocation", "Boucle de rendu 60 FPS constants (<16.6 ms de budget CPU par frame), Grille Spatiale O(1) de détection de collision, mémoire pré-allouée et compilation WebAssembly ultra-légère sans dépendances externes lourdes."),
                ];

                for (idx, (ctitle, cdesc)) in concepts.iter().enumerate() {
                    let cy = content_y + 30 + (idx as i32 * 96);
                    canvas.draw_rect(mx + 20, cy, 800, 86, 0x00131C2E);
                    canvas.draw_rect_outline(mx + 20, cy, 800, 86, 0x00334155, 1);
                    canvas.draw_corner_brackets(mx + 20, cy, 800, 86, 10, 0x0038BDF8);

                    canvas.draw_text(mx + 36, cy + 12, ctitle, 0x0038BDF8, 1);
                    canvas.draw_text_body(mx + 36, cy + 34, cdesc, 0x0094A3B8, 13.0);
                }
            }
            _ => {}
        }

        // Bouton de fermeture en bas
        let close_btn_y = my + mh - 46;
        canvas.draw_badge(mx + 20, close_btn_y, (mw - 40) as u32, 34, 0x00065F46, 0x0010B981, "[ FERMER LE GUIDE PÉDAGOGIQUE (H OU ÉCHAP) ]", 0x00ECFDF5, 2);
        self.editor.guide_close_btn_hitbox = Some((mx + 20, close_btn_y, (mw - 40) as u32, 34));
    }

    /// Rendu du Hub de Projets & Accueil KaractiRage Unifié (Section 1 de la spécification fonctionnelle)
    fn render_welcome_modal(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = 940;
        let mh = 620;
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        self.editor.welcome_tab_hitboxes.clear();
        self.editor.welcome_scene_hitboxes.clear();
        self.editor.welcome_archetype_hitboxes.clear();
        self.editor.welcome_ai_provider_hitboxes.clear();
        self.editor.welcome_ai_preset_hitboxes.clear();
        self.editor.welcome_hero_ai_btn_hitbox = None;
        self.editor.welcome_ai_test_btn_hitbox = None;
        self.editor.welcome_ai_generate_btn_hitbox = None;
        self.editor.welcome_bottom_ai_btn_hitbox = None;
        self.editor.welcome_play_btn_hitbox = None;
        self.editor.welcome_studio_btn_hitbox = None;
        self.editor.welcome_guide_btn_hitbox = None;
        self.editor.welcome_recipe_btn_hitbox = None;
        self.editor.welcome_close_btn_hitbox = None;

        // Ombrage d'arrière-plan et cadre Dark Atelier
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000B0F19);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x007C3AED, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 24, 0x00A855F7);

        // Barre d'en-tête (Thème Dark Améthyste)
        canvas.draw_rect(mx, my, mw as u32, 54, 0x0017122E);
        canvas.draw_badge(mx + 18, my + 12, 38, 30, 0x002E1065, 0x00A855F7, "KR", 0x00F3E8FF, 2);
        canvas.draw_text(mx + 66, my + 14, "KARACTIRAGE ATELIER — HUB DE PROJETS UNIFIÉ", 0x00F8FAFC, 2);
        canvas.draw_text_body(mx + 66, my + 34, "Atelier de création 2D multi-genres • Logique Visuelle KVL • Déterministe 60 FPS", 0x00A855F7, 12.0);

        let close_w = 126;
        let close_x = mx + mw - close_w - 18;
        canvas.draw_badge(close_x, my + 12, close_w as u32, 30, 0x001E293B, 0x0064748B, "[✕ FERMER (ÉCHAP)]", 0x00CBD5E1, 1);
        self.editor.welcome_close_btn_hitbox = Some((close_x, my + 12, close_w as u32, 30));

        // Sélecteur d'Onglets Segmented Control (Pill) — 3 Sections : 1.1, 1.2, 1.3
        let tab_y = my + 64;
        let tab_h = 32;

        let tabs = [
            (0, "⚡ 1.1 NOUVEAU PROJET / 6 ÉTAPES", 260),
            (1, "● 1.2 PROJETS SUR DISQUE", 260),
            (2, "🤖 1.3 INTENTION IA & CONFIG LLM [I]", 340),
        ];

        let mut tx_offset = mx + 24;
        for &(tab_idx, label, tw) in &tabs {
            let is_active = self.editor.welcome_tab == tab_idx;
            let (bg, bd, fg) = if is_active {
                (0x003B0764, 0x00A855F7, 0x00F3E8FF)
            } else {
                (0x000F172A, 0x00334155, 0x0094A3B8)
            };
            canvas.draw_badge(tx_offset, tab_y, tw as u32, tab_h as u32, bg, bd, label, fg, 1);
            self.editor.welcome_tab_hitboxes.push((tab_idx, tx_offset, tab_y, tw as u32, tab_h as u32));
            tx_offset += tw + 14;
        }

        // ==========================================
        // VUE ONGLET 0 : NOUVEAU PROJET & ASSISTANT (1.1)
        // ==========================================
        if self.editor.welcome_tab == 0 {
            // Bannière de l'Assistant de Création en 6 Étapes
            let ban_y = my + 108;
            let ban_h = 80;
            canvas.draw_rect(mx + 24, ban_y, (mw - 48) as u32, ban_h as u32, 0x00170F28);
            canvas.draw_rect_outline(mx + 24, ban_y, (mw - 48) as u32, ban_h as u32, 0x00A855F7, 1);
            canvas.draw_corner_brackets(mx + 24, ban_y, (mw - 48) as u32, ban_h as u32, 14, 0x00C084FC);

            canvas.draw_badge(mx + 38, ban_y + 14, 46, 28, 0x002E1065, 0x00C084FC, "1.1", 0x00F3E8FF, 2);
            canvas.draw_text(mx + 94, ban_y + 14, "ASSISTANT DE CRÉATION GUIDÉ (6 ÉTAPES)", 0x00F8FAFC, 2);
            canvas.draw_text_body(mx + 94, ban_y + 36, "Configurez genres, héros, physique et gameplay 2D.", 0x00CBD5E1, 12.0);

            let wiz_btn_w = 200;
            let wiz_btn_x = mx + mw - 24 - wiz_btn_w - 12;
            canvas.draw_badge(wiz_btn_x, ban_y + 22, wiz_btn_w as u32, 36, 0x003B0764, 0x00A855F7, "▶ ASSISTANT [W]", 0x00F3E8FF, 1);
            self.editor.welcome_recipe_btn_hitbox = Some((wiz_btn_x, ban_y + 22, wiz_btn_w as u32, 36));

            let ai_hero_w = 190;
            let ai_hero_x = wiz_btn_x - ai_hero_w - 12;
            canvas.draw_badge(ai_hero_x, ban_y + 22, ai_hero_w as u32, 36, 0x002E1065, 0x0038BDF8, "🤖 INTENTION IA [I]", 0x00F0F9FF, 1);
            self.editor.welcome_hero_ai_btn_hitbox = Some((ai_hero_x, ban_y + 22, ai_hero_w as u32, 36));

            // Section Démarrage Rapide : 5 Archétypes
            let sec_y = my + 204;
            canvas.draw_text(mx + 26, sec_y, "⚡ OU DÉMARRAGE IMMÉDIAT : 5 ARCHÉTYPES DE JEU EN 1 CLIC", 0x0038BDF8, 2);
            canvas.draw_text_body(mx + 26, sec_y + 20, "Instancie instantanément la recette, configure les composants ECS et lance la simulation à 60 FPS :", 0x0094A3B8, 12.0);

            let card_y = sec_y + 40;
            let card_w: i32 = 170;
            let card_h: i32 = 264;
            let spacing = 178;

            let archetypes = [
                ("rpg_dungeon", "RPG DONJON", "Top-Down 2D", 0x000E1C1A, 0x0010B981, 0x00D1FAE5, [
                    "• Combat à l'épée",
                    "• Inventaire 5 slots",
                    "• Portes & clés",
                    "• Coffres au trésor",
                ]),
                ("platformer", "PLATFORMER", "Vue Latérale 2D", 0x00151828, 0x008B5CF6, 0x00EDE9FE, [
                    "• Gravité & Sauts",
                    "• Moteurs mobiles",
                    "• Dalles réactives",
                    "• Dash cinétique",
                ]),
                ("shootemup", "SHMUP SPATIAL", "Action Espace", 0x001C1226, 0x00EC4899, 0x00FCE7F3, [
                    "• Inertie spatiale",
                    "• Lasers balistiques",
                    "• Vagues d'ennemis",
                    "• Score réactif KVL",
                ]),
                ("puzzle_sokoban", "PUZZLE SOKOBAN", "Grille Tactique", 0x001A180E, 0x00F59E0B, 0x00FEF3C7, [
                    "• Poussée de blocs",
                    "• Dalles de sol",
                    "• Énigmes logiques",
                    "• Déplacement case",
                ]),
                ("boss_arena", "TITAN BOSS", "Combat Arène", 0x00220F16, 0x00EF4444, 0x00FEE2E2, [
                    "• Boss géant KVL",
                    "• Télégraphie frappe",
                    "• Barres de santé",
                    "• Esquives & Dash",
                ]),
            ];

            for (idx, (arch_key, title, genre_sub, bg, bd, fg, bullet_points)) in archetypes.iter().enumerate() {
                let cx = mx + 24 + (idx as i32 * spacing);
                canvas.draw_rect(cx, card_y, card_w as u32, card_h as u32, *bg);
                canvas.draw_rect_outline(cx, card_y, card_w as u32, card_h as u32, *bd, 2);
                canvas.draw_corner_brackets(cx, card_y, card_w as u32, card_h as u32, 12, *bd);

                canvas.draw_badge(cx + 10, card_y + 12, (card_w - 20) as u32, 26, 0x000F172A, *bd, title, *fg, 1);
                canvas.draw_text_body(cx + 12, card_y + 44, genre_sub, *bd, 11.5);

                let mut by = card_y + 66;
                for pt in bullet_points {
                    canvas.draw_text_body(cx + 12, by, pt, 0x00CBD5E1, 11.0);
                    by += 18;
                }

                let btn_h = 30;
                let btn_y = card_y + card_h - btn_h - 12;
                canvas.draw_badge(cx + 10, btn_y, (card_w - 20) as u32, btn_h as u32, 0x001E293B, *bd, "▶ Lancer", *fg, 1);

                self.editor.welcome_archetype_hitboxes.push((arch_key, cx, card_y, card_w as u32, card_h as u32));
            }
        }
        // ==========================================
        // VUE ONGLET 1 : CHARGER UN PROJET SUR DISQUE (1.2)
        // ==========================================
        else if self.editor.welcome_tab == 1 {
            let sec_y = my + 106;
            canvas.draw_text(mx + 26, sec_y, "PROJETS RÉCENTS ET SCÈNES D'EXEMPLE SUR DISQUE (SECTION 1.2) :", 0x00F8FAFC, 2);
            canvas.draw_text_body(mx + 26, sec_y + 20, "Sélectionnez un projet pour le lancer immédiatement en Mode Jeu ou l'éditer dans l'Atelier Studio :", 0x0094A3B8, 12.0);

            let row_y_base = sec_y + 42;
            let row_h = 72;
            let row_gap = 78;

            let scenes_meta = [
                ("Donjon Niveau 1 — Aventure RPG Top-Down", "assets/scenes/level_01.scene.json", "36 Ko", "Aventure • 2D Top-Down • Quêtes, Clés, Portes & Sentinelles"),
                ("Cyber Platformer 2D — Gravité & Sauts", "assets/scenes/platformer_01.scene.json", "8.8 Ko", "Plateforme • 2D Vue Latérale • Moteurs de saut, Plateformes mobiles"),
                ("Shmup Stellaire — Shoot'em Up & Vagues Spatiales", "assets/scenes/space_01.scene.json", "5.0 Ko", "Shoot'em Up • 2D Inertie • Tirs lasers, Vagues d'astronefs"),
                ("Puzzle Sokoban — Énigmes & Poussée de Blocs", "assets/scenes/puzzle_01.scene.json", "9.3 Ko", "Réflexion • 2D Grille • Déplacement case par case, Dalles & Caisses"),
                ("Arène du Titan — Combat de Boss & Télégraphie", "assets/scenes/boss_01.scene.json", "11.6 Ko", "Combat • 2D Arène • Boss réactif KVL, Télégraphie & Phase 2"),
            ];

            for (idx, (title, path, size, desc)) in scenes_meta.iter().enumerate() {
                let ry = row_y_base + (idx as i32 * row_gap);
                let rw = mw - 48;

                canvas.draw_rect(mx + 24, ry, rw as u32, row_h as u32, 0x000F172A);
                canvas.draw_rect_outline(mx + 24, ry, rw as u32, row_h as u32, 0x001E293B, 1);
                canvas.draw_corner_brackets(mx + 24, ry, rw as u32, row_h as u32, 10, 0x00334155);

                canvas.draw_badge(mx + 36, ry + 12, 30, 24, 0x001E1B4B, 0x00818CF8, &format!("{}", idx + 1), 0x00E0E7FF, 1);
                canvas.draw_text(mx + 76, ry + 12, title, 0x00F8FAFC, 1);
                canvas.draw_text_body(mx + 76, ry + 32, &format!("{} • Taille : {}", path, size), 0x0038BDF8, 11.5);
                canvas.draw_text_body(mx + 76, ry + 48, desc, 0x0094A3B8, 11.0);

                // Badge Statut
                canvas.draw_badge(mx + rw - 310, ry + 22, 130, 26, 0x00064E3B, 0x0010B981, "✓ SUR DISQUE", 0x00D1FAE5, 1);

                // Bouton [ ▶ JOUER ]
                let play_btn_x = mx + rw - 170;
                let play_btn_w = 78;
                canvas.draw_badge(play_btn_x, ry + 18, play_btn_w as u32, 34, 0x00065F46, 0x0010B981, "▶ JOUER", 0x00ECFDF5, 1);
                self.editor.welcome_scene_hitboxes.push((idx, false, play_btn_x, ry + 18, play_btn_w as u32, 34));

                // Bouton [ ★ STUDIO ]
                let studio_btn_x = mx + rw - 84;
                let studio_btn_w = 80;
                canvas.draw_badge(studio_btn_x, ry + 18, studio_btn_w as u32, 34, 0x003B0764, 0x00A855F7, "★ STUDIO", 0x00F3E8FF, 1);
                self.editor.welcome_scene_hitboxes.push((idx, true, studio_btn_x, ry + 18, studio_btn_w as u32, 34));
            }
        }
        // ==============================================================
        // VUE ONGLET 2 : INTENTION IA & CONFIGURATION DU MODÈLE LLM (1.3)
        // ==============================================================
        else {
            let sec_y = my + 104;
            canvas.draw_text(mx + 26, sec_y, "🤖 INTENTION IA & CONFIGURATION DU MODÈLE LLM (SECTION 1.3) :", 0x0038BDF8, 2);
            canvas.draw_text_body(
                mx + 26,
                sec_y + 20,
                "Connectez un modèle local (LM Studio sur http://127.0.0.1:1234, Ollama) ou Cloud, et pilotez la création de jeu par intention.",
                0x0094A3B8,
                12.0,
            );

            // Sélecteur de Fournisseur / Provider LLM (6 pills)
            let prov_y = sec_y + 42;
            let providers = crate::ai_client::AiProvider::ALL;
            let prov_gap = 8;
            let prov_w = (mw - 48 - (providers.len() as i32 - 1) * prov_gap) / providers.len() as i32;
            let prov_h = 30;

            for (idx, &prov) in providers.iter().enumerate() {
                let px = mx + 24 + idx as i32 * (prov_w + prov_gap);
                let is_selected = self.editor.ai_config.provider == prov;
                let is_hov = is_selected || (self.editor.mouse_x >= px as f32 && self.editor.mouse_x <= (px + prov_w) as f32 && self.editor.mouse_y >= prov_y as f32 && self.editor.mouse_y <= (prov_y + prov_h) as f32);

                let (bg, bd, fg) = if is_selected {
                    (0x003B0764, 0x00C084FC, 0x00FAF5FF)
                } else if is_hov {
                    (0x001E293B, 0x0038BDF8, 0x00F0F9FF)
                } else {
                    (0x000F172A, 0x00334155, 0x0064748B)
                };

                canvas.draw_badge(px, prov_y, prov_w as u32, prov_h as u32, bg, bd, prov.short_label(), fg, 1);
                self.editor.welcome_ai_provider_hitboxes.push((prov, px, prov_y, prov_w as u32, prov_h as u32));
            }

            // Ligne de Statut & Test de Connexion LLM
            let stat_y = prov_y + 38;
            let stat_h = 34;
            canvas.draw_rect(mx + 24, stat_y, (mw - 48) as u32, stat_h as u32, 0x000F172A);
            canvas.draw_rect_outline(mx + 24, stat_y, (mw - 48) as u32, stat_h as u32, 0x001E293B, 1);

            let active_prov = self.editor.ai_config.provider;
            let default_desc = match active_prov {
                crate::ai_client::AiProvider::LmStudio => "🏢 LM Studio Local • http://127.0.0.1:1234 • nvidia/nemotron-3-nano-4b",
                crate::ai_client::AiProvider::Ollama => "🦙 Ollama Local • http://127.0.0.1:11434 • qwen2.5-coder / llama3",
                crate::ai_client::AiProvider::Claude => "🤖 Claude (Anthropic) • claude-3-5-sonnet • Clé requise : ANTHROPIC_API_KEY",
                crate::ai_client::AiProvider::Gemini => "🔮 Google Gemini • gemini-1.5-flash • Clé requise : GEMINI_API_KEY",
                crate::ai_client::AiProvider::DeepSeek => "⚡ DeepSeek • deepseek-chat • Clé requise : DEEPSEEK_API_KEY",
                crate::ai_client::AiProvider::OpenAI => "🟢 OpenAI • gpt-4o-mini • Clé requise : OPENAI_API_KEY",
                crate::ai_client::AiProvider::OfflineLocal => "⚡ Moteur Local Intégré • 100% Autonome, Déterministe et Hors-ligne",
            };

            let (disp_msg, disp_col) = if let Some((ok, ref msg)) = self.editor.ai_test_result {
                (msg.as_str(), if ok { 0x0010B981 } else { 0x00EF4444 })
            } else {
                (default_desc, 0x0094A3B8)
            };

            canvas.draw_text_body(mx + 36, stat_y + 10, disp_msg, disp_col, 12.0);

            let test_w = 185;
            let test_x = mx + mw - 24 - test_w - 6;
            canvas.draw_badge(test_x, stat_y + 4, test_w as u32, 26, 0x001E293B, 0x0038BDF8, "🔌 TESTER LA CONNEXION", 0x0038BDF8, 1);
            self.editor.welcome_ai_test_btn_hitbox = Some((test_x, stat_y + 4, test_w as u32, 26));

            // Champ de saisie d'intention libre (Jusqu'à 400 caractères)
            let input_title_y = stat_y + 44;
            canvas.draw_text(mx + 26, input_title_y, "VOTRE INTENTION EN LANGAGE NATUREL :", 0x00F8FAFC, 1);

            // Compteur de caractères
            let char_count = self.editor.intent_input_text.chars().count();
            let count_txt = format!("{}/400 car.", char_count);
            let count_col = if char_count >= 380 { 0x00EF4444 } else if char_count > 0 { 0x0038BDF8 } else { 0x0064748B };

            // Badge de bascule Clavier AZERTY / QWERTY
            let kbd_txt = format!("[CLAVIER : {} (CLIC)]", self.editor.keyboard_layout.label());
            let kbd_w = 180;
            let kbd_x = mx + mw - 24 - kbd_w;
            canvas.draw_badge(kbd_x, input_title_y - 4, kbd_w as u32, 22, 0x001E293B, 0x00A855F7, &kbd_txt, 0x00F3E8FF, 1);
            self.editor.keyboard_layout_hitbox = Some((kbd_x, input_title_y - 4, kbd_w as u32, 22));

            canvas.draw_text(kbd_x - 96, input_title_y, &count_txt, count_col, 1);

            let box_y = input_title_y + 20;
            let btn_gen_w = 220;
            let box_w = (mw - 48) - btn_gen_w - 12;
            let box_h = 38;

            canvas.draw_rect(mx + 24, box_y, box_w as u32, box_h as u32, 0x000F172A);
            canvas.draw_rect_outline(mx + 24, box_y, box_w as u32, box_h as u32, 0x0038BDF8, 2);

            if self.editor.ai_generation_in_progress {
                let blink = (self.elapsed_time * 5.0).sin() > 0.0;
                let col = if blink { 0x00F59E0B } else { 0x00D97706 };
                canvas.draw_text(mx + 36, box_y + 12, &self.editor.ai_status_text, col, 1);
            } else if self.editor.intent_input_text.is_empty() {
                canvas.draw_text(mx + 36, box_y + 12, "Décrivez votre univers (ex: 'donjon de lave avec pièges et boss dragon')...", 0x0064748B, 1);
            } else {
                let full_text = &self.editor.intent_input_text;
                let c_count = full_text.chars().count();
                let display_slice: String = if c_count > 60 {
                    format!("...{}", full_text.chars().skip(c_count - 60).collect::<String>())
                } else {
                    full_text.clone()
                };
                let cursor = if (self.elapsed_time * 3.0).fract() > 0.5 { "_" } else { " " };
                let display_txt = format!("> {}{}", display_slice, cursor);
                canvas.draw_text(mx + 36, box_y + 12, &display_txt, 0x00F3E8FF, 1);
            }

            let gen_btn_x = mx + 24 + box_w + 12;
            let gen_txt = if self.editor.ai_generation_in_progress { "GÉNÉRATION..." } else { "⚡ GÉNÉRER MON JEU [ENTRÉE]" };
            let gen_bg = if self.editor.ai_generation_in_progress { 0x0078350F } else { 0x003B0764 };
            canvas.draw_badge(gen_btn_x, box_y, btn_gen_w as u32, box_h as u32, gen_bg, 0x00A855F7, gen_txt, 0x00F3E8FF, 1);
            self.editor.welcome_ai_generate_btn_hitbox = Some((gen_btn_x, box_y, btn_gen_w as u32, box_h as u32));

            // Grille de 4 Presets de Mondes Prédéfinis en Français
            let pres_title_y = box_y + box_h + 16;
            canvas.draw_text(mx + 26, pres_title_y, "OU SÉLECTIONNEZ UN ARCHÉTYPE DE MONDE PRÉDÉFINI EN 1 CLIC :", 0x00CBD5E1, 1);

            let presets = [
                ("🏰 Donjon de Lave & Boss Titan", "Héros ninja, gouffres de lave ardents, clé dorée et combat contre le boss titan démon."),
                ("❄️ Forteresse Glacée & Énigmes", "Chevalier errant, slimes de givre, dalles de saut cinétiques et coffre scellé."),
                ("⚡ Laboratoire Cyber & Drones A*", "Drones patrouilleurs A* en réseau, portails quantiques et cristaux d'énergie réactifs."),
                ("🌲 Forêt Mystique & Sokoban", "Golem de pierre, énigme de blocs Sokoban, portes runiques et potions de vitalité."),
            ];

            let card_y_base = pres_title_y + 16;
            let card_h = 46;
            let card_gap = 50;

            for (idx, &(title, desc)) in presets.iter().enumerate() {
                let cy = card_y_base + (idx as i32 * card_gap);
                let cw = mw - 48;

                canvas.draw_rect(mx + 24, cy, cw as u32, card_h as u32, 0x000F172A);
                canvas.draw_rect_outline(mx + 24, cy, cw as u32, card_h as u32, 0x001E293B, 1);

                canvas.draw_text(mx + 36, cy + 8, title, 0x0038BDF8, 1);
                canvas.draw_text_body(mx + 36, cy + 26, desc, 0x0094A3B8, 11.5);

                let pbtn_w = 110;
                let pbtn_x = mx + cw - pbtn_w - 6;
                canvas.draw_badge(pbtn_x, cy + 8, pbtn_w as u32, 30, 0x002E1065, 0x00A855F7, "⚡ GÉNÉRER", 0x00F3E8FF, 1);

                self.editor.welcome_ai_preset_hitboxes.push((idx, mx + 24, cy, cw as u32, card_h as u32));
            }
        }

        // ==========================================
        // BARRE D'ACTIONS INFÉRIEURE DU HUB
        // ==========================================
        let bottom_btn_y = my + mh - 50;
        let btn_h = 36;

        // Bouton 1 : Play Immédiat
        let b1_w = 165;
        let b1_x = mx + 24;
        canvas.draw_badge(b1_x, bottom_btn_y, b1_w as u32, btn_h as u32, 0x00065F46, 0x0010B981, "▶ CONTINUER DÉMO", 0x00ECFDF5, 1);
        self.editor.welcome_play_btn_hitbox = Some((b1_x, bottom_btn_y, b1_w as u32, btn_h as u32));

        // Bouton 2 : Studio Mode
        let b2_w = 155;
        let b2_x = b1_x + b1_w + 12;
        canvas.draw_badge(b2_x, bottom_btn_y, b2_w as u32, btn_h as u32, 0x004C1D95, 0x00A855F7, "★ ATELIER STUDIO", 0x00F3E8FF, 1);
        self.editor.welcome_studio_btn_hitbox = Some((b2_x, bottom_btn_y, b2_w as u32, btn_h as u32));

        // Bouton 3 : Intention IA & Config LLM
        let b3_w = 185;
        let b3_x = b2_x + b2_w + 12;
        let (ia_bg, ia_bd, ia_fg) = if self.editor.welcome_tab == 2 {
            (0x00581C87, 0x00C084FC, 0x00FAF5FF)
        } else {
            (0x002E1065, 0x00A855F7, 0x00F3E8FF)
        };
        canvas.draw_badge(b3_x, bottom_btn_y, b3_w as u32, btn_h as u32, ia_bg, ia_bd, "🤖 INTENTION IA [I]", ia_fg, 1);
        self.editor.welcome_bottom_ai_btn_hitbox = Some((b3_x, bottom_btn_y, b3_w as u32, btn_h as u32));

        // Bouton 4 : Guide Pédagogique
        let b4_w = 185;
        let b4_x = b3_x + b3_w + 12;
        canvas.draw_badge(b4_x, bottom_btn_y, b4_w as u32, btn_h as u32, 0x00075985, 0x0038BDF8, "● GUIDE PÉDAGOGIQUE [H]", 0x00F0F9FF, 1);
        self.editor.welcome_guide_btn_hitbox = Some((b4_x, bottom_btn_y, b4_w as u32, btn_h as u32));

        // Bouton 5 : Fermer / Échap
        let b5_w = (mx + mw - 24) - (b4_x + b4_w + 12);
        let b5_x = b4_x + b4_w + 12;
        canvas.draw_badge(b5_x, bottom_btn_y, b5_w as u32, btn_h as u32, 0x001E293B, 0x0064748B, "✕ FERMER [ÉCHAP]", 0x00CBD5E1, 1);
        self.editor.welcome_close_btn_hitbox = Some((b5_x, bottom_btn_y, b5_w as u32, btn_h as u32));
    }

    /// Déclenche l'analyse d'intention en langage naturel et génère un patch d'assistance IA avec validation Sandbox
    pub fn trigger_ai_intent(&mut self, prompt: &str) {
        if self.editor.ai_config.provider == crate::ai_client::AiProvider::OfflineLocal {
            let player_pos = self.player_entity().map(|p| (p.transform.x, p.transform.y)).unwrap_or((100.0, 100.0));
            let next_id = self.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
            let existing_graph = self.kvl_graphs.get("ai_patch_guard")
                .or_else(|| self.kvl_graphs.get("ai_patch_chest"))
                .or_else(|| self.kvl_graphs.values().next());

            let patch = crate::ai_patch::AiPatchBuilder::build_from_prompt(
                prompt,
                player_pos,
                next_id,
                &self.entities,
                existing_graph,
            );

            // Intégration temporaire des entités pour prévisualisation visuelle immédiate
            for ent in &patch.created_entities {
                self.entities.push(ent.clone());
            }
            self.kvl_graphs.insert(patch.target_graph_id.clone(), patch.generated_graph.clone());

            self.editor.show_intent_bar = false;
            self.editor.intent_input_text.clear();
            self.set_status(format!("✨ Patch IA généré : '{}'. Examinez le halo violet et validez (Entrée).", patch.title));
            self.audio.play(SoundType::ItemPickup);
            self.editor.pending_ai_patch = Some(patch);
        } else {
            // Lancement asynchrone non-bloquant vers le LLM sélectionné (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI)
            self.editor.ai_generation_in_progress = true;
            let model_name = self.editor.ai_config.custom_model.clone().unwrap_or_else(|| self.editor.ai_config.provider.default_model().to_string());
            let status_msg = format!("⚡ Requête vers {} [{}] en cours...", self.editor.ai_config.provider.label(), model_name);
            self.editor.ai_status_text = status_msg.clone();
            self.set_status(&status_msg);
            self.editor.show_intent_bar = false;

            let (tx, rx) = std::sync::mpsc::channel();
            self.ai_rx = Some(rx);

            let config_clone = self.editor.ai_config.clone();
            let prompt_string = prompt.to_string();

            std::thread::spawn(move || {
                let res = crate::ai_client::AiClient::send_prompt(&config_clone, &prompt_string);
                let _ = tx.send((prompt_string, res));
            });
        }
    }

    /// Applique le payload structuré généré par un modèle LLM (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI)
    pub fn apply_ai_payload(&mut self, payload: crate::ai_client::AiGeneratedGamePayload, prompt: &str) {
        let player_pos = self.player_entity().map(|p| (p.transform.x, p.transform.y)).unwrap_or((100.0, 100.0));
        let next_id = self.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let existing_graph = self.kvl_graphs.get("ai_patch_guard")
            .or_else(|| self.kvl_graphs.get("ai_patch_chest"))
            .or_else(|| self.kvl_graphs.values().next());

        let patch = crate::ai_patch::AiPatchBuilder::build_from_generated_payload(
            &payload,
            prompt,
            player_pos,
            next_id,
            &self.entities,
            existing_graph,
        );

        for ent in &patch.created_entities {
            self.entities.push(ent.clone());
        }
        self.kvl_graphs.insert(patch.target_graph_id.clone(), patch.generated_graph.clone());

        if let Some(ref gl) = payload.gameplay_loop {
            self.scene_objective = Some(gl.objective.clone());
        }

        self.editor.intent_input_text.clear();
        self.set_status(format!("✨ Patch {} généré : '{}'. Examinez le halo violet et validez (Entrée).", self.editor.ai_config.provider.label(), patch.title));
        self.audio.play(SoundType::ItemPickup);
        self.editor.pending_ai_patch = Some(patch);
    }

    /// Valide définitivement le patch IA dans la scène et l'enregistre dans l'historique Undo/Redo
    pub fn accept_ai_patch(&mut self) {
        if let Some(patch) = self.editor.pending_ai_patch.take() {
            let created_ids: Vec<usize> = patch.created_entities.iter().map(|e| e.id).collect();

            // 1. Enregistrement de la commande réversible dans la pile Undo
            self.editor.undo_stack.push(StudioCommand::ApplyAiPatch {
                patch_id: patch.id.clone(),
                description: patch.title.clone(),
                created_entity_ids: created_ids,
                created_entities: patch.created_entities.clone(),
                target_graph_id: patch.target_graph_id.clone(),
                previous_graph: patch.previous_graph_backup,
                patched_graph: patch.generated_graph,
            });

            // 2. Journalisation dans l'historique d'audit IA
            self.editor.ai_patch_history.push(crate::ai_patch::AiPatchHistoryItem {
                timestamp: format!("{:.1}s", self.elapsed_time),
                prompt: patch.prompt.clone(),
                title: patch.title.clone(),
                status: "Accepté".to_string(),
                summary: patch.description.clone(),
            });

            // 3. Gerbe de particules festives et fanfare de triomphe
            let player_pos = self.player_entity().map(|p| (p.transform.x, p.transform.y)).unwrap_or((100.0, 100.0));
            self.spawn_particles(player_pos.0, player_pos.1, 25, 0x00A855F7);
            self.audio.play(SoundType::QuestSuccess);
            self.set_status(&format!("✨ Patch IA '{}' validé et scellé ! (Annulable avec Ctrl+Z)", patch.title));
        }
    }

    /// Rejette le patch IA et applique un rollback instantané sans effet résiduel
    pub fn reject_ai_patch(&mut self) {
        if let Some(patch) = self.editor.pending_ai_patch.take() {
            // 1. Restauration exacte des entités antérieures
            self.entities = patch.previous_entities_backup;

            // 2. Restauration ou suppression du graphe KVL
            if let Some(prev) = patch.previous_graph_backup {
                self.kvl_graphs.insert(patch.target_graph_id, prev);
            } else {
                self.kvl_graphs.remove(&patch.target_graph_id);
            }

            // 3. Journalisation du rejet
            self.editor.ai_patch_history.push(crate::ai_patch::AiPatchHistoryItem {
                timestamp: format!("{:.1}s", self.elapsed_time),
                prompt: patch.prompt.clone(),
                title: patch.title.clone(),
                status: "Rejeté".to_string(),
                summary: "Rollback instantané sans altération".to_string(),
            });

            self.audio.play(SoundType::UiClick);
            self.set_status("✕ Proposition IA rejetée — Scène restaurée à son état antérieur.");
        }
    }

    /// Rendu du panneau flottant de revue et validation du patch IA (Phase C : Boucle de Confiance)
    fn render_ai_patch_review_bar(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let Some(ref patch) = self.editor.pending_ai_patch else { return; };

        let rw = (screen_w - 40).min(980);
        let rh = 90;
        let rx = (screen_w - rw) / 2;
        let ry = screen_h - rh - 18;

        // Ombrage et fond ardoise sombre
        canvas.draw_rect(rx, ry, rw as u32, rh as u32, 0x000F172A);
        canvas.draw_rect_outline(rx, ry, rw as u32, rh as u32, 0x00A855F7, 2);
        canvas.draw_corner_brackets(rx, ry, rw as u32, rh as u32, 16, 0x00E9D5FF);

        // Badge En-tête
        canvas.draw_badge(rx + 16, ry + 12, 165, 26, 0x002E1065, 0x00A855F7, "✨ REVUE DU PATCH IA", 0x00F3E8FF, 1);
        canvas.draw_text(rx + 192, ry + 14, &format!("\"{}\"", patch.title), 0x0038BDF8, 2);

        // Badge Bac à sable (Sandbox)
        let sim_text = format!("✓ BAC À SABLE : {} PAS SIMULÉS · 0 ERREUR", patch.sandbox_report.simulation_steps_tested);
        canvas.draw_badge(rx + 16, ry + 44, 280, 20, 0x00064E3B, 0x0010B981, &sim_text, 0x00A7F3D0, 1);

        // Description concise
        canvas.draw_text_body(rx + 16, ry + 70, &patch.description, 0x00CBD5E1, 12.0);

        // Boutons d'Action
        let btn_y = ry + 26;
        let btn_h = 38;

        // [✓ ACCEPTER (Entrée)]
        let accept_w = 145;
        let accept_x = rx + rw - 440;
        canvas.draw_badge(accept_x, btn_y, accept_w, btn_h, 0x00065F46, 0x0010B981, "✓ ACCEPTER (Entrée)", 0x00ECFDF5, 1);
        self.editor.ai_patch_accept_hitbox = Some((accept_x, btn_y, accept_w, btn_h));

        // [✏ MODIFIER (Espace)]
        let mod_w = 135;
        let mod_x = rx + rw - 285;
        canvas.draw_badge(mod_x, btn_y, mod_w, btn_h, 0x00451A03, 0x00F59E0B, "✏ MODIFIER (Espace)", 0x00FEF3C7, 1);
        self.editor.ai_patch_modify_hitbox = Some((mod_x, btn_y, mod_w, btn_h));

        // [✕ REJETER (Échap)]
        let rej_w = 135;
        let rej_x = rx + rw - 140;
        canvas.draw_badge(rej_x, btn_y, rej_w, btn_h, 0x004C0519, 0x00EF4444, "✕ REJETER (Échap)", 0x00FFE4E6, 1);
        self.editor.ai_patch_reject_hitbox = Some((rej_x, btn_y, rej_w, btn_h));
    }

    /// Rendu de la fenêtre modale d'Assistant IA Universel (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI, Offline)
    fn render_intent_modal(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = 840;
        let mh = 560;
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        self.editor.intent_preset_hitboxes.clear();
        self.editor.ai_provider_btn_hitboxes.clear();

        // Ombrage et cadre Dark Atelier
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000F172A);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x00A855F7, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 20, 0x0038BDF8);

        // Barre d'en-tête
        canvas.draw_rect(mx, my, mw as u32, 42, 0x002E1065);
        canvas.draw_text(mx + 18, my + 12, "✨ ASSISTANT IA UNIVERSEL (OLLAMA, LM STUDIO, CLAUDE, GEMINI, DEEPSEEK)", 0x00F3E8FF, 2);
        canvas.draw_badge(mx + mw - 100, my + 8, 85, 26, 0x003B1D66, 0x00A855F7, "[ÉCHAP]", 0x00F3E8FF, 1);

        // Section 0 : Sélecteur de Modèle / Fournisseur IA
        canvas.draw_text(mx + 20, my + 50, "CHOISIR LE MODÈLE D'IA (LOCAL OU CLOUD) :", 0x0094A3B8, 1);

        let providers = crate::ai_client::AiProvider::ALL;
        let prov_gap = 6;
        let prov_w = (mw - 40 - (providers.len() as i32 - 1) * prov_gap) / providers.len() as i32;
        let prov_h = 28;
        let prov_y = my + 66;

        for (idx, &prov) in providers.iter().enumerate() {
            let px = mx + 20 + idx as i32 * (prov_w + prov_gap);
            let is_selected = self.editor.ai_config.provider == prov;
            let is_hov = is_selected || (self.editor.mouse_x >= px as f32 && self.editor.mouse_x <= (px + prov_w) as f32 && self.editor.mouse_y >= prov_y as f32 && self.editor.mouse_y <= (prov_y + prov_h) as f32);

            let (bg, bd, fg) = if is_selected {
                (0x003B0764, 0x00C084FC, 0x00FAF5FF)
            } else if is_hov {
                (0x001E293B, 0x0038BDF8, 0x00F0F9FF)
            } else {
                (0x000F172A, 0x00334155, 0x0064748B)
            };

            canvas.draw_badge(px, prov_y, prov_w as u32, prov_h as u32, bg, bd, prov.short_label(), fg, 1);
            self.editor.ai_provider_btn_hitboxes.push((prov, px, prov_y, prov_w as u32, prov_h as u32));
        }

        // Ligne d'état du fournisseur actif
        let active_prov = self.editor.ai_config.provider;
        let status_info = match active_prov {
            crate::ai_client::AiProvider::Ollama => "🦙 Ollama Local • http://localhost:11434 (qwen2.5-coder / llama3) • Zéro clé requise",
            crate::ai_client::AiProvider::LmStudio => "🏢 LM Studio Local • http://localhost:1234 (Modèle actif en mémoire) • Zéro clé requise",
            crate::ai_client::AiProvider::Claude => "🤖 Claude (Anthropic) • claude-3-5-sonnet • Clé requise : ANTHROPIC_API_KEY",
            crate::ai_client::AiProvider::Gemini => "🔮 Google Gemini • gemini-1.5-flash • Clé requise : GEMINI_API_KEY",
            crate::ai_client::AiProvider::DeepSeek => "⚡ DeepSeek • deepseek-chat • Clé requise : DEEPSEEK_API_KEY",
            crate::ai_client::AiProvider::OpenAI => "🟢 OpenAI • gpt-4o-mini • Clé requise : OPENAI_API_KEY",
            crate::ai_client::AiProvider::OfflineLocal => "⚡ Moteur Local Intégré • 100% Autonome, Déterministe et Hors-ligne",
        };

        let key_ok = active_prov.is_ready();
        let key_tag = if key_ok { " [ ✓ PRÊT ]" } else { " [ ⚠️ CLÉ ABSENTE ]" };
        let key_col = if key_ok { 0x0010B981 } else { 0x00F59E0B };

        // Affichage dynamique du statut ou du résultat du test de connexion
        let (display_status, status_col) = if let Some((ok, ref msg)) = self.editor.ai_test_result {
            (msg.as_str(), if ok { 0x0010B981 } else { 0x00EF4444 })
        } else {
            (status_info, 0x00CBD5E1)
        };

        canvas.draw_text(mx + 20, my + 102, display_status, status_col, 1);

        // Bouton interactif pour tester la connexion immédiatement
        let test_btn_w = 110;
        let test_btn_h = 20;
        let test_btn_x = mx + mw - test_btn_w - 20;
        let test_btn_y = my + 98;
        canvas.draw_badge(test_btn_x, test_btn_y, test_btn_w as u32, test_btn_h as u32, 0x001E293B, 0x0038BDF8, "🔌 TESTER", 0x0038BDF8, 1);
        self.editor.ai_test_btn_hitbox = Some((test_btn_x, test_btn_y, test_btn_w as u32, test_btn_h as u32));

        if self.editor.ai_test_result.is_none() {
            canvas.draw_text(test_btn_x - 110, my + 102, key_tag, key_col, 1);
        }

        // Section 1 : Champ de saisie d'intention libre
        canvas.draw_text(mx + 20, my + 124, "INTENTION EN LANGAGE NATUREL :", 0x0038BDF8, 1);

        // Compteur de caractères dynamique (jusqu'à 400 car.)
        let char_count = self.editor.intent_input_text.chars().count();
        let count_txt = format!("{}/400 car.", char_count);
        let count_col = if char_count >= 380 { 0x00EF4444 } else if char_count > 0 { 0x0038BDF8 } else { 0x0064748B };

        // Badge cliquable d'adaptation disposition Clavier (AZERTY Français / QWERTY)
        let kbd_txt = format!("[CLAVIER : {} (CLIC)]", self.editor.keyboard_layout.label());
        let kbd_w = 180;
        let kbd_x = mx + mw - 200 - kbd_w;
        canvas.draw_badge(kbd_x, my + 118, kbd_w as u32, 20, 0x001E293B, 0x00A855F7, &kbd_txt, 0x00F3E8FF, 1);
        self.editor.keyboard_layout_hitbox = Some((kbd_x, my + 118, kbd_w as u32, 20));

        canvas.draw_text(kbd_x - 96, my + 124, &count_txt, count_col, 1);

        let input_box_w = mw - 220;
        let input_box_h = 36;
        let input_box_x = mx + 20;
        let input_box_y = my + 140;

        canvas.draw_rect(input_box_x, input_box_y, input_box_w as u32, input_box_h as u32, 0x000B0F19);
        canvas.draw_rect_outline(input_box_x, input_box_y, input_box_w as u32, input_box_h as u32, 0x0038BDF8, 2);

        if self.editor.ai_generation_in_progress {
            let blink = (self.elapsed_time * 5.0).sin() > 0.0;
            let col = if blink { 0x00F59E0B } else { 0x00D97706 };
            canvas.draw_text(input_box_x + 12, input_box_y + 11, &self.editor.ai_status_text, col, 1);
        } else if self.editor.intent_input_text.is_empty() {
            canvas.draw_text(input_box_x + 12, input_box_y + 11, "Tapez votre intention en français (ex: 'robot ninja avec shurikens')...", 0x0064748B, 1);
        } else {
            let full_text = &self.editor.intent_input_text;
            let c_count = full_text.chars().count();
            let display_slice: String = if c_count > 58 {
                format!("...{}", full_text.chars().skip(c_count - 58).collect::<String>())
            } else {
                full_text.clone()
            };
            let cursor = if (self.elapsed_time * 3.0).fract() > 0.5 { "_" } else { " " };
            let display_txt = format!("> {}{}", display_slice, cursor);
            canvas.draw_text(input_box_x + 12, input_box_y + 11, &display_txt, 0x00F3E8FF, 1);
        }

        // Bouton de soumission du prompt
        let submit_w = 170;
        let submit_x = mx + mw - submit_w - 20;
        let submit_txt = if self.editor.ai_generation_in_progress { "GÉNÉRATION..." } else { "GÉNÉRER AVEC L'IA" };
        let submit_bg = if self.editor.ai_generation_in_progress { 0x0078350F } else { 0x004C1D95 };
        canvas.draw_badge(submit_x, input_box_y, submit_w as u32, input_box_h as u32, submit_bg, 0x00A855F7, submit_txt, 0x00F3E8FF, 1);
        self.editor.intent_submit_btn_hitbox = Some((submit_x, input_box_y, submit_w as u32, input_box_h as u32));

        // Section 2 : Suggestions rapides
        canvas.draw_text(mx + 20, my + 188, "OU CHOISISSEZ UNE SUGGESTION RAPIDE DU STUDIO :", 0x00CBD5E1, 1);

        let presets = [
            ("🛡️ Garde Patrouilleur A* & Alerte", "Ajoute un garde hostile avec vision conique, alerte sonore et poursuite A* vers le joueur."),
            ("📦 Coffre au Trésor & Clé de Donjon", "Génère un coffre scellé nécessitant la clé dorée et conférant 150 points de score."),
            ("⚠️ Zone de Lave & Piège Sismique", "Place une zone de lave ardente infligeant 35 dégâts au contact avec étincelles continues."),
            ("🌀 Portail Dimensionnel Quantique", "Crée un portail téléportant le joueur avec distorsion arcanique et audio spatial."),
        ];

        for (idx, &(title, desc)) in presets.iter().enumerate() {
            let py = my + 206 + (idx as i32 * 68);
            let pw = mw - 40;
            let ph = 58;

            canvas.draw_rect(mx + 20, py, pw as u32, ph as u32, 0x001E293B);
            canvas.draw_rect_outline(mx + 20, py, pw as u32, ph as u32, 0x00475569, 1);

            canvas.draw_text(mx + 34, py + 10, title, 0x0038BDF8, 1);
            canvas.draw_text_body(mx + 34, py + 30, desc, 0x0094A3B8, 12.0);

            canvas.draw_badge(mx + pw - 90, py + 14, 90, 30, 0x002E1065, 0x00A855F7, "GÉNÉRER", 0x00F3E8FF, 1);

            self.editor.intent_preset_hitboxes.push((idx, mx + 20, py, pw as u32, ph as u32));
        }

        // Bouton de fermeture
        let close_y = my + mh - 42;
        canvas.draw_badge(mx + 20, close_y, (mw - 40) as u32, 30, 0x001E293B, 0x0064748B, "[ ANNULER ET RETOURNER À L'ATELIER (ÉCHAP) ]", 0x0094A3B8, 1);
        self.editor.intent_close_btn_hitbox = Some((mx + 20, close_y, (mw - 40) as u32, 30));
    }

    /// Exécute l'exportation du jeu selon le format sélectionné dans le dossier export/
    pub fn perform_export(&mut self) {
        let title = self.editor.export_title.clone();
        let author = self.editor.export_author.clone();
        let fmt = self.editor.export_format_idx;

        // Création du répertoire d'export local
        let _ = std::fs::create_dir_all("export");

        match fmt {
            0 => {
                // 1. Export Web HTML5 Standalone (fichier autonome unique)
                let html_content = crate::exporter::StandaloneExporter::export_html(self, &title, &author);
                let out_path = "export/standalone_game.html";
                if std::fs::write(out_path, &html_content).is_ok() {
                    self.set_status(&format!("✨ Jeu exporté en HTML5 autonome dans '{}' (Prêt à jouer) !", out_path));
                } else {
                    self.set_status(&format!("✨ Export HTML5 prêt en mémoire : {} Ko !", html_content.len() / 1024));
                }
            }
            1 => {
                // 2. Export Package universel .karacti.json
                let json_content = crate::exporter::StandaloneExporter::export_package_json(self, &title, &author)
                    .unwrap_or_else(|_| "{}".to_string());
                let out_path = "export/game.karacti.json";
                if std::fs::write(out_path, &json_content).is_ok() {
                    self.set_status(&format!("💾 Package complet exporté dans '{}' ({} Ko) !", out_path, json_content.len() / 1024));
                } else {
                    self.set_status("💾 Package universel sérialisé avec succès !");
                }
            }
            _ => {
                // 3. Export Dossier Bundle Desktop avec lanceurs autonomes
                let _ = std::fs::create_dir_all("export/desktop_bundle");
                let html_content = crate::exporter::StandaloneExporter::export_html(self, &title, &author);
                let _ = std::fs::write("export/desktop_bundle/index.html", &html_content);
                let runner_script = "#!/bin/bash\necho 'Lancement de KaractiRage Standalone...'\nopen index.html || xdg-open index.html\n";
                let _ = std::fs::write("export/desktop_bundle/lancer_jeu.sh", runner_script);
                self.set_status("🖥️ Bundle Desktop distribuable généré dans 'export/desktop_bundle/' !");
            }
        }

        self.editor.show_export_modal = false;
        self.audio.play(SoundType::QuestSuccess);
    }

    /// Rendu de la fenêtre modale d'Exportation Standalone (Option 3 : Packageur Web & Desktop 1 Clic)
    fn render_export_modal(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = 840;
        let mh = 540;
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        self.editor.export_format_hitboxes.clear();

        // Ombrage d'arrière-plan et cadre Dark Atelier
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000D1322);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x000284C7, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 22, 0x0038BDF8);

        // Bandeau d'en-tête
        canvas.draw_rect(mx, my, mw as u32, 42, 0x00082F49);
        canvas.draw_text(mx + 18, my + 12, "📦 EXPORTATEUR STANDALONE — PACKAGING DU JEU EN 1 CLIC", 0x00BAE6FD, 2);
        canvas.draw_badge(mx + mw - 100, my + 8, 85, 26, 0x000C4A6E, 0x0038BDF8, "[ÉCHAP]", 0x00FFFFFF, 1);
        self.editor.export_close_btn_hitbox = Some((mx + mw - 100, my + 8, 85, 26));

        // Section 1 : Métadonnées et Informations de la Scène
        let info_y = my + 56;
        canvas.draw_text(mx + 24, info_y, "1. Métadonnées du jeu à packager :", 0x0038BDF8, 1);

        let info_box_y = info_y + 18;
        canvas.draw_rect(mx + 24, info_box_y, (mw - 48) as u32, 70, 0x00131C2E);
        canvas.draw_rect_outline(mx + 24, info_box_y, (mw - 48) as u32, 70, 0x001E293B, 1);

        let title_label = format!("TITRE : {}", self.editor.export_title);
        canvas.draw_text(mx + 38, info_box_y + 12, &title_label, 0x00F8FAFC, 2);
        let author_label = format!("AUTEUR : {}  •  VERSION : 1.0.0", self.editor.export_author);
        canvas.draw_text(mx + 38, info_box_y + 30, &author_label, 0x0094A3B8, 1);

        let stats_label = format!("SCÈNE : {}  •  ENTITÉS : {}  •  GRAPHES KVL : {}  •  CIBLE : 60 FPS",
            self.scene_name, self.entities.len(), self.kvl_graphs.len());
        canvas.draw_text(mx + 38, info_box_y + 48, &stats_label, 0x0034D399, 1);

        // Section 2 : Sélection du Format de Distribution
        let fmt_title_y = info_box_y + 86;
        canvas.draw_text(mx + 24, fmt_title_y, "2. Choisissez le format de distribution souhaité :", 0x00CBD5E1, 1);

        let formats = [
            (
                "📦 1. WEB HTML5 AUTONOME (1 Clic)",
                "Fichier unique prêt à jouer hébergeable sur Itch.io / Web ou exécutable en local.",
                "standalone_game.html",
                0x000284C7
            ),
            (
                "💾 2. PACKAGE JEU COMPLET (.karacti.json)",
                "Sérialisation complète de la scène, des entités et des graphes KVL pour sauvegarde et partage.",
                "game.karacti.json",
                0x0010B981
            ),
            (
                "🖥️ 3. BUNDLE DESKTOP LOCAL DISTRIBUABLE",
                "Dossier autonome prêt avec scripts lanceurs (Linux / Mac / Windows).",
                "export/desktop_bundle/",
                0x00A855F7
            ),
        ];

        for (idx, (title, desc, filename, theme_col)) in formats.iter().enumerate() {
            let fy = fmt_title_y + 20 + (idx as i32 * 74);
            let is_selected = self.editor.export_format_idx == idx;

            let card_bg = if is_selected { 0x001E293B } else { 0x000F172A };
            let card_bd = if is_selected { *theme_col } else { 0x00334155 };
            let card_bw = if is_selected { 2 } else { 1 };

            canvas.draw_rect(mx + 24, fy, (mw - 48) as u32, 64, card_bg);
            canvas.draw_rect_outline(mx + 24, fy, (mw - 48) as u32, 64, card_bd, card_bw);

            let prefix = if is_selected { "✓ " } else { "○ " };
            let head_text = format!("{}{}", prefix, title);
            canvas.draw_text(mx + 38, fy + 12, &head_text, if is_selected { *theme_col } else { 0x00F8FAFC }, 2);
            canvas.draw_text_body(mx + 38, fy + 32, desc, 0x0094A3B8, 12.0);

            // Badge du nom de fichier généré
            canvas.draw_badge(mx + mw - 230, fy + 16, 190, 28, 0x000B0F19, card_bd, filename, 0x00F1F5F9, 1);

            self.editor.export_format_hitboxes.push((idx, mx + 24, fy, (mw - 48) as u32, 64));
        }

        // Section 3 : Boutons d'Action Inférieurs
        let action_y = my + mh - 58;
        let action_w = 340;
        let action_x = mx + (mw - action_w) / 2;

        canvas.draw_badge(action_x, action_y, action_w as u32, 42, 0x00065F46, 0x0010B981, "✨ EXPORTER LE JEU MAINTENANT (Entrée)", 0x00ECFDF5, 1);
        self.editor.export_action_btn_hitbox = Some((action_x, action_y, action_w as u32, 42));
    }

    /// Applique la recette active dans le jeu et démarre immédiatement la session
    pub fn apply_active_recipe(&mut self) {
        let (scene, graphs) = crate::recipe::RecipeProjectGenerator::generate(&self.editor.recipe_wizard.recipe);
        self.scene_name = scene.name;
        self.entities = scene.entities;
        self.kvl_graphs = graphs;
        let player_pos = self.player_entity().map(|p| (p.transform.x, p.transform.y));
        if let Some(pos) = player_pos {
            self.player_spawn_pos = pos;
            self.camera.target_x = pos.0;
            self.camera.target_y = pos.1;
            self.camera.snap_to_target();
        }
        self.editor.recipe_wizard.close();
        self.editor.mode = crate::editor::EditorMode::Play;
        self.audio.play(crate::audio::SoundType::QuestSuccess);
        self.set_status(&format!("🚀 Recette instanciée : '{}' (Niveau 1 prêt à jouer)", self.scene_name));
    }

    /// Rendu de la fenêtre modale de l'Assistant de Recette de Jeu en 6 Étapes (Desktop Native Game Recipe Wizard)
    fn render_game_recipe_wizard(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = 940;
        let mh = 620;
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        // Reset hitboxes
        self.editor.recipe_wizard.step_tab_hitboxes.clear();
        self.editor.recipe_wizard.archetype_hitboxes.clear();
        self.editor.recipe_wizard.genre_card_hitboxes.clear();
        self.editor.recipe_wizard.dimension_hitboxes.clear();
        self.editor.recipe_wizard.camera_hitboxes.clear();
        self.editor.recipe_wizard.movement_hitboxes.clear();
        self.editor.recipe_wizard.perf_hitboxes.clear();

        // 1. Fond et encadrement Dark Atelier
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000D1322);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x007C3AED, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 22, 0x00C084FC);

        // 2. Bandeau supérieur d'en-tête
        canvas.draw_rect(mx, my, mw as u32, 46, 0x001B0E3A);
        canvas.draw_badge(mx + 16, my + 10, 36, 26, 0x003B0764, 0x00A855F7, "KR", 0x00F3E8FF, 2);
        canvas.draw_text(mx + 60, my + 14, "🪄 ASSISTANT DE RECETTE DE JEU — CRÉATEUR EN 6 ÉTAPES", 0x00F3E8FF, 2);
        canvas.draw_badge(mx + mw - 95, my + 10, 80, 26, 0x002E1065, 0x00A855F7, "[ÉCHAP]", 0x00FFFFFF, 1);
        self.editor.recipe_wizard.close_btn_hitbox = Some((mx + mw - 95, my + 10, 80, 26));

        // 3. Barre des 6 étapes (Stepper Tabs)
        let tab_bar_y = my + 46;
        canvas.draw_rect(mx, tab_bar_y, mw as u32, 38, 0x000B0F19);
        canvas.draw_line(mx, tab_bar_y + 38, mx + mw, tab_bar_y + 38, 0x001E293B);

        let steps_info = [
            (1, "1. ARCHETYPES"),
            (2, "2. DIMENSION"),
            (3, "3. JOUEUR"),
            (4, "4. MONDE"),
            (5, "5. GAMEPLAY"),
            (6, "6. VALIDATION"),
        ];

        let tab_w = mw / 6;
        for (step_num, title) in steps_info {
            let tx = mx + (step_num as i32 - 1) * tab_w;
            let is_active = self.editor.recipe_wizard.step == step_num;
            let is_done = self.editor.recipe_wizard.step > step_num;

            let tab_bg = if is_active { 0x003B0764 } else if is_done { 0x00131C2E } else { 0x000B0F19 };
            let tab_text_col = if is_active { 0x00F3E8FF } else if is_done { 0x0034D399 } else { 0x0064748B };

            canvas.draw_rect(tx, tab_bar_y, tab_w as u32, 38, tab_bg);
            if is_active {
                canvas.draw_line(tx, tab_bar_y + 36, tx + tab_w, tab_bar_y + 36, 0x00C084FC);
            }
            canvas.draw_badge(tx + 4, tab_bar_y + 6, (tab_w - 8) as u32, 26, tab_bg, if is_active { 0x00A855F7 } else { 0x001E293B }, title, tab_text_col, 1);
            self.editor.recipe_wizard.step_tab_hitboxes.push((step_num, tx, tab_bar_y, tab_w as u32, 38));
        }

        // Zone de contenu principale (y: my + 94 .. my + mh - 54)
        let cy = my + 94;
        let cw = mw - 40;
        let cx = mx + 20;

        match self.editor.recipe_wizard.step {
            1 => self.render_recipe_step_1(canvas, cx, cy, cw),
            2 => self.render_recipe_step_2(canvas, cx, cy, cw),
            3 => self.render_recipe_step_3(canvas, cx, cy, cw),
            4 => self.render_recipe_step_4(canvas, cx, cy, cw),
            5 => self.render_recipe_step_5(canvas, cx, cy, cw),
            6 => self.render_recipe_step_6(canvas, cx, cy, cw),
            _ => {}
        }

        // 4. Barre d'action inférieure (Footer Navigation & Execution)
        let footer_y = my + mh - 54;
        canvas.draw_rect(mx, footer_y, mw as u32, 54, 0x000B0F19);
        canvas.draw_line(mx, footer_y, mx + mw, footer_y, 0x001E293B);

        // Bouton Précédent
        if self.editor.recipe_wizard.step > 1 {
            canvas.draw_badge(mx + 20, footer_y + 11, 130, 32, 0x001E293B, 0x0064748B, "◀ PRÉCÉDENT", 0x00CBD5E1, 1);
            self.editor.recipe_wizard.prev_btn_hitbox = Some((mx + 20, footer_y + 11, 130, 32));
        } else {
            self.editor.recipe_wizard.prev_btn_hitbox = None;
        }

        // Bouton Suivant
        if self.editor.recipe_wizard.step < 6 {
            canvas.draw_badge(mx + 160, footer_y + 11, 130, 32, 0x002E1065, 0x00A855F7, "SUIVANT ▶", 0x00F3E8FF, 1);
            self.editor.recipe_wizard.next_btn_hitbox = Some((mx + 160, footer_y + 11, 130, 32));
        } else {
            self.editor.recipe_wizard.next_btn_hitbox = None;
        }

        // Bouton Exporter Recette .json
        let exp_w = 190;
        let exp_x = mx + mw - 190 - 20 - 240;
        canvas.draw_badge(exp_x, footer_y + 11, exp_w as u32, 32, 0x00082F49, 0x000284C7, "💾 EXPORTER RECETTE", 0x00BAE6FD, 1);
        self.editor.recipe_wizard.export_recipe_btn_hitbox = Some((exp_x, footer_y + 11, exp_w as u32, 32));

        // Bouton Ultime : INSTANCIER ET JOUER LE JEU
        let apply_w = 230;
        let apply_x = mx + mw - apply_w - 20;
        let is_valid = self.editor.recipe_wizard.validation_report.as_ref().map_or(true, |r| r.is_valid);
        let apply_bg = if is_valid { 0x00065F46 } else { 0x007F1D1D };
        let apply_border = if is_valid { 0x0010B981 } else { 0x00EF4444 };
        let apply_txt = if is_valid { "🚀 INSTANCIER & JOUER" } else { "⚠️ INSTANCIER (AVERT.)" };
        canvas.draw_badge(apply_x, footer_y + 11, apply_w as u32, 32, apply_bg, apply_border, apply_txt, 0x00ECFDF5, 1);
        self.editor.recipe_wizard.apply_btn_hitbox = Some((apply_x, footer_y + 11, apply_w as u32, 32));
    }

    fn render_recipe_step_1(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32) {
        canvas.draw_text(cx, cy, "1. PRÉSÉLECTIONS D'ARCHETYPES PRÊTS À L'EMPLOI (1 CLIC) :", 0x00CBD5E1, 1);

        let archetypes = [
            ("dungeon", "🏰 DONJON", "Aventure & Salles", "Top-down, 4 dir, clés, coffres", 0x00A855F7),
            ("platformer", "🏃 PLATFORMER", "Cyber Saut 2D", "Gravité, saut, dash, pièges", 0x0038BDF8),
            ("shootemup", "🚀 SHMUP", "Stell. Spatial", "Inertie, projectiles, vagues", 0x00F59E0B),
            ("sokoban", "📦 SOKOBAN", "Puzzle & Grille", "Grille, caisses, dalles, portes", 0x0010B981),
            ("boss", "👹 TITAN", "Arène du Titan", "Combat mêlée, télégraphie, 8 HP", 0x00EF4444),
        ];

        let card_w = (cw - 40) / 5;
        let card_h = 100;
        let card_y = cy + 18;

        for (idx, &(key, title, subtitle, desc, theme_col)) in archetypes.iter().enumerate() {
            let kx = cx + idx as i32 * (card_w + 10);
            let is_sel = self.editor.recipe_wizard.selected_archetype == Some(key);

            let bg = if is_sel { 0x002E1065 } else { 0x00131C2E };
            let border = if is_sel { theme_col } else { 0x00334155 };
            let bw = if is_sel { 2 } else { 1 };

            canvas.draw_rect(kx, card_y, card_w as u32, card_h as u32, bg);
            canvas.draw_rect_outline(kx, card_y, card_w as u32, card_h as u32, border, bw);

            canvas.draw_badge(kx + 10, card_y + 10, (card_w - 20) as u32, 24, if is_sel { 0x003B0764 } else { 0x000F172A }, border, title, if is_sel { 0x00F3E8FF } else { theme_col }, 1);
            canvas.draw_text_body(kx + 10, card_y + 40, subtitle, 0x00E2E8F0, 11.0);
            canvas.draw_text_body(kx + 10, card_y + 62, desc, 0x0094A3B8, 10.0);

            self.editor.recipe_wizard.archetype_hitboxes.push((key, kx, card_y, card_w as u32, card_h as u32));
        }

        // Section 2 : 12 Genres fondamentaux
        let genre_title_y = card_y + card_h + 16;
        canvas.draw_text(cx, genre_title_y, "2. OU CHOISISSEZ LE GENRE PRINCIPAL DU PROJET :", 0x00CBD5E1, 1);

        let genres = [
            GameGenre::Adventure, GameGenre::Action, GameGenre::ShootEmUp, GameGenre::Platformer,
            GameGenre::Rpg, GameGenre::Simulation, GameGenre::Strategy, GameGenre::Racing,
            GameGenre::Puzzle, GameGenre::Horror, GameGenre::Combat, GameGenre::Educational,
        ];

        let gw = (cw - 30) / 4;
        let gh = 38;
        let gy_start = genre_title_y + 18;

        for (idx, &genre) in genres.iter().enumerate() {
            let row = idx / 4;
            let col = idx % 4;
            let gx = cx + col as i32 * (gw + 10);
            let gy = gy_start + row as i32 * (gh + 8);

            let is_sel = self.editor.recipe_wizard.recipe.genres.primary == genre;
            let bg = if is_sel { 0x003B0764 } else { 0x00131C2E };
            let border = if is_sel { 0x00A855F7 } else { 0x00334155 };
            let txt_col = if is_sel { 0x00F3E8FF } else { 0x0094A3B8 };

            let label = format!("{} {}", if is_sel { "✓" } else { "○" }, genre.label());
            canvas.draw_badge(gx, gy, gw as u32, gh as u32, bg, border, &label, txt_col, 1);

            self.editor.recipe_wizard.genre_card_hitboxes.push((genre, gx, gy, gw as u32, gh as u32));
        }

        // Info récapitulatif
        let info_y = gy_start + 3 * (gh + 8) + 16;
        let info_txt = format!("PROJET : '{}'  •  AUTEUR : '{}'  •  GENRE ACTIF : {}",
            self.editor.recipe_wizard.recipe.title,
            self.editor.recipe_wizard.recipe.author,
            self.editor.recipe_wizard.recipe.genres.primary.label()
        );
        canvas.draw_badge(cx, info_y, cw as u32, 32, 0x000F172A, 0x001E293B, &info_txt, 0x0038BDF8, 1);
    }

    fn render_recipe_step_2(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32) {
        canvas.draw_text(cx, cy, "1. DIMENSION SPATIALE DU MOTEUR (ESPACE GRAPHIQUE) :", 0x00CBD5E1, 1);

        let dimensions = [
            (GameDimension::TwoD, "2D PURE (Pixel Art / Orthogonal)", "Rendu logiciel 2D 60 FPS constants sans accélération GPU."),
            (GameDimension::TwoAndHalfD, "2.5D ISOMÉTRIQUE (Axonométrique)", "Vue diagonale avec profondeur Z et tri de calques visuels."),
            (GameDimension::ThreeD, "3D PERSPECTIVE (Projet)", "Caméra perspective avec matrices de projection et ombrage."),
        ];

        let dim_w = (cw - 20) / 3;
        let dim_h = 76;
        let dim_y = cy + 18;

        for (idx, &(dim, title, desc)) in dimensions.iter().enumerate() {
            let dx = cx + idx as i32 * (dim_w + 10);
            let is_sel = self.editor.recipe_wizard.recipe.dimension == dim;

            let bg = if is_sel { 0x002E1065 } else { 0x00131C2E };
            let border = if is_sel { 0x00A855F7 } else { 0x00334155 };

            canvas.draw_rect(dx, dim_y, dim_w as u32, dim_h as u32, bg);
            canvas.draw_rect_outline(dx, dim_y, dim_w as u32, dim_h as u32, border, if is_sel { 2 } else { 1 });

            let prefix = if is_sel { "✓ " } else { "○ " };
            canvas.draw_text_body(dx + 12, dim_y + 12, &format!("{}{}", prefix, title), if is_sel { 0x00F3E8FF } else { 0x00CBD5E1 }, 12.0);
            canvas.draw_text_body(dx + 12, dim_y + 36, desc, 0x0094A3B8, 10.5);

            self.editor.recipe_wizard.dimension_hitboxes.push((dim, dx, dim_y, dim_w as u32, dim_h as u32));
        }

        // Section 2 : Comportement de la caméra
        let cam_title_y = dim_y + dim_h + 20;
        canvas.draw_text(cx, cam_title_y, "2. COMPORTEMENT DE LA CAMÉRA DANS LE MONDE :", 0x00CBD5E1, 1);

        let cameras = [
            (CameraType::FollowPlayer, "Suivi Dynamique du Joueur", "La caméra centre automatiquement le héros avec lissage fluide (Lerp/Deadzone)."),
            (CameraType::Fixed, "Caméra Fixe par Salle / Écran", "La caméra reste immobile dans l'arène ou saute de salle en salle (Zelda 1 / Sokoban)."),
            (CameraType::AutoScrollHorizontal, "Défilement Forcé Horizontal", "Avance continue obligatoire vers la droite (Endless runner ou Shmup horizontal)."),
            (CameraType::AutoScrollVertical, "Défilement Forcé Vertical", "Avance continue arcade de bas en haut (Shmup arcade classique 1942)."),
        ];

        let cam_w = (cw - 15) / 2;
        let cam_h = 74;
        let cam_y_start = cam_title_y + 18;

        for (idx, &(cam, title, desc)) in cameras.iter().enumerate() {
            let row = idx / 2;
            let col = idx % 2;
            let kx = cx + col as i32 * (cam_w + 15);
            let ky = cam_y_start + row as i32 * (cam_h + 12);

            let is_sel = self.editor.recipe_wizard.recipe.camera == cam;
            let bg = if is_sel { 0x00082F49 } else { 0x00131C2E };
            let border = if is_sel { 0x0038BDF8 } else { 0x00334155 };

            canvas.draw_rect(kx, ky, cam_w as u32, cam_h as u32, bg);
            canvas.draw_rect_outline(kx, ky, cam_w as u32, cam_h as u32, border, if is_sel { 2 } else { 1 });

            let prefix = if is_sel { "✓ " } else { "○ " };
            canvas.draw_text_body(kx + 12, ky + 12, &format!("{}{}", prefix, title), if is_sel { 0x00BAE6FD } else { 0x00CBD5E1 }, 12.0);
            canvas.draw_text_body(kx + 12, ky + 34, desc, 0x0094A3B8, 10.5);

            self.editor.recipe_wizard.camera_hitboxes.push((cam, kx, ky, cam_w as u32, cam_h as u32));
        }
    }

    fn render_recipe_step_3(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32) {
        canvas.draw_text(cx, cy, "1. TOPOLOGIE DES DÉPLACEMENTS DU JOUEUR :", 0x00CBD5E1, 1);

        let movements = [
            (MovementDirections::FourDirections, "4 Directions", "Haut, Bas, Gauche, Droite.", 0x00A855F7),
            (MovementDirections::EightDirections, "8 Directions", "Omnidirectionnel libre.", 0x0038BDF8),
            (MovementDirections::HorizontalOnly, "Horizontal 1D", "Gauche/Droite (Plateforme).", 0x00F59E0B),
            (MovementDirections::FreeInertia, "Inertie", "Poussée spatiale Newtonienne.", 0x0010B981),
            (MovementDirections::Grid, "Grille Pas-à-Pas", "Case par case (Sokoban).", 0x00EC4899),
        ];

        let m_w = (cw - 40) / 5;
        let m_h = 76;
        let m_y = cy + 18;

        for (idx, &(mvt, title, desc, theme_col)) in movements.iter().enumerate() {
            let mx = cx + idx as i32 * (m_w + 10);
            let is_sel = self.editor.recipe_wizard.recipe.player.directions == mvt;

            let bg = if is_sel { 0x002E1065 } else { 0x00131C2E };
            let border = if is_sel { theme_col } else { 0x00334155 };

            canvas.draw_rect(mx, m_y, m_w as u32, m_h as u32, bg);
            canvas.draw_rect_outline(mx, m_y, m_w as u32, m_h as u32, border, if is_sel { 2 } else { 1 });

            let prefix = if is_sel { "✓ " } else { "○ " };
            canvas.draw_text_body(mx + 8, m_y + 12, &format!("{}{}", prefix, title), if is_sel { 0x00F3E8FF } else { 0x00CBD5E1 }, 12.0);
            canvas.draw_text_body(mx + 8, m_y + 36, desc, 0x0094A3B8, 10.0);

            self.editor.recipe_wizard.movement_hitboxes.push((mvt, mx, m_y, m_w as u32, m_h as u32));
        }

        // Section 2 : Capacités et Style de combat
        let abil_title_y = m_y + m_h + 20;
        canvas.draw_text(cx, abil_title_y, "2. CAPACITÉS ATHLÉTIQUES & STYLE DE COMBAT (CLIQUEZ POUR BASCULER) :", 0x00CBD5E1, 1);

        let jump_active = self.editor.recipe_wizard.recipe.player.abilities.jump;
        let dash_active = self.editor.recipe_wizard.recipe.player.abilities.dash;
        let shoot_active = self.editor.recipe_wizard.recipe.player.combat == CombatStyle::Projectiles;
        let melee_active = self.editor.recipe_wizard.recipe.player.combat == CombatStyle::Melee;
        let interact_active = self.editor.recipe_wizard.recipe.player.abilities.interact;

        let toggles = [
            ("Saut Vertical [Espace]", "Permet de bondir contre la gravité (Plateforme)", jump_active, 0x0038BDF8),
            ("Dash / Esquive [Shift]", "Impulsion rapide pour esquiver tirs et pièges", dash_active, 0x00A855F7),
            ("Tir Projectiles [Clic/F]", "Lance des projectiles à distance avec audio tir", shoot_active, 0x00F59E0B),
            ("Attaque Mêlée [E]", "Coup au corps-à-corps infligeant des dégâts directs", melee_active, 0x00EF4444),
            ("Interagir [E / Clic]", "Ouvrir portes, coffres et actionner leviers", interact_active, 0x0010B981),
        ];

        let tw = (cw - 20) / 3;
        let th = 66;
        let ty_start = abil_title_y + 18;

        for (idx, &(title, desc, active, theme_col)) in toggles.iter().enumerate() {
            let row = idx / 3;
            let col = idx % 3;
            let tx = cx + col as i32 * (tw + 10);
            let ty = ty_start + row as i32 * (th + 10);

            let bg = if active { 0x00131C2E } else { 0x000F172A };
            let border = if active { theme_col } else { 0x00334155 };

            canvas.draw_rect(tx, ty, tw as u32, th as u32, bg);
            canvas.draw_rect_outline(tx, ty, tw as u32, th as u32, border, if active { 2 } else { 1 });

            let prefix = if active { "☑ " } else { "☐ " };
            canvas.draw_text_body(tx + 10, ty + 10, &format!("{}{}", prefix, title), if active { theme_col } else { 0x0094A3B8 }, 12.0);
            canvas.draw_text_body(tx + 10, ty + 32, desc, 0x0064748B, 10.0);

            let hb = Some((tx, ty, tw as u32, th as u32));
            match idx {
                0 => self.editor.recipe_wizard.ability_jump_hitbox = hb,
                1 => self.editor.recipe_wizard.ability_dash_hitbox = hb,
                2 => self.editor.recipe_wizard.ability_shoot_hitbox = hb,
                3 => self.editor.recipe_wizard.ability_melee_hitbox = hb,
                4 => self.editor.recipe_wizard.ability_interact_hitbox = hb,
                _ => {}
            }
        }
    }

    fn render_recipe_step_4(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32) {
        canvas.draw_text(cx, cy, "1. PHYSIQUE DU MONDE ET CONDITIONS ENVIRONNEMENTALES :", 0x00CBD5E1, 1);

        let grav_active = self.editor.recipe_wizard.recipe.world.gravity_enabled;
        let gw_w = (cw - 20) / 3;
        let gw_h = 72;
        let gw_y = cy + 18;

        // Gravité
        let grav_bg = if grav_active { 0x00064E3B } else { 0x00131C2E };
        let grav_bd = if grav_active { 0x0010B981 } else { 0x00334155 };
        canvas.draw_rect(cx, gw_y, gw_w as u32, gw_h as u32, grav_bg);
        canvas.draw_rect_outline(cx, gw_y, gw_w as u32, gw_h as u32, grav_bd, if grav_active { 2 } else { 1 });
        canvas.draw_text_body(cx + 10, gw_y + 10, &format!("{} Gravité Active (980 px/s²)", if grav_active { "☑" } else { "☐" }), if grav_active { 0x00D1FAE5 } else { 0x0094A3B8 }, 12.0);
        canvas.draw_text_body(cx + 10, gw_y + 34, "Indispensable pour jeux de plateforme (saut et chutes physiques).", 0x0094A3B8, 10.0);
        self.editor.recipe_wizard.world_gravity_hitbox = Some((cx, gw_y, gw_w as u32, gw_h as u32));

        // Note information
        let note_x = cx + gw_w + 10;
        let note_w = cw - gw_w - 10;
        canvas.draw_rect(note_x, gw_y, note_w as u32, gw_h as u32, 0x000F172A);
        canvas.draw_rect_outline(note_x, gw_y, note_w as u32, gw_h as u32, 0x001E293B, 1);
        canvas.draw_text_body(note_x + 12, gw_y + 12, "💡 Règle de cohérence R1 du moteur :", 0x0038BDF8, 11.5);
        canvas.draw_text_body(note_x + 12, gw_y + 34, "Si le saut est actif, le résolveur de dépendance active automatiquement la gravité.", 0x0094A3B8, 10.5);

        // Section 2 : Éléments interactifs
        let elem_title_y = gw_y + gw_h + 20;
        canvas.draw_text(cx, elem_title_y, "2. ÉLÉMENTS INTERACTIFS DÉPLOYÉS DANS LE MONDE :", 0x00CBD5E1, 1);

        let elements = &self.editor.recipe_wizard.recipe.world.interactive_elements;
        let has_coin = elements.contains(&InteractiveElement::Collectible);
        let has_door = elements.contains(&InteractiveElement::Door);
        let has_chest = elements.contains(&InteractiveElement::Chest);
        let has_lever = elements.contains(&InteractiveElement::Lever);
        let has_trap = elements.contains(&InteractiveElement::Trap);

        let elem_items = [
            ("🪙 Collectibles (Pièces/Bonus)", "Objets ramassables conférant des points ou recharges.", has_coin, 0x00F59E0B),
            ("🚪 Portes & Clés de Donjon", "Portes solides nécessitant une clé ou la fin d'énigme.", has_door, 0x0038BDF8),
            ("📦 Coffres au Trésor Scellés", "Contient de l'équipement ou potions à ouvrir avec E.", has_chest, 0x00A855F7),
            ("⚡ Leviers & Dalles Pression", "Interrupteurs reliés à des blocs de logique KVL.", has_lever, 0x0010B981),
            ("⚠️ Pièges & Zones de Lave", "Zones de danger causant des dégâts continus.", has_trap, 0x00EF4444),
        ];

        let ew = (cw - 20) / 3;
        let eh = 68;
        let ey_start = elem_title_y + 18;

        for (idx, &(title, desc, active, theme_col)) in elem_items.iter().enumerate() {
            let row = idx / 3;
            let col = idx % 3;
            let ex = cx + col as i32 * (ew + 10);
            let ey = ey_start + row as i32 * (eh + 10);

            let bg = if active { 0x00131C2E } else { 0x000F172A };
            let border = if active { theme_col } else { 0x00334155 };

            canvas.draw_rect(ex, ey, ew as u32, eh as u32, bg);
            canvas.draw_rect_outline(ex, ey, ew as u32, eh as u32, border, if active { 2 } else { 1 });

            let prefix = if active { "☑ " } else { "☐ " };
            canvas.draw_text_body(ex + 10, ey + 10, &format!("{}{}", prefix, title), if active { theme_col } else { 0x0094A3B8 }, 12.0);
            canvas.draw_text_body(ex + 10, ey + 34, desc, 0x0064748B, 10.0);

            let hb = Some((ex, ey, ew as u32, eh as u32));
            match idx {
                0 => self.editor.recipe_wizard.elem_collectible_hitbox = hb,
                1 => self.editor.recipe_wizard.elem_door_hitbox = hb,
                2 => self.editor.recipe_wizard.elem_chest_hitbox = hb,
                3 => self.editor.recipe_wizard.elem_lever_hitbox = hb,
                4 => self.editor.recipe_wizard.elem_trap_hitbox = hb,
                _ => {}
            }
        }
    }

    fn render_recipe_step_5(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32) {
        canvas.draw_text(cx, cy, "1. SOUS-SYSTÈMES DE GAMEPLAY ET LOGIQUE VISUELLE KVL :", 0x00CBD5E1, 1);

        let gp = &self.editor.recipe_wizard.recipe.gameplay;
        let features = [
            ("🎒 Sacoche & Inventaire Hotbar", "Gestion de slots d'objets, clés, fioles et consommables.", gp.inventory, 0x00A855F7),
            ("💬 Boîtes de Dialogue & PNJ", "Dialogues interactifs branchés avec portraits et choix.", gp.dialogues, 0x0038BDF8),
            ("📜 Quêtes & Objectifs de Scène", "Journal de quêtes, suivi d'étapes et notifications.", gp.quest_objectives, 0x0010B981),
            ("👹 Combat de Boss Scénarisé", "Boss avec télégraphie au sol, multi-phases et PV dédiés.", gp.boss_fight, 0x00EF4444),
            ("🏆 Système de Score & Combos", "Calcul de score, multiplicateurs de pièces et alertes sonores.", gp.score_system, 0x00F59E0B),
            ("🌊 Vagues d'Ennemis (Wave Spawner)", "Apparition cyclique d'ennemis patrouilleurs ou volants.", gp.wave_spawner, 0x00EC4899),
        ];

        let fw = (cw - 20) / 3;
        let fh = 68;
        let fy_start = cy + 18;

        for (idx, &(title, desc, active, theme_col)) in features.iter().enumerate() {
            let row = idx / 3;
            let col = idx % 3;
            let fx = cx + col as i32 * (fw + 10);
            let fy = fy_start + row as i32 * (fh + 10);

            let bg = if active { 0x00131C2E } else { 0x000F172A };
            let border = if active { theme_col } else { 0x00334155 };

            canvas.draw_rect(fx, fy, fw as u32, fh as u32, bg);
            canvas.draw_rect_outline(fx, fy, fw as u32, fh as u32, border, if active { 2 } else { 1 });

            let prefix = if active { "☑ " } else { "☐ " };
            canvas.draw_text_body(fx + 10, fy + 10, &format!("{}{}", prefix, title), if active { theme_col } else { 0x0094A3B8 }, 12.0);
            canvas.draw_text_body(fx + 10, fy + 34, desc, 0x0064748B, 10.0);

            let hb = Some((fx, fy, fw as u32, fh as u32));
            match idx {
                0 => self.editor.recipe_wizard.feat_inventory_hitbox = hb,
                1 => self.editor.recipe_wizard.feat_dialogues_hitbox = hb,
                2 => self.editor.recipe_wizard.feat_quests_hitbox = hb,
                3 => self.editor.recipe_wizard.feat_boss_hitbox = hb,
                4 => self.editor.recipe_wizard.feat_score_hitbox = hb,
                5 => self.editor.recipe_wizard.feat_waves_hitbox = hb,
                _ => {}
            }
        }

        // Section 2 : Profil de performance cible
        let perf_title_y = fy_start + 2 * (fh + 10) + 16;
        canvas.draw_text(cx, perf_title_y, "2. PROFIL MATÉRIEL CIBLE (GARANTIE 60 FPS / BUDGET LATENCE) :", 0x00CBD5E1, 1);

        let perfs = [
            (PerformanceProfile::LowEnd, "Low-End (60 FPS Garanti)", "Budget < 16.6ms, zéro alloc par frame, compatible toutes machines."),
            (PerformanceProfile::Balanced, "Équilibré (PC Standard)", "Effets de particules et éclairage doux activés."),
            (PerformanceProfile::HighEnd, "Haut de Gamme (GPU)", "Particules denses et shaders complets."),
            (PerformanceProfile::Mobile, "Mobile / Batterie", "Économie d'énergie et fréquence de rafraîchissement adaptative."),
        ];

        let pw = (cw - 30) / 4;
        let ph = 74;
        let py_start = perf_title_y + 18;

        for (idx, &(perf, title, desc)) in perfs.iter().enumerate() {
            let px = cx + idx as i32 * (pw + 10);
            let is_sel = self.editor.recipe_wizard.recipe.performance == perf;

            let bg = if is_sel { 0x00064E3B } else { 0x00131C2E };
            let border = if is_sel { 0x0010B981 } else { 0x00334155 };

            canvas.draw_rect(px, py_start, pw as u32, ph as u32, bg);
            canvas.draw_rect_outline(px, py_start, pw as u32, ph as u32, border, if is_sel { 2 } else { 1 });

            let prefix = if is_sel { "✓ " } else { "○ " };
            canvas.draw_text_body(px + 8, py_start + 10, &format!("{}{}", prefix, title), if is_sel { 0x00D1FAE5 } else { 0x00CBD5E1 }, 11.5);
            canvas.draw_text_body(px + 8, py_start + 32, desc, 0x0094A3B8, 10.0);

            self.editor.recipe_wizard.perf_hitboxes.push((perf, px, py_start, pw as u32, ph as u32));
        }
    }

    fn render_recipe_step_6(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32) {
        canvas.draw_text(cx, cy, "AUDIT DE COHÉRENCE DU RÉSOLVEUR DE DÉPENDANCES (RÈGLES R1 À R6) :", 0x00CBD5E1, 1);

        let report = self.editor.recipe_wizard.validation_report.clone();
        let is_valid = report.as_ref().map_or(true, |r| r.is_valid);

        // Grande boîte de statut de validation
        let stat_box_y = cy + 18;
        let stat_bg = if is_valid { 0x00064E3B } else { 0x00450A0A };
        let stat_bd = if is_valid { 0x0010B981 } else { 0x00EF4444 };

        canvas.draw_rect(cx, stat_box_y, cw as u32, 70, stat_bg);
        canvas.draw_rect_outline(cx, stat_box_y, cw as u32, 70, stat_bd, 2);

        let badge_txt = if is_valid { "✓ RECETTE PARFAITEMENT COHÉRENTE & VALIDÉE" } else { "⚠️ ATTENTION : RÈGLES DE COHÉRENCE EN ALERTE" };
        canvas.draw_text(cx + 16, stat_box_y + 14, badge_txt, if is_valid { 0x00ECFDF5 } else { 0x00FEE2E2 }, 2);

        let sub_txt = if is_valid {
            "Toutes les dépendances (mouvement, gravité, caméra, logique) sont vérifiées et prêtes à être instanciées !"
        } else {
            "Certains paramètres présentent des incohérences détectées par le résolveur automatique."
        };
        canvas.draw_text_body(cx + 16, stat_box_y + 42, sub_txt, if is_valid { 0x00A7F3D0 } else { 0x00FCA5A5 }, 12.0);

        // Deux colonnes d'informations
        let col_y = stat_box_y + 82;
        let col_w = (cw - 20) / 2;
        let col_h = 175;

        // Colonne 1 : Dépendances auto-résolues
        let c1_x = cx;
        canvas.draw_rect(c1_x, col_y, col_w as u32, col_h as u32, 0x00131C2E);
        canvas.draw_rect_outline(c1_x, col_y, col_w as u32, col_h as u32, 0x001E293B, 1);
        canvas.draw_text_body(c1_x + 14, col_y + 12, "RÉSOLUTIONS AUTOMATIQUES & AJUSTEMENTS :", 0x0038BDF8, 12.0);

        if let Some(rep) = &report {
            if rep.resolved_dependencies.is_empty() {
                canvas.draw_text_body(c1_x + 14, col_y + 38, "• Aucun conflit détecté.", 0x0034D399, 11.0);
                canvas.draw_text_body(c1_x + 14, col_y + 58, "• Les choix sont nativement équilibrés et harmonieux.", 0x0094A3B8, 11.0);
            } else {
                for (d_idx, dep) in rep.resolved_dependencies.iter().enumerate().take(5) {
                    let dy = col_y + 38 + d_idx as i32 * 22;
                    canvas.draw_text_body(c1_x + 14, dy, &format!("✓ {}", dep), 0x0034D399, 11.0);
                }
            }

            if !rep.warnings.is_empty() {
                let wy = col_y + 110;
                canvas.draw_text_body(c1_x + 14, wy, "AVERTISSEMENTS D'OPTIMISATION :", 0x00F59E0B, 11.0);
                for (w_idx, warn) in rep.warnings.iter().enumerate().take(2) {
                    let wy2 = wy + 20 + w_idx as i32 * 20;
                    canvas.draw_text_body(c1_x + 14, wy2, &format!("⚠️ {}", warn), 0x00CBD5E1, 10.5);
                }
            }
        }

        // Colonne 2 : Fiche d'Identité du Projet Généré
        let c2_x = cx + col_w + 20;
        canvas.draw_rect(c2_x, col_y, col_w as u32, col_h as u32, 0x00131C2E);
        canvas.draw_rect_outline(c2_x, col_y, col_w as u32, col_h as u32, 0x001E293B, 1);
        canvas.draw_text_body(c2_x + 14, col_y + 12, "FICHE D'IDENTITÉ DU JEU PRÊT À TOURNER :", 0x00C084FC, 12.0);

        let r = &self.editor.recipe_wizard.recipe;
        let l1 = format!("• TITRE : {}", r.title);
        let l2 = format!("• GENRE : {} (Schéma V{})", r.genres.primary.label(), r.schema_version);
        let l3 = format!("• DIMENSION : {:?}  |  CAMÉRA : {:?}", r.dimension, r.camera);
        let l4 = format!("• DÉPLACEMENT : {:?}", r.player.directions);
        let l5 = format!("• ÉLÉMENTS : {} actifs  |  BOSS : {}", r.world.interactive_elements.len(), if r.gameplay.boss_fight { "OUI" } else { "NON" });
        let l6 = format!("• PROFIL : {:?} (Zéro allocation hot-path)", r.performance);

        canvas.draw_text_body(c2_x + 14, col_y + 36, &l1, 0x00F8FAFC, 11.0);
        canvas.draw_text_body(c2_x + 14, col_y + 58, &l2, 0x00E2E8F0, 11.0);
        canvas.draw_text_body(c2_x + 14, col_y + 80, &l3, 0x00CBD5E1, 11.0);
        canvas.draw_text_body(c2_x + 14, col_y + 102, &l4, 0x00CBD5E1, 11.0);
        canvas.draw_text_body(c2_x + 14, col_y + 124, &l5, 0x00A855F7, 11.0);
        canvas.draw_text_body(c2_x + 14, col_y + 146, &l6, 0x0034D399, 11.0);
    }

    /// Applique les choix de l'Assistant Guidé KVL : construit le graphe KVL typé et l'attache à une entité
    pub fn apply_guided_wizard_mechanic(&mut self) {
        let cat = self.editor.wizard.selected_category;
        let c_choice = self.editor.wizard.condition_choice;
        let a_choice = self.editor.wizard.action_choice;
        let s_choice = self.editor.wizard.sound_choice;

        let sound_name = match s_choice {
            0 => Some("victory_bell".to_string()),
            1 => Some("click_retro".to_string()),
            _ => None,
        };

        let (graph, trigger_event, default_kind) = match cat {
            WizardCategory::Chest => {
                let required_key = match c_choice {
                    0 => Some("key_gold".to_string()),
                    1 => Some("crystal_energy".to_string()),
                    _ => None,
                };
                let (reward_score, reward_item) = match a_choice {
                    0 => (100, Some("potion_soin".to_string())),
                    1 => (250, Some("epee_ancienne".to_string())),
                    _ => (500, None),
                };
                let intent = ChestIntent {
                    chest_name: "Coffre aux Merveilles".to_string(),
                    required_key,
                    reward_score,
                    reward_item,
                    play_sound: sound_name,
                };
                (GuidedBuilder::build_chest(&intent), "OnDoorInteract", "crate")
            }
            WizardCategory::Door => {
                let condition = match c_choice {
                    0 => DoorConditionType::ItemRequired("key_dungeon".to_string()),
                    1 => DoorConditionType::ScoreRequired(200),
                    _ => DoorConditionType::VariableEquals("door_unlocked".to_string(), Value::Bool(true)),
                };
                let intent = DoorIntent {
                    door_name: "Porte de Donjon".to_string(),
                    trigger: DoorTriggerType::OnKeyPress("E".to_string()),
                    condition,
                    toggle: a_choice == 1,
                    play_sound: sound_name,
                };
                (GuidedBuilder::build_door(&intent), "OnDoorInteract", "door")
            }
            WizardCategory::Hazard => {
                let damage_amount = match a_choice {
                    0 => 15,
                    1 => 35,
                    _ => 75,
                };
                let intent = HazardIntent {
                    hazard_name: "Zone Dangereuse".to_string(),
                    damage_amount,
                    play_sound: sound_name,
                };
                (GuidedBuilder::build_hazard(&intent), "OnPlayerOverlap", "hazard")
            }
            WizardCategory::Teleport => {
                let (target_x, target_y) = match c_choice {
                    0 => (100.0, 150.0),
                    1 => (600.0, 350.0),
                    _ => (950.0, 200.0),
                };
                let intent = TeleportIntent {
                    portal_name: "Portail Dimensionnel".to_string(),
                    target_x,
                    target_y,
                    play_sound: sound_name,
                };
                (GuidedBuilder::build_teleport(&intent), "OnPlayerOverlap", "portal")
            }
            WizardCategory::SwitchPlate => {
                let door_var = match a_choice {
                    0 => "door_secret".to_string(),
                    1 => "laser_field".to_string(),
                    _ => "bridge_active".to_string(),
                };
                let intent = SwitchPlateIntent {
                    switch_name: "Dalle Pression".to_string(),
                    door_var_to_open: door_var,
                    play_sound: sound_name,
                };
                (GuidedBuilder::build_switch_plate(&intent), "OnPressurePlate", "plate")
            }
            WizardCategory::Collectible => {
                let score_points = match c_choice {
                    0 => 20,
                    1 => 50,
                    _ => 100,
                };
                let intent = CollectibleIntent {
                    item_name: "Trésor Étincelant".to_string(),
                    score_points,
                    destroy_on_pickup: true,
                    play_sound: sound_name,
                };
                (GuidedBuilder::build_collectible(&intent), "OnPlayerOverlap", "collectible")
            }
        };

        let slug = match cat {
            WizardCategory::Chest => "chest",
            WizardCategory::Door => "door",
            WizardCategory::Hazard => "hazard",
            WizardCategory::Teleport => "teleport",
            WizardCategory::SwitchPlate => "switch",
            WizardCategory::Collectible => "collectible",
        };
        let script_path = format!("assets/scripts/guided_{}_{}.kvl.json", slug, self.entities.len() + 1);
        self.kvl_graphs.insert(script_path.clone(), graph);

        let target_entity_idx = match self.editor.selected {
            SelectedEntity::Entity(idx) if idx < self.entities.len() => idx,
            _ => {
                let spawn_x = (self.camera.x + 360.0).clamp(60.0, 1100.0);
                let spawn_y = (self.camera.y + 240.0).clamp(60.0, 520.0);
                self.spawn_entity(default_kind, spawn_x, spawn_y)
            }
        };

        if let Some(ent) = self.entities.get_mut(target_entity_idx) {
            ent.script = Some(crate::entity::KVLScriptComponent::new(&script_path).with_overlap(trigger_event));
            self.status_message = format!("Mécanique '{}' générée et attachée avec succès à '{}' !", cat.title(), ent.name);
        }
        self.spawn_particles(self.camera.x + 400.0, self.camera.y + 300.0, 25, cat.color());
    }

    /// Rendu de l'Assistant Guidé KVL (Niveau 1 : Continuum Visuel)
    pub fn render_guided_wizard(&mut self, canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
        let mw = (screen_w - 40).min(840);
        let mh = (screen_h - 40).min(580);
        let mx = (screen_w - mw) / 2;
        let my = (screen_h - mh) / 2;

        // Ombrage d'arrière-plan semi-transparent et panneau principal
        canvas.draw_rect(0, 0, screen_w as u32, screen_h as u32, 0x00060913);
        canvas.draw_rect(mx, my, mw as u32, mh as u32, 0x000D1322);
        canvas.draw_rect_outline(mx, my, mw as u32, mh as u32, 0x004F46E5, 2);
        canvas.draw_corner_brackets(mx, my, mw as u32, mh as u32, 24, 0x00818CF8);

        // Barre d'en-tête (Thème Dark Améthyste)
        canvas.draw_rect(mx, my, mw as u32, 42, 0x001E1B4B);
        canvas.draw_text(mx + 16, my + 12, "CREATEUR GUIDE KVL — NIVEAU 1 : INTENTION DE JEU", 0x00E0E7FF, 2);

        // Bouton Fermer [X]
        let close_w = 34;
        let close_h = 26;
        let close_x = mx + mw - 44;
        let close_y = my + 8;
        canvas.draw_badge(close_x, close_y, close_w, close_h, 0x00312E81, 0x00EF4444, "X", 0x00FEE2E2, 1);
        self.editor.wizard.close_btn_hitbox = Some((close_x, close_y, close_w, close_h));

        // Onglets / Étapes horizontales
        let step = self.editor.wizard.step;
        let tab_w = (mw - 40 - 16) / 3;
        let tab_h = 26;
        let tab_y = my + 48;

        let steps_meta = [
            (1, "1. INTENTION"),
            (2, "2. PARAMÈTRES"),
            (3, "3. SYNTHÈSE & DÉPLOIEMENT"),
        ];

        for (s_num, s_label) in steps_meta {
            let tx = mx + 20 + (s_num - 1) * (tab_w + 8);
            let (bg, bd, fg) = if step == s_num as u32 {
                (0x00312E81, 0x00818CF8, 0x00FFFFFF)
            } else if step > s_num as u32 {
                (0x00064E3B, 0x0010B981, 0x00A7F3D0)
            } else {
                (0x000F172A, 0x00334155, 0x0064748B)
            };
            let prefix = if step > s_num as u32 { "[OK] " } else { "" };
            let full_label = format!("{}{}", prefix, s_label);
            canvas.draw_badge(tx, tab_y, tab_w as u32, tab_h, bg, bd, &full_label, fg, 1);
        }

        // Rendu du contenu spécifique à l'étape
        match step {
            1 => self.render_wizard_step_1(canvas, mx, my, mw, mh),
            2 => self.render_wizard_step_2(canvas, mx, my, mw, mh),
            3 => self.render_wizard_step_3(canvas, mx, my, mw, mh),
            _ => {}
        }

        // Barre inférieure de navigation
        let nav_y = my + mh - 48;
        canvas.draw_rect(mx + 16, nav_y - 8, (mw - 32) as u32, 1, 0x001E293B);

        if step > 1 {
            canvas.draw_badge(mx + 20, nav_y, 140, 34, 0x001E293B, 0x00475569, "< PRÉCÉDENT", 0x00CBD5E1, 1);
            self.editor.wizard.prev_btn_hitbox = Some((mx + 20, nav_y, 140, 34));
        } else {
            self.editor.wizard.prev_btn_hitbox = None;
        }

        if step < 3 {
            canvas.draw_badge(mx + mw - 160, nav_y, 140, 34, 0x003730A3, 0x00818CF8, "SUIVANT >", 0x00EEF2FF, 1);
            self.editor.wizard.next_btn_hitbox = Some((mx + mw - 160, nav_y, 140, 34));
        } else {
            self.editor.wizard.next_btn_hitbox = None;
        }
    }

    fn render_wizard_step_1(&mut self, canvas: &mut Canvas, mx: i32, my: i32, mw: i32, _mh: i32) {
        canvas.draw_text(mx + 22, my + 84, "Que voulez-vous créer ? Choisissez une intention de jeu :", 0x00F8FAFC, 1);

        self.editor.wizard.category_hitboxes.clear();
        let cols = 3;
        let card_gap = 12;
        let card_w = (mw - 40 - (cols - 1) * card_gap) / cols;
        let card_h = 160;

        for (idx, &cat) in WizardCategory::all().iter().enumerate() {
            let row = (idx / 3) as i32;
            let col = (idx % 3) as i32;
            let cx = mx + 20 + col * (card_w + card_gap);
            let cy = my + 106 + row * (card_h + 12);
            let is_selected = self.editor.wizard.selected_category == cat;

            let (bg, bd) = if is_selected {
                (0x001E293B, cat.color())
            } else {
                (0x000F172A, 0x00334155)
            };
            canvas.draw_rect(cx, cy, card_w as u32, card_h as u32, bg);
            canvas.draw_rect_outline(cx, cy, card_w as u32, card_h as u32, bd, if is_selected { 2 } else { 1 });

            // Badge d'en-tête de la carte
            let title = format!("{} {}", cat.icon(), cat.title());
            let header_bg = if is_selected { 0x00334155 } else { 0x0018181B };
            canvas.draw_badge(cx + 8, cy + 8, card_w as u32 - 16, 24, header_bg, cat.color(), &title, 0x00FFFFFF, 1);

            // Description textuelle
            let desc = cat.desc();
            let parts: Vec<&str> = desc.split(" -> ").collect();
            let mut dy = cy + 40;
            for (p_i, p) in parts.iter().enumerate() {
                let prefix = if p_i == 0 { "Quand: " } else { "Effet: " };
                let line_str = format!("{}{}", prefix, p);
                let display_str = if line_str.len() > 34 {
                    format!("{}...", &line_str[..32])
                } else {
                    line_str
                };
                canvas.draw_text(cx + 10, dy, &display_str, 0x0094A3B8, 1);
                dy += 18;
            }

            // Bouton de sélection
            let pill_txt = if is_selected { "[V] SELECTIONNE" } else { "Choisir l'intention" };
            let pill_bg = if is_selected { 0x00064E3B } else { 0x00131C2E };
            let pill_col = if is_selected { 0x0034D399 } else { 0x0064748B };
            canvas.draw_badge(cx + 10, cy + card_h - 32, card_w as u32 - 20, 22, pill_bg, pill_col, pill_txt, pill_col, 1);

            self.editor.wizard.category_hitboxes.push((cat, cx, cy, card_w as u32, card_h as u32));
        }
    }

    fn render_wizard_step_2(&mut self, canvas: &mut Canvas, mx: i32, my: i32, mw: i32, _mh: i32) {
        self.editor.wizard.condition_option_hitboxes.clear();
        self.editor.wizard.action_option_hitboxes.clear();
        self.editor.wizard.sound_option_hitboxes.clear();

        let cat = self.editor.wizard.selected_category;
        let header_str = format!("{} Personnalisation : {}", cat.icon(), cat.title());
        canvas.draw_badge(mx + 20, my + 82, (mw - 40) as u32, 28, 0x001E293B, cat.color(), &header_str, 0x00FFFFFF, 1);

        let (q1_title, opt1, q2_title, opt2, q3_title, opt3) = match cat {
            WizardCategory::Chest => (
                "Condition d'ouverture : Quelle clé requiert ce coffre ?",
                ["Clé d'Or (\"key_gold\")", "Cristal d'Énergie", "Sans condition (Libre)"],
                "Contenu du trésor : Quelle récompense offrir ?",
                ["+100 Pts & Potion", "+250 Pts & Épée Ancienne", "+500 Pts (Pactole pur)"],
                "Effet sonore : Quel retour audio jouer ?",
                ["Fanfare de victoire", "Clic mécanique doux", "Silencieux"],
            ),
            WizardCategory::Door => (
                "Verrouillage : Comment la porte s'ouvre-t-elle ?",
                ["Clé Donjon (\"key_dungeon\")", "Score minimal >= 200", "Variable énigme active"],
                "Comportement : Que fait la porte activée ?",
                ["Ouvre & s'efface", "Bascule (Ouvre / Ferme)", "Ouvre + 50 Pts bonus"],
                "Effet sonore d'ouverture :",
                ["Grincement de pierre", "Carillon magique", "Silencieux"],
            ),
            WizardCategory::Hazard => (
                "Déclencheur : Quand le piège frappe-t-il ?",
                ["Contact Joueur direct", "Zone de proximité", "Pulsation continue"],
                "Dégâts : Combien de PV retirer ?",
                ["Dégâts légers (-15 PV)", "Dégâts moyens (-35 PV)", "Critique mortel (-75 PV)"],
                "Feedback sensoriel :",
                ["Impact violent & secousse", "Décharge électrique", "Silencieux"],
            ),
            WizardCategory::Teleport => (
                "Destination : Vers quelle zone téléporter ?",
                ["Entrée (X:100, Y:150)", "Arène (X:600, Y:350)", "Trésor (X:950, Y:200)"],
                "Effet additionnel du vortex :",
                ["Téléportation standard", "Téléportation + Soin 10 PV", "Téléportation + 50 Pts"],
                "Son de passage dans le vortex :",
                ["Vortex cosmique", "Éclair d'énergie", "Silencieux"],
            ),
            WizardCategory::SwitchPlate => (
                "Poids d'activation : Qui peut presser la dalle ?",
                ["Caisse ou Joueur (Sokoban)", "Joueur uniquement", "Caisse uniquement"],
                "Mécanisme débloqué : Quelle variable activer ?",
                ["Porte secrète (\"door_secret\")", "Coupe lasers (\"laser_field\")", "Pont-levis (\"bridge_active\")"],
                "Signal sonore :",
                ["Clic mécanique lourd", "Cloche de résolution", "Silencieux"],
            ),
            WizardCategory::Collectible => (
                "Valeur en points de score :",
                ["Pièce d'argent (+20 Pts)", "Trésor royal (+50 Pts)", "Gemme rare (+100 Pts)"],
                "Bonus additionnel :",
                ["Disparaît simplement", "Disparaît + Soin 15 PV", "Disparaît + Boost Vitesse"],
                "Son de récompense :",
                ["Carillon cristallin", "Ding arcade rétro", "Silencieux"],
            ),
        };

        let opt_w = (mw - 40 - 16) / 3;
        let opt_h = 36;
        let c_choice = self.editor.wizard.condition_choice;
        let a_choice = self.editor.wizard.action_choice;
        let s_choice = self.editor.wizard.sound_choice;

        // Question 1
        let mut qy = my + 118;
        canvas.draw_text(mx + 22, qy, q1_title, 0x00FBBF24, 1);
        for i in 0..3 {
            let ox = mx + 20 + i * (opt_w + 8);
            let oy = qy + 18;
            let is_sel = c_choice == i as usize;
            let (bg, bd, fg) = if is_sel {
                (0x00134E4A, 0x002DD4BF, 0x00F0FDFA)
            } else {
                (0x000F172A, 0x00334155, 0x0094A3B8)
            };
            let label = format!("{}{}", if is_sel { "[V] " } else { "" }, opt1[i as usize]);
            canvas.draw_badge(ox, oy, opt_w as u32, opt_h, bg, bd, &label, fg, 1);
            self.editor.wizard.condition_option_hitboxes.push((i as usize, ox, oy, opt_w as u32, opt_h));
        }

        // Question 2
        qy += 72;
        canvas.draw_text(mx + 22, qy, q2_title, 0x0038BDF8, 1);
        for i in 0..3 {
            let ox = mx + 20 + i * (opt_w + 8);
            let oy = qy + 18;
            let is_sel = a_choice == i as usize;
            let (bg, bd, fg) = if is_sel {
                (0x00134E4A, 0x002DD4BF, 0x00F0FDFA)
            } else {
                (0x000F172A, 0x00334155, 0x0094A3B8)
            };
            let label = format!("{}{}", if is_sel { "[V] " } else { "" }, opt2[i as usize]);
            canvas.draw_badge(ox, oy, opt_w as u32, opt_h, bg, bd, &label, fg, 1);
            self.editor.wizard.action_option_hitboxes.push((i as usize, ox, oy, opt_w as u32, opt_h));
        }

        // Question 3
        qy += 72;
        canvas.draw_text(mx + 22, qy, q3_title, 0x00A855F7, 1);
        for i in 0..3 {
            let ox = mx + 20 + i * (opt_w + 8);
            let oy = qy + 18;
            let is_sel = s_choice == i as usize;
            let (bg, bd, fg) = if is_sel {
                (0x00134E4A, 0x002DD4BF, 0x00F0FDFA)
            } else {
                (0x000F172A, 0x00334155, 0x0094A3B8)
            };
            let label = format!("{}{}", if is_sel { "[V] " } else { "" }, opt3[i as usize]);
            canvas.draw_badge(ox, oy, opt_w as u32, opt_h, bg, bd, &label, fg, 1);
            self.editor.wizard.sound_option_hitboxes.push((i as usize, ox, oy, opt_w as u32, opt_h));
        }
    }

    fn render_wizard_step_3(&mut self, canvas: &mut Canvas, mx: i32, my: i32, mw: i32, mh: i32) {
        self.editor.wizard.generate_btn_hitbox = None;

        let cat = self.editor.wizard.selected_category;
        let c_choice = self.editor.wizard.condition_choice;
        let a_choice = self.editor.wizard.action_choice;

        let header_str = format!("[OK] Synthese d'Intention : {}", cat.title());
        canvas.draw_badge(mx + 20, my + 82, (mw - 40) as u32, 28, 0x001E293B, 0x0010B981, &header_str, 0x00ECFDF5, 1);

        // Bloc 1 : Résumé en langage naturel
        let sum_y = my + 118;
        canvas.draw_rect(mx + 20, sum_y, (mw - 40) as u32, 106, 0x000F172A);
        canvas.draw_rect_outline(mx + 20, sum_y, (mw - 40) as u32, 106, 0x00334155, 1);
        canvas.draw_text(mx + 32, sum_y + 10, "RÉSUMÉ DU COMPORTEMENT GÉNÉRÉ (LANGAGE NATUREL) :", 0x00FBBF24, 1);

        let (line1, line2, line3) = match cat {
            WizardCategory::Chest => {
                let key_str = match c_choice {
                    0 => "Vérifie si le joueur possède 'key_gold'",
                    1 => "Vérifie si le joueur possède 'crystal_energy'",
                    _ => "Ouverture directe sans condition de clé",
                };
                let reward_str = match a_choice {
                    0 => "Ajoute +100 points et dépose une potion de soin",
                    1 => "Ajoute +250 points et dépose l'épée ancienne",
                    _ => "Ajoute un trésor massif de +500 points de score",
                };
                ("Déclenché lors de l'interaction touche [E] avec le coffre.", key_str, reward_str)
            }
            WizardCategory::Door => {
                let cond_str = match c_choice {
                    0 => "Requiert la possession de 'key_dungeon' pour s'ouvrir",
                    1 => "Requiert un score minimum de 200 points pour s'ouvrir",
                    _ => "Requiert l'activation de la variable énigme 'door_unlocked'",
                };
                let act_str = match a_choice {
                    0 => "Ouvre le passage et désactive le collider bloquant",
                    1 => "Bascule entre état ouvert et fermé à chaque pression",
                    _ => "Ouvre le passage et récompense le joueur de 50 points",
                };
                ("Déclenché lors de l'interaction touche [E] devant la porte.", cond_str, act_str)
            }
            WizardCategory::Hazard => {
                let cond_str = match c_choice {
                    0 => "S'enclenche au contact direct AABB avec le joueur",
                    1 => "S'enclenche quand le joueur approche dans la zone",
                    _ => "Pulsation de dégâts continue à intervalle régulier",
                };
                let act_str = match a_choice {
                    0 => "Inflige 15 points de dégâts légers",
                    1 => "Inflige 35 points de dégâts modérés",
                    _ => "Inflige 75 points de dégâts critiques dévastateurs",
                };
                ("Zone de danger / piège actif dans l'environnement.", cond_str, act_str)
            }
            WizardCategory::Teleport => {
                let dest_str = match c_choice {
                    0 => "Transfère le joueur vers l'entrée (X: 100, Y: 150)",
                    1 => "Transfère le joueur vers le centre de l'arène (X: 600, Y: 350)",
                    _ => "Transfère le joueur vers la salle secrète (X: 950, Y: 200)",
                };
                let act_str = match a_choice {
                    0 => "Vortex de particules immédiat au point d'arrivée",
                    1 => "Vortex et régénération immédiate de 10 PV",
                    _ => "Vortex et attribution d'un bonus explorateur de 50 points",
                };
                ("Portail dimensionnel de téléportation instantanée.", dest_str, act_str)
            }
            WizardCategory::SwitchPlate => {
                let trig_str = match c_choice {
                    0 => "Détecte la pression d'une caisse Sokoban ou du joueur",
                    1 => "Détecte la pression du joueur uniquement",
                    _ => "Détecte strictement la pression d'une caisse poussable",
                };
                let act_str = match a_choice {
                    0 => "Bascule la variable 'door_secret' à VRAI",
                    1 => "Bascule la variable 'laser_field' à FAUX",
                    _ => "Bascule la variable 'bridge_active' à VRAI",
                };
                ("Dalle d'énigme au sol sensible à la pression.", trig_str, act_str)
            }
            WizardCategory::Collectible => {
                let val_str = match c_choice {
                    0 => "Incrémente le score du joueur de +20 points",
                    1 => "Incrémente le score du joueur de +50 points",
                    _ => "Incrémente le score du joueur de +100 points rares",
                };
                let act_str = match a_choice {
                    0 => "Supprime l'objet du monde immédiatement après contact",
                    1 => "Supprime l'objet et restaure 15 points de vie",
                    _ => "Supprime l'objet et applique un boost d'accélération",
                };
                ("Objet à ramasser au contact du joueur (Trigger2D).", val_str, act_str)
            }
        };

        canvas.draw_text(mx + 32, sum_y + 32, line1, 0x00F8FAFC, 1);
        canvas.draw_text(mx + 32, sum_y + 52, &format!("• {}", line2), 0x0038BDF8, 1);
        canvas.draw_text(mx + 32, sum_y + 72, &format!("• {}", line3), 0x0034D399, 1);

        // Bloc 2 : Aperçu visuel du flux de nœuds KVL
        let flow_y = sum_y + 116;
        canvas.draw_rect(mx + 20, flow_y, (mw - 40) as u32, 88, 0x000B132B);
        canvas.draw_rect_outline(mx + 20, flow_y, (mw - 40) as u32, 88, 0x001E293B, 1);
        canvas.draw_text(mx + 32, flow_y + 10, "FLUX D'EXÉCUTION DU GRAPHE KVL GÉNÉRÉ (NIVEAU 2 COMPATIBLE) :", 0x00A855F7, 1);

        let flow_nodes = [
            ("[EVENT] EVENEMENT", 0x00F59E0B),
            ("[COND] CONDITION", 0x0038BDF8),
            ("[ACT] ACTION", 0x0010B981),
            ("[SND] AUDIO / ETAT", 0x00EC4899),
        ];
        let fn_w = (mw - 40 - 72) / 4;
        for (i, (fn_title, fn_col)) in flow_nodes.iter().enumerate() {
            let fn_x = mx + 30 + i as i32 * (fn_w + 16);
            let fn_y = flow_y + 34;
            canvas.draw_badge(fn_x, fn_y, fn_w as u32, 34, 0x00131C2E, *fn_col, fn_title, *fn_col, 1);
            if i < 3 {
                canvas.draw_text(fn_x + fn_w as i32 + 3, fn_y + 10, ">", 0x0064748B, 1);
            }
        }

        // Info cible
        let target_y = flow_y + 98;
        let target_str = match self.editor.selected {
            SelectedEntity::Entity(idx) if idx < self.entities.len() => {
                format!("Entité cible : '{}' [Tag: {} | ID {}] (Attachera le script KVL)", self.entities[idx].name, self.entities[idx].tag, self.entities[idx].id)
            }
            _ => "Entité cible : Nouvelle entité créée automatiquement au centre du monde".into(),
        };
        canvas.draw_badge(mx + 20, target_y, (mw - 40) as u32, 26, 0x00142A1E, 0x0010B981, &target_str, 0x00A7F3D0, 1);

        // Bouton de déploiement
        let btn_w = 380;
        let btn_h = 36;
        let btn_x = mx + (mw - btn_w) / 2;
        let btn_y = my + mh - 48;
        canvas.draw_badge(btn_x, btn_y, btn_w as u32, btn_h, 0x00065F46, 0x0010B981, "APPLIQUER LA MECANIQUE AU JEU (ENTREE)", 0x00ECFDF5, 2);
        self.editor.wizard.generate_btn_hitbox = Some((btn_x, btn_y, btn_w as u32, btn_h));
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
                let is_ai_node = self.editor.pending_ai_patch.as_ref().map_or(false, |p| {
                    p.added_node_ids.contains(&node.id)
                });

                let (box_bg, box_bd, badge) = if is_ai_node {
                    (0x002E1065, 0x00A855F7, "✨ IA :")
                } else {
                    match node.kind {
                        NodeKind::Event { .. } => (0x00082F49, 0x000284C7, "1. QUAND :"),
                        NodeKind::Condition { .. } => (0x00451A03, 0x00D97706, "2. SI (TEST) :"),
                        NodeKind::Action { ref action_type } if action_type == "BonusPoints" => (0x0078350F, 0x00F59E0B, "BONUS :"),
                        NodeKind::Action { ref action_type } if action_type == "BoostSpeed" => (0x00164E63, 0x0006B6D4, "VITESSE :"),
                        NodeKind::Action { ref action_type } if action_type == "SpawnSparkles" => (0x00831843, 0x00EC4899, "PARTICULES :"),
                        NodeKind::Action { ref action_type } if action_type == "DenyAccess" => (0x007F1D1D, 0x00DC2626, "! REFUS :"),
                        _ if idx == 1 => (0x00064E3B, 0x00059669, "3. ALORS :"),
                        _ => (0x003B0764, 0x009333EA, "EFFET :"),
                    }
                };

                canvas.draw_rect(insp_x + 18, block_y, btn_w, 36, box_bg);
                canvas.draw_rect_outline(insp_x + 18, block_y, btn_w, 36, box_bd, if is_ai_node { 2 } else { 1 });

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

                let is_ai_node = self.editor.pending_ai_patch.as_ref().map_or(false, |p| {
                    p.added_node_ids.contains(&node.id)
                });

                let (header_col, header_tag) = if is_ai_node {
                    (0x00A855F7, "✨ IA PROPOSÉ")
                } else {
                    match node.kind {
                        NodeKind::Event { .. } => (0x000284C7, "EVENT"),
                        NodeKind::Condition { .. } => (0x00D97706, "CONDITION"),
                        _ => (0x00059669, "ACTION"),
                    }
                };

                let card_bg = if is_ai_node { 0x002E1065 } else { 0x00131C2E };
                canvas.draw_rect(insp_x + 18, cur_y, btn_w, card_h as u32, card_bg);
                canvas.draw_rect_outline(insp_x + 18, cur_y, btn_w, card_h as u32, header_col, if is_ai_node { 3 } else { 2 });

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
                    let is_ai_conn = self.editor.pending_ai_patch.as_ref().map_or(false, |p| {
                        p.added_node_ids.contains(&conn.from_node)
                            || p.added_node_ids.contains(&conn.to_node)
                    });
                    let line_col = if is_ai_conn { 0x00A855F7 } else { 0x0010B981 };
                    canvas.draw_thick_line(fx, fy, tx, ty, if is_ai_conn { 4 } else { 3 }, line_col);

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

    fn save_test_ppm(canvas: &Canvas, path: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::File::create(path) {
            let _ = writeln!(f, "P6\n{} {}\n255", canvas.width, canvas.height);
            let mut bytes = Vec::with_capacity(canvas.width * canvas.height * 3);
            for &pixel in &canvas.buffer {
                bytes.push(((pixel >> 16) & 0xFF) as u8);
                bytes.push(((pixel >> 8) & 0xFF) as u8);
                bytes.push((pixel & 0xFF) as u8);
            }
            let _ = f.write_all(&bytes);
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
    fn test_collision_resolves_more_than_16_candidates() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let p_idx = state.player_index().unwrap();
        let player_x = 300.0;
        let player_y = 300.0;
        state.entities[p_idx].transform.x = player_x;
        state.entities[p_idx].transform.y = player_y;

        // Grappe dense de 20 murs solides empilés au même endroit, juste à droite
        // du joueur : la requête spatiale doit renvoyer > 16 candidats.
        let wall_left = player_x + 40.0;
        for k in 0..20 {
            let wall = Entity::new(
                1000 + k,
                format!("dense_wall_{k}"),
                "wall",
                Transform2D::new(wall_left, player_y - 10.0, 32.0, 32.0),
            )
            .with_collider(Collider2D::solid(32.0, 32.0));
            state.entities.push(wall);
        }
        state.rebuild_static_spatial_grid();

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyD);
        for _ in 0..20 {
            state.update(0.016, &input);
        }

        let p = &state.entities[p_idx];
        assert!(
            p.transform.x < wall_left,
            "Le joueur ne doit pas traverser une grappe dense de murs (> 16 candidats de collision)"
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
    fn test_player_moves_through_chamber_to_door_and_opens_it() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let p_idx = state.player_index().unwrap();
        let init_x = state.entities[p_idx].transform.x;
        let init_y = state.entities[p_idx].transform.y;
        assert_eq!(init_x as i32, 189);
        assert_eq!(init_y as i32, 106);

        // 1. Déplacement vers le bas pour contourner le mur supérieur (x=350, y=80..250)
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyS);
        for _ in 0..45 {
            state.update(0.016, &input);
        }
        input.on_key_up(KeyCode::KeyS);

        // 2. Déplacement vers la droite à travers la salle
        input.on_key_down(KeyCode::KeyD);
        for _ in 0..120 {
            state.update(0.016, &input);
        }
        input.on_key_up(KeyCode::KeyD);

        let p_pos_x = state.entities[p_idx].transform.x;
        assert!(p_pos_x > 500.0, "Le joueur a bien avancé vers la droite");

        // 3. Navigation A* directe vers la porte (830, 310)
        let mut nav = crate::navigation::NavAgent::go_to(260.0, 830.0, 310.0);
        nav.reach_distance = 18.0;
        nav.avoid_hazards = true;
        state.entities[p_idx].nav_agent = Some(nav);

        let empty_input = InputState::new();
        for _ in 0..150 {
            state.update(0.016, &empty_input);
        }

        let final_p = &state.entities[p_idx];
        let door_idx = state.entities.iter().position(|e| e.tag == "door").unwrap();
        let door_x = state.entities[door_idx].transform.x;
        let door_y = state.entities[door_idx].transform.y;
        let dist_to_door = ((final_p.transform.x - door_x).powi(2) + (final_p.transform.y - door_y).powi(2)).sqrt();
        assert!(dist_to_door < 80.0, "Le joueur est arrivé devant la porte (dist: {:.1})", dist_to_door);

        // 4. Interaction pour ouvrir la porte
        state.kvl_context.set_var("score", Value::Int(50));
        let mut interact_input = InputState::new();
        interact_input.on_key_down(KeyCode::KeyE);
        state.update(0.016, &interact_input);

        assert!(!state.entities[door_idx].collider.as_ref().unwrap().is_solid, "La porte s'est ouverte");
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
    fn test_platformer_one_way_platform_support() {
        let mut state = GameState::load_from_scene("assets/scenes/platformer_01.scene.json")
            .expect("Chargement du niveau plateforme");

        let p_idx = state.player_index().expect("Joueur présent");

        // 1. Ajouter une plateforme semi-solide One-Way en Y = 300.0
        let plat_id = state.next_entity_id();
        let mut plat = crate::entity::Entity::new(
            plat_id,
            "Plateforme_OneWay",
            "platform",
            crate::entity::Transform2D::new(60.0, 300.0, 200.0, 16.0),
        );
        plat.collider = Some(crate::entity::Collider2D::one_way(200.0, 16.0));
        state.entities.push(plat);
        state.rebuild_static_spatial_grid();

        // 2. Positionner le joueur au-dessus en chute libre
        state.entities[p_idx].transform.x = 100.0;
        state.entities[p_idx].transform.y = 200.0;
        if let Some(ref mut mov) = state.entities[p_idx].movement {
            mov.vy = 100.0;
        }

        let input = InputState::new();
        for _ in 0..30 {
            state.update(0.016, &input);
        }

        // Le joueur doit avoir atterri sur le dessus de la plateforme (Y = 300 - hauteur_joueur)
        let player_h = state.entities[p_idx].transform.h;
        assert_eq!(
            state.entities[p_idx].transform.y,
            300.0 - player_h,
            "Le joueur doit atterrir sur la plateforme semi-solide"
        );

        if let MovementMode::Platformer { is_grounded, .. } = state.entities[p_idx].movement.as_ref().unwrap().mode {
            assert!(is_grounded, "Le joueur doit être grounded sur la plateforme OneWay");
        }

        // 3. Traversée descendante volontaire avec Flèche Bas
        let mut drop_input = InputState::new();
        drop_input.on_key_down(KeyCode::ArrowDown);
        state.update(0.016, &drop_input);
        state.update(0.016, &drop_input);

        // Le joueur doit avoir traversé la plateforme vers le bas
        assert!(
            state.entities[p_idx].transform.y > 300.0 - player_h,
            "L'appui sur Bas doit permettre de descendre à travers la plateforme"
        );
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

        // 4. Bascule vers la scène suivante (Boss Fight Titan)
        state.cycle_scene_forward();
        assert!(state.scene_path.contains("boss"));
        assert!(state.scene_name.contains("Boss") || state.scene_name.contains("Titan"));

        // 5. Rebouclage vers la première scène (Top-Down Aventure)
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

    #[test]
    fn test_inventory_hotbar_slot_selection_and_key_navigation() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        // Vérifier l'inventaire initial (2 potions et 1 clef de bronze)
        assert_eq!(state.inventory.get_item_count("potion_heal"), 2);
        assert_eq!(state.inventory.get_item_count("key_bronze"), 1);
        assert_eq!(state.inventory.selected_hotbar_index, 0);

        // Appui sur la touche 2 pour sélectionner le slot 1
        let mut input2 = InputState::new();
        input2.on_key_down(KeyCode::Digit2);
        state.update(0.016, &input2);

        assert_eq!(state.inventory.selected_hotbar_index, 1);
        assert_eq!(state.kvl_context.get_var("hotbar_selected_slot"), Some(&Value::Int(1)));

        // Appui sur la touche 1 pour revenir au slot 0
        let mut input1 = InputState::new();
        input1.on_key_down(KeyCode::Digit1);
        state.update(0.016, &input1);

        assert_eq!(state.inventory.selected_hotbar_index, 0);
        assert_eq!(state.kvl_context.get_var("hotbar_selected_slot"), Some(&Value::Int(0)));
    }

    #[test]
    fn test_potion_consumption_restores_player_health_and_kvl() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        let p_idx = state.player_index().expect("Joueur");
        // Infliger des dégâts au joueur : 3.0 -> 1.0 PV
        if let Some(ref mut h) = state.entities[p_idx].health {
            h.current = 1.0;
        }

        assert_eq!(state.inventory.get_item_count("potion_heal"), 2);

        // Consommer la potion sur le slot 0 via use_hotbar_item
        let res = state.use_hotbar_item(0);
        match res {
            ItemUseResult::ConsumedHeal { amount, new_hp } => {
                assert_eq!(amount, 1);
                assert_eq!(new_hp, 2.0);
            }
            other => panic!("Attendu ConsumedHeal, obtenu {:?}", other),
        }

        // Vérifier les PV du joueur et la synchronisation KVL
        let current_hp = state.entities[p_idx].health.as_ref().unwrap().current;
        assert_eq!(current_hp, 2.0);
        assert_eq!(state.kvl_context.get_var("player_hp"), Some(&Value::Int(2)));
        assert_eq!(state.inventory.get_item_count("potion_heal"), 1);
    }

    #[test]
    fn test_world_item_pickup_collection_and_kvl_synchronization() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        let p_idx = state.player_index().expect("Joueur");
        let px = state.entities[p_idx].transform.x;
        let py = state.entities[p_idx].transform.y;

        // Spawner un cristal magique exactement sur la position du joueur
        let crystal_item = Item::energy_crystal("crystal_test", "Cristal de Test");
        let pickup_id = state.entities.len();
        state.entities.push(Entity::spawn_item_pickup(pickup_id, px, py, crystal_item, 1));

        let initial_crystals = state.inventory.get_item_count("crystal_test");
        assert_eq!(initial_crystals, 0);

        // Faire tourner une frame de simulation
        let input = InputState::new();
        state.update(0.016, &input);

        // L'entité doit être désactivée et l'objet doit être dans l'inventaire
        assert!(!state.entities[pickup_id].is_active, "Le pickup doit être collecté et désactivé");
        assert_eq!(state.inventory.get_item_count("crystal_test"), 1);
        assert_eq!(state.kvl_context.get_var("has_crystal_test"), Some(&Value::Bool(true)));
        assert_eq!(state.kvl_context.get_var("item_count_crystal_test"), Some(&Value::Int(1)));
        assert_eq!(state.kvl_context.get_var("last_picked_item"), Some(&Value::String("crystal_test".into())));
    }

    #[test]
    fn test_mouse_click_hotbar_hitbox_interaction() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        // Rendu d'une frame pour calculer les hitboxes réelles
        let mut canvas = Canvas::new(800, 600);
        state.inventory.render_hotbar(&mut canvas, 800, 600);

        // Récupérer la hitbox du slot 2 (index 1)
        let (hx, hy, hw, hh) = state.inventory.hotbar_hitboxes[1].expect("Hitbox slot 2");

        // Clic de la souris au centre du slot 2
        let mut input_click = InputState::new();
        input_click.on_mouse_move((hx + (hw / 2) as i32) as f32, (hy + (hh / 2) as i32) as f32);
        input_click.on_mouse_down();
        state.update(0.016, &input_click);

        // Le slot 1 doit être sélectionné
        assert_eq!(state.inventory.selected_hotbar_index, 1);
        assert_eq!(state.kvl_context.get_var("hotbar_selected_slot"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_studio_spawn_item_presets() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        let p1 = state.spawn_entity("item_potion", 200.0, 200.0);
        let p2 = state.spawn_entity("item_key", 250.0, 200.0);
        let p3 = state.spawn_entity("item_crystal", 300.0, 200.0);

        assert!(state.entities[p1].item_pickup.is_some());
        assert_eq!(state.entities[p1].tag, "item_pickup");
        assert!(state.entities[p2].item_pickup.is_some());
        assert!(state.entities[p3].item_pickup.is_some());
    }

    #[test]
    fn test_render_desktop_snapshots() {
        use std::io::Write;
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);

        // Save play mode ppm
        let mut f_play = std::fs::File::create("/tmp/karactirage_play.ppm").unwrap();
        writeln!(f_play, "P6\n1200 800\n255").unwrap();
        let mut bytes = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes.push(((pixel >> 16) & 0xFF) as u8);
            bytes.push(((pixel >> 8) & 0xFF) as u8);
            bytes.push((pixel & 0xFF) as u8);
        }
        f_play.write_all(&bytes).unwrap();

        // Switch to Studio mode and render
        state.editor.toggle_mode();
        state.render(&mut canvas);
        let mut f_studio = std::fs::File::create("/tmp/karactirage_studio.ppm").unwrap();
        writeln!(f_studio, "P6\n1200 800\n255").unwrap();
        let mut bytes2 = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes2.push(((pixel >> 16) & 0xFF) as u8);
            bytes2.push(((pixel >> 8) & 0xFF) as u8);
            bytes2.push((pixel & 0xFF) as u8);
        }
        f_studio.write_all(&bytes2).unwrap();

        // 3. Welcome / Onboarding Modal Snapshot
        let mut welcome_state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        welcome_state.editor.show_welcome_modal = true;
        welcome_state.render(&mut canvas);
        let mut f_welcome = std::fs::File::create("/tmp/karactirage_welcome.ppm").unwrap();
        writeln!(f_welcome, "P6\n1200 800\n255").unwrap();
        let mut bytes_w = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes_w.push(((pixel >> 16) & 0xFF) as u8);
            bytes_w.push(((pixel >> 8) & 0xFF) as u8);
            bytes_w.push((pixel & 0xFF) as u8);
        }
        f_welcome.write_all(&bytes_w).unwrap();

        // 4. Guide Pédagogique 3 Onglets Snapshot
        let mut guide_state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        guide_state.editor.show_help_modal = true;
        guide_state.editor.guide_tab = 0;
        guide_state.editor.guide_step_done[0] = true;
        guide_state.editor.guide_step_done[1] = true;
        guide_state.render(&mut canvas);
        let mut f_guide = std::fs::File::create("/tmp/karactirage_guide_tabs.ppm").unwrap();
        writeln!(f_guide, "P6\n1200 800\n255").unwrap();
        let mut bytes_g = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes_g.push(((pixel >> 16) & 0xFF) as u8);
            bytes_g.push(((pixel >> 8) & 0xFF) as u8);
            bytes_g.push((pixel & 0xFF) as u8);
        }
        f_guide.write_all(&bytes_g).unwrap();

        // 5. Intent IA Modal Snapshot
        let mut intent_state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        intent_state.editor.mode = EditorMode::Edit;
        intent_state.editor.show_intent_bar = true;
        intent_state.render(&mut canvas);
        let mut f_intent = std::fs::File::create("/tmp/karactirage_intent_ia.ppm").unwrap();
        writeln!(f_intent, "P6\n1200 800\n255").unwrap();
        let mut bytes_i = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes_i.push(((pixel >> 16) & 0xFF) as u8);
            bytes_i.push(((pixel >> 8) & 0xFF) as u8);
            bytes_i.push((pixel & 0xFF) as u8);
        }
        f_intent.write_all(&bytes_i).unwrap();

        // 6. AI Patch Review Snapshot (Halos Violets #A855F7 & Review Bar Bac à Sable)
        let mut patch_state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        patch_state.editor.mode = EditorMode::Edit;
        patch_state.trigger_ai_intent("Garde patrouilleur hostile A* avec alarme");
        patch_state.render(&mut canvas);
        let mut f_patch = std::fs::File::create("/tmp/karactirage_ai_patch_review.ppm").unwrap();
        writeln!(f_patch, "P6\n1200 800\n255").unwrap();
        let mut bytes_p = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes_p.push(((pixel >> 16) & 0xFF) as u8);
            bytes_p.push(((pixel >> 8) & 0xFF) as u8);
            bytes_p.push((pixel & 0xFF) as u8);
        }
        f_patch.write_all(&bytes_p).unwrap();

        // 7. Full AI Generated Pixel-Art Level Snapshot
        let mut ai_level_state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let payload = crate::ai_client::AiGeneratedGamePayload {
            title: "Donjon Pixel-Art de l'Ombre".to_string(),
            description: "Donjon Pixel-Art avec gouffre d'acide, ninja, boss squelette et coffre mystique".to_string(),
            primary_genre: Some("Platformer".to_string()),
            terrain: Some(crate::ai_client::AiGeneratedTerrain {
                theme: "dungeon".to_string(),
                platforms: vec![
                    crate::ai_client::AiGeneratedPlatform {
                        name: "Sol Entrée".to_string(),
                        offset_x: -120.0,
                        offset_y: 120.0,
                        width: 280.0,
                        height: 40.0,
                        is_solid: true,
                        color_hex: Some("#475569".to_string()),
                    },
                    crate::ai_client::AiGeneratedPlatform {
                        name: "Plateforme Arène".to_string(),
                        offset_x: 240.0,
                        offset_y: 100.0,
                        width: 480.0,
                        height: 40.0,
                        is_solid: true,
                        color_hex: Some("#334155".to_string()),
                    },
                    crate::ai_client::AiGeneratedPlatform {
                        name: "Sanctuaire Portail".to_string(),
                        offset_x: 780.0,
                        offset_y: 110.0,
                        width: 160.0,
                        height: 40.0,
                        is_solid: true,
                        color_hex: Some("#475569".to_string()),
                    },
                ],
                hazards: vec![
                    crate::ai_client::AiGeneratedHazard {
                        name: "Gouffre d'Acide".to_string(),
                        offset_x: 160.0,
                        offset_y: 140.0,
                        width: 80.0,
                        height: 60.0,
                        hazard_type: Some("acid".to_string()),
                        damage: 35.0,
                    },
                    crate::ai_client::AiGeneratedHazard {
                        name: "Fosse de Pointes".to_string(),
                        offset_x: 720.0,
                        offset_y: 140.0,
                        width: 60.0,
                        height: 60.0,
                        hazard_type: Some("spikes".to_string()),
                        damage: 25.0,
                    },
                ],
            }),
            gameplay_loop: Some(crate::ai_client::AiGeneratedGameplayLoop {
                objective: "Traversez les gouffres, anéantissez le Boss Squelette et rejoignez le Portail".to_string(),
                win_condition: "boss_defeated".to_string(),
                lose_condition: "player_health_zero".to_string(),
                target_item_name: Some("Cle_Donjon".to_string()),
                win_fanfare: Some("Victoire Légendaire !".to_string()),
            }),
            entities: vec![
                crate::ai_client::AiGeneratedEntity {
                    name: "Héros Ninja".to_string(),
                    kind: "player".to_string(),
                    offset_x: -60.0,
                    offset_y: 70.0,
                    width: 36.0,
                    height: 48.0,
                    health: 6.0,
                    color_hex: Some("#3B82F6".to_string()),
                    sprite_archetype: Some("ninja".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    movement: Some("none".to_string()),
                    has_collider: true,
                    has_vision: false,
                    is_flocking: false,
                },
                crate::ai_client::AiGeneratedEntity {
                    name: "Boss Squelette".to_string(),
                    kind: "boss".to_string(),
                    offset_x: 480.0,
                    offset_y: 30.0,
                    width: 56.0,
                    height: 72.0,
                    health: 50.0,
                    color_hex: Some("#EF4444".to_string()),
                    sprite_archetype: Some("skull".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    movement: Some("patrol".to_string()),
                    has_collider: true,
                    has_vision: true,
                    is_flocking: false,
                },
                crate::ai_client::AiGeneratedEntity {
                    name: "Coffre Ancien".to_string(),
                    kind: "item".to_string(),
                    offset_x: 680.0,
                    offset_y: 65.0,
                    width: 36.0,
                    height: 36.0,
                    health: 1.0,
                    color_hex: Some("#F59E0B".to_string()),
                    sprite_archetype: Some("chest".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    movement: Some("none".to_string()),
                    has_collider: true,
                    has_vision: false,
                    is_flocking: false,
                },
                crate::ai_client::AiGeneratedEntity {
                    name: "Portail Cosmique".to_string(),
                    kind: "door".to_string(),
                    offset_x: 840.0,
                    offset_y: 50.0,
                    width: 40.0,
                    height: 60.0,
                    health: 100.0,
                    color_hex: Some("#8B5CF6".to_string()),
                    sprite_archetype: Some("portal".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    movement: Some("none".to_string()),
                    has_collider: true,
                    has_vision: false,
                    is_flocking: false,
                },
            ],
            kvl_actions: vec![],
            variables: None,
        };
        ai_level_state.apply_ai_payload(payload, "Donjon Pixel-Art avec gouffres et boss");
        ai_level_state.editor.mode = EditorMode::Play;
        ai_level_state.render(&mut canvas);
        save_test_ppm(&canvas, "/tmp/karactirage_ai_pixel_level.ppm");
    }

    #[test]
    fn test_guided_wizard_lifecycle_and_generation() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        // Passer en mode Studio
        state.editor.mode = EditorMode::Edit;
        assert!(!state.editor.wizard.is_open);

        // 1. Ouvrir l'Assistant Guidé via touche G
        let mut input_g = InputState::new();
        input_g.on_key_down(KeyCode::KeyG);
        state.update(0.016, &input_g);
        assert!(state.editor.wizard.is_open);
        assert_eq!(state.editor.wizard.step, 1);

        // 2. Rendu de l'étape 1 et vérification des 6 cartes d'intention
        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);
        assert_eq!(state.editor.wizard.category_hitboxes.len(), 6);
        assert!(state.editor.guided_btn_hitbox.is_some());

        // 3. Cliquer sur la carte "Coffre au Trésor"
        let (chest_cat, cx, cy, cw, ch) = state
            .editor
            .wizard
            .category_hitboxes
            .iter()
            .find(|(cat, ..)| *cat == WizardCategory::Chest)
            .copied()
            .expect("Carte coffre trouvée");

        let mut input_click_cat = InputState::new();
        input_click_cat.on_mouse_move((cx + (cw / 2) as i32) as f32, (cy + (ch / 2) as i32) as f32);
        input_click_cat.on_mouse_down();
        state.update(0.016, &input_click_cat);

        assert_eq!(state.editor.wizard.selected_category, chest_cat);
        assert_eq!(state.editor.wizard.step, 2, "Doit passer à l'étape 2 des questions");

        // 4. Rendu de l'étape 2 (Questions & Options)
        state.render(&mut canvas);
        assert_eq!(state.editor.wizard.condition_option_hitboxes.len(), 3);
        assert_eq!(state.editor.wizard.action_option_hitboxes.len(), 3);
        assert_eq!(state.editor.wizard.sound_option_hitboxes.len(), 3);

        // Choisir l'option action 1 (+250 Pts & Épée)
        let (_, ax, ay, aw, ah) = state.editor.wizard.action_option_hitboxes[1];
        let mut input_opt = InputState::new();
        input_opt.on_mouse_move((ax + (aw / 2) as i32) as f32, (ay + (ah / 2) as i32) as f32);
        input_opt.on_mouse_down();
        state.update(0.016, &input_opt);
        assert_eq!(state.editor.wizard.action_choice, 1);

        // Passer à l'étape 3 via la touche Entrée
        let mut input_enter = InputState::new();
        input_enter.on_key_down(KeyCode::Enter);
        state.update(0.016, &input_enter);
        assert_eq!(state.editor.wizard.step, 3);

        // 5. Rendu de l'étape 3 (Synthèse et Validation)
        state.render(&mut canvas);
        assert!(state.editor.wizard.generate_btn_hitbox.is_some());

        // Cliquer sur [✨ APPLIQUER LA MÉCANIQUE AU JEU]
        let (bx, by, bw, bh) = state.editor.wizard.generate_btn_hitbox.unwrap();
        let mut input_gen = InputState::new();
        input_gen.on_mouse_move((bx + (bw / 2) as i32) as f32, (by + (bh / 2) as i32) as f32);
        input_gen.on_mouse_down();
        state.update(0.016, &input_gen);

        // L'assistant doit être fermé
        assert!(!state.editor.wizard.is_open);

        // Une mécanique doit être générée et stockée
        let generated_scripts: Vec<&String> = state.kvl_graphs.keys().filter(|k| k.starts_with("assets/scripts/guided_chest_")).collect();
        assert!(!generated_scripts.is_empty(), "Un graphe pour le coffre guidé doit avoir été généré");

        let graph = state.kvl_graphs.get(generated_scripts[0]).unwrap();
        let diagnostics = Validator::validate(graph);
        assert!(diagnostics.is_empty(), "Le graphe généré doit être 100% valide sans diagnostic : {:?}", diagnostics);

        // L'entité cible doit posséder le composant KVLScriptComponent
        let selected_idx = match state.editor.selected {
            SelectedEntity::Entity(idx) => idx,
            _ => panic!("Une entité doit être sélectionnée après génération"),
        };
        let ent = &state.entities[selected_idx];
        assert!(ent.script.is_some(), "L'entité doit avoir le composant KVLScript attaché");
        assert_eq!(ent.script.as_ref().unwrap().script_path, *generated_scripts[0]);
    }

    #[test]
    fn test_guided_wizard_renders_desktop_snapshots() {
        use std::io::Write;
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement scène");

        state.editor.mode = EditorMode::Edit;
        let mut canvas = Canvas::new(1200, 800);

        // Capture Étape 1 : Choix d'Intention
        state.editor.wizard.open();
        state.editor.wizard.step = 1;
        state.render(&mut canvas);
        let mut f_s1 = std::fs::File::create("/tmp/karactirage_wizard_step1.ppm").unwrap();
        writeln!(f_s1, "P6\n1200 800\n255").unwrap();
        let mut bytes1 = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes1.push(((pixel >> 16) & 0xFF) as u8);
            bytes1.push(((pixel >> 8) & 0xFF) as u8);
            bytes1.push((pixel & 0xFF) as u8);
        }
        f_s1.write_all(&bytes1).unwrap();

        // Capture Étape 2 : Questions / Paramètres
        state.editor.wizard.step = 2;
        state.editor.wizard.selected_category = WizardCategory::Chest;
        state.editor.wizard.condition_choice = 0;
        state.editor.wizard.action_choice = 1;
        state.editor.wizard.sound_choice = 0;
        state.render(&mut canvas);
        let mut f_s2 = std::fs::File::create("/tmp/karactirage_wizard_step2.ppm").unwrap();
        writeln!(f_s2, "P6\n1200 800\n255").unwrap();
        let mut bytes2 = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes2.push(((pixel >> 16) & 0xFF) as u8);
            bytes2.push(((pixel >> 8) & 0xFF) as u8);
            bytes2.push((pixel & 0xFF) as u8);
        }
        f_s2.write_all(&bytes2).unwrap();

        // Capture Étape 3 : Synthèse & Validation
        state.editor.wizard.step = 3;
        state.render(&mut canvas);
        let mut f_s3 = std::fs::File::create("/tmp/karactirage_wizard_step3.ppm").unwrap();
        writeln!(f_s3, "P6\n1200 800\n255").unwrap();
        let mut bytes3 = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes3.push(((pixel >> 16) & 0xFF) as u8);
            bytes3.push(((pixel >> 8) & 0xFF) as u8);
            bytes3.push((pixel & 0xFF) as u8);
        }
        f_s3.write_all(&bytes3).unwrap();
    }

    #[test]
    fn test_boss_scene_loading_and_entity_composition() {
        let state = GameState::load_from_scene("assets/scenes/boss_01.scene.json")
            .expect("Chargement de la scène de Boss boss_01.scene.json");

        assert_eq!(state.scene_name, "Arène du Titan : Sanctuaire de la Sentinelle");
        let boss = state.entities.iter().find(|e| e.tag == "boss").expect("Boss Titan présent");
        assert_eq!(boss.name, "TITAN PROTO-SENTINELLE");
        let boss_comp = boss.boss.as_ref().expect("Composant BossComponent attaché");
        assert_eq!(boss_comp.phase, BossPhase::Phase1);
        let health = boss.health.as_ref().expect("Composant Health2D attaché");
        assert_eq!(health.max, 60.0);
        assert_eq!(health.current, 60.0);

        // Vérification des 4 piliers d'abri
        let pillars_count = state.entities.iter().filter(|e| e.name.contains("Pilier")).count();
        assert_eq!(pillars_count, 4, "L'arène doit comporter 4 piliers de couverture");

        // Vérification du joueur
        let player = state.entities.iter().find(|e| e.tag == "player").expect("Joueur présent");
        assert_eq!(player.health.as_ref().map(|h| h.max), Some(5.0));
    }

    #[test]
    fn test_boss_telegraph_generation_and_bullet_nova_attack() {
        let mut state = GameState::load_from_scene("assets/scenes/boss_01.scene.json")
            .expect("Chargement de l'arène");
        let boss_idx = state.entities.iter().position(|e| e.tag == "boss").unwrap();

        // Armer manuellement une télégraphie de test
        if let Some(ref mut boss) = state.entities[boss_idx].boss {
            boss.telegraphs.push(TelegraphZone::new_circle(
                460.0, 200.0, 95.0, 0.5, BossAttackType::RadialNova, 0x0038BDF8, 1.0,
            ));
        }

        let input = InputState::default();
        let initial_bullets = state.entities.iter().filter(|e| e.tag == "projectile").count();
        assert_eq!(initial_bullets, 0);

        // Faire progresser le temps de 0.6s pour déclencher la fin de la télégraphie
        state.update(0.6, &input);

        let bullets_after = state.entities.iter().filter(|e| e.tag == "projectile").count();
        assert_eq!(bullets_after, 12, "La Nova radiale en Phase 1 doit projeter 12 boules d'énergie");

        // Vérifier que la télégraphie terminée a été consommée
        let active_telegraphs = state.entities[boss_idx].boss.as_ref().unwrap().telegraphs.len();
        assert!(active_telegraphs <= 1);
    }

    #[test]
    fn test_boss_phase_transitions_damage_and_enrage_mode() {
        let mut state = GameState::load_from_scene("assets/scenes/boss_01.scene.json")
            .expect("Chargement scène");
        let boss_idx = state.entities.iter().position(|e| e.tag == "boss").unwrap();
        let p_idx = state.player_index().unwrap();

        // 1. Simuler un tir joueur touchant le Boss
        let bx = state.entities[boss_idx].transform.center_x();
        let by = state.entities[boss_idx].transform.center_y();

        let bullet = Entity::spawn_projectile(999, bx, by, 0.0, 0.0, p_idx);
        state.entities.push(bullet);

        let input = InputState::default();
        state.update(0.016, &input);

        // Santé réduite de 10 PV (60 -> 50)
        let cur_hp = state.entities[boss_idx].health.as_ref().unwrap().current;
        assert_eq!(cur_hp, 50.0);
        assert_eq!(state.entities[boss_idx].boss.as_ref().unwrap().phase, BossPhase::Phase1);

        // 2. Infliger des dégâts pour passer sous les 66% (ex: 35 PV)
        let bullet2 = Entity::spawn_projectile(1000, bx, by, 0.0, 0.0, p_idx);
        state.entities.push(bullet2);
        state.entities[boss_idx].health.as_mut().unwrap().current = 40.0;
        state.update(0.016, &input);

        let phase_after = state.entities[boss_idx].boss.as_ref().unwrap().phase;
        assert_eq!(phase_after, BossPhase::Phase2, "Le Titan doit basculer en Phase 2 sous 66% de PV");

        // 3. Infliger des dégâts critiques pour passer sous les 33% (ex: 15 PV)
        state.entities[boss_idx].health.as_mut().unwrap().current = 22.0;
        let bullet3 = Entity::spawn_projectile(1001, bx, by, 0.0, 0.0, p_idx);
        state.entities.push(bullet3);
        state.update(0.016, &input);

        let phase_3 = state.entities[boss_idx].boss.as_ref().unwrap().phase;
        assert_eq!(phase_3, BossPhase::Phase3, "Le Titan doit entrer en Overdrive Enragé sous 33% de PV");
        assert_eq!(state.kvl_context.get_var("boss_enraged"), Some(&Value::Bool(true)));

        // 4. Coup de grâce (PV = 0)
        state.entities[boss_idx].health.as_mut().unwrap().current = 5.0;
        let bullet_fin = Entity::spawn_projectile(1002, bx, by, 0.0, 0.0, p_idx);
        state.entities.push(bullet_fin);
        state.update(0.016, &input);

        let phase_def = state.entities[boss_idx].boss.as_ref().unwrap().phase;
        assert_eq!(phase_def, BossPhase::Defeated, "Le Boss doit être terrassé à 0 PV");
        assert_eq!(state.kvl_context.get_var("boss_defeated"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_boss_fight_desktop_snapshot() {
        use std::io::Write;
        let mut state = GameState::load_from_scene("assets/scenes/boss_01.scene.json")
            .expect("Chargement de l'arène");
        let boss_idx = state.entities.iter().position(|e| e.tag == "boss").unwrap();

        // Cadrage panoramique de l'arène de combat
        state.telemetry.show_overlay = false;
        state.camera.target_x = 500.0;
        state.camera.target_y = 390.0;
        state.camera.snap_to_target();

        // Armer une télégraphie visuelle impressionnante pour le rendu
        if let Some(ref mut boss) = state.entities[boss_idx].boss {
            boss.telegraphs.push(TelegraphZone::new_circle(
                500.0, 440.0, 110.0, 1.2, BossAttackType::SeismicSlam, 0x00DC2626, 1.0,
            ));
            boss.telegraphs.push(TelegraphZone::new_circle(
                500.0, 240.0, 85.0, 1.0, BossAttackType::RadialNova, 0x0038BDF8, 1.0,
            ));
            boss.telegraphs[0].timer = 0.7; // ~58% de progression visible
            boss.telegraphs[1].timer = 0.4;
        }

        let mut canvas = Canvas::new(1200, 800);
        state.render(&mut canvas);

        let mut f = std::fs::File::create("/tmp/karactirage_boss_fight.ppm").unwrap();
        writeln!(f, "P6\n1200 800\n255").unwrap();
        let mut bytes = Vec::with_capacity(1200 * 800 * 3);
        for &pixel in &canvas.buffer {
            bytes.push(((pixel >> 16) & 0xFF) as u8);
            bytes.push(((pixel >> 8) & 0xFF) as u8);
            bytes.push((pixel & 0xFF) as u8);
        }
        f.write_all(&bytes).unwrap();
    }

    #[test]
    fn test_studio_undo_redo_lifecycle() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        state.editor.mode = EditorMode::Edit;
        let initial_count = state.entities.len();

        // 1. Peindre une tuile Mur via l'éditeur
        let mut input = InputState::new();
        input.mouse_pressed = true;
        input.mouse_x = 200.0;
        input.mouse_y = 200.0;
        state.editor.active_tool = StudioTool::Brush;
        state.update(0.016, &input);

        assert_eq!(state.entities.len(), initial_count + 1, "Une tuile doit être ajoutée");
        assert!(state.editor.undo_stack.can_undo(), "L'action doit être dans l'historique Undo");

        // 2. Annuler (Undo)
        assert!(state.undo(), "L'annulation doit réussir");
        assert_eq!(state.entities.len(), initial_count, "Le nombre d'entités doit revenir à l'initial");
        assert!(state.editor.undo_stack.can_redo(), "L'action doit être dans l'historique Redo");

        // 3. Rétablir (Redo)
        assert!(state.redo(), "Le rétablissement doit réussir");
        assert_eq!(state.entities.len(), initial_count + 1, "L'entité doit être réinsérée");
    }

    #[test]
    fn test_deterministic_replay_and_state_hash() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        let initial_hash = state.state_hash();

        // 1. Enregistrer une session de 30 ticks
        state.replay.start_recording(&state.scene_path, initial_hash);
        assert_eq!(state.replay.mode, ReplayMode::Recording);

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyD);

        for _ in 0..30 {
            state.update(0.016, &input);
        }

        let recorded_hash = state.state_hash();
        let session = state.replay.stop_recording(recorded_hash, 0).expect("Session enregistrée").clone();
        assert_eq!(session.frames.len(), 30);
        assert_eq!(session.final_state_hash, recorded_hash);

        // 2. Réinitialiser la scène et relancer les mêmes 30 ticks
        let mut fresh_state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        assert_eq!(fresh_state.state_hash(), initial_hash);

        for _ in 0..30 {
            fresh_state.update(0.016, &input);
        }

        let replay_hash = fresh_state.state_hash();
        assert_eq!(recorded_hash, replay_hash, "La simulation doit être 100% déterministe au bit près");
    }

    #[test]
    fn test_transform_hierarchy_parent_child_sync() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        let parent_id = 9001;
        let child_id = 9002;

        let parent = Entity::spawn_wall(parent_id, 100.0, 200.0, 32.0, 32.0);
        let mut child = Entity::spawn_torch(child_id, 0.0, 0.0);
        child.transform.attach_to(parent_id, 15.0, -25.0);

        state.entities.push(parent);
        state.entities.push(child);

        let input = InputState::new();
        state.update(0.016, &input);

        let c_idx = state.entities.iter().position(|e| e.id == child_id).unwrap();
        assert_eq!(state.entities[c_idx].transform.x, 115.0);
        assert_eq!(state.entities[c_idx].transform.y, 175.0);

        // Déplacer le parent
        let p_idx = state.entities.iter().position(|e| e.id == parent_id).unwrap();
        state.entities[p_idx].transform.x = 300.0;
        state.entities[p_idx].transform.y = 400.0;

        state.update(0.016, &input);
        assert_eq!(state.entities[c_idx].transform.x, 315.0);
        assert_eq!(state.entities[c_idx].transform.y, 375.0);
    }

    #[test]
    fn test_ai_astar_obstacle_avoidance_navigation() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();

        // 1. Créer un agent intelligent en (64, 64) se dirigeant vers (256, 64)
        let agent_id = 9500;
        let agent = Entity::new(agent_id, "smart_drone", "enemy", Transform2D::new(64.0, 64.0, 24.0, 24.0))
            .with_collider(Collider2D::solid(24.0, 24.0))
            .with_nav_agent(crate::navigation::NavAgent::go_to(120.0, 256.0, 64.0));

        // 2. Placer un mur obstacle bloquant la trajectoire directe en x = 160
        let wall_id = 9501;
        let wall = Entity::spawn_wall(wall_id, 160.0, 32.0, 32.0, 96.0); // Couvre y de 32 à 128

        state.entities.push(agent);
        state.entities.push(wall);

        let input = InputState::new();

        // Première mise à jour pour générer le chemin A*
        state.update(0.016, &input);

        let a_idx = state.entities.iter().position(|e| e.id == agent_id).unwrap();
        let nav = state.entities[a_idx].nav_agent.as_ref().unwrap();
        assert!(!nav.waypoints.is_empty(), "Le pathfinding A* doit avoir calculé un chemin de contournement");

        // Faire progresser la simulation pendant 2 secondes (120 ticks)
        for _ in 0..120 {
            state.update(0.016, &input);
        }

        let final_x = state.entities[a_idx].transform.x;
        assert!(final_x > 180.0, "L'agent doit avoir contourné le mur (x > 180, actuel: {:.1})", final_x);
    }

    #[test]
    fn test_vision_triggers_nav_agent_chase() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        let enemy_idx = state.entities.iter().position(|e| e.tag == "enemy").unwrap();

        // Attacher un NavAgent en mode Idle à l'ennemi
        state.entities[enemy_idx].nav_agent = Some(crate::navigation::NavAgent::new(80.0));

        let p_idx = state.player_index().unwrap();
        let e_x = state.entities[enemy_idx].transform.x;
        let e_y = state.entities[enemy_idx].transform.y;

        // Positionner le joueur dans le champ de vision dégagé
        state.entities[p_idx].transform.x = e_x - 70.0;
        state.entities[p_idx].transform.y = e_y;

        let input = InputState::new();
        state.update(0.016, &input);

        let nav = state.entities[enemy_idx].nav_agent.as_ref().unwrap();
        assert_eq!(nav.mode, crate::navigation::NavMode::ChasePlayer, "La détection visuelle doit automatiquement engager la traque A*");
    }

    #[test]
    fn test_add_navigation_component_in_studio() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        state.editor.mode = EditorMode::Edit;

        // Sélectionner une entité sans agent de navigation
        let wall_idx = state.entities.iter().position(|e| e.tag == "wall").unwrap();
        state.editor.selected = SelectedEntity::Entity(wall_idx);
        assert!(state.entities[wall_idx].nav_agent.is_none());

        // Attacher le composant Navigation
        let added = state.add_component_to_selected("Navigation");
        assert!(added);
        assert!(state.entities[wall_idx].nav_agent.is_some());
        assert_eq!(state.entities[wall_idx].nav_agent.as_ref().unwrap().mode, crate::navigation::NavMode::ChasePlayer);
    }

    #[test]
    fn test_flock_swarm_cohesion_and_target_following() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        let swarm_id = 42;

        // Spawner 4 drones de meute en formation proche autour de (100, 100)
        let drone_ids = [9601, 9602, 9603, 9604];
        let offsets = [(0.0, 0.0), (20.0, 5.0), (-15.0, 15.0), (10.0, -15.0)];

        for (i, &did) in drone_ids.iter().enumerate() {
            let is_leader = i == 0;
            let d = Entity::spawn_flock_drone(did, swarm_id, 100.0 + offsets[i].0, 100.0 + offsets[i].1, is_leader);
            state.entities.push(d);
        }

        // Définir une cible KVL pour toute la meute en (300, 100)
        state.kvl_context.set_var(format!("swarm_target_x_{}", swarm_id), Value::Int(300));
        state.kvl_context.set_var(format!("swarm_target_y_{}", swarm_id), Value::Int(100));

        let input = InputState::new();

        // Simuler 90 ticks (~1.5 seconde)
        for _ in 0..90 {
            state.update(0.016, &input);
        }

        // Vérifier que tous les drones se sont déplacés vers la droite (x a nettement progressé)
        for &did in &drone_ids {
            let idx = state.entities.iter().position(|e| e.id == did).unwrap();
            let x = state.entities[idx].transform.x;
            assert!(x > 150.0, "Le drone {} doit avoir progressé vers la cible (actuel: {})", did, x);
        }

        // Vérifier la cohésion : la distance entre le premier et les autres reste bornée (< 90 px)
        let lead_idx = state.entities.iter().position(|e| e.id == drone_ids[0]).unwrap();
        let lx = state.entities[lead_idx].transform.x;
        let ly = state.entities[lead_idx].transform.y;

        for &did in &drone_ids[1..] {
            let idx = state.entities.iter().position(|e| e.id == did).unwrap();
            let dx = state.entities[idx].transform.x - lx;
            let dy = state.entities[idx].transform.y - ly;
            let dist = (dx * dx + dy * dy).sqrt();
            assert!(dist < 90.0, "La cohésion de la meute doit être maintenue en vol (dist: {})", dist);
        }
    }

    #[test]
    fn test_flock_kvl_scatter_action_and_dispersion() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        let swarm_id = 99;

        // Créer 2 drones très proches en (200, 200) et (205, 200)
        let d1 = Entity::spawn_flock_drone(9701, swarm_id, 200.0, 200.0, false);
        let d2 = Entity::spawn_flock_drone(9702, swarm_id, 208.0, 200.0, false);
        state.entities.push(d1);
        state.entities.push(d2);

        // Déclencher le mode dispersion / panique via variable KVL
        state.kvl_context.set_var(format!("swarm_scatter_{}", swarm_id), Value::Float(2.0));

        let input = InputState::new();
        // 20 ticks de panique
        for _ in 0..20 {
            state.update(0.016, &input);
        }

        let idx1 = state.entities.iter().position(|e| e.id == 9701).unwrap();
        let idx2 = state.entities.iter().position(|e| e.id == 9702).unwrap();
        let x1 = state.entities[idx1].transform.x;
        let x2 = state.entities[idx2].transform.x;

        let final_dist = (x2 - x1).abs();
        assert!(final_dist > 20.0, "L'éparpillement doit avoir fortement séparé les drones (dist: {:.1})", final_dist);
    }

    #[test]
    fn test_add_flock_component_in_studio() {
        let mut state = GameState::load_from_scene("assets/scenes/level_01.scene.json").unwrap();
        state.editor.mode = EditorMode::Edit;

        let wall_idx = state.entities.iter().position(|e| e.tag == "wall").unwrap();
        state.editor.selected = SelectedEntity::Entity(wall_idx);
        assert!(state.entities[wall_idx].flock.is_none());

        let added = state.add_component_to_selected("Flock");
        assert!(added);
        assert!(state.entities[wall_idx].flock.is_some());
        assert_eq!(state.entities[wall_idx].flock.as_ref().unwrap().swarm_id, 1);
    }

    #[test]
    fn test_interactive_guide_tabs_and_initiation_steps() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let mut canvas = Canvas::new(1200, 800);

        // 1. Ouvrir le guide avec la touche H
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyH);
        state.update(0.016, &input);
        assert!(state.editor.show_help_modal);
        assert_eq!(state.editor.guide_tab, 0);

        // 2. Rendu de la modale d'aide pour générer les hitboxes d'onglets
        state.render(&mut canvas);
        assert_eq!(state.editor.guide_tab_btn_hitboxes.len(), 3);

        // 3. Cliquer sur l'onglet 1 (RACCOURCIS)
        let (_, tx, ty, tw, th) = state.editor.guide_tab_btn_hitboxes[1];
        let mut input_tab = InputState::new();
        input_tab.on_mouse_move((tx + (tw / 2) as i32) as f32, (ty + (th / 2) as i32) as f32);
        input_tab.on_mouse_down();
        state.update(0.016, &input_tab);
        assert_eq!(state.editor.guide_tab, 1, "Doit basculer vers l'onglet Raccourcis");

        // 4. Cliquer sur l'onglet 2 (CONCEPTS)
        let (_, tx2, ty2, tw2, th2) = state.editor.guide_tab_btn_hitboxes[2];
        let mut input_tab2 = InputState::new();
        input_tab2.on_mouse_move((tx2 + (tw2 / 2) as i32) as f32, (ty2 + (th2 / 2) as i32) as f32);
        input_tab2.on_mouse_down();
        state.update(0.016, &input_tab2);
        assert_eq!(state.editor.guide_tab, 2, "Doit basculer vers l'onglet Concepts");

        // 5. Fermer avec Échap
        let mut input_esc = InputState::new();
        input_esc.on_key_down(KeyCode::Escape);
        state.update(0.016, &input_esc);
        assert!(!state.editor.show_help_modal);

        // 6. Tester la complétion automatique des étapes du parcours d'initiation
        assert!(!state.editor.guide_step_done[0]);
        let mut input_move = InputState::new();
        input_move.on_key_down(KeyCode::KeyD);
        state.update(0.016, &input_move);
        assert!(state.editor.guide_step_done[0], "L'étape 1 (Déplacement) doit être validée après mouvement");

        // Bascule studio valide l'étape 4
        assert!(!state.editor.guide_step_done[3]);
        let mut input_tab_key = InputState::new();
        input_tab_key.on_key_down(KeyCode::Tab);
        state.update(0.016, &input_tab_key);
        assert!(state.editor.guide_step_done[3], "L'étape 4 (Studio) doit être validée");
    }

    #[test]
    fn test_welcome_onboarding_modal_interactions() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let mut canvas = Canvas::new(1200, 800);

        // Ouvrir le Hub de Projets & Accueil
        state.editor.show_welcome_modal = true;
        state.render(&mut canvas);

        assert!(state.editor.welcome_play_btn_hitbox.is_some());
        assert!(state.editor.welcome_studio_btn_hitbox.is_some());
        assert!(state.editor.welcome_guide_btn_hitbox.is_some());
        assert_eq!(state.editor.welcome_archetype_hitboxes.len(), 5, "Doit proposer les 5 archétypes en 1 clic dans l'onglet 1.1");

        // Cliquer sur le bouton Studio
        let (sx, sy, sw, sh) = state.editor.welcome_studio_btn_hitbox.unwrap();
        let mut input_studio = InputState::new();
        input_studio.on_mouse_move((sx + (sw / 2) as i32) as f32, (sy + (sh / 2) as i32) as f32);
        input_studio.on_mouse_down();
        state.update(0.016, &input_studio);

        assert!(!state.editor.show_welcome_modal, "La modale doit être fermée après sélection");
        assert_eq!(state.editor.mode, EditorMode::Edit, "Le mode Studio doit être activé");
    }

    #[test]
    fn test_project_hub_tab_switching_archetypes_and_scenes() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let mut canvas = Canvas::new(1200, 800);

        // 1. Ouvrir le Hub de Projets
        state.editor.show_welcome_modal = true;
        state.editor.welcome_tab = 0;
        state.render(&mut canvas);

        assert_eq!(state.editor.welcome_tab_hitboxes.len(), 3, "Doit avoir les 3 onglets 1.1, 1.2 et 1.3");
        assert_eq!(state.editor.welcome_archetype_hitboxes.len(), 5, "Doit afficher les 5 archétypes");

        // Sauvegarder un snapshot de l'onglet 1.1
        save_test_ppm(&canvas, "/tmp/desktop_hub_tab1.ppm");

        // 2. Basculer vers l'onglet 1 (Charger Scène sur Disque)
        let (_, tx, ty, tw, th) = state.editor.welcome_tab_hitboxes[1];
        let mut input_tab1 = InputState::new();
        input_tab1.on_mouse_move((tx + (tw / 2) as i32) as f32, (ty + (th / 2) as i32) as f32);
        input_tab1.on_mouse_down();
        state.update(0.016, &input_tab1);

        assert_eq!(state.editor.welcome_tab, 1, "L'onglet 1.2 doit être sélectionné");
        state.render(&mut canvas);
        assert_eq!(state.editor.welcome_scene_hitboxes.len(), 10, "5 scènes x 2 boutons (Jouer + Studio)");

        // Sauvegarder un snapshot de l'onglet 1.2
        save_test_ppm(&canvas, "/tmp/desktop_hub_tab2.ppm");

        // 3. Charger la scène Boss (index 4) en mode Jeu via le bouton [▶ JOUER]
        let (scene_idx, is_studio, bx, by, bw, bh) = state.editor.welcome_scene_hitboxes.iter()
            .find(|(idx, studio, ..)| *idx == 4 && !*studio)
            .cloned()
            .unwrap();
        assert_eq!(scene_idx, 4);
        assert!(!is_studio);

        let mut input_load_boss = InputState::new();
        input_load_boss.on_mouse_move((bx + (bw / 2) as i32) as f32, (by + (bh / 2) as i32) as f32);
        input_load_boss.on_mouse_down();
        state.update(0.016, &input_load_boss);

        assert!(!state.editor.show_welcome_modal, "Le Hub se ferme au chargement de projet");
        assert!(state.scene_path.contains("boss_01"), "La scène Boss doit être chargée");

        // 4. Ré-ouvrir le Hub et instancier l'archétype Platformer en 1 clic
        state.editor.show_welcome_modal = true;
        state.editor.welcome_tab = 0;
        state.render(&mut canvas);

        let (arch_key, ax, ay, aw, ah) = state.editor.welcome_archetype_hitboxes.iter()
            .find(|(k, ..)| *k == "platformer")
            .cloned()
            .unwrap();
        assert_eq!(arch_key, "platformer");

        let mut input_arch = InputState::new();
        input_arch.on_mouse_move((ax + (aw / 2) as i32) as f32, (ay + (ah / 2) as i32) as f32);
        input_arch.on_mouse_down();
        state.update(0.016, &input_arch);

        assert!(!state.editor.show_welcome_modal);
        let p_idx = state.player_index().unwrap();
        assert!(matches!(state.entities[p_idx].movement.as_ref().unwrap().mode, MovementMode::Platformer { .. }), "L'archétype Platformer doit être appliqué avec succès");
    }

    #[test]
    fn test_ai_patch_trigger_and_review_lifecycle() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        state.editor.mode = EditorMode::Edit;
        let mut canvas = Canvas::new(1200, 800);
        let initial_entity_count = state.entities.len();

        // 1. Ouvrir l'Assistant IA via touche I
        let mut input_i = InputState::new();
        input_i.on_key_down(KeyCode::KeyI);
        state.update(0.016, &input_i);
        assert!(state.editor.show_intent_bar, "La modale IA doit s'ouvrir avec la touche I");

        // 2. Rendu pour générer les hitboxes des presets
        state.render(&mut canvas);
        assert_eq!(state.editor.intent_preset_hitboxes.len(), 4, "Doit exposer 4 suggestions d'intentions rapides");

        // 3. Cliquer sur le Preset 0 (Garde hostile A*)
        let (_, px, py, pw, ph) = state.editor.intent_preset_hitboxes[0];
        let mut input_click = InputState::new();
        input_click.on_mouse_move((px + (pw / 2) as i32) as f32, (py + (ph / 2) as i32) as f32);
        input_click.on_mouse_down();
        state.update(0.016, &input_click);

        // La modale IA se referme et le patch IA est généré en attente de révision
        assert!(!state.editor.show_intent_bar);
        assert!(state.editor.pending_ai_patch.is_some(), "Un patch IA doit être en cours de révision");
        let patch = state.editor.pending_ai_patch.as_ref().unwrap();
        assert_eq!(patch.sandbox_report.simulation_steps_tested, 100);
        assert!(patch.sandbox_report.simulation_passed);
        assert!(state.entities.len() > initial_entity_count, "Les entités du patch doivent être injectées pour prévisualisation");

        // 4. Rendu de la Review Bar et vérification des hitboxes
        state.render(&mut canvas);
        assert!(state.editor.ai_patch_accept_hitbox.is_some());
        assert!(state.editor.ai_patch_reject_hitbox.is_some());

        // 5. Accepter le patch via Entrée
        let mut input_enter = InputState::new();
        input_enter.on_key_down(KeyCode::Enter);
        state.update(0.016, &input_enter);

        assert!(state.editor.pending_ai_patch.is_none(), "Le patch doit être consommé et validé");
        assert_eq!(state.editor.ai_patch_history.len(), 1, "Le patch doit être enregistré dans l'historique d'audit");
        assert!(!state.editor.undo_stack.undo_list.is_empty(), "La commande réversible doit être sur la pile Undo");

        // 6. Test du Rollback Undo / Redo
        state.undo();
        assert_eq!(state.entities.len(), initial_entity_count, "L'annulation doit restaurer le nombre d'entités exact");

        state.redo();
        assert!(state.entities.len() > initial_entity_count, "Le rétablissement doit restaurer les entités du patch");
    }

    #[test]
    fn test_universal_ai_provider_bridge_and_switching() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        state.editor.mode = EditorMode::Edit;
        let mut canvas = Canvas::new(1200, 800);

        // Ouvrir modale IA
        state.editor.show_intent_bar = true;
        state.render(&mut canvas);

        // 7 fournisseurs d'IA exposés
        assert_eq!(state.editor.ai_provider_btn_hitboxes.len(), 7, "Doit exposer les 7 fournisseurs (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI, Offline)");

        // Cliquer sur le badge Ollama (index 0)
        let (prov, px, py, pw, ph) = state.editor.ai_provider_btn_hitboxes[0];
        assert_eq!(prov, crate::ai_client::AiProvider::Ollama);

        let mut input_click = InputState::new();
        input_click.on_mouse_move((px + (pw / 2) as i32) as f32, (py + (ph / 2) as i32) as f32);
        input_click.on_mouse_down();
        state.update(0.016, &input_click);

        assert_eq!(state.editor.ai_config.provider, crate::ai_client::AiProvider::Ollama);

        // Tester l'injection et la validation sandbox d'un payload LLM structuré
        let fake_payload = crate::ai_client::AiGeneratedGamePayload {
            title: "Simulated Alien Boss".into(),
            description: "Test d'intégration du pont LLM".into(),
            primary_genre: Some("ShootEmUp".into()),
            terrain: None,
            gameplay_loop: None,
            entities: vec![
                crate::ai_client::AiGeneratedEntity {
                    name: "Alien_Boss".into(),
                    kind: "boss".into(),
                    offset_x: 200.0,
                    offset_y: 0.0,
                    width: 48.0,
                    height: 48.0,
                    health: 50.0,
                    color_hex: Some("#F43F5E".into()),
                    sprite_archetype: Some("boss".into()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    movement: Some("patrol".into()),
                    has_collider: true,
                    has_vision: true,
                    is_flocking: false,
                }
            ],
            kvl_actions: vec![
                crate::ai_client::AiGeneratedKvlAction {
                    event: "OnDamage".into(),
                    action: "SpawnParticles:burst".into(),
                    target_entity: None,
                    condition: None,
                    on_true_action: None,
                    on_false_action: None,
                    second_action: None,
                }
            ],
            variables: None,
        };

        state.apply_ai_payload(fake_payload, "Prompt de test");
        assert!(state.editor.pending_ai_patch.is_some());
        let patch = state.editor.pending_ai_patch.as_ref().unwrap();
        assert_eq!(patch.title, "Simulated Alien Boss");
        assert!(patch.sandbox_report.simulation_passed, "Le bac à sable doit valider le graphe généré par le LLM");
    }

    #[test]
    fn test_ai_patch_rejection_and_instant_rollback() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        state.editor.mode = EditorMode::Edit;
        let mut canvas = Canvas::new(1200, 800);
        let initial_entity_count = state.entities.len();

        // Déclencher un prompt personnalisé
        state.trigger_ai_intent("coffre magique avec clé secrète");
        assert!(state.editor.pending_ai_patch.is_some());
        assert!(state.entities.len() > initial_entity_count);

        // Rendu pour vérifier l'absence de panique avec halos
        state.render(&mut canvas);

        // Rejeter le patch via Échap
        let mut input_esc = InputState::new();
        input_esc.on_key_down(KeyCode::Escape);
        state.update(0.016, &input_esc);

        assert!(state.editor.pending_ai_patch.is_none());
        assert_eq!(state.entities.len(), initial_entity_count, "Le rejet doit restaurer immédiatement la scène antérieure");
    }

    #[test]
    fn test_interactive_badge_hover_and_tooltips() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let mut canvas = Canvas::new(1200, 800);

        // Placer la souris au-dessus du badge AIDE (rx ~ w - 82, ry ~ 8)
        let mut input = InputState::new();
        input.on_mouse_move(1200.0 - 50.0, 16.0);
        state.update(0.016, &input);

        // Rendu
        state.render(&mut canvas);

        // Une infobulle active doit être enregistrée
        assert!(state.editor.active_tooltip.is_some(), "Un tooltip doit être actif lors du survol d'un badge");
        let (title, shortcut, _, _, _) = state.editor.active_tooltip.as_ref().unwrap();
        assert_eq!(title, "Guide Pédagogique");
        assert_eq!(shortcut, "H");
    }

    #[test]
    fn test_standalone_exporter_lifecycle_and_modal() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let mut canvas = Canvas::new(1200, 800);

        // 1. Rendu initial pour générer le bouton [📦 EXPORT] dans l'en-tête
        state.render(&mut canvas);
        assert!(state.editor.export_btn_hitbox.is_some(), "Le bouton EXPORT doit avoir une hitbox dans l'en-tête");

        // 2. Bascule de la modale d'export via la touche U
        let mut input_u = InputState::new();
        input_u.on_key_down(KeyCode::KeyU);
        state.update(0.016, &input_u);
        assert!(state.editor.show_export_modal, "La modale d'export doit s'ouvrir avec la touche U");

        // 3. Rendu pour générer les hitboxes de la modale
        state.render(&mut canvas);
        assert_eq!(state.editor.export_format_hitboxes.len(), 3, "Doit exposer les 3 formats d'export");
        assert!(state.editor.export_action_btn_hitbox.is_some(), "Le bouton d'action EXPORTER doit avoir une hitbox");
        assert!(state.editor.export_close_btn_hitbox.is_some(), "Le bouton de fermeture doit avoir une hitbox");

        // 4. Navigation au clavier : cycle des formats avec Tab
        assert_eq!(state.editor.export_format_idx, 0);
        let mut input_tab = InputState::new();
        input_tab.on_key_down(KeyCode::Tab);
        state.update(0.016, &input_tab);
        assert_eq!(state.editor.export_format_idx, 1, "Tab doit passer au format suivant");

        // 5. Clic souris sur le format HTML (index 0)
        let (_, fx, fy, fw, fh) = state.editor.export_format_hitboxes[0];
        let mut input_fmt_click = InputState::new();
        input_fmt_click.on_mouse_move((fx + (fw / 2) as i32) as f32, (fy + (fh / 2) as i32) as f32);
        input_fmt_click.on_mouse_down();
        state.update(0.016, &input_fmt_click);
        assert_eq!(state.editor.export_format_idx, 0, "Le clic doit sélectionner le format HTML5");

        // 6. Déclenchement de l'exportation via Entrée
        let mut input_enter = InputState::new();
        input_enter.on_key_down(KeyCode::Enter);
        state.update(0.016, &input_enter);

        assert!(!state.editor.show_export_modal, "La modale doit se fermer après confirmation d'export");
        assert!(state.status_message.contains("exporté"), "Un message de confirmation doit s'afficher");

        // 7. Vérification de l'intégrité du package généré et réimportation
        let pkg_json = crate::exporter::StandaloneExporter::export_package_json(&state, "Jeu Test", "Testeur").unwrap();
        let loaded_pkg = crate::exporter::KaractiGamePackage::from_json(&pkg_json).unwrap();
        assert_eq!(loaded_pkg.metadata.title, "Jeu Test");
        assert_eq!(loaded_pkg.metadata.author, "Testeur");
        assert_eq!(loaded_pkg.scene_name, state.scene_name);
        assert_eq!(loaded_pkg.entities.len(), state.entities.len());

        // Importation dans un nouvel état
        let mut imported_state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        crate::exporter::StandaloneExporter::import_package(&mut imported_state, &loaded_pkg);
        assert_eq!(imported_state.entities.len(), state.entities.len());
        assert_eq!(imported_state.scene_name, state.scene_name);
    }

    #[test]
    fn test_desktop_native_recipe_wizard_interaction_and_instantiation() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).expect("Scène chargée");
        let mut canvas = Canvas::new(1280, 720);

        // 1. Vérification de la présence du bouton RECETTE dans l'en-tête natif
        state.render(&mut canvas);
        assert!(state.editor.recipe_btn_hitbox.is_some(), "Le bouton RECETTE doit être visible et cliquable dans l'en-tête");

        // 2. Ouverture de la modale via la touche W
        assert!(!state.editor.recipe_wizard.is_open);
        let mut input_w = InputState::new();
        input_w.on_key_down(KeyCode::KeyW);
        state.update(0.016, &input_w);
        assert!(state.editor.recipe_wizard.is_open, "Touche W doit ouvrir l'Assistant de Recette");
        assert_eq!(state.editor.recipe_wizard.step, 1);

        // 3. Rendu pour générer les hitboxes de l'étape 1
        state.render(&mut canvas);
        assert_eq!(state.editor.recipe_wizard.step_tab_hitboxes.len(), 6, "Les 6 étapes doivent être affichées");
        assert_eq!(state.editor.recipe_wizard.archetype_hitboxes.len(), 5, "Les 5 archétypes rapides doivent être affichés");
        assert!(state.editor.recipe_wizard.apply_btn_hitbox.is_some(), "Le bouton INSTANCIER doit avoir une hitbox");

        // Sauvegarder un snapshot visuel de l'étape 1
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::File::create("/tmp/karactirage_recipe_step1.ppm") {
                let _ = writeln!(f, "P6\n1280 720\n255");
                let mut bytes = Vec::with_capacity(1280 * 720 * 3);
                for &pixel in &canvas.buffer {
                    bytes.push(((pixel >> 16) & 0xFF) as u8);
                    bytes.push(((pixel >> 8) & 0xFF) as u8);
                    bytes.push((pixel & 0xFF) as u8);
                }
                let _ = f.write_all(&bytes);
            }
        }

        // 4. Clic sur l'archétype Platformer
        let (_, px, py, pw, ph) = state.editor.recipe_wizard.archetype_hitboxes.iter()
            .find(|(k, ..)| *k == "platformer")
            .cloned()
            .expect("Archétype platformer présent");

        let mut click_arch = InputState::new();
        click_arch.on_mouse_move((px + (pw / 2) as i32) as f32, (py + (ph / 2) as i32) as f32);
        click_arch.on_mouse_down();
        state.update(0.016, &click_arch);

        assert_eq!(state.editor.recipe_wizard.selected_archetype, Some("platformer"));
        assert!(state.editor.recipe_wizard.recipe.world.gravity_enabled, "Le platformer requiert la gravité");
        assert!(state.editor.recipe_wizard.recipe.player.abilities.jump, "Le platformer requiert le saut");

        // 5. Navigation vers l'étape 6 (Validation et Audit du résolveur de dépendances)
        state.editor.recipe_wizard.step = 6;
        state.editor.recipe_wizard.run_audit();
        state.render(&mut canvas);

        let report = state.editor.recipe_wizard.validation_report.as_ref().expect("Rapport d'audit présent");
        assert!(report.is_valid, "La recette Platformer doit être 100% cohérente");

        // Sauvegarder un snapshot visuel de l'étape 6
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::File::create("/tmp/karactirage_recipe_step6.ppm") {
                let _ = writeln!(f, "P6\n1280 720\n255");
                let mut bytes = Vec::with_capacity(1280 * 720 * 3);
                for &pixel in &canvas.buffer {
                    bytes.push(((pixel >> 16) & 0xFF) as u8);
                    bytes.push(((pixel >> 8) & 0xFF) as u8);
                    bytes.push((pixel & 0xFF) as u8);
                }
                let _ = f.write_all(&bytes);
            }
        }

        // 6. Instanciation du jeu complet en direct via Entrée
        let mut input_enter = InputState::new();
        input_enter.on_key_down(KeyCode::Enter);
        state.update(0.016, &input_enter);

        assert!(!state.editor.recipe_wizard.is_open, "La modale doit être fermée après instanciation");
        assert_eq!(state.editor.mode, EditorMode::Play, "Le jeu doit passer immédiatement en mode Play");
        assert!(state.player_entity().is_some(), "Le héros doit être instancié dans la scène");
        assert!(!state.entities.is_empty(), "Les blocs et entités doivent être générés");

        // Vérification de la physique du joueur (Plateforme avec gravité)
        let player = state.player_entity().unwrap();
        assert!(matches!(player.movement.as_ref().unwrap().mode, MovementMode::Platformer { .. }));
    }

    #[test]
    fn test_welcome_ai_tab_configuration_and_azerty_keyboard() {
        let mut state = GameState::load_from_scene(get_test_scene_path()).unwrap();
        let mut canvas = Canvas::new(1200, 800);

        // 1. Ouvrir le Hub de Projets & Accueil
        state.editor.show_welcome_modal = true;
        state.editor.welcome_tab = 0;
        state.render(&mut canvas);
        save_test_ppm(&canvas, "/tmp/desktop_hub_tab0.ppm");

        assert_eq!(state.editor.welcome_tab_hitboxes.len(), 3, "Doit exposer 3 onglets (1.1, 1.2, 1.3)");
        assert!(state.editor.welcome_hero_ai_btn_hitbox.is_some(), "Le bouton héro IA doit être présent dans l'onglet 1.1");
        assert!(state.editor.welcome_bottom_ai_btn_hitbox.is_some(), "Le bouton IA inférieur doit être présent");

        // 2. Basculer vers l'onglet 1.3 Intention IA via touche I
        let mut input_i = InputState::new();
        input_i.on_key_down(KeyCode::KeyI);
        state.update(0.016, &input_i);
        assert_eq!(state.editor.welcome_tab, 2, "L'onglet 1.3 Intention IA doit être sélectionné");

        // 3. Rendu de l'onglet 1.3
        state.render(&mut canvas);
        save_test_ppm(&canvas, "/tmp/desktop_hub_tab2_ai.ppm");

        assert_eq!(state.editor.welcome_ai_provider_hitboxes.len(), 7, "Doit exposer les 7 providers LLM (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI, Offline)");
        assert_eq!(state.editor.welcome_ai_preset_hitboxes.len(), 4, "Doit exposer 4 presets de mondes");
        assert!(state.editor.welcome_ai_test_btn_hitbox.is_some(), "Le bouton de test LLM doit être cliquable");
        assert!(state.editor.welcome_ai_generate_btn_hitbox.is_some(), "Le bouton de génération doit être présent");
        assert!(state.editor.keyboard_layout_hitbox.is_some(), "Le badge de disposition clavier doit être cliquable");

        // 4. Test du clic sur le bouton de test de connexion LLM
        let (tx, ty, tw, th) = state.editor.welcome_ai_test_btn_hitbox.unwrap();
        let mut input_test = InputState::new();
        input_test.on_mouse_move((tx + (tw / 2) as i32) as f32, (ty + (th / 2) as i32) as f32);
        input_test.on_mouse_down();
        state.update(0.016, &input_test);
        assert!(state.editor.ai_test_result.is_some(), "Le test LLM doit avoir produit un résultat");

        // 5. Test de saisie fluide avec caractères accentués français (AZERTY)
        state.editor.intent_input_text.clear();
        let mut input_chars = InputState::new();
        input_chars.on_char('d');
        input_chars.on_char('o');
        input_chars.on_char('n');
        input_chars.on_char('j');
        input_chars.on_char('o');
        input_chars.on_char('n');
        input_chars.on_char(' ');
        input_chars.on_char('p');
        input_chars.on_char('i');
        input_chars.on_char('è');
        input_chars.on_char('g');
        input_chars.on_char('é');
        state.update(0.016, &input_chars);
        assert_eq!(state.editor.intent_input_text, "donjon piègé");

        // 6. Test de bascule de disposition de clavier (AZERTY <-> QWERTY)
        let (kx, ky, kw, kh) = state.editor.keyboard_layout_hitbox.unwrap();
        let mut input_kbd = InputState::new();
        input_kbd.on_mouse_move((kx + (kw / 2) as i32) as f32, (ky + (kh / 2) as i32) as f32);
        input_kbd.on_mouse_down();
        state.update(0.016, &input_kbd);
        assert_eq!(state.editor.keyboard_layout, crate::editor::KeyboardLayout::UsQwerty);

        // 7. Test de la limite stricte de 400 caractères
        state.editor.intent_input_text.clear();
        let mut input_overflow = InputState::new();
        for _ in 0..450 {
            input_overflow.on_char('a');
        }
        state.update(0.016, &input_overflow);
        assert_eq!(state.editor.intent_input_text.chars().count(), 400, "Le prompt ne doit pas dépasser 400 caractères");

        // 8. Rendu et capture de la boîte de dialogue Assistant IA dans l'Atelier Studio
        state.editor.show_welcome_modal = false;
        state.editor.mode = EditorMode::Edit;
        state.editor.show_intent_bar = true;
        state.editor.intent_input_text = "Créer un boss démon cracheur de lave avec pièges et énigme Sokoban".to_string();
        state.render(&mut canvas);
        save_test_ppm(&canvas, "/tmp/desktop_studio_ai_modal.ppm");
        assert!(state.editor.keyboard_layout_hitbox.is_some());
        assert_eq!(state.editor.intent_preset_hitboxes.len(), 4);
    }

    #[test]
    fn test_blank_project_initialization_starts_in_edit_mode() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement initial");

        state.new_blank_project("Mon Studio Vierge", crate::recipe::GameGenre::Platformer);

        assert_eq!(state.scene_name, "Mon Studio Vierge");
        assert_eq!(state.editor.mode, EditorMode::Edit, "Un projet vierge doit démarrer en mode Studio Edit");
        assert!(state.entities.len() >= 2, "Doit contenir le joueur et le sol de référence");
        assert!(state.entities.iter().any(|e| e.tag == "player"));
        assert!(state.entities.iter().any(|e| e.tag == "platform"));
    }

    #[test]
    fn test_export_current_project_and_reload_roundtrip() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement initial");

        let exported = state.export_current_project();
        assert_eq!(exported.metadata.title, state.scene_name);
        assert_eq!(exported.scene.entities.len(), state.entities.len());

        let json = exported.to_json().expect("Sérialisation JSON du projet");
        let loaded_proj = crate::project::KaractiProject::from_json(&json).expect("Désérialisation JSON");

        state.load_project(loaded_proj);
        assert_eq!(state.entities.len(), exported.scene.entities.len());
    }

    #[test]
    fn test_get_entities_summary_json_contains_all_fields() {
        let state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement initial");

        let json_summary = state.get_entities_summary_json();
        assert!(json_summary.contains("\"id\":"));
        assert!(json_summary.contains("\"name\":"));
        assert!(json_summary.contains("\"tag\":"));
        assert!(json_summary.contains("\"x\":"));
        assert!(json_summary.contains("\"y\":"));
        assert!(json_summary.contains("\"is_solid\":"));
    }

    #[test]
    fn test_update_and_delete_entity_by_id() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement initial");

        let initial_count = state.entities.len();
        let target_id = state.entities[0].id;

        // Mise à jour
        let ok = state.update_entity_transform_by_id(target_id, 450.0, 320.0, 60.0, 60.0);
        assert!(ok);
        assert_eq!(state.entities[0].transform.x, 450.0);
        assert_eq!(state.entities[0].transform.w, 60.0);

        // Suppression
        let del_ok = state.delete_entity_by_id(target_id);
        assert!(del_ok);
        assert_eq!(state.entities.len(), initial_count - 1);
        assert!(!state.entities.iter().any(|e| e.id == target_id));
    }
}



