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

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
