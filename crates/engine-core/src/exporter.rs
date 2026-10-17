use crate::entity::Entity;
use crate::game::GameState;
use kvl_core::{Graph, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Métadonnées descriptives du jeu exporté
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Titre du jeu
    pub title: String,
    /// Auteur ou studio créateur
    pub author: String,
    /// Version du jeu (ex: "1.0.0")
    pub version: String,
    /// Genre prédominant (Aventure, Platformer, Shmup, Sokoban, Boss Fight)
    pub genre: String,
    /// Horodatage de génération de l'export
    pub created_at: String,
    /// Taux de rafraîchissement cible (par défaut 60 FPS)
    pub target_fps: u32,
    /// Largeur de rendu native (1200 px)
    pub screen_width: u32,
    /// Hauteur de rendu native (800 px)
    pub screen_height: u32,
    /// Démarrage direct en mode joueur
    pub play_mode_only: bool,
}

impl Default for PackageMetadata {
    fn default() -> Self {
        Self {
            title: "Jeu KaractiRage".to_string(),
            author: "Créateur Indépendant".to_string(),
            version: "1.0.0".to_string(),
            genre: "Aventure & Donjon".to_string(),
            created_at: "2026-10-16".to_string(),
            target_fps: 60,
            screen_width: 1200,
            screen_height: 800,
            play_mode_only: true,
        }
    }
}

/// Package autonome universel (.karacti.json) embarquant scène et logique KVL
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KaractiGamePackage {
    /// Métadonnées du package
    pub metadata: PackageMetadata,
    /// Nom du niveau ou de la scène
    pub scene_name: String,
    /// Objectif narratif ou de victoire
    pub scene_objective: Option<String>,
    /// Liste complète des entités avec leurs composants ECS
    pub entities: Vec<Entity>,
    /// Table de tous les graphes KVL associés aux entités
    pub kvl_graphs: HashMap<String, Graph>,
    /// Variables globales initiales de contexte
    pub initial_variables: HashMap<String, Value>,
}

impl KaractiGamePackage {
    /// Convertit le package en chaîne JSON formatée
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convertit le package en chaîne JSON compacte
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Charge un package depuis une chaîne JSON
    #[allow(dead_code)]
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Génère un document HTML5 standalone complet intégrant le package et le lecteur Web
    pub fn generate_standalone_html(&self) -> String {
        let package_json = self.to_json().unwrap_or_else(|_| "{}".to_string());
        let escaped_json = package_json.replace("</script>", "<\\/script>");
        let template = include_str!("standalone_template.html");
        template
            .replace("{{TITLE}}", &self.metadata.title)
            .replace("{{AUTHOR}}", &self.metadata.author)
            .replace("{{GENRE}}", &self.metadata.genre)
            .replace("{{EMBEDDED_JSON}}", &escaped_json)
    }
}

/// Packageur et exportateur autonome de jeu KaractiRage
pub struct StandaloneExporter;

impl StandaloneExporter {
    /// Crée un package complet prêt pour l'exportation à partir de l'état actuel du jeu
    pub fn create_package(game: &GameState, title: &str, author: &str) -> KaractiGamePackage {
        let genre = match game.scene_name.to_lowercase().as_str() {
            s if s.contains("platformer") => "Plateforme 2D",
            s if s.contains("space") => "Space Shooter Arcade",
            s if s.contains("puzzle") => "Casse-Tête Sokoban",
            s if s.contains("boss") => "Combat de Boss Épique",
            _ => "Aventure & Donjon KVL",
        };

        let metadata = PackageMetadata {
            title: if title.trim().is_empty() { "Mon Jeu KaractiRage".to_string() } else { title.trim().to_string() },
            author: if author.trim().is_empty() { "Créateur Indépendant".to_string() } else { author.trim().to_string() },
            version: "1.0.0".to_string(),
            genre: genre.to_string(),
            created_at: "2026-10-16".to_string(),
            target_fps: 60,
            screen_width: 1200,
            screen_height: 800,
            play_mode_only: true,
        };

        // Extraction des variables de contexte KVL
        let mut initial_vars = HashMap::new();
        let score_val = game.kvl_context.get_var("score").map_or(0, |v| match v { Value::Int(n) => *n, _ => 0 });
        initial_vars.insert("score".to_string(), Value::Int(score_val));
        if let Some(keys) = game.kvl_context.get_var("keys") {
            initial_vars.insert("keys".to_string(), keys.clone());
        }

        KaractiGamePackage {
            metadata,
            scene_name: game.scene_name.clone(),
            scene_objective: game.scene_objective.clone(),
            entities: game.entities.clone(),
            kvl_graphs: game.kvl_graphs.clone(),
            initial_variables: initial_vars,
        }
    }

