//! Édition contextuelle : contexte de sélection, cible résolue, proposition, transaction.
//!
//! Chaîne couverte par ce module : **cible → delta → validation → preview → undo**.
//!
//! Principes :
//! - **La sélection est le prompt.** `EditContext` porte la sélection/le survol/le curseur ;
//!   l'IA ne reçoit jamais la scène entière pour deviner une cible.
//! - **Le LLM propose, le moteur dispose.** Une proposition est immuable et hashée ; elle
//!   n'est appliquée qu'après validation, sur un clone, en une transaction.
//! - **Une op invalide est une erreur**, jamais un no-op silencieux : si une édition ne
//!   change rien, c'est `EditError::NoChange`.
//! - **Pas de code généré** : uniquement des `AiOp` déclaratives validées.
//!
//! L'exécution des ops vit dans [`crate::ai_executor`] ; ce module fournit les primitives
//! de mutation (patch, propriétés, composants, nœuds KVL) et la résolution de cible.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ai_dictionary::hash_text;
use crate::ai_plan::AiOp;
use crate::entity::{
    Collider2D, Entity, Health2D, MaterialRef, MeshRef, Movement2D, Renderable2D, Renderable3D,
    ShapeKind, Transform2D, Transform3D, VisionSensing,
};
use crate::project::KaractiProject;
use crate::subsystems::Camera3DData;
use glam::Vec3;
use kvl_core::{Graph, Value};

// ============================================================================
// 1. Contexte d'édition
// ============================================================================

/// Dimension du rendu édité (déclarative, portée par le projet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    TwoD,
    TwoAndHalfD,
    ThreeD,
}

impl Dimension {
    /// Lit la dimension depuis `metadata.dimension` (« 2D », « 2.5D », « 3D »).
    pub fn from_metadata(raw: Option<&str>) -> Self {
        match raw.map(|value| value.trim().to_ascii_lowercase()).as_deref() {
            Some("3d") => Self::ThreeD,
            Some("2.5d") | Some("2,5d") => Self::TwoAndHalfD,
            _ => Self::TwoD,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TwoD => "2D",
            Self::TwoAndHalfD => "2.5D",
            Self::ThreeD => "3D",
        }
    }
}

/// Panneau actif au moment de la demande : pondère les capacités proposées au prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivePanel {
    #[default]
    Viewport,
    Inspector,
    Kvl,
    ThreeD,
}

/// Contexte d'édition envoyé au prompt micro : **jamais la scène entière**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EditContext {
    pub project_id: String,
    pub scene_id: String,
    pub dimension: Dimension,
    /// Sélection courante (viewport, hiérarchie, inspecteur ou graphe KVL).
    #[serde(default)]
    pub selection: Vec<TargetRef>,
    #[serde(default)]
    pub hover: Option<TargetRef>,
    /// Position monde sous le curseur, pour les demandes « ici ».
    #[serde(default)]
    pub cursor_world: Option<[f32; 3]>,
    #[serde(default)]
    pub active_panel: ActivePanel,
    pub active_tool: String,
    #[serde(default)]
    pub camera: Option<Camera3DData>,
    #[serde(default)]
    pub recent_ops: Vec<String>,
}

impl Default for EditContext {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            scene_id: String::new(),
            dimension: Dimension::TwoD,
            selection: Vec::new(),
            hover: None,
            cursor_world: None,
            active_panel: ActivePanel::default(),
            active_tool: "select".to_string(),
            camera: None,
            recent_ops: Vec::new(),
        }
    }
}

impl EditContext {
    /// Contexte minimal dérivé du projet (sans sélection : le Studio la fournit).
    pub fn for_project(project: &KaractiProject) -> Self {
        Self {
            project_id: project.metadata.id.clone(),
            scene_id: project.scene.name.clone(),
            dimension: Dimension::from_metadata(project.metadata.dimension.as_deref()),
            ..Self::default()
        }
    }

    /// Contexte dérivé du projet avec une sélection explicite.
    pub fn with_selection(project: &KaractiProject, selection: Vec<TargetRef>) -> Self {
        Self {
            selection,
            ..Self::for_project(project)
        }
    }

    pub fn is_selection_empty(&self) -> bool {
        self.selection.is_empty()
    }
}

// ============================================================================
// 2. Cible
// ============================================================================

/// Référence à une cible, telle que fournie par le Studio ou proposée par le LLM.
///
/// Représentation JSON externe (compacte, adaptée au prompt) : `"selected"`,
/// `{"entity_id": 42}`, `{"kvl_node_id": "evt"}`, `{"area": {"x": 0.0, …}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetRef {
    /// L'élément actuellement sélectionné (résolu via `EditContext::selection`).
    Selected,
    /// L'élément survolé (résolu via `EditContext::hover`).
    Hovered,
    /// Identifiant stable d'entité.
    EntityId(usize),
    /// Nom d'entité (doit être unique, sinon la cible est ambiguë).
    EntityName(String),
    /// Nœud d'un graphe KVL.
    KvlNodeId(String),
    /// Position monde sous le curseur (pour « ici »).
    Cursor { x: f32, y: f32, z: Option<f32> },
    /// Zone rectangulaire monde (sélection multiple).
    Area { x: f32, y: f32, w: f32, h: f32 },
    /// Toutes les entités portant ce tag.
    Tag(String),
    /// Toutes les entités de ce type (`kind` ≈ `tag` dans le modèle actuel).
    AllOfKind(String),
}

impl TargetRef {
    /// Vrai si la cible ne peut être résolue qu'avec un `EditContext`.
    pub fn is_contextual(&self) -> bool {
        matches!(self, Self::Selected | Self::Hovered | Self::Cursor { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Entity,
    KvlNode,
    Scene,
}

/// Cible effectivement résolue : identifiants réels + libellé humain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolvedTarget {
    pub kind: TargetKind,
    /// Libellé lisible (« Boss_Squelette #42 », « 3 entités », « nœud evt (main) »).
    pub label: String,
    #[serde(default)]
    pub entity_ids: Vec<usize>,
    #[serde(default)]
    pub kvl_graph: Option<String>,
    #[serde(default)]
    pub kvl_node: Option<String>,
}

impl ResolvedTarget {
    fn empty_scene_label() -> Self {
        Self {
            kind: TargetKind::Scene,
            label: "scène".to_string(),
            entity_ids: Vec::new(),
            kvl_graph: None,
            kvl_node: None,
        }
    }
}

// ============================================================================
// 3. Delta typé
// ============================================================================

/// Niveau de risque d'une édition : pilote la confirmation et le preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Safe,
    Confirm,
    Destructive,
}

impl RiskLevel {
    fn rank(self) -> u8 {
        match self {
            Self::Safe => 0,
            Self::Confirm => 1,
            Self::Destructive => 2,
        }
    }

    pub fn max(self, other: Self) -> Self {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }
}

/// Valeur typée d'une propriété (édition d'un champ quelconque).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    /// Teinte `0x00RRGGBB`.
    Color(u32),
}

/// Patch partiel et typé d'une entité : seuls les champs modifiés sont présents.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EntityPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<f32>,
    /// Asset de sprite pixel-art (`ShapeKind::ImageRef`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_asset: Option<String>,
    /// Asset de maillage 3D (`MeshRef::Asset`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<MaterialPatch>,
}

impl EntityPatch {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.tag.is_none()
            && self.color.is_none()
            && self.x.is_none()
            && self.y.is_none()
            && self.w.is_none()
            && self.h.is_none()
            && self.rotation.is_none()
            && self.speed.is_none()
            && self.health.is_none()
            && self.sprite_asset.is_none()
            && self.mesh_asset.is_none()
            && self.material.is_none()
    }
}

/// Patch partiel d'un matériau 3D.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MaterialPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metalness: Option<f32>,
    /// Bascule vers un matériau asset nommé.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
}

// ============================================================================
// 4. Erreurs
// ============================================================================

#[derive(Debug, Error)]
pub enum EditError {
    #[error("aucune sélection : précisez une cible (sélection, nom, tag ou zone)")]
    EmptySelection,
    #[error("cible non résolue : {0}")]
    Unresolved(String),
    #[error("aucune entité ne correspond à {0}")]
    NoTarget(String),
    #[error("cible ambiguë : {0}")]
    Ambiguous(String),
    #[error("opération {index} : {message}")]
    Op { index: usize, message: String },
    #[error("proposition périmée : la cible a changé depuis l'aperçu")]
    Stale,
    #[error("aucun changement : la cible est déjà dans cet état")]
    NoChange,
    #[error("édition invalide : {0}")]
    Invalid(String),
}

// ============================================================================
// 5. Résolution de cible
// ============================================================================

/// Résout une cible **contextuelle** (sélection, survol, curseur) puis les cibles pures.
pub fn resolve_target(
    project: &KaractiProject,
    ctx: &EditContext,
    target: &TargetRef,
) -> Result<ResolvedTarget, EditError> {
    match target {
        TargetRef::Selected => {
            if ctx.selection.is_empty() {
                return Err(EditError::EmptySelection);
            }
            let mut resolved = Vec::new();
            for selected in &ctx.selection {
                if selected.is_contextual() {
                    return Err(EditError::Unresolved(format!(
                        "sélection récursive : {selected:?}"
                    )));
                }
                resolved.push(resolve_target(project, ctx, selected)?);
            }
            Ok(merge_targets(resolved))
        }
        TargetRef::Hovered => match &ctx.hover {
            Some(hovered) if !hovered.is_contextual() => resolve_target(project, ctx, hovered),
            Some(_) => Err(EditError::Unresolved("survol récursif".to_string())),
            None => Err(EditError::Unresolved(
                "aucun élément survolé : sélectionnez une cible".to_string(),
            )),
        },
        TargetRef::Cursor { .. } => Err(EditError::Unresolved(
            "le curseur seul n'identifie pas un élément : il précise une position pour les \
             opérations de placement"
                .to_string(),
        )),
        other => resolve_pure_target(project, other),
    }
}

/// Résout une cible sans contexte (identifiants, nom, tag, zone, kind).
pub fn resolve_pure_target(
    project: &KaractiProject,
    target: &TargetRef,
) -> Result<ResolvedTarget, EditError> {
    match target {
        TargetRef::KvlNodeId(node) => {
            let (graph_id, label) = locate_kvl_node(project, node)?;
            Ok(ResolvedTarget {
                kind: TargetKind::KvlNode,
                label,
                entity_ids: Vec::new(),
                kvl_graph: Some(graph_id),
                kvl_node: Some(node.clone()),
            })
        }
        TargetRef::Selected | TargetRef::Hovered | TargetRef::Cursor { .. } => {
            Err(EditError::Unresolved(format!(
                "cible contextuelle non résolue : {target:?}"
            )))
        }
        other => {
            let ids = resolve_entity_ids(project, other)?;
            let label = entity_ids_label(project, &ids);
            Ok(ResolvedTarget {
                kind: TargetKind::Entity,
                label,
                entity_ids: ids,
                kvl_graph: None,
                kvl_node: None,
            })
        }
    }
}

