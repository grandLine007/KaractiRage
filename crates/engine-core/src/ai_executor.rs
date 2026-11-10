//! Exécuteur de plan : applique des opérations atomiques sur un **clone** du projet.
//!
//! Commit atomique : soit toutes les opérations passent et le clone est renvoyé, soit une
//! opération échoue et le projet d'origine reste intact. Une opération non supportée est une
//! **erreur explicite**, jamais un no-op silencieux.

use thiserror::Error;

use crate::ai_plan::{AiOp, AiPlan, PlanError};
use crate::entity::{Collider2D, Entity, Health2D, MaterialRef, MeshRef, Movement2D, Renderable2D, Renderable3D, Transform2D, Transform3D, VisionSensing};
use crate::subsystems::{Camera3DData, ExportTargetKind, Light3DData, UiScreen, UiScreenKind};
use glam::Vec3;
use crate::project::KaractiProject;
use crate::recipe::GameGenre;
use kvl_core::{Connection, Graph, Node, NodeFlavor, NodeKind, Pin, PinDirection, Value};

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
        AiOp::AddKvlNode { node_id, node_type } => {
            let registry = catalog_registry();
            let resolved = registry
                .resolve(node_type)
                .ok_or_else(|| format!("type de nœud inconnu du catalogue : {node_type}"))?;
            let node = build_node(node_id, node_type, resolved.schema);
            get_or_create_main_graph(project).add_node(node);
            Ok(())
        }
        AiOp::ConnectPins { from, from_pin, to, to_pin } => {
            let graph = get_or_create_main_graph(project);
            let conn_id = format!("c_{}_{}", from, graph.connections.len());
            graph.add_connection(Connection::new(conn_id, from, from_pin, to, to_pin));
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
        AiOp::GenerateSprite { archetype } => {
            let id = project.scene.entities.len();
            let mut entity = Entity::new(
                id,
                archetype.clone(),
                "sprite",
                Transform2D::new(0.0, 0.0, 32.0, 32.0),
            );
            entity.renderable = Some(Renderable2D::procedural_sprite(archetype.as_str(), 0x00A855F7, 2));
            project.scene.entities.push(entity);
            Ok(())
        }
        AiOp::SetEntityComponent { entity, component } => {
            let target = project
                .scene
                .entities
                .iter_mut()
                .find(|e| e.name == entity.as_str())
                .ok_or_else(|| format!("entité introuvable : {entity}"))?;
            match component.as_str() {
                "collider" => {
                    target.collider = Some(Collider2D::solid(32.0, 32.0));
                    Ok(())
                }
                "health" => {
                    target.health = Some(Health2D::new(5.0));
                    Ok(())
                }
                "movement" => {
                    target.movement = Some(Movement2D::new(110.0));
                    Ok(())
                }
                "vision" => {
                    target.vision = Some(VisionSensing::new(200.0));
                    Ok(())
                }
                other => Err(format!("composant inconnu : {other}")),
            }
        }
        AiOp::AddLevel { name } => {
            project.levels.push(name.clone());
            Ok(())
        }
        AiOp::AddMesh { asset } => {
            let id = project.scene.entities.len();
            let mut entity = Entity::new(id, asset.clone(), "mesh", Transform2D::new(0.0, 0.0, 1.0, 1.0));
            entity.transform3d = Some(Transform3D::from_euler_deg(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
            ));
            entity.renderable3d = Some(Renderable3D {
                mesh: MeshRef::Asset { id: asset.clone(), node: None },
                material: MaterialRef::Solid { color: 0x00FFFFFF, roughness: 0.4, metalness: 0.0 },
                cast_shadow: true,
                receive_shadow: true,
            });
            project.scene.entities.push(entity);
            Ok(())
        }
        AiOp::SetMaterial { entity, material } => {
            let target = project
                .scene
                .entities
                .iter_mut()
                .find(|e| e.name == entity.as_str())
                .ok_or_else(|| format!("entité introuvable : {entity}"))?;
            let renderable = target
                .renderable3d
                .as_mut()
                .ok_or_else(|| format!("l'entité {entity} n'a pas de rendu 3D"))?;
            renderable.material = match material.as_str() {
                "metal" => MaterialRef::Solid { color: 0x00C0C0C0, roughness: 0.2, metalness: 0.9 },
                "glow" => MaterialRef::Solid { color: 0x00FFFFFF, roughness: 0.1, metalness: 0.0 },
                other => return Err(format!("matériau inconnu : {other}")),
            };
            Ok(())
        }
        AiOp::SetCamera { position } => {
            project.camera3d = Some(Camera3DData {
                position: *position,
                target: [0.0, 0.0, 0.0],
            });
            Ok(())
        }
        AiOp::AddLight { kind } => {
            project.lights.push(Light3DData {
                kind: kind.clone(),
                position: [0.0, 0.0, 0.0],
                color: 0x00FFFFFF,
                intensity: 1.0,
            });
            Ok(())
        }
        AiOp::SetUiScreen { screen, title, elements } => {
            let kind = parse_screen_kind(screen);
            // Idempotent par type d'écran : on met à jour l'écran existant du même
            // `kind` s'il y en a un, sinon on en crée un nouveau.
            if let Some(existing) = project.ui_screens.iter_mut().find(|s| s.kind == kind) {
                if title.is_some() {
                    existing.title = title.clone();
                }
                if !elements.is_empty() {
                    existing.elements = elements.clone();
                }
            } else {
                project.ui_screens.push(UiScreen {
                    id: format!("ui_{}", project.ui_screens.len()),
                    kind,
                    title: title.clone(),
                    elements: elements.clone(),
                });
            }
            Ok(())
        }
        AiOp::SetSaveConfig { enabled } => {
            project.save_config.enabled = *enabled;
            Ok(())
        }
        AiOp::ExportTarget { format } => {
            project.export_target = Some(match format.to_lowercase().as_str() {
                "html" => ExportTargetKind::Html,
                "package" | "json" => ExportTargetKind::Package,
                "desktop" => ExportTargetKind::Desktop,
                "wasm" => ExportTargetKind::Wasm,
                other => return Err(format!("format d'export inconnu : {other}")),
            });
            Ok(())
        }
        other => Err(format!("opération non encore supportée : {other:?}")),
    }
}

