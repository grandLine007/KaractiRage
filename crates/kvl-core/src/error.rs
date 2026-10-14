use thiserror::Error;

#[derive(Error, Debug)]
pub enum KvlError {
    #[error("Erreur de sérialisation JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Erreur de validation du graphe: {0}")]
    Validation(String),

    #[error("Nœud introuvable: {0}")]
    NodeNotFound(String),

    #[error("Variable introuvable: {0}")]
    VariableNotFound(String),
}
