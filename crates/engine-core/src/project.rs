use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::recipe::{GameGenre, KaractiGameRecipe, RecipeProjectGenerator, DependencyResolver};
use crate::scene::SceneData;
use crate::entity::{Entity, Transform2D, Collider2D, Movement2D, MovementMode, Health2D};

pub const PROJECT_SCHEMA_VERSION: u32 = 1;
pub const PROJECT_EXTENSION: &str = "karacti";

/// Métadonnées d'identification et de traçabilité du projet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub id: String,
    pub title: String,
    pub author: String,
    pub version: String,
    pub created_at: String,
    pub description: String,
    pub genre_primary: GameGenre,
    #[serde(default)]
    pub genres_secondary: Vec<GameGenre>,
    /// Dimension du rendu (« 2D », « 2.5D », « 3D ») — rétrocompatible (`None` = 2D).
    #[serde(default)]
    pub dimension: Option<String>,
}

/// Paramètres techniques de rendu, physique et adaptabilité
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub target_fps: u32,
    pub software_width: u32,
    pub software_height: u32,
    pub gravity: f32,
    pub performance_profile: String,
    pub show_minimap: bool,
    pub default_mode: String, // "play" ou "edit"
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            target_fps: 60,
            software_width: 1200,
            software_height: 800,
            gravity: 980.0,
            performance_profile: "LowEnd".to_string(),
            show_minimap: true,
            default_mode: "edit".to_string(), // Un studio démarre en mode édition par défaut
        }
    }
}

/// Structure racine d'un projet souverain KaractiRage (.karacti)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KaractiProject {
    pub schema_version: u32,
    pub metadata: ProjectMetadata,
    pub settings: ProjectSettings,
    pub recipe: Option<KaractiGameRecipe>,
    pub scene: SceneData,
    pub kvl_graphs: HashMap<String, kvl_core::Graph>,
    #[serde(default)]
    pub kvl_variables: HashMap<String, kvl_core::Value>,
}

