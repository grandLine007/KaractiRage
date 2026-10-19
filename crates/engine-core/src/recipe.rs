//! Système de Modèles de Jeu et Recettes Universelles (KaractiRage Game Recipe)
//!
//! Spécifié selon `systeme_de_mode_de_jeu.md` et `Fonctionnement et logique.md` :
//! 1. Modèle de données `KaractiGameRecipe` (sérialisable et versionné).
//! 2. Résolveur de dépendances automatique (règles de cohérence et détection de conflits).
//! 3. Générateur de scènes et projets immédiatement jouables selon les genres choisis.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::entity::{Entity, MovementMode, Transform2D, Collider2D, Movement2D, Health2D};
use crate::scene::SceneData;
use kvl_core::Graph;
use std::collections::HashMap;

/// Version du schéma des recettes KaractiRage
pub const RECIPE_SCHEMA_VERSION: u32 = 1;

/// Genres de jeu supportés par l'assistant de création
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameGenre {
    Adventure,
    Action,
    ShootEmUp,
    Platformer,
    Rpg,
    Simulation,
    Strategy,
    Racing,
    Puzzle,
    Horror,
    Combat,
    Educational,
    Custom,
}

impl GameGenre {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Adventure => "Aventure",
            Self::Action => "Action",
            Self::ShootEmUp => "Shoot'em up",
            Self::Platformer => "Plateforme",
            Self::Rpg => "Jeu de rôle (RPG)",
            Self::Simulation => "Simulation",
            Self::Strategy => "Stratégie",
            Self::Racing => "Course",
            Self::Puzzle => "Puzzle et réflexion",
            Self::Horror => "Horreur et survie",
            Self::Combat => "Combat",
            Self::Educational => "Jeu éducatif",
            Self::Custom => "Projet personnalisé",
        }
    }
}

/// Sélection de genres combinables (Genre principal + genres secondaires)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeGenres {
    pub primary: GameGenre,
    pub secondary: Vec<GameGenre>,
}

impl RecipeGenres {
    pub fn new(primary: GameGenre) -> Self {
        Self {
            primary,
            secondary: Vec::new(),
        }
    }

    pub fn with_secondary(mut self, genre: GameGenre) -> Self {
        if genre != self.primary && !self.secondary.contains(&genre) {
            self.secondary.push(genre);
        }
        self
    }
}

/// Dimension du projet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameDimension {
    TwoD,
    TwoAndHalfD,
    ThreeD,
}

/// Type de caméra configurée
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraType {
    Fixed,
    FollowPlayer,
    AutoScrollHorizontal,
    AutoScrollVertical,
    TopDownFree,
    ThirdPerson,
}

/// Direction des déplacements autorisés
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementDirections {
    HorizontalOnly,
    VerticalOnly,
    FourDirections,
    EightDirections,
    FreeInertia,
    Grid,
}

/// Capacités de déplacement du personnage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovementAbilities {
    pub jump: bool,
    pub double_jump: bool,
    pub run: bool,
    pub crouch: bool,
    pub dash: bool,
    pub climb: bool,
    pub swim: bool,
    pub teleport: bool,
    pub interact: bool,
}

impl Default for MovementAbilities {
    fn default() -> Self {
        Self {
            jump: false,
            double_jump: false,
            run: false,
            crouch: false,
            dash: false,
            climb: false,
            swim: false,
            teleport: false,
            interact: true,
        }
    }
}

/// Style de combat du personnage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatStyle {
    None,
    Melee,
    Projectiles,
    SpecialAbilities,
    Hybrid,
}

/// Configuration du joueur dans la recette
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerRecipe {
    pub name: String,
    pub directions: MovementDirections,
    pub abilities: MovementAbilities,
    pub combat: CombatStyle,
    pub max_health: i32,
    pub speed: i32,
}

impl Default for PlayerRecipe {
    fn default() -> Self {
        Self {
            name: "Héros".to_string(),
            directions: MovementDirections::EightDirections,
            abilities: MovementAbilities::default(),
            combat: CombatStyle::None,
            max_health: 3,
            speed: 160,
        }
    }
}

/// Types d'éléments interactifs du monde
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractiveElement {
    Door,
    Chest,
    Lever,
    PressurePlate,
    Trap,
    Collectible,
    MovingPlatform,
    BreakableWall,
    Checkpoint,
}

/// Structure environnementale du monde
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRecipe {
    pub gravity_enabled: bool,
    pub day_night_cycle: bool,
    pub dynamic_lighting: bool,
    pub interactive_elements: Vec<InteractiveElement>,
}