/// Résout une référence en identifiants d'entités (cible non contextuelle uniquement).
pub(crate) fn resolve_entity_ids(
    project: &KaractiProject,
    target: &TargetRef,
) -> Result<Vec<usize>, EditError> {
    let entities = &project.scene.entities;
    let ids: Vec<usize> = match target {
        TargetRef::EntityId(id) => {
            if entities.iter().any(|entity| entity.id == *id) {
                vec![*id]
            } else {
                return Err(EditError::NoTarget(format!("l'entité #{id}")));
            }
        }
        TargetRef::EntityName(name) => {
            let found: Vec<usize> = entities
                .iter()
                .filter(|entity| entity.name == *name)
                .map(|entity| entity.id)
                .collect();
            match found.len() {
                0 => return Err(EditError::NoTarget(format!("l'entité « {name} »"))),
                1 => found,
                _ => {
                    return Err(EditError::Ambiguous(format!(
                        "{} entités portent le nom « {name} » : utilisez un identifiant",
                        found.len()
                    )))
                }
            }
        }
        TargetRef::Tag(tag) => entities
            .iter()
            .filter(|entity| entity.tag == *tag)
            .map(|entity| entity.id)
            .collect(),
        TargetRef::AllOfKind(kind) => entities
            .iter()
            .filter(|entity| entity.tag == *kind)
            .map(|entity| entity.id)
            .collect(),
        TargetRef::Area { x, y, w, h } => {
            let (ax, ay, aw, ah) = (*x, *y, *w, *h);
            if !(ax.is_finite() && ay.is_finite() && aw.is_finite() && ah.is_finite()) {
                return Err(EditError::Invalid(
                    "zone non finie : coordonnées attendues en pixels".to_string(),
                ));
            }
            if aw <= 0.0 || ah <= 0.0 {
                return Err(EditError::Invalid(format!(
                    "zone vide : largeur et hauteur doivent être > 0 (reçu {aw}×{ah})"
                )));
            }
            entities
                .iter()
                .filter(|entity| {
                    rects_intersect(
                        (entity.transform.x, entity.transform.y, entity.transform.w, entity.transform.h),
                        (ax, ay, aw, ah),
                    )
                })
                .map(|entity| entity.id)
                .collect()
        }
        TargetRef::KvlNodeId(_) => {
            return Err(EditError::Unresolved(
                "un nœud KVL n'est pas une entité : utilisez une op KVL".to_string(),
            ))
        }
        TargetRef::Selected | TargetRef::Hovered | TargetRef::Cursor { .. } => {
            return Err(EditError::Unresolved(format!(
                "cible contextuelle non résolue : {target:?}"
            )))
        }
    };
    if ids.is_empty() {
        return Err(EditError::NoTarget(format!("{target:?}")));
    }
    Ok(ids)
}

/// Localise un nœud KVL et le graphe qui le contient (erreur si ambigu).
pub(crate) fn locate_kvl_node(
    project: &KaractiProject,
    node_id: &str,
) -> Result<(String, String), EditError> {
    let mut found: Vec<String> = Vec::new();
    for (graph_id, graph) in &project.kvl_graphs {
        if graph.nodes.contains_key(node_id) {
            found.push(graph_id.clone());
        }
    }
    found.sort();
    match found.len() {
        0 => Err(EditError::NoTarget(format!("le nœud KVL « {node_id} »"))),
        1 => {
            let graph = &found[0];
            Ok((graph.clone(), format!("nœud {node_id} ({graph})")))
        }
        _ => Err(EditError::Ambiguous(format!(
            "le nœud « {node_id} » existe dans {} graphes",
            found.len()
        ))),
    }
}

/// Identifiants touchés par une op (pour les rapports et la validation).
pub(crate) fn op_targets(op: &AiOp) -> Vec<&TargetRef> {
    match op {
        AiOp::UpdateEntity { target, .. }
        | AiOp::SetTransform2D { target, .. }
        | AiOp::SetTransform3D { target, .. }
        | AiOp::SetEntityProperty { target, .. }
        | AiOp::AddComponent { target, .. }
        | AiOp::RemoveComponent { target, .. }
        | AiOp::SetKvlNodeParam { target, .. }
        | AiOp::DeleteKvlNode { target } => vec![target],
        AiOp::DeleteEntity { targets } | AiOp::DuplicateEntity { targets, .. } => {
            targets.iter().collect()
        }
        AiOp::Batch { ops } => ops.iter().flat_map(op_targets).collect(),
        _ => Vec::new(),
    }
}

fn merge_targets(resolved: Vec<ResolvedTarget>) -> ResolvedTarget {
    if resolved.len() == 1 {
        return resolved.into_iter().next().expect("un élément");
    }
    let mut entity_ids: Vec<usize> = Vec::new();
    let mut kvl: Option<(String, String)> = None;
    let mut labels: Vec<String> = Vec::new();
    for target in resolved {
        for id in target.entity_ids {
            if !entity_ids.contains(&id) {
                entity_ids.push(id);
            }
        }
        if let (Some(graph), Some(node)) = (target.kvl_graph, target.kvl_node) {
            kvl = Some((graph, node));
        }
        if !target.label.is_empty() && !labels.contains(&target.label) {
            labels.push(target.label);
        }
    }
    let label = labels.iter().take(3).cloned().collect::<Vec<_>>().join(" · ");
    let label = if labels.len() > 3 {
        format!("{label} …")
    } else {
        label
    };
    match kvl {
        Some((graph, node)) => ResolvedTarget {
            kind: TargetKind::KvlNode,
            label: if label.is_empty() { format!("nœud {node} ({graph})") } else { label },
            entity_ids,
            kvl_graph: Some(graph),
            kvl_node: Some(node),
        },
        None => ResolvedTarget {
            kind: TargetKind::Entity,
            label,
            entity_ids,
            kvl_graph: None,
            kvl_node: None,
        },
    }
}

fn entity_ids_label(project: &KaractiProject, ids: &[usize]) -> String {
    if ids.len() == 1 {
        let id = ids[0];
        match project.scene.entities.iter().find(|entity| entity.id == id) {
            Some(entity) => format!("{} #{}", entity.name, entity.id),
            None => format!("entité #{id}"),
        }
    } else {
        format!("{} entités", ids.len())
    }
}

