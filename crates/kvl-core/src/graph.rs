use crate::connection::Connection;
use crate::node::Node;
use crate::types::{DataType, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub data_type: DataType,
    pub initial_value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub id: String,
    pub name: String,
    /// Résumé en langage naturel de la mécanique globale
    pub description: String,
    pub version: u32,
    pub variables: HashMap<String, Variable>,
    pub nodes: HashMap<String, Node>,
    pub connections: Vec<Connection>,
}

impl Graph {
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            version: 1,
            variables: HashMap::new(),
            nodes: HashMap::new(),
            connections: Vec::new(),
        }
    }

    pub fn add_variable(&mut self, name: impl Into<String>, initial_value: Value) {
        let name_str = name.into();
        let data_type = initial_value.data_type();
        self.variables.insert(
            name_str.clone(),
            Variable {
                name: name_str,
                data_type,
                initial_value,
            },
        );
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    pub fn remove_connection(&mut self, from_node: &str, from_pin: &str, to_node: &str, to_pin: &str) -> bool {
        let initial_len = self.connections.len();
        self.connections.retain(|c| !(c.from_node == from_node && c.from_pin == from_pin && c.to_node == to_node && c.to_pin == to_pin));
        self.connections.len() < initial_len
    }

    pub fn remove_connection_by_id(&mut self, connection_id: &str) -> bool {
        let initial_len = self.connections.len();
        self.connections.retain(|c| c.id != connection_id);
        self.connections.len() < initial_len
    }

    pub fn disconnect_pin(&mut self, node_id: &str, pin_id: &str) -> usize {
        let initial_len = self.connections.len();
        self.connections.retain(|c| !((c.from_node == node_id && c.from_pin == pin_id) || (c.to_node == node_id && c.to_pin == pin_id)));
        initial_len - self.connections.len()
    }

    pub fn is_pin_connected(&self, node_id: &str, pin_id: &str) -> bool {
        self.connections.iter().any(|c| (c.from_node == node_id && c.from_pin == pin_id) || (c.to_node == node_id && c.to_pin == pin_id))
    }

    pub fn connect_pins(&mut self, from_node: &str, from_pin: &str, to_node: &str, to_pin: &str) -> Result<String, String> {
        let src_node = self.nodes.get(from_node).ok_or_else(|| format!("Noeud source '{}' introuvable", from_node))?;
        let dst_node = self.nodes.get(to_node).ok_or_else(|| format!("Noeud cible '{}' introuvable", to_node))?;

        let src_p = src_node.outputs.iter().find(|p| p.id == from_pin)
            .ok_or_else(|| format!("Pin de sortie '{}' introuvable sur '{}'", from_pin, from_node))?;
        let dst_p = dst_node.inputs.iter().find(|p| p.id == to_pin)
            .ok_or_else(|| format!("Pin d'entree '{}' introuvable sur '{}'", to_pin, to_node))?;

        if src_p.data_type != dst_p.data_type {
            return Err(format!("Types incompatibles : {:?} vs {:?}", src_p.data_type, dst_p.data_type));
        }

        if self.connections.iter().any(|c| c.from_node == from_node && c.from_pin == from_pin && c.to_node == to_node && c.to_pin == to_pin) {
            return Err("Connexion deja existante".into());
        }

        let conn_id = format!("conn_{}_{}_{}_{}", from_node, from_pin, to_node, to_pin);
        self.add_connection(Connection::new(&conn_id, from_node, from_pin, to_node, to_pin));
        Ok(conn_id)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