fn parse_screen_kind(raw: &str) -> UiScreenKind {
    match raw.to_lowercase().as_str() {
        "menu" => UiScreenKind::Menu,
        "pause" => UiScreenKind::Pause,
        "game_over" | "gameover" => UiScreenKind::GameOver,
        "victory" => UiScreenKind::Victory,
        _ => UiScreenKind::Hud,
    }
}

fn catalog_registry() -> kvl_core::NodeRegistry {
    let mut registry = kvl_core::NodeRegistry::new();
    let _ = registry.register_all(kvl_runtime::builtin_node_schemas());
    registry
}

fn get_or_create_main_graph(project: &mut KaractiProject) -> &mut Graph {
    project.kvl_graphs.entry("main".to_string()).or_insert_with(|| {
        Graph::new("main", "Graphe principal", "Graphe généré par le plan IA")
    })
}

fn build_node(node_id: &str, node_type: &str, schema: &kvl_core::NodeSchema) -> Node {
    let kind = match schema.flavor {
        NodeFlavor::Event => NodeKind::Event {
            event_type: node_type.into(),
        },
        NodeFlavor::Action => NodeKind::Action {
            action_type: node_type.into(),
        },
        NodeFlavor::Condition => NodeKind::Condition {
            operator: node_type.into(),
        },
        NodeFlavor::Composite => NodeKind::Composite {
            macro_name: node_type.into(),
            is_expanded: false,
        },
    };
    let mut node = Node::new(
        node_id,
        node_type,
        schema.category.clone(),
        schema.description.clone(),
        kind,
    );
    if schema.flow_input {
        node = node.with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));
    }
    for output in schema.effective_flow_outputs() {
        node = node.with_output(Pin::new_trigger(output.as_str(), "Exécution", PinDirection::Output));
    }
    node
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
    use crate::subsystems::UiElement;

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
            op: AiOp::SetAudio { sound: "coin".into() },
            artifact: "audio".into(),
            depends_on: vec!["s3".into()],
        });
        let result = apply_plan(&project, &plan);
        assert!(result.is_err(), "une op non supportée doit échouer");
        assert_eq!(project.metadata.title, "Base");
    }

    #[test]
    fn unsupported_op_is_an_error_not_a_silent_noop() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        let err = apply_op(&mut project, &AiOp::SetAudio { sound: "coin".into() });
        assert!(err.is_err());
    }

    #[test]
    fn add_kvl_node_creates_graph_and_node() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        apply_op(
            &mut project,
            &AiOp::AddKvlNode { node_id: "evt".into(), node_type: "OnStart".into() },
        )
        .expect("nœud valide");
        assert!(project.kvl_graphs["main"].nodes.contains_key("evt"));
        assert!(matches!(
            project.kvl_graphs["main"].nodes["evt"].kind,
            NodeKind::Event { .. }
        ));
    }

    #[test]
    fn add_kvl_node_rejects_unknown_type() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        let err = apply_op(
            &mut project,
            &AiOp::AddKvlNode { node_id: "evt".into(), node_type: "WinGame".into() },
        );
        assert!(err.is_err(), "un type inconnu doit être rejeté");
    }

    #[test]
    fn connect_pins_adds_connection() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        apply_op(&mut project, &AiOp::AddKvlNode { node_id: "evt".into(), node_type: "OnStart".into() }).unwrap();
        apply_op(&mut project, &AiOp::AddKvlNode { node_id: "act".into(), node_type: "AddScore".into() }).unwrap();
        apply_op(
            &mut project,
            &AiOp::ConnectPins {
                from: "evt".into(),
                from_pin: "out_trigger".into(),
                to: "act".into(),
                to_pin: "in_trigger".into(),
            },
        )
        .unwrap();
        assert_eq!(project.kvl_graphs["main"].connections.len(), 1);
    }

    #[test]
    fn generate_sprite_creates_entity_with_renderable() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();
        apply_op(&mut project, &AiOp::GenerateSprite { archetype: "coin".into() }).unwrap();
        assert_eq!(project.scene.entities.len(), 1);
        assert!(project.scene.entities[0].renderable.is_some());
    }

    #[test]
    fn set_entity_component_adds_component() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();
        apply_op(&mut project, &AiOp::AddEntity { name: "Joueur".into(), kind: "player".into() }).unwrap();
        apply_op(&mut project, &AiOp::SetEntityComponent { entity: "Joueur".into(), component: "collider".into() }).unwrap();
        assert!(project.scene.entities[0].collider.is_some());
    }

    #[test]
    fn set_entity_component_unknown_entity_errors() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();
        let err = apply_op(&mut project, &AiOp::SetEntityComponent { entity: "Inconnu".into(), component: "collider".into() });
        assert!(err.is_err());
    }

    #[test]
    fn subsystem_ops_set_project_fields() {
        let mut project = KaractiProject::new_empty("Base", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();

        apply_op(&mut project, &AiOp::SetCamera { position: [1.0, 2.0, 3.0] }).unwrap();
        apply_op(&mut project, &AiOp::AddLight { kind: "point".into() }).unwrap();
        apply_op(&mut project, &AiOp::SetSaveConfig { enabled: false }).unwrap();
        apply_op(&mut project, &AiOp::ExportTarget { format: "html".into() }).unwrap();
        apply_op(&mut project, &AiOp::AddLevel { name: "niveau_2".into() }).unwrap();
        apply_op(
            &mut project,
            &AiOp::SetUiScreen {
                screen: "hud".into(),
                title: None,
                elements: vec![
                    UiElement {
                        id: "score".into(),
                        kind: "text".into(),
                        label: Some("OR".into()),
                        bind: Some("score".into()),
                    },
                    UiElement {
                        id: "hp".into(),
                        kind: "bar".into(),
                        label: Some("VITALITÉ".into()),
                        bind: Some("player_hp".into()),
                    },
                ],
            },
        )
        .unwrap();
        // Un second `SetUiScreen` de même type met à jour le premier (idempotent).
        apply_op(
            &mut project,
            &AiOp::SetUiScreen {
                screen: "hud".into(),
                title: Some("HUD".into()),
                elements: vec![],
            },
        )
        .unwrap();
        apply_op(&mut project, &AiOp::AddMesh { asset: "pillar".into() }).unwrap();

        assert_eq!(project.camera3d.as_ref().unwrap().position, [1.0, 2.0, 3.0]);
        assert_eq!(project.lights.len(), 1);
        assert!(!project.save_config.enabled);
        assert_eq!(project.export_target, Some(crate::subsystems::ExportTargetKind::Html));
        assert_eq!(project.levels, vec!["niveau_2".to_string()]);
        assert_eq!(project.ui_screens.len(), 1, "upsert par type : pas de doublon HUD");
        assert_eq!(project.ui_screens[0].kind, crate::subsystems::UiScreenKind::Hud);
        assert_eq!(project.ui_screens[0].title.as_deref(), Some("HUD"));
        assert_eq!(project.ui_screens[0].elements.len(), 2);
        assert_eq!(project.ui_screens[0].elements[0].bind.as_deref(), Some("score"));
        assert!(project.scene.entities[0].renderable3d.is_some());
    }
}
