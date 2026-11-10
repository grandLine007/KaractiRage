//! Résolveur déterministe d'instructions micro-édition (« 60-80 % sans LLM »).
//!
//! Avant tout appel LLM, une instruction courte est confrontée à une table de règles
//! fermée. Objectif : latence < 100 ms, zéro hallucination, zéro no-op silencieux.
//!
//! Règles de conception :
//! - une règle ne s'applique que si elle est **certaine** ; sinon on renvoie `None` et le
//!   LLM prend le relais (le template ne devine jamais) ;
//! - une cible absente n'est pas devinée : on renvoie une demande de clarification ;
//! - une édition **relative** (« plus rapide », « 2× plus grand ») exige une cible unique,
//!   sinon la multiplication serait la même pour tous les éléments sélectionnés ;
//! - les opérations que le moteur ne sait pas encore exprimer correctement (position d'une
//!   lumière, par exemple) ne sont **pas** produites : mieux vaut `None` qu'une op fausse.

use crate::ai_edit::{EditContext, EntityPatch, MaterialPatch, TargetRef, resolve_target};
use crate::ai_plan::AiOp;
use crate::entity::Entity;
use crate::project::KaractiProject;

/// Décision du résolveur.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// Ops prêtes à être empaquetées dans une proposition.
    Ops(Vec<AiOp>),
    /// La demande est compréhensible mais incomplète : il faut préciser.
    NeedsClarification(String),
}

/// Aire de la palette nommée : mot français → teinte `0x00RRGGBB`.
///
/// Volontairement bornée (17 entrées) : une couleur inventée serait une hallucination.
pub const NAMED_COLORS: &[(&str, u32)] = &[
    ("rouge", 0x00DC2626),
    ("bleu", 0x002563EB),
    ("vert", 0x0016A34A),
    ("jaune", 0x00EAB308),
    ("orange", 0x00EA580C),
    ("violet", 0x007C3AED),
    ("rose", 0x00EC4899),
    ("noir", 0x00000000),
    ("blanc", 0x00FFFFFF),
    ("gris", 0x0064748B),
    ("cyan", 0x0006B6D4),
    ("magenta", 0x00C026D3),
    ("marron", 0x0092400E),
    ("or", 0x00F5C542),
    ("argent", 0x00CBD5E1),
    ("turquoise", 0x0014B8A6),
    ("indigo", 0x004F46E5),
];

/// Résout une couleur nommée ou une teinte hexadécimale (`#DC2626`).
pub fn named_color(word: &str) -> Option<u32> {
    let word = word.trim();
    if let Some(hex) = word.strip_prefix('#') {
        if hex.len() == 6 {
            return u32::from_str_radix(hex, 16).ok().map(|value| value & 0x00FF_FFFF);
        }
    }
    NAMED_COLORS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, value)| *value)
}

/// Décalage monde appliqué par une direction cardinale relative.
pub const NUDGE: f32 = 32.0;

