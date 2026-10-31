//! Manifeste d'un plugin (pack de contenu, node pack ou extension native).
//!
//! Le manifeste est le contrat public du système de plugins : identifiant, version,
//! version d'API hôte, type, capacités, déterminisme, budget par frame et contenu.
//! Toute la validation « statique » vit ici ; le chargement effectif des fichiers est
//! dans [`super::host`].

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Version de l'API hôte comprise par ce binaire.
pub const PLUGIN_API_VERSION: u32 = 1;

/// Budget maximal déclarable par un plugin runtime, en microsecondes par frame.
pub const MAX_PLUGIN_BUDGET_US: u32 = 5_000;

/// Nature du plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginKind {
    /// Données uniquement : scènes, graphes KVL, prefabs, tilesets. Aucun code.
    Content,
    /// Nœuds KVL compilés dans le moteur (schémas + handlers).
    NodePack,
    /// Extension native compilée (systèmes, importeurs, rendu).
    Native,
}

impl PluginKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Content => "contenu",
            Self::NodePack => "node pack",
            Self::Native => "extension native",
        }
    }

    /// Les plugins qui s'exécutent dans la simulation doivent être déterministes.
    pub fn requires_determinism(self) -> bool {
        !matches!(self, Self::Content)
    }
}

/// Capacité déclarée par un plugin. Une capacité non déclarée ne doit pas être utilisée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Lire les fichiers de son propre pack.
    ContentRead,
    /// Lire/écrire hors du pack.
    Filesystem,
    /// Accès réseau.
    Network,
    /// Audio.
    Audio,
    /// Entrées.
    Input,
    /// Rendu.
    Render,
    /// Édition (Studio).
    Editor,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Self::ContentRead => "content-read",
            Self::Filesystem => "filesystem",
            Self::Network => "network",
            Self::Audio => "audio",
            Self::Input => "input",
            Self::Render => "render",
            Self::Editor => "editor",
        }
    }
}

/// Fichiers de contenu déclarés par un pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentManifest {
    #[serde(default)]
    pub scenes: Vec<String>,
    #[serde(default)]
    pub graphs: Vec<String>,
    #[serde(default)]
    pub prefabs: Vec<String>,
    #[serde(default)]
    pub tilesets: Vec<String>,
}

impl ContentManifest {
    pub fn all_paths(&self) -> impl Iterator<Item = &String> {
        self.scenes
            .iter()
            .chain(self.graphs.iter())
            .chain(self.prefabs.iter())
            .chain(self.tilesets.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
            && self.graphs.is_empty()
            && self.prefabs.is_empty()
            && self.tilesets.is_empty()
    }
}

/// Manifeste `plugin.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Identifiant unique, ex. `karacti.pack.platformer`.
    pub id: String,
    pub name: String,
    /// Version sémantique `x.y.z`.
    pub version: String,
    /// Version d'API hôte exigée (défaut 0 : refusée).
    #[serde(default)]
    pub api: u32,
    pub kind: PluginKind,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// Obligatoire pour `node-pack` et `native`.
    #[serde(default)]
    pub deterministic: bool,
    /// Budget déclaré par frame (µs). 0 pour un pack de contenu.
    #[serde(default)]
    pub budget_us: u32,
    /// Identifiants de plugins requis.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Un plugin désactivé est ignoré au chargement.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub content: ContentManifest,
}

fn default_true() -> bool {
    true
}

