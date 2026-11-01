//! Dictionnaire IA compilé — la source de vérité machine pour les prompts.
//!
//! Le dictionnaire est **généré depuis le code** (schémas `schemars`, catalogue de nœuds
//! `PluginHost`) et enrichi de fichiers curés (playbooks, exemples, capacités). Il est
//! immuable pendant une session et sa version est enregistrée dans chaque run.
//!
//! Règles d'or : le code est la vérité, le dictionnaire est généré ; une valeur absente du
//! dictionnaire n'existe pas.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use kvl_core::NodeFlavor;

/// Version du dictionnaire (majeure.à.minor) — bump à chaque changement de contrat.
pub const DICTIONARY_VERSION: &str = "2026.11.1";
/// Version d'API moteur que ce dictionnaire décrit. Incompatibilité → refus de générer.
pub const ENGINE_API: u32 = 1;

/// Manifeste du dictionnaire : version, dimensions, artefacts et hashes des fichiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryManifest {
    pub dictionary_version: String,
    pub engine_api: u32,
    pub dimensions: Vec<String>,
    pub artifacts: Vec<String>,
    /// Chemin relatif (slash) → hash hexadécimal déterministe.
    pub files: BTreeMap<String, String>,
}

/// Erreurs de construction ou de chargement du dictionnaire.
#[derive(Debug, Error)]
pub enum DictionaryError {
    #[error("source absente : {0}")]
    MissingSource(PathBuf),
    #[error("E/S : {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON invalide : {0}")]
    Json(#[from] serde_json::Error),
    #[error("manifeste : {0}")]
    Manifest(String),
    #[error("hash invalide pour {path} : attendu {expected}, obtenu {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("exemple {0} invalide pour le schéma : {1}")]
    InvalidExample(String, String),
    #[error("exemple {0} référence un type inconnu du catalogue : '{1}'")]
    UnknownCatalogType(String, String),
}

/// Dictionnaire chargé (manifeste vérifié + accès aux fichiers).
#[derive(Debug, Clone)]
pub struct AiDictionary {
    manifest: DictionaryManifest,
    root: PathBuf,
}

/// Hash FNV-1a 64 bits, déterministe et sans dépendance externe.
///
/// Suffisant pour la détection de dérive (`git diff --exit-code dist/ai`) et la
/// vérification d'intégrité. Pas cryptographique (ce n'est pas un secret).
pub fn hash_text(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn write_and_hash(
    out_dir: &Path,
    rel: &str,
    bytes: &[u8],
    files: &mut BTreeMap<String, String>,
) -> Result<(), DictionaryError> {
    let full = out_dir.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&full, bytes)?;
    files.insert(rel.to_string(), hash_text(bytes));
    Ok(())
}

fn read_sorted_files(dir: &Path, ext: &str) -> Result<Vec<(String, Vec<u8>)>, DictionaryError> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some(ext))
        .collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .expect("fichier avec nom")
            .to_string_lossy()
            .to_string();
        out.push((name, fs::read(&path)?));
    }
    Ok(out)
}

/// Vérifie que chaque action/événement d'un exemple existe dans le catalogue de nœuds.
fn validate_example_catalog(
    payload: &crate::ai_client::AiGeneratedGamePayload,
    host: &crate::plugin::PluginHost,
    name: &str,
) -> Result<(), DictionaryError> {
    for kvl in &payload.kvl_actions {
        match host.nodes().resolve(&kvl.event) {
            Some(resolved) if resolved.schema.flavor == NodeFlavor::Event => {}
            _ => {
                return Err(DictionaryError::UnknownCatalogType(
                    name.to_string(),
                    kvl.event.clone(),
                ))
            }
        }
        match host.nodes().resolve(&kvl.action) {
            Some(resolved) if resolved.schema.flavor == NodeFlavor::Action => {}
            _ => {
                return Err(DictionaryError::UnknownCatalogType(
                    name.to_string(),
                    kvl.action.clone(),
                ))
            }
        }
    }
    Ok(())
}

