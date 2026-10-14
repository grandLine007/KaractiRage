pub mod context;
pub mod interpreter;

pub use context::{ExecutionContext, ExecutionTraceStep};
pub use interpreter::{Interpreter, RuntimeError};

#[cfg(test)]
mod tests {
    use super::*;
    use kvl_core::*;

    #[test]
    fn test_execute_coin_pickup_scenario() {
        let mut graph = Graph::new("coin_pickup", "Scénario Pièce", "Test d'exécution");
        graph.add_variable("score", Value::Int(0));

        // 1. Event
        let event_node = Node::new(
            "node_overlap",
            "Contact Pièce",
            Category::Interactions,
            "",
            NodeKind::Event {
                event_type: "OnPlayerOverlap".into(),
            },
        ).with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // 2. Action Score
        let score_node = Node::new(
            "node_score",
            "Ajout Score",
            Category::Progression,
            "",
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // 3. Action Destroy
        let destroy_node = Node::new(
            "node_destroy",
            "Détruire Pièce",
            Category::World,
            "",
            NodeKind::Action {
                action_type: "DestroySelf".into(),
            },
        ).with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(event_node);
        graph.add_node(score_node);
        graph.add_node(destroy_node);

        graph.add_connection(Connection::new(
            "c1",
            "node_overlap",
            "out_trigger",
            "node_score",
            "in_trigger",
        ));

        graph.add_connection(Connection::new(
            "c2",
            "node_score",
            "out_trigger",
            "node_destroy",
            "in_trigger",
        ));

        // Initialisation de l'exécution
        let mut ctx = Interpreter::init_context(&graph);
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(0)));

        // Déclenchement de l'événement
        let executed_count = Interpreter::trigger_event(&graph, &mut ctx, "OnPlayerOverlap")
            .expect("Exécution sans erreur");

        // 3 nœuds exécutés (Event -> Score -> Destroy)
        assert_eq!(executed_count, 3);

        // Vérification de la variable incrémentée
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(1)));

        // Vérification des logs de traçabilité
        assert!(ctx.execution_log.len() >= 3);
        println!("Trace d'exécution :");
        for step in &ctx.execution_log {
            println!("  [{}] {} : {}", step.timestamp_us, step.node_name, step.message);
        }
    }
}