impl Default for WorldRecipe {
    fn default() -> Self {
        Self {
            gravity_enabled: false,
            day_night_cycle: false,
            dynamic_lighting: false,
            interactive_elements: vec![
                InteractiveElement::Collectible,
                InteractiveElement::Door,
            ],
        }
    }
}

/// Fonctionnalités de gameplay activables
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplayFeatures {
    pub inventory: bool,
    pub dialogues: bool,
    pub quest_objectives: bool,
    pub save_system: bool,
    pub score_system: bool,
    pub boss_fight: bool,
    pub wave_spawner: bool,
}

impl Default for GameplayFeatures {
    fn default() -> Self {
        Self {
            inventory: false,
            dialogues: false,
            quest_objectives: true,
            save_system: true,
            score_system: true,
            boss_fight: false,
            wave_spawner: false,
        }
    }
}

/// Profil matériel et contraintes de performance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceProfile {
    LowEnd,   // Matériel modeste : 60 FPS constants, budget CPU < 16.6ms, zéro alloc
    Balanced, // PC milieu de gamme
    HighEnd,  // PC avec GPU dédié et effets avancés
    Mobile,   // Écrans tactiles et contrainte batterie
}

impl Default for PerformanceProfile {
    fn default() -> Self {
        Self::LowEnd
    }
}

/// Modèle de Données Universel : Recette de Jeu (KaractiRage Game Recipe)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KaractiGameRecipe {
    pub schema_version: u32,
    pub title: String,
    pub author: String,
    pub genres: RecipeGenres,
    pub dimension: GameDimension,
    pub camera: CameraType,
    pub player: PlayerRecipe,
    pub world: WorldRecipe,
    pub gameplay: GameplayFeatures,
    pub performance: PerformanceProfile,
}

impl KaractiGameRecipe {
    /// Crée une recette initiale selon un genre principal
    pub fn new(title: &str, author: &str, primary_genre: GameGenre) -> Self {
        let mut recipe = Self {
            schema_version: RECIPE_SCHEMA_VERSION,
            title: title.to_string(),
            author: author.to_string(),
            genres: RecipeGenres::new(primary_genre),
            dimension: GameDimension::TwoD,
            camera: CameraType::FollowPlayer,
            player: PlayerRecipe::default(),
            world: WorldRecipe::default(),
            gameplay: GameplayFeatures::default(),
            performance: PerformanceProfile::LowEnd,
        };

        // Configuration par défaut adaptée au genre principal (Gabarit initial intelligent)
        match primary_genre {
            GameGenre::Platformer => {
                recipe.player.directions = MovementDirections::HorizontalOnly;
                recipe.player.abilities.jump = true;
                recipe.world.gravity_enabled = true;
                recipe.camera = CameraType::FollowPlayer;
            }
            GameGenre::ShootEmUp => {
                recipe.player.directions = MovementDirections::FreeInertia;
                recipe.player.combat = CombatStyle::Projectiles;
                recipe.camera = CameraType::AutoScrollVertical;
                recipe.gameplay.wave_spawner = true;
                recipe.gameplay.score_system = true;
            }
            GameGenre::Puzzle => {
                recipe.player.directions = MovementDirections::Grid;
                recipe.camera = CameraType::Fixed;
                recipe.gameplay.quest_objectives = true;
                recipe.gameplay.score_system = true;
            }
            GameGenre::Adventure | GameGenre::Rpg => {
                recipe.player.directions = MovementDirections::EightDirections;
                recipe.gameplay.inventory = true;
                recipe.gameplay.dialogues = true;
                recipe.world.interactive_elements.push(InteractiveElement::Chest);
            }
            GameGenre::Combat => {
                recipe.player.directions = MovementDirections::EightDirections;
                recipe.player.combat = CombatStyle::Melee;
                recipe.gameplay.boss_fight = true;
            }
            GameGenre::Action => {
                recipe.player.combat = CombatStyle::Melee;
                recipe.gameplay.boss_fight = true;
            }
            _ => {}
        }

        recipe
    }

