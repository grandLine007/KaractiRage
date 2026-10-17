use crate::category::Category;
use crate::connection::Connection;
use crate::graph::Graph;
use crate::node::{Node, NodeKind};
use crate::pin::{Pin, PinDirection};
use crate::types::Value;
use serde::{Deserialize, Serialize};

/// Mode Guidé KVL : Définition des intentions de haut niveau
/// L'utilisateur (ou l'agent IA) répond à quelques questions simples pour générer un graphe complet et valide.

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
    ItemRequired(String),
    VariableEquals(String, Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorIntent {
    pub door_name: String,
    pub trigger: DoorTriggerType,
    pub condition: DoorConditionType,
    pub toggle: bool,
    pub play_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChestIntent {
    pub chest_name: String,
    pub required_key: Option<String>,
    pub reward_score: i64,
    pub reward_item: Option<String>,
    pub play_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HazardIntent {
    pub hazard_name: String,
    pub damage_amount: i64,
    pub play_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleportIntent {
    pub portal_name: String,
    pub target_x: f32,
    pub target_y: f32,
    pub play_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchPlateIntent {
    pub switch_name: String,
    pub door_var_to_open: String,
    pub play_sound: Option<String>,
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
                action_type: if intent.score_points == 10 {
                    "AddScore".into()
                } else {
                    format!("AddScore:{}", intent.score_points)
                },
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

    /// Construit automatiquement un graphe KVL valide pour une porte interactive avec conditions
    pub fn build_door(intent: &DoorIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_door_{}", intent.door_name.to_lowercase().replace(' ', "_")),
            format!("Logique : {}", intent.door_name),
            "Mécanique guidée : interaction porte avec vérification des conditions.",
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
        graph.add_node(evt_node);

        let action_type = if intent.toggle { "ToggleDoor" } else { "OpenDoor" };
        let action_id = "act_open_door";
        let action_node = Node::new(
            action_id,
            if intent.toggle { "Basculer état de la porte" } else { "Ouvrir la porte" },
            Category::World,
            "Modifie l'état de la porte dans le monde.",
            NodeKind::Action {
                action_type: action_type.into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(action_node);

        match &intent.condition {
            DoorConditionType::None => {
                graph.add_connection(Connection::new(
                    "conn_door",
                    evt_id,
                    "out_trigger",
                    action_id,
                    "in_trigger",
                ));
            }
            DoorConditionType::ScoreRequired(score) => {
                let cond_id = "cond_score_check";
                let cond_node = Node::new(
                    cond_id,
                    format!("Score >= {}", score),
                    Category::MathLogic,
                    format!("Vérifie si le score du joueur est d'au moins {}.", score),
                    NodeKind::Condition {
                        operator: format!("ScoreAtLeast:{}", score),
                    },
                )
                .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
                .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
                .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
                graph.add_node(cond_node);

                graph.add_connection(Connection::new("conn_evt_cond", evt_id, "out_trigger", cond_id, "in_trigger"));
                graph.add_connection(Connection::new("conn_cond_act", cond_id, "out_true", action_id, "in_trigger"));
            }
            DoorConditionType::ItemRequired(item_id) => {
                let cond_id = "cond_item_check";
                let cond_node = Node::new(
                    cond_id,
                    format!("Possède {}", item_id),
                    Category::MathLogic,
                    format!("Vérifie si le joueur possède l'objet '{}'.", item_id),
                    NodeKind::Condition {
                        operator: format!("HasItem:{}", item_id),
                    },
                )
                .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
                .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
                .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
                graph.add_node(cond_node);

                graph.add_connection(Connection::new("conn_evt_cond", evt_id, "out_trigger", cond_id, "in_trigger"));
                graph.add_connection(Connection::new("conn_cond_act", cond_id, "out_true", action_id, "in_trigger"));
            }
            DoorConditionType::VariableEquals(var, val) => {
                let cond_id = "cond_var_check";
                let val_str = match val {
                    Value::Bool(b) => b.to_string(),
                    Value::Int(i) => i.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::String(s) => s.clone(),
                    _ => format!("{:?}", val),
                };
                let cond_node = Node::new(
                    cond_id,
                    format!("{} == {}", var, val_str),
                    Category::MathLogic,
                    format!("Vérifie si la variable {} est égale à {}.", var, val_str),
                    NodeKind::Condition {
                        operator: format!("VariableEquals:{}:{}", var, val_str),
                    },
                )
                .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
                .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
                .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
                graph.add_node(cond_node);

                graph.add_connection(Connection::new("conn_evt_cond", evt_id, "out_trigger", cond_id, "in_trigger"));
                graph.add_connection(Connection::new("conn_cond_act", cond_id, "out_true", action_id, "in_trigger"));
            }
        }

        if let Some(snd) = &intent.play_sound {
            let snd_id = "act_door_sound";
            let snd_node = Node::new(
                snd_id,
                format!("Son : {}", snd),
                Category::Audio,
                "Déclenche le son de la porte.",
                NodeKind::Action { action_type: "PlaySound".into() },
            ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));
            graph.add_node(snd_node);
            graph.add_connection(Connection::new("conn_act_snd", action_id, "out_trigger", snd_id, "in_trigger"));
        }

        graph
    }

    /// Construit automatiquement un graphe KVL valide pour un coffre au trésor (avec clé optionnelle)
    pub fn build_chest(intent: &ChestIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_chest_{}", intent.chest_name.to_lowercase().replace(' ', "_")),
            format!("Coffre : {}", intent.chest_name),
            "Mécanique guidée : ouverture de coffre, récompense et sons.",
        );

        graph.add_variable("chest_opened", Value::Bool(false));

        let evt_id = "evt_chest_interact";
        let evt_node = Node::new(
            evt_id,
            format!("Ouvrir {}", intent.chest_name),
            Category::Interactions,
            "Déclenché lors de l'interaction du joueur avec le coffre.",
            NodeKind::Event { event_type: "OnDoorInteract".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(evt_node);

        // Chaîne d'actions au succès
        let mut first_act_id = String::new();
        let mut last_act_id = String::new();

        if intent.reward_score > 0 {
            let score_id = "act_chest_score";
            let score_node = Node::new(
                score_id,
                format!("Gain +{} Or", intent.reward_score),
                Category::Progression,
                "Ajoute le trésor au score.",
                NodeKind::Action { action_type: format!("AddScore:{}", intent.reward_score) },
            )
            .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
            graph.add_node(score_node);
            first_act_id = score_id.to_string();
            last_act_id = score_id.to_string();
        }

        if let Some(item_id) = &intent.reward_item {
            let item_act_id = "act_chest_give_item";
            let item_node = Node::new(
                item_act_id,
                format!("Donner {}", item_id),
                Category::Progression,
                "Ajoute l'objet à l'inventaire du joueur.",
                NodeKind::Action { action_type: format!("GiveItem:{}:1", item_id) },
            )
            .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
            graph.add_node(item_node);

            if first_act_id.is_empty() {
                first_act_id = item_act_id.to_string();
            } else {
                graph.add_connection(Connection::new("conn_score_item", &last_act_id, "out_trigger", item_act_id, "in_trigger"));
            }
            last_act_id = item_act_id.to_string();
        }

        let sound_act_id = "act_chest_sound";
        let sound_node = Node::new(
            sound_act_id,
            "Son de récompense",
            Category::Audio,
            "Joue le son d'obtention de trésor.",
            NodeKind::Action { action_type: "PlaySound".into() },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(sound_node);

        if first_act_id.is_empty() {
            first_act_id = sound_act_id.to_string();
        } else {
            graph.add_connection(Connection::new("conn_act_snd", &last_act_id, "out_trigger", sound_act_id, "in_trigger"));
        }
        last_act_id = sound_act_id.to_string();

        let open_act_id = "act_chest_open_state";
        let open_node = Node::new(
            open_act_id,
            "Marquer coffre ouvert",
            Category::World,
            "Enregistre l'état ouvert du coffre.",
            NodeKind::Action { action_type: "SetVariable:chest_opened:true".into() },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));
        graph.add_node(open_node);
        graph.add_connection(Connection::new("conn_snd_open", &last_act_id, "out_trigger", open_act_id, "in_trigger"));

        // Branchement avec ou sans clé requise
        if let Some(key_id) = &intent.required_key {
            let cond_id = "cond_key_check";
            let cond_node = Node::new(
                cond_id,
                format!("Clé '{}' requise", key_id),
                Category::MathLogic,
                "Vérifie la possession de la clé.",
                NodeKind::Condition { operator: format!("HasItem:{}", key_id) },
            )
            .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
            graph.add_node(cond_node);

            graph.add_connection(Connection::new("conn_evt_key", evt_id, "out_trigger", cond_id, "in_trigger"));
            graph.add_connection(Connection::new("conn_key_success", cond_id, "out_true", &first_act_id, "in_trigger"));
        } else {
            graph.add_connection(Connection::new("conn_evt_act", evt_id, "out_trigger", &first_act_id, "in_trigger"));
        }

        graph
    }

    /// Construit automatiquement un graphe KVL valide pour un piège ou une zone de danger
    pub fn build_hazard(intent: &HazardIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_hazard_{}", intent.hazard_name.to_lowercase().replace(' ', "_")),
            format!("Piège : {}", intent.hazard_name),
            "Mécanique guidée : dégâts au contact, son et flash.",
        );

        let evt_id = "evt_hazard_overlap";
        let evt_node = Node::new(
            evt_id,
            format!("Contact avec {}", intent.hazard_name),
            Category::Interactions,
            "Déclenché quand le joueur entre dans la zone du piège.",
            NodeKind::Event { event_type: "OnPlayerOverlap".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(evt_node);

        let dmg_id = "act_hazard_damage";
        let dmg_node = Node::new(
            dmg_id,
            format!("Infliger {} Dégâts", intent.damage_amount),
            Category::Progression,
            "Réduit les points de vie du joueur.",
            NodeKind::Action { action_type: format!("DamagePlayer:{}", intent.damage_amount) },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(dmg_node);

        let snd_id = "act_hazard_sound";
        let snd_node = Node::new(
            snd_id,
            "Son d'impact",
            Category::Audio,
            "Joue le son de blessure du joueur.",
            NodeKind::Action { action_type: "PlaySound".into() },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));
        graph.add_node(snd_node);

        graph.add_connection(Connection::new("conn_haz_dmg", evt_id, "out_trigger", dmg_id, "in_trigger"));
        graph.add_connection(Connection::new("conn_dmg_snd", dmg_id, "out_trigger", snd_id, "in_trigger"));

        graph
    }

    /// Construit automatiquement un graphe KVL valide pour un portail de téléportation
    pub fn build_teleport(intent: &TeleportIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_teleport_{}", intent.portal_name.to_lowercase().replace(' ', "_")),
            format!("Téléporteur : {}", intent.portal_name),
            "Mécanique guidée : transfert instantané du joueur vers une destination.",
        );

        let evt_id = "evt_teleport_overlap";
        let evt_node = Node::new(
            evt_id,
            "Entrée dans le portail",
            Category::Interactions,
            "Déclenché lors du passage du joueur dans le vortex.",
            NodeKind::Event { event_type: "OnPlayerOverlap".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(evt_node);

        let tp_id = "act_teleport";
        let tp_node = Node::new(
            tp_id,
            format!("Téléporter vers ({:.0}, {:.0})", intent.target_x, intent.target_y),
            Category::World,
            "Repositionne le joueur aux coordonnées cibles.",
            NodeKind::Action { action_type: format!("TeleportPlayer:{:.0}:{:.0}", intent.target_x, intent.target_y) },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(tp_node);

        let snd_id = "act_teleport_sound";
        let snd_node = Node::new(
            snd_id,
            "Son de portail",
            Category::Audio,
            "Effet sonore de téléportation spatio-temporelle.",
            NodeKind::Action { action_type: "PlaySound".into() },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));
        graph.add_node(snd_node);

        graph.add_connection(Connection::new("conn_tp_act", evt_id, "out_trigger", tp_id, "in_trigger"));
        graph.add_connection(Connection::new("conn_tp_snd", tp_id, "out_trigger", snd_id, "in_trigger"));

        graph
    }

    /// Construit automatiquement un graphe KVL valide pour une dalle d'énigme / interrupteur
    pub fn build_switch_plate(intent: &SwitchPlateIntent) -> Graph {
        let mut graph = Graph::new(
            format!("graph_switch_{}", intent.switch_name.to_lowercase().replace(' ', "_")),
            format!("Dalle : {}", intent.switch_name),
            "Mécanique guidée : activation par pression de bloc ou du joueur.",
        );

        let evt_id = "evt_plate_press";
        let evt_node = Node::new(
            evt_id,
            format!("Pression sur {}", intent.switch_name),
            Category::Interactions,
            "Déclenché quand un poids est placé sur la dalle.",
            NodeKind::Event { event_type: "OnPressurePlate".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(evt_node);

        let open_act_id = "act_switch_open";
        let open_node = Node::new(
            open_act_id,
            format!("Activer {}", intent.door_var_to_open),
            Category::World,
            "Déverrouille le passage ou le mécanisme relié.",
            NodeKind::Action { action_type: format!("SetVariable:{}:true", intent.door_var_to_open) },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
        graph.add_node(open_node);

        let snd_id = "act_switch_sound";
        let snd_node = Node::new(
            snd_id,
            "Son d'interrupteur mécanique",
            Category::Audio,
            "Clic métallique et grondement de porte.",
            NodeKind::Action { action_type: "PlaySound".into() },
        )
        .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));
        graph.add_node(snd_node);

        graph.add_connection(Connection::new("conn_sw_act", evt_id, "out_trigger", open_act_id, "in_trigger"));
        graph.add_connection(Connection::new("conn_act_snd", open_act_id, "out_trigger", snd_id, "in_trigger"));

        graph
    }
}
