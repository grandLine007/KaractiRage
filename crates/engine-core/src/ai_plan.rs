//! Plan multi-étapes et opérations atomiques (le moteur exécute, le LLM propose).
//!
//! Un plan est **petit** : une suite d'opérations (`AiOp`) avec dépendances. Chaque op a un
//! schéma, une validation et sera appliquée sur un clone du projet.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ai_dictionary::hash_text;
use crate::ai_intent::{IntentEnvelope, IntentTarget};

/// Opération atomique appliquée sur un clone du projet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum AiOp {
    CreateProject { title: String, genre: String },
    CreateScene { name: String },
    AddEntity { name: String, kind: String },
    SetEntityComponent { entity: String, component: String },
    AddKvlNode { node_id: String, node_type: String },
    ConnectPins { from: String, from_pin: String, to: String, to_pin: String },
    SetVariable { name: String, value: String },
    SetObjective { text: String },
    AddLevel { name: String },
    AddMesh { asset: String },
    SetMaterial { entity: String, material: String },
    SetCamera { position: [f32; 3] },
    AddLight { kind: String },
    GenerateSprite { archetype: String },
    SetAudio { sound: String },
    SetUiScreen { screen: String },
    SetSaveConfig { enabled: bool },
    ExportTarget { format: String },
}

/// Étape d'un plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiPlanStep {
    pub id: String,
    pub op: AiOp,
    pub artifact: String,
    pub depends_on: Vec<String>,
}

/// Plan officiel : la séquence d'opérations, générée avant les artefacts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiPlan {
    pub plan_id: String,
    pub intent_id: String,
    pub steps: Vec<AiPlanStep>,
}

/// Erreurs de validation d'un plan.
#[derive(Debug, Error)]
pub enum PlanError {
    #[error("étape inconnue référencée par {step} : {missing}")]
    UnknownDependency { step: String, missing: String },
    #[error("cycle de dépendances détecté")]
    Cycle,
    #[error("identifiant d'étape dupliqué : {0}")]
    DuplicateStep(String),
}

impl AiPlan {
    /// Vérifie l'intégrité des dépendances (ids connus, pas de cycle).
    pub fn validate(&self) -> Result<(), PlanError> {
        let mut ids = std::collections::HashSet::new();
        for step in &self.steps {
            if !ids.insert(step.id.clone()) {
                return Err(PlanError::DuplicateStep(step.id.clone()));
            }
        }
        for step in &self.steps {
            for dep in &step.depends_on {
                if !ids.contains(dep) {
                    return Err(PlanError::UnknownDependency {
                        step: step.id.clone(),
                        missing: dep.clone(),
                    });
                }
            }
        }

        // Tri topologique (Kahn) pour détecter les cycles.
        let mut indegree: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut edges: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for step in &self.steps {
            indegree.entry(step.id.clone()).or_insert(0);
            for dep in &step.depends_on {
                edges.entry(dep.clone()).or_default().push(step.id.clone());
                *indegree.entry(step.id.clone()).or_insert(0) += 1;
            }
        }
        let mut queue: Vec<String> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut visited = 0usize;
        while let Some(node) = queue.pop() {
            visited += 1;
            if let Some(neighbors) = edges.get(&node) {
                for next in neighbors {
                    let degree = indegree.get_mut(next).expect("nœud connu");
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(next.clone());
                    }
                }
            }
        }
        if visited != self.steps.len() {
            return Err(PlanError::Cycle);
        }
        Ok(())
    }
}

