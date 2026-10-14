use crate::input::InputState;
use crate::renderer::Canvas;
use kvl_core::*;
use kvl_runtime::*;
use winit::keyboard::KeyCode;

pub struct Coin {
    pub x: f32,
    pub y: f32,
    pub collected: bool,
}

pub struct Door {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub is_open: bool,
}

pub struct GameState {
    pub player_x: f32,
    pub player_y: f32,
    pub player_speed: f32,
    pub player_size: f32,
    pub coins: Vec<Coin>,
    pub door: Door,
    pub kvl_graph: Graph,
    pub kvl_context: ExecutionContext,
    pub status_message: String,
}

impl GameState {
    pub fn new() -> Self {
        // 1. Initialiser le graphe KVL
        let mut graph = Graph::new(
            "game_logic",
            "Gameplay Démo 2D",
            "Gère la collecte des pièces et l'interaction avec la porte.",
        );

        graph.add_variable("score", Value::Int(0));
        graph.add_variable("door_unlocked", Value::Bool(false));

        // Nœud événement : Pièce
        let node_coin = Node::new(
            "evt_coin",
            "Collecte Pièce",
            Category::Interactions,
            "Déclenché lors du contact joueur avec une pièce",
            NodeKind::Event {
                event_type: "OnPlayerOverlap".into(),
            },
        )
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let node_add_score = Node::new(
            "act_score",
            "Score +10",
            Category::Progression,
            "Incrémente le score de 10 points",
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(node_coin);
        graph.add_node(node_add_score);
        graph.add_connection(Connection::new(
            "c_coin",
            "evt_coin",
            "out_trigger",
            "act_score",
            "in_trigger",
        ));

        // Nœud événement : Porte
        let node_door = Node::new(
            "evt_door",
            "Interaction Porte",
            Category::Interactions,
            "Déclenché lors de l'appui sur E près de la porte",
            NodeKind::Event {
                event_type: "OnDoorInteract".into(),
            },
        )
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let node_toggle_door = Node::new(
            "act_door",
            "Basculer Porte",
            Category::World,
            "Ouvre ou ferme la porte",
            NodeKind::Action {
                action_type: "ToggleDoor".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(node_door);
        graph.add_node(node_toggle_door);
        graph.add_connection(Connection::new(
            "c_door",
            "evt_door",
            "out_trigger",
            "act_door",
            "in_trigger",
        ));

        let kvl_context = Interpreter::init_context(&graph);

        Self {
            player_x: 100.0,
            player_y: 240.0,
            player_speed: 220.0,
            player_size: 28.0,
            coins: vec![
                Coin { x: 260.0, y: 240.0, collected: false },
                Coin { x: 380.0, y: 180.0, collected: false },
                Coin { x: 500.0, y: 260.0, collected: false },
                Coin { x: 620.0, y: 200.0, collected: false },
            ],
            door: Door {
                x: 720.0,
                y: 190.0,
                w: 24.0,
                h: 80.0,
                is_open: false,
            },
            kvl_graph: graph,
            kvl_context,
            status_message: "Bienvenue ! Déplacez-vous avec les Flèches / ZQSD et appuyez sur E près de la porte.".into(),
        }
    }

    pub fn update(&mut self, dt: f32, input: &InputState) {
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

        // 1. Détection des collisions avec les pièces -> Déclencheur KVL
        for coin in &mut self.coins {
            if !coin.collected {
                let dx = self.player_x - coin.x;
                let dy = self.player_y - coin.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < (self.player_size / 2.0 + 12.0) {
                    coin.collected = true;

                    // Déclenchement de l'événement dans KVL !
                    let _ = Interpreter::trigger_event(
                        &self.kvl_graph,
                        &mut self.kvl_context,
                        "OnPlayerOverlap",
                    );

                    // Mettre à jour le score du jeu d'après la variable KVL
                    let score = match self.kvl_context.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };
                    self.status_message = format!("Pièce collectée ! Score KVL : {}", score);
                }
            }
        }

        // 2. Interaction avec la porte (touche E ou Espace)
        let dx_door = self.player_x - (self.door.x + self.door.w / 2.0);
        let dy_door = self.player_y - (self.door.y + self.door.h / 2.0);
        let dist_door = (dx_door * dx_door + dy_door * dy_door).sqrt();

        if dist_door < 55.0 && (input.is_key_just_pressed(KeyCode::KeyE) || input.is_key_just_pressed(KeyCode::Space)) {
            // Déclencheur KVL
            let _ = Interpreter::trigger_event(
                &self.kvl_graph,
                &mut self.kvl_context,
                "OnDoorInteract",
            );

            self.door.is_open = !self.door.is_open;
            self.status_message = if self.door.is_open {
                "Porte déverrouillée et ouverte par la logique KVL !".into()
            } else {
                "Porte fermée.".into()
            };
        }
    }

    pub fn render(&self, canvas: &mut Canvas) {
        // Fond sombre moderne (Dark Theme)
        canvas.clear(0x00141923);

        // Bandeau supérieur (HUD)
        canvas.draw_rect(0, 0, canvas.width as u32, 45, 0x001E2638);
        canvas.draw_rect(0, 44, canvas.width as u32, 2, 0x003B82F6);

        // Titre & Score
        let score = match self.kvl_context.get_var("score") {
            Some(Value::Int(s)) => *s,
            _ => 0,
        };

        canvas.draw_text(15, 12, "KARACTIRAGE 2D ENGINE (LOW-END PROFILE)", 0x00FFFFFF, 2);
        let score_txt = format!("SCORE KVL : {}", score);
        canvas.draw_text((canvas.width as i32) - 220, 12, &score_txt, 0x00FBBF24, 2);

        // Aire de jeu (Cadre)
        canvas.draw_rect_outline(15, 55, (canvas.width as u32) - 30, 380, 0x002B364F, 2);

        // Rendu de la porte
        let door_color = if self.door.is_open { 0x0010B981 } else { 0x00EF4444 };
        canvas.draw_rect(
            self.door.x as i32,
            self.door.y as i32,
            self.door.w as u32,
            self.door.h as u32,
            door_color,
        );
        let door_label = if self.door.is_open { "OUVERTE" } else { "PORTE" };
        canvas.draw_text(self.door.x as i32 - 10, self.door.y as i32 - 18, door_label, door_color, 1);

        // Rendu des pièces dorées
        for coin in &self.coins {
            if !coin.collected {
                canvas.draw_circle(coin.x as i32, coin.y as i32, 11, 0x00F59E0B);
                canvas.draw_circle(coin.x as i32, coin.y as i32, 7, 0x00FDE047);
            }
        }

        // Rendu du joueur (rectangle bleu avec lueur)
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

        // Bandeau inférieur : Zone de statut et logs KVL
        let bottom_y = 445;
        canvas.draw_rect(0, bottom_y as i32, canvas.width as u32, 155, 0x000F172A);
        canvas.draw_rect(0, bottom_y as i32, canvas.width as u32, 1, 0x00334155);

        canvas.draw_text(15, bottom_y + 10, "MESSAGE DU MOTEUR :", 0x0094A3B8, 1);
        canvas.draw_text(15, bottom_y + 25, &self.status_message, 0x0038BDF8, 1);

        canvas.draw_text(15, bottom_y + 50, "DERNIERES TRACES DU RUNTIME KVL (DETERMINISTE) :", 0x0094A3B8, 1);
        let log_count = self.kvl_context.execution_log.len();
        let start_idx = log_count.saturating_sub(4);
        for (i, step) in self.kvl_context.execution_log[start_idx..].iter().enumerate() {
            let trace_line = format!("- [{}] {} : {}", step.node_id, step.node_name, step.message);
            canvas.draw_text(15, bottom_y + 70 + (i as i32 * 18), &trace_line, 0x00A7F3D0, 1);
        }
    }
}