impl KaractiProject {
    /// Crée un projet vierge pour démarrer la conception dans l'Atelier Studio
    pub fn new_empty(title: &str, author: &str, genre: GameGenre) -> Self {
        let proj_id = format!("proj_{:x}", (title.len() as u64) ^ 0xFEEDFACE);
        
        let metadata = ProjectMetadata {
            id: proj_id,
            title: title.to_string(),
            author: author.to_string(),
            version: "0.1.0".to_string(),
            created_at: "2026-10-17T00:00:00Z".to_string(),
            description: format!("Nouveau projet vierge {} créé dans KaractiRage Studio.", genre.label()),
            genre_primary: genre,
            genres_secondary: Vec::new(),
            dimension: None,
        };

        let mut settings = ProjectSettings::default();
        settings.default_mode = "edit".to_string(); // Studio direct

        // 1. Joueur de base selon le genre choisi
        let (move_mode, gravity) = match genre {
            GameGenre::Platformer => (
                MovementMode::Platformer {
                    gravity: 980.0,
                    jump_force: 420.0,
                    is_grounded: false,
                    max_fall_speed: 650.0,
                },
                980.0,
            ),
            GameGenre::ShootEmUp => (
                MovementMode::SpaceInertia {
                    angular_speed: 4.0,
                    thrust: 400.0,
                    drag: 0.98,
                },
                0.0,
            ),
            GameGenre::Puzzle => (
                MovementMode::Grid {
                    cell_size: 32.0,
                    cooldown: 0.18,
                    timer: 0.0,
                },
                0.0,
            ),
            _ => (MovementMode::TopDown, 0.0),
        };
        settings.gravity = gravity;

        let player_transform = Transform2D::new(600.0, 400.0, 36.0, 36.0);
        let mut player = Entity::new(1, "Héros_Joueur".to_string(), "player", player_transform);
        player.collider = Some(Collider2D::solid(36.0, 36.0));
        player.movement = Some(Movement2D {
            speed: 240.0,
            is_player_controlled: true,
            mode: move_mode,
            vx: 0.0,
            vy: 0.0,
            patrol: None,
        });
        player.health = Some(Health2D::new(5.0));

        // 2. Grille de sol de référence (plateforme ou sol initial pour repère visuel)
        let ground_transform = Transform2D::new(200.0, 520.0, 800.0, 40.0);
        let mut ground = Entity::new(2, "Sol_De_Reference".to_string(), "platform", ground_transform);
        ground.collider = Some(Collider2D::solid(800.0, 40.0));

        let scene = SceneData {
            name: title.to_string(),
            description: format!("Scène vierge pour {}", title),
            objective: Some("CONCEVEZ VOTRE MONDE DANS L'ATELIER STUDIO (TOUCHE TAB)".to_string()),
            entities: vec![player, ground],
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        };

        // Graphe KVL racine initial
        let mut root_graph = kvl_core::Graph::new("root_logic", "Logique Racine", "Graph KVL initial");
        root_graph.add_variable("score", kvl_core::Value::Int(0));

        let mut kvl_graphs = HashMap::new();
        kvl_graphs.insert("root_logic".to_string(), root_graph);

        let mut kvl_variables = HashMap::new();
        kvl_variables.insert("score".to_string(), kvl_core::Value::Int(0));

        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            metadata,
            settings,
            recipe: None,
            scene,
            kvl_graphs,
            kvl_variables,
        }
    }

    /// Génère un projet jouable complet à partir d'une recette déclarative
    pub fn from_recipe(mut recipe: KaractiGameRecipe) -> Self {
        // 1. Résolution stricte des dépendances (Règles R1 à R6)
        DependencyResolver::resolve(&mut recipe);

        // 2. Génération de la scène riche et des graphes KVL
        let (scene, kvl_graphs) = RecipeProjectGenerator::generate(&recipe);

        let proj_id = format!("proj_{:x}", (recipe.title.len() as u64) ^ 0xBADC0FFE);
        let metadata = ProjectMetadata {
            id: proj_id,
            title: recipe.title.clone(),
            author: recipe.author.clone(),
            version: "1.0.0".to_string(),
            created_at: "2026-10-17T00:00:00Z".to_string(),
            description: format!("Projet '{}' basé sur le genre {}.", recipe.title, recipe.genres.primary.label()),
            genre_primary: recipe.genres.primary,
            genres_secondary: recipe.genres.secondary.clone(),
            dimension: Some(match recipe.dimension {
                crate::recipe::GameDimension::TwoD => "2D",
                crate::recipe::GameDimension::TwoAndHalfD => "2.5D",
                crate::recipe::GameDimension::ThreeD => "3D",
            }
            .to_string()),
        };

        let mut settings = ProjectSettings::default();
        settings.gravity = if recipe.world.gravity_enabled { 980.0 } else { 0.0 };
        settings.default_mode = "play".to_string(); // Prêt à jouer

        let mut kvl_variables = HashMap::new();
        kvl_variables.insert("score".to_string(), kvl_core::Value::Int(0));

        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            metadata,
            settings,
            recipe: Some(recipe),
            scene,
            kvl_graphs,
            kvl_variables,
        }
    }

    /// Instancie un projet complet à partir d'un identifiant d'archétype officiel
    pub fn from_archetype(archetype_id: &str) -> Option<Self> {
        let genre = match archetype_id {
            "rpg" => GameGenre::Adventure,
            "platformer" => GameGenre::Platformer,
            "shmup" => GameGenre::ShootEmUp,
            "sokoban" => GameGenre::Puzzle,
            "boss" => GameGenre::Combat,
            _ => return None,
        };

        let title = match archetype_id {
            "rpg" => "Donjon RPG — La Crypte Émeraude",
            "platformer" => "Odyssée Plateforme — Terres Volantes",
            "shmup" => "Invasion Stellaire — Néon Cosmos",
            "sokoban" => "Labyrinthe Quantique — Énigmes de Blocs",
            "boss" => "Arène Titan — Duel contre le Colosse",
            _ => "Projet KaractiRage",
        };

        let recipe = KaractiGameRecipe::new(title, "Atelier KaractiRage", genre);
        Some(Self::from_recipe(recipe))
    }

    /// Sérialise le projet en chaîne JSON universelle
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Désérialise un projet depuis une chaîne JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Sauvegarde le projet complet dans un fichier `.karacti`
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.to_json()?;
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path_ref, json)?;
        Ok(())
    }

    /// Charge un projet complet depuis un fichier `.karacti`
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let project = Self::from_json(&content)?;
        Ok(project)
    }

    /// Génère le package standalone HTML5 souverain autonome du projet
    pub fn to_standalone_html(&self) -> String {
        let package = crate::exporter::KaractiGamePackage {
            metadata: crate::exporter::PackageMetadata {
                title: self.metadata.title.clone(),
                author: self.metadata.author.clone(),
                version: self.metadata.version.clone(),
                genre: self.metadata.genre_primary.label().to_string(),
                created_at: self.metadata.created_at.clone(),
                target_fps: self.settings.target_fps,
                screen_width: self.settings.software_width,
                screen_height: self.settings.software_height,
                play_mode_only: true,
            },
            scene_name: self.scene.name.clone(),
            scene_objective: self.scene.objective.clone(),
            entities: self.scene.entities.clone(),
            kvl_graphs: self.kvl_graphs.clone(),
            initial_variables: self.kvl_variables.clone(),
            sprite_assets: std::collections::HashMap::new(),
            sheet_assets: std::collections::HashMap::new(),
        };
        package.generate_standalone_html()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_project_creation_and_defaults() {
        let proj = KaractiProject::new_empty("Nouveau Jeu", "Dev", GameGenre::Adventure);
        assert_eq!(proj.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(proj.metadata.title, "Nouveau Jeu");
        assert_eq!(proj.settings.default_mode, "edit");
        assert!(proj.scene.entities.iter().any(|e| e.tag == "player"));
        assert!(proj.scene.entities.iter().any(|e| e.tag == "platform"));
        assert!(proj.kvl_graphs.contains_key("root_logic"));
    }

    #[test]
    fn test_project_json_roundtrip_fidelity() {
        let proj = KaractiProject::new_empty("Roundtrip", "Auteur", GameGenre::Platformer);
        let json = proj.to_json().expect("Sérialisation JSON");
        let loaded = KaractiProject::from_json(&json).expect("Désérialisation JSON");

        assert_eq!(loaded.metadata.title, proj.metadata.title);
        assert_eq!(loaded.metadata.genre_primary, GameGenre::Platformer);
        assert_eq!(loaded.scene.entities.len(), proj.scene.entities.len());
        assert_eq!(loaded.settings.gravity, 980.0);
    }

    #[test]
    fn test_project_from_all_five_archetypes() {
        for arch in &["rpg", "platformer", "shmup", "sokoban", "boss"] {
            let p = KaractiProject::from_archetype(arch)
                .unwrap_or_else(|| panic!("Archétype '{}' introuvable", arch));
            assert!(!p.scene.entities.is_empty(), "L'archétype {} doit avoir des entités", arch);
            assert!(p.scene.entities.iter().any(|e| e.tag == "player"), "L'archétype {} doit avoir un joueur", arch);
            assert!(p.recipe.is_some(), "L'archétype {} doit contenir sa recette déclarative", arch);
        }
    }

    #[test]
    fn test_project_save_and_load_file() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_souverain_project.karacti");

        let original = KaractiProject::new_empty("Sauvegarde Test", "Studio KR", GameGenre::ShootEmUp);
        original.save_to_file(&file_path).expect("Sauvegarde réussie");

        let loaded = KaractiProject::load_from_file(&file_path).expect("Chargement réussi");
        assert_eq!(loaded.metadata.title, "Sauvegarde Test");
        assert_eq!(loaded.metadata.genre_primary, GameGenre::ShootEmUp);

        let _ = fs::remove_file(file_path);
    }
}