impl PluginManifest {
    /// Constructeur pratique pour le noyau et les tests.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        kind: PluginKind,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            api: PLUGIN_API_VERSION,
            kind,
            description: String::new(),
            capabilities: Vec::new(),
            deterministic: false,
            budget_us: 0,
            dependencies: Vec::new(),
            enabled: true,
            content: ContentManifest::default(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    pub fn with_dependency(mut self, id: impl Into<String>) -> Self {
        self.dependencies.push(id.into());
        self
    }

    /// Valide les invariants du manifeste (sans toucher au disque).
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.trim().is_empty() {
            return Err(ManifestError::EmptyName);
        }
        validate_id(&self.id)?;
        validate_version(&self.version)?;

        if self.api > PLUGIN_API_VERSION {
            return Err(ManifestError::UnsupportedApi {
                found: self.api,
                supported: PLUGIN_API_VERSION,
            });
        }
        if self.api == 0 {
            return Err(ManifestError::MissingApi);
        }

        if self.budget_us > MAX_PLUGIN_BUDGET_US {
            return Err(ManifestError::BudgetTooHigh {
                budget_us: self.budget_us,
                max_us: MAX_PLUGIN_BUDGET_US,
            });
        }

        if self.kind.requires_determinism() && !self.deterministic {
            return Err(ManifestError::NonDeterministicRuntime {
                kind: self.kind,
            });
        }

        if self.kind == PluginKind::Content {
            if let Some(capability) = self
                .capabilities
                .iter()
                .find(|c| **c != Capability::ContentRead)
            {
                return Err(ManifestError::CapabilityNotAllowed {
                    capability: *capability,
                    kind: self.kind,
                });
            }
            if self.budget_us != 0 {
                return Err(ManifestError::ContentBudget { budget_us: self.budget_us });
            }
        }

        for path in self.content.all_paths() {
            if !is_safe_relative_path(path) {
                return Err(ManifestError::UnsafeContentPath(path.clone()));
            }
        }

        for dependency in &self.dependencies {
            validate_id(dependency)?;
            if dependency == &self.id {
                return Err(ManifestError::SelfDependency(self.id.clone()));
            }
        }

        Ok(())
    }

    pub fn is_content(&self) -> bool {
        self.kind == PluginKind::Content
    }
}

/// Erreurs de validation d'un manifeste.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ManifestError {
    #[error("nom de plugin vide")]
    EmptyName,
    #[error("identifiant de plugin invalide : {0}")]
    InvalidId(String),
    #[error("version invalide : {0} (attendu : x.y.z)")]
    InvalidVersion(String),
    #[error("api absente (le champ 'api' est obligatoire)")]
    MissingApi,
    #[error("api {found} non supportée par ce moteur (max {supported})")]
    UnsupportedApi { found: u32, supported: u32 },
    #[error("budget {budget_us} µs supérieur au maximum {max_us} µs")]
    BudgetTooHigh { budget_us: u32, max_us: u32 },
    #[error("un plugin de type {kind:?} doit déclarer deterministic: true")]
    NonDeterministicRuntime { kind: PluginKind },
    #[error("capacité {capability:?} interdite pour un plugin de type {kind:?}")]
    CapabilityNotAllowed {
        capability: Capability,
        kind: PluginKind,
    },
    #[error("un pack de contenu ne peut pas déclarer de budget ({budget_us} µs)")]
    ContentBudget { budget_us: u32 },
    #[error("chemin de contenu interdit : {0} (relatif, sans '..', sans absolu)")]
    UnsafeContentPath(String),
    #[error("un plugin ne peut pas dépendre de lui-même : {0}")]
    SelfDependency(String),
}

/// Identifiant : minuscules, chiffres, `.`, `_`, `-` ; commence et finit par un
/// caractère alphanumérique ; pas de `..`.
pub fn validate_id(id: &str) -> Result<(), ManifestError> {
    if id.is_empty() || id.len() > 64 {
        return Err(ManifestError::InvalidId(id.to_string()));
    }
    let valid_chars = id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    let first = id.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false);
    let last = id.chars().last().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false);
    if !valid_chars || !first || !last || id.contains("..") {
        return Err(ManifestError::InvalidId(id.to_string()));
    }
    Ok(())
}

/// Version `x.y.z`, chaque composante numérique.
pub fn validate_version(version: &str) -> Result<(), ManifestError> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(ManifestError::InvalidVersion(version.to_string()));
    }
    Ok(())
}

