use crate::adaptation::AdaptationManager;
use crate::editor::{EditorMode, EditorState, SelectedEntity};
use crate::input::InputState;
use crate::renderer::Canvas;
use crate::scene::SceneData;
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

pub struct GameState {
    pub scene_path: String,
    pub scene_name: String,
    pub player_x: f32,
    pub player_y: f32,
    pub player_speed: f32,
    pub player_size: f32,
    pub coins: Vec<CoinInstance>,
    pub doors: Vec<DoorInstance>,
    pub particles: Vec<Particle>,
    pub kvl_graphs: HashMap<String, Graph>,
    pub kvl_context: ExecutionContext,
    pub status_message: String,
    pub telemetry: Telemetry,
    pub adaptation: AdaptationManager,
    pub editor: EditorState,
}

impl GameState {
    pub fn load_from_scene(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let scene_data = SceneData::load_from_file(path)?;

        let mut kvl_graphs = HashMap::new();
        let mut kvl_context = ExecutionContext::new();
        kvl_context.set_var("score", Value::Int(0));

        // Charger les scripts KVL des pièces
        for coin in &scene_data.coins {
            if !kvl_graphs.contains_key(&coin.script_path) {
                if let Ok(content) = fs::read_to_string(&coin.script_path) {
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
                if let Ok(content) = fs::read_to_string(&door.script_path) {
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

        Ok(Self {
            scene_path: path.to_string(),
            scene_name: scene_data.name,
            player_x: scene_data.player.x,
            player_y: scene_data.player.y,
            player_speed: scene_data.player.speed,
            player_size: scene_data.player.size,
            coins,
            doors,
            particles: Vec::new(),
            kvl_graphs,
            kvl_context,
            status_message: "Prêt ! TAB: Mode Éditeur | F1: Télémétrie | R: Reload".into(),
            telemetry: Telemetry::new(),
            adaptation: AdaptationManager::new(),
            editor: EditorState::new(),
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

            // Annuler le câblage avec Échap ou Clic droit
            if input.is_key_just_pressed(KeyCode::Escape) || input.mouse_right_just_clicked {
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
                    _ => None,
                };

                // 1. Clic sur le bouton de bascule [Vue Détaillée / Vue Simplifiée] (haut du panneau droit)
                if mx >= 740.0 && mx <= 1180.0 && my >= 170.0 && my <= 215.0 {
                    self.editor.toggle_view_detail();
                    return;
                }

                // 2. Si vue détaillée active : vérifier clic sur les éléments KVL
                if self.editor.is_detailed_view {
                    if let Some(ref script_path) = selected_script_path {
                        // A. Clic sur le bouton [X] pour couper une liaison
                        if let Some(cut_hit) = self.editor.find_connection_cut_at(mx, my) {
                            let conn_id = cut_hit.connection_id.clone();
                            if let Some(graph) = self.kvl_graphs.get_mut(script_path) {
                                if graph.remove_connection_by_id(&conn_id) {
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
        if input.is_key_down(KeyCode::ArrowDown) || input.is_key_down(KeyCode::KeyS) {
            move_y += 1.0;
        }

        let len: f32 = (move_x * move_x + move_y * move_y).sqrt();
        if len > 0.0 {
            move_x /= len;
            move_y /= len;
        }

        self.player_x += move_x * self.player_speed * dt;
        self.player_y += move_y * self.player_speed * dt;

        self.player_x = self.player_x.clamp(40.0, 1140.0);
        self.player_y = self.player_y.clamp(80.0, 570.0);

        // Particules
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt * 1.5;
        }
        self.particles.retain(|p| p.life > 0.0);

        let mut spawn_requests = Vec::new();

        // Collision pièces
        for coin in &mut self.coins {
            if !coin.collected {
                let dx = self.player_x - coin.x;
                let dy = self.player_y - coin.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < (self.player_size / 2.0 + coin.radius) {
                    coin.collected = true;

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
                    self.status_message = format!("Pièce ({}) collectée ! Score KVL : {}", coin.id, score);
                }
            }
        }

        // Interaction porte
        for door in &mut self.doors {
            let dx_door = self.player_x - (door.x + door.w / 2.0);
            let dy_door = self.player_y - (door.y + door.h / 2.0);
            let dist_door = (dx_door * dx_door + dy_door * dy_door).sqrt();

            if dist_door < 75.0 && (input.is_key_just_pressed(KeyCode::KeyE) || input.is_key_just_pressed(KeyCode::Space)) {
                if let Some(graph) = self.kvl_graphs.get(&door.script_path) {
                    if let Ok(steps) = Interpreter::trigger_event(
                        graph,
                        &mut self.kvl_context,
                        "OnDoorInteract",
                    ) {
                        kvl_steps_this_frame += steps;
                    }
                }

                door.is_open = !door.is_open;
                let p_col = if door.is_open { 0x0010B981 } else { 0x00EF4444 };
                spawn_requests.push((door.x + door.w / 2.0, door.y + door.h / 2.0, 20, p_col));

                self.status_message = if door.is_open {
                    format!("Porte ({}) ouverte !", door.id)
                } else {
                    format!("Porte ({}) fermée.", door.id)
                };
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
        } else {
            let header_title = format!("KARACTIRAGE - {}", self.scene_name.to_uppercase());
            canvas.draw_text(20, 18, &header_title, 0x00FFFFFF, 2);

            let profile_color = self.adaptation.current_profile.color();
            let profile_txt = format!("[{}]", self.adaptation.current_profile.name());
            canvas.draw_text(w - 580, 20, &profile_txt, profile_color, 2);

            let score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };
            let score_txt = format!("SCORE: {}", score);
            canvas.draw_text(w - 220, 18, &score_txt, 0x00FBBF24, 2);
        }

        // 2. Zone de jeu (Dimensions réactives)
        let play_area_w = if is_edit_mode { (w - 490).max(400) as u32 } else { (w - 40) as u32 };
        let play_area_h = (h - 260).max(300) as u32;

        canvas.draw_rect_outline(20, 70, play_area_w, play_area_h, if is_edit_mode { 0x006D28D9 } else { 0x002B364F }, 2);

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
            }
        }

        // 3. Bandeau inférieur : Statut et traçabilité KVL (Hauteur 180px)
        let bottom_y = h - 180;
        canvas.draw_rect(0, bottom_y, w as u32, 180, 0x000F172A);
        canvas.draw_rect(0, bottom_y, w as u32, 2, 0x00334155);

        let info_line = if is_edit_mode {
            "MODE EDITION : Clic gauche (Deplacer) | ESPACE (Deplier KVL) | TAB (Reprendre)"
        } else {
            "MODE JEU : TAB (Editeur KVL) | F1 (Telemetrie) | F2 (Profil) | F3 (Auto) | R (Reload)"
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
            let ov_h = 320;
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

            canvas.draw_text(ov_x + 16, ov_y + 168, "TEMPS DE FRAME (HISTORIQUE) :", 0x0094A3B8, 2);
            let graph_y = ov_y + 192;
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

        // Bouton bascule de vue
        let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001D4ED8 };
        let btn_y = insp_y + 105;
        let btn_w = (insp_w - 36) as u32;
        canvas.draw_rect(insp_x + 18, btn_y, btn_w, 34, view_btn_bg);
        canvas.draw_rect_outline(insp_x + 18, btn_y, btn_w, 34, 0x0038BDF8, 1);
        let view_btn_txt = if self.editor.is_detailed_view {
            "[ VUE DETAILLEE : CABLAGE (ESPACE) ]"
        } else {
            "[ VUE SIMPLIFIEE : MACRO (ESPACE) ]"
        };
        canvas.draw_text(insp_x + 28, btn_y + 10, view_btn_txt, 0x00FFFFFF, 2);

        let graph = match self.kvl_graphs.get(script_path) {
            Some(g) => g,
            None => {
                canvas.draw_text(insp_x + 18, insp_y + 155, "Script KVL introuvable.", 0x00EF4444, 2);
                return;
            }
        };

        if !self.editor.is_detailed_view {
            // ==========================================
            // VUE SIMPLIFIÉE (MACRO-BLOCS)
            // ==========================================
            canvas.draw_text(insp_x + 18, insp_y + 155, "CHAINE LOGIQUE VISUELLE :", 0x0038BDF8, 2);

            let mut sorted_nodes: Vec<&Node> = graph.nodes.values().collect();
            sorted_nodes.sort_by_key(|n| match n.kind {
                NodeKind::Event { .. } => 0,
                NodeKind::Action { ref action_type } if action_type == "AddScore" => 1,
                _ => 2,
            });

            for (idx, node) in sorted_nodes.iter().enumerate() {
                let block_y = insp_y + 185 + (idx as i32 * 60);
                let (box_col, badge) = match node.kind {
                    NodeKind::Event { .. } => (0x000284C7, "1. EVENT :"),
                    _ if idx == 1 => (0x00059669, "2. ACTION :"),
                    _ => (0x009333EA, "3. EFFET :"),
                };

                canvas.draw_rect(insp_x + 18, block_y, btn_w, 40, 0x001E293B);
                canvas.draw_rect_outline(insp_x + 18, block_y, btn_w, 40, box_col, 2);

                let label = format!("{} {}", badge, node.name);
                canvas.draw_text(insp_x + 28, block_y + 12, &label, 0x00F8FAFC, 2);

                if idx + 1 < sorted_nodes.len() {
                    canvas.draw_text(insp_x + (btn_w as i32 / 2) - 4, block_y + 44, "V", 0x00FBBF24, 2);
                }
            }
        } else {
            // ==========================================
            // VUE DÉTAILLÉE : CÂBLAGE VISUEL DE PINS
            // ==========================================
            canvas.draw_text(insp_x + 18, insp_y + 150, "CABLAGE DE NOEUDS & PINS :", 0x0010B981, 2);

            let mut sorted_nodes: Vec<&Node> = graph.nodes.values().collect();
            sorted_nodes.sort_by_key(|n| match n.kind {
                NodeKind::Event { .. } => 0,
                NodeKind::Action { ref action_type } if action_type == "AddScore" => 1,
                _ => 2,
            });

            let mut pin_sockets: HashMap<(String, String), (i32, i32)> = HashMap::new();
            let mut cur_y = insp_y + 175;

            for node in &sorted_nodes {
                let max_pins = node.inputs.len().max(node.outputs.len()).max(1);
                let card_h = 30 + (max_pins as i32 * 32);

                let (header_col, header_tag) = match node.kind {
                    NodeKind::Event { .. } => (0x000284C7, "EVENT"),
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

                    let sock_col = if is_connected { 0x0006B6D4 } else { 0x00FBBF24 };
                    canvas.draw_rect(sock_x, sock_y, 22, 20, sock_col);
                    canvas.draw_rect_outline(sock_x, sock_y, 22, 20, 0x00FFFFFF, 1);
                    canvas.draw_text(sock_x + 6, sock_y + 3, ">", 0x00000000, 2);

                    let pin_lbl = format!("{}:Out", pin.id);
                    let text_w = pin_lbl.len() as i32 * 12;
                    canvas.draw_text(sock_x - text_w - 6, sock_y + 3, &pin_lbl, 0x00FDE047, 2);

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
    }
}