/// Tente de résoudre une instruction sans LLM.
///
/// `None` signifie « aucune règle ne s'applique avec certitude » : l'appelant doit alors
/// passer par le prompt micro et le modèle.
pub fn resolve_instruction(
    instruction: &str,
    project: &KaractiProject,
    ctx: &EditContext,
) -> Option<Resolution> {
    // Le trait d'union sépare (« supprime-le », « déplace-le ») ; `#` reste collé
    // pour les couleurs hexadécimales.
    let raw: Vec<&str> = instruction
        .split(|c: char| !c.is_alphanumeric() && c != '#')
        .filter(|token| !token.is_empty())
        .collect();
    if raw.is_empty() {
        return None;
    }
    let norm: Vec<String> = raw.iter().map(|token| normalize(token)).collect();
    let has = |needle: &str| norm.iter().any(|token| token == needle);
    let has_pair = |first: &str, second: &str| {
        norm.windows(2)
            .any(|pair| pair[0] == first && pair[1] == second)
    };
    let selected = TargetRef::Selected;

    // ----- 1. Suppression (destructif) -----
    if has("supprime") || has("supprimer") || has("efface") || has("enleve") || has("retire") {
        return Some(match resolve_target(project, ctx, &selected) {
            Ok(_) => Resolution::Ops(vec![AiOp::DeleteEntity {
                targets: vec![selected],
            }]),
            Err(error) => Resolution::NeedsClarification(error.to_string()),
        });
    }

    // ----- 2. Duplication -----
    if has("duplique") || has("dupliquer") || has("copie") || has("cloner") {
        const OFFSET: [f32; 2] = [32.0, 32.0];
        return Some(match resolve_target(project, ctx, &selected) {
            Ok(_) => Resolution::Ops(vec![AiOp::DuplicateEntity {
                targets: vec![selected],
                offset: OFFSET,
            }]),
            Err(error) => Resolution::NeedsClarification(error.to_string()),
        });
    }

    // ----- 3. Déplacement -----
    if has("deplace") || has("deplacer") || has("bouge") || has("bouger") || has("place") {
        return Some(resolve_move(&raw, &norm, project, ctx, &selected));
    }

    // ----- 4. Renommage -----
    if let Some(index) = norm
        .iter()
        .position(|token| token == "renomme" || token == "renommer" || token == "appelle")
    {
        let after: Vec<&str> = raw
            .iter()
            .skip(index + 1)
            .copied()
            .filter(|token| {
                let token = normalize(token);
                token != "le" && token != "la" && token != "en" && token != "ce" && token != "cette"
            })
            .collect();
        if after.is_empty() {
            return Some(Resolution::NeedsClarification(
                "Renommer en quoi ? Précise le nouveau nom.".to_string(),
            ));
        }
        let name = after.join(" ");
        let patch = EntityPatch {
            name: Some(name),
            ..EntityPatch::default()
        };
        return Some(match resolve_target(project, ctx, &selected) {
            Ok(_) => Resolution::Ops(vec![AiOp::UpdateEntity {
                target: selected,
                patch,
            }]),
            Err(error) => Resolution::NeedsClarification(error.to_string()),
        });
    }

    // ----- 5. Attributs combinables (couleur + vitesse + taille + visibilité + matériau) -----
    let color = norm.iter().find_map(|token| named_color(token));
    let faster = has_pair("plus", "rapide") || has("accelere") || has("accelerer");
    let slower = has_pair("plus", "lent") || has_pair("plus", "lente") || has("ralentis");
    let bigger = has_pair("plus", "grand") || has_pair("plus", "grande") || has("agrandis");
    let smaller = has_pair("plus", "petit") || has_pair("plus", "petite") || has("reduis");
    let hide = has("masque") || has("cache") || has("invisible");
    let show = has("affiche") || has("montre") || has("visible");
    let metallic = has("metallique") || has("metal") || has("chrome");
    let relative = faster || slower || bigger || smaller;
    if !(color.is_some() || relative || hide || show || metallic) {
        return None;
    }

    let mut ids = match resolve_target(project, ctx, &selected) {
        Ok(resolved) => resolved.entity_ids,
        Err(error) => return Some(Resolution::NeedsClarification(error.to_string())),
    };

    // Une édition relative n'a de sens que sur une cible unique (sinon facteur commun).
    let id = if relative {
        if ids.len() != 1 {
            return Some(Resolution::NeedsClarification(format!(
                "Cette édition est relative à un seul élément, mais {} sont sélectionnés. \
                 Sélectionne-en un seul.",
                ids.len()
            )));
        }
        ids.pop().expect("un identifiant")
    } else {
        0
    };

    let entity = if relative {
        match project.scene.entities.iter().find(|candidate| candidate.id == id) {
            Some(entity) => Some(entity),
            None => return None,
        }
    } else {
        None
    };

    let mut patch = EntityPatch {
        color,
        ..EntityPatch::default()
    };
    if let Some(entity) = entity {
        if faster || slower {
            let speed = entity.movement.as_ref().map(|movement| movement.speed);
            let Some(speed) = speed else {
                return Some(Resolution::NeedsClarification(format!(
                    "« {} » n'a pas de composant movement : ajoute-le d'abord (AddComponent).",
                    entity.name
                )));
            };
            let factor = if faster { 1.5 } else { 0.6 };
            patch.speed = Some(round_to(speed * factor, 1.0));
        }
        if bigger || smaller {
            let factor = if bigger { 1.25 } else { 0.8 };
            patch.w = Some(round_to(entity.transform.w * factor, 1.0));
            patch.h = Some(round_to(entity.transform.h * factor, 1.0));
        }
    }
    if metallic {
        patch.material = Some(MaterialPatch {
            roughness: Some(0.2),
            metalness: Some(0.9),
            ..MaterialPatch::default()
        });
    }

    if patch.is_empty() && !(hide || show) {
        return None;
    }
    // `visible` vit dans le composant de rendu : c'est une op de propriété, pas un patch.
    let mut ops = Vec::new();
    if hide || show {
        ops.push(AiOp::SetEntityProperty {
            target: selected.clone(),
            path: "renderable.visible".to_string(),
            value: crate::ai_edit::PropertyValue::Bool(show),
        });
    }
    if !patch.is_empty() {
        ops.push(AiOp::UpdateEntity {
            target: selected,
            patch,
        });
    }
    Some(Resolution::Ops(ops))
}

