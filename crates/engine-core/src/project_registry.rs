//! Registre de Projets & Scènes KaractiRage (Section 1.2 de la spécification).
//!
//! Ce module est l'autorité unique de découverte, de création et de gestion des
//! projets sur disque. Il est consommé à la fois par le Hub Canvas (`game.rs`) et
//! par les commandes Tauri du Studio React — une seule logique, zéro duplication.
//!
//! Les « projets récents » sont persistés dans `$KARACTI_HOME/projects.json`
//! (par défaut `~/.karactirage/projects.json`). Retirer un projet de la liste des
//! récents **ne supprime jamais** ses fichiers (spécification §1.2).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::project::KaractiProject;
use crate::recipe::{GameDimension, GameGenre};
use crate::scene::SceneData;

/// Version de schéma du fichier de registre des projets récents.
pub const REGISTRY_SCHEMA_VERSION: u32 = 1;
/// Nombre maximal de projets conservés dans la liste des récents.
pub const MAX_RECENT: usize = 12;
/// Nom du fichier de registre (récents) au sein du répertoire `$KARACTI_HOME`.
pub const REGISTRY_FILE: &str = "projects.json";

/// Nature d'une entrée du hub : scène de démonstration ou projet souverain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Scène `.scene.json` (démo officielle, lecture directe dans le moteur).
    Scene,
    /// Projet `.karacti` (métadonnées + scène + graphes KVL + recette).
    Project,
}

/// État de disponibilité d'une entrée sur le disque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    /// Le fichier existe et est lisible.
    Available,
    /// Le fichier référencé par les récents a disparu ou a été déplacé.
    Missing,
}

/// Métadonnées d'une entrée de projet affichées dans le Hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    /// Chemin canonique (absolu) du fichier sur le disque.
    pub path: String,
    pub title: String,
    pub description: String,
    pub objective: Option<String>,
    /// Libellé lisible du genre principal (ex. « Aventure »).
    pub genre: String,
    /// Libellé de dimension : « 2D », « 2.5D » ou « 3D ».
    pub dimension: String,
    pub kind: EntryKind,
    /// Taille réelle du fichier en octets (lus via `fs::metadata`).
    pub size_bytes: u64,
    /// Date de dernière modification en secondes depuis l'époque UNIX.
    pub modified_at: u64,
    /// Nombre d'entités de la scène (alimente la miniature procédurale).
    pub entity_count: u32,
    /// Vrai si le chemin figure dans la liste des projets récents.
    pub is_recent: bool,
    pub status: EntryStatus,
}
impl ProjectEntry {
    fn genre_label(genre: GameGenre) -> String {
        genre.label().to_string()
    }

    fn dimension_label(dimension: GameDimension) -> String {
        match dimension {
            GameDimension::TwoD => "2D",
            GameDimension::TwoAndHalfD => "2.5D",
            GameDimension::ThreeD => "3D",
        }
        .to_string()
    }

    fn genre_from_name(name: &str) -> GameGenre {
        let lower = name.to_lowercase();
        if lower.contains("platformer") || lower.contains("plateforme") {
            GameGenre::Platformer
        } else if lower.contains("space") || lower.contains("shmup") || lower.contains("stellaire") {
            GameGenre::ShootEmUp
        } else if lower.contains("puzzle") || lower.contains("sokoban") || lower.contains("labyrinthe") {
            GameGenre::Puzzle
        } else if lower.contains("boss") || lower.contains("titan") || lower.contains("arène") || lower.contains("arene") {
            GameGenre::Combat
        } else {
            GameGenre::Adventure
        }
    }

    /// Construit une entrée à partir d'un chemin de fichier, en lisant réellement
    /// le contenu et les métadonnées du disque.
    pub fn read_from_path(path: &Path, is_recent: bool) -> Self {
        let path_str = path.display().to_string();

        let (size_bytes, modified_at) = fs::metadata(path)
            .map(|m| {
                let modified = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (m.len(), modified)
            })
            .unwrap_or((0, 0));

        let is_scene = path_str.ends_with(".scene.json");
        let mut entry = if is_scene {
            Self::read_scene(path)
        } else {
            Self::read_project(path)
        };

        entry.path = path_str;
        entry.kind = if is_scene { EntryKind::Scene } else { EntryKind::Project };
        entry.size_bytes = size_bytes;
        entry.modified_at = modified_at;
        entry.is_recent = is_recent;
        entry.status = if path.exists() { EntryStatus::Available } else { EntryStatus::Missing };
        entry
    }

