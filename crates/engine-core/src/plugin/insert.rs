//! Insertion d'un nœud du catalogue dans un graphe KVL.
//!
//! Séparé de l'API native pour être testable sans toucher au disque : l'API native
//! appelle `insert_schema_node`, puis persiste le graphe.

use kvl_core::{Graph, Node, NodeFlavor, NodeKind, NodeSchema, Pin, PinDirection};
use thiserror::Error;

/// Erreurs d'insertion d'un nœud.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InsertError {
    #[error("les blocs composés ne sont pas encore insérables dans un graphe")]
    UnsupportedComposite,
}

/// Insère un nœud décrit par `schema` et renvoie son identifiant.
///
/// `concrete_type` est le type effectivement écrit dans le graphe : il peut différer
/// de `schema.id` pour une forme préfixée (`SetVar:score:0`).
pub fn insert_schema_node(
    graph: &mut Graph,
    schema: &NodeSchema,
    concrete_type: &str,
    x: f32,
    y: f32,
) -> Result<String, InsertError> {
    let kind = match schema.flavor {
        NodeFlavor::Action => NodeKind::Action {
            action_type: concrete_type.to_string(),
        },
        NodeFlavor::Condition => NodeKind::Condition {
            operator: concrete_type.to_string(),
        },
        NodeFlavor::Event => NodeKind::Event {
            event_type: concrete_type.to_string(),
        },
        NodeFlavor::Composite => return Err(InsertError::UnsupportedComposite),
    };

    let node_id = next_node_id(graph, &schema.id);
    let mut node = Node::new(
        node_id.clone(),
        schema.label.clone(),
        schema.category.clone(),
        schema.description.clone(),
        kind,
    );

    if schema.flow_input {
        node = node.with_input(Pin::new_trigger(
            "in_trigger",
            "Exécution",
            PinDirection::Input,
        ));
    }

    for output in schema.effective_flow_outputs() {
        let label = match output.as_str() {
            "out_true" => "Vrai",
            "out_false" => "Faux",
            _ => "Exécution",
        };
        node = node.with_output(Pin::new_trigger(output, label, PinDirection::Output));
    }

    node.ui.position_x = x;
    node.ui.position_y = y;
    graph.add_node(node);

    Ok(node_id)
}

fn next_node_id(graph: &Graph, base: &str) -> String {
    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    let mut index = 1u32;
    loop {
        let candidate = format!("node_{}_{}", sanitized, index);
        if !graph.nodes.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvl_core::Category;

    fn schema(id: &str, flavor: NodeFlavor) -> NodeSchema {
        let mut schema = NodeSchema::action(id, "Libellé", Category::World, "Description");
        schema.flavor = flavor;
        if flavor == NodeFlavor::Event {
            schema.flow_input = false;
        }
        schema
    }

    #[test]
    fn inserts_action_with_flow_pins_and_position() {
        let mut graph = Graph::new("g", "G", "");
        let inserted = insert_schema_node(&mut graph, &schema("SetVar", NodeFlavor::Action), "SetVar:score:0", 12.0, 34.0)
            .expect("insertion");

        let node = graph.nodes.get(&inserted).expect("nœud présent");
        assert!(matches!(
            &node.kind,
            NodeKind::Action { action_type } if action_type == "SetVar:score:0"
        ));
        assert_eq!(node.inputs.len(), 1);
        assert_eq!(node.outputs.len(), 1);
        assert_eq!(node.ui.position_x, 12.0);
        assert_eq!(node.ui.position_y, 34.0);
    }

    #[test]
    fn events_have_no_flow_input_and_ids_are_unique() {
        let mut graph = Graph::new("g", "G", "");
        let first = insert_schema_node(
            &mut graph,
            &schema("Start", NodeFlavor::Event),
            "Start",
            0.0,
            0.0,
        )
        .expect("insertion");
        let second = insert_schema_node(
            &mut graph,
            &schema("Start", NodeFlavor::Event),
            "Start",
            0.0,
            0.0,
        )
        .expect("insertion");

        assert_ne!(first, second);
        assert!(graph.nodes[&first].inputs.is_empty());
        assert_eq!(graph.nodes[&first].outputs.len(), 1);
    }

    #[test]
    fn conditions_expose_true_and_false_outputs() {
        let mut graph = Graph::new("g", "G", "");
        let mut condition = schema("IsAlive", NodeFlavor::Condition);
        condition.flow_outputs = vec!["out_true".into(), "out_false".into()];

        let id = insert_schema_node(&mut graph, &condition, "IsAlive", 0.0, 0.0)
            .expect("insertion");
        let outputs: Vec<&str> = graph.nodes[&id].outputs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(outputs, vec!["out_true", "out_false"]);
    }

    #[test]
    fn composites_are_rejected() {
        let mut graph = Graph::new("g", "G", "");
        let error = insert_schema_node(
            &mut graph,
            &schema("Macro", NodeFlavor::Composite),
            "Macro",
            0.0,
            0.0,
        )
        .expect_err("composite refusé");
        assert_eq!(error, InsertError::UnsupportedComposite);
    }
}
