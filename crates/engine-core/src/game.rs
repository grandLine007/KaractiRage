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

                // 1. Bouton toggle vue détaillée (dans le panneau droit)
                if mx >= 720.0 && my >= 130.0 && my <= 175.0 {
                    self.editor.toggle_view_detail();
                    return;
                }

                // 2. Clic sur le Joueur
                let half = self.player_size / 2.0;
                if mx >= self.player_x - half - 5.0 && mx <= self.player_x + half + 5.0
                    && my >= self.player_y - half - 5.0 && my <= self.player_y + half + 5.0
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
                    if (dx * dx + dy * dy).sqrt() <= (coin.radius + 12.0) {
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
                    if mx >= door.x - 8.0 && mx <= door.x + door.w + 8.0 && my >= door.y - 8.0 && my <= door.y + door.h + 8.0 {
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
                if mx < 700.0 && my >= 65.0 && my <= 580.0 {
                    self.editor.selected = SelectedEntity::None;
                }
            }

            if input.mouse_just_released {
                self.editor.is_dragging = false;
            }

            // Déplacement de l'objet sélectionné à la souris
            if self.editor.is_dragging && input.mouse_pressed {
                let new_x = (input.mouse_x + self.editor.drag_offset_x).clamp(40.0, 680.0);
                let new_y = (input.mouse_y + self.editor.drag_offset_y).clamp(80.0, 560.0);

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

    pub fn render(&self, canvas: &mut Canvas) {
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

            canvas.draw_rect(insp_x, insp_y, insp_w as u32, insp_h, 0x000F172A);
            canvas.draw_rect_outline(insp_x, insp_y, insp_w as u32, insp_h, 0x0038BDF8, 2);

            canvas.draw_text(insp_x + 18, insp_y + 18, "INSPECTEUR D'OBJET & KVL", 0x0038BDF8, 2);
            canvas.draw_rect(insp_x + 18, insp_y + 42, (insp_w - 36) as u32, 2, 0x00334155);

            match self.editor.selected {
                SelectedEntity::None => {
                    canvas.draw_text(insp_x + 18, insp_y + 65, "AUCUN OBJET SELECTIONNE.", 0x0094A3B8, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 100, "Cliquez sur une piece,", 0x0064748B, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 125, "la porte ou le joueur pour", 0x0064748B, 2);
                    canvas.draw_text(insp_x + 18, insp_y + 150, "inspecter sa logique KVL.", 0x0064748B, 2);
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
                    canvas.draw_text(insp_x + 18, insp_y + 55, &format!("ENTITE : PIECE ({})", coin.id), 0x00FBBF24, 2);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", coin.x, coin.y);
                    canvas.draw_text(insp_x + 18, insp_y + 80, &pos_str, 0x00F8FAFC, 2);

                    // Bouton bascule de vue
                    let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001D4ED8 };
                    canvas.draw_rect(insp_x + 18, insp_y + 110, (insp_w - 36) as u32, 32, view_btn_bg);
                    let view_btn_txt = if self.editor.is_detailed_view { "[ VUE DETAILLEE (ESPACE) ]" } else { "[ VUE SIMPLIFIEE (ESPACE) ]" };
                    canvas.draw_text(insp_x + 28, insp_y + 118, view_btn_txt, 0x00FFFFFF, 2);

                    if !self.editor.is_detailed_view {
                        canvas.draw_text(insp_x + 18, insp_y + 160, "CHAINE LOGIQUE VISUELLE :", 0x0038BDF8, 2);

                        // Bloc 1
                        canvas.draw_rect(insp_x + 18, insp_y + 190, (insp_w - 36) as u32, 38, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 18, insp_y + 190, (insp_w - 36) as u32, 38, 0x003B82F6, 2);
                        canvas.draw_text(insp_x + 28, insp_y + 202, "1. Contact Joueur", 0x0093C5FD, 2);

                        canvas.draw_text(insp_x + 180, insp_y + 234, "V", 0x00FBBF24, 2);

                        // Bloc 2
                        canvas.draw_rect(insp_x + 18, insp_y + 255, (insp_w - 36) as u32, 38, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 18, insp_y + 255, (insp_w - 36) as u32, 38, 0x0010B981, 2);
                        canvas.draw_text(insp_x + 28, insp_y + 267, "2. Score +10", 0x006EE7B7, 2);

                        canvas.draw_text(insp_x + 180, insp_y + 299, "V", 0x00FBBF24, 2);

                        // Bloc 3
                        canvas.draw_rect(insp_x + 18, insp_y + 320, (insp_w - 36) as u32, 38, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 18, insp_y + 320, (insp_w - 36) as u32, 38, 0x00EF4444, 2);
                        canvas.draw_text(insp_x + 28, insp_y + 332, "3. Disparaitre", 0x00FCA5A5, 2);
                    } else {
                        canvas.draw_text(insp_x + 18, insp_y + 155, "NOEUDS & PINS TECHNIQUES :", 0x0010B981, 2);

                        canvas.draw_text(insp_x + 18, insp_y + 185, "[Event: evt_overlap]", 0x0093C5FD, 2);
                        canvas.draw_text(insp_x + 30, insp_y + 208, "Out : [Trigger: out] -->", 0x0094A3B8, 2);

                        canvas.draw_text(insp_x + 18, insp_y + 240, "[Action: act_add_score]", 0x006EE7B7, 2);
                        canvas.draw_text(insp_x + 30, insp_y + 263, "In  : [Trigger: in]", 0x0094A3B8, 2);
                        canvas.draw_text(insp_x + 30, insp_y + 285, "Out : [Trigger: out] -->", 0x0094A3B8, 2);

                        canvas.draw_text(insp_x + 18, insp_y + 318, "[Action: act_destroy]", 0x00FCA5A5, 2);
                        canvas.draw_text(insp_x + 30, insp_y + 340, "In  : [Trigger: in]", 0x0094A3B8, 2);
                    }
                }
                SelectedEntity::Door(idx) => {
                    let door = &self.doors[idx];
                    canvas.draw_text(insp_x + 18, insp_y + 55, &format!("ENTITE : PORTE ({})", door.id), 0x0010B981, 2);
                    let pos_str = format!("Pos : X:{:.0}  Y:{:.0}", door.x, door.y);
                    canvas.draw_text(insp_x + 18, insp_y + 80, &pos_str, 0x00F8FAFC, 2);

                    let view_btn_bg = if self.editor.is_detailed_view { 0x00047857 } else { 0x001D4ED8 };
                    canvas.draw_rect(insp_x + 18, insp_y + 110, (insp_w - 36) as u32, 32, view_btn_bg);
                    let view_btn_txt = if self.editor.is_detailed_view { "[ VUE DETAILLEE (ESPACE) ]" } else { "[ VUE SIMPLIFIEE (ESPACE) ]" };
                    canvas.draw_text(insp_x + 28, insp_y + 118, view_btn_txt, 0x00FFFFFF, 2);

                    if !self.editor.is_detailed_view {
                        canvas.draw_text(insp_x + 18, insp_y + 160, "CHAINE LOGIQUE VISUELLE :", 0x0038BDF8, 2);

                        canvas.draw_rect(insp_x + 18, insp_y + 195, (insp_w - 36) as u32, 42, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 18, insp_y + 195, (insp_w - 36) as u32, 42, 0x003B82F6, 2);
                        canvas.draw_text(insp_x + 28, insp_y + 208, "1. Touche E (Proximite)", 0x0093C5FD, 2);

                        canvas.draw_text(insp_x + 180, insp_y + 248, "V", 0x00FBBF24, 2);

                        canvas.draw_rect(insp_x + 18, insp_y + 275, (insp_w - 36) as u32, 42, 0x001E293B);
                        canvas.draw_rect_outline(insp_x + 18, insp_y + 275, (insp_w - 36) as u32, 42, 0x0010B981, 2);
                        canvas.draw_text(insp_x + 28, insp_y + 288, "2. Basculer Etat", 0x006EE7B7, 2);
                    } else {
                        canvas.draw_text(insp_x + 18, insp_y + 155, "NOEUDS & PINS TECHNIQUES :", 0x0010B981, 2);

                        canvas.draw_text(insp_x + 18, insp_y + 190, "[Event: evt_door_interact]", 0x0093C5FD, 2);
                        canvas.draw_text(insp_x + 30, insp_y + 215, "Out : [Trigger: out] -->", 0x0094A3B8, 2);

                        canvas.draw_text(insp_x + 18, insp_y + 250, "[Action: act_toggle_door]", 0x006EE7B7, 2);
                        canvas.draw_text(insp_x + 30, insp_y + 275, "In  : [Trigger: in]", 0x0094A3B8, 2);
                    }
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
}