    /// Sérialise la recette en chaîne JSON formatée
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Désérialise une recette depuis une chaîne JSON
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

/// Rapport d'analyse et de validation d'une recette
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecipeValidationReport {
    pub is_valid: bool,
    pub resolved_dependencies: Vec<String>,
    pub warnings: Vec<String>,
    pub conflicts: Vec<String>,
}

/// Résolveur de dépendances automatique pour les recettes de jeu
pub struct DependencyResolver;

impl DependencyResolver {
    /// Analyse une recette, résout les dépendances implicites (Règle 1) et vérifie les conflits (Règle 3)
    pub fn resolve(recipe: &mut KaractiGameRecipe) -> RecipeValidationReport {
        let mut report = RecipeValidationReport {
            is_valid: true,
            resolved_dependencies: Vec::new(),
            warnings: Vec::new(),
            conflicts: Vec::new(),
        };

        let mut auto_deps = HashSet::new();

        // RÈGLE 1 & 2 : Résolution automatique des dépendances requises
        // 1. Dépendance Saut -> Gravité et gestion des collisions de sol
        if recipe.player.abilities.jump || recipe.player.abilities.double_jump {
            if !recipe.world.gravity_enabled {
                recipe.world.gravity_enabled = true;
                auto_deps.insert("Activation automatique de la gravité pour le saut du joueur.".to_string());
            }
        }

        // 2. Dépendance Combat Projectiles -> Système de tir & collisions d'impact
        if recipe.player.combat == CombatStyle::Projectiles || recipe.player.combat == CombatStyle::Hybrid {
            auto_deps.insert("Activation du système de trajectoires et collisions de projectiles balistiques.".to_string());
        }

        // 3. Dépendance Inventaire -> Système de stockage et raccourcis hotbar
        if recipe.gameplay.inventory {
            auto_deps.insert("Configuration du conteneur d'inventaire joueur et de la hotbar 5 slots.".to_string());
        }

        // 4. Dépendance Dialogue -> Système d'interface typewriter et variables d'état
        if recipe.gameplay.dialogues {
            auto_deps.insert("Intégration du composant DialogueBox et dispatch d'événements KVL.".to_string());
        }

        // 5. Dépendance Boss Fight -> Barres de vie dédiées et motifs d'attaque KVL
        if recipe.gameplay.boss_fight {
            auto_deps.insert("Activation de l'orchestrateur de Boss et des patterns télégraphiés.".to_string());
        }

        // RÈGLE 3 : Détection de conflits logiques
        // Conflit : Shoot'em up avec saut de plateforme
        if recipe.genres.primary == GameGenre::ShootEmUp && recipe.player.abilities.jump {
            report.warnings.push("Avertissement : Le saut est activé dans un Shoot'em up (inhabituel mais autorisé).".to_string());
        }

        // Conflit : Déplacement horizontal exclusif sans gravité
        if recipe.player.directions == MovementDirections::HorizontalOnly && !recipe.world.gravity_enabled {
            report.warnings.push("Déplacement horizontal seul sans gravité : le joueur ne pourra se mouvoir que sur un axe 1D.".to_string());
        }

        // Conflit : Profil Low-End avec fonctionnalités trop lourdes
        if recipe.performance == PerformanceProfile::LowEnd && recipe.world.dynamic_lighting {
            report.warnings.push("Profil Matériel modeste sélectionné : l'éclairage dynamique sera approximé via shaders légers.".to_string());
        }

        report.resolved_dependencies = auto_deps.into_iter().collect();
        report.is_valid = report.conflicts.is_empty();

        report
    }
}

/// Générateur de Scène et de Projet Minimal Jouable à partir d'une Recette
pub struct RecipeProjectGenerator;

impl RecipeProjectGenerator {
    /// Construit une scène de départ et un ensemble de graphes KVL immédiatement jouables
    pub fn generate(recipe: &KaractiGameRecipe) -> (SceneData, HashMap<String, Graph>) {
        let (objective_text, entities) = match recipe.genres.primary {
            GameGenre::Platformer => Self::generate_platformer_world(recipe),
            GameGenre::ShootEmUp => Self::generate_shmup_world(recipe),
            GameGenre::Puzzle => Self::generate_sokoban_world(recipe),
            GameGenre::Combat => Self::generate_boss_arena_world(recipe),
            _ => Self::generate_dungeon_world(recipe),
        };

        let scene = SceneData {
            name: format!("{}: Niveau 1", recipe.title),
            description: format!("Niveau initial complet généré selon la recette '{}'", recipe.title),
            objective: Some(objective_text),
            entities,
            player: None,
            coins: Vec::new(),
            doors: Vec::new(),
            walls: Vec::new(),
            enemies: Vec::new(),
        };

        // Graphes KVL universels pour le niveau
        let mut kvl_graphs = HashMap::new();
        let mut main_graph = Graph::new("base_recipe_logic", "Logique de Recette", "Initialisée par le générateur de projet");
        main_graph.add_variable("score", kvl_core::Value::Int(0));
        main_graph.add_variable("door_unlocked", kvl_core::Value::Bool(false));
        kvl_graphs.insert("base_recipe_logic".to_string(), main_graph);

        (scene, kvl_graphs)
    }