fn rects_intersect(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

// ============================================================================
// 6. Binding : contexte → ops concrètes
// ============================================================================

/// Remplace les cibles contextuelles par des identifiants concrets et déplie le multi-cible.
pub fn bind_ops(
    project: &KaractiProject,
    ctx: &EditContext,
    ops: &[AiOp],
) -> Result<Vec<AiOp>, EditError> {
    let mut bound = Vec::new();
    for op in ops {
        bound.extend(bind_op(project, ctx, op)?);
    }
    Ok(bound)
}

fn bind_op(project: &KaractiProject, ctx: &EditContext, op: &AiOp) -> Result<Vec<AiOp>, EditError> {
    let entity_ids = |target: &TargetRef| -> Result<Vec<usize>, EditError> {
        match resolve_target(project, ctx, target)? {
            ResolvedTarget { entity_ids, .. } if !entity_ids.is_empty() => Ok(entity_ids),
            other => Err(EditError::Unresolved(format!(
                "la cible « {} » n'est pas une entité",
                other.label
            ))),
        }
    };
    let node_id = |target: &TargetRef| -> Result<String, EditError> {
        match resolve_target(project, ctx, target)? {
            ResolvedTarget { kvl_node: Some(node), .. } => Ok(node),
            other => Err(EditError::Unresolved(format!(
                "la cible « {} » n'est pas un nœud KVL",
                other.label
            ))),
        }
    };

    Ok(match op {
        AiOp::UpdateEntity { target, patch } => entity_ids(target)?
            .into_iter()
            .map(|id| AiOp::UpdateEntity { target: TargetRef::EntityId(id), patch: patch.clone() })
            .collect(),
        AiOp::DeleteEntity { targets } => {
            let mut ops = Vec::new();
            for target in targets {
                for id in entity_ids(target)? {
                    ops.push(AiOp::DeleteEntity { targets: vec![TargetRef::EntityId(id)] });
                }
            }
            ops
        }
        AiOp::DuplicateEntity { targets, offset } => {
            let mut ops = Vec::new();
            for target in targets {
                for id in entity_ids(target)? {
                    ops.push(AiOp::DuplicateEntity {
                        targets: vec![TargetRef::EntityId(id)],
                        offset: *offset,
                    });
                }
            }
            ops
        }
        AiOp::SetTransform2D { target, x, y, w, h } => entity_ids(target)?
            .into_iter()
            .map(|id| AiOp::SetTransform2D {
                target: TargetRef::EntityId(id),
                x: *x,
                y: *y,
                w: *w,
                h: *h,
            })
            .collect(),
        AiOp::SetTransform3D { target, position, rotation_euler_deg, scale } => entity_ids(target)?
            .into_iter()
            .map(|id| AiOp::SetTransform3D {
                target: TargetRef::EntityId(id),
                position: *position,
                rotation_euler_deg: *rotation_euler_deg,
                scale: *scale,
            })
            .collect(),
        AiOp::SetEntityProperty { target, path, value } => entity_ids(target)?
            .into_iter()
            .map(|id| AiOp::SetEntityProperty {
                target: TargetRef::EntityId(id),
                path: path.clone(),
                value: value.clone(),
            })
            .collect(),
        AiOp::AddComponent { target, component } => entity_ids(target)?
            .into_iter()
            .map(|id| AiOp::AddComponent {
                target: TargetRef::EntityId(id),
                component: component.clone(),
            })
            .collect(),
        AiOp::RemoveComponent { target, component } => entity_ids(target)?
            .into_iter()
            .map(|id| AiOp::RemoveComponent {
                target: TargetRef::EntityId(id),
                component: component.clone(),
            })
            .collect(),
        AiOp::SetKvlNodeParam { target, param, value } => vec![AiOp::SetKvlNodeParam {
            target: TargetRef::KvlNodeId(node_id(target)?),
            param: param.clone(),
            value: value.clone(),
        }],
        AiOp::DeleteKvlNode { target } => vec![AiOp::DeleteKvlNode {
            target: TargetRef::KvlNodeId(node_id(target)?),
        }],
        AiOp::Batch { ops } => vec![AiOp::Batch { ops: bind_ops(project, ctx, ops)? }],
        other => vec![other.clone()],
    })
}

// ============================================================================
// 7. Risque
// ============================================================================

/// Niveau de risque d'une op (destructif = confirmation obligatoire).
pub fn risk_of(op: &AiOp) -> RiskLevel {
    match op {
        AiOp::DeleteEntity { .. }
        | AiOp::DeleteKvlNode { .. }
        | AiOp::RemoveConnection { .. }
        | AiOp::RemoveComponent { .. } => RiskLevel::Destructive,
        AiOp::Batch { ops } => ops
            .iter()
            .map(risk_of)
            .fold(RiskLevel::Safe, |acc, risk| acc.max(risk)),
        _ => RiskLevel::Safe,
    }
}

/// Risque global d'une liste d'ops.
pub fn risk_of_ops(ops: &[AiOp]) -> RiskLevel {
    ops.iter()
        .map(risk_of)
        .fold(RiskLevel::Safe, |acc, risk| acc.max(risk))
}

// ============================================================================
// 8. Primitives de mutation (appelées par l'exécuteur)
// ============================================================================

/// Applique un patch partiel à toutes les entités ciblées.
pub(crate) fn apply_entity_patch(
    project: &mut KaractiProject,
    target: &TargetRef,
    patch: &EntityPatch,
) -> Result<(), EditError> {
    if patch.is_empty() {
        return Err(EditError::Invalid(
            "patch vide : aucun champ à modifier (name, tag, color, x, y, w, h, rotation, \
             speed, health, sprite_asset, mesh_asset, material)"
                .to_string(),
        ));
    }
    for id in resolve_entity_ids(project, target)? {
        let entity = entity_mut(project, id)?;
        if let Some(name) = &patch.name {
            if name.trim().is_empty() {
                return Err(EditError::Invalid("le nom ne peut pas être vide".to_string()));
            }
            entity.name = name.clone();
        }
        if let Some(tag) = &patch.tag {
            entity.tag = tag.clone();
        }
        if let Some(color) = patch.color {
            renderable_mut(entity, "color")?.color = color;
        }
        if let Some(sprite_asset) = &patch.sprite_asset {
            renderable_mut(entity, "sprite_asset")?.shape =
                ShapeKind::ImageRef { asset: sprite_asset.clone() };
        }
        if patch.x.is_some() || patch.y.is_some() || patch.w.is_some() || patch.h.is_some() {
            let x = patch.x.map(|value| finite(value, "x")).transpose()?;
            let y = patch.y.map(|value| finite(value, "y")).transpose()?;
            let w = patch.w.map(|value| positive(value, "w")).transpose()?;
            let h = patch.h.map(|value| positive(value, "h")).transpose()?;
            apply_transform_2d(&mut entity.transform, x, y, w, h);
        }
        if let Some(rotation) = patch.rotation {
            entity.transform.rotation = finite(rotation, "rotation")?;
        }
        if let Some(speed) = patch.speed {
            let speed = non_negative(speed, "speed")?;
            let name = entity.name.clone();
            let movement = entity.movement.as_mut().ok_or_else(|| {
                EditError::Unresolved(format!(
                    "l'entité « {name} » n'a pas de composant movement : utilisez AddComponent"
                ))
            })?;
            movement.speed = speed;
        }
        if let Some(health) = patch.health {
            let health = positive(health, "health")?;
            let name = entity.name.clone();
            let component = entity.health.as_mut().ok_or_else(|| {
                EditError::Unresolved(format!(
                    "l'entité « {name} » n'a pas de composant health : utilisez AddComponent"
                ))
            })?;
            component.max = health;
            component.current = component.current.min(health);
        }
        if let Some(mesh_asset) = &patch.mesh_asset {
            let name = entity.name.clone();
            let renderable = entity.renderable3d.as_mut().ok_or_else(|| {
                EditError::Unresolved(format!(
                    "l'entité « {name} » n'a pas de rendu 3D : utilisez AddComponent(renderable3d)"
                ))
            })?;
            renderable.mesh = MeshRef::Asset { id: mesh_asset.clone(), node: None };
        }
        if let Some(material) = &patch.material {
            apply_material_patch(entity, material)?;
        }
    }
    Ok(())
}

/// Pose un transform 2D complet ; les positions précédentes suivent pour éviter tout lerp.
pub(crate) fn set_transform_2d(
    project: &mut KaractiProject,
    target: &TargetRef,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Result<(), EditError> {
    let x = finite(x, "x")?;
    let y = finite(y, "y")?;
    let w = positive(w, "w")?;
    let h = positive(h, "h")?;
    for id in resolve_entity_ids(project, target)? {
        let entity = entity_mut(project, id)?;
        apply_transform_2d(&mut entity.transform, Some(x), Some(y), Some(w), Some(h));
    }
    Ok(())
}

/// Pose un transform 3D complet (position, rotation Euler en degrés, échelle).
pub(crate) fn set_transform_3d(
    project: &mut KaractiProject,
    target: &TargetRef,
    position: [f32; 3],
    rotation_euler_deg: [f32; 3],
    scale: [f32; 3],
) -> Result<(), EditError> {
    for value in position.iter().chain(rotation_euler_deg.iter()).chain(scale.iter()) {
        finite(*value, "transform 3D")?;
    }
    if scale.iter().any(|component| *component <= 0.0) {
        return Err(EditError::Invalid(format!(
            "échelle 3D strictement positive attendue (reçu {scale:?})"
        )));
    }
    for id in resolve_entity_ids(project, target)? {
        let entity = entity_mut(project, id)?;
        let name = entity.name.clone();
        let transform = entity.transform3d.as_mut().ok_or_else(|| {
            EditError::Unresolved(format!(
                "l'entité « {name} » n'a pas de transform3d : utilisez AddComponent(transform3d)"
            ))
        })?;
        let position = Vec3::from_array(position);
        transform.rotation = Transform3D::from_euler_deg(
            position,
            Vec3::from_array(rotation_euler_deg),
            Vec3::from_array(scale),
        )
        .rotation;
        transform.position = position;
        transform.scale = Vec3::from_array(scale);
        transform.prev_position = Some(position);
    }
    Ok(())
}

/// Chemins de propriétés éditables par entité (allowlist stricte du prompt micro).
pub const EDITABLE_ENTITY_PATHS: &[&str] = &[
    "name",
    "tag",
    "is_active",
    "transform.x",
    "transform.y",
    "transform.w",
    "transform.h",
    "transform.rotation",
    "speed",
    "health.max",
    "health.current",
    "vision.range",
    "collider.w",
    "collider.h",
    "collider.is_solid",
    "renderable.color",
    "renderable.z_index",
    "renderable.visible",
];

/// Définit une propriété typée par chemin (`transform.x`, `renderable.color`, …).
pub(crate) fn set_entity_property(
    project: &mut KaractiProject,
    target: &TargetRef,
    path: &str,
    value: &PropertyValue,
) -> Result<(), EditError> {
    if !EDITABLE_ENTITY_PATHS.contains(&path) {
        return Err(EditError::Invalid(format!(
            "propriété inconnue « {path} » ; propriétés éditables : {}",
            EDITABLE_ENTITY_PATHS.join(", ")
        )));
    }
    for id in resolve_entity_ids(project, target)? {
        let entity = entity_mut(project, id)?;
        match path {
            "name" => entity.name = string_value(value, "name")?,
            "tag" => entity.tag = string_value(value, "tag")?,
            "is_active" => entity.is_active = bool_value(value, "is_active")?,
            "transform.x" | "transform.y" | "transform.w" | "transform.h" => {
                let number = float_value(value, path)?;
                match path {
                    "transform.x" => apply_transform_2d(&mut entity.transform, Some(finite(number, path)?), None, None, None),
                    "transform.y" => apply_transform_2d(&mut entity.transform, None, Some(finite(number, path)?), None, None),
                    "transform.w" => apply_transform_2d(&mut entity.transform, None, None, Some(positive(number, path)?), None),
                    _ => apply_transform_2d(&mut entity.transform, None, None, None, Some(positive(number, path)?)),
                }
            }
            "transform.rotation" => {
                entity.transform.rotation = finite(float_value(value, path)?, path)?
            }
            "speed" => {
                let speed = non_negative(float_value(value, path)?, path)?;
                let name = entity.name.clone();
                entity
                    .movement
                    .as_mut()
                    .ok_or_else(|| {
                        EditError::Unresolved(format!(
                            "l'entité « {name} » n'a pas de composant movement"
                        ))
                    })?
                    .speed = speed;
            }
            "health.max" | "health.current" => {
                let number = non_negative(float_value(value, path)?, path)?;
                let name = entity.name.clone();
                let component = entity.health.as_mut().ok_or_else(|| {
                    EditError::Unresolved(format!("l'entité « {name} » n'a pas de composant health"))
                })?;
                if path == "health.max" {
                    component.max = number.max(0.0);
                    component.current = component.current.min(component.max);
                } else {
                    component.current = number.min(component.max);
                }
            }
            "vision.range" => {
                let range = non_negative(float_value(value, path)?, path)?;
                let name = entity.name.clone();
                entity
                    .vision
                    .as_mut()
                    .ok_or_else(|| {
                        EditError::Unresolved(format!(
                            "l'entité « {name} » n'a pas de composant vision"
                        ))
                    })?
                    .range = range;
            }
            "collider.w" | "collider.h" => {
                let number = positive(float_value(value, path)?, path)?;
                let name = entity.name.clone();
                let collider = entity.collider.as_mut().ok_or_else(|| {
                    EditError::Unresolved(format!(
                        "l'entité « {name} » n'a pas de composant collider"
                    ))
                })?;
                if path == "collider.w" {
                    collider.w = number;
                } else {
                    collider.h = number;
                }
            }
            "collider.is_solid" => {
                let flag = bool_value(value, path)?;
                let name = entity.name.clone();
                entity
                    .collider
                    .as_mut()
                    .ok_or_else(|| {
                        EditError::Unresolved(format!(
                            "l'entité « {name} » n'a pas de composant collider"
                        ))
                    })?
                    .is_solid = flag;
            }
            "renderable.color" => {
                renderable_mut(entity, path)?.color = color_value(value, path)?;
            }
            "renderable.z_index" => {
                let index = int_value(value, path)?;
                let index = i32::try_from(index).map_err(|_| {
                    EditError::Invalid(format!("z_index hors bornes i32 : {index}"))
                })?;
                renderable_mut(entity, path)?.z_index = index;
            }
            "renderable.visible" => {
                renderable_mut(entity, path)?.visible = bool_value(value, path)?;
            }
            other => {
                return Err(EditError::Invalid(format!(
                    "propriété non supportée : {other}"
                )))
            }
        }
    }
    Ok(())
}

/// Composants pouvant être ajoutés par une op.
pub const ADDABLE_COMPONENTS: &[&str] = &[
    "collider",
    "movement",
    "health",
    "vision",
    "projectile",
    "boss",
    "transform3d",
    "renderable3d",
];

/// Ajoute un composant (avec valeurs par défaut bornées) s'il est absent.
pub(crate) fn add_component(
    project: &mut KaractiProject,
    target: &TargetRef,
    component: &str,
) -> Result<(), EditError> {
    if !ADDABLE_COMPONENTS.contains(&component) {
        return Err(EditError::Invalid(format!(
            "composant non ajoutable « {component} » ; composants supportés : {}",
            ADDABLE_COMPONENTS.join(", ")
        )));
    }
    for id in resolve_entity_ids(project, target)? {
        let entity = entity_mut(project, id)?;
        let already = match component {
            "collider" => entity.collider.is_some(),
            "movement" => entity.movement.is_some(),
            "health" => entity.health.is_some(),
            "vision" => entity.vision.is_some(),
            "projectile" => entity.projectile.is_some(),
            "boss" => entity.boss.is_some(),
            "transform3d" => entity.transform3d.is_some(),
            _ => entity.renderable3d.is_some(),
        };
        if already {
            return Err(EditError::Invalid(format!(
                "l'entité « {} » possède déjà le composant {component}",
                entity.name
            )));
        }
        match component {
            "collider" => {
                entity.collider =
                    Some(Collider2D::solid(entity.transform.w, entity.transform.h));
            }
            "movement" => entity.movement = Some(Movement2D::new(110.0)),
            "health" => entity.health = Some(Health2D::new(5.0)),
            "vision" => entity.vision = Some(VisionSensing::new(200.0)),
            "projectile" => {
                entity.projectile =
                    Some(crate::entity::Projectile2D::new(5.0, 10, entity.id));
            }
            "boss" => {
                entity.boss = Some(crate::entity::BossComponent::new(
                    entity.name.clone(),
                    "Boss",
                ));
            }
            "transform3d" => {
                entity.transform3d = Some(Transform3D::from_euler_deg(
                    Vec3::ZERO,
                    Vec3::ZERO,
                    Vec3::ONE,
                ));
            }
            _ => {
                entity.renderable3d = Some(Renderable3D {
                    mesh: MeshRef::Cube,
                    material: MaterialRef::Solid {
                        color: 0x00FFFFFF,
                        roughness: 0.4,
                        metalness: 0.0,
                    },
                    cast_shadow: true,
                    receive_shadow: true,
                });
            }
        }
    }
    Ok(())
}

/// Composants pouvant être retirés par une op.
pub const REMOVABLE_COMPONENTS: &[&str] = &[
    "renderable",
    "collider",
    "movement",
    "vision",
    "script",
    "emitter",
    "projectile",
    "health",
    "dialogue",
    "item_pickup",
    "boss",
    "nav_agent",
    "flock",
    "transform3d",
    "renderable3d",
];

/// Retire un composant ; absent = erreur explicite (jamais un no-op).
pub(crate) fn remove_component(
    project: &mut KaractiProject,
    target: &TargetRef,
    component: &str,
) -> Result<(), EditError> {
    if !REMOVABLE_COMPONENTS.contains(&component) {
        return Err(EditError::Invalid(format!(
            "composant non retirable « {component} » ; composants supportés : {}",
            REMOVABLE_COMPONENTS.join(", ")
        )));
    }
    for id in resolve_entity_ids(project, target)? {
        let entity = entity_mut(project, id)?;
        let name = entity.name.clone();
        let removed = match component {
            "renderable" => entity.renderable.take().is_some(),
            "collider" => entity.collider.take().is_some(),
            "movement" => entity.movement.take().is_some(),
            "vision" => entity.vision.take().is_some(),
            "script" => entity.script.take().is_some(),
            "emitter" => entity.emitter.take().is_some(),
            "projectile" => entity.projectile.take().is_some(),
            "health" => entity.health.take().is_some(),
            "dialogue" => entity.dialogue.take().is_some(),
            "item_pickup" => entity.item_pickup.take().is_some(),
            "boss" => entity.boss.take().is_some(),
            "nav_agent" => entity.nav_agent.take().is_some(),
            "flock" => entity.flock.take().is_some(),
            "transform3d" => entity.transform3d.take().is_some(),
            _ => entity.renderable3d.take().is_some(),
        };
        if !removed {
            return Err(EditError::Unresolved(format!(
                "l'entité « {name} » n'a pas de composant {component}"
            )));
        }
    }
    Ok(())
}

/// Supprime les entités ciblées.
pub(crate) fn delete_entities(
    project: &mut KaractiProject,
    targets: &[TargetRef],
) -> Result<(), EditError> {
    let mut ids: Vec<usize> = Vec::new();
    for target in targets {
        for id in resolve_entity_ids(project, target)? {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let before = project.scene.entities.len();
    project.scene.entities.retain(|entity| !ids.contains(&entity.id));
    if project.scene.entities.len() == before {
        return Err(EditError::NoTarget(format!("{ids:?}")));
    }
    Ok(())
}

/// Duplique les entités ciblées avec un décalage, en allouant des identifiants libres.
pub(crate) fn duplicate_entities(
    project: &mut KaractiProject,
    targets: &[TargetRef],
    offset: [f32; 2],
) -> Result<(), EditError> {
    let dx = finite(offset[0], "offset.x")?;
    let dy = finite(offset[1], "offset.y")?;
    let mut ids: Vec<usize> = Vec::new();
    for target in targets {
        for id in resolve_entity_ids(project, target)? {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    for id in ids {
        let mut copy = project
            .scene
            .entities
            .iter()
            .find(|entity| entity.id == id)
            .cloned()
            .ok_or_else(|| EditError::NoTarget(format!("l'entité #{id}")))?;
        copy.id = next_entity_id(project);
        copy.name = format!("{}_copie", copy.name);
        let x = copy.transform.x + dx;
        let y = copy.transform.y + dy;
        apply_transform_2d(&mut copy.transform, Some(x), Some(y), None, None);
        project.scene.entities.push(copy);
    }
    Ok(())
}

/// Identifiant libre = max + 1 (les index ne sont pas fiables après suppression).
pub(crate) fn next_entity_id(project: &KaractiProject) -> usize {
    project
        .scene
        .entities
        .iter()
        .map(|entity| entity.id)
        .max()
        .map(|max| max + 1)
        .unwrap_or(0)
}

/// Paramètres de nœud KVL éditables.
pub const EDITABLE_NODE_PARAMS: &[&str] =
    &["name", "description", "position_x", "position_y", "color_tag"];

/// Définit un paramètre de nœud KVL (`input.<pin>` / `output.<pin>` pour une valeur par défaut).
pub(crate) fn set_kvl_node_param(
    project: &mut KaractiProject,
    target: &TargetRef,
    param: &str,
    value: &PropertyValue,
) -> Result<(), EditError> {
    let node_id = match target {
        TargetRef::KvlNodeId(node) => node.clone(),
        other => {
            return Err(EditError::Unresolved(format!(
                "la cible {other:?} n'est pas un nœud KVL"
            )))
        }
    };
    let (graph_id, _) = locate_kvl_node(project, &node_id)?;
    let graph = project
        .kvl_graphs
        .get_mut(&graph_id)
        .expect("graphe localisé");
    let node = graph
        .nodes
        .get_mut(&node_id)
        .expect("nœud localisé dans son graphe");

    if let Some(pin_id) = param.strip_prefix("input.") {
        let pin = node
            .inputs
            .iter_mut()
            .find(|pin| pin.id == pin_id)
            .ok_or_else(|| EditError::NoTarget(format!("le pin d'entrée « {pin_id} »")))?;
        ensure_pin_type(pin.data_type.clone(), value, pin_id)?;
        pin.default_value = Some(to_kvl_value(value));
        return Ok(());
    }
    if let Some(pin_id) = param.strip_prefix("output.") {
        let pin = node
            .outputs
            .iter_mut()
            .find(|pin| pin.id == pin_id)
            .ok_or_else(|| EditError::NoTarget(format!("le pin de sortie « {pin_id} »")))?;
        ensure_pin_type(pin.data_type.clone(), value, pin_id)?;
        pin.default_value = Some(to_kvl_value(value));
        return Ok(());
    }

    match param {
        "name" => node.name = string_value(value, param)?,
        "description" => node.description = string_value(value, param)?,
        "position_x" => node.ui.position_x = finite(float_value(value, param)?, param)?,
        "position_y" => node.ui.position_y = finite(float_value(value, param)?, param)?,
        "color_tag" => {
            node.ui.color_tag = match value {
                PropertyValue::String(text) if text.is_empty() => None,
                PropertyValue::String(text) => Some(text.clone()),
                _ => {
                    return Err(EditError::Invalid(format!(
                        "color_tag attend une chaîne (reçu {value:?})"
                    )))
                }
            }
        }
        other => {
            return Err(EditError::Invalid(format!(
                "paramètre de nœud inconnu « {other} » ; paramètres éditables : {} (ou input.<pin>, output.<pin>)",
                EDITABLE_NODE_PARAMS.join(", ")
            )))
        }
    }
    Ok(())
}

/// Supprime un nœud KVL et toutes ses connexions.
pub(crate) fn delete_kvl_node(
    project: &mut KaractiProject,
    target: &TargetRef,
) -> Result<(), EditError> {
    let node_id = match target {
        TargetRef::KvlNodeId(node) => node.clone(),
        other => {
            return Err(EditError::Unresolved(format!(
                "la cible {other:?} n'est pas un nœud KVL"
            )))
        }
    };
    let (graph_id, _) = locate_kvl_node(project, &node_id)?;
    let graph = project
        .kvl_graphs
        .get_mut(&graph_id)
        .expect("graphe localisé");
    if graph.nodes.remove(&node_id).is_none() {
        return Err(EditError::NoTarget(format!("le nœud « {node_id} »")));
    }
    graph
        .connections
        .retain(|connection| connection.from_node != node_id && connection.to_node != node_id);
    Ok(())
}

/// Supprime une connexion par identifiant (graphe précis, ou recherche globale).
pub(crate) fn remove_connection(
    project: &mut KaractiProject,
    graph: Option<&str>,
    connection_id: &str,
) -> Result<(), EditError> {
    let keys: Vec<String> = match graph {
        Some(graph_id) => {
            if !project.kvl_graphs.contains_key(graph_id) {
                return Err(EditError::NoTarget(format!("le graphe « {graph_id} »")));
            }
            vec![graph_id.to_string()]
        }
        None => {
            let mut keys: Vec<String> = project.kvl_graphs.keys().cloned().collect();
            keys.sort();
            keys
        }
    };
    for key in keys {
        let graph = project.kvl_graphs.get_mut(&key).expect("clé existante");
        if graph.remove_connection_by_id(connection_id) {
            return Ok(());
        }
    }
    Err(EditError::NoTarget(format!(
        "la connexion « {connection_id} »"
    )))
}

/// Réglages de scène éditables.
pub const EDITABLE_SCENE_SETTINGS: &[&str] = &["name", "description", "objective"];

/// Définit un réglage de scène (`objective` accepte une chaîne vide pour l'effacer).
pub(crate) fn set_scene_setting(
    project: &mut KaractiProject,
    key: &str,
    value: &PropertyValue,
) -> Result<(), EditError> {
    match key {
        "name" => project.scene.name = string_value(value, key)?,
        "description" => project.scene.description = string_value(value, key)?,
        "objective" => {
            let text = string_value(value, key)?;
            project.scene.objective = if text.is_empty() { None } else { Some(text) };
        }
        other => {
            return Err(EditError::Invalid(format!(
                "réglage de scène inconnu « {other} » ; supportés : {}",
                EDITABLE_SCENE_SETTINGS.join(", ")
            )))
        }
    }
    Ok(())
}

// ============================================================================
// 9. Aperçu et proposition
// ============================================================================

/// Changement sémantique d'un champ (`avant → après`), jamais un diff JSON brut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldChange {
    pub target: String,
    pub field: String,
    pub before: String,
    pub after: String,
}

impl FieldChange {
    fn new(target: impl Into<String>, field: impl Into<String>, before: impl Into<String>, after: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            field: field.into(),
            before: before.into(),
            after: after.into(),
        }
    }
}

/// Diff sémantique d'une proposition (ghost + liste `champ: avant → après`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PreviewDelta {
    pub changes: Vec<FieldChange>,
}

impl PreviewDelta {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Rapport de validation (cible, schéma, types, bornes, dimension).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub messages: Vec<String>,
}

/// Proposition d'édition : immuable, hashée, révisable avant application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditProposal {
    pub id: String,
    pub intent_id: String,
    pub target: ResolvedTarget,
    pub ops: Vec<AiOp>,
    pub rationale: String,
    pub risk: RiskLevel,
    pub validation: ValidationReport,
    pub preview: PreviewDelta,
    pub provider: String,
    pub model: String,
    pub tokens: TokenUsage,
    /// Hash de `(intent_id, target, ops)` : détecte une proposition périmée.
    pub hash: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt: u32,
    pub completion: u32,
}

impl EditProposal {
    /// Recalcule le hash ; utilisé pour refuser une proposition périmée.
    pub fn recompute_hash(&self) -> String {
        let payload = serde_json::json!({
            "intent_id": self.intent_id,
            "target": self.target,
            "ops": self.ops,
        });
        hash_text(payload.to_string().as_bytes())
    }

    /// Renseigne le modèle et la consommation de tokens (rempli par l'appel LLM).
    pub fn with_model(mut self, provider: impl Into<String>, model: impl Into<String>, tokens: TokenUsage) -> Self {
        self.provider = provider.into();
        self.model = model.into();
        self.tokens = tokens;
        self
    }
}

/// Construit une proposition à partir d'ops et d'un contexte d'édition.
///
/// Étapes : résolution de cible → binding → exécution sur clone → diff → risque → hash.
pub fn build_proposal(
    project: &KaractiProject,
    ctx: &EditContext,
    intent_id: &str,
    ops: Vec<AiOp>,
    rationale: &str,
) -> Result<EditProposal, EditError> {
    if ops.is_empty() {
        return Err(EditError::Invalid("aucune opération proposée".to_string()));
    }
    let target = resolve_ops_target(project, ctx, &ops)?;
    let bound = bind_ops(project, ctx, &ops)?;
    let (after, _) = execute(project, &bound)?;
    let preview = diff_project(project, &after);
    if preview.is_empty() {
        return Err(EditError::NoChange);
    }
    let risk = risk_of_ops(&bound);
    let validation = ValidationReport {
        ok: true,
        messages: vec![
            format!("cible : {} [{}]", target.label, ctx.dimension.label()),
            format!("{} changement(s)", preview.changes.len()),
        ],
    };
    let mut proposal = EditProposal {
        id: String::new(),
        intent_id: intent_id.to_string(),
        target,
        ops: bound,
        rationale: rationale.to_string(),
        risk,
        validation,
        preview,
        provider: "template".to_string(),
        model: "deterministic".to_string(),
        tokens: TokenUsage::default(),
        hash: String::new(),
    };
    proposal.hash = proposal.recompute_hash();
    proposal.id = format!("prop_{}", proposal.hash);
    Ok(proposal)
}

/// Contexte vide : résolution limitée aux cibles concrètes (tests, éditeur headless).
pub fn build_proposal_headless(
    project: &KaractiProject,
    intent_id: &str,
    ops: Vec<AiOp>,
    rationale: &str,
) -> Result<EditProposal, EditError> {
    build_proposal(project, &EditContext::for_project(project), intent_id, ops, rationale)
}

/// Calcule l'aperçu d'une proposition **sans rien appliquer**.
pub fn preview_edit(
    project: &KaractiProject,
    proposal: &EditProposal,
) -> Result<PreviewDelta, EditError> {
    ensure_fresh(proposal)?;
    let (after, _) = execute(project, &proposal.ops)?;
    let preview = diff_project(project, &after);
    if preview.is_empty() {
        return Err(EditError::NoChange);
    }
    Ok(preview)
}

/// Applique une proposition sur un clone (tout ou rien) et renvoie la transaction d'undo.
pub fn apply_edit(
    project: &KaractiProject,
    proposal: &EditProposal,
) -> Result<AppliedEdit, EditError> {
    ensure_fresh(proposal)?;
    let (after, mut transaction) = execute(project, &proposal.ops)?;
    if diff_project(project, &after).is_empty() {
        return Err(EditError::NoChange);
    }
    transaction.proposal_id = proposal.id.clone();
    transaction.intent_id = proposal.intent_id.clone();
    Ok(AppliedEdit {
        project: after,
        transaction,
    })
}

/// Résultat d'une application : nouveau projet + transaction réversible.
#[derive(Debug, Clone)]
pub struct AppliedEdit {
    pub project: KaractiProject,
    pub transaction: AiTransaction,
}

fn ensure_fresh(proposal: &EditProposal) -> Result<(), EditError> {
    if proposal.hash != proposal.recompute_hash() {
        return Err(EditError::Stale);
    }
    Ok(())
}

/// Exécute les ops sur un clone et capture la transaction.
pub(crate) fn execute(
    project: &KaractiProject,
    ops: &[AiOp],
) -> Result<(KaractiProject, AiTransaction), EditError> {
    let mut working = project.clone();
    for (index, op) in ops.iter().enumerate() {
        crate::ai_executor::apply_op(&mut working, op)
            .map_err(|message| EditError::Op { index, message })?;
    }
    let transaction = capture(project, &working);
    Ok((working, transaction))
}

fn resolve_ops_target(
    project: &KaractiProject,
    ctx: &EditContext,
    ops: &[AiOp],
) -> Result<ResolvedTarget, EditError> {
    let mut resolved = Vec::new();
    for op in ops {
        for target in op_targets(op) {
            resolved.push(resolve_target(project, ctx, target)?);
        }
    }
    match resolved.len() {
        0 => Ok(ResolvedTarget::empty_scene_label()),
        1 => Ok(resolved.remove(0)),
        _ => Ok(merge_targets(resolved)),
    }
}

// ============================================================================
// 10. Transaction et undo
// ============================================================================

/// Transaction d'une intention : une intention = une transaction = un `⌘Z`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTransaction {
    pub id: String,
    pub proposal_id: String,
    pub intent_id: String,
    /// Entités modifiées ou supprimées : `(index d'origine, état avant)`.
    pub before_entities: Vec<(usize, Entity)>,
    /// Graphes modifiés, créés ou supprimés : `(clé, état avant / None si créé)`.
    pub before_graphs: Vec<(String, Option<Graph>)>,
    pub created_entity_ids: Vec<usize>,
    pub deleted_entity_ids: Vec<usize>,
}

impl AiTransaction {
    pub fn touched_entities(&self) -> usize {
        self.before_entities.len() + self.created_entity_ids.len()
    }
}

fn capture(before: &KaractiProject, after: &KaractiProject) -> AiTransaction {
    let mut before_entities: Vec<(usize, Entity)> = Vec::new();
    let mut deleted_entity_ids: Vec<usize> = Vec::new();
    for (index, entity) in before.scene.entities.iter().enumerate() {
        match after.scene.entities.iter().find(|candidate| candidate.id == entity.id) {
            Some(updated) if updated != entity => before_entities.push((index, entity.clone())),
            Some(_) => {}
            None => {
                deleted_entity_ids.push(entity.id);
                before_entities.push((index, entity.clone()));
            }
        }
    }
    let before_ids: HashSet<usize> = before.scene.entities.iter().map(|entity| entity.id).collect();
    let created_entity_ids: Vec<usize> = after
        .scene
        .entities
        .iter()
        .filter(|entity| !before_ids.contains(&entity.id))
        .map(|entity| entity.id)
        .collect();

    let mut graph_keys: Vec<String> = before
        .kvl_graphs
        .keys()
        .chain(after.kvl_graphs.keys())
        .cloned()
        .collect();
    graph_keys.sort();
    graph_keys.dedup();
    let mut before_graphs: Vec<(String, Option<Graph>)> = Vec::new();
    for key in graph_keys {
        let old = before.kvl_graphs.get(&key);
        let new = after.kvl_graphs.get(&key);
        if old != new {
            before_graphs.push((key, old.cloned()));
        }
    }

    let signature = format!(
        "{}|{:?}|{:?}|{:?}|{:?}",
        before.metadata.id,
        before_entities
            .iter()
            .map(|(index, entity)| (*index, entity.id))
            .collect::<Vec<_>>(),
        created_entity_ids,
        deleted_entity_ids,
        before_graphs.iter().map(|(key, _)| key).collect::<Vec<_>>(),
    );
    let transaction_id = format!("tx_{}", hash_text(signature.as_bytes()));
    AiTransaction {
        id: transaction_id,
        proposal_id: String::new(),
        intent_id: String::new(),
        before_entities,
        before_graphs,
        created_entity_ids,
        deleted_entity_ids,
    }
}

/// Défait une transaction : restaure les entités et graphes touchés.
pub fn undo(project: &KaractiProject, transaction: &AiTransaction) -> Result<KaractiProject, EditError> {
    let mut restored = project.clone();
    if !transaction.created_entity_ids.is_empty() {
        restored
            .scene
            .entities
            .retain(|entity| !transaction.created_entity_ids.contains(&entity.id));
    }
    for (index, entity) in &transaction.before_entities {
        match restored
            .scene
            .entities
            .iter_mut()
            .find(|candidate| candidate.id == entity.id)
        {
            Some(slot) => *slot = entity.clone(),
            None => {
                let at = (*index).min(restored.scene.entities.len());
                restored.scene.entities.insert(at, entity.clone());
            }
        }
    }
    for (key, graph) in &transaction.before_graphs {
        match graph {
            Some(graph) => {
                restored.kvl_graphs.insert(key.clone(), graph.clone());
            }
            None => {
                restored.kvl_graphs.remove(key);
            }
        }
    }
    Ok(restored)
}

// ============================================================================
// 11. Diff sémantique
// ============================================================================

fn diff_project(before: &KaractiProject, after: &KaractiProject) -> PreviewDelta {
    let mut changes = Vec::new();
    let before_ids: HashSet<usize> = before.scene.entities.iter().map(|entity| entity.id).collect();

    for entity in &before.scene.entities {
        let target = format!("{} #{}", entity.name, entity.id);
        match after.scene.entities.iter().find(|candidate| candidate.id == entity.id) {
            Some(updated) => changes.extend(diff_entity(&target, entity, updated)),
            None => changes.push(FieldChange::new(
                target,
                "existence",
                "présente",
                "supprimée",
            )),
        }
    }
    for entity in &after.scene.entities {
        if !before_ids.contains(&entity.id) {
            changes.push(FieldChange::new(
                format!("{} #{}", entity.name, entity.id),
                "existence",
                "absente",
                "créée",
            ));
        }
    }

    if before.scene.name != after.scene.name {
        changes.push(FieldChange::new("scène", "nom", before.scene.name.clone(), after.scene.name.clone()));
    }
    if before.scene.description != after.scene.description {
        changes.push(FieldChange::new(
            "scène",
            "description",
            before.scene.description.clone(),
            after.scene.description.clone(),
        ));
    }
    if before.scene.objective != after.scene.objective {
        changes.push(FieldChange::new(
            "scène",
            "objectif",
            before.scene.objective.clone().unwrap_or_default(),
            after.scene.objective.clone().unwrap_or_default(),
        ));
    }

    changes.extend(diff_graphs(&before.kvl_graphs, &after.kvl_graphs));
    PreviewDelta { changes }
}

fn diff_entity(target: &str, before: &Entity, after: &Entity) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    let mut push = |field: &str, before: String, after: String| {
        if before != after {
            changes.push(FieldChange::new(target, field, before, after));
        }
    };
    push("nom", before.name.clone(), after.name.clone());
    push("tag", before.tag.clone(), after.tag.clone());
    push("actif", before.is_active.to_string(), after.is_active.to_string());
    push("x", num(before.transform.x), num(after.transform.x));
    push("y", num(before.transform.y), num(after.transform.y));
    push("w", num(before.transform.w), num(after.transform.w));
    push("h", num(before.transform.h), num(after.transform.h));
    push("rotation", num(before.transform.rotation), num(after.transform.rotation));
    push(
        "couleur",
        renderable_color(before.renderable.as_ref()),
        renderable_color(after.renderable.as_ref()),
    );
    push(
        "sprite",
        shape_label(before.renderable.as_ref().map(|renderable| &renderable.shape)),
        shape_label(after.renderable.as_ref().map(|renderable| &renderable.shape)),
    );
    push(
        "z_index",
        renderable_z(before.renderable.as_ref()),
        renderable_z(after.renderable.as_ref()),
    );
    push(
        "vitesse",
        movement_speed(before.movement.as_ref()),
        movement_speed(after.movement.as_ref()),
    );
    push(
        "vie max",
        health_max(before.health.as_ref()),
        health_max(after.health.as_ref()),
    );
    push(
        "vision",
        vision_range(before.vision.as_ref()),
        vision_range(after.vision.as_ref()),
    );
    push(
        "collider",
        collider_size(before.collider.as_ref()),
        collider_size(after.collider.as_ref()),
    );
    push(
        "mesh",
        mesh_label(before.renderable3d.as_ref().map(|renderable| &renderable.mesh)),
        mesh_label(after.renderable3d.as_ref().map(|renderable| &renderable.mesh)),
    );
    push(
        "matériau",
        material_label(before.renderable3d.as_ref().map(|renderable| &renderable.material)),
        material_label(after.renderable3d.as_ref().map(|renderable| &renderable.material)),
    );
    push(
        "transform 3D",
        transform3d_label(before.transform3d.as_ref()),
        transform3d_label(after.transform3d.as_ref()),
    );
    push(
        "script",
        before.script.as_ref().map(|script| script.script_path.clone()).unwrap_or_default(),
        after.script.as_ref().map(|script| script.script_path.clone()).unwrap_or_default(),
    );
    changes
}