/// Génère un plan déterministe à partir d'une intention normalisée.
pub fn plan_for(envelope: &IntentEnvelope) -> AiPlan {
    let mut steps: Vec<AiPlanStep> = Vec::new();
    let genre = envelope
        .genre
        .map(|g| g.label().to_string())
        .unwrap_or_else(|| "Platformer".to_string());
    let title = truncate(&envelope.raw_text, 40).to_string();

    match envelope.target {
        IntentTarget::FullGame => {
            add(&mut steps, "s1", AiOp::CreateProject { title, genre }, "project", &[]);
            add(&mut steps, "s2", AiOp::CreateScene { name: "niveau_1".into() }, "scene", &["s1"]);
            add(&mut steps, "s3", AiOp::AddEntity { name: "Joueur".into(), kind: "player".into() }, "entities", &["s2"]);
            add(&mut steps, "s4", AiOp::AddKvlNode { node_id: "evt_start".into(), node_type: "OnStart".into() }, "graph", &["s3"]);
            add(&mut steps, "s5", AiOp::SetObjective { text: envelope.raw_text.clone() }, "objective", &["s4"]);
            add(&mut steps, "s6", AiOp::ExportTarget { format: "html".into() }, "export", &["s5"]);
        }
        IntentTarget::Level => {
            add(&mut steps, "s1", AiOp::CreateScene { name: "niveau".into() }, "scene", &[]);
            add(&mut steps, "s2", AiOp::AddEntity { name: "Joueur".into(), kind: "player".into() }, "entities", &["s1"]);
            add(&mut steps, "s3", AiOp::AddKvlNode { node_id: "evt_start".into(), node_type: "OnStart".into() }, "graph", &["s2"]);
            add(&mut steps, "s4", AiOp::SetObjective { text: envelope.raw_text.clone() }, "objective", &["s3"]);
        }
        IntentTarget::Mechanic => {
            add(&mut steps, "s1", AiOp::AddKvlNode { node_id: "evt".into(), node_type: "OnPlayerOverlap".into() }, "graph", &[]);
            add(&mut steps, "s2", AiOp::AddKvlNode { node_id: "act".into(), node_type: "AddScore".into() }, "graph", &["s1"]);
            add(&mut steps, "s3", AiOp::ConnectPins { from: "evt".into(), from_pin: "out_trigger".into(), to: "act".into(), to_pin: "in_trigger".into() }, "graph", &["s1", "s2"]);
        }
        IntentTarget::Tweak => {
            add(&mut steps, "s1", AiOp::SetVariable { name: "tweak".into(), value: "1".into() }, "state", &[]);
        }
        _ => {
            add(&mut steps, "s1", AiOp::CreateScene { name: "scene".into() }, "scene", &[]);
        }
    }

    AiPlan {
        plan_id: format!("plan_{}", hash_text(envelope.intent_id.as_bytes())),
        intent_id: envelope.intent_id.clone(),
        steps,
    }
}

fn add(steps: &mut Vec<AiPlanStep>, id: &str, op: AiOp, artifact: &str, depends_on: &[&str]) {
    steps.push(AiPlanStep {
        id: id.to_string(),
        op,
        artifact: artifact.to_string(),
        depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
    });
}

fn truncate(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        text
    } else {
        let mut end = max_chars;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_intent::normalize_intent;

    #[test]
    fn plan_for_full_game_is_valid_and_ordered() {
        let envelope = normalize_intent("Crée un jeu complet de plateforme 2D");
        let plan = plan_for(&envelope);
        assert!(plan.validate().is_ok());
        assert_eq!(plan.steps.len(), 6);
        assert_eq!(plan.steps[0].artifact, "project");
        assert_eq!(plan.steps[5].artifact, "export");
    }

    #[test]
    fn plan_validate_detects_unknown_dependency() {
        let plan = AiPlan {
            plan_id: "p".into(),
            intent_id: "i".into(),
            steps: vec![AiPlanStep {
                id: "s1".into(),
                op: AiOp::CreateScene { name: "n".into() },
                artifact: "scene".into(),
                depends_on: vec!["s_missing".into()],
            }],
        };
        assert!(matches!(plan.validate(), Err(PlanError::UnknownDependency { .. })));
    }

    #[test]
    fn plan_validate_detects_cycle() {
        let plan = AiPlan {
            plan_id: "p".into(),
            intent_id: "i".into(),
            steps: vec![
                AiPlanStep {
                    id: "a".into(),
                    op: AiOp::CreateScene { name: "n".into() },
                    artifact: "scene".into(),
                    depends_on: vec!["b".into()],
                },
                AiPlanStep {
                    id: "b".into(),
                    op: AiOp::CreateScene { name: "n".into() },
                    artifact: "scene".into(),
                    depends_on: vec!["a".into()],
                },
            ],
        };
        assert!(matches!(plan.validate(), Err(PlanError::Cycle)));
    }
}

