use base64::{engine::general_purpose::STANDARD, Engine as _};
use crate::entity::{Entity, ShapeKind};
use crate::game::GameState;
use kvl_core::{Graph, Value};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

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

/// Spritesheet embarquée dans un package autonome (atlas PNG + JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SheetAssetExport {
    /// Atlas PNG en base64.
    pub png: String,
    /// JSON TexturePacker/Aseprite.
    pub json: String,
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
    /// Sprites pixel-art embarqués (id -> PNG base64), pour l'export autonome.
    #[serde(default)]
    pub sprite_assets: HashMap<String, String>,
    /// Spritesheets embarquées (id -> atlas PNG base64 + JSON), pour l'export autonome.
    #[serde(default)]
    pub sheet_assets: HashMap<String, SheetAssetExport>,
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

/// Sévérité d'un diagnostic de pré-vol d'export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreflightSeverity {
    /// Problème qui empêche un export jouable (script ou asset introuvable).
    Blocking,
    /// Problème tolérable qui mérite l'attention du créateur (budget de performance).
    Warning,
}

/// Diagnostic unitaire produit par le pré-vol d'export.
///
/// Le `code` est volontairement court, stable et sans accent : il sert de clé
/// technique à l'interface Studio, tandis que `message` est destiné au créateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightIssue {
    /// Gravité du diagnostic.
    pub severity: PreflightSeverity,
    /// Code technique stable : `"script-manquant"`, `"asset-manquant"`,
    /// `"budget-entites"` ou `"budget-kvl"`.
    pub code: &'static str,
    /// Message lisible en français, orienté créateur.
    pub message: String,
}

impl PreflightIssue {
    /// Construit un diagnostic bloquant.
    fn blocking(code: &'static str, message: String) -> Self {
        Self { severity: PreflightSeverity::Blocking, code, message }
    }

    /// Construit un diagnostic d'avertissement.
    fn warning(code: &'static str, message: String) -> Self {
        Self { severity: PreflightSeverity::Warning, code, message }
    }
}

/// Rapport de pré-vol d'export : l'état du jeu est-il exportable en l'état ?
///
/// `ok` vaut `true` quand aucun diagnostic [`PreflightSeverity::Blocking`] n'a
/// été relevé ; les avertissements de budget ne bloquent jamais l'export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    /// `true` si aucun bloquage n'a été relevé.
    pub ok: bool,
    /// Diagnostics détectés, dans l'ordre de détection (blocages puis budgets).
    pub issues: Vec<PreflightIssue>,
}