    /// 1. Générateur Archétype Plateforme 2D (Sauts, Plateformes étagées, Pièces, Danger)
    fn generate_platformer_world(recipe: &KaractiGameRecipe) -> (String, Vec<Entity>) {
        let mut entities = Vec::new();
        let mut next_id = 1;

        // Joueur en haut de la plateforme de départ
        let p_transform = Transform2D::new(100.0, 480.0, 36.0, 36.0);
        let mut player = Entity::new(next_id, recipe.player.name.clone(), "player", p_transform);
        next_id += 1;
        player.collider = Some(Collider2D::solid(36.0, 36.0));
        player.movement = Some(Movement2D {
            speed: recipe.player.speed.max(220) as f32,
            is_player_controlled: true,
            mode: MovementMode::Platformer {
                gravity: 980.0,
                jump_force: 420.0,
                is_grounded: false,
                max_fall_speed: 650.0,
            },
            vx: 0.0,
            vy: 0.0,
            patrol: None,
        });
        player.health = Some(Health2D::new(recipe.player.max_health.max(3) as f32));
        entities.push(player);

        // Sol principal étendu
        let ground = Entity::new(next_id, "Sol_Principal".to_string(), "platform", Transform2D::new(40.0, 560.0, 1120.0, 40.0));
        next_id += 1;
        let mut g = ground;
        g.collider = Some(Collider2D::solid(1120.0, 40.0));
        entities.push(g);

        // Plateformes étagées aériennes
        let plat_coords = [
            (240.0, 460.0, 150.0, 24.0, "Plateforme_01"),
            (460.0, 380.0, 160.0, 24.0, "Plateforme_02"),
            (700.0, 300.0, 150.0, 24.0, "Plateforme_03"),
            (920.0, 220.0, 160.0, 24.0, "Plateforme_Sommet"),
        ];

        for &(px, py, pw, ph, pname) in &plat_coords {
            let mut plat = Entity::new(next_id, pname.to_string(), "platform", Transform2D::new(px, py, pw, ph));
            next_id += 1;
            plat.collider = Some(Collider2D::solid(pw, ph));
            entities.push(plat);
        }

        // Pièces d'or en surplomb
        let coins = [(300.0, 410.0), (530.0, 330.0), (760.0, 250.0), (980.0, 170.0)];
        for (idx, &(cx, cy)) in coins.iter().enumerate() {
            let mut coin = Entity::new(next_id, format!("Piece_{:02}", idx + 1), "collectible", Transform2D::new(cx, cy, 24.0, 24.0));
            next_id += 1;
            coin.collider = Some(Collider2D::trigger(24.0, 24.0));
            entities.push(coin);
        }

        // Zone de piques de lave au sol
        let mut hazard = Entity::new(next_id, "Piques_Lave".to_string(), "hazard", Transform2D::new(420.0, 540.0, 160.0, 20.0));
        next_id += 1;
        hazard.collider = Some(Collider2D::trigger(160.0, 20.0));
        entities.push(hazard);

        // Portail dimensionnel d'arrivée sur le sommet
        let mut portal = Entity::new(next_id, "Portail_Fin".to_string(), "portal", Transform2D::new(1000.0, 150.0, 40.0, 70.0));
        portal.collider = Some(Collider2D::trigger(40.0, 70.0));
        entities.push(portal);

        ("FRANCHISSEZ LES PLATEFORMES ET ATTEIGNEZ LE PORTAIL DU SOMMET".to_string(), entities)
    }

