pub mod category;
pub mod connection;
pub mod error;
pub mod graph;
pub mod guided;
pub mod node;
pub mod pin;
pub mod types;
pub mod validator;

pub use category::Category;
pub use connection::Connection;
pub use error::KvlError;
pub use graph::{Graph, Variable};
pub use guided::{CollectibleIntent, DoorConditionType, DoorIntent, DoorTriggerType, GuidedBuilder};
pub use node::{Node, NodeKind};
pub use pin::{Pin, PinDirection};
pub use types::{DataType, Value};
pub use validator::{Diagnostic, ValidationSeverity, Validator};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_coin_pickup_graph() {
        let mut graph = Graph::new("graph_coin", "Ramassage de pièce", "Ajoute un point et détruit la pièce au contact");

        // Variable de score
        graph.add_variable("score", Value::Int(0));

        // 1. Événement : Contact joueur
        let node_event = Node::new(
            "node_overlap",
            "Au contact du joueur",
            Category::Interactions,
            "Déclenché quand le joueur touche le collider de la pièce",
            NodeKind::Event {
                event_type: "OnPlayerOverlap".into(),
            },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        // 2. Action : Ajouter 1 au score
        let node_score = Node::new(
            "node_add_score",
            "Incrémenter Score",
            Category::Progression,
            "Ajoute 1 point au score du joueur",
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        // 3. Action : Détruire la pièce
        let node_destroy = Node::new(
            "node_destroy",
            "Détruire Pièce",
            Category::World,
            "Supprime l'entité pièce du monde",
            NodeKind::Action {
                action_type: "DestroySelf".into(),
            },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

        graph.add_node(node_event);
        graph.add_node(node_score);
        graph.add_node(node_destroy);

        // Connexions
        graph.add_connection(Connection::new(
            "conn_1",
            "node_overlap",
            "out_trigger",
            "node_add_score",
            "in_trigger",
        ));

        graph.add_connection(Connection::new(
            "conn_2",
            "node_add_score",
            "out_trigger",
            "node_destroy",
            "in_trigger",
        ));

        // Validation statique
        let diagnostics = Validator::validate(&graph);
        assert_eq!(diagnostics.len(), 0, "Le graphe doit être parfaitement valide");

        // Sérialisation JSON (pour les agents IA et la sauvegarde de projet)
        let json = graph.to_json().expect("Sérialisation JSON réussie");
        assert!(json.contains("Ramassage de pièce"));

        // Désérialisation
        let reloaded = Graph::from_json(&json).expect("Désérialisation JSON réussie");
        assert_eq!(reloaded.nodes.len(), 3);
        assert_eq!(reloaded.connections.len(), 2);
    }

    #[test]
    fn test_validator_catches_invalid_connection() {
        let mut graph = Graph::new("invalid_graph", "Test invalide", "");

        let node1 = Node::new("n1", "Event", Category::Interactions, "", NodeKind::Event { event_type: "Start".into() })
            .with_output(Pin::new_data("out_data", "Val", PinDirection::Output, DataType::Int, None));

        let node2 = Node::new("n2", "Action", Category::World, "", NodeKind::Action { action_type: "Do".into() })
            .with_input(Pin::new_data("in_data", "Val", PinDirection::Input, DataType::String, None));

        graph.add_node(node1);
        graph.add_node(node2);

        // Connexion incompatible (Int vers String)
        graph.add_connection(Connection::new("c1", "n1", "out_data", "n2", "in_data"));

        let diagnostics = Validator::validate(&graph);
        assert!(diagnostics.iter().any(|d| d.severity == ValidationSeverity::Error && d.message.contains("Incompatibilité de types")));
    }

    #[test]
    fn test_guided_mode_builder() {
        let intent = CollectibleIntent {
            item_name: "Émeraude".into(),
            score_points: 50,
            destroy_on_pickup: true,
            play_sound: Some("gem_ping.wav".into()),
        };

        let graph = GuidedBuilder::build_collectible(&intent);
        let diagnostics = Validator::validate(&graph);
        assert_eq!(diagnostics.len(), 0, "Le graphe généré par le mode guidé doit être valide");

        let json = graph.to_json().unwrap();
        assert!(json.contains("Émeraude"));
        assert!(json.contains("gem_ping.wav"));
    }
}
