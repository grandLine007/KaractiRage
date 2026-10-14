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

    #[test]
    fn test_condition_branching_true_and_false() {
        let mut graph = Graph::new("test_cond", "Test Condition", "Branchement conditionnel KVL");
        graph.add_variable("score", Value::Int(0));
        graph.add_variable("door_is_open", Value::Bool(false));
        graph.add_variable("access_denied", Value::Bool(false));

        let evt = Node::new("evt_interact", "Event Interact", Category::Interactions, "Event", NodeKind::Event { event_type: "OnDoorInteract".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let cond = Node::new("cond_score", "Condition Score >= 50", Category::MathLogic, "Condition", NodeKind::Condition { operator: "ScoreAtLeast50".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));

        let act_unlock = Node::new("act_open", "Ouvrir Porte", Category::World, "Action", NodeKind::Action { action_type: "ToggleDoor".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        let act_deny = Node::new("act_deny", "Refuser Accès", Category::World, "Action", NodeKind::Action { action_type: "DenyAccess".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(cond);
        graph.add_node(act_unlock);
        graph.add_node(act_deny);

        graph.connect_pins("evt_interact", "out_trigger", "cond_score", "in_trigger").unwrap();
        graph.connect_pins("cond_score", "out_true", "act_open", "in_trigger").unwrap();
        graph.connect_pins("cond_score", "out_false", "act_deny", "in_trigger").unwrap();

        // CAS 1 : Score = 20 (< 50) -> Branche FAUX activée
        let mut ctx = Interpreter::init_context(&graph);
        ctx.set_var("score", Value::Int(20));

        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnDoorInteract").unwrap();
        assert_eq!(steps, 3); // Event -> Condition -> DenyAccess
        assert_eq!(ctx.get_var("access_denied"), Some(&Value::Bool(true)), "Accès doit être refusé");
        assert_eq!(ctx.get_var("door_is_open"), Some(&Value::Bool(false)), "Porte doit rester fermée");

        // CAS 2 : Score = 65 (>= 50) -> Branche VRAI activée
        ctx.set_var("score", Value::Int(65));
        ctx.set_var("access_denied", Value::Bool(false));

        let steps2 = Interpreter::trigger_event(&graph, &mut ctx, "OnDoorInteract").unwrap();
        assert_eq!(steps2, 3); // Event -> Condition -> ToggleDoor
        assert_eq!(ctx.get_var("door_is_open"), Some(&Value::Bool(true)), "Porte doit être ouverte");
        assert_eq!(ctx.get_var("access_denied"), Some(&Value::Bool(false)), "Pas de refus d'accès");
    }

    #[test]
    fn test_teleport_and_speed_alteration_actions() {
        let mut graph = Graph::new("test_teleport_speed", "Test Teleport & Speed", "");
        let evt = Node::new("evt", "Event", Category::Interactions, "", NodeKind::Event { event_type: "OnPortal".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_teleport = Node::new("act_tp", "Teleport", Category::Movement, "", NodeKind::Action { action_type: "TeleportTo:350,220".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_speed = Node::new("act_spd", "Slow", Category::Movement, "", NodeKind::Action { action_type: "ModifySpeed:-50".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_teleport);
        graph.add_node(act_speed);

        graph.connect_pins("evt", "out_trigger", "act_tp", "in_trigger").unwrap();
        graph.connect_pins("act_tp", "out_trigger", "act_spd", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        ctx.set_var("speed_boost", Value::Int(100));

        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnPortal").unwrap();
        assert_eq!(steps, 3);
        assert_eq!(ctx.get_var("teleport_target_x"), Some(&Value::Int(350)));
        assert_eq!(ctx.get_var("teleport_target_y"), Some(&Value::Int(220)));
        assert_eq!(ctx.get_var("speed_boost"), Some(&Value::Int(50))); // 100 - 50 = 50
    }

    #[test]
    fn test_counter_gate_and_variable_conditions() {
        let mut graph = Graph::new("test_counter", "Test Counter & Var", "");
        let evt = Node::new("evt", "Key Collect", Category::Interactions, "", NodeKind::Event { event_type: "OnKeyFound".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_inc = Node::new("act_inc", "Inc Keys", Category::Progression, "", NodeKind::Action { action_type: "IncrementCounter:keys".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let cond_keys = Node::new("cond_keys", "Condition Keys >= 3", Category::MathLogic, "", NodeKind::Condition { operator: "CounterAtLeast:keys:3".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
        let act_win = Node::new("act_win", "Unlock Safe", Category::Progression, "", NodeKind::Action { action_type: "SetVar:vault_open:1".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_inc);
        graph.add_node(cond_keys);
        graph.add_node(act_win);

        graph.connect_pins("evt", "out_trigger", "act_inc", "in_trigger").unwrap();
        graph.connect_pins("act_inc", "out_trigger", "cond_keys", "in_trigger").unwrap();
        graph.connect_pins("cond_keys", "out_true", "act_win", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        ctx.set_var("keys", Value::Int(0));

        // Clé 1
        Interpreter::trigger_event(&graph, &mut ctx, "OnKeyFound").unwrap();
        assert_eq!(ctx.get_var("keys"), Some(&Value::Int(1)));
        assert!(ctx.get_var("vault_open").is_none());

        // Clé 2
        Interpreter::trigger_event(&graph, &mut ctx, "OnKeyFound").unwrap();
        assert_eq!(ctx.get_var("keys"), Some(&Value::Int(2)));
        assert!(ctx.get_var("vault_open").is_none());

        // Clé 3 -> Déverrouillage coffre !
        Interpreter::trigger_event(&graph, &mut ctx, "OnKeyFound").unwrap();
        assert_eq!(ctx.get_var("keys"), Some(&Value::Int(3)));
        assert_eq!(ctx.get_var("vault_open"), Some(&Value::Int(1)));
    }
}
