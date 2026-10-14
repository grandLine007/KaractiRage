use crate::category::Category;
use crate::connection::Connection;
use crate::graph::Graph;
use crate::node::{Node, NodeKind};
use crate::pin::{Pin, PinDirection};
use crate::types::Value;
use serde::{Deserialize, Serialize};

/// Mode Guidé KVL : Définition des intentions de haut niveau
/// L'utilisateur (ou l'agent IA) répond à quelques questions simples pour générer un graphe complet.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectibleIntent {
    pub item_name: String,
    pub score_points: i64,
    pub destroy_on_pickup: bool,
    pub play_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DoorTriggerType {
    OnKeyPress(String),
    OnProximity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DoorConditionType {
    None,
    ScoreRequired(i64),
    VariableEquals(String, Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorIntent {
    pub door_name: String,
    pub trigger: DoorTriggerType,
    pub condition: DoorConditionType,
    pub toggle: bool,
}

pub struct GuidedBuilder;

impl GuidedBuilder {
    /// Construit automatiquement un graphe KVL valide pour un objet à ramasser
    pub fn build_collectible(intent: &CollectibleIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_collectible_{}", intent.item_name.to_lowercase().replace(' ', "_")),
            format!("Collecte : {}", intent.item_name),
            format!(
                "Mécanique guidée : ajoute {} points et retire l'objet lors du contact.",
                intent.score_points
            ),
        );

        // Variable de score
        graph.add_variable("score", Value::Int(0));

        // 1. Événement
        let evt_id = "evt_overlap";
        let evt_node = Node::new(
            evt_id,
            format!("Quand le joueur touche {}", intent.item_name),
            Category::Interactions,
            "Détecte la collision entre le joueur et l'objet.",
            NodeKind::Event {
                event_type: "OnPlayerOverlap".into(),
            },
        )
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        // 2. Action Score
        let score_id = "act_add_score";
        let score_node = Node::new(
            score_id,
            format!("Ajouter {} au score", intent.score_points),
            Category::Progression,
            format!("Incrémente la variable de score de {}.", intent.score_points),
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt_node);
        graph.add_node(score_node);

        graph.add_connection(Connection::new(
            "conn_evt_score",
            evt_id,
            "out_trigger",
            score_id,
            "in_trigger",
        ));

        let mut last_action_id = score_id.to_string();

        // 3. Action Destruction optionnelle
        if intent.destroy_on_pickup {
            let destroy_id = "act_destroy";
            let destroy_node = Node::new(
                destroy_id,
                format!("Faire disparaître {}", intent.item_name),
                Category::World,
                "Supprime l'objet du monde de jeu.",
                NodeKind::Action {
                    action_type: "DestroySelf".into(),
                },
            )
            .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

            graph.add_node(destroy_node);
            graph.add_connection(Connection::new(
                "conn_score_destroy",
                &last_action_id,
                "out_trigger",
                destroy_id,
                "in_trigger",
            ));
            last_action_id = destroy_id.to_string();
        }

        // 4. Action Son optionnelle
        if let Some(sound_name) = &intent.play_sound {
            let sound_id = "act_play_sound";
            let sound_node = Node::new(
                sound_id,
                format!("Jouer le son '{}'", sound_name),
                Category::Audio,
                "Déclenche l'effet sonore audio.",
                NodeKind::Action {
                    action_type: "PlaySound".into(),
                },
            )
            .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

            graph.add_node(sound_node);
            graph.add_connection(Connection::new(
                "conn_sound",
                &last_action_id,
                "out_trigger",
                sound_id,
                "in_trigger",
            ));
        }

        graph
    }

    /// Construit automatiquement un graphe KVL valide pour une porte interactive
    pub fn build_door(intent: &DoorIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_door_{}", intent.door_name.to_lowercase().replace(' ', "_")),
            format!("Logique : {}", intent.door_name),
            format!(
                "Mécanique guidée : interaction porte avec vérification des conditions."
            ),
        );

        graph.add_variable("door_is_open", Value::Bool(false));

        let evt_id = "evt_door_interact";
        let evt_node = Node::new(
            evt_id,
            format!("Interaction avec {}", intent.door_name),
            Category::Interactions,
            "Déclenché lors de l'appui sur la touche d'action près de la porte.",
            NodeKind::Event {
                event_type: "OnDoorInteract".into(),
            },
        )
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let action_id = "act_toggle_door";
        let action_node = Node::new(
            action_id,
            "Basculer état de la porte",
            Category::World,
            "Inverse l'état ouvert/fermé de la porte.",
            NodeKind::Action {
                action_type: "ToggleDoor".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

        graph.add_node(evt_node);
        graph.add_node(action_node);

        graph.add_connection(Connection::new(
            "conn_door",
            evt_id,
            "out_trigger",
            action_id,
            "in_trigger",
        ));

        graph
    }
}
