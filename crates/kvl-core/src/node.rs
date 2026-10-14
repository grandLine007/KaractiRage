use crate::category::Category;
use crate::pin::Pin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Événement déclencheur (ex: Contact joueur, Touche pressée, Début de niveau)
    Event { event_type: String },
    /// Condition booléenne (ex: Vérifier score, Possède clé, Objet actif)
    Condition { operator: String },
    /// Action unitaire (ex: Modifier score, Jouer son, Supprimer objet)
    Action { action_type: String },
    /// Bloc intelligent / Composé (regroupe une sous-logique avec vue simplifiée)
    Composite {
        macro_name: String,
        is_expanded: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeUiMetadata {
    pub position_x: f32,
    pub position_y: f32,
    pub color_tag: Option<String>,
}

impl Default for NodeUiMetadata {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            color_tag: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub category: Category,
    /// Explication claire en langage naturel du rôle de ce bloc (pédagogie & audit IA)
    pub description: String,
    pub kind: NodeKind,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    pub ui: NodeUiMetadata,
}

impl Node {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: Category,
        description: impl Into<String>,
        kind: NodeKind,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category,
            description: description.into(),
            kind,
            inputs: Vec::new(),
            outputs: Vec::new(),
            ui: NodeUiMetadata::default(),
        }
    }

    pub fn with_input(mut self, pin: Pin) -> Self {
        self.inputs.push(pin);
        self
    }

    pub fn with_output(mut self, pin: Pin) -> Self {
        self.outputs.push(pin);
        self
    }
}