    fn read_scene(path: &Path) -> Self {
        match SceneData::load_from_file(path) {
            Ok(scene) => {
                let genre = Self::genre_from_name(&scene.name);
                Self {
                    path: String::new(),
                    title: scene.name.clone(),
                    description: scene.description.clone(),
                    objective: scene.objective.clone(),
                    genre: Self::genre_label(genre),
                    dimension: "2D".to_string(),
                    kind: EntryKind::Scene,
                    size_bytes: 0,
                    modified_at: 0,
                    entity_count: scene.entities.len() as u32,
                    is_recent: false,
                    status: EntryStatus::Available,
                }
            }
            Err(_) => Self::unreadable(path, EntryKind::Scene),
        }
    }

    fn read_project(path: &Path) -> Self {
        match KaractiProject::load_from_file(path) {
            Ok(project) => {
                let dimension = project
                    .metadata
                    .dimension
                    .clone()
                    .or_else(|| project.recipe.as_ref().map(|r| Self::dimension_label(r.dimension)))
                    .unwrap_or_else(|| "2D".to_string());
                Self {
                    path: String::new(),
                    title: project.metadata.title.clone(),
                    description: project.metadata.description.clone(),
                    objective: project.scene.objective.clone(),
                    genre: Self::genre_label(project.metadata.genre_primary),
                    dimension,
                    kind: EntryKind::Project,
                    size_bytes: 0,
                    modified_at: 0,
                    entity_count: project.scene.entities.len() as u32,
                    is_recent: false,
                    status: EntryStatus::Available,
                }
            }
            Err(_) => Self::unreadable(path, EntryKind::Project),
        }
    }

    fn unreadable(path: &Path, kind: EntryKind) -> Self {
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Projet illisible".to_string());
        Self {
            path: path.display().to_string(),
            title,
            description: "Fichier illisible ou corrompu.".to_string(),
            objective: None,
            genre: "Inconnu".to_string(),
            dimension: "2D".to_string(),
            kind,
            size_bytes: 0,
            modified_at: 0,
            entity_count: 0,
            is_recent: false,
            status: EntryStatus::Available,
        }
    }
}
/// Configuration de création d'un nouveau projet depuis le Hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProjectConfig {
    pub title: String,
    #[serde(default = "default_author")]
    pub author: String,
    pub genre: GameGenre,
    #[serde(default = "default_dimension")]
    pub dimension: GameDimension,
    /// Identifiant d'archétype optionnel (`rpg`, `platformer`, `shmup`, `sokoban`, `boss`).
    #[serde(default)]
    pub archetype: Option<String>,
    /// Dossier de destination (créé si absent).
    #[serde(default = "default_destination")]
    pub destination_dir: String,
    #[serde(default = "default_default_mode")]
    pub default_mode: String,
}

fn default_author() -> String {
    "Créateur KaractiRage".to_string()
}

fn default_dimension() -> GameDimension {
    GameDimension::TwoD
}

fn default_destination() -> String {
    ProjectRegistry::home_dir().join("projects").display().to_string()
}

fn default_default_mode() -> String {
    "edit".to_string()
}

impl NewProjectConfig {
    pub fn new(title: &str, genre: GameGenre) -> Self {
        Self {
            title: title.to_string(),
            author: default_author(),
            genre,
            dimension: GameDimension::TwoD,
            archetype: None,
            destination_dir: default_destination(),
            default_mode: "edit".to_string(),
        }
    }
}
/// Registre persistant des projets récents + point d'entrée de découverte disque.
pub struct ProjectRegistry {
    pub home: PathBuf,
    pub recent: Vec<String>,
}

/// Représentation sérialisée du fichier `projects.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryFile {
    schema_version: u32,
    #[serde(default)]
    recent: Vec<String>,
}

impl ProjectRegistry {
    /// Résout le répertoire de données KaractiRage.
    ///
    /// Priorité : `$KARACTI_HOME` → `$HOME/.karactirage` (ou `$APPDATA`/`$USERPROFILE`
    /// sous Windows) → répertoire temporaire (repli sans écriture système).
    pub fn home_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("KARACTI_HOME") {
            if !dir.trim().is_empty() {
                return PathBuf::from(dir);
            }
        }

