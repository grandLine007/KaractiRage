use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub from_node: String,
    pub from_pin: String,
    pub to_node: String,
    pub to_pin: String,
}

impl Connection {
    pub fn new(
        id: impl Into<String>,
        from_node: impl Into<String>,
        from_pin: impl Into<String>,
        to_node: impl Into<String>,
        to_pin: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            from_node: from_node.into(),
            from_pin: from_pin.into(),
            to_node: to_node.into(),
            to_pin: to_pin.into(),
        }
    }
}