impl AiDictionary {
    /// Compile le dictionnaire depuis `source_root/ai` et l'écrit dans `out_dir`.
    ///
    /// Échoue si un exemple est invalide (schéma) ou référence un type absent du catalogue.
    pub fn build(source_root: &Path, out_dir: &Path) -> Result<DictionaryManifest, DictionaryError> {
        fs::create_dir_all(out_dir)?;

        let mut files = BTreeMap::new();

        // L2 — schémas `schemars` des types de patch (niveau = `AiGeneratedGamePayload`).
        let level_schema = schemars::schema_for!(crate::ai_client::AiGeneratedGamePayload);
        let level_value = serde_json::to_value(&level_schema)?;
        let level_json = serde_json::to_string_pretty(&level_value)? + "\n";
        write_and_hash(out_dir, "schemas/level.schema.json", level_json.as_bytes(), &mut files)?;

        // L2 — schéma du graphe KVL (vocabulaire des ops : nœuds, pins, connexions, variables).
        let graph_schema = schemars::schema_for!(kvl_core::Graph);
        let graph_value = serde_json::to_value(&graph_schema)?;
        let graph_json = serde_json::to_string_pretty(&graph_value)? + "\n";
        write_and_hash(out_dir, "schemas/graph.schema.json", graph_json.as_bytes(), &mut files)?;

        // L3 — catalogue de nœuds KVL (75) émis par l'hôte de plugins.
        let host = crate::plugin::PluginHost::with_builtins();
        let nodes_json = host.catalog_json()? + "\n";
        write_and_hash(out_dir, "nodes.json", nodes_json.as_bytes(), &mut files)?;

        // L1 — capacités (statut par artefact/dimension, limites connues).
        let caps_path = source_root.join("ai").join("capabilities.json");
        let caps = fs::read(&caps_path).map_err(|_| DictionaryError::MissingSource(caps_path))?;
        let _: serde_json::Value = serde_json::from_slice(&caps)?;
        write_and_hash(out_dir, "capabilities.json", &caps, &mut files)?;

        // L4 — playbooks (fichiers curés, non générés).
        for (name, content) in read_sorted_files(&source_root.join("ai").join("playbooks"), "md")? {
            write_and_hash(out_dir, &format!("playbooks/{name}"), &content, &mut files)?;
        }

        // L5 — exemples dorés, validés contre le schéma ET le catalogue.
        for (name, content) in read_sorted_files(&source_root.join("ai").join("examples"), "json")? {
            let payload: crate::ai_client::AiGeneratedGamePayload = serde_json::from_slice(&content)
                .map_err(|error| DictionaryError::InvalidExample(name.clone(), error.to_string()))?;
            validate_example_catalog(&payload, &host, &name)?;
            write_and_hash(out_dir, &format!("examples/{name}"), &content, &mut files)?;
        }

        let manifest = DictionaryManifest {
            dictionary_version: DICTIONARY_VERSION.to_string(),
            engine_api: ENGINE_API,
            dimensions: vec!["2d".into(), "2.5d".into(), "3d".into()],
            artifacts: vec![
                "project".into(),
                "scene".into(),
                "level".into(),
                "mechanic".into(),
                "sprite".into(),
                "mesh".into(),
                "audio".into(),
                "ui".into(),
            ],
            files,
        };

        let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
        fs::write(out_dir.join("manifest.json"), manifest_json)?;
        Ok(manifest)
    }

    /// Charge un dictionnaire compilé et vérifie version, `engine_api` et hashes.
    pub fn load(root: &Path) -> Result<Self, DictionaryError> {
        let manifest_path = root.join("manifest.json");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|_| DictionaryError::MissingSource(manifest_path))?;
        let manifest: DictionaryManifest = serde_json::from_str(&raw)?;