fn diff_graphs(before: &HashMap<String, Graph>, after: &HashMap<String, Graph>) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    let mut keys: Vec<String> = before.keys().chain(after.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    for key in keys {
        match (before.get(&key), after.get(&key)) {
            (None, Some(_)) => changes.push(FieldChange::new(format!("graphe {key}"), "existence", "absent", "créé")),
            (Some(_), None) => changes.push(FieldChange::new(format!("graphe {key}"), "existence", "présent", "supprimé")),
            (Some(old), Some(new)) if old != new => {
                let mut node_ids: Vec<String> = old.nodes.keys().chain(new.nodes.keys()).cloned().collect();
                node_ids.sort();
                node_ids.dedup();
                for node_id in node_ids {
                    match (old.nodes.get(&node_id), new.nodes.get(&node_id)) {
                        (None, Some(_)) => changes.push(FieldChange::new(
                            format!("{key} · {node_id}"),
                            "existence",
                            "absent",
                            "créé",
                        )),
                        (Some(_), None) => changes.push(FieldChange::new(
                            format!("{key} · {node_id}"),
                            "existence",
                            "présent",
                            "supprimé",
                        )),
                        (Some(old_node), Some(new_node)) if old_node != new_node => {
                            let target = format!("{key} · {node_id}");
                            if old_node.name != new_node.name {
                                changes.push(FieldChange::new(target.clone(), "nom", old_node.name.clone(), new_node.name.clone()));
                            }
                            if old_node.description != new_node.description {
                                changes.push(FieldChange::new(target.clone(), "description", old_node.description.clone(), new_node.description.clone()));
                            }
                            if old_node.ui.position_x != new_node.ui.position_x {
                                changes.push(FieldChange::new(target.clone(), "position_x", num(old_node.ui.position_x), num(new_node.ui.position_x)));
                            }
                            if old_node.ui.position_y != new_node.ui.position_y {
                                changes.push(FieldChange::new(target.clone(), "position_y", num(old_node.ui.position_y), num(new_node.ui.position_y)));
                            }
                            if old_node.ui.color_tag != new_node.ui.color_tag {
                                changes.push(FieldChange::new(
                                    target.clone(),
                                    "color_tag",
                                    old_node.ui.color_tag.clone().unwrap_or_default(),
                                    new_node.ui.color_tag.clone().unwrap_or_default(),
                                ));
                            }
                            for (old_pin, new_pin) in old_node.inputs.iter().zip(new_node.inputs.iter()) {
                                if old_pin.default_value != new_pin.default_value {
                                    changes.push(FieldChange::new(
                                        target.clone(),
                                        format!("input.{}", old_pin.id),
                                        value_label(old_pin.default_value.as_ref()),
                                        value_label(new_pin.default_value.as_ref()),
                                    ));
                                }
                            }
                            for (old_pin, new_pin) in old_node.outputs.iter().zip(new_node.outputs.iter()) {
                                if old_pin.default_value != new_pin.default_value {
                                    changes.push(FieldChange::new(
                                        target.clone(),
                                        format!("output.{}", old_pin.id),
                                        value_label(old_pin.default_value.as_ref()),
                                        value_label(new_pin.default_value.as_ref()),
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let old_connections: HashSet<&str> =
                    old.connections.iter().map(|connection| connection.id.as_str()).collect();
                let new_connections: HashSet<&str> =
                    new.connections.iter().map(|connection| connection.id.as_str()).collect();
                for connection in &old.connections {
                    if !new_connections.contains(connection.id.as_str()) {
                        changes.push(FieldChange::new(
                            format!("{key} · connexion {}", connection.id),
                            "existence",
                            "présente",
                            "supprimée",
                        ));
                    }
                }
                for connection in &new.connections {
                    if !old_connections.contains(connection.id.as_str()) {
                        changes.push(FieldChange::new(
                            format!("{key} · connexion {}", connection.id),
                            "existence",
                            "absente",
                            "créée",
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    changes
}

// ============================================================================
// 12. Helpers
// ============================================================================

fn entity_mut(project: &mut KaractiProject, id: usize) -> Result<&mut Entity, EditError> {
    project
        .scene
        .entities
        .iter_mut()
        .find(|entity| entity.id == id)
        .ok_or_else(|| EditError::NoTarget(format!("l'entité #{id}")))
}

fn renderable_mut<'a>(entity: &'a mut Entity, field: &str) -> Result<&'a mut Renderable2D, EditError> {
    let name = entity.name.clone();
    entity.renderable.as_mut().ok_or_else(|| {
        EditError::Unresolved(format!(
            "l'entité « {name} » n'a pas de rendu 2D : impossible de changer {field} \
             (utilisez AddComponent(renderable2d) ou ciblez une entité dessinée)"
        ))
    })
}

fn apply_transform_2d(
    transform: &mut Transform2D,
    x: Option<f32>,
    y: Option<f32>,
    w: Option<f32>,
    h: Option<f32>,
) {
    if let Some(x) = x {
        transform.x = x;
        transform.prev_x = x;
    }
    if let Some(y) = y {
        transform.y = y;
        transform.prev_y = y;
    }
    if let Some(w) = w {
        transform.w = w;
    }
    if let Some(h) = h {
        transform.h = h;
    }
}

fn apply_material_patch(entity: &mut Entity, patch: &MaterialPatch) -> Result<(), EditError> {
    let name = entity.name.clone();
    let renderable = entity.renderable3d.as_mut().ok_or_else(|| {
        EditError::Unresolved(format!(
            "l'entité « {name} » n'a pas de rendu 3D : impossible de changer le matériau"
        ))
    })?;
    if let Some(asset) = &patch.asset {
        renderable.material = MaterialRef::Asset { id: asset.clone() };
        return Ok(());
    }
    if patch.color.is_none() && patch.roughness.is_none() && patch.metalness.is_none() {
        return Err(EditError::Invalid(
            "patch de matériau vide : précisez color, roughness, metalness ou asset".to_string(),
        ));
    }
    let (color, roughness, metalness) = match &renderable.material {
        MaterialRef::Solid { color, roughness, metalness } => (*color, *roughness, *metalness),
        MaterialRef::Asset { id } => {
            return Err(EditError::Unresolved(format!(
                "matériau asset « {id} » : précisez asset, ou une couleur pour le remplacer"
            )))
        }
    };
    let roughness = patch.roughness.unwrap_or(roughness);
    let metalness = patch.metalness.unwrap_or(metalness);
    if !(0.0..=1.0).contains(&roughness) || !(0.0..=1.0).contains(&metalness) {
        return Err(EditError::Invalid(format!(
            "roughness/metalness attendus dans [0, 1] (reçu {roughness} / {metalness})"
        )));
    }
    renderable.material = MaterialRef::Solid {
        color: patch.color.unwrap_or(color),
        roughness,
        metalness,
    };
    Ok(())
}

fn ensure_pin_type(data_type: kvl_core::DataType, value: &PropertyValue, pin_id: &str) -> Result<(), EditError> {
    use kvl_core::DataType;
    let compatible = match data_type {
        DataType::Trigger => false,
        DataType::Bool => matches!(value, PropertyValue::Bool(_)),
        DataType::Int => matches!(value, PropertyValue::Int(_) | PropertyValue::Color(_)),
        DataType::Float => matches!(value, PropertyValue::Float(_) | PropertyValue::Int(_)),
        DataType::String => matches!(value, PropertyValue::String(_)),
        DataType::EntityRef => matches!(value, PropertyValue::String(_)),
        DataType::Vector2 | DataType::Vector3 | DataType::Any => true,
    };
    if compatible {
        Ok(())
    } else {
        Err(EditError::Invalid(format!(
            "le pin « {pin_id} » attend {data_type:?} (reçu {value:?})"
        )))
    }
}

fn to_kvl_value(value: &PropertyValue) -> Value {
    match value {
        PropertyValue::Bool(flag) => Value::Bool(*flag),
        PropertyValue::Int(number) => Value::Int(*number),
        PropertyValue::Color(color) => Value::Int(i64::from(*color)),
        PropertyValue::Float(number) => Value::Float(*number),
        PropertyValue::String(text) => Value::String(text.clone()),
    }
}

fn finite(value: f32, field: &str) -> Result<f32, EditError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(EditError::Invalid(format!(
            "{field} doit être un nombre fini (reçu {value})"
        )))
    }
}

fn positive(value: f32, field: &str) -> Result<f32, EditError> {
    let value = finite(value, field)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(EditError::Invalid(format!("{field} doit être > 0 (reçu {value})")))
    }
}

fn non_negative(value: f32, field: &str) -> Result<f32, EditError> {
    let value = finite(value, field)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(EditError::Invalid(format!(
            "{field} doit être ≥ 0 (reçu {value})"
        )))
    }
}

fn float_value(value: &PropertyValue, field: &str) -> Result<f32, EditError> {
    match value {
        PropertyValue::Float(number) => Ok(*number as f32),
        PropertyValue::Int(number) => Ok(*number as f32),
        other => Err(EditError::Invalid(format!(
            "{field} attend un nombre (reçu {other:?})"
        ))),
    }
}

fn int_value(value: &PropertyValue, field: &str) -> Result<i64, EditError> {
    match value {
        PropertyValue::Int(number) => Ok(*number),
        other => Err(EditError::Invalid(format!(
            "{field} attend un entier (reçu {other:?})"
        ))),
    }
}

fn bool_value(value: &PropertyValue, field: &str) -> Result<bool, EditError> {
    match value {
        PropertyValue::Bool(flag) => Ok(*flag),
        other => Err(EditError::Invalid(format!(
            "{field} attend un booléen (reçu {other:?})"
        ))),
    }
}

fn string_value(value: &PropertyValue, field: &str) -> Result<String, EditError> {
    match value {
        PropertyValue::String(text) => Ok(text.clone()),
        other => Err(EditError::Invalid(format!(
            "{field} attend une chaîne (reçu {other:?})"
        ))),
    }
}

fn color_value(value: &PropertyValue, field: &str) -> Result<u32, EditError> {
    match value {
        PropertyValue::Color(color) => Ok(*color),
        PropertyValue::Int(number) if (0..=0x00FF_FFFF).contains(number) => Ok(*number as u32),
        other => Err(EditError::Invalid(format!(
            "{field} attend une couleur 0x00RRGGBB (reçu {other:?})"
        ))),
    }
}

fn num(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn hex(color: u32) -> String {
    format!("#{:06X}", color & 0x00FF_FFFF)
}

fn renderable_color(renderable: Option<&Renderable2D>) -> String {
    renderable.map(|renderable| hex(renderable.color)).unwrap_or_default()
}

fn renderable_z(renderable: Option<&Renderable2D>) -> String {
    renderable.map(|renderable| renderable.z_index.to_string()).unwrap_or_default()
}

fn shape_label(shape: Option<&ShapeKind>) -> String {
    match shape {
        None => String::new(),
        Some(ShapeKind::Rect) => "Rect".to_string(),
        Some(ShapeKind::Circle { radius }) => format!("Circle({})", num(*radius)),
        Some(ShapeKind::Text { label, .. }) => format!("Text({label})"),
        Some(ShapeKind::PixelMatrix { width, height, .. }) => format!("PixelMatrix({width}×{height})"),
        Some(ShapeKind::SvgRef { asset }) => format!("SvgRef({asset})"),
        Some(ShapeKind::ImageRef { asset }) => format!("ImageRef({asset})"),
        Some(ShapeKind::SheetRef { asset, frame }) => format!("SheetRef({asset}:{frame})"),
        Some(ShapeKind::AnimRef { asset }) => format!("AnimRef({asset})"),
    }
}

fn movement_speed(movement: Option<&Movement2D>) -> String {
    movement.map(|movement| num(movement.speed)).unwrap_or_default()
}

fn health_max(health: Option<&Health2D>) -> String {
    health.map(|health| num(health.max)).unwrap_or_default()
}

fn vision_range(vision: Option<&VisionSensing>) -> String {
    vision.map(|vision| num(vision.range)).unwrap_or_default()
}

fn collider_size(collider: Option<&Collider2D>) -> String {
    match collider {
        Some(collider) => format!("{}×{}", num(collider.w), num(collider.h)),
        None => String::new(),
    }
}

fn mesh_label(mesh: Option<&MeshRef>) -> String {
    match mesh {
        None => String::new(),
        Some(MeshRef::Cube) => "Cube".to_string(),
        Some(MeshRef::Sphere) => "Sphere".to_string(),
        Some(MeshRef::Plane) => "Plane".to_string(),
        Some(MeshRef::Capsule) => "Capsule".to_string(),
        Some(MeshRef::Asset { id, node }) => match node {
            Some(node) => format!("Asset({id}:{node})"),
            None => format!("Asset({id})"),
        },
    }
}

fn material_label(material: Option<&MaterialRef>) -> String {
    match material {
        None => String::new(),
        Some(MaterialRef::Solid { color, roughness, metalness }) => {
            format!("solid {} r{} m{}", hex(*color), num(*roughness), num(*metalness))
        }
        Some(MaterialRef::Asset { id }) => format!("asset({id})"),
    }
}

fn transform3d_label(transform: Option<&Transform3D>) -> String {
    match transform {
        None => String::new(),
        Some(transform) => format!(
            "pos({}, {}, {}) scale({}, {}, {})",
            num(transform.position.x),
            num(transform.position.y),
            num(transform.position.z),
            num(transform.scale.x),
            num(transform.scale.y),
            num(transform.scale.z)
        ),
    }
}

fn value_label(value: Option<&Value>) -> String {
    match value {
        None => String::new(),
        Some(value) => serde_json::to_string(value).unwrap_or_default(),
    }
}

// ============================================================================
// 13. Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::GameGenre;

    fn project() -> KaractiProject {
        let mut project = KaractiProject::new_empty("Test", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();
        let mut boss = Entity::new(1, "Boss", "enemy", Transform2D::new(100.0, 100.0, 40.0, 40.0));
        boss.renderable = Some(Renderable2D::rect(0x00475569, 5));
        boss.movement = Some(Movement2D::new(110.0));
        boss.health = Some(Health2D::new(3.0));
        boss.collider = Some(Collider2D::solid(40.0, 40.0));
        project.scene.entities.push(boss);
        let mut coin = Entity::new(2, "Pièce", "collectible", Transform2D::new(300.0, 50.0, 24.0, 24.0));
        coin.renderable = Some(Renderable2D::circle(12.0, 0x00F5C542, 5));
        project.scene.entities.push(coin);
        project
    }

    fn update_ops(patch: EntityPatch) -> Vec<AiOp> {
        vec![AiOp::UpdateEntity {
            target: TargetRef::Selected,
            patch,
        }]
    }

    #[test]
    fn context_reads_project_identity_and_dimension() {
        let mut project = project();
        project.metadata.dimension = Some("3D".to_string());
        let ctx = EditContext::for_project(&project);
        assert_eq!(ctx.dimension, Dimension::ThreeD);
        assert_eq!(ctx.project_id, project.metadata.id);
        assert_eq!(ctx.scene_id, project.scene.name);
        assert!(ctx.is_selection_empty());
    }

    #[test]
    fn selected_without_selection_is_an_explicit_error() {
        let project = project();
        let ctx = EditContext::for_project(&project);
        let err = resolve_target(&project, &ctx, &TargetRef::Selected).unwrap_err();
        assert!(matches!(err, EditError::EmptySelection));
    }

    #[test]
    fn selected_resolves_to_concrete_entity() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        let resolved = resolve_target(&project, &ctx, &TargetRef::Selected).unwrap();
        assert_eq!(resolved.kind, TargetKind::Entity);
        assert_eq!(resolved.entity_ids, vec![1]);
        assert!(resolved.label.contains("Boss"));
    }

    #[test]
    fn tag_resolves_to_all_matching_entities() {
        let project = project();
        let resolved = resolve_pure_target(&project, &TargetRef::Tag("enemy".into())).unwrap();
        assert_eq!(resolved.entity_ids, vec![1]);
        let none = resolve_pure_target(&project, &TargetRef::Tag("absent".into()));
        assert!(matches!(none, Err(EditError::NoTarget(_))));
    }

    #[test]
    fn area_resolves_by_intersection() {
        let project = project();
        let resolved = resolve_pure_target(
            &project,
            &TargetRef::Area { x: 90.0, y: 90.0, w: 60.0, h: 60.0 },
        )
        .unwrap();
        assert_eq!(resolved.entity_ids, vec![1]);
        let empty = resolve_pure_target(
            &project,
            &TargetRef::Area { x: 0.0, y: 0.0, w: 0.0, h: 10.0 },
        );
        assert!(matches!(empty, Err(EditError::Invalid(_))));
    }

    #[test]
    fn duplicated_name_is_ambiguous() {
        let mut project = project();
        let mut copy = project.scene.entities[0].clone();
        copy.id = 9;
        project.scene.entities.push(copy);
        let err = resolve_pure_target(&project, &TargetRef::EntityName("Boss".into())).unwrap_err();
        assert!(matches!(err, EditError::Ambiguous(_)));
    }

    #[test]
    fn patch_updates_color_and_speed() {
        let mut project = project();
        let patch = EntityPatch {
            color: Some(0x00DC2626),
            speed: Some(160.0),
            ..EntityPatch::default()
        };
        apply_entity_patch(&mut project, &TargetRef::EntityId(1), &patch).unwrap();
        let boss = &project.scene.entities[0];
        assert_eq!(boss.renderable.as_ref().unwrap().color, 0x00DC2626);
        assert_eq!(boss.movement.as_ref().unwrap().speed, 160.0);
    }

    #[test]
    fn transform_update_keeps_interpolation_consistent() {
        let mut project = project();
        set_transform_2d(&mut project, &TargetRef::EntityId(1), 200.0, 240.0, 48.0, 48.0).unwrap();
        let transform = project.scene.entities[0].transform;
        assert_eq!(transform.x, 200.0);
        assert_eq!(transform.prev_x, 200.0, "prev_x doit suivre pour éviter un lerp");
        assert_eq!(transform.w, 48.0);
    }

    #[test]
    fn patch_on_entity_without_renderable_is_an_error() {
        let mut project = project();
        // Entité nue : aucun composant de rendu.
        project.scene.entities.push(Entity::new(
            3,
            "Marqueur",
            "marker",
            Transform2D::new(0.0, 0.0, 8.0, 8.0),
        ));
        let patch = EntityPatch { color: Some(0x00FF0000), ..EntityPatch::default() };
        let err = apply_entity_patch(&mut project, &TargetRef::EntityId(3), &patch).unwrap_err();
        assert!(matches!(err, EditError::Unresolved(_)));
        // Le message explique le composant manquant.
        assert!(err.to_string().contains("rendu 2D"));
    }

    #[test]
    fn empty_patch_is_an_error_not_a_noop() {
        let mut project = project();
        let err = apply_entity_patch(&mut project, &TargetRef::EntityId(1), &EntityPatch::default())
            .unwrap_err();
        assert!(matches!(err, EditError::Invalid(_)));
    }

    #[test]
    fn delete_entities_removes_target_and_reports_it() {
        let project = project();
        let ops = vec![AiOp::DeleteEntity { targets: vec![TargetRef::EntityId(2)] }];
        let proposal = build_proposal_headless(&project, "int_1", ops, "supprimer la pièce").unwrap();
        assert_eq!(proposal.risk, RiskLevel::Destructive);
        assert!(proposal.target.label.contains("Pièce"));
        let applied = apply_edit(&project, &proposal).unwrap();
        assert_eq!(applied.project.scene.entities.len(), 1);
        assert!(applied.project.scene.entities.iter().all(|entity| entity.id != 2));
    }

    #[test]
    fn duplicate_allocates_free_id_and_offsets_position() {
        let mut project = project();
        duplicate_entities(&mut project, &[TargetRef::EntityId(1)], [20.0, 30.0]).unwrap();
        assert_eq!(project.scene.entities.len(), 3);
        let copy = project.scene.entities.last().unwrap();
        assert_eq!(copy.id, 3);
        assert_eq!(copy.name, "Boss_copie");
        assert_eq!(copy.transform.x, 120.0);
        assert_eq!(copy.transform.y, 130.0);
        assert_eq!(copy.transform.prev_x, 120.0);
    }

    #[test]
    fn unknown_property_path_lists_supported_paths() {
        let mut project = project();
        let err = set_entity_property(
            &mut project,
            &TargetRef::EntityId(1),
            "couleur_magique",
            &PropertyValue::Int(1),
        )
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("propriété inconnue"));
        assert!(message.contains("transform.x"));
    }

    #[test]
    fn property_requires_its_component() {
        let mut project = project();
        // La pièce n'a pas de movement.
        let err = set_entity_property(
            &mut project,
            &TargetRef::EntityId(2),
            "speed",
            &PropertyValue::Float(200.0),
        )
        .unwrap_err();
        assert!(matches!(err, EditError::Unresolved(_)));
    }

    #[test]
    fn add_and_remove_component_roundtrip() {
        let mut project = project();
        add_component(&mut project, &TargetRef::EntityId(2), "movement").unwrap();
        assert!(project.scene.entities[1].movement.is_some());
        let duplicate = add_component(&mut project, &TargetRef::EntityId(2), "movement");
        assert!(duplicate.is_err(), "un composant déjà présent est une erreur");
        remove_component(&mut project, &TargetRef::EntityId(2), "movement").unwrap();
        assert!(project.scene.entities[1].movement.is_none());
    }

    #[test]
    fn unknown_component_is_rejected() {
        let mut project = project();
        let err = add_component(&mut project, &TargetRef::EntityId(1), "antigravity").unwrap_err();
        assert!(err.to_string().contains("composant non ajoutable"));
    }

    #[test]
    fn set_3d_transform_writes_rotation_and_scale() {
        let mut project = project();
        add_component(&mut project, &TargetRef::EntityId(1), "transform3d").unwrap();
        set_transform_3d(
            &mut project,
            &TargetRef::EntityId(1),
            [1.0, 2.0, 3.0],
            [0.0, 90.0, 0.0],
            [2.0, 2.0, 2.0],
        )
        .unwrap();
        let transform = project.scene.entities[0].transform3d.as_ref().unwrap();
        assert_eq!(transform.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(transform.scale, Vec3::new(2.0, 2.0, 2.0));
        let bad = set_transform_3d(
            &mut project,
            &TargetRef::EntityId(1),
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 1.0],
        );
        assert!(bad.is_err(), "une échelle nulle est invalide");
    }

    #[test]
    fn kvl_node_param_moves_node_and_deletes_connections() {
        let mut project = project();
        project.kvl_graphs.insert(
            "main".to_string(),
            Graph::new("main", "Principal", "test"),
        );
        let graph = project.kvl_graphs.get_mut("main").unwrap();
        graph.add_node(kvl_core::Node::new(
            "evt",
            "Début",
            kvl_core::Category::Interactions,
            "démarrage",
            kvl_core::NodeKind::Event { event_type: "OnStart".into() },
        ));
        graph.add_node(kvl_core::Node::new(
            "act",
            "Score",
            kvl_core::Category::Progression,
            "ajoute du score",
            kvl_core::NodeKind::Action { action_type: "AddScore".into() },
        ));
        graph.add_connection(kvl_core::Connection::new(
            "conn_1", "evt", "out_trigger", "act", "in_trigger",
        ));

        set_kvl_node_param(
            &mut project,
            &TargetRef::KvlNodeId("evt".into()),
            "position_x",
            &PropertyValue::Int(120),
        )
        .unwrap();
        assert_eq!(
            project.kvl_graphs["main"].nodes["evt"].ui.position_x,
            120.0
        );

        let ops = vec![AiOp::DeleteKvlNode { target: TargetRef::KvlNodeId("evt".into()) }];
        let proposal = build_proposal_headless(&project, "int_2", ops, "supprimer l'événement").unwrap();
        assert_eq!(proposal.risk, RiskLevel::Destructive);
        let applied = apply_edit(&project, &proposal).unwrap();
        let graph = &applied.project.kvl_graphs["main"];
        assert!(!graph.nodes.contains_key("evt"));
        assert!(graph.connections.is_empty(), "les connexions du nœud partent avec lui");
    }

    #[test]
    fn remove_connection_by_id_searches_graphs() {
        let mut project = project();
        let mut graph = Graph::new("main", "Principal", "test");
        graph.add_connection(kvl_core::Connection::new(
            "conn_1", "a", "out_trigger", "b", "in_trigger",
        ));
        project.kvl_graphs.insert("main".to_string(), graph);

        remove_connection(&mut project, None, "conn_1").unwrap();
        assert!(project.kvl_graphs["main"].connections.is_empty());
        let err = remove_connection(&mut project, None, "conn_1").unwrap_err();
        assert!(matches!(err, EditError::NoTarget(_)));
    }

    #[test]
    fn batch_is_atomic_when_one_op_fails() {
        let mut project = project();
        let ops = vec![AiOp::Batch {
            ops: vec![
                AiOp::UpdateEntity {
                    target: TargetRef::EntityId(1),
                    patch: EntityPatch { color: Some(0x00DC2626), ..EntityPatch::default() },
                },
                AiOp::SetEntityProperty {
                    target: TargetRef::EntityId(1),
                    path: "propriete_inconnue".into(),
                    value: PropertyValue::Int(1),
                },
            ],
        }];
        let before = project
            .scene
            .entities
            .iter()
            .map(|entity| entity.clone())
            .collect::<Vec<_>>();
        let err = execute(&project, &ops).unwrap_err();
        assert!(matches!(err, EditError::Op { .. }), "obtenu {err:?}");
        // Tout ou rien : la première op valide n'a pas été conservée.
        let unchanged = project.scene.entities == before;
        assert!(unchanged);
        // Et le projet reste utilisable après l'échec.
        let patch = EntityPatch { color: Some(0x00DC2626), ..EntityPatch::default() };
        apply_entity_patch(&mut project, &TargetRef::EntityId(1), &patch).unwrap();
        assert_eq!(project.scene.entities[0].renderable.as_ref().unwrap().color, 0x00DC2626);
    }

    #[test]
    fn proposal_on_selection_binds_target_and_previews_diff() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        let ops = update_ops(EntityPatch {
            color: Some(0x00DC2626),
            speed: Some(160.0),
            ..EntityPatch::default()
        });
        let proposal = build_proposal(&project, &ctx, "int_1", ops, "plus rapide et rouge").unwrap();
        assert_eq!(proposal.target.entity_ids, vec![1]);
        assert_eq!(proposal.risk, RiskLevel::Safe);
        assert!(proposal.id.starts_with("prop_"));
        assert!(!proposal.hash.is_empty());
        let fields: Vec<&str> = proposal
            .preview
            .changes
            .iter()
            .map(|change| change.field.as_str())
            .collect();
        assert!(fields.contains(&"couleur"));
        assert!(fields.contains(&"vitesse"));
        // Les ops stockées sont concrètes (plus de `Selected`).
        match &proposal.ops[0] {
            AiOp::UpdateEntity { target, .. } => assert_eq!(target, &TargetRef::EntityId(1)),
            other => panic!("attendu UpdateEntity, obtenu {other:?}"),
        }
    }

    #[test]
    fn same_value_is_an_error_not_a_silent_noop() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        let ops = update_ops(EntityPatch { speed: Some(110.0), ..EntityPatch::default() });
        let err = build_proposal(&project, &ctx, "int", ops, "même vitesse").unwrap_err();
        assert!(matches!(err, EditError::NoChange), "obtenu {err:?}");
    }

    #[test]
    fn apply_then_undo_restores_everything() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        let ops = vec![
            AiOp::UpdateEntity {
                target: TargetRef::Selected,
                patch: EntityPatch { name: Some("Boss_Ancien".into()), ..EntityPatch::default() },
            },
            AiOp::AddComponent { target: TargetRef::EntityId(1), component: "vision".into() },
        ];
        let proposal = build_proposal(&project, &ctx, "int", ops, "renommer et voir").unwrap();
        assert_eq!(proposal.risk, RiskLevel::Safe);
        let applied = apply_edit(&project, &proposal).unwrap();
        assert_eq!(applied.project.scene.entities[0].name, "Boss_Ancien");
        assert!(applied.project.scene.entities[0].vision.is_some());
        assert_eq!(applied.transaction.touched_entities(), 1);

        let undone = undo(&applied.project, &applied.transaction).unwrap();
        assert_eq!(undone.scene.entities[0].name, "Boss");
        assert!(undone.scene.entities[0].vision.is_none());
        assert_eq!(undone.scene.entities.len(), project.scene.entities.len());
    }

    #[test]
    fn undo_restores_deleted_entities_in_order() {
        let project = project();
        let ops = vec![AiOp::DeleteEntity { targets: vec![TargetRef::EntityId(1)] }];
        let proposal = build_proposal_headless(&project, "int", ops, "supprimer le boss").unwrap();
        let applied = apply_edit(&project, &proposal).unwrap();
        assert_eq!(applied.project.scene.entities[0].id, 2);
        let undone = undo(&applied.project, &applied.transaction).unwrap();
        assert_eq!(undone.scene.entities[0].id, 1, "l'ordre d'origine est restauré");
        assert_eq!(undone.scene.entities[1].id, 2);
    }

    #[test]
    fn undo_removes_created_entities() {
        let project = project();
        let ops = vec![AiOp::DuplicateEntity {
            targets: vec![TargetRef::EntityId(1)],
            offset: [10.0, 10.0],
        }];
        let proposal = build_proposal_headless(&project, "int", ops, "dupliquer").unwrap();
        let applied = apply_edit(&project, &proposal).unwrap();
        assert_eq!(applied.project.scene.entities.len(), 3);
        let undone = undo(&applied.project, &applied.transaction).unwrap();
        assert_eq!(undone.scene.entities.len(), 2);
    }

    #[test]
    fn stale_proposal_is_refused_after_target_removal() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(2)]);
        let ops = update_ops(EntityPatch { color: Some(0x00DC2626), ..EntityPatch::default() });
        let proposal = build_proposal(&project, &ctx, "int", ops, "rouge").unwrap();

        // Le Studio supprime la cible entre l'aperçu et l'application.
        let mut edited = project.clone();
        edited.scene.entities.retain(|entity| entity.id != 2);
        let err = apply_edit(&edited, &proposal).unwrap_err();
        assert!(matches!(err, EditError::Op { .. }), "obtenu {err:?}");
    }

    #[test]
    fn tampered_proposal_is_stale() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        let ops = update_ops(EntityPatch { speed: Some(160.0), ..EntityPatch::default() });
        let mut proposal = build_proposal(&project, &ctx, "int", ops, "plus rapide").unwrap();
        proposal.ops.push(AiOp::DeleteEntity { targets: vec![TargetRef::EntityId(1)] });
        let err = apply_edit(&project, &proposal).unwrap_err();
        assert!(matches!(err, EditError::Stale));
    }

    #[test]
    fn proposal_hash_is_deterministic() {
        let project = project();
        let ctx = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        let ops = || update_ops(EntityPatch { color: Some(0x00DC2626), ..EntityPatch::default() });
        let first = build_proposal(&project, &ctx, "int", ops(), "rouge").unwrap();
        let second = build_proposal(&project, &ctx, "int", ops(), "rouge").unwrap();
        assert_eq!(first.hash, second.hash);
        assert_eq!(first.id, second.id);
    }

    #[test]
    fn contextual_target_reaches_executor_unresolved() {
        let mut project = project();
        let op = AiOp::UpdateEntity {
            target: TargetRef::Selected,
            patch: EntityPatch { color: Some(0x00FF0000), ..EntityPatch::default() },
        };
        let err = crate::ai_executor::apply_op(&mut project, &op).unwrap_err();
        assert!(err.contains("contextuelle"), "obtenu {err}");
    }

    #[test]
    fn scene_setting_objective_is_editable() {
        let mut project = project();
        set_scene_setting(
            &mut project,
            "objective",
            &PropertyValue::String("ATTRAPEZ LE BOSS".to_string()),
        )
        .unwrap();
        assert_eq!(project.scene.objective.as_deref(), Some("ATTRAPEZ LE BOSS"));
        set_scene_setting(&mut project, "objective", &PropertyValue::String(String::new())).unwrap();
        assert!(project.scene.objective.is_none());
        let err = set_scene_setting(&mut project, "gravity", &PropertyValue::Float(1.0)).unwrap_err();
        assert!(err.to_string().contains("réglage de scène inconnu"));
    }

    #[test]
    fn risk_of_batch_is_the_maximum() {
        let ops = vec![
            AiOp::UpdateEntity { target: TargetRef::EntityId(1), patch: EntityPatch::default() },
            AiOp::DeleteEntity { targets: vec![TargetRef::EntityId(2)] },
        ];
        assert_eq!(risk_of_ops(&ops), RiskLevel::Destructive);
        assert_eq!(risk_of_ops(&ops[..1]), RiskLevel::Safe);
    }
}