impl PreflightReport {
    /// Nombre de diagnostics bloquants (export refusé).
    pub fn blocking_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Blocking)
            .count()
    }

    /// Diagnostics non bloquants, dans l'ordre de détection.
    pub fn warnings(&self) -> Vec<&PreflightIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Warning)
            .collect()
    }

    /// Résumé lisible pour le créateur, par exemple
    /// `« 2 blocages, 1 avertissement : script-manquant — … ; budget-entites — … »`.
    pub fn summary(&self) -> String {
        if self.issues.is_empty() {
            return "Pré-vol réussi : aucun problème détecté.".to_string();
        }
        let blocages = self.blocking_count();
        let avertissements = self.warnings().len();
        let entete = format!(
            "{} blocage{}, {} avertissement{}",
            blocages,
            if blocages > 1 { "s" } else { "" },
            avertissements,
            if avertissements > 1 { "s" } else { "" }
        );
        let details = self
            .issues
            .iter()
            .map(|i| format!("{} — {}", i.code, i.message))
            .collect::<Vec<_>>()
            .join(" ; ");
        format!("{entete} : {details}")
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

        // Collecte des assets pixel-art référencés (sprites + spritesheets/anims)
        // pour rendre le package autonome (pas de dépendance au système de fichiers).
        let mut sprite_assets = HashMap::new();
        let mut sheet_assets = HashMap::new();
        for ent in &game.entities {
            if let Some(rend) = &ent.renderable {
                match &rend.shape {
                    ShapeKind::ImageRef { asset } => {
                        if let Some(bytes) = crate::vector::read_png_source(asset) {
                            sprite_assets.entry(asset.clone()).or_insert_with(|| STANDARD.encode(&bytes));
                        }
                    }
                    ShapeKind::SheetRef { asset, .. } | ShapeKind::AnimRef { asset } => {
                        if let Some((png, json)) = crate::sheet::read_sheet_source(asset) {
                            sheet_assets.entry(asset.clone()).or_insert_with(|| SheetAssetExport {
                                png: STANDARD.encode(&png),
                                json,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        KaractiGamePackage {
            metadata,
            scene_name: game.scene_name.clone(),
            scene_objective: game.scene_objective.clone(),
            entities: game.entities.clone(),
            kvl_graphs: game.kvl_graphs.clone(),
            initial_variables: initial_vars,
            sprite_assets,
            sheet_assets,
        }
    }

    /// Exporte le jeu en un fichier HTML standalone complet
    pub fn export_html(game: &GameState, title: &str, author: &str) -> String {
        let package = Self::create_package(game, title, author);
        package.generate_standalone_html()
    }

    /// Pré-vol d'export : vérifie que chaque script et chaque asset référencés
    /// sont résolvables, puis signale les budgets de performance dépassés.
    ///
    /// Un script est considéré présent s'il figure dans `game.kvl_graphs` (graphe
    /// déjà chargé) ou si son fichier existe sur disque. Un asset de rendu est
    /// considéré présent s'il est connu du cache correspondant (`sprite_cache`
    /// pour les PNG/SVG, `sheet_cache` pour les spritesheets/anims) ou si son
    /// fichier source existe sous `assets/`.
    ///
    /// Les dépassements de budget (> 500 entités, > 5000 connexions KVL
    /// cumulées) ne produisent que des avertissements : ils n'empêchent jamais
    /// l'export.
    pub fn preflight(game: &GameState) -> PreflightReport {
        const BUDGET_ENTITES: usize = 500;
        const BUDGET_CONNEXIONS_KVL: usize = 5000;

        let mut issues = Vec::new();
        // Évite de répéter le même diagnostic pour N entités partageant la même
        // référence (cas fréquent : un script de collectible sur 20 pièces).
        let mut deja_signales: HashSet<String> = HashSet::new();

        for ent in &game.entities {
            // --- Scripts KVL -------------------------------------------------
            if let Some(script) = &ent.script {
                let chemin = script.script_path.trim();
                if !chemin.is_empty()
                    && !game.kvl_graphs.contains_key(&script.script_path)
                    && !fichier_existe(chemin)
                {
                    let cle = format!("script-manquant\u{1f}{chemin}");
                    if deja_signales.insert(cle) {
                        issues.push(PreflightIssue::blocking(
                            "script-manquant",
                            format!(
                                "le script « {chemin} » est référencé par l'entité « {} » mais introuvable : aucun graphe KVL chargé et aucun fichier sur disque",
                                ent.name
                            ),
                        ));
                    }
                }
            }

            // --- Assets de rendu ---------------------------------------------
            let Some(rend) = &ent.renderable else { continue };
            let asset_absent = match &rend.shape {
                ShapeKind::SvgRef { asset } => {
                    // `vector_sprite` est renseigné à l'import quand le SVG a pu
                    // être rastérisé (y compris via un asset embarqué).
                    ent.vector_sprite.is_none()
                        && game.sprite_cache.get(asset).is_none()
                        && !asset_svg_existe(asset)
                }
                ShapeKind::ImageRef { asset } => {
                    game.sprite_cache.get(asset).is_none()
                        && crate::vector::read_png_source(asset).is_none()
                }
                ShapeKind::SheetRef { asset, .. } | ShapeKind::AnimRef { asset } => {
                    game.sheet_cache.get(asset).is_none()
                        && crate::sheet::read_sheet_source(asset).is_none()
                }
                _ => false,
            };
            if asset_absent {
                let (reference, libelle) = match &rend.shape {
                    ShapeKind::SvgRef { asset } | ShapeKind::ImageRef { asset } => (asset, "sprite"),
                    ShapeKind::SheetRef { asset, .. } | ShapeKind::AnimRef { asset } => {
                        (asset, "spritesheet")
                    }
                    _ => unreachable!("seuls les assets de rendu sont vérifiés"),
                };
                let cle = format!("asset-manquant\u{1f}{reference}");
                if deja_signales.insert(cle) {
                    issues.push(PreflightIssue::blocking(
                        "asset-manquant",
                        format!(
                            "le {libelle} « {reference} » est référencé par l'entité « {} » mais introuvable : absent du cache et absent sous assets/",
                            ent.name
                        ),
                    ));
                }
            }
        }

        // --- Budgets (avertissements uniquement) -----------------------------
        if game.entities.len() > BUDGET_ENTITES {
            issues.push(PreflightIssue::warning(
                "budget-entites",
                format!(
                    "{} entités : au-delà de {BUDGET_ENTITES}, les profils Bas de gamme et Batterie risquent de passer sous 60 FPS",
                    game.entities.len()
                ),
            ));
        }
        let connexions: usize = game.kvl_graphs.values().map(|g| g.connections.len()).sum();
        if connexions > BUDGET_CONNEXIONS_KVL {
            issues.push(PreflightIssue::warning(
                "budget-kvl",
                format!(
                    "{connexions} connexions KVL cumulées : au-delà de {BUDGET_CONNEXIONS_KVL}, l'interpréteur peut coûter cher à chaque frame"
                ),
            ));
        }

        PreflightReport {
            ok: !issues
                .iter()
                .any(|i| i.severity == PreflightSeverity::Blocking),
            issues,
        }
    }

    /// Export HTML protégé par le pré-vol : renvoie le rapport en `Err` dès qu'un
    /// diagnostic bloquant est présent, sinon produit l'export standard.
    pub fn export_html_checked(
        game: &GameState,
        title: &str,
        author: &str,
    ) -> Result<String, PreflightReport> {
        let report = Self::preflight(game);
        if !report.ok {
            return Err(report);
        }
        Ok(Self::export_html(game, title, author))
    }

    /// Empreinte stable du contenu exporté (FNV-1a 64 bits, 16 caractères hex).
    ///
    /// Le paquet JSON embarqué dans le HTML est isolé et haché en priorité : la
    /// coquille HTML (titre, styles, lecteur) n'influence donc pas l'empreinte et
    /// une éventuelle date volatile ajoutée dans la page (pied de page, bannière
    /// « généré le … ») ne la fait pas dériver. Si le marqueur du paquet est
    /// absent (HTML étranger), l'empreinte porte sur la chaîne entière.
    ///
    /// Limite connue : `PackageMetadata::created_at` est actuellement une
    /// constante (`"2026-10-16"`), donc l'export est reproductible. Si ce champ
    /// devenait un horodatage réel, il faudrait l'exclure du paquet avant
    /// hachage, sinon deux exports identiques produiraient deux empreintes.
    ///
    /// Ce n'est pas un hash cryptographique : il sert à détecter une dérive de
    /// contenu, pas à authentifier un export.
    pub fn export_fingerprint(html: &str) -> String {
        let empreinte = paquet_embarque(html).unwrap_or(html);
        crate::ai_dictionary::hash_text(empreinte.as_bytes())
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

        // Décodage des assets embarqués + re-résolution des sprites/spritesheets.
        let mut sprites = HashMap::new();
        for (id, b64) in &package.sprite_assets {
            if let Ok(bytes) = STANDARD.decode(b64) {
                sprites.insert(id.clone(), bytes);
            }
        }
        let mut sheets = HashMap::new();
        for (id, s) in &package.sheet_assets {
            if let Ok(png) = STANDARD.decode(&s.png) {
                sheets.insert(id.clone(), (png, s.json.clone()));
            }
        }
        game.apply_embedded_assets(sprites, sheets);

        // Réinitialise la caméra et la sélection du Studio
        game.editor.selected = crate::editor::SelectedEntity::None;
        game.editor.mode = crate::editor::EditorMode::Play;
        game.set_status(format!("🎮 Jeu '{}' chargé avec succès ! ({} entités, {} graphes KVL)", package.metadata.title, package.entities.len(), package.kvl_graphs.len()));
    }
}

/// Vérifie l'existence d'un fichier relatif au dépôt.
///
/// Même motif de résolution que le reste du projet : chemin courant, puis
/// `../../<chemin>` (les tests s'exécutent depuis `crates/engine-core`), puis la
/// racine du dépôt pour rester indépendant du cwd.
fn fichier_existe(rel: &str) -> bool {
    if Path::new(rel).is_file() {
        return true;
    }
    if Path::new(&format!("../../{rel}")).is_file() {
        return true;
    }
    let racine = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    Path::new(&format!("{racine}/{rel}")).is_file()
}

/// Vérifie qu'un sprite vectoriel référencé par `SvgRef` est résolvable sur
/// disque (chemin explicite en `.svg` ou identifiant sous `assets/sprites/`).
fn asset_svg_existe(asset: &str) -> bool {
    if fichier_existe(asset) {
        return true;
    }
    let rel = if asset.ends_with(".svg") {
        asset.to_string()
    } else {
        format!("assets/sprites/{asset}.svg")
    };
    fichier_existe(&rel)
}

/// Marqueur du paquet JSON embarqué dans le template HTML standalone.
const BALISE_PAQUET: &str = "<script id=\"embedded-package\" type=\"application/json\">";

/// Extrait le JSON embarqué entre `BALISE_PAQUET` et `</script>`, s'il existe.
fn paquet_embarque(html: &str) -> Option<&str> {
    let debut = html.find(BALISE_PAQUET)? + BALISE_PAQUET.len();
    let fin = html[debut..].find("</script>")? + debut;
    let paquet = html[debut..fin].trim();
    if paquet.is_empty() {
        None
    } else {
        Some(paquet)
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
            sprite_assets: HashMap::new(),
            sheet_assets: HashMap::new(),
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
            sprite_assets: HashMap::new(),
            sheet_assets: HashMap::new(),
        };

        let html = package.generate_standalone_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Quest"));
        assert!(html.contains("Tester"));
        assert!(html.contains("canvas id=\"game-canvas\""));
        assert!(html.contains("id=\"embedded-package\""));
    }

    /// Charge le niveau de référence P1 depuis le cwd, avec repli `../../`
    /// (cargo exécute les tests depuis `crates/engine-core`).
    fn jeu_reference() -> GameState {
        let candidats = [
            "assets/scenes/ref_niveau_1.scene.json",
            "../../assets/scenes/ref_niveau_1.scene.json",
        ];
        for chemin in candidats {
            if Path::new(chemin).is_file() {
                return GameState::load_from_scene(chemin)
                    .expect("la scène de référence doit être chargeable");
            }
        }
        panic!("scène de référence introuvable : assets/scenes/ref_niveau_1.scene.json");
    }

    #[test]
    fn test_preflight_projet_sain() {
        let jeu = jeu_reference();
        let report = StandaloneExporter::preflight(&jeu);
        assert!(report.ok, "pré-vol du niveau de référence : {}", report.summary());
        assert_eq!(report.blocking_count(), 0, "aucun blocage attendu : {}", report.summary());
        assert!(report.issues.is_empty(), "aucun diagnostic attendu : {}", report.summary());
    }

    #[test]
    fn test_preflight_script_manquant_bloque() {
        let mut jeu = jeu_reference();
        jeu.entities[0].script = Some(crate::entity::KVLScriptComponent::new(
            "assets/scripts/script_fantome_introuvable.kvl.json",
        ));

        let report = StandaloneExporter::preflight(&jeu);
        assert!(!report.ok);
        assert_eq!(report.blocking_count(), 1, "rapport : {}", report.summary());
        let issue = report
            .issues
            .iter()
            .find(|i| i.code == "script-manquant")
            .expect("un blocage script-manquant est attendu");
        assert_eq!(issue.severity, PreflightSeverity::Blocking);
        assert!(issue.message.contains("script_fantome_introuvable.kvl.json"));
        assert!(issue.message.contains("Joueur"));
        assert!(report.summary().contains("blocage"));
    }

    #[test]
    fn test_export_html_checked_refuse_un_script_manquant() {
        let sain = jeu_reference();
        let html = StandaloneExporter::export_html_checked(&sain, "Niveau 1", "Testeur")
            .expect("le niveau de référence doit s'exporter");
        assert!(html.contains("<!DOCTYPE html>"));

        let mut casse = jeu_reference();
        casse.entities[1].script = Some(crate::entity::KVLScriptComponent::new(
            "assets/scripts/script_fantome_introuvable.kvl.json",
        ));
        let err = StandaloneExporter::export_html_checked(&casse, "Niveau 1", "Testeur")
            .expect_err("un script manquant doit refuser l'export");
        assert!(!err.ok);
        assert!(err.issues.iter().any(|i| i.code == "script-manquant"));
    }

    #[test]
    fn test_preflight_budget_entites_avertit_sans_bloquer() {
        let mut jeu = jeu_reference();
        let modele = jeu.entities.clone();
        assert!(!modele.is_empty());
        let mut i = 0usize;
        while jeu.entities.len() <= 500 {
            let mut copie = modele[i % modele.len()].clone();
            copie.id = jeu.entities.len() + 1;
            jeu.entities.push(copie);
            i += 1;
        }

        let report = StandaloneExporter::preflight(&jeu);
        assert!(report.ok, "un budget dépassé n'est pas bloquant : {}", report.summary());
        assert!(report
            .warnings()
            .iter()
            .any(|a| a.code == "budget-entites"));
        assert!(report
            .issues
            .iter()
            .all(|i| i.severity == PreflightSeverity::Warning));
    }

    #[test]
    fn test_export_fingerprint_stable() {
        let jeu = jeu_reference();
        let a = StandaloneExporter::export_html(&jeu, "Empreinte", "Testeur");
        let b = StandaloneExporter::export_html(&jeu, "Empreinte", "Testeur");
        let fa = StandaloneExporter::export_fingerprint(&a);
        let fb = StandaloneExporter::export_fingerprint(&b);

        assert_eq!(fa, fb, "deux exports du même état doivent partager l'empreinte");
        assert_eq!(fa.len(), 16, "FNV-1a 64 bits encodé en 16 caractères hex");
        assert!(fa.chars().all(|c| c.is_ascii_hexdigit()));

        let autre = StandaloneExporter::export_html(&jeu, "Autre titre", "Testeur");
        assert_ne!(fa, StandaloneExporter::export_fingerprint(&autre));
    }
}