        #[cfg(target_os = "windows")]
        let base = std::env::var("APPDATA")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var("HOME").map(PathBuf::from);

        match base {
            Ok(b) if !b.as_os_str().is_empty() => b.join(".karactirage"),
            _ => std::env::temp_dir().join(".karactirage"),
        }
    }

    /// Ouvre (ou initialise) le registre dans le répertoire par défaut.
    pub fn open() -> Self {
        Self::open_at(Self::home_dir())
    }

    /// Ouvre le registre dans un répertoire précis (utilisé par les tests).
    pub fn open_at(home: PathBuf) -> Self {
        let mut registry = Self { home, recent: Vec::new() };
        registry.load();
        registry
    }

    pub fn registry_path(&self) -> PathBuf {
        self.home.join(REGISTRY_FILE)
    }

    fn load(&mut self) {
        self.recent.clear();
        if let Ok(content) = fs::read_to_string(self.registry_path()) {
            if let Ok(file) = serde_json::from_str::<RegistryFile>(&content) {
                self.recent = file.recent;
            }
        }
        // Dédoublonnage défensif et plafonnement.
        let mut seen = HashSet::new();
        self.recent.retain(|p| seen.insert(p.clone()));
        self.recent.truncate(MAX_RECENT);
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let file = RegistryFile {
            schema_version: REGISTRY_SCHEMA_VERSION,
            recent: self.recent.clone(),
        };
        fs::create_dir_all(&self.home)?;
        let json = serde_json::to_string_pretty(&file)?;
        fs::write(self.registry_path(), json)?;
        Ok(())
    }

    /// Enregistre un chemin en tête de liste des récents (dédupliqué, plafonné).
    pub fn record_recent(&mut self, path: &str) {
        let canonical = canonicalize_lossy(path);
        self.recent.retain(|p| p != &canonical);
        self.recent.insert(0, canonical);
        self.recent.truncate(MAX_RECENT);
    }

    /// Retire un chemin de la liste des récents **sans toucher au fichier**.
    /// Retourne `true` si le chemin était présent.
    pub fn remove_recent(&mut self, path: &str) -> bool {
        let canonical = canonicalize_lossy(path);
        let before = self.recent.len();
        self.recent.retain(|p| p != &canonical);
        self.recent.len() != before
    }

    /// Découvre les projets et scènes de démonstration dans les sources par défaut.
    pub fn scan_default_sources(&self) -> Vec<ProjectEntry> {
        let mut dirs: Vec<PathBuf> = scene_source_dirs();
        let home_projects = self.home.join("projects");
        if !dirs.contains(&home_projects) {
            dirs.push(home_projects);
        }

        let mut result = self.scan(&dirs);
        let known: HashSet<String> =
            result.iter().map(|e| canonicalize_lossy(&e.path)).collect();

        // Toujours sonder les chemins récents (même hors des racines ou disparus).
        for path in &self.recent {
            let canonical = canonicalize_lossy(path);
            if !known.contains(&canonical) {
                let p = PathBuf::from(path);
                result.push(ProjectEntry::read_from_path(&p, true));
            }
        }
        result.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        result
    }

    /// Scan récursif d'une liste de répertoires, dédupliqué par chemin canonique.
    pub fn scan(&self, dirs: &[PathBuf]) -> Vec<ProjectEntry> {
        let recent_set: HashSet<String> = self.recent.iter().cloned().collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();

        for dir in dirs {
            if dir.exists() {
                collect_entries(dir, &recent_set, &mut seen, &mut result);
            }
        }

        result.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        result
    }

    /// Crée un nouveau projet sur disque, l'enregistre dans les récents et retourne
    /// son entrée de Hub.
    pub fn create_project(&mut self, config: &NewProjectConfig) -> Result<ProjectEntry, String> {
        let title = config.title.trim();
        if title.is_empty() {
            return Err("Le nom du projet ne peut pas être vide.".to_string());
        }

        // 1. Construction du projet (archétype prêt-à-jouer ou projet vierge Studio).
        let mut project = match config.archetype.as_deref() {
            Some(arch) if KaractiProject::from_archetype(arch).is_some() => {
                let mut p = KaractiProject::from_archetype(arch).unwrap();
                p.metadata.title = title.to_string();
                p.metadata.author = config.author.clone();
                p.metadata.description =
                    format!("Projet '{}' créé depuis l'archétype {}.", title, arch);
                p.metadata.dimension = Some(ProjectEntry::dimension_label(config.dimension));
                p.settings.default_mode = config.default_mode.clone();
                p
            }
            _ => {
                let mut p = KaractiProject::new_empty(title, &config.author, config.genre);
                p.metadata.dimension = Some(ProjectEntry::dimension_label(config.dimension));
                p.settings.default_mode = config.default_mode.clone();
                p
            }
        };

        // 2. Résolution du chemin de destination (dédupliqué). Un chemin vide
        //    bascule vers le dossier projets par défaut de l'utilisateur.
        let dest = if config.destination_dir.trim().is_empty() {
            self.home.join("projects")
        } else {
            PathBuf::from(&config.destination_dir)
        };
        fs::create_dir_all(&dest)
            .map_err(|e| format!("Impossible de créer le dossier '{}': {}", dest.display(), e))?;
        let file_path = unique_file_path(&dest, &slugify(title), "karacti");

        // 3. Métadonnées de traçabilité (date de création réelle + id stable).
        let now = now_epoch_secs();
        project.metadata.created_at = format_timestamp(now);
        project.metadata.id = format!("proj_{:x}", now ^ (title.len() as u64));

        project
            .save_to_file(&file_path)
            .map_err(|e| format!("Échec de sauvegarde du projet : {}", e))?;

        self.record_recent(&file_path.display().to_string());
        let _ = self.save();

        Ok(ProjectEntry::read_from_path(&file_path, true))
    }

    /// Duplique un projet (ou une scène) sous un nouveau titre, sans altérer l'original.
    pub fn duplicate_project(&mut self, path: &str, new_title: &str) -> Result<ProjectEntry, String> {
        let src = PathBuf::from(path);
        if !src.exists() {
            return Err(format!("Le fichier '{}' est introuvable.", path));
        }
        let title = new_title.trim();
        if title.is_empty() {
            return Err("Le nouveau nom ne peut pas être vide.".to_string());
        }

        let parent = src.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let is_scene = path.ends_with(".scene.json");

        let dest = if is_scene {
            let d = unique_file_path(&parent, &slugify(title), "scene.json");
            fs::copy(&src, &d).map_err(|e| format!("Échec de duplication : {}", e))?;
            d
        } else {
            let mut project = KaractiProject::load_from_file(&src)
                .map_err(|e| format!("Projet illisible : {}", e))?;
            project.metadata.title = title.to_string();
            let d = unique_file_path(&parent, &slugify(title), "karacti");
            project.save_to_file(&d).map_err(|e| format!("Échec de sauvegarde : {}", e))?;
            d
        };

        self.record_recent(&dest.display().to_string());
        let _ = self.save();
        Ok(ProjectEntry::read_from_path(&dest, true))
    }

    /// Renomme un projet souverain (`.karacti`) en mettant à jour ses métadonnées.
    /// Les scènes de démonstration ne sont pas modifiables en place.
    pub fn rename_project(&mut self, path: &str, new_title: &str) -> Result<ProjectEntry, String> {
        if path.ends_with(".scene.json") {
            return Err(
                "Les scènes de démonstration ne peuvent pas être renommées — dupliquez-les d'abord."
                    .to_string(),
            );
        }
        let src = PathBuf::from(path);
        let mut project = KaractiProject::load_from_file(&src)
            .map_err(|e| format!("Projet illisible : {}", e))?;
        let title = new_title.trim();
        if title.is_empty() {
            return Err("Le nouveau nom ne peut pas être vide.".to_string());
        }
        project.metadata.title = title.to_string();
        project.save_to_file(&src).map_err(|e| format!("Échec de sauvegarde : {}", e))?;
        Ok(ProjectEntry::read_from_path(&src, true))
    }

    /// Archive un fichier projet en le déplaçant vers `$KARACTI_HOME/archive/`.
    /// Ne supprime jamais de données.
    pub fn archive_project(&mut self, path: &str) -> Result<PathBuf, String> {
        let src = PathBuf::from(path);
        if !src.exists() {
            return Err(format!("Le fichier '{}' est introuvable.", path));
        }
        let archive_dir = self.home.join("archive");
        fs::create_dir_all(&archive_dir)
            .map_err(|e| format!("Impossible de créer le dossier d'archive : {}", e))?;
        let file_name = src
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "projet.karacti".to_string());
        let base = file_name.trim_end_matches(".karacti");
        let dest = unique_file_path(&archive_dir, base, "karacti");
        fs::rename(&src, &dest).map_err(|e| format!("Échec d'archivage : {}", e))?;
        self.remove_recent(path);
        let _ = self.save();
        Ok(dest)
    }
}