    /// Exporte le jeu en un fichier HTML standalone complet
    pub fn export_html(game: &GameState, title: &str, author: &str) -> String {
        let package = Self::create_package(game, title, author);
        package.generate_standalone_html()
    }

    /// Exporte le package en JSON universel
    pub fn export_package_json(game: &GameState, title: &str, author: &str) -> Result<String, serde_json::Error> {
        let package = Self::create_package(game, title, author);
        package.to_json_pretty()
    }

    /// Importe et applique un package de jeu dans l'état du jeu
    #[allow(dead_code)]
    pub fn import_package(game: &mut GameState, package: &KaractiGamePackage) {
        game.scene_name = package.scene_name.clone();
        game.scene_objective = package.scene_objective.clone();
        game.entities = package.entities.clone();
        game.kvl_graphs = package.kvl_graphs.clone();

        for (k, v) in &package.initial_variables {
            game.kvl_context.set_var(k.clone(), v.clone());
        }

        // Réinitialise la caméra et la sélection du Studio
        game.editor.selected = crate::editor::SelectedEntity::None;
        game.editor.mode = crate::editor::EditorMode::Play;
        game.set_status(format!("🎮 Jeu '{}' chargé avec succès ! ({} entités, {} graphes KVL)", package.metadata.title, package.entities.len(), package.kvl_graphs.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_metadata_serialization() {
        let meta = PackageMetadata::default();
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: PackageMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, deserialized);
        assert_eq!(deserialized.target_fps, 60);
        assert_eq!(deserialized.screen_width, 1200);
    }

    #[test]
    fn test_standalone_package_creation_and_json_roundtrip() {
        let mut graphs = HashMap::new();
        graphs.insert("script_door".to_string(), Graph::new("door_interaction", "Interactions Porte", "Logique de porte"));

        let package = KaractiGamePackage {
            metadata: PackageMetadata {
                title: "L'Éveil du Héros".to_string(),
                author: "Studio Karacti".to_string(),
                version: "1.0.0".to_string(),
                genre: "Aventure".to_string(),
                created_at: "2026-10-16".to_string(),
                target_fps: 60,
                screen_width: 1200,
                screen_height: 800,
                play_mode_only: true,
            },
            scene_name: "Donjon Oublié".to_string(),
            scene_objective: Some("Trouver la clé et vaincre le gardien".to_string()),
            entities: Vec::new(),
            kvl_graphs: graphs,
            initial_variables: HashMap::new(),
        };

        let json = package.to_json_pretty().expect("Sérialisation package");
        assert!(json.contains("L'Éveil du Héros"));
        assert!(json.contains("Studio Karacti"));

        let reloaded = KaractiGamePackage::from_json(&json).expect("Désérialisation package");
        assert_eq!(package, reloaded);
    }

    #[test]
    fn test_generate_standalone_html_content() {
        let package = KaractiGamePackage {
            metadata: PackageMetadata {
                title: "Test Quest".to_string(),
                author: "Tester".to_string(),
                version: "1.0.0".to_string(),
                genre: "Platformer".to_string(),
                created_at: "2026-10-16".to_string(),
                target_fps: 60,
                screen_width: 1200,
                screen_height: 800,
                play_mode_only: true,
            },
            scene_name: "Niveau 1".to_string(),
            scene_objective: None,
            entities: Vec::new(),
            kvl_graphs: HashMap::new(),
            initial_variables: HashMap::new(),
        };

        let html = package.generate_standalone_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Quest"));
        assert!(html.contains("Tester"));
        assert!(html.contains("canvas id=\"game-canvas\""));
        assert!(html.contains("id=\"embedded-package\""));
    }
}