        if manifest.dictionary_version != DICTIONARY_VERSION {
            return Err(DictionaryError::Manifest(format!(
                "version {} non supportée (attendue {})",
                manifest.dictionary_version, DICTIONARY_VERSION
            )));
        }
        if manifest.engine_api != ENGINE_API {
            return Err(DictionaryError::Manifest(format!(
                "engine_api {} non supporté (attendu {})",
                manifest.engine_api, ENGINE_API
            )));
        }

        for (rel, expected) in &manifest.files {
            let content = fs::read(root.join(rel))
                .map_err(|_| DictionaryError::MissingSource(root.join(rel)))?;
            let actual = hash_text(&content);
            if &actual != expected {
                return Err(DictionaryError::HashMismatch {
                    path: rel.clone(),
                    expected: expected.clone(),
                    actual,
                });
            }
        }

        Ok(Self {
            manifest,
            root: root.to_path_buf(),
        })
    }

    pub fn manifest(&self) -> &DictionaryManifest {
        &self.manifest
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn read_file(&self, rel: &str) -> Result<String, DictionaryError> {
        Ok(fs::read_to_string(self.root.join(rel))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("karacti_ai_dict_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("création tmp");
        dir
    }

    fn source_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn build_is_deterministic() {
        let out1 = tmp_dir("det1");
        let out2 = tmp_dir("det2");
        let m1 = AiDictionary::build(&source_root(), &out1).expect("build 1");
        let m2 = AiDictionary::build(&source_root(), &out2).expect("build 2");

        assert_eq!(m1.files, m2.files, "manifeste différent entre deux builds");

        for rel in m1.files.keys() {
            let a = fs::read(out1.join(rel)).expect("fichier out1");
            let b = fs::read(out2.join(rel)).expect("fichier out2");
            assert_eq!(a, b, "fichier non déterministe : {rel}");
        }

        let _ = fs::remove_dir_all(&out1);
        let _ = fs::remove_dir_all(&out2);
    }

    #[test]
    fn load_verifies_hashes_and_rejects_corruption() {
        let out = tmp_dir("load");
        AiDictionary::build(&source_root(), &out).expect("build");

        assert!(AiDictionary::load(&out).is_ok());

        let rel = "schemas/level.schema.json";
        let path = out.join(rel);
        let mut content = fs::read_to_string(&path).expect("lecture");
        content.push_str("\n// corrupt\n");
        fs::write(&path, content).expect("écriture");

        match AiDictionary::load(&out) {
            Err(DictionaryError::HashMismatch { path, .. }) => assert_eq!(path, rel),
            other => panic!("attendu HashMismatch, obtenu {other:?}"),
        }

        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn catalog_completeness_and_ghost_values_absent() {
        let host = crate::plugin::PluginHost::with_builtins();
        assert!(
            host.nodes().len() >= 76,
            "catalogue de nœuds trop petit : {}",
            host.nodes().len()
        );

        // Parité schéma ↔ handler : chaque action du catalogue doit avoir un handler runtime.
        for schema in host.nodes().iter() {
            if schema.flavor == NodeFlavor::Action {
                let handled = host.handlers().is_registered(&schema.id)
                    || host.handlers().is_registered(&format!("{}:1", schema.id))
                    || schema
                        .prefixes
                        .iter()
                        .any(|prefix| host.handlers().is_registered(prefix));
                assert!(handled, "handler manquant pour l'action {}", schema.id);
            }
        }

        // Nœuds ajoutés pour fermer les trous du catalogue (fixtures + objectif).
        for valid in ["OnDamage", "SetObjective", "OnStart", "OnWinConditionMet"] {
            assert!(
                host.nodes().resolve(valid).is_some(),
                "type absent du catalogue : {valid}"
            );
        }

        // Les valeurs fantômes du prompt historique ne doivent PAS exister au catalogue.
        for ghost in ["WinGame", "OnEntityDefeated", "PlayerDeath"] {
            assert!(
                host.nodes().resolve(ghost).is_none(),
                "valeur fantôme encore présente au catalogue : {ghost}"
            );
        }
    }
}