    /// 2. Générateur Archétype Donjon Aventure / RPG (Murs, Clé, Coffre, Garde A*, Porte)
    fn generate_dungeon_world(recipe: &KaractiGameRecipe) -> (String, Vec<Entity>) {
        let mut entities = Vec::new();
        let mut next_id = 1;

        // Joueur Aventurier Top-Down
        let p_transform = Transform2D::new(120.0, 120.0, 36.0, 36.0);
        let mut player = Entity::new(next_id, recipe.player.name.clone(), "player", p_transform);
        next_id += 1;
        player.collider = Some(Collider2D::solid(36.0, 36.0));
        player.movement = Some(Movement2D {
            speed: recipe.player.speed.max(240) as f32,
            is_player_controlled: true,
            mode: MovementMode::TopDown,
            vx: 0.0,
            vy: 0.0,
            patrol: None,
        });
        player.health = Some(Health2D::new(recipe.player.max_health.max(5) as f32));
        entities.push(player);

        // Murs d'enceinte du Donjon
        let walls = [
            (50.0, 50.0, 1100.0, 28.0, "Mur_Nord"),
            (50.0, 680.0, 1100.0, 28.0, "Mur_Sud"),
            (50.0, 50.0, 28.0, 650.0, "Mur_Ouest"),
            (1122.0, 50.0, 28.0, 650.0, "Mur_Est"),
            // Cloisons intérieures
            (420.0, 50.0, 28.0, 360.0, "Cloison_Haute"),
            (780.0, 320.0, 28.0, 380.0, "Cloison_Basse"),
        ];

        for &(wx, wy, ww, wh, wname) in &walls {
            let mut w = Entity::new(next_id, wname.to_string(), "wall", Transform2D::new(wx, wy, ww, wh));
            next_id += 1;
            w.collider = Some(Collider2D::solid(ww, wh));
            entities.push(w);
        }

        // Pièces d'or réparties dans les couloirs
        let coins = [(220.0, 280.0), (280.0, 480.0), (550.0, 180.0), (620.0, 450.0), (950.0, 520.0)];
        for (idx, &(cx, cy)) in coins.iter().enumerate() {
            let mut coin = Entity::new(next_id, format!("Piece_Or_{:02}", idx + 1), "collectible", Transform2D::new(cx, cy, 24.0, 24.0));
            next_id += 1;
            coin.collider = Some(Collider2D::trigger(24.0, 24.0));
            entities.push(coin);
        }

        // Clé Dorée
        let mut key = Entity::new(next_id, "Cle_Doree".to_string(), "item", Transform2D::new(160.0, 520.0, 24.0, 24.0));
        next_id += 1;
        key.collider = Some(Collider2D::trigger(24.0, 24.0));
        entities.push(key);

        // Coffre au Trésor Scellé
        let mut chest = Entity::new(next_id, "Coffre_Tresor".to_string(), "item", Transform2D::new(580.0, 100.0, 36.0, 36.0));
        next_id += 1;
        chest.collider = Some(Collider2D::solid(36.0, 36.0));
        entities.push(chest);

        // Garde Patrouilleur avec Vision Conique A*
        let mut guard = Entity::new(next_id, "Garde_Patrouilleur".to_string(), "enemy", Transform2D::new(580.0, 280.0, 36.0, 36.0));
        next_id += 1;
        guard.collider = Some(Collider2D::solid(36.0, 36.0));
        guard.vision = Some(crate::entity::VisionSensing::new(180.0));
        guard.nav_agent = Some(crate::navigation::NavAgent::chase_player(110.0));
        entities.push(guard);

        // Porte du Temple
        let mut door = Entity::new(next_id, "Porte_Donjon".to_string(), "door", Transform2D::new(780.0, 180.0, 28.0, 120.0));
        door.collider = Some(Collider2D::solid(28.0, 120.0));
        entities.push(door);

        ("COLLECTEZ LA CLÉ ET 50 OR POUR OUVRIR LA PORTE DU DONJON".to_string(), entities)
    }

    /// 3. Générateur Archétype Shoot'em Up Spatial (Inertie, Astéroïdes, Escouade ennemie)
    fn generate_shmup_world(recipe: &KaractiGameRecipe) -> (String, Vec<Entity>) {
        let mut entities = Vec::new();
        let mut next_id = 1;

        // Vaisseau Spatial
        let p_transform = Transform2D::new(600.0, 640.0, 36.0, 36.0);
        let mut player = Entity::new(next_id, recipe.player.name.clone(), "player", p_transform);
        next_id += 1;
        player.collider = Some(Collider2D::solid(36.0, 36.0));
        player.movement = Some(Movement2D {
            speed: recipe.player.speed.max(300) as f32,
            is_player_controlled: true,
            mode: MovementMode::SpaceInertia {
                angular_speed: 4.5,
                thrust: 450.0,
                drag: 0.985,
            },
            vx: 0.0,
            vy: 0.0,
            patrol: None,
        });
        player.health = Some(Health2D::new(recipe.player.max_health.max(4) as f32));
        entities.push(player);

        // Astéroïdes flottants solides
        let asteroids = [
            (280.0, 420.0, 64.0, 64.0, "Asteroide_Alpha"),
            (600.0, 340.0, 80.0, 80.0, "Asteroide_Beta"),
            (920.0, 400.0, 64.0, 64.0, "Asteroide_Gamma"),
        ];

        for &(ax, ay, aw, ah, aname) in &asteroids {
            let mut ast = Entity::new(next_id, aname.to_string(), "wall", Transform2D::new(ax, ay, aw, ah));
            next_id += 1;
            ast.collider = Some(Collider2D::solid(aw, ah));
            entities.push(ast);
        }

        // Escouade de drones ennemis en volée
        let drones = [(450.0, 160.0), (600.0, 130.0), (750.0, 160.0)];
        for (idx, &(dx, dy)) in drones.iter().enumerate() {
            let mut drone = Entity::new(next_id, format!("Drone_Hostile_{:02}", idx + 1), "enemy", Transform2D::new(dx, dy, 32.0, 32.0));
            next_id += 1;
            drone.collider = Some(Collider2D::solid(32.0, 32.0));
            drone.health = Some(Health2D::new(3.0));
            entities.push(drone);
        }

        // Orbes d'énergie
        for idx in 0..4 {
            let ox = 350.0 + (idx as f32 * 160.0);
            let mut orbe = Entity::new(next_id, format!("Orbe_Energie_{:02}", idx + 1), "collectible", Transform2D::new(ox, 240.0, 20.0, 20.0));
            next_id += 1;
            orbe.collider = Some(Collider2D::trigger(20.0, 20.0));
            entities.push(orbe);
        }

        ("SURVIVEZ AUX VAGUES ENNEMIES DANS LE COSMOS ET DÉTRUISEZ LES DRONES".to_string(), entities)
    }

