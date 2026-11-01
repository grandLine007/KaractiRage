//! Exécuteur de plan : applique des opérations atomiques sur un **clone** du projet.
//!
//! Commit atomique : soit toutes les opérations passent et le clone est renvoyé, soit une
//! opération échoue et le projet d'origine reste intact. Une opération non supportée est une
//! **erreur explicite**, jamais un no-op silencieux.

use thiserror::Error;

use crate::ai_plan::{AiOp, AiPlan, PlanError};
use crate::entity::{Entity, Transform2D};
use crate::project::KaractiProject;
use crate::recipe::GameGenre;
use kvl_core::Value;

/// Erreurs d'exécution d'un plan.
#[derive(Debug, Error)]
pub enum ExecError {
    #[error("plan invalide : {0}")]
    Plan(#[from] PlanError),
    #[error("opération {step} : {message}")]
    Op { step: String, message: String },
}

/// Applique un plan sur un clone du projet (tout ou rien).
pub fn apply_plan(project: &KaractiProject, plan: &AiPlan) -> Result<KaractiProject, ExecError> {
    plan.validate()?;

    let mut working = project.clone();
    for step in &plan.steps {
        apply_op(&mut working, &step.op)
            .map_err(|message| ExecError::Op { step: step.id.clone(), message })?;
    }
    Ok(working)
}

fn apply_op(project: &mut KaractiProject, op: &AiOp) -> Result<(), String> {
    match op {
        AiOp::CreateProject { title, genre } => {
            project.metadata.title = title.clone();
            project.metadata.genre_primary = parse_genre(genre);
            Ok(())
        }
        AiOp::CreateScene { name } => {
            project.scene.name = name.clone();
            Ok(())
        }
        AiOp::AddEntity { name, kind } => {
            let id = project.scene.entities.len();
            let entity = Entity::new(
                id,
                name.clone(),
                kind.clone(),
                Transform2D::new(0.0, 0.0, 32.0, 32.0),
            );
            project.scene.entities.push(entity);
            Ok(())
        }
        AiOp::SetObjective { text } => {
            project.scene.objective = Some(text.clone());
            Ok(())
        }
        AiOp::SetVariable { name, value } => {
            project.kvl_variables.insert(name.clone(), parse_value(value));
            Ok(())
        }
        other => Err(format!("opération non encore supportée : {other:?}")),
    }
}

fn parse_value(raw: &str) -> Value {
    if raw == "true" {
        Value::Bool(true)
    } else if raw == "false" {
        Value::Bool(false)
    } else if let Ok(number) = raw.parse::<i64>() {
        Value::Int(number)
    } else if let Ok(number) = raw.parse::<f64>() {
        Value::Float(number)
    } else {
        Value::String(raw.to_string())
    }
}

fn parse_genre(raw: &str) -> GameGenre {
    match raw.to_lowercase().as_str() {
        "plateforme" | "platformer" => GameGenre::Platformer,
        "rpg" => GameGenre::Rpg,
        "shmup" | "shoot'em up" => GameGenre::ShootEmUp,
        "puzzle" => GameGenre::Puzzle,
        "aventure" | "adventure" => GameGenre::Adventure,
        "horreur" | "horror" => GameGenre::Horror,
        _ => GameGenre::Custom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_plan::AiPlanStep;

    fn sample_plan() -> AiPlan {
        AiPlan {
            plan_id: "p".into(),
            intent_id: "i".into(),
            steps: vec![
                AiPlanStep {
                    id: "s1".into(),
                    op: AiOp::CreateProject { title: "Titre".into(), genre: "plateforme".into() },
                    artifact: "project".into(),
                    depends_on: vec![],
                },
                AiPlanStep {
                    id: "s2".into(),
                    op: AiOp::CreateScene { name: "niveau_1".into() },
                    artifact: "scene".into(),
                    depends_on: vec!["s1".into()],
                },
                AiPlanStep {
                    id: "s3".into(),
                    op: AiOp::SetObjective { text: "Objectif".into() },
                    artifact: "objective".into(),
                    depends_on: vec!["s2".into()],
                },
            ],
        }
    }

    #[test]
    fn apply_plan_applies_supported_ops_on_a_clone() {
        let project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        let result = apply_plan(&project, &sample_plan()).expect("plan valide");
        assert_eq!(result.metadata.title, "Titre");
        assert_eq!(result.scene.name, "niveau_1");
        assert_eq!(result.scene.objective.as_deref(), Some("Objectif"));
        // Le projet d'origine est intact (clone).
        assert_eq!(project.metadata.title, "Base");
    }

    #[test]
    fn apply_plan_rolls_back_on_unsupported_op() {
        let project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        let mut plan = sample_plan();
        plan.steps.push(AiPlanStep {
            id: "s4".into(),
            op: AiOp::AddMesh { asset: "pillar".into() },
            artifact: "mesh".into(),
            depends_on: vec!["s3".into()],
        });
        let result = apply_plan(&project, &plan);
        assert!(result.is_err(), "une op non supportée doit échouer");
        assert_eq!(project.metadata.title, "Base");
    }

    #[test]
    fn unsupported_op_is_an_error_not_a_silent_noop() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        let err = apply_op(&mut project, &AiOp::AddLight { kind: "point".into() });
        assert!(err.is_err());
    }
}
