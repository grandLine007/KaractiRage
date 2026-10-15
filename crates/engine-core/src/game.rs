use crate::adaptation::AdaptationManager;
use crate::audio::{AudioEngine, SoundType};
use crate::editor::{EditorMode, EditorState, SelectedEntity};
use crate::entity::{Entity, ShapeKind};
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
            _ => return false,
        }
        self.audio.play(SoundType::UiClick);
        true
    }

    pub fn load_from_scene(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let scene_data = match SceneData::load_from_file(path) {
            Ok(s) => s,
            Err(_) => {
                let default_json = include_str!("../../../assets/scenes/level_01.scene.json");
                SceneData::from_json_str(default_json)?
            }
        };

        let entities = scene_data.to_entities();

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

        Ok(Self {
            scene_path: path.to_string(),
            scene_name: scene_data.name,
            entities,
            particles: Vec::new(),
            kvl_graphs,
            kvl_context,
            status_message: "Moteur KaractiRage initialisé ! Modèle ECS universel actif.".into(),
            telemetry: Telemetry::new(),
            adaptation: AdaptationManager::new(),
            editor: EditorState::new(),
            audio: AudioEngine::new(),
            show_minimap: true,
            spatial_grid: SpatialHashGrid::new(64.0, 256),
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

    pub fn save_scene_and_scripts(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Sauvegarder la scène JSON universelle avec les entités génériques
        let mut saved_entities = self.entities.clone();
        for ent in &mut saved_entities {
            ent.is_active = true;
        }

        let scene_data = SceneData {
            name: self.scene_name.clone(),
            description: "Scène universelle KaractiRage avec entités ECS et scripts KVL.".into(),
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

                // 5. Sélection universelle d'entité à la souris
                let mut found_idx = None;
                for (idx, ent) in self.entities.iter().enumerate().rev() {
                    let t = &ent.transform;
                    if mx >= t.x - 8.0 && mx <= t.x + t.w + 8.0 && my >= t.y - 8.0 && my <= t.y + t.h + 8.0 {
                        found_idx = Some(idx);
                        break;
                    }
                }

                if let Some(idx) = found_idx {
                    self.editor.selected = SelectedEntity::Entity(idx);
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.entities[idx].transform.x - mx;
                    self.editor.drag_offset_y = self.entities[idx].transform.y - my;
                    self.status_message = format!("Entité '{}' ({}) sélectionnée.", self.entities[idx].name, self.entities[idx].tag);
                    return;
                }

                // Clic dans la zone de jeu vide -> désélectionner
                if mx < 720.0 && my >= 65.0 && my <= 620.0 {
                    self.editor.selected = SelectedEntity::None;
                }
            }

            if input.mouse_just_released {
                self.editor.is_dragging = false;
            }

            // Déplacement universel de l'entité sélectionnée à la souris
            if self.editor.is_dragging && input.mouse_pressed {
                if let SelectedEntity::Entity(idx) = self.editor.selected {
                    if idx < self.entities.len() {
                        let new_x = (input.mouse_x + self.editor.drag_offset_x).clamp(20.0, 1150.0);
                        let new_y = (input.mouse_y + self.editor.drag_offset_y).clamp(70.0, 580.0);
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
        if input.is_key_just_pressed(KeyCode::KeyR) {
            self.reload();
            return;
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
        // SYSTEM 2 : PHYSIQUE ET GLISSEMENT AABB POUR LE JOUEUR
        // ==========================================
        let mut player_pos = None;
        if let Some(p_idx) = self.player_index() {
            let (px, py, pw, ph, speed) = {
                let p = &mut self.entities[p_idx];
                let speed = p.movement.as_ref().map_or(240.0, |m| m.speed) + speed_boost;
                (p.transform.x, p.transform.y, p.transform.w, p.transform.h, speed)
            };

            let mut cur_x = px;
            let mut cur_y = py;

            // Déplacement & résolution AABB axe X
            if move_x != 0.0 {
                cur_x += move_x * speed * dt;
                cur_x = cur_x.clamp(30.0, 1160.0 - pw);

                let candidates = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                for item in candidates {
                    if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                        if move_x > 0.0 {
                            cur_x = item.x - pw;
                        } else if move_x < 0.0 {
                            cur_x = item.x + item.w;
                        }
                    }
                }
            }

            // Déplacement & résolution AABB axe Y
            if move_y != 0.0 {
                cur_y += move_y * speed * dt;
                cur_y = cur_y.clamp(75.0, 595.0 - ph);

                let candidates = self.spatial_grid.query_aabb(cur_x, cur_y, pw, ph);
                for item in candidates {
                    if item.id != p_idx && matches!(item.kind, SpatialKind::Solid) {
                        if move_y > 0.0 {
                            cur_y = item.y - ph;
                        } else if move_y < 0.0 {
                            cur_y = item.y + item.h;
                        }
                    }
                }
            }

            self.entities[p_idx].transform.x = cur_x;
            self.entities[p_idx].transform.y = cur_y;
            player_pos = Some((p_idx, cur_x, cur_y, pw, ph));
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
        // SYSTEM 4 : TRIGGER OVERLAP & COLLECTIBLES (PIÈCES, BONUS)
        // ==========================================
        if let Some((_p_idx, px, py, pw, ph)) = player_pos {
            let mut collected_indices = Vec::new();
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
                            if dist < max_reach {
                                collected_indices.push(idx);
                            }
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

            // Brand Badge Studio
            canvas.draw_badge(16, 12, 130, 32, 0x002E1065, 0x00A855F7, "STUDIO ECS", 0x00F3E8FF, 2);

            // Bouton 1 : [+ SPAWN (P)]
            let spawn_x = 156;
            let spawn_y = 12;
            let spawn_w = 125;
            let spawn_h = 32;
            let spawn_bg = if self.editor.show_spawn_palette { 0x007C3AED } else { 0x003730A3 };
            canvas.draw_badge(spawn_x, spawn_y, spawn_w as u32, spawn_h as u32, spawn_bg, 0x00818CF8, "[+ SPAWN (P)]", 0x00FFFFFF, 2);
            self.editor.spawn_btn_hitbox = Some((spawn_x, spawn_y, spawn_w as u32, spawn_h as u32));

            // Bouton 2 : [SAUVER (C+S)]
            let save_x = 290;
            let save_y = 12;
            let save_w = 135;
            let save_h = 32;
            canvas.draw_badge(save_x, save_y, save_w as u32, save_h as u32, 0x00064E3B, 0x0010B981, "[SAUVER (C+S)]", 0x00D1FAE5, 2);
            self.editor.top_save_btn_hitbox = Some((save_x, save_y, save_w as u32, save_h as u32));

            // Boutons contextuels si une entité est sélectionnée
            if let SelectedEntity::Entity(idx) = self.editor.selected {
                if idx < self.entities.len() {
                    let dup_x = 434;
                    let dup_y = 12;
                    let dup_w = 150;
                    let dup_h = 32;
                    canvas.draw_badge(dup_x, dup_y, dup_w as u32, dup_h as u32, 0x0078350F, 0x00F59E0B, "[DUPLIQUER (C+D)]", 0x00FEF3C7, 2);
                    self.editor.duplicate_btn_hitbox = Some((dup_x, dup_y, dup_w as u32, dup_h as u32));

                    let is_player = self.entities[idx].movement.as_ref().map_or(false, |m| m.is_player_controlled);
                    if !is_player {
                        let del_x = 592;
                        let del_y = 12;
                        let del_w = 135;
                        let del_h = 32;
                        canvas.draw_badge(del_x, del_y, del_w as u32, del_h as u32, 0x007F1D1D, 0x00EF4444, "[SUPPR (DEL)]", 0x00FEE2E2, 2);
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
            canvas.draw_badge(164, 12, 95, 32, 0x001E293B, 0x00334155, "LVL 01", 0x0094A3B8, 2);

            let score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };

            // Carte d'objectif pédagogique dynamique au centre
            let (obj_txt, obj_bg, obj_bd, obj_fg) = if score < 50 {
                ("OBJECTIF: COLLECTEZ 50 OR POUR LA PORTE", 0x00451A03, 0x00D97706, 0x00FDE047)
            } else {
                ("OBJECTIF ATTEINT ! PORTE OUVERTE, SORTEZ", 0x00064E3B, 0x0010B981, 0x006EE7B7)
            };
            canvas.draw_badge(268, 12, 400, 32, obj_bg, obj_bd, obj_txt, obj_fg, 2);

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

        let frame_border = if is_edit_mode { 0x004C1D95 } else { 0x001E293B };
        let bracket_col = if is_edit_mode { 0x00A855F7 } else { 0x0038BDF8 };
        canvas.draw_rect_outline(20, 66, play_area_w, play_area_h, frame_border, 2);
        canvas.draw_corner_brackets(20, 66, play_area_w, play_area_h, 16, bracket_col);

        // Légende discrète de zone de jeu
        let viewport_tag = if is_edit_mode {
            "STUDIO VIEWPORT (CLIQUEZ / GLISSEZ UNE ENTITE | P: SPAWN)"
        } else {
            "MONDE 2D ECS (ZQSD / FLECHES : DEPLACEMENT | E / ESPACE : INTERACTION)"
        };
        canvas.draw_text(30, 72, viewport_tag, 0x00475569, 1);

        // Visualisation de débogage de la Grille Spatiale (Touche G)
        if self.spatial_grid.debug_visible {
            let max_x = 20 + play_area_w as i32;
            let max_y = 66 + play_area_h as i32;
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
            let max_x = 20 + play_area_w as i32;
            let max_y = 66 + play_area_h as i32;
            for gx in (30..max_x).step_by(40) {
                for gy in (90..max_y).step_by(40) {
                    canvas.draw_rect(gx, gy, 2, 2, 0x001E293B);
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
            let rend = ent.renderable.as_ref().unwrap();
            let is_selected = is_edit_mode && self.editor.selected == SelectedEntity::Entity(idx);

            match rend.shape {
                ShapeKind::Rect => {
                    let rx = ent.transform.x as i32;
                    let ry = ent.transform.y as i32;
                    let rw = ent.transform.w as u32;
                    let rh = ent.transform.h as u32;

                    canvas.draw_rect(rx, ry, rw, rh, rend.color);

                    if let Some(outline_col) = rend.outline_color {
                        canvas.draw_rect_outline(rx, ry, rw, rh, outline_col, rend.outline_width);
                    }

                    if is_selected {
                        canvas.draw_rect_outline(rx - 4, ry - 4, rw + 8, rh + 8, 0x00FBBF24, 3);
                        let label = format!("{}: {}", ent.tag.to_uppercase(), ent.name);
                        canvas.draw_text(rx, ry - 22, &label, 0x00FBBF24, 2);
                    }
                }
                ShapeKind::Circle { radius } => {
                    let cx = (ent.transform.x + radius) as i32;
                    let cy = (ent.transform.y + radius) as i32;
                    canvas.draw_circle(cx, cy, radius as i32, rend.color);
                    canvas.draw_circle(cx, cy, (radius * 0.6) as i32, 0x00FDE047);

                    if is_selected {
                        let sz = (radius + 6.0) * 2.0;
                        canvas.draw_rect_outline(
                            cx - radius as i32 - 6,
                            cy - radius as i32 - 6,
                            sz as u32,
                            sz as u32,
                            0x00FBBF24,
                            3,
                        );
                        let label = format!("{}: {}", ent.tag.to_uppercase(), ent.name);
                        canvas.draw_text(cx - 20, cy - radius as i32 - 24, &label, 0x00FBBF24, 2);
                    }
                }
                ShapeKind::Text { ref label, scale } => {
                    canvas.draw_text(ent.transform.x as i32, ent.transform.y as i32, label, rend.color, scale as usize);
                }
            }

            // Rendu des cônes de vision et alertes pour les entités douées de perception
            if let Some(ref vision) = ent.vision {
                let ex = ent.transform.center_x() as i32;
                let ey = ent.transform.center_y() as i32;
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
                    canvas.draw_text(ex - 12, ey - (ent.transform.h as i32 / 2) - 26, "(!)", 0x00EF4444, 2);
                }
            }
        }

        // Particules
        for p in &self.particles {
            canvas.draw_rect(p.x as i32, p.y as i32, 3, 3, p.current_color());
        }

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

            let scale_x = (mm_w - 14) as f32 / 1130.0;
            let scale_y = (mm_h - 32) as f32 / 520.0;

            let map_to_radar = |rx: f32, ry: f32| -> (i32, i32) {
                let px = mm_x + 7 + ((rx - 30.0) * scale_x) as i32;
                let py = mm_y + 22 + ((ry - 75.0) * scale_y) as i32;
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
                    _ => {
                        canvas.draw_rect(ex, ey, 3, 3, 0x0094A3B8);
                    }
                }
            }
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
                        if let Some(ref col) = ent.collider {
                            let col_desc = if col.is_solid { "Collider2D : Solide AABB (Bloquant)" } else { "Collider2D : Trigger (Zone detection)" };
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x00064E3B, 0x0010B981, col_desc, 0x00A7F3D0, 1);
                            comp_y += 30;
                        }
                        if let Some(ref mov) = ent.movement {
                            let mov_desc = if mov.is_player_controlled { "Movement2D : Joueur (Controle par ZQSD)" } else { "Movement2D : Patrouille IA autonome" };
                            canvas.draw_badge(insp_x + 18, comp_y, (insp_w - 36) as u32, 24, 0x000C4A6E, 0x000284C7, mov_desc, 0x00BAE6FD, 1);
                            comp_y += 30;
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
                        if ent.script.is_none() {
                            canvas.draw_badge(insp_x + 18, comp_y, btn_w, btn_h, 0x001E293B, 0x00A78BFA, "[ + SCRIPT KVL (LOGIQUE) ]", 0x00A78BFA, 2);
                            self.editor.add_comp_hitboxes.push(("Script".into(), insp_x + 18, comp_y, btn_w, btn_h));
                        }
                    }
                }
                _ => {}
            }
        }

        // Overlay de la palette de spawn d'entités (Touche P ou Bouton [+ SPAWN])
        if is_edit_mode && self.editor.show_spawn_palette {
            let pal_w = 440;
            let pal_h = 370;
            let pal_x = 160;
            let pal_y = 80;

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
                ("portal", "+ Portail Quantique (Teleporteur)", 0x0006B6D4),
                ("torch", "+ Torche Brulante (Emetteur Flammes)", 0x00F97316),
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
                ("ZQSD / Fleches: Bouger", 0x000369A1, 0x00E0F2FE),
                ("E / Espace: Interagir", 0x00B45309, 0x00FDE68A),
                ("TAB: Studio ECS", 0x006B21A8, 0x00F3E8FF),
                ("H: Guide Interactif", 0x004338CA, 0x00E0E7FF),
                ("N: Radar", 0x00047857, 0x00D1FAE5),
                ("G: Grille O(1)", 0x001E293B, 0x0094A3B8),
                ("M: Audio", 0x001E293B, 0x0094A3B8),
                ("F1: Telemetrie", 0x001E293B, 0x0094A3B8),
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

        // Section 1 : LES 2 MODES DE TRAVAIL
        let mut cy = my + 54;
        canvas.draw_badge(mx + 20, cy, 260, 24, 0x001E293B, 0x0038BDF8, "1. LES DEUX MODES DE TRAVAIL", 0x0038BDF8, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "- [TAB] MODE JEU : Test en direct, controle du joueur, physique et scripts KVL.", 0x00F8FAFC, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- [TAB] MODE STUDIO ECS : Creation de scene, authoring d'objets, cablage visuel KVL.", 0x00F8FAFC, 1);
        cy += 28;

        // Section 2 : ARCHITECTURE ENTITE-COMPOSANTS (ECS)
        canvas.draw_badge(mx + 20, cy, 330, 24, 0x001E293B, 0x0010B981, "2. ARCHITECTURE ENTITE-COMPOSANTS (ECS)", 0x0010B981, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "- ENTITE : Conteneur universel identifie par un ID unique, nom et tag.", 0x00E2E8F0, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- COMPOSANTS : Caracteristiques modulaires attachables a toute entite :", 0x00E2E8F0, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Transform2D (x, y, w, h) | Collider2D (Solide AABB / Trigger)", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* Movement2D (Joueur ou IA) | VisionSensing (Raycast de vue)", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 36, cy, "* ParticleEmitter2D (Flammes/Aura) | KVLScriptComponent (Logique)", 0x0094A3B8, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- SYSTEMES AGNOSTIQUES : Boucles decouplees executant la logique (Zero Alloc per frame).", 0x00E2E8F0, 1);
        cy += 28;

        // Section 3 : LANGAGE VISUEL KVL
        canvas.draw_badge(mx + 20, cy, 310, 24, 0x001E293B, 0x00F59E0B, "3. LE LANGAGE VISUEL KVL EN ACTION", 0x00F59E0B, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "- EVENT : Point d'entree declenche par le moteur (OnOverlap, OnInteract, OnSeen).", 0x00FDE047, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- CONDITION : Evaluation logique orientant vers [Vrai:Out] ou [Faux:Out].", 0x00FDE047, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "- ACTION : Mutation du monde ou joueur (BonusPoints, BoostSpeed, ToggleDoor...).", 0x00FDE047, 1);
        cy += 28;

        // Section 4 : RACCOURCIS CLAVIER DU STUDIO
        canvas.draw_badge(mx + 20, cy, 320, 24, 0x001E293B, 0x00A855F7, "4. RACCOURCIS CLAVIER DU STUDIO", 0x00A855F7, 2);
        cy += 30;
        canvas.draw_text(mx + 24, cy, "P : Spawn Entite | Ctrl+D : Dupliquer | Suppr : Effacer | Ctrl+S : Sauvegarder", 0x00C084FC, 1);
        cy += 18;
        canvas.draw_text(mx + 24, cy, "Espace : Vue Macro / Cablage | N : Radar | G : Grille O(1) | M : Audio | F1 : Profilage", 0x00C084FC, 1);

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
}