    /// 4. Générateur Archétype Puzzle Sokoban (Grille tactique, Caisses poussables, Dalles cibles)
    fn generate_sokoban_world(recipe: &KaractiGameRecipe) -> (String, Vec<Entity>) {
        let mut entities = Vec::new();
        let mut next_id = 1;

        // Joueur en déplacement Grille
        let p_transform = Transform2D::new(360.0, 300.0, 32.0, 32.0);
        let mut player = Entity::new(next_id, recipe.player.name.clone(), "player", p_transform);
        next_id += 1;
        player.collider = Some(Collider2D::solid(32.0, 32.0));
        player.movement = Some(Movement2D {
            speed: 150.0,
            is_player_controlled: true,
            mode: MovementMode::Grid {
                cell_size: 32.0,
                cooldown: 0.18,
                timer: 0.0,
            },
            vx: 0.0,
            vy: 0.0,
            patrol: None,
        });
        player.health = Some(Health2D::new(1.0));
        entities.push(player);

        // Murs de la salle de puzzle
        let walls = [
            (280.0, 180.0, 640.0, 24.0, "Mur_Puzzle_Nord"),
            (280.0, 580.0, 640.0, 24.0, "Mur_Puzzle_Sud"),
            (280.0, 180.0, 24.0, 424.0, "Mur_Puzzle_Ouest"),
            (896.0, 180.0, 24.0, 424.0, "Mur_Puzzle_Est"),
        ];

        for &(wx, wy, ww, wh, wname) in &walls {
            let mut w = Entity::new(next_id, wname.to_string(), "wall", Transform2D::new(wx, wy, ww, wh));
            next_id += 1;
            w.collider = Some(Collider2D::solid(ww, wh));
            entities.push(w);
        }

        // Blocs / Caisses poussables
        let mut crate1 = Entity::new(next_id, "Bloc_Poussable_01".to_string(), "crate", Transform2D::new(480.0, 320.0, 32.0, 32.0));
        next_id += 1;
        let mut c1 = Collider2D::solid(32.0, 32.0);
        c1.is_pushable = true;
        crate1.collider = Some(c1);
        entities.push(crate1);

        let mut crate2 = Entity::new(next_id, "Bloc_Poussable_02".to_string(), "crate", Transform2D::new(560.0, 400.0, 32.0, 32.0));
        next_id += 1;
        let mut c2 = Collider2D::solid(32.0, 32.0);
        c2.is_pushable = true;
        crate2.collider = Some(c2);
        entities.push(crate2);

        // Dalles de pression réceptrices (Triggers)
        let mut plate1 = Entity::new(next_id, "Dalle_Cible_01".to_string(), "plate", Transform2D::new(720.0, 320.0, 32.0, 32.0));
        next_id += 1;
        plate1.collider = Some(Collider2D::trigger(32.0, 32.0));
        entities.push(plate1);

        let mut plate2 = Entity::new(next_id, "Dalle_Cible_02".to_string(), "plate", Transform2D::new(720.0, 400.0, 32.0, 32.0));
        next_id += 1;
        plate2.collider = Some(Collider2D::trigger(32.0, 32.0));
        entities.push(plate2);

        // Porte secrète qui s'ouvrira
        let mut door = Entity::new(next_id, "Porte_Secrete".to_string(), "door", Transform2D::new(896.0, 340.0, 24.0, 80.0));
        door.collider = Some(Collider2D::solid(24.0, 80.0));
        entities.push(door);

        ("POUSSEZ LES BLOCS SUR LES DALLES DE PRESSION POUR DÉVERROUILLER LA PORTE".to_string(), entities)
    }

