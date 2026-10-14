use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    Trigger, // Flow d'exécution
    Bool,
    Int,
    Float,
    String,
    Vector2,
    Vector3,
    EntityRef,
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    EntityRef(String),
}

impl Value {
    pub fn data_type(&self) -> DataType {
        match self {
            Value::Bool(_) => DataType::Bool,
            Value::Int(_) => DataType::Int,
            Value::Float(_) => DataType::Float,
            Value::String(_) => DataType::String,
            Value::Vector2(_) => DataType::Vector2,
            Value::Vector3(_) => DataType::Vector3,
            Value::EntityRef(_) => DataType::EntityRef,
        }
    }
}
