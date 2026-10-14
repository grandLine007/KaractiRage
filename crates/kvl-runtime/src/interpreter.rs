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
                        "DestroySelf" => {
                            ctx.record_step(&next_node.id, &next_node.name, "Objet marqué pour destruction");
                        }
                        _ => {
                            ctx.record_step(&next_node.id, &next_node.name, format!("Action personnalisée {}", action_type));
                        }
                    }

                    // Poursuivre le flow si une sortie trigger existe
                    Self::execute_flow(graph, ctx, &next_node.id, "out_trigger", executed_nodes)?;
                }

                NodeKind::Condition { operator } => {
                    ctx.record_step(
                        &next_node.id,
                        &next_node.name,
                        format!("Évaluation Condition [{}]", operator),
                    );

                    // Dans un cas complet, on évalue les pins de données connectés
                    // Pour ce premier prototype, on teste le branchement 'true' / 'false'
                    let is_true = true; // par défaut
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
