use kvl_core::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ExecutionTraceStep {
    pub node_id: String,
    pub node_name: String,
    pub timestamp_us: u128,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub variables: HashMap<String, Value>,
    pub execution_log: Vec<ExecutionTraceStep>,
    pub max_steps: usize,
    pub steps_count: usize,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            execution_log: Vec::new(),
            max_steps: 1000,
            steps_count: 0,
        }
    }

    pub fn set_var(&mut self, name: impl Into<String>, value: Value) {
        self.variables.insert(name.into(), value);
    }

    pub fn get_var(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    pub fn record_step(&mut self, node_id: impl Into<String>, node_name: impl Into<String>, msg: impl Into<String>) {
        // Sur wasm32-unknown-unknown, `SystemTime::now()` panique (non implémenté) :
        // une simple pièce ramassée ferait planter le jeu exporté. La télémétrie n'a pas
        // besoin d'horodatage wall-clock pour rester utile : on garde un compteur monotone.
        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        #[cfg(target_arch = "wasm32")]
        let now: u128 = (self.steps_count as u128) * 16_667;

        self.execution_log.push(ExecutionTraceStep {
            node_id: node_id.into(),
            node_name: node_name.into(),
            timestamp_us: now,
            message: msg.into(),
        });
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}
