use std::sync::OnceLock;

use crate::context::ExecutionContext;
use crate::handlers::HandlerRegistry;
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

    /// Déclenche un événement avec le registre de handlers cœur.
    pub fn trigger_event(
        graph: &Graph,
        ctx: &mut ExecutionContext,
        event_type: &str,
    ) -> Result<usize, RuntimeError> {
        Self::trigger_event_with(graph, ctx, event_type, builtin_handlers())
    }

    /// Déclenche un événement avec un registre explicite (plugins, tests, outils).
    pub fn trigger_event_with(
        graph: &Graph,
        ctx: &mut ExecutionContext,
        event_type: &str,
        handlers: &HandlerRegistry,
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
            Self::execute_flow(
                graph,
                ctx,
                &event_node.id,
                "out_trigger",
                &mut executed_nodes,
                handlers,
            )?;
        }

        Ok(executed_nodes)
    }

    fn execute_flow(
        graph: &Graph,
        ctx: &mut ExecutionContext,
        from_node_id: &str,
        from_pin_id: &str,
        executed_nodes: &mut usize,
        handlers: &HandlerRegistry,
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

                    // Toutes les actions du catalogue passent par le registre ;
                    // un type inconnu est seulement journalisé.
                    if let Some(result) =
                        handlers.dispatch(ctx, &next_node.id, &next_node.name, action_type)
                    {
                        result?;
                    } else {
                        ctx.record_step(
                            &next_node.id,
                            &next_node.name,
                            format!("Action personnalisée {}", action_type),
                        );
                    }

                    // Poursuivre le flow si une sortie trigger existe
                    Self::execute_flow(
                        graph,
                        ctx,
                        &next_node.id,
                        "out_trigger",
                        executed_nodes,
                        handlers,
                    )?;
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
                    Self::execute_flow(
                        graph,
                        ctx,
                        &next_node.id,
                        out_pin,
                        executed_nodes,
                        handlers,
                    )?;
                }

                NodeKind::Composite { macro_name, .. } => {
                    ctx.record_step(
                        &next_node.id,
                        &next_node.name,
                        format!("Exécution Bloc Composé [{}]", macro_name),
                    );
                    Self::execute_flow(
                        graph,
                        ctx,
                        &next_node.id,
                        "out_trigger",
                        executed_nodes,
                        handlers,
                    )?;
                }

                NodeKind::Event { .. } => {
                    // Les événements ne sont pas déclenchés par une entrée flow classique
                }
            }
        }

        Ok(())
    }
}

/// Registre cœur, construit une seule fois par processus.
pub fn builtin_handlers() -> &'static HandlerRegistry {
    static REGISTRY: OnceLock<HandlerRegistry> = OnceLock::new();
    REGISTRY.get_or_init(crate::actions::builtin_handlers)
}