fn resolve_move(
    raw: &[&str],
    norm: &[String],
    project: &KaractiProject,
    ctx: &EditContext,
    selected: &TargetRef,
) -> Resolution {
    let ids = match resolve_target(project, ctx, selected) {
        Ok(resolved) => resolved.entity_ids,
        Err(error) => return Resolution::NeedsClarification(error.to_string()),
    };
    if ids.len() != 1 {
        return Resolution::NeedsClarification(format!(
            "Le déplacement vise un seul élément, mais {} sont sélectionnés.",
            ids.len()
        ));
    }
    let id = ids[0];
    let Some(entity) = project.scene.entities.iter().find(|candidate| candidate.id == id) else {
        return Resolution::NeedsClarification(format!("L'entité #{id} est introuvable."));
    };

    // Coordonnées absolues : deux nombres dans l'instruction.
    let numbers: Vec<f32> = raw
        .iter()
        .filter_map(|token| token.replace(',', ".").parse::<f32>().ok())
        .collect();
    if numbers.len() >= 2 {
        return Resolution::Ops(vec![AiOp::SetTransform2D {
            target: selected.clone(),
            x: numbers[0],
            y: numbers[1],
            w: entity.transform.w,
            h: entity.transform.h,
        }]);
    }

    // « ici » : position du curseur fournie par le Studio.
    if norm.iter().any(|token| token == "ici") || norm.iter().any(|token| token == "curseur") {
        return match ctx.cursor_world {
            Some([x, y, _]) => Resolution::Ops(vec![AiOp::SetTransform2D {
                target: selected.clone(),
                x,
                y,
                w: entity.transform.w,
                h: entity.transform.h,
            }]),
            None => Resolution::NeedsClarification(
                "Le Studio ne fournit pas la position du curseur : clique dans le viewport \
                 ou donne des coordonnées (ex. « déplace en 320, 180 »)."
                    .to_string(),
            ),
        };
    }

    // Direction relative.
    let (dx, dy) = if norm.iter().any(|token| token == "gauche") {
        (-NUDGE, 0.0)
    } else if norm.iter().any(|token| token == "droite") {
        (NUDGE, 0.0)
    } else if norm.iter().any(|token| token == "haut") {
        (0.0, -NUDGE)
    } else if norm.iter().any(|token| token == "bas") {
        (0.0, NUDGE)
    } else {
        return Resolution::NeedsClarification(
            "Déplacer où ? Donne une direction (gauche, droite, haut, bas), des coordonnées \
             (ex. « en 320, 180 ») ou « ici »."
                .to_string(),
        );
    };
    Resolution::Ops(vec![AiOp::SetTransform2D {
        target: selected.clone(),
        x: entity.transform.x + dx,
        y: entity.transform.y + dy,
        w: entity.transform.w,
        h: entity.transform.h,
    }])
}

fn normalize(token: &str) -> String {
    token
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'œ' => 'o',
            other => other,
        })
        .collect()
}

fn round_to(value: f32, step: f32) -> f32 {
    (value / step).round() * step
}

