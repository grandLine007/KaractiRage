use crate::types::{DataType, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    pub id: String,
    pub name: String,
    pub direction: PinDirection,
    pub data_type: DataType,
    pub default_value: Option<Value>,
}

impl Pin {
    pub fn new_trigger(id: impl Into<String>, name: impl Into<String>, direction: PinDirection) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            direction,
            data_type: DataType::Trigger,
            default_value: None,
        }
    }

    pub fn new_data(
        id: impl Into<String>,
        name: impl Into<String>,
        direction: PinDirection,
        data_type: DataType,
        default_value: Option<Value>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            direction,
            data_type,
            default_value,
        }
    }
}
