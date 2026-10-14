use crate::adaptation::AdaptationManager;
use crate::audio::{AudioEngine, SoundType};
use crate::editor::{EditorMode, EditorState, SelectedEntity};
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

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub color: u32,
}

pub struct CoinInstance {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub script_path: String,
    pub collected: bool,
}

pub struct DoorInstance {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub script_path: String,
    pub is_open: bool,
}

pub struct WallInstance {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub struct EnemyInstance {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub patrol_min_y: f32,
    pub patrol_max_y: f32,
    pub speed: f32,
    pub patrol_dir: f32,
    pub vision_range: f32,
    pub is_alerted: bool,
    pub alert_timer: f32,
    pub script_path: String,
}

/// Algorithme de Liang-Barsky ultra-rapide et déterministe pour l'intersection segment de vue / AABB
fn segment_intersects_aabb(x1: f32, y1: f32, x2: f32, y2: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
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

pub struct GameState {
    pub scene_path: String,
    pub scene_name: String,
    pub player_x: f32,
    pub player_y: f32,
    pub player_speed: f32,
    pub player_size: f32,
    pub coins: Vec<CoinInstance>,
    pub doors: Vec<DoorInstance>,
    pub walls: Vec<WallInstance>,
    pub enemies: Vec<EnemyInstance>,
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
    pub fn load_from_scene(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let scene_data = SceneData::load_from_file(path)?;

        let mut kvl_graphs = HashMap::new();
        let mut kvl_context = ExecutionContext::new();
        kvl_context.set_var("score", Value::Int(0));

        let resolve_script_content = |path: &str| -> Option<String> {
            if let Ok(c) = fs::read_to_string(path) {
                Some(c)
            } else if let Ok(c) = fs::read_to_string(format!("../../{}", path)) {
                Some(c)
            } else {
                None
            }
        };

        // Charger les scripts KVL des pièces
        for coin in &scene_data.coins {
            if !kvl_graphs.contains_key(&coin.script_path) {
                if let Some(content) = resolve_script_content(&coin.script_path) {
                    if let Ok(graph) = Graph::from_json(&content) {
                        for (k, v) in &graph.variables {
                            if kvl_context.get_var(k).is_none() {
                                kvl_context.set_var(k.clone(), v.initial_value.clone());
                            }
                        }
                        kvl_graphs.insert(coin.script_path.clone(), graph);
                    }
                }
            }
        }

        // Charger les scripts KVL des portes
        for door in &scene_data.doors {
            if !kvl_graphs.contains_key(&door.script_path) {
                if let Some(content) = resolve_script_content(&door.script_path) {
                    if let Ok(graph) = Graph::from_json(&content) {
                        for (k, v) in &graph.variables {
                            if kvl_context.get_var(k).is_none() {
                                kvl_context.set_var(k.clone(), v.initial_value.clone());
                            }
                        }
                        kvl_graphs.insert(door.script_path.clone(), graph);
                    }
                }
            }
        }

        // Charger les scripts KVL des ennemis
        for enemy in &scene_data.enemies {
            if !kvl_graphs.contains_key(&enemy.script_path) {
                if let Some(content) = resolve_script_content(&enemy.script_path) {
                    if let Ok(graph) = Graph::from_json(&content) {
                        for (k, v) in &graph.variables {
                            if kvl_context.get_var(k).is_none() {
                                kvl_context.set_var(k.clone(), v.initial_value.clone());
                            }
                        }
                        kvl_graphs.insert(enemy.script_path.clone(), graph);
                    }
                }
            }
        }

        let coins = scene_data
            .coins
            .into_iter()
            .map(|c| CoinInstance {
                id: c.id,
                x: c.x,
                y: c.y,
                radius: c.radius,
                script_path: c.script_path,
                collected: false,
            })
            .collect();

        let doors = scene_data
            .doors
            .into_iter()
            .map(|d| DoorInstance {
                id: d.id,
                x: d.x,
                y: d.y,
                w: d.w,
                h: d.h,
                script_path: d.script_path,
                is_open: false,
            })
            .collect();

        let walls = scene_data
            .walls
            .into_iter()
            .map(|w| WallInstance {
                id: w.id,
                x: w.x,
                y: w.y,
                w: w.w,
                h: w.h,
            })
            .collect();

        let enemies = scene_data
            .enemies
            .into_iter()
            .map(|e| EnemyInstance {
                id: e.id,
                x: e.x,
                y: e.y,
                size: e.size,
                patrol_min_y: e.patrol_min_y,
                patrol_max_y: e.patrol_max_y,
                speed: e.speed,
                patrol_dir: 1.0,
                vision_range: e.vision_range,
                is_alerted: false,
                alert_timer: 0.0,
                script_path: e.script_path,
            })
            .collect();

        Ok(Self {
            scene_path: path.to_string(),
            scene_name: scene_data.name,
            player_x: scene_data.player.x,
            player_y: scene_data.player.y,
            player_speed: scene_data.player.speed,
            player_size: scene_data.player.size,
            coins,
            doors,
            walls,
            enemies,
            particles: Vec::new(),
            kvl_graphs,
            kvl_context,
            status_message: "Prêt ! TAB: Mode Éditeur | N: Radar | M: Son | F1: Télémétrie".into(),
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
            self.status_message = "Hot-Reloading réussi depuis les fichiers JSON !".into();
        } else {
            self.status_message = "Erreur de rechargement JSON.".into();
        }
    }

    pub fn save_scene_and_scripts(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Sauvegarder la scène JSON avec les positions actuelles des entités
        let scene_data = SceneData {
            name: self.scene_name.clone(),
            description: "Scène de démonstration data-driven avec entités et liaisons KVL externes.".into(),
            player: crate::scene::PlayerSpawn {
                x: self.player_x,
                y: self.player_y,
                speed: self.player_speed,
                size: self.player_size,
            },
            coins: self.coins.iter().map(|c| crate::scene::CoinEntity {
                id: c.id.clone(),
                x: c.x,
                y: c.y,
                radius: c.radius,
                script_path: c.script_path.clone(),
            }).collect(),
            doors: self.doors.iter().map(|d| crate::scene::DoorEntity {
                id: d.id.clone(),
                x: d.x,
                y: d.y,
                w: d.w,
                h: d.h,
                script_path: d.script_path.clone(),
            }).collect(),
            walls: self.walls.iter().map(|w| crate::scene::WallEntity {
                id: w.id.clone(),
                x: w.x,
                y: w.y,
                w: w.w,
                h: w.h,
            }).collect(),
            enemies: self.enemies.iter().map(|e| crate::scene::EnemyEntity {
                id: e.id.clone(),
                x: e.x,
                y: e.y,
                size: e.size,
                patrol_min_y: e.patrol_min_y,
                patrol_max_y: e.patrol_max_y,
                speed: e.speed,
                vision_range: e.vision_range,
                script_path: e.script_path.clone(),
            }).collect(),
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
        self.status_message = "Scène et scripts KVL sauvegardés sur disque avec succès ! (Ctrl+S)".into();
        Ok(())
    }

    pub fn spawn_particles(&mut self, x: f32, y: f32, count: usize, color: u32) {
        let max_allowed = self.adaptation.current_profile.max_particles();
        let to_spawn = count.min(max_allowed.saturating_sub(self.particles.len()));

        for i in 0..to_spawn {
            let angle = (i as f32) * (std::f32::consts::TAU / (to_spawn as f32));
            let speed = 60.0 + (i as f32 % 5.0) * 30.0;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                life: 1.0,
                color,
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

        // Bascule Mode Jeu / Mode Édition (TAB)
        if input.is_key_just_pressed(KeyCode::Tab) {
            self.editor.toggle_mode();
            self.status_message = match self.editor.mode {
                EditorMode::Play => "Mode Jeu actif ! (ZQSD/Flèches pour jouer | TAB pour éditer)".into(),
                EditorMode::Edit => "Mode Édition actif ! Cliquez sur un objet pour inspecter ses blocs KVL.".into(),
            };
            return;
        }

        // ==========================================
        // GESTION DU MODE ÉDITION
        // ==========================================
        if self.editor.mode == EditorMode::Edit {
            self.editor.mouse_x = input.mouse_x;
            self.editor.mouse_y = input.mouse_y;

            // Annuler le câblage ou la palette avec Échap ou Clic droit
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
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

                // Récupérer le script_path de l'entité sélectionnée si applicable
                let selected_script_path = match self.editor.selected {
                    SelectedEntity::Coin(idx) if idx < self.coins.len() => Some(self.coins[idx].script_path.clone()),
                    SelectedEntity::Door(idx) if idx < self.doors.len() => Some(self.doors[idx].script_path.clone()),
                    SelectedEntity::Enemy(idx) if idx < self.enemies.len() => Some(self.enemies[idx].script_path.clone()),
                    _ => None,
                };

                // 0. Clic sur le bouton Sauvegarder
                if self.editor.is_save_btn_clicked(mx, my) {
                    if let Err(e) = self.save_scene_and_scripts() {
                        self.status_message = format!("Erreur lors de la sauvegarde : {}", e);
                    }
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
                        self.status_message = "Veuillez d'abord sélectionner une entité (pièce ou porte).".into();
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
                        // Clic hors palette -> la fermer
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

                // 4. Si vue détaillée active : vérifier clic sur les éléments KVL
                if self.editor.is_detailed_view {
                    if let Some(ref script_path) = selected_script_path {
                        // A. Clic sur le bouton [X] pour couper une liaison
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

                        // B. Clic sur une broche (Pin)
                        if let Some(pin_hit) = self.editor.find_pin_at(mx, my) {
                            let node_id = pin_hit.node_id.clone();
                            let pin_id = pin_hit.pin_id.clone();
                            let is_output = pin_hit.is_output;
                            let sock_x = pin_hit.socket_x;
                            let sock_y = pin_hit.socket_y;

                            if let Some(wire) = self.editor.active_wire.take() {
                                if !is_output {
                                    // Relier la sortie active vers cette entrée
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
                                    // Changer la broche source de câblage
                                    self.editor.start_wire(node_id.clone(), pin_id.clone(), sock_x, sock_y);
                                    self.status_message = format!("Nouvelle source de câblage : {}:{}", node_id, pin_id);
                                }
                            } else {
                                if is_output {
                                    self.editor.start_wire(node_id.clone(), pin_id.clone(), sock_x, sock_y);
                                    self.status_message = format!("Fil de câblage actif depuis {}:{}. Cliquez sur une entrée.", node_id, pin_id);
                                } else {
                                    // Clic sur une entrée existante : si connectée, la déconnecter !
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

                    // Si on a un fil actif et qu'on a cliqué ailleurs dans l'espace vide : annuler le fil
                    if self.editor.active_wire.is_some() {
                        self.editor.cancel_wire();
                        self.status_message = "Câblage annulé.".into();
                        return;
                    }
                }

                // 3. Clic sur le Joueur
                let half = self.player_size / 2.0;
                if mx >= self.player_x - half - 8.0 && mx <= self.player_x + half + 8.0
                    && my >= self.player_y - half - 8.0 && my <= self.player_y + half + 8.0
                {
                    self.editor.selected = SelectedEntity::Player;
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.player_x - mx;
                    self.editor.drag_offset_y = self.player_y - my;
                    self.status_message = "Joueur sélectionné. Déplacez-le à la souris.".into();
                    return;
                }

                // 4. Clic sur une Pièce
                let mut found_coin = None;
                for (idx, coin) in self.coins.iter().enumerate() {
                    let dx = mx - coin.x;
                    let dy = my - coin.y;
                    if (dx * dx + dy * dy).sqrt() <= (coin.radius + 16.0) {
                        found_coin = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = found_coin {
                    self.editor.selected = SelectedEntity::Coin(idx);
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.coins[idx].x - mx;
                    self.editor.drag_offset_y = self.coins[idx].y - my;
                    self.status_message = format!("Pièce '{}' sélectionnée. Blocs KVL affichés à droite.", self.coins[idx].id);
                    return;
                }

                // 5. Clic sur une Porte
                let mut found_door = None;
                for (idx, door) in self.doors.iter().enumerate() {
                    if mx >= door.x - 12.0 && mx <= door.x + door.w + 12.0 && my >= door.y - 12.0 && my <= door.y + door.h + 12.0 {
                        found_door = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = found_door {
                    self.editor.selected = SelectedEntity::Door(idx);
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.doors[idx].x - mx;
                    self.editor.drag_offset_y = self.doors[idx].y - my;
                    self.status_message = format!("Porte '{}' sélectionnée. Blocs KVL affichés à droite.", self.doors[idx].id);
                    return;
                }

                // 6. Clic sur un Mur
                let mut found_wall = None;
                for (idx, wall) in self.walls.iter().enumerate() {
                    if mx >= wall.x - 8.0 && mx <= wall.x + wall.w + 8.0 && my >= wall.y - 8.0 && my <= wall.y + wall.h + 8.0 {
                        found_wall = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = found_wall {
                    self.editor.selected = SelectedEntity::Wall(idx);
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.walls[idx].x - mx;
                    self.editor.drag_offset_y = self.walls[idx].y - my;
                    self.status_message = format!("Mur solide '{}' sélectionné. Déplacez-le à la souris.", self.walls[idx].id);
                    return;
                }

                // 7. Clic sur un Ennemi
                let mut found_enemy = None;
                for (idx, enemy) in self.enemies.iter().enumerate() {
                    let half_e = enemy.size / 2.0;
                    if mx >= enemy.x - half_e - 10.0 && mx <= enemy.x + half_e + 10.0
                        && my >= enemy.y - half_e - 10.0 && my <= enemy.y + half_e + 10.0
                    {
                        found_enemy = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = found_enemy {
                    self.editor.selected = SelectedEntity::Enemy(idx);
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.enemies[idx].x - mx;
                    self.editor.drag_offset_y = self.enemies[idx].y - my;
                    self.status_message = format!("Garde '{}' sélectionné. Blocs KVL d'IA affichés à droite.", self.enemies[idx].id);
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

            // Déplacement de l'objet sélectionné à la souris
            if self.editor.is_dragging && input.mouse_pressed {
                let new_x = (input.mouse_x + self.editor.drag_offset_x).clamp(50.0, 700.0);
                let new_y = (input.mouse_y + self.editor.drag_offset_y).clamp(80.0, 580.0);

                match self.editor.selected {
                    SelectedEntity::Player => {
                        self.player_x = new_x;
                        self.player_y = new_y;
                    }
                    SelectedEntity::Coin(idx) => {
                        if idx < self.coins.len() {
                            self.coins[idx].x = new_x;
                            self.coins[idx].y = new_y;
                        }
                    }
                    SelectedEntity::Door(idx) => {
                        if idx < self.doors.len() {
                            self.doors[idx].x = new_x;
                            self.doors[idx].y = new_y;
                        }
                    }
                    SelectedEntity::Wall(idx) => {
                        if idx < self.walls.len() {
                            self.walls[idx].x = new_x;
                            self.walls[idx].y = new_y;
                        }
                    }
                    SelectedEntity::Enemy(idx) => {
                        if idx < self.enemies.len() {
                            self.enemies[idx].x = new_x;
                            self.enemies[idx].y = new_y;
                        }
                    }
                    SelectedEntity::None => {}
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

        // Déplacement joueur
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

        // Vitesse effective incluant les modificateurs KVL (BoostSpeed)
        let speed_boost = match self.kvl_context.get_var("speed_boost") {
            Some(Value::Int(b)) => *b as f32,
            _ => 0.0,
        };
        let effective_speed = self.player_speed + speed_boost;

        // ==========================================
        // PARTITIONNEMENT SPATIAL (ZERO-ALLOCATION)
        // ==========================================
        self.spatial_grid.clear();
        for (idx, wall) in self.walls.iter().enumerate() {
            self.spatial_grid.insert(SpatialItem {
                id: idx,
                kind: SpatialKind::Solid,
                x: wall.x,
                y: wall.y,
                w: wall.w,
                h: wall.h,
            });
        }
        for (idx, door) in self.doors.iter().enumerate() {
            if !door.is_open {
                self.spatial_grid.insert(SpatialItem {
                    id: idx,
                    kind: SpatialKind::Door(idx),
                    x: door.x,
                    y: door.y,
                    w: door.w,
                    h: door.h,
                });
            }
        }
        for (idx, coin) in self.coins.iter().enumerate() {
            if !coin.collected {
                self.spatial_grid.insert(SpatialItem {
                    id: idx,
                    kind: SpatialKind::Coin(idx),
                    x: coin.x - coin.radius,
                    y: coin.y - coin.radius,
                    w: coin.radius * 2.0,
                    h: coin.radius * 2.0,
                });
            }
        }
        for (idx, enemy) in self.enemies.iter().enumerate() {
            self.spatial_grid.insert(SpatialItem {
                id: idx,
                kind: SpatialKind::Enemy(idx),
                x: enemy.x - enemy.size / 2.0,
                y: enemy.y - enemy.size / 2.0,
                w: enemy.size,
                h: enemy.size,
            });
        }

        // ==========================================
        // PHYSIQUE 2D & GLISSEMENT AABB ACCÉLÉRÉ
        // ==========================================
        let half = self.player_size / 2.0;

        // 1. Déplacement et résolution sur l'axe X via requêtes spatiales O(1)
        if move_x != 0.0 {
            self.player_x += move_x * effective_speed * dt;
            self.player_x = self.player_x.clamp(30.0 + half, 1160.0 - half);

            let p_min_x = self.player_x - half;
            let p_min_y = self.player_y - half;
            let candidates = self.spatial_grid.query_aabb(p_min_x, p_min_y, self.player_size, self.player_size);
            for item in candidates {
                if matches!(item.kind, SpatialKind::Solid | SpatialKind::Door(_)) {
                    if move_x > 0.0 {
                        self.player_x = item.x - half;
                    } else if move_x < 0.0 {
                        self.player_x = item.x + item.w + half;
                    }
                }
            }
        }

        // 2. Déplacement et résolution sur l'axe Y via requêtes spatiales O(1)
        if move_y != 0.0 {
            self.player_y += move_y * effective_speed * dt;
            self.player_y = self.player_y.clamp(75.0 + half, 595.0 - half);

            let p_min_x = self.player_x - half;
            let p_min_y = self.player_y - half;
            let candidates = self.spatial_grid.query_aabb(p_min_x, p_min_y, self.player_size, self.player_size);
            for item in candidates {
                if matches!(item.kind, SpatialKind::Solid | SpatialKind::Door(_)) {
                    if move_y > 0.0 {
                        self.player_y = item.y - half;
                    } else if move_y < 0.0 {
                        self.player_y = item.y + item.h + half;
                    }
                }
            }
        }

        // Particules
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt * 1.5;
        }
        self.particles.retain(|p| p.life > 0.0);

        let mut spawn_requests = Vec::new();

        // Collision et collecte des pièces accélérées par la Grille Spatiale
        let mut collected_indices = Vec::new();
        {
            let candidates = self.spatial_grid.query_aabb(
                self.player_x - half - 2.0,
                self.player_y - half - 2.0,
                self.player_size + 4.0,
                self.player_size + 4.0,
            );
            for item in candidates {
                if let SpatialKind::Coin(idx) = item.kind {
                    if idx < self.coins.len() && !self.coins[idx].collected {
                        let coin = &self.coins[idx];
                        let dx = self.player_x - coin.x;
                        let dy = self.player_y - coin.y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist < (half + coin.radius) {
                            collected_indices.push(idx);
                        }
                    }
                }
            }
        }

        for idx in collected_indices {
            let coin = &mut self.coins[idx];
            coin.collected = true;

            let prev_score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };
            let prev_boost = match self.kvl_context.get_var("speed_boost") {
                Some(Value::Int(b)) => *b,
                _ => 0,
            };

            if let Some(graph) = self.kvl_graphs.get(&coin.script_path) {
                if let Ok(steps) = Interpreter::trigger_event(
                    graph,
                    &mut self.kvl_context,
                    "OnPlayerOverlap",
                ) {
                    kvl_steps_this_frame += steps;
                }
            }

            spawn_requests.push((coin.x, coin.y, 25, 0x00FDE047));

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

            self.status_message = format!("Pièce ({}) collectée ! Score KVL : {}", coin.id, score);
        }

        // Interaction porte
        for door in &mut self.doors {
            let dx_door = self.player_x - (door.x + door.w / 2.0);
            let dy_door = self.player_y - (door.y + door.h / 2.0);
            let dist_door = (dx_door * dx_door + dy_door * dy_door).sqrt();

            if dist_door < 75.0 && (input.is_key_just_pressed(KeyCode::KeyE) || input.is_key_just_pressed(KeyCode::Space)) {
                // Réinitialiser le flag de refus avant exécution
                self.kvl_context.set_var("access_denied", Value::Bool(false));

                if let Some(graph) = self.kvl_graphs.get(&door.script_path) {
                    if let Ok(steps) = Interpreter::trigger_event(
                        graph,
                        &mut self.kvl_context,
                        "OnDoorInteract",
                    ) {
                        kvl_steps_this_frame += steps;
                    }
                }

                let was_denied = match self.kvl_context.get_var("access_denied") {
                    Some(Value::Bool(d)) => *d,
                    _ => false,
                };

                if was_denied {
                    // Refus d'accès : son d'erreur, particules rouges et message d'avertissement
                    self.audio.play(SoundType::AccessDenied);
                    spawn_requests.push((door.x + door.w / 2.0, door.y + door.h / 2.0, 25, 0x00EF4444));
                    let current_score = match self.kvl_context.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };
                    self.status_message = format!("Accès Refusé (Porte {}) ! Score insuffisant : {} / 50 requis.", door.id, current_score);
                } else {
                    let new_open = match self.kvl_context.get_var("door_is_open") {
                        Some(Value::Bool(b)) => *b,
                        _ => !door.is_open,
                    };
                    door.is_open = new_open;
                    if door.is_open {
                        self.audio.play(SoundType::DoorOpen);
                    } else {
                        self.audio.play(SoundType::DoorClose);
                    }
                    let p_col = if door.is_open { 0x0010B981 } else { 0x00EF4444 };
                    spawn_requests.push((door.x + door.w / 2.0, door.y + door.h / 2.0, 20, p_col));

                    self.status_message = if door.is_open {
                        format!("Porte ({}) déverrouillée et ouverte !", door.id)
                    } else {
                        format!("Porte ({}) fermée.", door.id)
                    };
                }
            }
        }

        // Traitement des étincelles déclenchées par KVL (SpawnSparkles)
        if let Some(Value::Int(sparkles)) = self.kvl_context.get_var("sparkle_burst") {
            if *sparkles > 0 {
                self.audio.play(SoundType::Sparkles);
                spawn_requests.push((self.player_x, self.player_y, (*sparkles as usize) * 35, 0x00FDE047));
                self.kvl_context.set_var("sparkle_burst", Value::Int(0));
            }
        }

        // ==========================================
        // PATROUILLE ENNEMIE & IA KVL (RAYCASTING)
        // ==========================================
        for enemy in &mut self.enemies {
            // 1. Déplacement de patrouille ping-pong sur l'axe Y
            enemy.y += enemy.speed * enemy.patrol_dir * dt;
            if enemy.y >= enemy.patrol_max_y {
                enemy.y = enemy.patrol_max_y;
                enemy.patrol_dir = -1.0;
            } else if enemy.y <= enemy.patrol_min_y {
                enemy.y = enemy.patrol_min_y;
                enemy.patrol_dir = 1.0;
            }

            // 2. Détection par cône de vision et Raycasting Line of Sight
            let dx = self.player_x - enemy.x;
            let dy = self.player_y - enemy.y;
            let dist_sq = dx * dx + dy * dy;
            let range_sq = enemy.vision_range * enemy.vision_range;

            // Le garde surveille vers la gauche (approche du joueur)
            let in_cone = dist_sq < range_sq && dx < 25.0 && dy.abs() < (enemy.vision_range * 0.70);

            if in_cone {
                // Raycast accéléré par requête segment sur la Grille Spatiale
                let mut sight_blocked = false;
                let candidates = self.spatial_grid.query_segment(enemy.x, enemy.y, self.player_x, self.player_y);
                for obs in candidates {
                    if matches!(obs.kind, SpatialKind::Solid | SpatialKind::Door(_)) {
                        if segment_intersects_aabb(enemy.x, enemy.y, self.player_x, self.player_y, obs.x, obs.y, obs.w, obs.h) {
                            sight_blocked = true;
                            break;
                        }
                    }
                }

                if !sight_blocked {
                    if !enemy.is_alerted {
                        enemy.is_alerted = true;
                        self.audio.play(SoundType::Alarm);
                        spawn_requests.push((enemy.x, enemy.y - 25.0, 30, 0x00EF4444));
                        self.status_message = format!("ALERTE ROUGE ! Le garde '{}' vous a repéré !", enemy.id);

                        if let Some(graph) = self.kvl_graphs.get(&enemy.script_path) {
                            if let Ok(steps) = Interpreter::trigger_event(
                                graph,
                                &mut self.kvl_context,
                                "OnPlayerSeen",
                            ) {
                                kvl_steps_this_frame += steps;
                            }
                        }
                    }
                    enemy.alert_timer = 2.5;
                }
            }

            // Gestion de l'extinction de l'alerte
            if enemy.is_alerted {
                enemy.alert_timer -= dt;
                if enemy.alert_timer <= 0.0 {
                    enemy.is_alerted = false;
                    self.kvl_context.set_var("alarm_active", Value::Bool(false));
                    self.status_message = format!("Le garde '{}' a perdu votre trace. Alerte levée.", enemy.id);
                }
            }
        }

        // Si l'action LockAllDoors a été déclenchée par KVL, verrouiller les portes
        if let Some(Value::Bool(open)) = self.kvl_context.get_var("door_is_open") {
            for door in &mut self.doors {
                door.is_open = *open;
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
        canvas.clear(0x00141923);

        let is_edit_mode = self.editor.mode == EditorMode::Edit;
        let w = canvas.width as i32;
        let h = canvas.height as i32;

        // 1. Bandeau supérieur (HUD) : Hauteur 60px
        let header_bg = if is_edit_mode { 0x004C1D95 } else { 0x001E2638 };
        let header_border = if is_edit_mode { 0x00A855F7 } else { 0x003B82F6 };
        canvas.draw_rect(0, 0, w as u32, 60, header_bg);
        canvas.draw_rect(0, 59, w as u32, 2, header_border);

        if is_edit_mode {
            canvas.draw_text(20, 18, "KARACTIRAGE [MODE EDITION] - TAB: JOUER | CLIC: DEPLACER", 0x00F8FAFC, 2);
            let (audio_txt, audio_col) = if !self.audio.is_available {
                ("[AUDIO: N/D]", 0x0094A3B8)
            } else if self.audio.is_muted() {
                ("[AUDIO: MUET]", 0x00EF4444)
            } else {
                ("[AUDIO: ACTIF]", 0x0010B981)
            };
            canvas.draw_text(w - 220, 18, audio_txt, audio_col, 2);
        } else {
            let header_title = format!("KARACTIRAGE - {}", self.scene_name.to_uppercase());
            canvas.draw_text(20, 18, &header_title, 0x00FFFFFF, 2);

            let profile_color = self.adaptation.current_profile.color();
            let profile_txt = format!("[{}]", self.adaptation.current_profile.name());
            canvas.draw_text(w - 580, 20, &profile_txt, profile_color, 2);

            let (audio_txt, audio_col) = if !self.audio.is_available {
                ("[AUDIO: N/D]", 0x0094A3B8)
            } else if self.audio.is_muted() {
                ("[AUDIO: MUET (M)]", 0x00EF4444)
            } else {
                ("[AUDIO: ACTIF (M)]", 0x0010B981)
            };
            canvas.draw_text(w - 410, 20, audio_txt, audio_col, 2);

            let score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };
            let score_txt = format!("SCORE: {}", score);
            canvas.draw_text(w - 200, 18, &score_txt, 0x00FBBF24, 2);
        }

        // 2. Zone de jeu (Dimensions réactives)
        let play_area_w = if is_edit_mode { (w - 490).max(400) as u32 } else { (w - 40) as u32 };
        let play_area_h = (h - 260).max(300) as u32;

        canvas.draw_rect_outline(20, 70, play_area_w, play_area_h, if is_edit_mode { 0x006D28D9 } else { 0x002B364F }, 2);

        // Visualisation de débogage de la Grille Spatiale (Touche G)
        if self.spatial_grid.debug_visible {
            let max_x = 20 + play_area_w as i32;
            let max_y = 70 + play_area_h as i32;
            let cs = self.spatial_grid.cell_size as i32;
            let mut gx = 20;
            while gx <= max_x {
                canvas.draw_line(gx, 70, gx, max_y, 0x001B263B);
                gx += cs;
            }
            let mut gy = 70;
            while gy <= max_y {
                canvas.draw_line(20, gy, max_x, gy, 0x001B263B);
                gy += cs;
            }
            canvas.draw_text(30, 75, "GRILLE SPATIALE O(1) : ACTIVE (64px)", 0x0038BDF8, 1);
        }

        // Grille de repérage en mode édition
        if is_edit_mode {
            let max_x = 20 + play_area_w as i32;
            let max_y = 70 + play_area_h as i32;
            for gx in (30..max_x).step_by(40) {
                for gy in (80..max_y).step_by(40) {
                    canvas.draw_rect(gx, gy, 2, 2, 0x00334155);
                }
            }
        }

        // Rendu des murs solides (Physique AABB)
        for (idx, wall) in self.walls.iter().enumerate() {
            let wx = wall.x as i32;
            let wy = wall.y as i32;
            let ww = wall.w as u32;
            let wh = wall.h as u32;

            canvas.draw_rect(wx, wy, ww, wh, 0x001E293B);
            canvas.draw_rect_outline(wx, wy, ww, wh, 0x00475569, 2);
            if ww > 8 && wh > 8 {
                canvas.draw_rect_outline(wx + 3, wy + 3, ww - 6, wh - 6, 0x00334155, 1);
            }

            if is_edit_mode && self.editor.selected == SelectedEntity::Wall(idx) {
                canvas.draw_rect_outline(wx - 4, wy - 4, ww + 8, wh + 8, 0x00FBBF24, 2);
                canvas.draw_text(wx, wy - 20, &format!("MUR: {}", wall.id), 0x00FBBF24, 2);
            }
        }

        // Rendu des portes
        for (idx, door) in self.doors.iter().enumerate() {
            let door_color = if door.is_open { 0x0010B981 } else { 0x00EF4444 };
            canvas.draw_rect(door.x as i32, door.y as i32, door.w as u32, door.h as u32, door_color);

            if is_edit_mode && self.editor.selected == SelectedEntity::Door(idx) {
                canvas.draw_rect_outline(door.x as i32 - 6, door.y as i32 - 6, door.w as u32 + 12, door.h as u32 + 12, 0x00FBBF24, 3);
            }

            let door_label = if door.is_open { "OUVERTE" } else { "PORTE" };
            canvas.draw_text(door.x as i32 - 12, door.y as i32 - 24, door_label, door_color, 2);
        }

        // Rendu des pièces dorées
        for (idx, coin) in self.coins.iter().enumerate() {
            if !coin.collected || is_edit_mode {
                canvas.draw_circle(coin.x as i32, coin.y as i32, coin.radius as i32, 0x00F59E0B);
                canvas.draw_circle(coin.x as i32, coin.y as i32, (coin.radius * 0.6) as i32, 0x00FDE047);

                if is_edit_mode && self.editor.selected == SelectedEntity::Coin(idx) {
                    canvas.draw_rect_outline(
                        coin.x as i32 - coin.radius as i32 - 6,
                        coin.y as i32 - coin.radius as i32 - 6,
                        (coin.radius as u32 + 6) * 2,
                        (coin.radius as u32 + 6) * 2,
                        0x00FBBF24,
                        3,
                    );
                }
            }
        }

        // Particules
        for p in &self.particles {
            canvas.draw_rect(p.x as i32, p.y as i32, 3, 3, p.color);
        }

        // Rendu du joueur
        let half = (self.player_size / 2.0) as i32;
        canvas.draw_rect(
            (self.player_x as i32) - half,
            (self.player_y as i32) - half,
            self.player_size as u32,
            self.player_size as u32,
            0x003B82F6,
        );
        canvas.draw_rect_outline(
            (self.player_x as i32) - half,
            (self.player_y as i32) - half,
            self.player_size as u32,
            self.player_size as u32,
            0x0093C5FD,
            3,
        );
        if is_edit_mode && self.editor.selected == SelectedEntity::Player {
            canvas.draw_rect_outline(
                (self.player_x as i32) - half - 6,
                (self.player_y as i32) - half - 6,
                self.player_size as u32 + 12,
                self.player_size as u32 + 12,
                0x00FBBF24,
                3,
            );
        }
        canvas.draw_text((self.player_x as i32) - 26, (self.player_y as i32) - half - 24, "JOUEUR", 0x0093C5FD, 2);

        // Rendu des ennemis et de leurs cônes de vision
        for (idx, enemy) in self.enemies.iter().enumerate() {
            let ex = enemy.x as i32;
            let ey = enemy.y as i32;
            let es = enemy.size as i32;
            let half_e = es / 2;

            // 1. Cône de vision en éventail vers la gauche
            let cone_len = enemy.vision_range;
            let cone_col = if enemy.is_alerted { 0x00EF4444 } else { 0x0038BDF8 };
            let steps = 7;
            for s in 0..=steps {
                let factor = (s as f32 / steps as f32) - 0.5;
                let target_x = ex - cone_len as i32;
                let target_y = ey + (factor * cone_len * 1.3) as i32;
                canvas.draw_line(ex - half_e, ey, target_x, target_y, cone_col);
            }

            // 2. Corps de la sentinelle (violet tech si calme, rouge néon si alerté)
            let body_col = if enemy.is_alerted { 0x00DC2626 } else { 0x007C3AED };
            canvas.draw_rect(ex - half_e, ey - half_e, es as u32, es as u32, body_col);
            canvas.draw_rect_outline(ex - half_e, ey - half_e, es as u32, es as u32, 0x00F8FAFC, 2);

            // Visière robotique
            let eye_col = if enemy.is_alerted { 0x00FBBF24 } else { 0x0006B6D4 };
            canvas.draw_rect(ex - half_e + 4, ey - 5, 8, 10, eye_col);

            // Badge d'alerte [!] au-dessus du garde
            if enemy.is_alerted {
                canvas.draw_text(ex - 12, ey - half_e - 26, "(!)", 0x00EF4444, 2);
            }

            // Sélection en mode édition
            if is_edit_mode && self.editor.selected == SelectedEntity::Enemy(idx) {
                canvas.draw_rect_outline(ex - half_e - 6, ey - half_e - 6, (es + 12) as u32, (es + 12) as u32, 0x00FBBF24, 3);
                canvas.draw_line(ex, enemy.patrol_min_y as i32, ex, enemy.patrol_max_y as i32, 0x00FBBF24);
                canvas.draw_text(ex + 12, ey - 20, &format!("GARDE: {}", enemy.id), 0x00FBBF24, 2);
            }
        }

        // 4. Rendu du Radar Tactique 2D (Minimap)
        if self.show_minimap {
            let mm_w = 170;
            let mm_h = 100;
            let mm_x = if is_edit_mode { w - 460 - mm_w - 20 } else { w - mm_w - 30 };
            let mm_y = 75;

            // Fond radar bleu nuit tech avec bordure néon cyan
            canvas.draw_rect(mm_x, mm_y, mm_w as u32, mm_h as u32, 0x0008101E);
            canvas.draw_rect_outline(mm_x, mm_y, mm_w as u32, mm_h as u32, 0x000284C7, 2);

            // Bandeau de titre radar
            canvas.draw_rect(mm_x, mm_y, mm_w as u32, 16, 0x000369A1);
            canvas.draw_text(mm_x + 6, mm_y + 3, "RADAR TACTIQUE (N)", 0x00E0F2FE, 1);

            // Grille radar interne
            canvas.draw_line(mm_x, mm_y + 55, mm_x + mm_w, mm_y + 55, 0x000E2A47);
            canvas.draw_line(mm_x + (mm_w / 2), mm_y + 16, mm_x + (mm_w / 2), mm_y + mm_h, 0x000E2A47);

            // Échelle de conversion : aire de jeu 1130 x 520
            let scale_x = (mm_w - 14) as f32 / 1130.0;
            let scale_y = (mm_h - 28) as f32 / 520.0;

            let map_to_radar = |rx: f32, ry: f32| -> (i32, i32) {
                let px = mm_x + 7 + ((rx - 30.0) * scale_x) as i32;
                let py = mm_y + 22 + ((ry - 75.0) * scale_y) as i32;
                (px, py)
            };

            // Murs sur le radar
            for wall in &self.walls {
                let (wx, wy) = map_to_radar(wall.x, wall.y);
                let ww = ((wall.w * scale_x) as u32).max(2);
                let wh = ((wall.h * scale_y) as u32).max(2);
                canvas.draw_rect(wx, wy, ww, wh, 0x00475569);
            }

            // Portes sur le radar
            for door in &self.doors {
                let (dx, dy) = map_to_radar(door.x, door.y);
                let col = if door.is_open { 0x0010B981 } else { 0x00EF4444 };
                canvas.draw_rect(dx, dy, 4, 8, col);
            }

            // Pièces dorées
            for coin in &self.coins {
                if !coin.collected {
                    let (cx, cy) = map_to_radar(coin.x, coin.y);
                    canvas.draw_rect(cx - 1, cy - 1, 3, 3, 0x00FDE047);
                }
            }

            // Ennemis
            for enemy in &self.enemies {
                let (ex, ey) = map_to_radar(enemy.x, enemy.y);
                let ecol = if enemy.is_alerted { 0x00EF4444 } else { 0x00A855F7 };
                canvas.draw_rect(ex - 2, ey - 2, 5, 5, ecol);
                // Viseur de regard radar
                canvas.draw_line(ex, ey, ex - 10, ey, ecol);
            }

            // Joueur (cyan avec halo blanc)
            let (px, py) = map_to_radar(self.player_x, self.player_y);
            canvas.draw_rect(px - 2, py - 2, 5, 5, 0x0038BDF8);
            canvas.draw_rect_outline(px - 4, py - 4, 9, 9, 0x00FFFFFF, 1);
        }

        // ========================================================
        // PANNEAU INSPECTEUR KVL (EN MODE ÉDITION) : Largeur 450px
        // ========================================================
        if is_edit_mode {
            let insp_x = w - 460;
            let insp_y = 70;
            let insp_w = 440;
            let insp_h = play_area_h;

            self.editor.pin_hitboxes.clear();
            self.editor.connection_hitboxes.clear();
            self.editor.palette_hitboxes.clear();
            self.editor.save_btn_hitbox = None;
            self.editor.add_node_btn_hitbox = None;
            self.editor.view_btn_hitbox = None;

            canvas.draw_rect(insp_x, insp_y, insp_w as u32, insp_h, 0x000F172A);
            canvas.draw_rect_outline(insp_x, insp_y, insp_w as u32, insp_h, 0x0038BDF8, 2);

            canvas.draw_text(insp_x + 18, insp_y + 18, "INSPECTEUR D'OBJET & KVL", 0x0038BDF8, 2);
            canvas.draw_rect(insp_x + 18, insp_y + 42, (insp_w - 36) as u32, 2, 0x00334155);

            match self.editor.selected {
                SelectedEntity::None => {
                    canvas.draw_text(insp_x + 18, insp_y + 65, "AUCUN OBJET SELECTIONNE.", 0x0094A3B8, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 100, "Cliquez sur une piece,", 0x0064748B, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 125, "la porte ou le joueur pour", 0x0064748B, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 150, "inspecter ou relier sa logique.", 0x0064748B, 2);
                }
                SelectedEntity::Player => {
                    canvas.draw_text(insp_x + 18, insp_y + 60, "ENTITE : JOUEUR", 0x0038BDF8, 2);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", self.player_x, self.player_y);
                    canvas.draw_text(insp_x + 18, insp_y + 90, &pos_str, 0x00F8FAFC, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 115, "Vitesse : 300 px/sec", 0x0094A3B8, 2);

                    canvas.draw_rect(insp_x + 18, insp_y + 145, (insp_w - 36) as u32, 1, 0x00334155);
                    canvas.draw_text(insp_x + 18, insp_y + 165, "LOGIQUE KVL ASSOCIEE :", 0x00FBBF24, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 195, "- Emetteur d'evenements", 0x00A7F3D0, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 225, "- Declenche 'OnPlayerOverlap'", 0x00A7F3D0, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 255, "- Declenche 'OnDoorInteract'", 0x00A7F3D0, 2);
                }
                SelectedEntity::Coin(idx) => {
                    let coin = &self.coins[idx];
                    let script_path = coin.script_path.clone();
                    let ent_title = format!("ENTITE : PIECE ({})", coin.id);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", coin.x, coin.y);

                    self.render_entity_inspector(canvas, insp_x, insp_y, insp_w, &ent_title, 0x00FBBF24, &pos_str, &script_path);
                }
                SelectedEntity::Door(idx) => {
                    let door = &self.doors[idx];
                    let script_path = door.script_path.clone();
                    let ent_title = format!("ENTITE : PORTE ({})", door.id);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", door.x, door.y);

                    self.render_entity_inspector(canvas, insp_x, insp_y, insp_w, &ent_title, 0x0010B981, &pos_str, &script_path);
                }
                SelectedEntity::Wall(idx) => {
                    let wall = &self.walls[idx];
                    canvas.draw_text(insp_x + 18, insp_y + 60, &format!("ENTITE : MUR SOLIDE ({})", wall.id), 0x0094A3B8, 2);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", wall.x, wall.y);
                    canvas.draw_text(insp_x + 18, insp_y + 90, &pos_str, 0x00F8FAFC, 2);
                    let dim_str = format!("Taille : {} x {} px", wall.w as u32, wall.h as u32);
                    canvas.draw_text(insp_x + 18, insp_y + 115, &dim_str, 0x00CBD5E1, 2);

                    canvas.draw_rect(insp_x + 18, insp_y + 145, (insp_w - 36) as u32, 1, 0x00334155);
                    canvas.draw_text(insp_x + 18, insp_y + 165, "PROPRIETES PHYSIQUES :", 0x00FBBF24, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 195, "- Collision AABB Solide", 0x0034D399, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 225, "- Glissement continu X/Y", 0x0034D399, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 255, "- Bloque passage joueur", 0x0034D399, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 295, "(Deplacement libre souris)", 0x0094A3B8, 2);
                }
                SelectedEntity::Enemy(idx) => {
                    let enemy = &self.enemies[idx];
                    let script_path = enemy.script_path.clone();
                    let ent_title = format!("ENTITE : GARDE ({})", enemy.id);
                    let pos_str = format!("Pos: X:{:.0} Y:{:.0} (Patrouille Y: {:.0}-{:.0})", enemy.x, enemy.y, enemy.patrol_min_y, enemy.patrol_max_y);

                    self.render_entity_inspector(canvas, insp_x, insp_y, insp_w, &ent_title, 0x00DC2626, &pos_str, &script_path);
                }
            }
        }

        // 3. Bandeau inférieur : Statut et traçabilité KVL (Hauteur 180px)
        let bottom_y = h - 180;
        canvas.draw_rect(0, bottom_y, w as u32, 180, 0x000F172A);
        canvas.draw_rect(0, bottom_y, w as u32, 2, 0x00334155);

        let info_line = if is_edit_mode {
            "MODE EDITION : Clic (Deplacer) | M (Son) | ESPACE (Deplier KVL) | TAB (Reprendre)"
        } else {
            "MODE JEU : TAB (Editeur KVL) | N (Radar) | G (Grille) | M (Son) | F1 (Telemetrie) | F2 (Profil) | F3 (Auto) | R (Reload)"
        };
        canvas.draw_text(20, bottom_y + 14, info_line, 0x0094A3B8, 2);
        canvas.draw_text(20, bottom_y + 40, &self.status_message, 0x0038BDF8, 2);

        canvas.draw_text(20, bottom_y + 72, "TRACES RUNTIME KVL :", 0x0094A3B8, 2);
        let log_count = self.kvl_context.execution_log.len();
        let start_idx = log_count.saturating_sub(3);
        for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
            let trace_line = format!("- [{}] {} : {}", step.node_id, step.node_name, step.message);
            canvas.draw_text(20, bottom_y + 102 + (i as i32 * 26), &trace_line, 0x00A7F3D0, 2);
        }

        // Overlay Télémétrie en Mode Jeu (F1)
        if !is_edit_mode && self.telemetry.show_overlay {
            let ov_w = 400;
            let ov_h = 346;
            let ov_x = w - ov_w - 20;
            let ov_y = 70;

            canvas.draw_rect(ov_x, ov_y, ov_w as u32, ov_h as u32, 0x000A0F1D);
            canvas.draw_rect_outline(ov_x, ov_y, ov_w as u32, ov_h as u32, 0x0038BDF8, 2);

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
            for (idx, &time_us) in samples.iter().enumerate() {
                let bar_x = (ov_x + 18) + (idx as i32 * 3);
                if bar_x < (ov_x + ov_w - 18) {
                    let bar_h = ((time_us as f32 / 3000.0) * (graph_h as f32 * 0.7)).min(graph_h as f32 - 4.0) as u32;
                    let bar_color = if time_us > 16_666 { 0x00EF4444 } else if time_us > 8_000 { 0x00FBBF24 } else { 0x0010B981 };
                    canvas.draw_rect(bar_x, graph_y + graph_h - (bar_h as i32) - 2, 2, bar_h.max(2), bar_color);
                }
            }
        }
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

        // Boutons de contrôle supérieur de l'inspecteur
        let btn_w = (insp_w - 36) as u32;
        let btn_half_w = (btn_w - 8) / 2;

        // Ligne 1 : Bouton Sauvegarder (Ctrl+S) + Bouton [+ Nouveau Nœud]
        let row1_y = insp_y + 105;
        let save_x = insp_x + 18;
        let add_x = save_x + btn_half_w as i32 + 8;

        // Bouton Sauvegarder
        canvas.draw_rect(save_x, row1_y, btn_half_w, 32, 0x00059669);
        canvas.draw_rect_outline(save_x, row1_y, btn_half_w, 32, 0x0010B981, 1);
        canvas.draw_text(save_x + 10, row1_y + 8, "[ SAUVER (CTRL+S) ]", 0x00FFFFFF, 2);
        self.editor.save_btn_hitbox = Some((save_x, row1_y, btn_half_w, 32));

        // Bouton + Nouveau Nœud
        canvas.draw_rect(add_x, row1_y, btn_half_w, 32, 0x004F46E5);
        canvas.draw_rect_outline(add_x, row1_y, btn_half_w, 32, 0x00818CF8, 1);
        canvas.draw_text(add_x + 12, row1_y + 8, "[ + NOUVEAU NOEUD ]", 0x00FFFFFF, 2);
        self.editor.add_node_btn_hitbox = Some((add_x, row1_y, btn_half_w, 32));

        // Ligne 2 : Bouton bascule de vue
        let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001D4ED8 };
        let view_btn_y = insp_y + 144;
        canvas.draw_rect(insp_x + 18, view_btn_y, btn_w, 32, view_btn_bg);
        canvas.draw_rect_outline(insp_x + 18, view_btn_y, btn_w, 32, 0x0038BDF8, 1);
        let view_btn_txt = if self.editor.is_detailed_view {
            "[ VUE DETAILLEE : CABLAGE (ESPACE) ]"
        } else {
            "[ VUE SIMPLIFIEE : MACRO (ESPACE) ]"
        };
        canvas.draw_text(insp_x + 28, view_btn_y + 8, view_btn_txt, 0x00FFFFFF, 2);
        self.editor.view_btn_hitbox = Some((insp_x + 18, view_btn_y, btn_w, 32));

        let graph = match self.kvl_graphs.get(script_path) {
            Some(g) => g,
            None => {
                canvas.draw_text(insp_x + 18, insp_y + 190, "Script KVL introuvable.", 0x00EF4444, 2);
                return;
            }
        };

        if !self.editor.is_detailed_view {
            // ==========================================
            // VUE SIMPLIFIÉE (MACRO-BLOCS)
            // ==========================================
            canvas.draw_text(insp_x + 18, insp_y + 186, "CHAINE LOGIQUE VISUELLE :", 0x0038BDF8, 2);

            let mut sorted_nodes: Vec<&Node> = graph.nodes.values().collect();
            sorted_nodes.sort_by_key(|n| match n.kind {
                NodeKind::Event { .. } => 0,
                NodeKind::Action { ref action_type } if action_type == "AddScore" => 1,
                _ => 2,
            });

            for (idx, node) in sorted_nodes.iter().enumerate() {
                let block_y = insp_y + 215 + (idx as i32 * 56);
                let (box_col, badge) = match node.kind {
                    NodeKind::Event { .. } => (0x000284C7, "1. EVENT :"),
                    NodeKind::Condition { .. } => (0x00D97706, "? CONDITION :"),
                    NodeKind::Action { ref action_type } if action_type == "BonusPoints" => (0x00F59E0B, "BONUS :"),
                    NodeKind::Action { ref action_type } if action_type == "BoostSpeed" => (0x0006B6D4, "SPEED :"),
                    NodeKind::Action { ref action_type } if action_type == "SpawnSparkles" => (0x00EC4899, "FX :"),
                    NodeKind::Action { ref action_type } if action_type == "DenyAccess" => (0x00DC2626, "! REFUS :"),
                    _ if idx == 1 => (0x00059669, "ACTION :"),
                    _ => (0x009333EA, "EFFET :"),
                };

                canvas.draw_rect(insp_x + 18, block_y, btn_w, 38, 0x001E293B);
                canvas.draw_rect_outline(insp_x + 18, block_y, btn_w, 38, box_col, 2);

                let label = format!("{} {}", badge, node.name);
                canvas.draw_text(insp_x + 28, block_y + 10, &label, 0x00F8FAFC, 2);

                if idx + 1 < sorted_nodes.len() {
                    canvas.draw_text(insp_x + (btn_w as i32 / 2) - 4, block_y + 40, "V", 0x00FBBF24, 2);
                }
            }
        } else {
            // ==========================================
            // VUE DÉTAILLÉE : CÂBLAGE VISUEL DE PINS
            // ==========================================
            canvas.draw_text(insp_x + 18, insp_y + 186, "CABLAGE DE NOEUDS & PINS :", 0x0010B981, 2);

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

                // Fond de la boîte de nœud
                canvas.draw_rect(insp_x + 18, cur_y, btn_w, card_h as u32, 0x00131C2E);
                canvas.draw_rect_outline(insp_x + 18, cur_y, btn_w, card_h as u32, header_col, 2);

                // En-tête du nœud
                canvas.draw_rect(insp_x + 18, cur_y, btn_w, 24, header_col);
                let header_title = format!("[{}] {}", header_tag, node.id);
                canvas.draw_text(insp_x + 26, cur_y + 5, &header_title, 0x00FFFFFF, 2);

                // Inputs (à gauche)
                for (p_idx, pin) in node.inputs.iter().enumerate() {
                    let py = cur_y + 32 + (p_idx as i32 * 30);
                    let sock_x = insp_x + 26;
                    let sock_y = py;
                    let is_connected = graph.is_pin_connected(&node.id, &pin.id);

                    // Couleur du connecteur
                    let sock_col = if is_connected { 0x0010B981 } else { 0x00475569 };
                    canvas.draw_rect(sock_x, sock_y, 22, 20, sock_col);
                    canvas.draw_rect_outline(sock_x, sock_y, 22, 20, 0x00FFFFFF, 1);
                    canvas.draw_text(sock_x + 6, sock_y + 3, ">", 0x00FFFFFF, 2);

                    let pin_lbl = format!("{}:In", pin.id);
                    canvas.draw_text(sock_x + 28, sock_y + 3, &pin_lbl, 0x00CBD5E1, 2);

                    // Enregistrer hitbox & position
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

            // ==========================================
            // TRACÉ DES CONNEXIONS ET BOUTON DE COUPURE [X]
            // ==========================================
            for conn in &graph.connections {
                let from_key = (conn.from_node.clone(), conn.from_pin.clone());
                let to_key = (conn.to_node.clone(), conn.to_pin.clone());

                if let (Some(&(fx, fy)), Some(&(tx, ty))) = (pin_sockets.get(&from_key), pin_sockets.get(&to_key)) {
                    // Fil reliant la sortie à l'entrée
                    canvas.draw_thick_line(fx, fy, tx, ty, 3, 0x0010B981);

                    // Bouton de coupure [X] au milieu du fil
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

            // ==========================================
            // FIL EN COURS DE CÂBLAGE (DRAG & HOVER)
            // ==========================================
            if let Some(ref wire) = self.editor.active_wire {
                let mx = self.editor.mouse_x as i32;
                let my = self.editor.mouse_y as i32;

                canvas.draw_thick_line(wire.start_x, wire.start_y, mx, my, 3, 0x00F59E0B);
                canvas.draw_circle(mx, my, 6, 0x00FBBF24);
                canvas.draw_text(mx + 12, my - 10, "RELIER A L'ENTREE", 0x00FDE047, 1);
            }
        }

        // ==========================================
        // PALETTE OVERLAY : AJOUTER UN NOUVEAU NŒUD KVL
        // ==========================================
        if self.editor.show_node_palette {
            let pal_x = insp_x + 12;
            let pal_y = insp_y + 80;
            let pal_w = btn_w + 12;
            let pal_h = 410;

            // Fond sombre avec bordure violette néon
            canvas.draw_rect(pal_x, pal_y, pal_w, pal_h, 0x000B0F19);
            canvas.draw_rect_outline(pal_x, pal_y, pal_w, pal_h, 0x00A855F7, 3);

            // Barre de titre
            canvas.draw_rect(pal_x, pal_y, pal_w, 32, 0x006B21A8);
            canvas.draw_text(pal_x + 12, pal_y + 8, "PALETTE KVL : CHOISIR UN NOEUD", 0x00FFFFFF, 2);

            let templates = [
                ("BonusPoints", "+ Bonus Majeur (+50 pts)", 0x00F59E0B),
                ("BoostSpeed", "+ Boost Vitesse (+120 px/s)", 0x0006B6D4),
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

    fn get_test_scene_path() -> &'static str {
        if std::path::Path::new("assets/scenes/level_01.scene.json").exists() {
            "assets/scenes/level_01.scene.json"
        } else {
            "../../assets/scenes/level_01.scene.json"
        }
    }

    #[test]
    fn test_aabb_wall_blocking_and_sliding() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let wall_x = state.walls[0].x;
        let wall_y = state.walls[0].y;
        let wall_h = state.walls[0].h;
        let half = state.player_size / 2.0;

        // Position initiale : 5 px avant le mur sur X, centré sur Y
        state.player_x = wall_x - half - 5.0;
        state.player_y = wall_y + (wall_h / 2.0);

        let initial_y = state.player_y;

        // Simuler une entrée de déplacement diagonale : Droite + Bas
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyD); // Droite
        input.on_key_down(KeyCode::KeyS); // Bas

        // Mettre à jour avec un dt de 0.1s
        state.update(0.1, &input);

        // 1. Le joueur doit avoir glissé vers le bas (Y a augmenté)
        assert!(state.player_y > initial_y, "Le joueur doit pouvoir glisser librement sur l'axe Y");

        // 2. Le joueur ne doit PAS avoir pénétré le mur sur X (bloqué au bord exact du mur)
        assert!(
            (state.player_x - (wall_x - half)).abs() < 0.01,
            "Le joueur doit être bloqué exactement au bord du mur (X: {}, attendu: {})",
            state.player_x,
            wall_x - half
        );
    }

    #[test]
    fn test_door_solidity_when_closed_vs_open() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let half = state.player_size / 2.0;
        let door_x = state.doors[0].x;
        let door_h = state.doors[0].h;
        assert!(!state.doors[0].is_open, "La porte doit démarrer fermée");

        // Positionner le joueur à gauche de la porte fermée
        state.player_x = door_x - half - 5.0;
        state.player_y = state.doors[0].y + (door_h / 2.0);

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyD); // Tenter de traverser vers la droite

        state.update(0.1, &input);

        // La porte fermée doit bloquer le joueur
        assert!(
            (state.player_x - (door_x - half)).abs() < 0.01,
            "La porte fermée doit bloquer le joueur comme un mur solide"
        );

        // Ouvrir la porte
        state.doors[0].is_open = true;

        // Réessayer d'avancer vers la droite
        state.update(0.1, &input);

        // Le joueur doit maintenant pouvoir traverser la porte ouverte
        assert!(
            state.player_x > door_x - half,
            "La porte ouverte doit laisser passer le joueur"
        );
    }

    #[test]
    fn test_palette_node_creation_and_execution() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        state.editor.mode = EditorMode::Edit;
        state.editor.selected = SelectedEntity::Coin(0);
        let script_path = state.coins[0].script_path.clone();

        // 1. Simuler l'ouverture de la palette
        state.editor.toggle_palette();
        assert!(state.editor.show_node_palette);

        // 2. Instancier le nœud BonusPoints (+50 pts) dans le graphe
        let graph = state.kvl_graphs.get_mut(&script_path).expect("Graphe de la pièce présent");
        let initial_node_count = graph.nodes.len();

        let new_id = format!("bonus_pts_{}", initial_node_count + 1);
        let bonus_node = Node::new(
            &new_id,
            "Bonus Majeur (+50)",
            Category::Progression,
            "Accorde +50 points",
            NodeKind::Action { action_type: "BonusPoints".into() },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        graph.add_node(bonus_node);
        assert_eq!(graph.nodes.len(), initial_node_count + 1);

        // 3. Relier la sortie de l'action existante vers l'entrée du nouveau nœud
        // Ex: act_add_score:out_trigger -> bonus_pts_x:in_trigger
        let conn_res = graph.connect_pins("act_add_score", "out_trigger", &new_id, "in_trigger");
        assert!(conn_res.is_ok(), "Connexion valide entre act_add_score et bonus_node");

        // 4. Exécuter l'événement OnPlayerOverlap et vérifier l'effet cumulé (score initial 0 + 1 normal + 50 bonus = 51)
        state.kvl_context.set_var("score", Value::Int(0));
        let steps = Interpreter::trigger_event(graph, &mut state.kvl_context, "OnPlayerOverlap")
            .expect("Exécution KVL");

        assert!(steps >= 3, "Au moins 3 étapes exécutées");
        assert_eq!(
            state.kvl_context.get_var("score"),
            Some(&Value::Int(51)),
            "Le score doit refléter l'effet du nouveau nœud Bonus (+50)"
        );
    }

    #[test]
    fn test_save_scene_persistence() {
        let temp_dir = std::env::temp_dir().join("karactirage_test_save");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("Création dossier temp");

        let test_scene_content = fs::read_to_string(get_test_scene_path()).expect("Lecture scène");
        let temp_scene_path = temp_dir.join("test.scene.json");
        fs::write(&temp_scene_path, &test_scene_content).expect("Écriture scène temp");

        let mut state = GameState::load_from_scene(temp_scene_path.to_str().unwrap())
            .expect("Chargement scène temp");

        // Modifier les coordonnées du joueur et d'une pièce
        state.player_x = 422.0;
        state.player_y = 311.0;
        state.coins[0].x = 555.0;

        // Sauvegarder sur disque
        state.save_scene_and_scripts().expect("Sauvegarde de la scène");

        // Recharger depuis le fichier sauvegardé
        let reloaded = GameState::load_from_scene(temp_scene_path.to_str().unwrap())
            .expect("Rechargement de la scène sauvegardée");

        assert_eq!(reloaded.player_x, 422.0, "Position X du joueur persistée");
        assert_eq!(reloaded.player_y, 311.0, "Position Y du joueur persistée");
        assert_eq!(reloaded.coins[0].x, 555.0, "Position X de la pièce persistée");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_door_condition_blocks_and_unlocks_with_score() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        let door_x = state.doors[0].x;
        let door_y = state.doors[0].y;
        let door_h = state.doors[0].h;

        // Positionner le joueur à portée d'interaction de la porte
        state.player_x = door_x - 30.0;
        state.player_y = door_y + (door_h / 2.0);

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyE);

        // 1. Score initial = 0 (< 50) -> La porte doit refuser l'accès et rester fermée
        state.update(0.016, &input);
        assert!(!state.doors[0].is_open, "La porte doit rester fermée quand le score < 50");
        assert!(state.status_message.contains("Accès Refusé"), "Un message d'accès refusé doit être émis");

        // 2. Augmenter le score à 50+
        state.kvl_context.set_var("score", Value::Int(50));

        // Interagir à nouveau avec la touche E
        input.on_key_up(KeyCode::KeyE);
        input.on_key_down(KeyCode::KeyE);
        state.update(0.016, &input);

        assert!(state.doors[0].is_open, "La porte doit s'ouvrir une fois le score >= 50 atteint");
        assert!(state.status_message.contains("déverrouillée et ouverte"), "Le message d'ouverture doit être émis");
    }

    #[test]
    fn test_audio_mute_toggle_and_gameplay_sound_triggers() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(!state.audio.is_muted(), "L'audio doit être actif au démarrage");

        // 1. Bascule Mute via KeyCode::KeyM
        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyM);
        state.update(0.016, &input);

        assert!(state.audio.is_muted(), "La touche M doit couper le son (Mute)");
        assert!(state.status_message.contains("MUET"));

        // Rétablir le son
        input.on_key_up(KeyCode::KeyM);
        input.on_key_down(KeyCode::KeyM);
        state.update(0.016, &input);

        assert!(!state.audio.is_muted(), "Un second appui sur M doit réactiver le son");
        assert!(state.status_message.contains("ACTIF"));

        // 2. Émission d'effets sonores sans panique
        state.audio.play(SoundType::Coin);
        state.audio.play(SoundType::DoorOpen);
        state.audio.play(SoundType::DoorClose);
        state.audio.play(SoundType::AccessDenied);
        state.audio.play(SoundType::BonusFanfare);
        state.audio.play(SoundType::SpeedBoost);
        state.audio.play(SoundType::Sparkles);
        state.audio.play(SoundType::UiClick);
        state.audio.play(SoundType::WireCut);
        state.audio.play(SoundType::Alarm);
        state.audio.play(SoundType::RadarPing);
    }

    #[test]
    fn test_enemy_patrol_and_vision_raycast_alert() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(!state.enemies.is_empty(), "Au moins un garde patrouilleur doit exister");
        let initial_y = state.enemies[0].y;

        let input = InputState::new();

        // 1. Positionner le joueur loin hors de portée de vision
        state.player_x = 100.0;
        state.player_y = 100.0;

        state.update(0.1, &input);

        // Le garde doit s'être déplacé (patrouille active)
        assert_ne!(state.enemies[0].y, initial_y, "Le garde doit patrouiller sur l'axe Y");
        assert!(!state.enemies[0].is_alerted, "Le garde ne doit pas être alerté si le joueur est hors de portée");

        // 2. Positionner le joueur directement dans le cône de vision dégagé du garde
        // Le garde est en X ~760, regarde vers la gauche (X décroissant)
        state.player_x = state.enemies[0].x - 80.0;
        state.player_y = state.enemies[0].y;

        state.update(0.016, &input);

        // Le garde doit repérer le joueur et déclencher l'alarme
        assert!(state.enemies[0].is_alerted, "Le garde doit repérer le joueur dans son cône de vision");
        assert!(state.status_message.contains("ALERTE"), "Un message d'alerte doit être émis");

        // Vérifier l'effet KVL de l'alerte
        let alarm = state.kvl_context.get_var("alarm_active");
        assert_eq!(alarm, Some(&Value::Bool(true)), "L'alarme KVL doit être active");

        // 3. Test d'occultation par un mur solide
        // Positionner un mur entre le joueur et le garde
        state.enemies[0].is_alerted = false;
        state.enemies[0].alert_timer = 0.0;
        state.walls.push(WallInstance {
            id: "test_shield_wall".into(),
            x: state.enemies[0].x - 40.0,
            y: state.enemies[0].y - 50.0,
            w: 20.0,
            h: 100.0,
        });

        state.update(0.016, &input);

        // Le mur doit occulter la ligne de mire (Raycast bloqué)
        assert!(!state.enemies[0].is_alerted, "Le mur solide doit masquer le joueur (occultation Raycast)");
    }

    #[test]
    fn test_minimap_radar_toggle() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        assert!(state.show_minimap, "Le radar tactique doit être affiché par défaut");

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyN);
        state.update(0.016, &input);

        assert!(!state.show_minimap, "La touche N doit masquer le radar tactique");

        input.on_key_up(KeyCode::KeyN);
        input.on_key_down(KeyCode::KeyN);
        state.update(0.016, &input);

        assert!(state.show_minimap, "Un second appui sur N doit réactiver le radar");
    }

    #[test]
    fn test_spatial_hash_grid_collision_and_queries() {
        let mut state = GameState::load_from_scene(get_test_scene_path())
            .expect("Chargement de la scène de test");

        // 1. Vérification de la bascule de débogage spatial (Touche G)
        assert!(!state.spatial_grid.debug_visible, "La grille spatiale démarre masquée");

        let mut input = InputState::new();
        input.on_key_down(KeyCode::KeyG);
        state.update(0.016, &input);

        assert!(state.spatial_grid.debug_visible, "La touche G doit activer la visualisation de la grille spatiale");
        assert!(state.status_message.contains("AFFICHÉE"));

        input.on_key_up(KeyCode::KeyG);
        input.on_key_down(KeyCode::KeyG);
        state.update(0.016, &input);
        assert!(!state.spatial_grid.debug_visible, "Un second appui sur G doit masquer la grille");

        // 2. Vérification que les requêtes spatiales et candidats sont enregistrés dans les stats
        input.on_key_up(KeyCode::KeyG);
        input.on_key_down(KeyCode::KeyD); // Déplacer vers la droite pour déclencher les requêtes
        state.update(0.016, &input);

        assert!(state.spatial_grid.stats_queries > 0, "Des requêtes broadphase doivent être effectuées");

        // 3. Collecte de pièce via la grille spatiale
        assert!(!state.coins.is_empty());
        let coin_x = state.coins[0].x;
        let coin_y = state.coins[0].y;
        state.player_x = coin_x;
        state.player_y = coin_y;

        let empty_input = InputState::new();
        state.update(0.016, &empty_input);

        assert!(state.coins[0].collected, "La pièce doit être détectée et collectée via le partitionnement spatial");
        let score = match state.kvl_context.get_var("score") {
            Some(Value::Int(s)) => *s,
            _ => 0,
        };
        assert!(score > 0, "Le score KVL doit être incrémenté lors de la collecte");
    }
}
