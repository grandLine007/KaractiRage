use crate::adaptation::AdaptationManager;
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
            status_message: "Scène JSON chargée. F1: Télémétrie | F2: Profil Qualité | F3: Auto-Adapt | R: Reload".into(),
            telemetry: Telemetry::new(),
            adaptation: AdaptationManager::new(),
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

        // Contrôles de diagnostic
        if input.is_key_just_pressed(KeyCode::F1) {
            self.telemetry.toggle_overlay();
        }
        if input.is_key_just_pressed(KeyCode::F2) {
            self.adaptation.cycle_profile_manual();
            self.status_message = format!("Profil forcé manuellement : {}", self.adaptation.current_profile.name());
        }
        if input.is_key_just_pressed(KeyCode::F3) {
            self.adaptation.toggle_auto_mode();
            self.status_message = if self.adaptation.auto_adapt {
                "Adaptation dynamique automatique : ACTIVEE".into()
            } else {
                "Adaptation dynamique automatique : DESACTIVEE (Manuel)".into()
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

        // Mise à jour des particules
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt * 1.5;
        }
        self.particles.retain(|p| p.life > 0.0);

        let mut spawn_requests = Vec::new();

        // 1. Détection des collisions avec les pièces -> Déclencheur KVL
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

                    // Enregistrement des particules à émettre
                    spawn_requests.push((coin.x, coin.y, 20, 0x00FDE047));

                    let score = match self.kvl_context.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };
                    self.status_message = format!("Pièce ({}) collectée ! Score KVL : {}", coin.id, score);
                }
            }
        }

        // 2. Interaction avec la porte (touche E ou Espace)
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
                    format!("Porte ({}) déverrouillée et ouverte !", door.id)
                } else {
                    format!("Porte ({}) fermée.", door.id)
                };
            }
        }

        // Émission des particules après la fin des emprunts sur coins et doors
        for (px, py, count, col) in spawn_requests {
            self.spawn_particles(px, py, count, col);
        }

        // Mesure de télémétrie de la frame
        let frame_duration_us = frame_start.elapsed().as_micros() as u64;
        self.telemetry.record_frame(frame_duration_us, kvl_steps_this_frame);

        // Boucle d'adaptation matérielle dynamique par hystérésis
        if let Some(msg) = self.adaptation.evaluate_adaptation(frame_duration_us) {
            self.status_message = msg.into();
        }
    }

    pub fn render(&self, canvas: &mut Canvas) {
        canvas.clear(0x00141923);

        // Bandeau supérieur (HUD)
        canvas.draw_rect(0, 0, canvas.width as u32, 45, 0x001E2638);
        canvas.draw_rect(0, 44, canvas.width as u32, 2, 0x003B82F6);

        let score = match self.kvl_context.get_var("score") {
            Some(Value::Int(s)) => *s,
            _ => 0,
        };

        let header_title = format!("KARACTIRAGE 2D - {}", self.scene_name.to_uppercase());
        canvas.draw_text(15, 12, &header_title, 0x00FFFFFF, 2);

        // Profil Badge
        let profile_color = self.adaptation.current_profile.color();
        let profile_name = self.adaptation.current_profile.name();
        let profile_txt = format!("[{}]", profile_name);
        canvas.draw_text((canvas.width as i32) - 490, 14, &profile_txt, profile_color, 1);

        // Score
        let score_txt = format!("SCORE KVL: {}", score);
        canvas.draw_text((canvas.width as i32) - 180, 12, &score_txt, 0x00FBBF24, 2);

        // Aire de jeu (Cadre)
        canvas.draw_rect_outline(15, 55, (canvas.width as u32) - 30, 380, 0x002B364F, 2);

        // Rendu des portes
        for door in &self.doors {
            let door_color = if door.is_open { 0x0010B981 } else { 0x00EF4444 };
            canvas.draw_rect(
                door.x as i32,
                door.y as i32,
                door.w as u32,
                door.h as u32,
                door_color,
            );
            let door_label = if door.is_open { "OUVERTE" } else { "PORTE" };
            canvas.draw_text(door.x as i32 - 10, door.y as i32 - 18, door_label, door_color, 1);
        }

        // Rendu des pièces dorées
        for coin in &self.coins {
            if !coin.collected {
                canvas.draw_circle(coin.x as i32, coin.y as i32, coin.radius as i32, 0x00F59E0B);
                canvas.draw_circle(coin.x as i32, coin.y as i32, (coin.radius * 0.6) as i32, 0x00FDE047);
            }
        }

        // Rendu des particules
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
        canvas.draw_text(
            (self.player_x as i32) - 18,
            (self.player_y as i32) - half - 15,
            "JOUEUR",
            0x0093C5FD,
            1,
        );

        // Bandeau inférieur : Statut et traçabilité KVL
        let bottom_y = 445;
        canvas.draw_rect(0, bottom_y as i32, canvas.width as u32, 155, 0x000F172A);
        canvas.draw_rect(0, bottom_y as i32, canvas.width as u32, 1, 0x00334155);

        canvas.draw_text(15, bottom_y + 8, "CONTROLES : F1 (Overlay Telemetrie) | F2 (Changer Profil) | F3 (Auto) | R (Reload)", 0x0094A3B8, 1);
        canvas.draw_text(15, bottom_y + 24, &self.status_message, 0x0038BDF8, 1);

        canvas.draw_text(15, bottom_y + 45, "TRACES RUNTIME KVL :", 0x0094A3B8, 1);
        let log_count = self.kvl_context.execution_log.len();
        let start_idx = log_count.saturating_sub(4);
        for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
            let trace_line = format!("- [{}] {} : {}", step.node_id, step.node_name, step.message);
            canvas.draw_text(15, bottom_y + 65 + (i as i32 * 18), &trace_line, 0x00A7F3D0, 1);
        }

        // Overlay Télémétrie & Profiling (F1)
        if self.telemetry.show_overlay {
            let ov_x = (canvas.width as i32) - 310;
            let ov_y = 65;
            let ov_w = 295;
            let ov_h = 240;

            // Panneau semi-transparent sombre
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

            let particles_line = format!("Particules actives : {} / {}", self.particles.len(), self.adaptation.current_profile.max_particles());
            canvas.draw_text(ov_x + 10, ov_y + 92, &particles_line, 0x0094A3B8, 1);

            // Sparkline Graph des 120 dernières frames
            canvas.draw_text(ov_x + 10, ov_y + 115, "TEMPS DE FRAME (HISTORIQUE) :", 0x0094A3B8, 1);
            let graph_y = ov_y + 130;
            let graph_h = 70;
            canvas.draw_rect(ov_x + 10, graph_y, 275, graph_h as u32, 0x00161F33);
            canvas.draw_rect_outline(ov_x + 10, graph_y, 275, graph_h as u32, 0x002B364F, 1);

            let samples = &self.telemetry.frame_times_us;
            for (idx, &time_us) in samples.iter().enumerate() {
                let bar_x = (ov_x + 12) + (idx as i32 * 2);
                if bar_x < (ov_x + 10 + 273) {
                    // Échelle : 16666 us = 50px de hauteur
                    let bar_h = ((time_us as f32 / 3000.0) * (graph_h as f32 * 0.7)).min(graph_h as f32 - 4.0) as u32;
                    let bar_color = if time_us > 16_666 { 0x00EF4444 } else if time_us > 8_000 { 0x00FBBF24 } else { 0x0010B981 };
                    canvas.draw_rect(bar_x, graph_y + graph_h - (bar_h as i32) - 2, 2, bar_h.max(2), bar_color);
                }
            }

            canvas.draw_text(ov_x + 10, ov_y + 215, "Touche F2 pour forcer le profil", 0x0064748B, 1);
        }
    }
}