/// Entité visée, pour les tests et les suggestions contextuelles.
pub fn entity_by_id<'a>(project: &'a KaractiProject, id: usize) -> Option<&'a Entity> {
    project.scene.entities.iter().find(|entity| entity.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Health2D, Movement2D, Renderable2D, Transform2D};
    use crate::recipe::GameGenre;

    fn project() -> KaractiProject {
        let mut project = KaractiProject::new_empty("Test", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();
        let mut boss = Entity::new(1, "Boss", "enemy", Transform2D::new(100.0, 100.0, 40.0, 40.0));
        boss.renderable = Some(Renderable2D::rect(0x00475569, 5));
        boss.movement = Some(Movement2D::new(110.0));
        boss.health = Some(Health2D::new(3.0));
        project.scene.entities.push(boss);
        let mut coin = Entity::new(2, "Pièce", "collectible", Transform2D::new(300.0, 50.0, 24.0, 24.0));
        coin.renderable = Some(Renderable2D::circle(12.0, 0x00F5C542, 5));
        project.scene.entities.push(coin);
        project
    }

    fn resolve(instruction: &str, ids: &[usize]) -> Option<Resolution> {
        let project = project();
        let context = EditContext::with_selection(
            &project,
            ids.iter().map(|id| TargetRef::EntityId(*id)).collect(),
        );
        resolve_instruction(instruction, &project, &context)
    }

    fn expect_update(instruction: &str, ids: &[usize]) -> EntityPatch {
        match resolve(instruction, ids) {
            Some(Resolution::Ops(ops)) => match &ops[0] {
                AiOp::UpdateEntity { patch, .. } => patch.clone(),
                other => panic!("attendu UpdateEntity, obtenu {other:?}"),
            },
            other => panic!("attendu des ops, obtenu {other:?}"),
        }
    }

    // Les 10 instructions du plan §5.2.
    #[test]
    fn instruction_01_color() {
        let patch = expect_update("mets ce boss en rouge", &[1]);
        assert_eq!(patch.color, Some(0x00DC2626));
    }

    #[test]
    fn instruction_02_faster() {
        let patch = expect_update("rends-le plus rapide", &[1]);
        assert_eq!(patch.speed, Some(165.0), "110 × 1.5 arrondi");
    }

    #[test]
    fn instruction_03_slower_and_smaller() {
        let patch = expect_update("un peu plus lent", &[1]);
        assert_eq!(patch.speed, Some(66.0), "110 × 0.6");
        let smaller = expect_update("rends-le plus petit", &[1]);
        assert_eq!(smaller.w, Some(32.0), "40 × 0.8");
        assert_eq!(smaller.h, Some(32.0));
    }

    #[test]
    fn instruction_04_duplicate() {
        match resolve("duplique cet élément", &[1]) {
            Some(Resolution::Ops(ops)) => match &ops[0] {
                AiOp::DuplicateEntity { offset, targets } => {
                    assert_eq!(*offset, [32.0, 32.0]);
                    assert_eq!(targets.len(), 1);
                }
                other => panic!("attendu DuplicateEntity, obtenu {other:?}"),
            },
            other => panic!("attendu des ops, obtenu {other:?}"),
        }
    }

    #[test]
    fn instruction_05_delete() {
        match resolve("supprime-le", &[1]) {
            Some(Resolution::Ops(ops)) => {
                assert!(matches!(ops[0], AiOp::DeleteEntity { .. }))
            }
            other => panic!("attendu une suppression, obtenu {other:?}"),
        }
    }

    #[test]
    fn instruction_06_move_absolute() {
        match resolve("déplace en 320, 180", &[1]) {
            Some(Resolution::Ops(ops)) => match &ops[0] {
                AiOp::SetTransform2D { x, y, w, h, .. } => {
                    assert_eq!((*x, *y), (320.0, 180.0));
                    assert_eq!((*w, *h), (40.0, 40.0), "la taille est conservée");
                }
                other => panic!("attendu SetTransform2D, obtenu {other:?}"),
            },
            other => panic!("attendu des ops, obtenu {other:?}"),
        }
    }

    #[test]
    fn instruction_07_move_relative_direction() {
        match resolve("déplace à droite", &[1]) {
            Some(Resolution::Ops(ops)) => match &ops[0] {
                AiOp::SetTransform2D { x, y, .. } => {
                    assert_eq!((*x, *y), (132.0, 100.0));
                }
                other => panic!("attendu SetTransform2D, obtenu {other:?}"),
            },
            other => panic!("attendu des ops, obtenu {other:?}"),
        }
    }

    #[test]
    fn instruction_08_move_here_needs_cursor() {
        match resolve("déplace ici", &[1]) {
            Some(Resolution::NeedsClarification(message)) => {
                assert!(message.contains("curseur"), "obtenu {message}")
            }
            other => panic!("attendu une clarification, obtenu {other:?}"),
        }
        // Avec un curseur fourni, l'op est produite.
        let project = project();
        let mut context = EditContext::with_selection(&project, vec![TargetRef::EntityId(1)]);
        context.cursor_world = Some([640.0, 360.0, 0.0]);
        match resolve_instruction("déplace ici", &project, &context) {
            Some(Resolution::Ops(ops)) => match &ops[0] {
                AiOp::SetTransform2D { x, y, .. } => assert_eq!((*x, *y), (640.0, 360.0)),
                other => panic!("attendu SetTransform2D, obtenu {other:?}"),
            },
            other => panic!("attendu des ops, obtenu {other:?}"),
        }
    }

    #[test]
    fn instruction_09_rename() {
        let patch = expect_update("renomme-le en Boss Suprême", &[1]);
        assert_eq!(patch.name.as_deref(), Some("Boss Suprême"));
    }

    #[test]
    fn instruction_10_metallic() {
        let patch = expect_update("rends-le métallique", &[1]);
        let material = patch.material.expect("matériau");
        assert_eq!(material.metalness, Some(0.9));
        assert_eq!(material.roughness, Some(0.2));
    }

    #[test]
    fn compound_color_and_speed_in_one_patch() {
        let patch = expect_update("rends ce boss plus rapide et rouge", &[1]);
        assert_eq!(patch.color, Some(0x00DC2626));
        assert_eq!(patch.speed, Some(165.0));
    }

    #[test]
    fn hex_color_is_accepted() {
        let patch = expect_update("couleur #16A34A", &[1]);
        assert_eq!(patch.color, Some(0x0016A34A));
    }

    #[test]
    fn missing_selection_asks_for_a_target_instead_of_inventing() {
        match resolve("mets-le en rouge", &[]) {
            Some(Resolution::NeedsClarification(message)) => {
                assert!(message.contains("sélection"), "obtenu {message}")
            }
            other => panic!("attendu une clarification, obtenu {other:?}"),
        }
    }

    #[test]
    fn relative_edit_refuses_multi_selection() {
        match resolve("plus rapide", &[1, 2]) {
            Some(Resolution::NeedsClarification(message)) => {
                assert!(message.contains("un seul"), "obtenu {message}")
            }
            other => panic!("attendu une clarification, obtenu {other:?}"),
        }
    }

    #[test]
    fn relative_speed_without_movement_component_is_explained() {
        match resolve("plus rapide", &[2]) {
            Some(Resolution::NeedsClarification(message)) => {
                assert!(message.contains("movement"), "obtenu {message}")
            }
            other => panic!("attendu une clarification, obtenu {other:?}"),
        }
    }

    #[test]
    fn unknown_instruction_defers_to_the_llm() {
        assert!(resolve("crée une mécanique de saut mural", &[1]).is_none());
        assert!(resolve("ajoute une lumière au-dessus du boss", &[1]).is_none());
        assert!(resolve("", &[1]).is_none());
    }

    #[test]
    fn nudge_constant_is_a_tile() {
        assert_eq!(NUDGE, 32.0);
    }

    #[test]
    fn entity_by_id_finds_the_target() {
        let project = project();
        assert_eq!(entity_by_id(&project, 2).map(|entity| entity.name.as_str()), Some("Pièce"));
        assert!(entity_by_id(&project, 99).is_none());
    }
}
