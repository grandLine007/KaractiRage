use crate::context::ExecutionContext;
use kvl_core::{Graph, Node, NodeKind, Value};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Boucle infinie détectée : nombre maximum de pas ({0}) dépassé.")]
    MaxStepsExceeded(usize),

    #[error("Nœud non trouvé : {0}")]
    NodeNotFound(String),

    #[error("Condition invalide sur le nœud {0} : type attendu Bool")]
    InvalidConditionType(String),
}

pub struct Interpreter;

impl Interpreter {
    /// Initialise le contexte avec les variables par défaut du graphe
    pub fn init_context(graph: &Graph) -> ExecutionContext {
        let mut ctx = ExecutionContext::new();
        for (name, var) in &graph.variables {
            ctx.set_var(name.clone(), var.initial_value.clone());
        }
        ctx
    }

    /// Déclenche un événement par son type (ex: "OnPlayerOverlap")
    pub fn trigger_event(
        graph: &Graph,
        ctx: &mut ExecutionContext,
        event_type: &str,
    ) -> Result<usize, RuntimeError> {
        let mut executed_nodes = 0;

        // Trouver tous les nœuds événements correspondant
        let event_nodes: Vec<&Node> = graph
            .nodes
            .values()
            .filter(|n| match &n.kind {
                NodeKind::Event { event_type: et } => et == event_type,
                _ => false,
            })
            .collect();

        for event_node in event_nodes {
            ctx.record_step(
                &event_node.id,
                &event_node.name,
                format!("Événement déclenché : {}", event_type),
            );
            executed_nodes += 1;

            // Suivre le flow depuis les sorties Trigger de ce nœud
            Self::execute_flow(graph, ctx, &event_node.id, "out_trigger", &mut executed_nodes)?;
        }

        Ok(executed_nodes)
    }

