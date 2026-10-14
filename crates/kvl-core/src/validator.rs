use crate::graph::Graph;
use crate::pin::PinDirection;
use crate::types::DataType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: ValidationSeverity,
    pub message: String,
    pub node_id: Option<String>,
    pub pin_id: Option<String>,
}

pub struct Validator;

impl Validator {
    pub fn validate(graph: &Graph) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // 1. Vérifier si le graphe contient au moins un point d'entrée (Événement)
        let has_event = graph
            .nodes
            .values()
            .any(|n| matches!(n.kind, crate::node::NodeKind::Event { .. }));

        if !has_event && !graph.nodes.is_empty() {
            diagnostics.push(Diagnostic {
                severity: ValidationSeverity::Warning,
                message: "Le graphe ne contient aucun nœud Événement déclencheur.".to_string(),
                node_id: None,
                pin_id: None,
            });
        }

        // 2. Vérifier chaque connexion
        for conn in &graph.connections {
            let from_node = match graph.nodes.get(&conn.from_node) {
                Some(n) => n,
                None => {
                    diagnostics.push(Diagnostic {
                        severity: ValidationSeverity::Error,
                        message: format!("Connexion orpheline : le nœud source '{}' n'existe pas.", conn.from_node),
                        node_id: Some(conn.from_node.clone()),
                        pin_id: None,
                    });
                    continue;
                }
            };

            let to_node = match graph.nodes.get(&conn.to_node) {
                Some(n) => n,
                None => {
                    diagnostics.push(Diagnostic {
                        severity: ValidationSeverity::Error,
                        message: format!("Connexion orpheline : le nœud cible '{}' n'existe pas.", conn.to_node),
                        node_id: Some(conn.to_node.clone()),
                        pin_id: None,
                    });
                    continue;
                }
            };

            let from_pin = match from_node.outputs.iter().find(|p| p.id == conn.from_pin) {
                Some(p) => p,
                None => {
                    diagnostics.push(Diagnostic {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Le pin de sortie '{}' n'existe pas sur le nœud '{}'.",
                            conn.from_pin, from_node.name
                        ),
                        node_id: Some(conn.from_node.clone()),
                        pin_id: Some(conn.from_pin.clone()),
                    });
                    continue;
                }
            };

            let to_pin = match to_node.inputs.iter().find(|p| p.id == conn.to_pin) {
                Some(p) => p,
                None => {
                    diagnostics.push(Diagnostic {
                        severity: ValidationSeverity::Error,
                        message: format!(
                            "Le pin d'entrée '{}' n'existe pas sur le nœud '{}'.",
                            conn.to_pin, to_node.name
                        ),
                        node_id: Some(conn.to_node.clone()),
                        pin_id: Some(conn.to_pin.clone()),
                    });
                    continue;
                }
            };

            // Vérification de la compatibilité des directions
            if from_pin.direction != PinDirection::Output || to_pin.direction != PinDirection::Input {
                diagnostics.push(Diagnostic {
                    severity: ValidationSeverity::Error,
                    message: "Une connexion doit obligatoirement relier une sortie (Output) vers une entrée (Input).".to_string(),
                    node_id: Some(to_node.id.clone()),
                    pin_id: Some(to_pin.id.clone()),
                });
            }

            // Vérification de la compatibilité des types de données
            if from_pin.data_type != to_pin.data_type
                && from_pin.data_type != DataType::Any
                && to_pin.data_type != DataType::Any
            {
                diagnostics.push(Diagnostic {
                    severity: ValidationSeverity::Error,
                    message: format!(
                        "Incompatibilité de types entre le pin source {:?} et le pin cible {:?}.",
                        from_pin.data_type, to_pin.data_type
                    ),
                    node_id: Some(to_node.id.clone()),
                    pin_id: Some(to_pin.id.clone()),
                });
            }
        }

        diagnostics
    }
}