/// Collecte récursivement les fichiers projet/scène d'un répertoire.
fn collect_entries(
    dir: &Path,
    recent_set: &HashSet<String>,
    seen: &mut HashSet<String>,
    out: &mut Vec<ProjectEntry>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for item in entries.flatten() {
        let path = item.path();
        if path.is_dir() {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name != "archive" {
                collect_entries(&path, recent_set, seen, out);
            }
            continue;
        }
        let name = path.to_string_lossy().to_string();
        let is_scene = name.ends_with(".scene.json");
        let is_project = name.ends_with(".karacti");
        if !is_scene && !is_project {
            continue;
        }
        let canonical = canonicalize_lossy(&name);
        if !seen.insert(canonical.clone()) {
            continue;
        }
        out.push(ProjectEntry::read_from_path(&path, recent_set.contains(&canonical)));
    }
}

/// Résout les répertoires de scènes de démonstration (racine du dépôt).
fn scene_source_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for candidate in [
        PathBuf::from("assets/scenes"),
        PathBuf::from(format!("{}/../../assets/scenes", env!("CARGO_MANIFEST_DIR"))),
    ] {
        if candidate.exists() && !dirs.contains(&candidate) {
            dirs.push(candidate);
        }
    }
    dirs
}

/// Canonicalise un chemin, avec repli sur le chemin d'origine si la
/// canonicalisation échoue (fichier inexistant).
fn canonicalize_lossy(path: &str) -> String {
    fs::canonicalize(path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}

/// Génère un chemin de fichier unique en suffixant `-2`, `-3`, ... si nécessaire.
fn unique_file_path(dir: &Path, base: &str, extension: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{}.{}", base, extension));
    let mut i = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{}-{}.{}", base, i, extension));
        i += 1;
    }
    candidate
}