    fn execute_flow(
        graph: &Graph,
        ctx: &mut ExecutionContext,
        from_node_id: &str,
        from_pin_id: &str,
        executed_nodes: &mut usize,
    ) -> Result<(), RuntimeError> {
        // Trouver les connexions partant de ce pin
        let connections: Vec<&kvl_core::Connection> = graph
            .connections
            .iter()
            .filter(|c| c.from_node == from_node_id && c.from_pin == from_pin_id)
            .collect();

        for conn in connections {
            ctx.steps_count += 1;
            if ctx.steps_count > ctx.max_steps {
                return Err(RuntimeError::MaxStepsExceeded(ctx.max_steps));
            }

            let next_node = match graph.nodes.get(&conn.to_node) {
                Some(n) => n,
                None => return Err(RuntimeError::NodeNotFound(conn.to_node.clone())),
            };

            *executed_nodes += 1;

            match &next_node.kind {
                NodeKind::Action { action_type } => {
                    ctx.record_step(
                        &next_node.id,
                        &next_node.name,
                        format!("Exécution Action [{}]", action_type),
                    );

                    // Exemple de traitement d'actions intégrées
                    match action_type.as_str() {
                        "AddScore" => {
                            let current_score = match ctx.get_var("score") {
                                Some(Value::Int(s)) => *s,
                                _ => 0,
                            };
                            ctx.set_var("score", Value::Int(current_score + 1));
                        }
                        "BonusPoints" => {
                            let current_score = match ctx.get_var("score") {
                                Some(Value::Int(s)) => *s,
                                _ => 0,
                            };
                            ctx.set_var("score", Value::Int(current_score + 50));
                            ctx.record_step(&next_node.id, &next_node.name, "Bonus majeur de +50 points appliqué !");
                        }
                        "BoostSpeed" => {
                            let current_boost = match ctx.get_var("speed_boost") {
                                Some(Value::Int(b)) => *b,
                                _ => 0,
                            };
                            ctx.set_var("speed_boost", Value::Int(current_boost + 120));
                            ctx.record_step(&next_node.id, &next_node.name, "Accélération du joueur (+120 px/s) !");
                        }
                        "SpawnSparkles" => {
                            let count = match ctx.get_var("sparkle_burst") {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            ctx.set_var("sparkle_burst", Value::Int(count + 1));
                            ctx.record_step(&next_node.id, &next_node.name, "Pluie d'étincelles dorées déclenchée !");
                        }
                        "ToggleDoor" => {
                            let is_open = match ctx.get_var("door_is_open") {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            };
                            ctx.set_var("door_is_open", Value::Bool(!is_open));
                            ctx.record_step(
                                &next_node.id,
                                &next_node.name,
                                if !is_open { "Porte déverrouillée et ouverte !" } else { "Porte refermée." },
                            );
                        }
                        "DenyAccess" => {
                            ctx.set_var("access_denied", Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, "Accès refusé ! Condition non remplie.");
                        }
                        "TriggerAlarm" => {
                            ctx.set_var("alarm_active", Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, "Alerte de patrouille déclenchée : intrus repéré !");
                        }
                        "ClearAlarm" => {
                            ctx.set_var("alarm_active", Value::Bool(false));
                            ctx.record_step(&next_node.id, &next_node.name, "Alerte levée : retour à la patrouille normale.");
                        }
                        "LockAllDoors" => {
                            ctx.set_var("door_is_open", Value::Bool(false));
                            ctx.record_step(&next_node.id, &next_node.name, "Verrouillage de sécurité d'urgence des portes !");
                        }
                        "ResetScore" => {
                            ctx.set_var("score", Value::Int(0));
                            ctx.record_step(&next_node.id, &next_node.name, "Score réinitialisé à 0 !");
                        }
                        "DamagePlayer" => {
                            let current_hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(hp)) => *hp,
                                _ => 3,
                            };
                            let new_hp = (current_hp - 1).max(0);
                            ctx.set_var("player_hp", Value::Int(new_hp));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Dégâts infligés au joueur : -1 PV (Reste: {})", new_hp));
                        }
                        op if op.starts_with("DamagePlayer:") => {
                            let amount = op.trim_start_matches("DamagePlayer:").parse::<i64>().unwrap_or(1);
                            let current_hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(hp)) => *hp,
                                _ => 3,
                            };
                            let new_hp = (current_hp - amount).max(0);
                            ctx.set_var("player_hp", Value::Int(new_hp));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Dégâts infligés au joueur : -{} PV (Reste: {})", amount, new_hp));
                        }
                        "HealPlayer" => {
                            let current_hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(hp)) => *hp,
                                _ => 3,
                            };
                            let max_hp = match ctx.get_var("player_max_hp") {
                                Some(Value::Int(m)) => *m,
                                _ => 3,
                            };
                            let new_hp = (current_hp + 1).min(max_hp);
                            ctx.set_var("player_hp", Value::Int(new_hp));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Soin accordé au joueur : +1 PV (Total: {})", new_hp));
                        }
                        op if op.starts_with("HealPlayer:") => {
                            let amount = op.trim_start_matches("HealPlayer:").parse::<i64>().unwrap_or(1);
                            let current_hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(hp)) => *hp,
                                _ => 3,
                            };
                            let max_hp = match ctx.get_var("player_max_hp") {
                                Some(Value::Int(m)) => *m,
                                _ => 3,
                            };
                            let new_hp = (current_hp + amount).min(max_hp);
                            ctx.set_var("player_hp", Value::Int(new_hp));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Soin accordé au joueur : +{} PV (Total: {})", amount, new_hp));
                        }
                        "TeleportPlayer" => {
                            ctx.set_var("teleport_target_x", Value::Int(120));
                            ctx.set_var("teleport_target_y", Value::Int(480));
                            ctx.record_step(&next_node.id, &next_node.name, "Téléportation du joueur vers (120, 480) !");
                        }
                        op if op.starts_with("TeleportTo:") => {
                            let coords = op.trim_start_matches("TeleportTo:");
                            let mut parts = coords.split(',');
                            let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(120);
                            let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(480);
                            ctx.set_var("teleport_target_x", Value::Int(tx));
                            ctx.set_var("teleport_target_y", Value::Int(ty));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Téléportation de l'entité vers ({}, {}) !", tx, ty));
                        }
                        "SlowSpeed" => {
                            let current_boost = match ctx.get_var("speed_boost") {
                                Some(Value::Int(b)) => *b,
                                _ => 0,
                            };
                            ctx.set_var("speed_boost", Value::Int(current_boost - 80));
                            ctx.record_step(&next_node.id, &next_node.name, "Ralentissement de l'entité (-80 px/s) !");
                        }
                        op if op.starts_with("ModifySpeed:") => {
                            let delta = op.trim_start_matches("ModifySpeed:").parse::<i64>().unwrap_or(0);
                            let current_boost = match ctx.get_var("speed_boost") {
                                Some(Value::Int(b)) => *b,
                                _ => 0,
                            };
                            ctx.set_var("speed_boost", Value::Int(current_boost + delta));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Vitesse modifiée ({:+} px/s) !", delta));
                        }
                        "NavChasePlayer" | "ChasePlayer" => {
                            ctx.set_var("nav_mode", Value::String("ChasePlayer".into()));
                            ctx.record_step(&next_node.id, &next_node.name, "IA : Ordre de traque A* du joueur activé !");
                        }
                        "NavStop" | "StopNavigation" => {
                            ctx.set_var("nav_mode", Value::String("Idle".into()));
                            ctx.record_step(&next_node.id, &next_node.name, "IA : Ordre d'arrêt de navigation.");
                        }
                        op if op.starts_with("NavGoTo:") || op.starts_with("NavigateTo:") => {
                            let prefix = if op.starts_with("NavGoTo:") { "NavGoTo:" } else { "NavigateTo:" };
                            let coords = op.trim_start_matches(prefix);
                            let mut parts = coords.split(',');
                            let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                            let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                            ctx.set_var("nav_mode", Value::String(format!("GoTo:{},{}", tx, ty)));
                            ctx.set_var("nav_target_x", Value::Int(tx));
                            ctx.set_var("nav_target_y", Value::Int(ty));
                            ctx.record_step(&next_node.id, &next_node.name, format!("IA : Navigation A* vers ({}, {}) !", tx, ty));
                        }
                        "SwarmScatter" => {
                            ctx.set_var("swarm_scatter_1", Value::Float(2.0));
                            ctx.set_var("swarm_is_scattered_1", Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, "Meute : Éparpillement d'urgence (Scatter) activé !");
                        }
                        op if op.starts_with("SwarmScatter:") => {
                            let rest = op.trim_start_matches("SwarmScatter:");
                            let mut parts = rest.split(',');
                            let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                            let dur = parts.next().and_then(|s| s.parse::<f64>().ok()).unwrap_or(2.0);
                            ctx.set_var(format!("swarm_scatter_{}", swarm_id), Value::Float(dur));
                            ctx.set_var(format!("swarm_is_scattered_{}", swarm_id), Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Meute #{} : Éparpillement d'urgence ({:.1}s) déclenché !", swarm_id, dur));
                        }
                        "SwarmTargetPlayer" => {
                            ctx.set_var("swarm_target_mode_1", Value::String("Player".into()));
                            ctx.record_step(&next_node.id, &next_node.name, "Meute : Traque collective du joueur enclenchée !");
                        }
                        op if op.starts_with("SwarmTargetPlayer:") => {
                            let swarm_id = op.trim_start_matches("SwarmTargetPlayer:").parse::<u32>().unwrap_or(1);
                            ctx.set_var(format!("swarm_target_mode_{}", swarm_id), Value::String("Player".into()));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Meute #{} : Traque collective du joueur enclenchée !", swarm_id));
                        }
                        op if op.starts_with("SwarmSetTarget:") => {
                            let rest = op.trim_start_matches("SwarmSetTarget:");
                            let mut parts = rest.split(',');
                            let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                            let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                            let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                            ctx.set_var(format!("swarm_target_x_{}", swarm_id), Value::Int(tx));
                            ctx.set_var(format!("swarm_target_y_{}", swarm_id), Value::Int(ty));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Meute #{} : Cible collective assignée en ({}, {})", swarm_id, tx, ty));
                        }
                        op if op.starts_with("SwarmSetCohesion:") => {
                            let rest = op.trim_start_matches("SwarmSetCohesion:");
                            let mut parts = rest.split(',');
                            let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                            let weight = parts.next().and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
                            ctx.set_var(format!("swarm_cohesion_{}", swarm_id), Value::Float(weight));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Meute #{} : Poids de cohésion ajusté à {:.2}", swarm_id, weight));
                        }
                        op if op.starts_with("IncrementCounter:") => {
                            let counter_name = op.trim_start_matches("IncrementCounter:");
                            let current_count = match ctx.get_var(counter_name) {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            ctx.set_var(counter_name, Value::Int(current_count + 1));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Compteur '{}' incrémenté : {}", counter_name, current_count + 1));
                        }
                        op if op.starts_with("AddVar:") => {
                            let rest = op.trim_start_matches("AddVar:");
                            let mut parts = rest.split(':');
                            if let (Some(var_name), Some(delta_str)) = (parts.next(), parts.next()) {
                                let delta = delta_str.parse::<i64>().unwrap_or(0);
                                let current_val = match ctx.get_var(var_name) {
                                    Some(Value::Int(v)) => *v,
                                    _ => 0,
                                };
                                ctx.set_var(var_name, Value::Int(current_val + delta));
                                ctx.record_step(&next_node.id, &next_node.name, format!("Variable '{}' ajustée de {} -> {}", var_name, delta, current_val + delta));
                            }
                        }
                        op if op.starts_with("SetVar:") => {
                            let rest = op.trim_start_matches("SetVar:");
                            let mut parts = rest.split(':');
                            if let (Some(var_name), Some(val_str)) = (parts.next(), parts.next()) {
                                let val = val_str.parse::<i64>().unwrap_or(0);
                                ctx.set_var(var_name, Value::Int(val));
                                ctx.record_step(&next_node.id, &next_node.name, format!("Variable '{}' fixée à {}", var_name, val));
                            }
                        }
                        "DestroySelf" => {
                            ctx.record_step(&next_node.id, &next_node.name, "Objet marqué pour destruction");
                        }
                        "EmitBurst" => {
                            let current = match ctx.get_var("emitter_burst") {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            ctx.set_var("emitter_burst", Value::Int(current + 25));
                            ctx.record_step(&next_node.id, &next_node.name, "Gerbe de particules (Burst) déclenchée !");
                        }
                        op if op.starts_with("EmitBurst:") => {
                            let count = op.trim_start_matches("EmitBurst:").parse::<i64>().unwrap_or(25);
                            let current = match ctx.get_var("emitter_burst") {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            ctx.set_var("emitter_burst", Value::Int(current + count));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Gerbe de {} particules déclenchée !", count));
                        }
                        "ToggleEmitter" => {
                            let active = match ctx.get_var("emitter_active") {
                                Some(Value::Bool(b)) => *b,
                                _ => true,
                            };
                            ctx.set_var("emitter_active", Value::Bool(!active));
                            ctx.record_step(&next_node.id, &next_node.name, if !active { "Émetteur de particules activé !" } else { "Émetteur de particules éteint." });
                        }
                        op if op.starts_with("ToggleEmitter:") => {
                            let state_str = op.trim_start_matches("ToggleEmitter:");
                            let state = state_str == "true" || state_str == "1" || state_str == "on";
                            ctx.set_var("emitter_active", Value::Bool(state));
                            ctx.record_step(&next_node.id, &next_node.name, if state { "Émetteur de particules allumé !" } else { "Émetteur de particules éteint." });
                        }
                        op if op.starts_with("ShowDialogue:") => {
                            let rest = op.trim_start_matches("ShowDialogue:");
                            let mut parts = rest.splitn(2, ':');
                            let speaker = parts.next().unwrap_or("PNJ");
                            let text = parts.next().unwrap_or("");
                            ctx.set_var("dialogue_active", Value::Bool(true));
                            ctx.set_var("dialogue_speaker", Value::String(speaker.to_string()));
                            ctx.set_var("dialogue_text", Value::String(text.to_string()));
                            ctx.set_var("dialogue_has_choices", Value::Bool(false));
                            ctx.set_var("dialogue_choice_1", Value::String(String::new()));
                            ctx.set_var("dialogue_choice_2", Value::String(String::new()));
                            ctx.set_var("dialogue_selected_choice", Value::Int(0));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Ouverture dialogue [{}] : {}", speaker, text));
                        }
                        op if op.starts_with("ShowDialogueChoice:") => {
                            let rest = op.trim_start_matches("ShowDialogueChoice:");
                            let mut parts = rest.splitn(4, ':');
                            let speaker = parts.next().unwrap_or("PNJ");
                            let text = parts.next().unwrap_or("");
                            let c1 = parts.next().unwrap_or("Option 1");
                            let c2 = parts.next().unwrap_or("Option 2");
                            ctx.set_var("dialogue_active", Value::Bool(true));
                            ctx.set_var("dialogue_speaker", Value::String(speaker.to_string()));
                            ctx.set_var("dialogue_text", Value::String(text.to_string()));
                            ctx.set_var("dialogue_has_choices", Value::Bool(true));
                            ctx.set_var("dialogue_choice_1", Value::String(c1.to_string()));
                            ctx.set_var("dialogue_choice_2", Value::String(c2.to_string()));
                            ctx.set_var("dialogue_selected_choice", Value::Int(0));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Dialogue à choix [{}] : {} ([1] {}, [2] {})", speaker, text, c1, c2));
                        }
                        "CloseDialogue" => {
                            ctx.set_var("dialogue_active", Value::Bool(false));
                            ctx.record_step(&next_node.id, &next_node.name, "Fermeture de la boîte de dialogue");
                        }
                        op if op.starts_with("SetQuest:") => {
                            let rest = op.trim_start_matches("SetQuest:");
                            let mut parts = rest.split(':');
                            if let (Some(qid), Some(step_str)) = (parts.next(), parts.next()) {
                                let step = step_str.parse::<i64>().unwrap_or(1);
                                ctx.set_var(format!("quest_{}", qid), Value::Int(step));
                                ctx.record_step(&next_node.id, &next_node.name, format!("Quête '{}' positionnée à l'étape {}", qid, step));
                            }
                        }
                        op if op.starts_with("AdvanceQuest:") => {
                            let qid = op.trim_start_matches("AdvanceQuest:");
                            let key = format!("quest_{}", qid);
                            let current = match ctx.get_var(&key) {
                                Some(Value::Int(s)) => *s,
                                _ => 0,
                            };
                            let next = current + 1;
                            ctx.set_var(&key, Value::Int(next));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Quête '{}' avancée à l'étape {}", qid, next));
                        }
                        op if op.starts_with("CompleteQuest:") => {
                            let qid = op.trim_start_matches("CompleteQuest:");
                            ctx.set_var(format!("quest_{}", qid), Value::Int(100));
                            ctx.set_var(format!("quest_completed_{}", qid), Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Quête '{}' accomplie avec succès !", qid));
                        }
                        op if op.starts_with("GiveItem:") => {
                            let rest = op.trim_start_matches("GiveItem:");
                            let mut parts = rest.split(':');
                            let item_id = parts.next().unwrap_or("item");
                            let count = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
                            let current_count = match ctx.get_var(&format!("item_count_{}", item_id)) {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            let new_count = current_count + count;
                            ctx.set_var(format!("item_count_{}", item_id), Value::Int(new_count));
                            ctx.set_var(format!("has_{}", item_id), Value::Bool(true));
                            ctx.set_var("last_picked_item", Value::String(item_id.to_string()));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Objet '{}' ajouté à l'inventaire (+{}) (Total: {})", item_id, count, new_count));
                        }
                        op if op.starts_with("RemoveItem:") => {
                            let rest = op.trim_start_matches("RemoveItem:");
                            let mut parts = rest.split(':');
                            let item_id = parts.next().unwrap_or("item");
                            let count = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
                            let current_count = match ctx.get_var(&format!("item_count_{}", item_id)) {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            let new_count = (current_count - count).max(0);
                            ctx.set_var(format!("item_count_{}", item_id), Value::Int(new_count));
                            ctx.set_var(format!("has_{}", item_id), Value::Bool(new_count > 0));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Objet '{}' retiré de l'inventaire (-{}) (Reste: {})", item_id, count, new_count));
                        }
                        op if op.starts_with("SelectHotbarSlot:") => {
                            let slot_idx = op.trim_start_matches("SelectHotbarSlot:").parse::<i64>().unwrap_or(0);
                            ctx.set_var("hotbar_selected_slot", Value::Int(slot_idx));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Emplacement de barre d'action sélectionné : {}", slot_idx));
                        }
                        op if op.starts_with("UseItem:") => {
                            let item_id = op.trim_start_matches("UseItem:");
                            ctx.set_var(format!("used_item_{}", item_id), Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Objet '{}' utilisé via KVL !", item_id));
                        }
                        op if op.starts_with("SetBossPhase:") => {
                            let phase_str = op.trim_start_matches("SetBossPhase:");
                            let phase_val = match phase_str {
                                "1" | "Phase1" => 1,
                                "2" | "Phase2" => 2,
                                "3" | "Phase3" => 3,
                                "Defeated" | "0" | "4" => 4,
                                _ => phase_str.parse::<i64>().unwrap_or(1),
                            };
                            ctx.set_var("boss_phase", Value::Int(phase_val));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Phase du Boss basculée vers {}", phase_str));
                        }
                        "BossEnrage" => {
                            ctx.set_var("boss_enraged", Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, "Le Boss entre en mode OVERDRIVE ENRAGÉ !");
                        }
                        "SetBossDefeated" => {
                            ctx.set_var("boss_defeated", Value::Bool(true));
                            ctx.set_var("boss_phase", Value::Int(4));
                            ctx.record_step(&next_node.id, &next_node.name, "Le Boss est terrassé ! Victoire totale !");
                        }
                        op if op.starts_with("TriggerBossTelegraph:") => {
                            let atk = op.trim_start_matches("TriggerBossTelegraph:");
                            ctx.set_var("boss_telegraph_attack", Value::String(atk.to_string()));
                            ctx.record_step(&next_node.id, &next_node.name, format!("Télégraphie d'attaque armée : [{}]", atk));
                        }
                        "TriggerBossRoar" => {
                            ctx.set_var("boss_roar_triggered", Value::Bool(true));
                            ctx.record_step(&next_node.id, &next_node.name, "Grondement terrifiant du Boss !");
                        }
                        _ => {
                            ctx.record_step(&next_node.id, &next_node.name, format!("Action personnalisée {}", action_type));
                        }
                    }

                    // Poursuivre le flow si une sortie trigger existe
                    Self::execute_flow(graph, ctx, &next_node.id, "out_trigger", executed_nodes)?;
                }

                NodeKind::Condition { operator } => {
                    let current_score = match ctx.get_var("score") {
                        Some(Value::Int(s)) => *s,
                        _ => 0,
                    };

                    let is_true = match operator.as_str() {
                        "IsPlayerAlive" => {
                            let hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(h)) => *h,
                                _ => 3,
                            };
                            hp > 0
                        }
                        op if op.starts_with("HealthLessThan:") || op.starts_with("HealthLt:") => {
                            let threshold = if op.starts_with("HealthLessThan:") {
                                op.trim_start_matches("HealthLessThan:")
                            } else {
                                op.trim_start_matches("HealthLt:")
                            }.parse::<i64>().unwrap_or(1);
                            let hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(h)) => *h,
                                _ => 3,
                            };
                            hp < threshold
                        }
                        op if op.starts_with("HealthGte:") => {
                            let threshold = op.trim_start_matches("HealthGte:").parse::<i64>().unwrap_or(1);
                            let hp = match ctx.get_var("player_hp") {
                                Some(Value::Int(h)) => *h,
                                _ => 3,
                            };
                            hp >= threshold
                        }
                        "ScoreAtLeast50" | "ScoreGte50" => current_score >= 50,
                        "ScoreAtLeast20" | "ScoreGte20" => current_score >= 20,
                        "ScoreAtLeast10" | "ScoreGte10" => current_score >= 10,
                        op if op.starts_with("ScoreAtLeast:") || op.starts_with("ScoreGte:") => {
                            let threshold = op.split(':').nth(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(50);
                            current_score >= threshold
                        }
                        op if op.starts_with("CounterAtLeast:") || op.starts_with("CounterGte:") => {
                            let rest = if op.starts_with("CounterAtLeast:") {
                                op.trim_start_matches("CounterAtLeast:")
                            } else {
                                op.trim_start_matches("CounterGte:")
                            };
                            let mut parts = rest.split(':');
                            if let (Some(counter_name), Some(threshold_str)) = (parts.next(), parts.next()) {
                                let threshold = threshold_str.parse::<i64>().unwrap_or(1);
                                let count = match ctx.get_var(counter_name) {
                                    Some(Value::Int(c)) => *c,
                                    _ => 0,
                                };
                                count >= threshold
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("VarGte:") => {
                            let rest = op.trim_start_matches("VarGte:");
                            let mut parts = rest.split(':');
                            if let (Some(var_name), Some(threshold_str)) = (parts.next(), parts.next()) {
                                let threshold = threshold_str.parse::<i64>().unwrap_or(0);
                                let val = match ctx.get_var(var_name) {
                                    Some(Value::Int(v)) => *v,
                                    _ => 0,
                                };
                                val >= threshold
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("VarEqBool:") => {
                            let rest = op.trim_start_matches("VarEqBool:");
                            let mut parts = rest.split(':');
                            if let (Some(var_name), Some(bool_str)) = (parts.next(), parts.next()) {
                                let target_b = bool_str == "true";
                                match ctx.get_var(var_name) {
                                    Some(Value::Bool(b)) => *b == target_b,
                                    _ => false,
                                }
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("CheckVarBool:") => {
                            let var_name = op.split(':').nth(1).unwrap_or("");
                            match ctx.get_var(var_name) {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            }
                        }
                        "IsDialogueOpen" => {
                            match ctx.get_var("dialogue_active") {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            }
                        }
                        op if op.starts_with("DialogueChoiceEq:") => {
                            let idx = op.trim_start_matches("DialogueChoiceEq:").parse::<i64>().unwrap_or(1);
                            match ctx.get_var("dialogue_selected_choice") {
                                Some(Value::Int(c)) => *c == idx,
                                _ => false,
                            }
                        }
                        op if op.starts_with("QuestGte:") => {
                            let rest = op.trim_start_matches("QuestGte:");
                            let mut parts = rest.split(':');
                            if let (Some(qid), Some(step_str)) = (parts.next(), parts.next()) {
                                let target_step = step_str.parse::<i64>().unwrap_or(1);
                                match ctx.get_var(&format!("quest_{}", qid)) {
                                    Some(Value::Int(s)) => *s >= target_step,
                                    _ => false,
                                }
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("QuestEq:") => {
                            let rest = op.trim_start_matches("QuestEq:");
                            let mut parts = rest.split(':');
                            if let (Some(qid), Some(step_str)) = (parts.next(), parts.next()) {
                                let target_step = step_str.parse::<i64>().unwrap_or(1);
                                match ctx.get_var(&format!("quest_{}", qid)) {
                                    Some(Value::Int(s)) => *s == target_step,
                                    _ => false,
                                }
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("IsQuestCompleted:") => {
                            let qid = op.trim_start_matches("IsQuestCompleted:");
                            let is_done_flag = match ctx.get_var(&format!("quest_completed_{}", qid)) {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            };
                            let step_done = match ctx.get_var(&format!("quest_{}", qid)) {
                                Some(Value::Int(s)) => *s >= 100,
                                _ => false,
                            };
                            is_done_flag || step_done
                        }
                        op if op.starts_with("HasItem:") => {
                            let item_id = op.trim_start_matches("HasItem:");
                            let has_flag = match ctx.get_var(&format!("has_{}", item_id)) {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            };
                            let count = match ctx.get_var(&format!("item_count_{}", item_id)) {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            has_flag || count > 0
                        }
                        op if op.starts_with("ItemCountGte:") => {
                            let rest = op.trim_start_matches("ItemCountGte:");
                            let mut parts = rest.split(':');
                            if let (Some(item_id), Some(count_str)) = (parts.next(), parts.next()) {
                                let target_count = count_str.parse::<i64>().unwrap_or(1);
                                match ctx.get_var(&format!("item_count_{}", item_id)) {
                                    Some(Value::Int(c)) => *c >= target_count,
                                    _ => false,
                                }
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("ItemCountLt:") => {
                            let rest = op.trim_start_matches("ItemCountLt:");
                            let mut parts = rest.split(':');
                            if let (Some(item_id), Some(count_str)) = (parts.next(), parts.next()) {
                                let target_count = count_str.parse::<i64>().unwrap_or(1);
                                match ctx.get_var(&format!("item_count_{}", item_id)) {
                                    Some(Value::Int(c)) => *c < target_count,
                                    _ => true,
                                }
                            } else {
                                false
                            }
                        }
                        op if op.starts_with("HotbarSlotEq:") => {
                            let target_slot = op.trim_start_matches("HotbarSlotEq:").parse::<i64>().unwrap_or(0);
                            match ctx.get_var("hotbar_selected_slot") {
                                Some(Value::Int(s)) => *s == target_slot,
                                _ => false,
                            }
                        }
                        op if op.starts_with("BossPhaseEq:") || op.starts_with("BossPhaseIs:") => {
                            let target_str = if op.starts_with("BossPhaseEq:") {
                                op.trim_start_matches("BossPhaseEq:")
                            } else {
                                op.trim_start_matches("BossPhaseIs:")
                            };
                            let target_phase = match target_str {
                                "1" | "Phase1" => 1,
                                "2" | "Phase2" => 2,
                                "3" | "Phase3" => 3,
                                "Defeated" | "4" => 4,
                                _ => target_str.parse::<i64>().unwrap_or(1),
                            };
                            match ctx.get_var("boss_phase") {
                                Some(Value::Int(p)) => *p == target_phase,
                                _ => false,
                            }
                        }
                        "IsBossEnraged" => {
                            match ctx.get_var("boss_enraged") {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            }
                        }
                        "IsBossDefeated" => {
                            match ctx.get_var("boss_defeated") {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            }
                        }
                        op if op.starts_with("BossHpLt:") => {
                            let threshold = op.trim_start_matches("BossHpLt:").parse::<i64>().unwrap_or(1);
                            match ctx.get_var("boss_hp") {
                                Some(Value::Int(hp)) => *hp < threshold,
                                _ => false,
                            }
                        }
                        op if op.starts_with("BossHpGte:") => {
                            let threshold = op.trim_start_matches("BossHpGte:").parse::<i64>().unwrap_or(1);
                            match ctx.get_var("boss_hp") {
                                Some(Value::Int(hp)) => *hp >= threshold,
                                _ => false,
                            }
                        }
                        "IsNavigating" | "HasPath" => {
                            match ctx.get_var("nav_mode") {
                                Some(Value::String(m)) => m != "Idle",
                                _ => false,
                            }
                        }
                        op if op.starts_with("NavModeIs:") => {
                            let expected = op.trim_start_matches("NavModeIs:");
                            match ctx.get_var("nav_mode") {
                                Some(Value::String(m)) => m == expected,
                                _ => false,
                            }
                        }
                        op if op.starts_with("SwarmCountGreater:") || op.starts_with("SwarmCountGte:") => {
                            let rest = if op.starts_with("SwarmCountGreater:") {
                                op.trim_start_matches("SwarmCountGreater:")
                            } else {
                                op.trim_start_matches("SwarmCountGte:")
                            };
                            let mut parts = rest.split(',');
                            let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                            let threshold = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
                            let count = match ctx.get_var(&format!("swarm_count_{}", swarm_id)) {
                                Some(Value::Int(c)) => *c,
                                _ => 0,
                            };
                            count >= threshold
                        }
                        op if op.starts_with("SwarmIsScattered:") => {
                            let swarm_id = op.trim_start_matches("SwarmIsScattered:").parse::<u32>().unwrap_or(1);
                            match ctx.get_var(&format!("swarm_is_scattered_{}", swarm_id)) {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            }
                        }
                        "IsSwarmScattered" => {
                            match ctx.get_var("swarm_is_scattered_1") {
                                Some(Value::Bool(b)) => *b,
                                _ => false,
                            }
                        }
                        _ => {
                            current_score > 0
                        }
                    };

                    let msg = if is_true {
                        format!("Condition [{}] VRAIE (Score : {}) -> Branche [out_true]", operator, current_score)
                    } else {
                        format!("Condition [{}] FAUSSE (Score : {}) -> Branche [out_false]", operator, current_score)
                    };
                    ctx.record_step(&next_node.id, &next_node.name, msg);

                    let out_pin = if is_true { "out_true" } else { "out_false" };
                    Self::execute_flow(graph, ctx, &next_node.id, out_pin, executed_nodes)?;
                }

                NodeKind::Composite { macro_name, .. } => {
                    ctx.record_step(
                        &next_node.id,
                        &next_node.name,
                        format!("Exécution Bloc Composé [{}]", macro_name),
                    );
                    Self::execute_flow(graph, ctx, &next_node.id, "out_trigger", executed_nodes)?;
                }

                NodeKind::Event { .. } => {
                    // Les événements ne sont pas déclenchés par une entrée flow classique
                }
            }
        }

        Ok(())
    }
}
