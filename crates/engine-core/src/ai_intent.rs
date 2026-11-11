//! Intention normalisée : la demande utilisateur transformée en contrat structuré.
//!
//! Le classifieur est volontairement **léger et déterministe** (mots-clés). En cas
//! d'ambiguïté sur la dimension ou le genre, `needs_clarification` passe à `true` pour
//! que le Studio pose une question au lieu de deviner.

use serde::{Deserialize, Serialize};

use crate::ai_dictionary::hash_text;
use crate::recipe::{GameDimension, GameGenre};

/// Cible de génération demandée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentTarget {
    Project,
    Scene,
    Level,
    Mechanic,
    Asset,
    FullGame,
    Tweak,
}

/// Contraintes bornées de l'intention.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntentConstraints {
    pub levels: Option<u32>,
    pub max_entities: Option<u32>,
    pub difficulty: Option<String>,
    pub art_direction: Option<String>,
    pub must_include: Vec<String>,
}

/// Budget d'exécution d'une intention.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntentBudget {
    pub max_steps: u32,
    pub max_tokens: u32,
}

impl Default for IntentBudget {
    fn default() -> Self {
        Self {
            max_steps: 12,
            max_tokens: 24_000,
        }
    }
}

/// Enveloppe d'intention normalisée.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntentEnvelope {
    pub intent_id: String,
    pub raw_text: String,
    pub locale: String,
    pub target: IntentTarget,
    pub dimension: Option<GameDimension>,
    pub genre: Option<GameGenre>,
    pub constraints: IntentConstraints,
    pub acceptance: Vec<String>,
    pub budget: IntentBudget,
    pub needs_clarification: bool,
}

/// Normalise un texte libre en intention structurée.
pub fn normalize_intent(raw_text: &str) -> IntentEnvelope {
    let lower = raw_text.to_lowercase();

    let dimension_hits = count_dimension_hits(&lower);
    let genre_hits = count_genre_hits(&lower);

    let dimension = if dimension_hits > 1 {
        None
    } else {
        detect_dimension(&lower)
    };
    let genre = if genre_hits > 1 {
        None
    } else {
        detect_genre(&lower)
    };

    IntentEnvelope {
        intent_id: format!("int_{}", hash_text(raw_text.as_bytes())),
        raw_text: raw_text.to_string(),
        locale: "fr".to_string(),
        target: detect_target(&lower),
        dimension,
        genre,
        constraints: IntentConstraints::default(),
        acceptance: Vec::new(),
        budget: IntentBudget::default(),
        needs_clarification: dimension_hits > 1 || genre_hits > 1,
    }
}

fn detect_target(lower: &str) -> IntentTarget {
    if lower.contains("jeu complet") || lower.contains("full game") || lower.contains("jeu jouable") {
        IntentTarget::FullGame
    } else if lower.contains("projet") {
        IntentTarget::Project
    } else if lower.contains("scène") || lower.contains("scene") {
        IntentTarget::Scene
    } else if lower.contains("mécanique") || lower.contains("mechanic") || lower.contains("mécanisme") {
        IntentTarget::Mechanic
    } else if lower.contains("asset") || lower.contains("sprite") || lower.contains("musique") {
        IntentTarget::Asset
    } else if lower.contains("modifie") || lower.contains("change") || lower.contains("tweak") || lower.contains("corrige") {
        IntentTarget::Tweak
    } else {
        IntentTarget::Level
    }
}

fn detect_dimension(lower: &str) -> Option<GameDimension> {
    if lower.contains("2.5d") || lower.contains("2,5d") || lower.contains("2.5 d") {
        Some(GameDimension::TwoAndHalfD)
    } else if lower.contains("3d") {
        Some(GameDimension::ThreeD)
    } else if lower.contains("2d") {
        Some(GameDimension::TwoD)
    } else {
        None
    }
}

fn count_dimension_hits(lower: &str) -> usize {
    [
        lower.contains("3d"),
        lower.contains("2.5d") || lower.contains("2,5d"),
        lower.contains("2d"),
    ]
    .iter()
    .filter(|hit| **hit)
    .count()
}

fn detect_genre(lower: &str) -> Option<GameGenre> {
    if lower.contains("plateforme") || lower.contains("platformer") {
        Some(GameGenre::Platformer)
    } else if lower.contains("rpg") || lower.contains("rôle") || lower.contains("role") {
        Some(GameGenre::Rpg)
    } else if lower.contains("shmup") || lower.contains("shoot'em") || lower.contains("shoot-em") || lower.contains("shoot em") {
        Some(GameGenre::ShootEmUp)
    } else if lower.contains("puzzle") || lower.contains("sokoban") || lower.contains("énigme") || lower.contains("enigme") {
        Some(GameGenre::Puzzle)
    } else if lower.contains("horreur") || lower.contains("horror") {
        Some(GameGenre::Horror)
    } else if lower.contains("course") || lower.contains("racing") {
        Some(GameGenre::Racing)
    } else if lower.contains("aventure") || lower.contains("adventure") {
        Some(GameGenre::Adventure)
    } else {
        None
    }
}

fn count_genre_hits(lower: &str) -> usize {
    [
        lower.contains("plateforme") || lower.contains("platformer"),
        lower.contains("rpg") || lower.contains("rôle"),
        lower.contains("shmup") || lower.contains("shoot"),
        lower.contains("puzzle") || lower.contains("sokoban") || lower.contains("énigme"),
        lower.contains("horreur") || lower.contains("horror"),
        lower.contains("course") || lower.contains("racing"),
        lower.contains("aventure") || lower.contains("adventure"),
    ]
    .iter()
    .filter(|hit| **hit)
    .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_detects_target_dimension_and_genre() {
        let e = normalize_intent("Crée un jeu complet de plateforme 2D");
        assert_eq!(e.target, IntentTarget::FullGame);
        assert_eq!(e.dimension, Some(GameDimension::TwoD));
        assert_eq!(e.genre, Some(GameGenre::Platformer));
        assert!(!e.needs_clarification);
    }

    #[test]
    fn normalize_asks_clarification_when_dimension_ambiguous() {
        let e = normalize_intent("Je veux un niveau 3D ou 2D ?");
        assert!(e.needs_clarification);
        assert_eq!(e.dimension, None);
    }

    #[test]
    fn normalize_asks_clarification_when_genre_ambiguous() {
        let e = normalize_intent("Un RPG ou un puzzle ?");
        assert!(e.needs_clarification);
        assert_eq!(e.genre, None);
    }

    #[test]
    fn normalize_is_deterministic() {
        let a = normalize_intent("Crée un jeu complet de plateforme 2D");
        let b = normalize_intent("Crée un jeu complet de plateforme 2D");
        assert_eq!(a.intent_id, b.intent_id);
    }
}