/// Convertit un titre lisible en nom de fichier sûr.
pub fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                slug.push(c);
            }
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "projet".to_string()
    } else {
        slug
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Formate une taille en octets de manière lisible (« 36 Ko », « 1.2 Mo »).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} o", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} Ko", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} Mo", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Formate un timestamp UNIX (secondes) en « AAAA-MM-JJ HH:MM » sans dépendance
/// externe (algorithme civil-from-days de Howard Hinnant).
pub fn format_timestamp(epoch: u64) -> String {
    let days = (epoch / 86_400) as i64;
    let secs_of_day = epoch % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hour, minute)
}

/// Convertit un nombre de jours depuis l'époque UNIX en (année, mois, jour).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "karactirage_registry_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Mon Projet — Édition Finale!"), "mon-projet-édition-finale");
        assert_eq!(slugify("   "), "projet");
        assert_eq!(slugify("Odyssée 2D"), "odyssée-2d");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 o");
        assert_eq!(format_bytes(1023), "1023 o");
        assert_eq!(format_bytes(1024), "1.0 Ko");
        assert_eq!(format_bytes(2_621_440), "2.5 Mo");
    }

    #[test]
    fn test_format_timestamp_epoch() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
        assert_eq!(format_timestamp(86_400), "1970-01-02 00:00");
        // 2026-10-20 12:34:56 UTC
        assert_eq!(format_timestamp(1_792_499_696), "2026-10-20 12:34");
    }

    #[test]
    fn test_registry_record_and_remove_recent_are_non_destructive() {
        let home = temp_home("recent");
        let mut reg = ProjectRegistry::open_at(home.clone());
        let proj = home.join("projects").join("mon-jeu.karacti");

        reg.record_recent(&proj.display().to_string());
        assert_eq!(reg.recent.len(), 1);
        assert!(reg.recent[0].ends_with("mon-jeu.karacti"));

        assert!(reg.remove_recent(&proj.display().to_string()));
        assert!(reg.recent.is_empty());
        assert!(!proj.exists(), "remove_recent ne doit jamais créer le fichier");

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn test_registry_persistence_roundtrip() {
        let home = temp_home("persist");
        let mut reg = ProjectRegistry::open_at(home.clone());
        reg.record_recent("/tmp/fake-project.karacti");
        reg.save().unwrap();

        let reloaded = ProjectRegistry::open_at(home.clone());
        assert_eq!(reloaded.recent, reg.recent);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn test_create_project_writes_file_and_returns_entry() {
        let home = temp_home("create");
        let dest = home.join("projects");
        let mut reg = ProjectRegistry::open_at(home.clone());
        let cfg = NewProjectConfig {
            destination_dir: dest.display().to_string(),
            ..NewProjectConfig::new("Mon Donjon", GameGenre::Adventure)
        };

        let entry = reg.create_project(&cfg).expect("création projet");
        assert_eq!(entry.title, "Mon Donjon");
        assert_eq!(entry.kind, EntryKind::Project);
        assert_eq!(entry.status, EntryStatus::Available);
        assert!(entry.path.ends_with(".karacti"));
        assert!(entry.is_recent);
        assert!(Path::new(&entry.path).exists());
        assert_eq!(entry.genre, "Aventure");
        assert_eq!(entry.dimension, "2D");

        let loaded = KaractiProject::load_from_file(&entry.path).unwrap();
        assert_eq!(loaded.metadata.title, "Mon Donjon");
        assert_eq!(loaded.metadata.dimension.as_deref(), Some("2D"));

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn test_create_project_with_archetype() {
        let home = temp_home("archetype");
        let dest = home.join("projects");
        let mut reg = ProjectRegistry::open_at(home.clone());
        let cfg = NewProjectConfig {
            destination_dir: dest.display().to_string(),
            archetype: Some("platformer".to_string()),
            ..NewProjectConfig::new("Super Sauts", GameGenre::Platformer)
        };

        let entry = reg.create_project(&cfg).expect("création archétype");
        let loaded = KaractiProject::load_from_file(&entry.path).unwrap();
        assert!(loaded.recipe.is_some(), "L'archétype doit embarquer sa recette");
        assert!(loaded.scene.entities.iter().any(|e| e.tag == "player"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn test_duplicate_rename_and_archive_project() {
        let home = temp_home("lifecycle");
        let dest = home.join("projects");
        let mut reg = ProjectRegistry::open_at(home.clone());
        let cfg = NewProjectConfig {
            destination_dir: dest.display().to_string(),
            ..NewProjectConfig::new("Original", GameGenre::Puzzle)
        };
        let original = reg.create_project(&cfg).unwrap();

        let dup = reg.duplicate_project(&original.path, "Copie").unwrap();
        assert_eq!(dup.title, "Copie");
        assert!(Path::new(&dup.path).exists());
        assert_ne!(dup.path, original.path);

        let renamed = reg.rename_project(&dup.path, "Copie Renommée").unwrap();
        assert_eq!(renamed.title, "Copie Renommée");
        let loaded = KaractiProject::load_from_file(&dup.path).unwrap();
        assert_eq!(loaded.metadata.title, "Copie Renommée");

        let archived = reg.archive_project(&dup.path).unwrap();
        assert!(!Path::new(&dup.path).exists());
        assert!(archived.exists());
        assert!(archived.display().to_string().contains("archive"));

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn test_scan_detects_scenes_and_missing_recent() {
        let home = temp_home("scan");
        let mut reg = ProjectRegistry::open_at(home.clone());

        let scenes = home.join("scenes");
        fs::create_dir_all(&scenes).unwrap();
        let scene_path = scenes.join("demo_01.scene.json");
        let scene_json = r#"{"name":"Démo Test","description":"Une scène","entities":[]}"#;
        fs::write(&scene_path, scene_json).unwrap();

        let missing = home.join("disparu.karacti");
        reg.record_recent(&missing.display().to_string());

        let entries = reg.scan(&[scenes]);
        assert!(entries.iter().any(|e| e.title == "Démo Test" && e.kind == EntryKind::Scene));

        let all = reg.scan_default_sources();
        let m = all
            .iter()
            .find(|e| e.path == missing.display().to_string())
            .expect("récent manquant présent");
        assert_eq!(m.status, EntryStatus::Missing);

        let _ = fs::remove_dir_all(&home);
    }
}
// __CHUNK_END__