    /// 5. Générateur Archétype Titan Boss Arène (Arène fermée, Boss géant, Piliers destructibles)
    fn generate_boss_arena_world(recipe: &KaractiGameRecipe) -> (String, Vec<Entity>) {
        let mut entities = Vec::new();
        let mut next_id = 1;

        // Joueur Combattant Chasseur
        let p_transform = Transform2D::new(240.0, 400.0, 40.0, 40.0);
        let mut player = Entity::new(next_id, recipe.player.name.clone(), "player", p_transform);
        next_id += 1;
        player.collider = Some(Collider2D::solid(40.0, 40.0));
        player.movement = Some(Movement2D {
            speed: recipe.player.speed.max(260) as f32,
            is_player_controlled: true,
            mode: MovementMode::TopDown,
            vx: 0.0,
            vy: 0.0,
            patrol: None,
        });
        player.health = Some(Health2D::new(recipe.player.max_health.max(10) as f32));
        entities.push(player);

        // Murs de l'Arène
        let walls = [
            (100.0, 80.0, 1000.0, 30.0, "Arene_Mur_Nord"),
            (100.0, 690.0, 1000.0, 30.0, "Arene_Mur_Sud"),
            (100.0, 80.0, 30.0, 640.0, "Arene_Mur_Ouest"),
            (1070.0, 80.0, 30.0, 640.0, "Arene_Mur_Est"),
        ];

        for &(wx, wy, ww, wh, wname) in &walls {
            let mut w = Entity::new(next_id, wname.to_string(), "wall", Transform2D::new(wx, wy, ww, wh));
            next_id += 1;
            w.collider = Some(Collider2D::solid(ww, wh));
            entities.push(w);
        }

        // Piliers de couverture
        let mut pillar1 = Entity::new(next_id, "Pilier_Nord".to_string(), "wall", Transform2D::new(480.0, 240.0, 44.0, 88.0));
        next_id += 1;
        pillar1.collider = Some(Collider2D::solid(44.0, 88.0));
        entities.push(pillar1);

        let mut pillar2 = Entity::new(next_id, "Pilier_Sud".to_string(), "wall", Transform2D::new(480.0, 480.0, 44.0, 88.0));
        next_id += 1;
        pillar2.collider = Some(Collider2D::solid(44.0, 88.0));
        entities.push(pillar2);

        // Boss Démon Titan Colossal
        let mut boss = Entity::new(next_id, "Boss_Colosse_Obsidienne".to_string(), "boss", Transform2D::new(840.0, 360.0, 80.0, 80.0));
        next_id += 1;
        boss.collider = Some(Collider2D::solid(80.0, 80.0));
        boss.health = Some(Health2D::new(100.0));
        boss.nav_agent = Some(crate::navigation::NavAgent::chase_player(90.0));
        entities.push(boss);

        // Potions de réanimation
        let mut pot1 = Entity::new(next_id, "Potion_Soin_01".to_string(), "item", Transform2D::new(320.0, 180.0, 24.0, 24.0));
        next_id += 1;
        pot1.collider = Some(Collider2D::trigger(24.0, 24.0));
        entities.push(pot1);

        let mut pot2 = Entity::new(next_id, "Potion_Soin_02".to_string(), "item", Transform2D::new(320.0, 600.0, 24.0, 24.0));
        pot2.collider = Some(Collider2D::trigger(24.0, 24.0));
        entities.push(pot2);

        ("AFFRONTEZ ET VAINQUEZ LE COLOSSE D'OBSIDIENNE DANS L'ARÈNE".to_string(), entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_serialize_recipe() {
        let recipe = KaractiGameRecipe::new("Donjon Maudit", "Studio KR", GameGenre::Adventure);
        assert_eq!(recipe.schema_version, RECIPE_SCHEMA_VERSION);
        assert_eq!(recipe.genres.primary, GameGenre::Adventure);
        assert!(recipe.gameplay.inventory);

        let json = recipe.to_json().expect("Sérialisation JSON");
        assert!(json.contains("Donjon Maudit"));
        assert!(json.contains("adventure"));

        let roundtrip = KaractiGameRecipe::from_json(&json).expect("Désérialisation JSON");
        assert_eq!(roundtrip.title, recipe.title);
        assert_eq!(roundtrip.genres.primary, recipe.genres.primary);
    }

    #[test]
    fn test_platformer_recipe_dependency_resolution() {
        let mut recipe = KaractiGameRecipe::new("Plateforme Hero", "Testeur", GameGenre::Platformer);
        recipe.world.gravity_enabled = false; // Simulation d'une omission
        recipe.player.abilities.jump = true;

        let report = DependencyResolver::resolve(&mut recipe);
        assert!(report.is_valid);
        assert!(recipe.world.gravity_enabled, "Le saut doit forcer l'activation de la gravité");
        assert!(report.resolved_dependencies.iter().any(|d| d.contains("gravité")));
    }

    #[test]
    fn test_shootemup_recipe_dependency_resolution() {
        let mut recipe = KaractiGameRecipe::new("Galaxy Strike", "Pilote", GameGenre::ShootEmUp);
        let report = DependencyResolver::resolve(&mut recipe);

        assert!(report.is_valid);
        assert_eq!(recipe.player.combat, CombatStyle::Projectiles);
        assert_eq!(recipe.camera, CameraType::AutoScrollVertical);
        assert!(recipe.gameplay.wave_spawner);
    }

    #[test]
    fn test_generate_playable_project_from_recipe() {
        let recipe = KaractiGameRecipe::new("Projet Test", "Dev", GameGenre::Adventure);
        let (scene, graphs) = RecipeProjectGenerator::generate(&recipe);

        assert!(scene.entities.len() >= 2, "La scène générée doit contenir le joueur et les interactifs");
        assert_eq!(scene.entities[0].name, "Héros");
        assert!(scene.entities[0].movement.is_some());
        assert!(graphs.contains_key("base_recipe_logic"));
    }

    #[test]
    fn test_all_five_archetype_presets_generation() {
        // 1. Archetype Donjon Aventure
        let mut r_dungeon = KaractiGameRecipe::new("Donjon", "Studio", GameGenre::Adventure);
        let rep_dungeon = DependencyResolver::resolve(&mut r_dungeon);
        assert!(rep_dungeon.is_valid);
        let (s_dungeon, _) = RecipeProjectGenerator::generate(&r_dungeon);
        assert!(s_dungeon.entities.iter().any(|e| e.tag == "player"));
        assert!(s_dungeon.entities.iter().any(|e| e.tag == "door"));

        // 2. Archetype Platformer
        let mut r_plat = KaractiGameRecipe::new("Platformer", "Studio", GameGenre::Platformer);
        let rep_plat = DependencyResolver::resolve(&mut r_plat);
        assert!(rep_plat.is_valid);
        let (s_plat, _) = RecipeProjectGenerator::generate(&r_plat);
        let p_plat = &s_plat.entities[0];
        assert!(matches!(p_plat.movement.as_ref().unwrap().mode, MovementMode::Platformer { .. }));

        // 3. Archetype Shoot'em up
        let mut r_shmup = KaractiGameRecipe::new("Shmup", "Studio", GameGenre::ShootEmUp);
        let rep_shmup = DependencyResolver::resolve(&mut r_shmup);
        assert!(rep_shmup.is_valid);
        let (s_shmup, _) = RecipeProjectGenerator::generate(&r_shmup);
        let p_shmup = &s_shmup.entities[0];
        assert!(matches!(p_shmup.movement.as_ref().unwrap().mode, MovementMode::SpaceInertia { .. }));

        // 4. Archetype Puzzle Sokoban
        let mut r_puzzle = KaractiGameRecipe::new("Sokoban", "Studio", GameGenre::Puzzle);
        let rep_puzzle = DependencyResolver::resolve(&mut r_puzzle);
        assert!(rep_puzzle.is_valid);
        let (s_puzzle, _) = RecipeProjectGenerator::generate(&r_puzzle);
        let p_puzzle = &s_puzzle.entities[0];
        assert!(matches!(p_puzzle.movement.as_ref().unwrap().mode, MovementMode::Grid { .. }));

        // 5. Archetype Boss Fight
        let mut r_boss = KaractiGameRecipe::new("Boss Arena", "Studio", GameGenre::Combat);
        r_boss.gameplay.boss_fight = true;
        let rep_boss = DependencyResolver::resolve(&mut r_boss);
        assert!(rep_boss.is_valid);
        let (s_boss, _) = RecipeProjectGenerator::generate(&r_boss);
        assert!(s_boss.entities.iter().any(|e| e.tag == "boss"), "La scène de boss doit contenir une entité boss");
    }
}

