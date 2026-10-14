use crate::input::InputState;
use crate::renderer::Canvas;
use crate::scene::SceneData;
use kvl_core::*;
use kvl_runtime::*;
use std::collections::HashMap;
use std::fs;
use winit::keyboard::KeyCode;

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
    pub kvl_graphs: HashMap<String, Graph>,
    pub kvl_context: ExecutionContext,
    pub status_message: String,
}

impl GameState {
    pub fn load_from_scene(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let scene_data = SceneData::load_from_file(path)?;

        let mut kvl_graphs = HashMap::new();
        let mut kvl_context = ExecutionContext::new();
        kvl_context.set_var("score", Value::Int(0));

        // Charger les scripts KVL référencés par les pièces
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

        // Charger les scripts KVL référencés par les portes
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
            kvl_graphs,
            kvl_context,
            status_message: "Scène et scripts KVL chargés depuis les fichiers JSON ! (R = Recharger)".into(),
        })
    }

    pub fn reload(&mut self) {
        if let Ok(new_state) = Self::load_from_scene(&self.scene_path) {
            *self = new_state;
            self.status_message = "Rechargement à chaud (Hot-Reload) de la scène et des KVL réussi !".into();
        } else {
            self.status_message = "Erreur lors du rechargement de la scène JSON.".into();
        }
    }

    pub fn update(&mut self, dt: f32, input: &InputState) {
        // Hot-Reload avec touche R
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

        // Limites écran
        self.player_x = self.player_x.clamp(20.0, 780.0);
        self.player_y = self.player_y.clamp(60.0, 420.0);

        // 1. Détection des collisions avec les pièces -> Déclencheur KVL chargé depuis le JSON
        for coin in &mut self.coins {
            if !coin.collected {
                let dx = self.player_x - coin.x;
                let dy = self.player_y - coin.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < (self.player_size / 2.0 + coin.radius) {
                    coin.collected = true;

                    if let Some(graph) = self.kvl_graphs.get(&coin.script_path) {
                        let _ = Interpreter::trigger_event(
                            graph,
                            &mut self.kvl_context,
                            "OnPlayerOverlap",
                        );
                    }

                    let score = match self.kvl_context.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };
                    self.status_message = format!("Pièce ({}) collectée ! Score : {}", coin.id, score);
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
                    let _ = Interpreter::trigger_event(
                        graph,
                        &mut self.kvl_context,
                        "OnDoorInteract",
                    );
                }

                door.is_open = !door.is_open;
                self.status_message = if door.is_open {
                    format!("Porte ({}) déverrouillée et ouverte !", door.id)
                } else {
                    format!("Porte ({}) fermée.", door.id)
                };
            }
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
        let score_txt = format!("SCORE KVL : {}", score);
        canvas.draw_text((canvas.width as i32) - 220, 12, &score_txt, 0x00FBBF24, 2);

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

        // Rendu du joueur (rectangle bleu)
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

        canvas.draw_text(15, bottom_y + 10, "MESSAGE DU MOTEUR (R = RECHARGER JSON) :", 0x0094A3B8, 1);
        canvas.draw_text(15, bottom_y + 25, &self.status_message, 0x0038BDF8, 1);

        canvas.draw_text(15, bottom_y + 50, "DERNIERES TRACES DU RUNTIME KVL (HOT-RELOADABLE) :", 0x0094A3B8, 1);
        let log_count = self.kvl_context.execution_log.len();
        let start_idx = log_count.saturating_sub(4);
        for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
            let trace_line = format!("- [{}] {} : {}", step.node_id, step.node_name, step.message);
            canvas.draw_text(15, bottom_y + 70 + (i as i32 * 18), &trace_line, 0x00A7F3D0, 1);
        }
    }
}