/// Un chemin de contenu doit rester **dans** le dossier du pack.
pub fn is_safe_relative_path(path: &str) -> bool {
    if path.trim().is_empty() || path.contains('\\') {
        return false;
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return false;
    }
    let mut depth = 0i32;
    for component in candidate.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    depth > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content_manifest() -> PluginManifest {
        PluginManifest::new(
            "karacti.pack.platformer",
            "Pack plateforme",
            "0.1.0",
            PluginKind::Content,
        )
        .with_capability(Capability::ContentRead)
    }

    #[test]
    fn valid_content_manifest_passes() {
        content_manifest().validate().expect("manifeste valide");
    }

    #[test]
    fn id_rules_are_enforced() {
        for bad in ["", "Karacti.Pack", ".pack", "pack.", "a..b", "a b", "a/b"] {
            let mut manifest = content_manifest();
            manifest.id = bad.to_string();
            assert!(
                matches!(manifest.validate(), Err(ManifestError::InvalidId(_))),
                "id accepté à tort : {:?}",
                bad
            );
        }
        for good in ["a", "karacti.pack.platformer", "karacti-pack_2"] {
            let mut manifest = content_manifest();
            manifest.id = good.to_string();
            manifest.validate().expect("id valide");
        }
    }

    #[test]
    fn version_must_be_semver_triplet() {
        for bad in ["", "1", "1.2", "1.2.3.4", "1.2.x", "v1.2.3"] {
            let mut manifest = content_manifest();
            manifest.version = bad.to_string();
            assert!(manifest.validate().is_err(), "version acceptée à tort : {:?}", bad);
        }
    }

    #[test]
    fn api_must_be_present_and_supported() {
        let mut manifest = content_manifest();
        manifest.api = 0;
        assert_eq!(manifest.validate().unwrap_err(), ManifestError::MissingApi);

        manifest.api = PLUGIN_API_VERSION + 1;
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::UnsupportedApi { .. })
        ));
    }

    #[test]
    fn runtime_kinds_must_be_deterministic() {
        let manifest = PluginManifest::new("a.b", "A", "1.0.0", PluginKind::NodePack);
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::NonDeterministicRuntime { .. })
        ));
    }

    #[test]
    fn content_cannot_declare_runtime_capability_or_budget() {
        let mut manifest = content_manifest();
        manifest.capabilities.push(Capability::Network);
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::CapabilityNotAllowed { .. })
        ));

        let mut manifest = content_manifest();
        manifest.budget_us = 100;
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::ContentBudget { .. })
        ));
    }

    #[test]
    fn budget_is_capped() {
        let mut manifest = PluginManifest::new("a.b", "A", "1.0.0", PluginKind::Native);
        manifest.deterministic = true;
        manifest.budget_us = MAX_PLUGIN_BUDGET_US + 1;
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::BudgetTooHigh { .. })
        ));
    }

    #[test]
    fn content_paths_cannot_escape_pack() {
        for bad in ["../secret.json", "/etc/passwd", "a/../../b", "C:\\x", "..", ""] {
            assert!(!is_safe_relative_path(bad), "chemin accepté à tort : {:?}", bad);
        }
        for good in ["scenes/a.json", "./scenes/a.json", "graphs/bonus.kvl.json"] {
            assert!(is_safe_relative_path(good), "chemin refusé à tort : {:?}", good);
        }

        let mut manifest = content_manifest();
        manifest.content.scenes.push("../escape.json".into());
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::UnsafeContentPath(_))
        ));
    }

    #[test]
    fn dependencies_cannot_self_reference() {
        let mut manifest = content_manifest();
        manifest.dependencies.push(manifest.id.clone());
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::SelfDependency(_))
        ));
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let manifest = content_manifest().with_description("démo");
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, back);
        assert!(json.contains("\"kind\": \"content\""));
        assert!(json.contains("\"content-read\""));
    }
}
