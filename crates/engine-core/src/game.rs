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
            let speed = 40.0 + (i as f32 % 5.0) * 20.0;
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
            if input.is_key_just_pressed(KeyCode::Space) {
                self.editor.toggle_view_detail();
                self.status_message = if self.editor.is_detailed_view {
                    "Inspecteur KVL : Vue détaillée technique dépliée".into()
                } else {
                    "Inspecteur KVL : Vue simplifiée (macro-blocs)".into()
                };
            }

            // Sélection à la souris
            if input.mouse_just_clicked {
                let mx = input.mouse_x;
                let my = input.mouse_y;

                // 1. Bouton toggle vue détaillée
                if mx >= 510.0 && mx <= 770.0 && my >= 280.0 && my <= 305.0 {
                    self.editor.toggle_view_detail();
                    return;
                }

                // 2. Clic sur le Joueur
                let half = self.player_size / 2.0;
                if mx >= self.player_x - half && mx <= self.player_x + half
                    && my >= self.player_y - half && my <= self.player_y + half
                {
                    self.editor.selected = SelectedEntity::Player;
                    self.editor.is_dragging = true;
                    self.editor.drag_offset_x = self.player_x - mx;
                    self.editor.drag_offset_y = self.player_y - my;
                    self.status_message = "Joueur sélectionné. Déplacez-le à la souris.".into();
                    return;
                }

                // 3. Clic sur une Pièce
                let mut found_coin = None;
                for (idx, coin) in self.coins.iter().enumerate() {
                    let dx = mx - coin.x;
                    let dy = my - coin.y;
                    if (dx * dx + dy * dy).sqrt() <= (coin.radius + 8.0) {
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

                // 4. Clic sur une Porte
                let mut found_door = None;
                for (idx, door) in self.doors.iter().enumerate() {
                    if mx >= door.x && mx <= door.x + door.w && my >= door.y && my <= door.y + door.h {
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
                if mx < 500.0 && my >= 55.0 && my <= 435.0 {
                    self.editor.selected = SelectedEntity::None;
                }
            }

            if input.mouse_just_released {
                self.editor.is_dragging = false;
            }

            // Déplacement de l'objet sélectionné à la souris
            if self.editor.is_dragging && input.mouse_pressed {
                let new_x = (input.mouse_x + self.editor.drag_offset_x).clamp(30.0, 480.0);
                let new_y = (input.mouse_y + self.editor.drag_offset_y).clamp(70.0, 410.0);

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

            // En mode édition, le gameplay est suspendu pour éditer tranquillement
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

        self.player_x = self.player_x.clamp(20.0, 780.0);
        self.player_y = self.player_y.clamp(60.0, 420.0);

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

                    spawn_requests.push((coin.x, coin.y, 20, 0x00FDE047));

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

            if dist_door < 55.0 && (input.is_key_just_pressed(KeyCode::KeyE) || input.is_key_just_pressed(KeyCode::Space)) {
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
                spawn_requests.push((door.x + door.w / 2.0, door.y + door.h / 2.0, 15, p_col));

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

    pub fn render(&self, canvas: &mut Canvas) {
        canvas.clear(0x00141923);

        let is_edit_mode = self.editor.mode == EditorMode::Edit;

        // Bandeau supérieur (HUD)
        let header_bg = if is_edit_mode { 0x004C1D95 } else { 0x001E2638 };
        let header_border = if is_edit_mode { 0x00A855F7 } else { 0x003B82F6 };
        canvas.draw_rect(0, 0, canvas.width as u32, 45, header_bg);
        canvas.draw_rect(0, 44, canvas.width as u32, 2, header_border);

        if is_edit_mode {
            canvas.draw_text(15, 12, "KARACTIRAGE [MODE EDITION ACTIF - APPUYEZ SUR TAB POUR JOUER]", 0x00F8FAFC, 2);
        } else {
            let header_title = format!("KARACTIRAGE 2D - {}", self.scene_name.to_uppercase());
            canvas.draw_text(15, 12, &header_title, 0x00FFFFFF, 2);

            let profile_color = self.adaptation.current_profile.color();
            let profile_txt = format!("[{}]", self.adaptation.current_profile.name());
            canvas.draw_text((canvas.width as i32) - 490, 14, &profile_txt, profile_color, 1);

            let score = match self.kvl_context.get_var("score") {
                Some(Value::Int(s)) => *s,
                _ => 0,
            };
            let score_txt = format!("SCORE KVL: {}", score);
            canvas.draw_text((canvas.width as i32) - 180, 12, &score_txt, 0x00FBBF24, 2);
        }

        // Aire de jeu (Cadre)
        let play_area_w = if is_edit_mode { 485 } else { (canvas.width as u32) - 30 };
        canvas.draw_rect_outline(15, 55, play_area_w, 380, if is_edit_mode { 0x006D28D9 } else { 0x002B364F }, 2);

        // Grille d'édition en mode édition
        if is_edit_mode {
            for gx in (25..500).step_by(30) {
                for gy in (65..430).step_by(30) {
                    canvas.set_pixel(gx, gy, 0x00334155);
                }
            }
        }

        // Rendu des portes
        for (idx, door) in self.doors.iter().enumerate() {
            let door_color = if door.is_open { 0x0010B981 } else { 0x00EF4444 };
            canvas.draw_rect(door.x as i32, door.y as i32, door.w as u32, door.h as u32, door_color);

            // Halo de sélection
            if is_edit_mode && self.editor.selected == SelectedEntity::Door(idx) {
                canvas.draw_rect_outline(door.x as i32 - 4, door.y as i32 - 4, door.w as u32 + 8, door.h as u32 + 8, 0x00FBBF24, 2);
            }

            let door_label = if door.is_open { "OUVERTE" } else { "PORTE" };
            canvas.draw_text(door.x as i32 - 10, door.y as i32 - 18, door_label, door_color, 1);
        }

        // Rendu des pièces dorées
        for (idx, coin) in self.coins.iter().enumerate() {
            if !coin.collected || is_edit_mode {
                canvas.draw_circle(coin.x as i32, coin.y as i32, coin.radius as i32, 0x00F59E0B);
                canvas.draw_circle(coin.x as i32, coin.y as i32, (coin.radius * 0.6) as i32, 0x00FDE047);

                // Halo de sélection
                if is_edit_mode && self.editor.selected == SelectedEntity::Coin(idx) {
                    canvas.draw_rect_outline(
                        coin.x as i32 - coin.radius as i32 - 4,
                        coin.y as i32 - coin.radius as i32 - 4,
                        (coin.radius as u32 + 4) * 2,
                        (coin.radius as u32 + 4) * 2,
                        0x00FBBF24,
                        2,
                    );
                }
            }
        }

        // Particules
        for p in &self.particles {
            canvas.draw_rect(p.x as i32, p.y as i32, 2, 2, p.color);
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
            2,
        );
        if is_edit_mode && self.editor.selected == SelectedEntity::Player {
            canvas.draw_rect_outline(
                (self.player_x as i32) - half - 4,
                (self.player_y as i32) - half - 4,
                self.player_size as u32 + 8,
                self.player_size as u32 + 8,
                0x00FBBF24,
                2,
            );
        }
        canvas.draw_text((self.player_x as i32) - 18, (self.player_y as i32) - half - 15, "JOUEUR", 0x0093C5FD, 1);

        // ========================================================
        // PANNEAU INSPECTEUR KVL (EN MODE ÉDITION)
        // ========================================================
        if is_edit_mode {
            let insp_x = 510;
            let insp_y = 55;
            let insp_w = 275;
            let insp_h = 380;

            canvas.draw_rect(insp_x, insp_y, insp_w, insp_h, 0x000F172A);
            canvas.draw_rect_outline(insp_x, insp_y, insp_w, insp_h, 0x0038BDF8, 2);

            canvas.draw_text(insp_x + 12, insp_y + 12, "INSPECTEUR D'OBJET & KVL", 0x0038BDF8, 1);
            canvas.draw_rect(insp_x + 12, insp_y + 26, insp_w - 24, 1, 0x00334155);

            match self.editor.selected {
                SelectedEntity::None => {
                    canvas.draw_text(insp_x + 12, insp_y + 45, "AUCUN OBJET SELECTIONNE.", 0x0094A3B8, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 65, "Cliquez sur une piece,", 0x0064748B, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 80, "la porte ou le joueur pour", 0x0064748B, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 95, "afficher et deplier sa logique.", 0x0064748B, 1);
                }
                SelectedEntity::Player => {
                    canvas.draw_text(insp_x + 12, insp_y + 40, "ENTITE : JOUEUR", 0x0038BDF8, 1);
                    let pos_str = format!("Position : X:{:.0}  Y:{:.0}", self.player_x, self.player_y);
                    canvas.draw_text(insp_x + 12, insp_y + 60, &pos_str, 0x00F8FAFC, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 80, "Vitesse  : 220 px/sec", 0x0094A3B8, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 100, "Controle : Clavier ZQSD / Fleches", 0x0094A3B8, 1);

                    canvas.draw_rect(insp_x + 12, insp_y + 130, insp_w - 24, 1, 0x00334155);
                    canvas.draw_text(insp_x + 12, insp_y + 145, "LOGIQUE KVL ASSOCIEE :", 0x00FBBF24, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 165, "- Emetteur d'evenements", 0x00A7F3D0, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 185, "- Declenche 'OnPlayerOverlap'", 0x00A7F3D0, 1);
                    canvas.draw_text(insp_x + 12, insp_y + 205, "- Declenche 'OnDoorInteract'", 0x00A7F3D0, 1);
                }
                SelectedEntity::Coin(idx) => {
                    let coin = &self.coins[idx];
                    canvas.draw_text(insp_x + 12, insp_y + 35, &format!("ENTITE : PIECE ({})", coin.id), 0x00FBBF24, 1);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", coin.x, coin.y);
                    canvas.draw_text(insp_x + 12, insp_y + 52, &pos_str, 0x00F8FAFC, 1);

                    // Bouton bascule de vue
                    let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001D4ED8 };
                    canvas.draw_rect(insp_x + 12, insp_y + 72, insp_w - 24, 22, view_btn_bg);
                    let view_btn_txt = if self.editor.is_detailed_view { "[ VUE DETAILLEE (ESPACE) ]" } else { "[ VUE SIMPLIFIEE (ESPACE) ]" };
                    canvas.draw_text(insp_x + 20, insp_y + 78, view_btn_txt, 0x00FFFFFF, 1);

                    // Visualisation KVL
                    if !self.editor.is_detailed_view {
                        // VUE SIMPLIFIÉE (Macro-blocs ergonomiques KaractiRage)
                        canvas.draw_text(insp_x + 12, insp_y + 110, "CHAINE LOGIQUE VISUELLE :", 0x0038BDF8, 1);

                        // Bloc 1
                        canvas.draw_rect(insp_x + 12, insp_y + 130, insp_w - 24, 30, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 12, insp_y + 130, insp_w - 24, 30, 0x003B82F6, 1);
                        canvas.draw_text(insp_x + 18, insp_y + 140, "1. Contact avec le joueur", 0x0093C5FD, 1);

                        canvas.draw_text(insp_x + 120, insp_y + 165, "|", 0x00FBBF24, 1);
                        canvas.draw_text(insp_x + 120, insp_y + 175, "V", 0x00FBBF24, 1);

                        // Bloc 2
                        canvas.draw_rect(insp_x + 12, insp_y + 190, insp_w - 24, 30, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 12, insp_y + 190, insp_w - 24, 30, 0x0010B981, 1);
                        canvas.draw_text(insp_x + 18, insp_y + 200, "2. Ajouter 10 au score", 0x006EE7B7, 1);

                        canvas.draw_text(insp_x + 120, insp_y + 225, "|", 0x00FBBF24, 1);
                        canvas.draw_text(insp_x + 120, insp_y + 235, "V", 0x00FBBF24, 1);

                        // Bloc 3
                        canvas.draw_rect(insp_x + 12, insp_y + 250, insp_w - 24, 30, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 12, insp_y + 250, insp_w - 24, 30, 0x00EF4444, 1);
                        canvas.draw_text(insp_x + 18, insp_y + 260, "3. Faire disparaitre", 0x00FCA5A5, 1);

                        canvas.draw_text(insp_x + 12, insp_y + 310, "-> Pressez ESPACE pour deplier", 0x0094A3B8, 1);
                    } else {
                        // VUE DÉTAILLÉE (Nœuds techniques et Pins dépliés)
                        canvas.draw_text(insp_x + 12, insp_y + 105, "NOEUDS & PINS TECHNIQUES :", 0x0010B981, 1);

                        // Noeud Event
                        canvas.draw_text(insp_x + 12, insp_y + 125, "[Event: evt_overlap]", 0x0093C5FD, 1);
                        canvas.draw_text(insp_x + 20, insp_y + 140, "Sortie : [Trigger: out] -->", 0x0064748B, 1);

                        // Noeud Action 1
                        canvas.draw_text(insp_x + 12, insp_y + 165, "[Action: act_add_score]", 0x006EE7B7, 1);
                        canvas.draw_text(insp_x + 20, insp_y + 180, "Entree: [Trigger: in]", 0x0064748B, 1);
                        canvas.draw_text(insp_x + 20, insp_y + 195, "Sortie: [Trigger: out] -->", 0x0064748B, 1);

                        // Noeud Action 2
                        canvas.draw_text(insp_x + 12, insp_y + 220, "[Action: act_destroy]", 0x00FCA5A5, 1);
                        canvas.draw_text(insp_x + 20, insp_y + 235, "Entree: [Trigger: in]", 0x0064748B, 1);

                        canvas.draw_text(insp_x + 12, insp_y + 270, "Connexions : 2 fils valides", 0x00FBBF24, 1);
                        canvas.draw_text(insp_x + 12, insp_y + 310, "-> Pressez ESPACE pour replier", 0x0094A3B8, 1);
                    }
                }
                SelectedEntity::Door(idx) => {
                    let door = &self.doors[idx];
                    canvas.draw_text(insp_x + 12, insp_y + 35, &format!("ENTITE : PORTE ({})", door.id), 0x0010B981, 1);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", door.x, door.y);
                    canvas.draw_text(insp_x + 12, insp_y + 52, &pos_str, 0x00F8FAFC, 1);

                    let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001D4ED8 };
                    canvas.draw_rect(insp_x + 12, insp_y + 72, insp_w - 24, 22, view_btn_bg);
                    let view_btn_txt = if self.editor.is_detailed_view { "[ VUE DETAILLEE (ESPACE) ]" } else { "[ VUE SIMPLIFIEE (ESPACE) ]" };
                    canvas.draw_text(insp_x + 20, insp_y + 78, view_btn_txt, 0x00FFFFFF, 1);

                    if !self.editor.is_detailed_view {
                        canvas.draw_text(insp_x + 12, insp_y + 115, "CHAINE LOGIQUE VISUELLE :", 0x0038BDF8, 1);

                        canvas.draw_rect(insp_x + 12, insp_y + 140, insp_w - 24, 34, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 12, insp_y + 140, insp_w - 24, 34, 0x003B82F6, 1);
                        canvas.draw_text(insp_x + 18, insp_y + 152, "1. Appui sur E (Proximite)", 0x0093C5FD, 1);

                        canvas.draw_text(insp_x + 120, insp_y + 190, "|", 0x00FBBF24, 1);
                        canvas.draw_text(insp_x + 120, insp_y + 200, "V", 0x00FBBF24, 1);

                        canvas.draw_rect(insp_x + 12, insp_y + 220, insp_w - 24, 34, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 12, insp_y + 220, insp_w - 24, 34, 0x0010B981, 1);
                        canvas.draw_text(insp_x + 18, insp_y + 232, "2. Basculer Ouvert/Ferme", 0x006EE7B7, 1);

                        canvas.draw_text(insp_x + 12, insp_y + 310, "-> Pressez ESPACE pour deplier", 0x0094A3B8, 1);
                    } else {
                        canvas.draw_text(insp_x + 12, insp_y + 110, "NOEUDS & PINS TECHNIQUES :", 0x0010B981, 1);

                        canvas.draw_text(insp_x + 12, insp_y + 135, "[Event: evt_door_interact]", 0x0093C5FD, 1);
                        canvas.draw_text(insp_x + 20, insp_y + 155, "Sortie : [Trigger: out] -->", 0x0064748B, 1);

                        canvas.draw_text(insp_x + 12, insp_y + 185, "[Action: act_toggle_door]", 0x006EE7B7, 1);
                        canvas.draw_text(insp_x + 20, insp_y + 205, "Entree: [Trigger: in]", 0x0064748B, 1);

                        canvas.draw_text(insp_x + 12, insp_y + 260, "Connexions : 1 fil valide", 0x00FBBF24, 1);
                        canvas.draw_text(insp_x + 12, insp_y + 310, "-> Pressez ESPACE pour replier", 0x0094A3B8, 1);
                    }
                }
            }
        }

        // Bandeau inférieur : Statut et traçabilité KVL
        let bottom_y = 445;
        canvas.draw_rect(0, bottom_y as i32, canvas.width as u32, 155, 0x000F172A);
        canvas.draw_rect(0, bottom_y as i32, canvas.width as u32, 1, 0x00334155);

        let info_line = if is_edit_mode {
            "MODE EDITION : Clic gauche (Selectionner/Deplacer) | ESPACE (Deplier KVL) | TAB (Reprendre le jeu)"
        } else {
            "MODE JEU : TAB (Editeur/Inspecteur) | F1 (Telemetrie) | F2 (Profil) | F3 (Auto) | R (Reload)"
        };
        canvas.draw_text(15, bottom_y + 8, info_line, 0x0094A3B8, 1);
        canvas.draw_text(15, bottom_y + 24, &self.status_message, 0x0038BDF8, 1);

        canvas.draw_text(15, bottom_y + 45, "TRACES RUNTIME KVL :", 0x0094A3B8, 1);
        let log_count = self.kvl_context.execution_log.len();
        let start_idx = log_count.saturating_sub(4);
        for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
            let trace_line = format!("- [{}] {} : {}", step.node_id, step.node_name, step.message);
            canvas.draw_text(15, bottom_y + 65 + (i as i32 * 18), &trace_line, 0x00A7F3D0, 1);
        }

        // Overlay Télémétrie en Mode Jeu
        if !is_edit_mode && self.telemetry.show_overlay {
            let ov_x = (canvas.width as i32) - 310;
            let ov_y = 65;
            let ov_w = 295;
            let ov_h = 240;

            canvas.draw_rect(ov_x, ov_y, ov_w, ov_h, 0x000A0F1D);
            canvas.draw_rect_outline(ov_x, ov_y, ov_w, ov_h, 0x0038BDF8, 1);

            canvas.draw_text(ov_x + 10, ov_y + 10, "PROFILAGE & TELEMETRIE (F1)", 0x0038BDF8, 1);

            let fps_line = format!("FPS : {:.1} (Cible : {:.0})", self.telemetry.current_fps, self.adaptation.target_fps);
            canvas.draw_text(ov_x + 10, ov_y + 28, &fps_line, 0x00F8FAFC, 1);

            let cpu_line = format!("CPU Frame : {} us (Avg)", self.telemetry.avg_frame_time_us);
            canvas.draw_text(ov_x + 10, ov_y + 44, &cpu_line, 0x0034D399, 1);

            let budget_status = if self.telemetry.avg_frame_time_us <= self.adaptation.target_budget_us {
                "BUDGET : RESPECTE (<16.6ms)"
            } else {
                "BUDGET : DEPASSE !"
            };
            let budget_color = if self.telemetry.avg_frame_time_us <= self.adaptation.target_budget_us { 0x0010B981 } else { 0x00EF4444 };
            canvas.draw_text(ov_x + 10, ov_y + 60, budget_status, budget_color, 1);

            let auto_str = if self.adaptation.auto_adapt { "AUTO-ADAPTATIF (ON)" } else { "MANUEL (OFF)" };
            canvas.draw_text(ov_x + 10, ov_y + 76, auto_str, 0x00FCD34D, 1);

            let particles_line = format!("Particules : {} / {}", self.particles.len(), self.adaptation.current_profile.max_particles());
            canvas.draw_text(ov_x + 10, ov_y + 92, &particles_line, 0x0094A3B8, 1);

            canvas.draw_text(ov_x + 10, ov_y + 115, "TEMPS DE FRAME (HISTORIQUE) :", 0x0094A3B8, 1);
            let graph_y = ov_y + 130;
            let graph_h = 70;
            canvas.draw_rect(ov_x + 10, graph_y, 275, graph_h as u32, 0x00161F33);
            canvas.draw_rect_outline(ov_x + 10, graph_y, 275, graph_h as u32, 0x002B364F, 1);

            let samples = &self.telemetry.frame_times_us;
            for (idx, &time_us) in samples.iter().enumerate() {
                let bar_x = (ov_x + 12) + (idx as i32 * 2);
                if bar_x < (ov_x + 10 + 273) {
                    let bar_h = ((time_us as f32 / 3000.0) * (graph_h as f32 * 0.7)).min(graph_h as f32 - 4.0) as u32;
                    let bar_color = if time_us > 16_666 { 0x00EF4444 } else if time_us > 8_000 { 0x00FBBF24 } else { 0x0010B981 };
                    canvas.draw_rect(bar_x, graph_y + graph_h - (bar_h as i32) - 2, 2, bar_h.max(2), bar_color);
                }
            }
        }
    }
}
